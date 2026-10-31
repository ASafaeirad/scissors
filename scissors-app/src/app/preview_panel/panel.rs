use super::*;
use crate::app::m3::{self, M3Text};
use crate::material::{self, shape, typography};
use crate::numeric::{f64_to_u32, u32_to_f32};

#[expect(
    clippy::struct_excessive_bools,
    reason = "Preview crop render state mirrors independent toggle controls from the toolbar."
)]
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PreviewCropRenderState {
    pub(in crate::app) crop_mode: bool,
    pub(in crate::app) draft_crop: Option<CropRect>,
    pub(in crate::app) applied_crop: Option<CropRect>,
    pub(in crate::app) crop_aspect: String,
    pub(in crate::app) has_crop_dimensions: bool,
    /// Output pixel size of the draft crop while crop mode is open.
    pub(in crate::app) draft_size: Option<CropSourceDimensions>,
    /// Status pill text: black-bar detection progress or the draft crop's size.
    pub(in crate::app) status: Option<String>,
    pub(in crate::app) can_detect_bars: bool,
    pub(in crate::app) rotation: String,
    pub(in crate::app) flip_horizontal: bool,
    pub(in crate::app) flip_vertical: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PreviewShellState {
    pub(in crate::app) tooltip_visible_id: Option<String>,
    pub(in crate::app) palette: &'static theme::ThemePalette,
    pub(in crate::app) selected_file_name: Option<String>,
    pub(in crate::app) waveforms: Vec<Option<Arc<super::super::waveform::Waveform>>>,
    pub(in crate::app) filmstrip: Option<Arc<super::super::filmstrip::Filmstrip>>,
    /// Whether the source exports an audio stream, drawn as a waveform under its thumbnails.
    pub(in crate::app) source_audio: bool,
    pub(in crate::app) audio_tracks: Vec<scissors_core::types::ExternalAudioTrack>,
    pub(in crate::app) metadata_status: PreviewMetadataStatus,
    pub(in crate::app) metadata_error: Option<String>,
    pub(in crate::app) controls_disabled: bool,
    pub(in crate::app) availability: PreviewControlAvailability,
    pub(in crate::app) playback: PreviewPlaybackState,
    pub(in crate::app) duration_seconds: f64,
    pub(in crate::app) canvas: PreviewCanvasRenderState,
    pub(in crate::app) crop: PreviewCropRenderState,
    pub(in crate::app) overlay: PreviewOverlayRenderState,
    pub(in crate::app) presentation: PreviewRenderPresentation,
    pub(in crate::app) media: Option<PreviewMediaRenderState>,
    pub(in crate::app) render_image: Option<Arc<RenderImage>>,
    pub(in crate::app) runtime_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct PreviewCanvasRenderState {
    pub(in crate::app) zoom: f64,
    pub(in crate::app) pan_x: f64,
    pub(in crate::app) pan_y: f64,
    pub(in crate::app) viewport_width: f64,
    pub(in crate::app) viewport_height: f64,
}

impl Default for PreviewCanvasRenderState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            viewport_width: 0.0,
            viewport_height: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct PreviewMediaRenderState {
    pub(in crate::app) width: u32,
    pub(in crate::app) height: u32,
}

impl PreviewMediaRenderState {
    pub(in crate::app) fn aspect_ratio(self) -> f32 {
        if self.height == 0 {
            return 1.0;
        }

        u32_to_f32(self.width) / u32_to_f32(self.height)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct PreviewPresentedFrame {
    pub(in crate::app) full_width: u32,
    pub(in crate::app) full_height: u32,
    pub(in crate::app) visible_x: u32,
    pub(in crate::app) visible_y: u32,
    pub(in crate::app) visible_width: u32,
    pub(in crate::app) visible_height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PreviewOverlayRenderState {
    pub(in crate::app) overlay_mode: bool,
    pub(in crate::app) has_overlay: bool,
    pub(in crate::app) overlay: Option<PreviewOverlay>,
    pub(in crate::app) image_dimensions: Option<PreviewOverlayImageDimensions>,
}

#[cfg(test)]
impl PreviewOverlayRenderState {
    #[must_use]
    pub(in crate::app) const fn empty() -> Self {
        Self {
            overlay_mode: false,
            has_overlay: false,
            overlay: None,
            image_dimensions: None,
        }
    }
}

pub(in crate::app) struct PreviewPanelProps<'a> {
    pub(in crate::app) project: Option<crate::app::project::ProjectRenderState>,
    pub(in crate::app) canvas: PreviewCanvasRenderState,
    pub(in crate::app) crop: PreviewCropRenderState,
    pub(in crate::app) overlay: PreviewOverlayRenderState,
    pub(in crate::app) viewport_focuses: PreviewViewportFocuses<'a>,
    pub(in crate::app) seek_focus: &'a FocusHandle,
    pub(in crate::app) playback: PreviewPlaybackState,
    pub(in crate::app) presentation: PreviewRenderPresentation,
    pub(in crate::app) render_image: Option<Arc<RenderImage>>,
    pub(in crate::app) runtime_error: Option<String>,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct PreviewViewportFocuses<'a> {
    pub(in crate::app) viewport: &'a FocusHandle,
    pub(in crate::app) tools: PreviewToolFocuses<'a>,
    pub(in crate::app) edit_toolbars: PreviewEditToolbarFocuses<'a>,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct PreviewToolFocuses<'a> {
    pub(in crate::app) crop: &'a FocusHandle,
    pub(in crate::app) overlay: &'a FocusHandle,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct PreviewEditToolbarFocus<'a> {
    pub(in crate::app) panel: &'a FocusHandle,
    pub(in crate::app) first: &'a FocusHandle,
    pub(in crate::app) last: &'a FocusHandle,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct PreviewEditToolbarFocuses<'a> {
    pub(in crate::app) crop: PreviewEditToolbarFocus<'a>,
    pub(in crate::app) overlay: PreviewEditToolbarFocus<'a>,
}

pub(in crate::app) struct PreviewShellStateInput<'a> {
    pub(in crate::app) selected_file: Option<&'a FileItem>,
    pub(in crate::app) settings: &'a SettingsRenderState<'a>,
    pub(in crate::app) crop: PreviewCropRenderState,
    pub(in crate::app) overlay: PreviewOverlayRenderState,
    pub(in crate::app) canvas: PreviewCanvasRenderState,
    pub(in crate::app) playback: PreviewPlaybackState,
    pub(in crate::app) presentation: PreviewRenderPresentation,
    pub(in crate::app) render_image: Option<Arc<RenderImage>>,
    pub(in crate::app) runtime_error: Option<String>,
}

#[expect(
    clippy::too_many_lines,
    reason = "The preview combines viewport, playback toolbar, and the source or project timeline."
)]
pub(in crate::app) fn preview_panel(
    file_queue: &FileQueue,
    settings: &SettingsRenderState<'_>,
    props: PreviewPanelProps<'_>,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let project_enabled = props.project.is_some();
    let mut state = preview_shell_state(PreviewShellStateInput {
        selected_file: file_queue.selected_file(),
        settings,
        crop: props.crop,
        overlay: props.overlay,
        canvas: props.canvas,
        playback: props.playback,
        presentation: props.presentation,
        render_image: props.render_image,
        runtime_error: props.runtime_error,
    });

    if !project_enabled {
        let selected = file_queue.selected_file();
        let video = state.availability.media_kind == PreviewMediaKind::Video;
        state.filmstrip = selected.filter(|_| video).and_then(|file| {
            super::super::filmstrip::request(&file.path, state.duration_seconds, cx)
        });
        // Audio sources fall back to their first stream. A video shows the audio it exports.
        let source = selected.and_then(|file| {
            let stream = file.config.selected_audio_tracks.first().copied();
            match state.availability.media_kind {
                PreviewMediaKind::Audio => Some((file, stream)),
                PreviewMediaKind::Video => stream.map(|stream| (file, Some(stream))),
                PreviewMediaKind::Image | PreviewMediaKind::Unknown => None,
            }
        });
        state.source_audio = source.is_some();
        let source = source.and_then(|(file, stream)| {
            super::super::waveform::request(&file.path, stream, state.duration_seconds, cx)
        });
        state.waveforms.push(source);
        for track in &state.audio_tracks {
            state.waveforms.push(super::super::waveform::request(
                &track.path,
                None,
                std::time::Duration::from_millis(track.duration_millis).as_secs_f64(),
                cx,
            ));
        }
    }
    if project_enabled {
        state.duration_seconds = state.playback.duration();
        if props.project.as_ref().is_some_and(|project| {
            project
                .timeline
                .clips
                .iter()
                .any(|(clip, _)| clip.kind != SourceKind::Audio)
        }) {
            state.availability = preview_control_availability(PreviewControlInput {
                metadata_status: state.metadata_status,
                source_media_kind: Some(SourceMediaKind::Video),
                controls_disabled: state.controls_disabled,
                processing_mode: crate::settings::ProcessingMode::Reencode,
                container: Some("mp4"),
            });
        }
    }
    let scheme = &crate::app::m3::scheme(
        if state.palette.is_light() {
            ColorTheme::Light
        } else {
            ColorTheme::Dark
        },
        cx,
    );
    div()
        .flex()
        .flex_col()
        .min_h_0()
        .gap(theme::ui_rem(12.0))
        .child(
            m3::m3_surface(scheme.surface_container_lowest, shape::CORNER_EXTRA_LARGE)
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .items_center()
                .p(theme::ui_rem(20.0))
                .gap(theme::ui_rem(8.0))
                .child(preview_viewport(&state, props.viewport_focuses, window, cx))
                .child(
                    m3::toolbar(
                        m3::ToolbarVariant::Floating,
                        m3::ToolbarColor::Standard,
                        scheme,
                    )
                    .child(
                        m3::IconButton::new(
                            "preview-back-five",
                            material::Symbol::Replay5,
                            "Back 5 seconds",
                        )
                        .enabled(preview_trim_enabled(&state))
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, _, cx| {
                            if root.skip_preview_seconds(-5.0, cx) {
                                cx.notify();
                            }
                        })),
                    )
                    .child(preview_play_button(&state, window, cx))
                    .child(
                        m3::IconButton::new(
                            "preview-forward-five",
                            material::Symbol::Forward5,
                            "Forward 5 seconds",
                        )
                        .enabled(preview_trim_enabled(&state))
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, _, cx| {
                            if root.skip_preview_seconds(5.0, cx) {
                                cx.notify();
                            }
                        })),
                    )
                    .child(
                        div()
                            .px(theme::ui_rem(12.0))
                            .m3_type(typography::TITLE_SMALL)
                            .child(format!(
                                "{} / {}",
                                format_timeline_time(
                                    state.playback.current_time(),
                                    state.duration_seconds
                                ),
                                format_timeline_time(
                                    state.duration_seconds,
                                    state.duration_seconds
                                )
                            )),
                    )
                    .when(!project_enabled, |bar| {
                        bar.child(preview_cut_button(&state, window, cx))
                    }),
                ),
        )
        .when(!project_enabled, |panel| {
            panel.child(preview_timeline(&state, props.seek_focus, window, cx))
        })
        .when(project_enabled, |panel| {
            panel.child(crate::app::project_timeline::project_timeline(
                &props.project.as_ref().unwrap().timeline,
                props.seek_focus,
                window,
                cx,
            ))
        })
}

pub(in crate::app) fn preview_shell_state(input: PreviewShellStateInput<'_>) -> PreviewShellState {
    let PreviewShellStateInput {
        selected_file,
        settings,
        crop,
        overlay,
        canvas,
        playback,
        presentation,
        render_image,
        runtime_error,
    } = input;
    let metadata_status = preview_metadata_status(settings.metadata_status);
    let source_media_kind = settings.metadata.map(preview_source_media_kind);
    let media = preview_media_render_state(render_image.as_ref(), presentation);
    let availability = preview_control_availability(PreviewControlInput {
        metadata_status,
        source_media_kind,
        controls_disabled: settings.settings_disabled,
        processing_mode: settings.config.processing_mode,
        container: Some(settings.config.container.as_str()),
    });
    let duration_seconds = preview_duration_seconds(settings.metadata);
    PreviewShellState {
        tooltip_visible_id: settings.tooltip_visible_id.map(str::to_string),
        palette: settings.palette,
        selected_file_name: selected_file.map(|file| file.name.clone()),
        waveforms: Vec::new(),
        filmstrip: None,
        source_audio: false,
        audio_tracks: selected_file
            .map(|file| file.config.external_audio_tracks.clone())
            .unwrap_or_default(),
        metadata_status,
        metadata_error: settings.metadata_error.map(str::to_string),
        controls_disabled: settings.settings_disabled,
        availability,
        playback,
        duration_seconds,
        canvas,
        crop,
        overlay,
        presentation,
        media,
        render_image,
        runtime_error,
    }
}

pub(in crate::app) fn preview_media_render_state(
    render_image: Option<&Arc<RenderImage>>,
    presentation: PreviewRenderPresentation,
) -> Option<PreviewMediaRenderState> {
    let frame = preview_presented_frame(render_image?, presentation)?;
    Some(PreviewMediaRenderState {
        width: frame.visible_width,
        height: frame.visible_height,
    })
}

pub(in crate::app) fn preview_presented_frame(
    render_image: &Arc<RenderImage>,
    presentation: PreviewRenderPresentation,
) -> Option<PreviewPresentedFrame> {
    let size = render_image.size(0);
    let raw_width = u32::try_from(size.width.0).ok()?;
    let raw_height = u32::try_from(size.height.0).ok()?;
    if raw_width == 0 || raw_height == 0 {
        return None;
    }

    let (full_width, full_height) = if presentation.transform.has_side_rotation() {
        (raw_height, raw_width)
    } else {
        (raw_width, raw_height)
    };

    let Some(crop) = presentation.crop else {
        return Some(PreviewPresentedFrame {
            full_width,
            full_height,
            visible_x: 0,
            visible_y: 0,
            visible_width: full_width,
            visible_height: full_height,
        });
    };

    let source_width = presentation.crop_source_width?;
    let source_height = presentation.crop_source_height?;
    let crop_right = crop.x.checked_add(crop.width)?;
    let crop_bottom = crop.y.checked_add(crop.height)?;
    if crop.width == 0
        || crop.height == 0
        || crop_right > source_width
        || crop_bottom > source_height
    {
        return None;
    }

    let left = scale_preview_crop_start(crop.x, source_width, full_width);
    let top = scale_preview_crop_start(crop.y, source_height, full_height);
    let right = scale_preview_crop_end(crop_right, source_width, full_width)
        .max(left.saturating_add(1))
        .min(full_width);
    let bottom = scale_preview_crop_end(crop_bottom, source_height, full_height)
        .max(top.saturating_add(1))
        .min(full_height);

    Some(PreviewPresentedFrame {
        full_width,
        full_height,
        visible_x: left,
        visible_y: top,
        visible_width: right.saturating_sub(left),
        visible_height: bottom.saturating_sub(top),
    })
}

fn scale_preview_crop_start(value: u32, source_extent: u32, image_extent: u32) -> u32 {
    f64_to_u32(
        ((f64::from(value) / f64::from(source_extent)) * f64::from(image_extent))
            .floor()
            .clamp(0.0, f64::from(image_extent.saturating_sub(1))),
    )
}

fn scale_preview_crop_end(value: u32, source_extent: u32, image_extent: u32) -> u32 {
    f64_to_u32(
        ((f64::from(value) / f64::from(source_extent)) * f64::from(image_extent))
            .ceil()
            .clamp(1.0, f64::from(image_extent)),
    )
}

pub(in crate::app) const fn preview_metadata_status(
    status: MetadataStatus,
) -> PreviewMetadataStatus {
    match status {
        MetadataStatus::Idle => PreviewMetadataStatus::Idle,
        MetadataStatus::Loading => PreviewMetadataStatus::Loading,
        MetadataStatus::Ready => PreviewMetadataStatus::Ready,
        MetadataStatus::Error => PreviewMetadataStatus::Error,
    }
}

pub(in crate::app) fn preview_source_media_kind(metadata: &SourceMetadata) -> SourceMediaKind {
    match metadata.source_kind() {
        SourceKind::Video => SourceMediaKind::Video,
        SourceKind::Audio => SourceMediaKind::Audio,
        SourceKind::Image => SourceMediaKind::Image,
    }
}

pub(in crate::app) fn preview_duration_seconds(metadata: Option<&SourceMetadata>) -> f64 {
    let Some(raw) = metadata.and_then(|metadata| metadata.duration.as_deref()) else {
        return 0.0;
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return 0.0;
    }

    let duration = if raw.contains(':') {
        parse_time_to_seconds(raw)
    } else {
        raw.parse::<f64>().unwrap_or(0.0)
    };

    if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        0.0
    }
}

pub(in crate::app) fn preview_playback_state(
    media_kind: PreviewMediaKind,
    duration_seconds: f64,
    start_time: Option<&str>,
    end_time: Option<&str>,
) -> PreviewPlaybackState {
    let is_image = media_kind == PreviewMediaKind::Image;
    let mut playback = PreviewPlaybackState::new(is_image);
    if media_kind != PreviewMediaKind::Unknown && !is_image {
        playback.sync_media(MediaSnapshot {
            current_time: 0.0,
            duration: duration_seconds,
            paused: true,
        });
        playback.sync_initial_values(start_time, end_time);
    }
    playback
}
