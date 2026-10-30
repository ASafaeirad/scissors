use super::{
    Bounds, FocusHandle, Pixels, Range, SETTINGS_CONTROL_HEIGHT, ShapedLine,
    TEXT_INPUT_CARET_BASE_HEIGHT, TEXT_INPUT_CARET_WIDTH, Task,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct FrameTextInputMetrics {
    pub(in crate::app) control_height: f32,
    pub(in crate::app) caret_height: f32,
    pub(in crate::app) caret_width: f32,
}

impl FrameTextInputMetrics {
    pub(in crate::app) fn from_ui_scale(ui_scale: f32) -> Self {
        Self {
            control_height: SETTINGS_CONTROL_HEIGHT * ui_scale,
            caret_height: TEXT_INPUT_CARET_BASE_HEIGHT * ui_scale,
            caret_width: TEXT_INPUT_CARET_WIDTH * ui_scale,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum FrameTextInputKind {
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

impl FrameTextInputKind {
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
pub(in crate::app) struct FrameTimecodeInputDraft {
    pub(in crate::app) file_id: String,
    pub(in crate::app) value: String,
}

pub(in crate::app) struct FrameTextInputRuntime {
    pub(in crate::app) selected_range: Range<usize>,
    pub(in crate::app) selection_reversed: bool,
    pub(in crate::app) marked_range: Option<Range<usize>>,
    pub(in crate::app) last_layout: Option<ShapedLine>,
    pub(in crate::app) last_bounds: Option<Bounds<Pixels>>,
    pub(in crate::app) scroll_x: Pixels,
    pub(in crate::app) is_selecting: bool,
    pub(in crate::app) timecode_draft: Option<FrameTimecodeInputDraft>,
}

impl Default for FrameTextInputRuntime {
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
        let metrics = FrameTextInputMetrics::from_ui_scale(1.0);
        assert_eq!(metrics.control_height, SETTINGS_CONTROL_HEIGHT);
        assert_eq!(metrics.caret_height, TEXT_INPUT_CARET_BASE_HEIGHT);
        assert_eq!(metrics.caret_width, TEXT_INPUT_CARET_WIDTH);
    }

    #[test]
    fn ui_scale_resizes_control_and_caret_together() {
        let metrics = FrameTextInputMetrics::from_ui_scale(2.0);
        assert_eq!(metrics.control_height, 60.0);
        assert_eq!(metrics.caret_height, 28.0);
        assert_eq!(metrics.caret_width, 3.0);
    }
}

#[derive(Default)]
pub(in crate::app) struct FrameTextInputStore {
    audio_bitrate: FrameTextInputRuntime,
    video_width: FrameTextInputRuntime,
    video_height: FrameTextInputRuntime,
    export_name: FrameTextInputRuntime,
    gif_loop: FrameTextInputRuntime,
    preview_start_time: FrameTextInputRuntime,
    preview_end_time: FrameTextInputRuntime,
    metadata_title: FrameTextInputRuntime,
    metadata_artist: FrameTextInputRuntime,
    metadata_album: FrameTextInputRuntime,
    metadata_genre: FrameTextInputRuntime,
    metadata_date: FrameTextInputRuntime,
    metadata_comment: FrameTextInputRuntime,
    metadata_service_name: FrameTextInputRuntime,
    metadata_service_provider: FrameTextInputRuntime,
    preset_name: FrameTextInputRuntime,
}

impl FrameTextInputStore {
    pub(in crate::app) const fn runtime(&self, kind: FrameTextInputKind) -> &FrameTextInputRuntime {
        match kind {
            FrameTextInputKind::AudioBitrate => &self.audio_bitrate,
            FrameTextInputKind::VideoCustomWidth => &self.video_width,
            FrameTextInputKind::VideoCustomHeight => &self.video_height,
            FrameTextInputKind::ExportName => &self.export_name,
            FrameTextInputKind::GifLoop => &self.gif_loop,
            FrameTextInputKind::PreviewStartTime => &self.preview_start_time,
            FrameTextInputKind::PreviewEndTime => &self.preview_end_time,
            FrameTextInputKind::MetadataTitle => &self.metadata_title,
            FrameTextInputKind::MetadataArtist => &self.metadata_artist,
            FrameTextInputKind::MetadataAlbum => &self.metadata_album,
            FrameTextInputKind::MetadataGenre => &self.metadata_genre,
            FrameTextInputKind::MetadataDate => &self.metadata_date,
            FrameTextInputKind::MetadataComment => &self.metadata_comment,
            FrameTextInputKind::MetadataServiceName => &self.metadata_service_name,
            FrameTextInputKind::MetadataServiceProvider => &self.metadata_service_provider,
            FrameTextInputKind::PresetName => &self.preset_name,
        }
    }

    pub(in crate::app) const fn runtime_mut(
        &mut self,
        kind: FrameTextInputKind,
    ) -> &mut FrameTextInputRuntime {
        match kind {
            FrameTextInputKind::AudioBitrate => &mut self.audio_bitrate,
            FrameTextInputKind::VideoCustomWidth => &mut self.video_width,
            FrameTextInputKind::VideoCustomHeight => &mut self.video_height,
            FrameTextInputKind::ExportName => &mut self.export_name,
            FrameTextInputKind::GifLoop => &mut self.gif_loop,
            FrameTextInputKind::PreviewStartTime => &mut self.preview_start_time,
            FrameTextInputKind::PreviewEndTime => &mut self.preview_end_time,
            FrameTextInputKind::MetadataTitle => &mut self.metadata_title,
            FrameTextInputKind::MetadataArtist => &mut self.metadata_artist,
            FrameTextInputKind::MetadataAlbum => &mut self.metadata_album,
            FrameTextInputKind::MetadataGenre => &mut self.metadata_genre,
            FrameTextInputKind::MetadataDate => &mut self.metadata_date,
            FrameTextInputKind::MetadataComment => &mut self.metadata_comment,
            FrameTextInputKind::MetadataServiceName => &mut self.metadata_service_name,
            FrameTextInputKind::MetadataServiceProvider => &mut self.metadata_service_provider,
            FrameTextInputKind::PresetName => &mut self.preset_name,
        }
    }
}

#[derive(Default)]
pub(in crate::app) struct FrameTextInputFocusStore {
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

impl FrameTextInputFocusStore {
    pub(in crate::app) const fn focus(&self, kind: FrameTextInputKind) -> Option<&FocusHandle> {
        match kind {
            FrameTextInputKind::AudioBitrate => self.audio_bitrate.as_ref(),
            FrameTextInputKind::VideoCustomWidth => self.video_width.as_ref(),
            FrameTextInputKind::VideoCustomHeight => self.video_height.as_ref(),
            FrameTextInputKind::ExportName => self.export_name.as_ref(),
            FrameTextInputKind::GifLoop => self.gif_loop.as_ref(),
            FrameTextInputKind::PreviewStartTime => self.preview_start_time.as_ref(),
            FrameTextInputKind::PreviewEndTime => self.preview_end_time.as_ref(),
            FrameTextInputKind::MetadataTitle => self.metadata_title.as_ref(),
            FrameTextInputKind::MetadataArtist => self.metadata_artist.as_ref(),
            FrameTextInputKind::MetadataAlbum => self.metadata_album.as_ref(),
            FrameTextInputKind::MetadataGenre => self.metadata_genre.as_ref(),
            FrameTextInputKind::MetadataDate => self.metadata_date.as_ref(),
            FrameTextInputKind::MetadataComment => self.metadata_comment.as_ref(),
            FrameTextInputKind::MetadataServiceName => self.metadata_service_name.as_ref(),
            FrameTextInputKind::MetadataServiceProvider => self.metadata_service_provider.as_ref(),
            FrameTextInputKind::PresetName => self.preset_name.as_ref(),
        }
    }

    pub(in crate::app) const fn focus_mut(
        &mut self,
        kind: FrameTextInputKind,
    ) -> &mut Option<FocusHandle> {
        match kind {
            FrameTextInputKind::AudioBitrate => &mut self.audio_bitrate,
            FrameTextInputKind::VideoCustomWidth => &mut self.video_width,
            FrameTextInputKind::VideoCustomHeight => &mut self.video_height,
            FrameTextInputKind::ExportName => &mut self.export_name,
            FrameTextInputKind::GifLoop => &mut self.gif_loop,
            FrameTextInputKind::PreviewStartTime => &mut self.preview_start_time,
            FrameTextInputKind::PreviewEndTime => &mut self.preview_end_time,
            FrameTextInputKind::MetadataTitle => &mut self.metadata_title,
            FrameTextInputKind::MetadataArtist => &mut self.metadata_artist,
            FrameTextInputKind::MetadataAlbum => &mut self.metadata_album,
            FrameTextInputKind::MetadataGenre => &mut self.metadata_genre,
            FrameTextInputKind::MetadataDate => &mut self.metadata_date,
            FrameTextInputKind::MetadataComment => &mut self.metadata_comment,
            FrameTextInputKind::MetadataServiceName => &mut self.metadata_service_name,
            FrameTextInputKind::MetadataServiceProvider => &mut self.metadata_service_provider,
            FrameTextInputKind::PresetName => &mut self.preset_name,
        }
    }
}

pub(in crate::app) struct FrameTextInputUiState {
    pub(in crate::app) active: Option<FrameTextInputKind>,
    pub(in crate::app) runtimes: FrameTextInputStore,
    pub(in crate::app) focuses: FrameTextInputFocusStore,
    pub(in crate::app) cursor_visible: bool,
    pub(in crate::app) cursor_paused: bool,
    pub(in crate::app) cursor_epoch: usize,
    pub(in crate::app) cursor_task: Task<()>,
}

impl Default for FrameTextInputUiState {
    fn default() -> Self {
        Self {
            active: None,
            runtimes: FrameTextInputStore::default(),
            focuses: FrameTextInputFocusStore::default(),
            cursor_visible: false,
            cursor_paused: false,
            cursor_epoch: 0,
            cursor_task: Task::ready(()),
        }
    }
}
