use super::{
    Bounds, FocusHandle, Pixels, Range, SETTINGS_CONTROL_HEIGHT, ShapedLine,
    TEXT_INPUT_CARET_BASE_HEIGHT, TEXT_INPUT_CARET_WIDTH, Task,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct ScissorsTextInputMetrics {
    pub(in crate::app) control_height: f32,
    pub(in crate::app) caret_height: f32,
    pub(in crate::app) caret_width: f32,
}

impl ScissorsTextInputMetrics {
    pub(in crate::app) fn from_ui_scale(ui_scale: f32) -> Self {
        Self {
            control_height: SETTINGS_CONTROL_HEIGHT * ui_scale,
            caret_height: TEXT_INPUT_CARET_BASE_HEIGHT * ui_scale,
            caret_width: TEXT_INPUT_CARET_WIDTH * ui_scale,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum ScissorsTextInputKind {
    AudioBitrate,
    VideoCustomWidth,
    VideoCustomHeight,
    ExportName,
    GifLoop,
    PreviewStartTime,
    PreviewEndTime,
    MetadataTitle,
    MetadataArtist,
    MetadataAlbum,
    MetadataGenre,
    MetadataDate,
    MetadataComment,
    MetadataServiceName,
    MetadataServiceProvider,
    PresetName,
}

impl ScissorsTextInputKind {
    pub(in crate::app) const ALL: [Self; 16] = [
        Self::AudioBitrate,
        Self::VideoCustomWidth,
        Self::VideoCustomHeight,
        Self::ExportName,
        Self::GifLoop,
        Self::PreviewStartTime,
        Self::PreviewEndTime,
        Self::MetadataTitle,
        Self::MetadataArtist,
        Self::MetadataAlbum,
        Self::MetadataGenre,
        Self::MetadataDate,
        Self::MetadataComment,
        Self::MetadataServiceName,
        Self::MetadataServiceProvider,
        Self::PresetName,
    ];

    pub(in crate::app) const fn accessibility_label(self) -> &'static str {
        match self {
            Self::AudioBitrate => "Audio bitrate",
            Self::VideoCustomWidth => "Video width",
            Self::VideoCustomHeight => "Video height",
            Self::ExportName => "Export file name",
            Self::GifLoop => "GIF loop count",
            Self::PreviewStartTime => "Preview start time",
            Self::PreviewEndTime => "Preview end time",
            Self::MetadataTitle => "Metadata title",
            Self::MetadataArtist => "Metadata artist",
            Self::MetadataAlbum => "Metadata album",
            Self::MetadataGenre => "Metadata genre",
            Self::MetadataDate => "Metadata date",
            Self::MetadataComment => "Metadata comment",
            Self::MetadataServiceName => "MPEG transport stream service name",
            Self::MetadataServiceProvider => "MPEG transport stream service provider",
            Self::PresetName => "Preset name",
        }
    }

    pub(in crate::app) const fn is_preview_timecode(self) -> bool {
        matches!(self, Self::PreviewStartTime | Self::PreviewEndTime)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct ScissorsTimecodeInputDraft {
    pub(in crate::app) file_id: String,
    pub(in crate::app) value: String,
}

pub(in crate::app) struct ScissorsTextInputRuntime {
    pub(in crate::app) selected_range: Range<usize>,
    pub(in crate::app) selection_reversed: bool,
    pub(in crate::app) marked_range: Option<Range<usize>>,
    pub(in crate::app) last_layout: Option<ShapedLine>,
    pub(in crate::app) last_bounds: Option<Bounds<Pixels>>,
    pub(in crate::app) scroll_x: Pixels,
    pub(in crate::app) is_selecting: bool,
    pub(in crate::app) timecode_draft: Option<ScissorsTimecodeInputDraft>,
}

impl Default for ScissorsTextInputRuntime {
    fn default() -> Self {
        Self {
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            scroll_x: Pixels::ZERO,
            is_selecting: false,
            timecode_draft: None,
        }
    }
}

pub(in crate::app) fn clamp_text_input_scroll_x(
    scroll_x: Pixels,
    content_width: Pixels,
    viewport_width: Pixels,
) -> Pixels {
    scroll_x.clamp(
        Pixels::ZERO,
        (content_width - viewport_width).max(Pixels::ZERO),
    )
}

pub(in crate::app) fn text_input_scroll_x_for_cursor(
    current: Pixels,
    cursor_x: Pixels,
    content_width: Pixels,
    viewport_width: Pixels,
    caret_width: Pixels,
) -> Pixels {
    let mut next = current;
    let trailing_edge = (viewport_width - caret_width).max(Pixels::ZERO);

    if cursor_x - next > trailing_edge {
        next = cursor_x - trailing_edge;
    } else if cursor_x < next {
        next = cursor_x;
    }

    clamp_text_input_scroll_x(next, content_width, viewport_width)
}

#[cfg(test)]
mod metrics_tests {
    #![expect(
        clippy::float_cmp,
        reason = "Input metrics tests compare exact deterministic scale products."
    )]

    use super::*;

    #[test]
    fn input_metrics_preserve_baseline_dimensions() {
        let metrics = ScissorsTextInputMetrics::from_ui_scale(1.0);
        assert_eq!(metrics.control_height, SETTINGS_CONTROL_HEIGHT);
        assert_eq!(metrics.caret_height, TEXT_INPUT_CARET_BASE_HEIGHT);
        assert_eq!(metrics.caret_width, TEXT_INPUT_CARET_WIDTH);
    }

    #[test]
    fn ui_scale_resizes_control_and_caret_together() {
        let metrics = ScissorsTextInputMetrics::from_ui_scale(2.0);
        assert_eq!(metrics.control_height, 60.0);
        assert_eq!(metrics.caret_height, 28.0);
        assert_eq!(metrics.caret_width, 3.0);
    }
}

#[derive(Default)]
pub(in crate::app) struct ScissorsTextInputStore {
    audio_bitrate: ScissorsTextInputRuntime,
    video_width: ScissorsTextInputRuntime,
    video_height: ScissorsTextInputRuntime,
    export_name: ScissorsTextInputRuntime,
    gif_loop: ScissorsTextInputRuntime,
    preview_start_time: ScissorsTextInputRuntime,
    preview_end_time: ScissorsTextInputRuntime,
    metadata_title: ScissorsTextInputRuntime,
    metadata_artist: ScissorsTextInputRuntime,
    metadata_album: ScissorsTextInputRuntime,
    metadata_genre: ScissorsTextInputRuntime,
    metadata_date: ScissorsTextInputRuntime,
    metadata_comment: ScissorsTextInputRuntime,
    metadata_service_name: ScissorsTextInputRuntime,
    metadata_service_provider: ScissorsTextInputRuntime,
    preset_name: ScissorsTextInputRuntime,
}

impl ScissorsTextInputStore {
    pub(in crate::app) const fn runtime(
        &self,
        kind: ScissorsTextInputKind,
    ) -> &ScissorsTextInputRuntime {
        match kind {
            ScissorsTextInputKind::AudioBitrate => &self.audio_bitrate,
            ScissorsTextInputKind::VideoCustomWidth => &self.video_width,
            ScissorsTextInputKind::VideoCustomHeight => &self.video_height,
            ScissorsTextInputKind::ExportName => &self.export_name,
            ScissorsTextInputKind::GifLoop => &self.gif_loop,
            ScissorsTextInputKind::PreviewStartTime => &self.preview_start_time,
            ScissorsTextInputKind::PreviewEndTime => &self.preview_end_time,
            ScissorsTextInputKind::MetadataTitle => &self.metadata_title,
            ScissorsTextInputKind::MetadataArtist => &self.metadata_artist,
            ScissorsTextInputKind::MetadataAlbum => &self.metadata_album,
            ScissorsTextInputKind::MetadataGenre => &self.metadata_genre,
            ScissorsTextInputKind::MetadataDate => &self.metadata_date,
            ScissorsTextInputKind::MetadataComment => &self.metadata_comment,
            ScissorsTextInputKind::MetadataServiceName => &self.metadata_service_name,
            ScissorsTextInputKind::MetadataServiceProvider => &self.metadata_service_provider,
            ScissorsTextInputKind::PresetName => &self.preset_name,
        }
    }

    pub(in crate::app) const fn runtime_mut(
        &mut self,
        kind: ScissorsTextInputKind,
    ) -> &mut ScissorsTextInputRuntime {
        match kind {
            ScissorsTextInputKind::AudioBitrate => &mut self.audio_bitrate,
            ScissorsTextInputKind::VideoCustomWidth => &mut self.video_width,
            ScissorsTextInputKind::VideoCustomHeight => &mut self.video_height,
            ScissorsTextInputKind::ExportName => &mut self.export_name,
            ScissorsTextInputKind::GifLoop => &mut self.gif_loop,
            ScissorsTextInputKind::PreviewStartTime => &mut self.preview_start_time,
            ScissorsTextInputKind::PreviewEndTime => &mut self.preview_end_time,
            ScissorsTextInputKind::MetadataTitle => &mut self.metadata_title,
            ScissorsTextInputKind::MetadataArtist => &mut self.metadata_artist,
            ScissorsTextInputKind::MetadataAlbum => &mut self.metadata_album,
            ScissorsTextInputKind::MetadataGenre => &mut self.metadata_genre,
            ScissorsTextInputKind::MetadataDate => &mut self.metadata_date,
            ScissorsTextInputKind::MetadataComment => &mut self.metadata_comment,
            ScissorsTextInputKind::MetadataServiceName => &mut self.metadata_service_name,
            ScissorsTextInputKind::MetadataServiceProvider => &mut self.metadata_service_provider,
            ScissorsTextInputKind::PresetName => &mut self.preset_name,
        }
    }
}

#[derive(Default)]
pub(in crate::app) struct ScissorsTextInputFocusStore {
    audio_bitrate: Option<FocusHandle>,
    video_width: Option<FocusHandle>,
    video_height: Option<FocusHandle>,
    export_name: Option<FocusHandle>,
    gif_loop: Option<FocusHandle>,
    preview_start_time: Option<FocusHandle>,
    preview_end_time: Option<FocusHandle>,
    metadata_title: Option<FocusHandle>,
    metadata_artist: Option<FocusHandle>,
    metadata_album: Option<FocusHandle>,
    metadata_genre: Option<FocusHandle>,
    metadata_date: Option<FocusHandle>,
    metadata_comment: Option<FocusHandle>,
    metadata_service_name: Option<FocusHandle>,
    metadata_service_provider: Option<FocusHandle>,
    preset_name: Option<FocusHandle>,
}

impl ScissorsTextInputFocusStore {
    pub(in crate::app) const fn focus(&self, kind: ScissorsTextInputKind) -> Option<&FocusHandle> {
        match kind {
            ScissorsTextInputKind::AudioBitrate => self.audio_bitrate.as_ref(),
            ScissorsTextInputKind::VideoCustomWidth => self.video_width.as_ref(),
            ScissorsTextInputKind::VideoCustomHeight => self.video_height.as_ref(),
            ScissorsTextInputKind::ExportName => self.export_name.as_ref(),
            ScissorsTextInputKind::GifLoop => self.gif_loop.as_ref(),
            ScissorsTextInputKind::PreviewStartTime => self.preview_start_time.as_ref(),
            ScissorsTextInputKind::PreviewEndTime => self.preview_end_time.as_ref(),
            ScissorsTextInputKind::MetadataTitle => self.metadata_title.as_ref(),
            ScissorsTextInputKind::MetadataArtist => self.metadata_artist.as_ref(),
            ScissorsTextInputKind::MetadataAlbum => self.metadata_album.as_ref(),
            ScissorsTextInputKind::MetadataGenre => self.metadata_genre.as_ref(),
            ScissorsTextInputKind::MetadataDate => self.metadata_date.as_ref(),
            ScissorsTextInputKind::MetadataComment => self.metadata_comment.as_ref(),
            ScissorsTextInputKind::MetadataServiceName => self.metadata_service_name.as_ref(),
            ScissorsTextInputKind::MetadataServiceProvider => {
                self.metadata_service_provider.as_ref()
            }
            ScissorsTextInputKind::PresetName => self.preset_name.as_ref(),
        }
    }

    pub(in crate::app) const fn focus_mut(
        &mut self,
        kind: ScissorsTextInputKind,
    ) -> &mut Option<FocusHandle> {
        match kind {
            ScissorsTextInputKind::AudioBitrate => &mut self.audio_bitrate,
            ScissorsTextInputKind::VideoCustomWidth => &mut self.video_width,
            ScissorsTextInputKind::VideoCustomHeight => &mut self.video_height,
            ScissorsTextInputKind::ExportName => &mut self.export_name,
            ScissorsTextInputKind::GifLoop => &mut self.gif_loop,
            ScissorsTextInputKind::PreviewStartTime => &mut self.preview_start_time,
            ScissorsTextInputKind::PreviewEndTime => &mut self.preview_end_time,
            ScissorsTextInputKind::MetadataTitle => &mut self.metadata_title,
            ScissorsTextInputKind::MetadataArtist => &mut self.metadata_artist,
            ScissorsTextInputKind::MetadataAlbum => &mut self.metadata_album,
            ScissorsTextInputKind::MetadataGenre => &mut self.metadata_genre,
            ScissorsTextInputKind::MetadataDate => &mut self.metadata_date,
            ScissorsTextInputKind::MetadataComment => &mut self.metadata_comment,
            ScissorsTextInputKind::MetadataServiceName => &mut self.metadata_service_name,
            ScissorsTextInputKind::MetadataServiceProvider => &mut self.metadata_service_provider,
            ScissorsTextInputKind::PresetName => &mut self.preset_name,
        }
    }
}

pub(in crate::app) struct ScissorsTextInputUiState {
    pub(in crate::app) active: Option<ScissorsTextInputKind>,
    pub(in crate::app) runtimes: ScissorsTextInputStore,
    pub(in crate::app) focuses: ScissorsTextInputFocusStore,
    pub(in crate::app) cursor_visible: bool,
    pub(in crate::app) cursor_paused: bool,
    pub(in crate::app) cursor_epoch: usize,
    pub(in crate::app) cursor_task: Task<()>,
}

impl Default for ScissorsTextInputUiState {
    fn default() -> Self {
        Self {
            active: None,
            runtimes: ScissorsTextInputStore::default(),
            focuses: ScissorsTextInputFocusStore::default(),
            cursor_visible: false,
            cursor_paused: false,
            cursor_epoch: 0,
            cursor_task: Task::ready(()),
        }
    }
}
