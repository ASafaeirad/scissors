//! Finds black bars in the selected source and loads them into the draft crop.

use super::preview_panel::{CropSourceDimensions, crop_size_label};
use super::*;
use frame_core::cropdetect::{self, DetectedCrop};

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) enum CropDetectState {
    #[default]
    Idle,
    /// Detection is running for this file id.
    Running(String),
    /// Detection finished without changing the crop. Holds the draft crop at that moment so
    /// the message clears as soon as the crop changes.
    Notice {
        draft: Option<CropRect>,
        message: &'static str,
    },
}

/// What the crop toolbar's status pill shows: detection progress or failure, otherwise the
/// crop's output size.
pub(in crate::app) fn crop_status_label(
    detect: &CropDetectState,
    draft_crop: Option<CropRect>,
    draft_size: Option<CropSourceDimensions>,
) -> Option<String> {
    match detect {
        CropDetectState::Running(_) => Some("Detecting black bars…".to_string()),
        CropDetectState::Notice { draft, message } if *draft == draft_crop => {
            Some((*message).to_string())
        }
        _ => draft_size.map(crop_size_label),
    }
}

/// Converts a detected crop in source pixels to a normalized draft crop rect.
pub(in crate::app) fn detected_crop_rect(crop: DetectedCrop, width: u32, height: u32) -> CropRect {
    let (width, height) = (f64::from(width.max(1)), f64::from(height.max(1)));
    clamp_rect(CropRect {
        x: f64::from(crop.x) / width,
        y: f64::from(crop.y) / height,
        width: f64::from(crop.width) / width,
        height: f64::from(crop.height) / height,
    })
}

/// Runs `cropdetect` over samples of `input`. Blocks; call it from a background task.
fn detect_black_bars(input: &str, duration: Option<f64>) -> Result<Option<DetectedCrop>, String> {
    let ffmpeg = crate::runtime_binaries::ffmpeg_executable();
    let samples = cropdetect::sample_times(duration)
        .into_iter()
        .map(|seek| {
            let output = std::process::Command::new(&ffmpeg)
                .args(cropdetect::cropdetect_args(input, seek))
                .output()
                .map_err(|error| error.to_string())?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !output.status.success() {
                return Err(stderr.lines().last().unwrap_or_default().to_string());
            }
            Ok(cropdetect::parse_cropdetect_output(&stderr))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cropdetect::merge_samples(samples))
}

impl FrameRoot {
    pub(super) fn crop_detection_available(&self) -> bool {
        self.preview_ui.crop_mode
            && !self.editing_project_output()
            && !matches!(self.preview_ui.crop_detect, CropDetectState::Running(_))
            && self.file_queue.selected_file().is_some()
            && self
                .selected_source_metadata()
                .is_some_and(|metadata| metadata.width.is_some() && metadata.height.is_some())
    }

    pub(super) fn detect_preview_crop_bars(&mut self, cx: &Context<Self>) -> bool {
        if !self.crop_detection_available() {
            return false;
        }
        let Some((file_id, path)) = self
            .file_queue
            .selected_file()
            .map(|file| (file.id.clone(), file.path.clone()))
        else {
            return false;
        };
        let duration = self
            .selected_source_metadata()
            .and_then(|metadata| metadata.duration)
            .as_deref()
            .and_then(frame_core::utils::parse_time);

        self.preview_ui.crop_detect = CropDetectState::Running(file_id.clone());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { detect_black_bars(&path, duration) })
                .await;
            this.update(cx, |root, cx| {
                if root.finish_crop_detection(&file_id, &result) {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        true
    }

    /// Applies a finished detection to the draft crop when crop mode is still open on the same
    /// file.
    pub(super) fn finish_crop_detection(
        &mut self,
        file_id: &str,
        result: &Result<Option<DetectedCrop>, String>,
    ) -> bool {
        if !matches!(&self.preview_ui.crop_detect, CropDetectState::Running(id) if id == file_id) {
            return false;
        }
        self.preview_ui.crop_detect = CropDetectState::Idle;
        let metadata = self.selected_source_metadata();
        let dimensions = metadata
            .as_ref()
            .and_then(|metadata| metadata.width.zip(metadata.height));
        let still_cropping =
            self.preview_ui.crop_mode && self.file_queue.selected_file_id() == Some(file_id);
        let (Some((width, height)), true) = (dimensions, still_cropping) else {
            return true;
        };

        let message = match result {
            Ok(Some(crop)) if crop.width < width || crop.height < height => {
                self.preview_ui.draft_crop = Some(detected_crop_rect(*crop, width, height));
                self.preview_ui.crop_aspect = "free".to_string();
                return true;
            }
            Ok(Some(_)) => "No black bars found",
            Ok(None) | Err(_) => "Couldn't detect black bars",
        };
        self.preview_ui.crop_detect = CropDetectState::Notice {
            draft: self.preview_ui.draft_crop,
            message,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_with_video() -> FrameRoot {
        let mut root = FrameRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );
        assert!(root.toggle_selected_crop_mode());
        root
    }

    const LETTERBOX: DetectedCrop = DetectedCrop {
        x: 0,
        y: 132,
        width: 1920,
        height: 816,
    };

    #[test]
    fn detected_crop_is_normalized_to_the_source_size() {
        let rect = detected_crop_rect(LETTERBOX, 1920, 1080);
        assert!((rect.x - 0.0).abs() < 1e-9);
        assert!((rect.y - 132.0 / 1080.0).abs() < 1e-9);
        assert!((rect.width - 1.0).abs() < 1e-9);
        assert!((rect.height - 816.0 / 1080.0).abs() < 1e-9);
    }

    #[test]
    fn finished_detection_replaces_the_draft_crop() {
        let mut root = root_with_video();
        root.preview_ui.crop_aspect = "16:9".to_string();
        root.preview_ui.crop_detect = CropDetectState::Running("video".to_string());

        assert!(root.finish_crop_detection("video", &Ok(Some(LETTERBOX))));

        assert_eq!(
            root.preview_ui.draft_crop,
            Some(detected_crop_rect(LETTERBOX, 1920, 1080))
        );
        assert_eq!(root.preview_ui.crop_aspect, "free");
        assert_eq!(root.preview_ui.crop_detect, CropDetectState::Idle);
        assert!(root.apply_selected_crop());
        let crop = root
            .file_queue
            .file_by_id("video")
            .unwrap()
            .config
            .crop
            .clone();
        let crop = crop.expect("crop settings");
        assert_eq!(
            (crop.x, crop.y, crop.width, crop.height),
            (0, 132, 1920, 816)
        );
    }

    #[test]
    fn full_frame_detection_keeps_the_draft_and_says_no_bars() {
        let mut root = root_with_video();
        let draft = root.preview_ui.draft_crop;
        root.preview_ui.crop_detect = CropDetectState::Running("video".to_string());
        let full = DetectedCrop {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        assert!(root.finish_crop_detection("video", &Ok(Some(full))));

        assert_eq!(root.preview_ui.draft_crop, draft);
        assert_eq!(
            crop_status_label(&root.preview_ui.crop_detect, draft, None).as_deref(),
            Some("No black bars found")
        );
    }

    #[test]
    fn failed_detection_reports_until_the_crop_changes() {
        let mut root = root_with_video();
        let draft = root.preview_ui.draft_crop;
        root.preview_ui.crop_detect = CropDetectState::Running("video".to_string());

        assert!(root.finish_crop_detection("video", &Err("No such file".to_string())));

        assert_eq!(root.preview_ui.draft_crop, draft);
        assert_eq!(
            crop_status_label(&root.preview_ui.crop_detect, draft, None).as_deref(),
            Some("Couldn't detect black bars")
        );
        assert!(root.reset_preview_crop_selection());
        assert_eq!(
            crop_status_label(
                &root.preview_ui.crop_detect,
                root.preview_ui.draft_crop,
                None
            ),
            None
        );
    }

    #[test]
    fn stale_detection_is_ignored_after_leaving_crop_mode() {
        let mut root = root_with_video();
        root.preview_ui.crop_detect = CropDetectState::Running("video".to_string());
        assert!(root.toggle_selected_crop_mode());

        assert!(root.finish_crop_detection("video", &Ok(Some(LETTERBOX))));

        assert_eq!(root.preview_ui.draft_crop, None);
        assert_eq!(root.preview_ui.crop_detect, CropDetectState::Idle);
        assert!(!root.finish_crop_detection("video", &Ok(Some(LETTERBOX))));
    }

    #[test]
    fn running_detection_shows_progress_and_blocks_a_second_run() {
        let mut root = root_with_video();
        assert!(root.crop_detection_available());
        root.preview_ui.crop_detect = CropDetectState::Running("video".to_string());

        assert!(!root.crop_detection_available());
        assert_eq!(
            crop_status_label(&root.preview_ui.crop_detect, None, None).as_deref(),
            Some("Detecting black bars…")
        );
    }
}
