//! Cached source thumbnails and the library action menu state.
use super::{FocusHandle, RenderImage};
use std::{collections::HashMap, process::Command, sync::Arc};

#[derive(Default)]
pub(super) struct MediaAssetsState {
    pub(super) thumbnails: HashMap<String, Arc<RenderImage>>,
    pub(super) menu: Option<AssetMenu>,
}

#[derive(Clone, Copy)]
pub(super) struct MediaAssetsProps<'a> {
    pub(super) state: &'a MediaAssetsState,
    pub(super) delete_enabled: bool,
}

#[derive(Clone)]
pub(super) struct AssetMenu {
    pub(super) source_id: String,
    pub(super) focuses: [FocusHandle; 2],
    pub(super) trigger: FocusHandle,
}

/// Decode a single small frame off the UI thread. GPUI uploads BGRA pixels.
pub(super) fn video_thumbnail(path: &str) -> Option<Arc<RenderImage>> {
    let output = Command::new(crate::runtime_binaries::ffmpeg_executable())
        .args([
            "-v",
            "error",
            "-nostdin",
            "-i",
            path,
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            "-vf",
            "scale=256:144:force_original_aspect_ratio=decrease",
            "-threads",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "pipe:1",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut pixels = image::load_from_memory(&output.stdout).ok()?.into_rgba8();
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(smallvec::smallvec![
        image::Frame::new(pixels)
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires FFmpeg; run with --ignored"]
    fn video_thumbnail_decodes_a_bounded_bgra_frame_and_handles_missing_media() {
        let path = std::env::temp_dir().join(format!(
            "scissors-library-thumbnail-{}.mp4",
            std::process::id()
        ));
        let status = Command::new(crate::runtime_binaries::ffmpeg_executable())
            .args([
                "-v",
                "error",
                "-nostdin",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=640x360:d=0.1",
                "-threads",
                "1",
            ])
            .arg(&path)
            .status()
            .expect("generate thumbnail fixture");
        assert!(status.success());
        let thumbnail = video_thumbnail(path.to_str().unwrap()).expect("decode video thumbnail");
        std::fs::remove_file(path).unwrap();
        assert_eq!(thumbnail.size(0).width, gpui::DevicePixels(256));
        assert_eq!(thumbnail.size(0).height, gpui::DevicePixels(144));
        let pixel = &thumbnail.as_bytes(0).unwrap()[..4];
        assert!(pixel[2] > 240 && pixel[0] < 10 && pixel[1] < 10);
        assert_eq!(pixel[3], 255);
        assert!(video_thumbnail("/missing/scissors-library-thumbnail.mp4").is_none());
    }
}
