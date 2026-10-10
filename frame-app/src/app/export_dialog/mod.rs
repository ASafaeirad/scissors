//! The export dialog, opened by the workspace header's Export button. It edits the selected
//! file's encoding settings (format, size, frame rate, quality and the advanced encoder options
//! that used to live in the Video settings tab), then starts the export and shows its progress.

use super::accessibility::handle_modal_tab_navigation;
use super::input::{FrameTextInputColors, FrameTextInputSpec, m3_text_input};
use super::m3::{self, M3Text};
use super::settings_panel::{range_value_for_key, range_value_from_fraction};
use super::*;
use crate::material::{self, ColorScheme, Symbol, shape, state, typography};
use crate::numeric::u32_to_u8;
use crate::settings::{VideoCodecOption, is_audio_only_container, source_kind_for};

mod model;

use model::{
    EXPORT_FORMATS, ExportPhase, GIF_COLOR_OPTIONS, GIF_DITHER_OPTIONS, QualityTarget,
    RESOLUTION_OPTIONS, SCALING_OPTIONS, advanced_summary, apply_export_format, audio_summary,
    custom_dimensions_error, encoder_description, estimate_size_bytes, export_phase, format_clock,
    format_dimensions, format_rate, format_size, fps_note, fps_option_label, fps_options,
    is_recommended, output_dimensions, output_duration_seconds, output_fps, parse_dimension,
    pixel_format_text, quality_control, reset_to_recommended, resolution_note, scaled_even,
    scaling_note, source_dimensions, speed_presets,
};

const DIALOG_WIDTH: f32 = 920.0;
const DIALOG_HEIGHT: f32 = 780.0;
const DIALOG_ENTER_OFFSET: f32 = 12.0;
const SIDE_PANEL_WIDTH: f32 = 300.0;
const SIDE_PANEL_PADDING: f32 = 20.0;
const PREVIEW_MAX_HEIGHT: f32 = 200.0;
const SECTION_GAP: f32 = 28.0;
const STATUS_ICON_SIZE: f32 = 48.0;
const PROGRESS_WIDTH: f32 = 420.0;
const SUMMARY_ROW_HEIGHT: f32 = 32.0;

#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag is an independent piece of dialog state: visibility, presence during the close animation, the advanced section, and the aspect link."
)]
pub(super) struct ExportUiState {
    pub(super) open: bool,
    pub(super) present: bool,
    pub(super) advanced: bool,
    /// Typing a custom width fills in the height from the source aspect, and the reverse.
    pub(super) lock_aspect: bool,
    /// Files started from the dialog. Empty until the user exports.
    pub(super) task_ids: Vec<String>,
    scroll_handle: ScrollHandle,
}

impl Default for ExportUiState {
    fn default() -> Self {
        Self {
            open: false,
            present: false,
            advanced: false,
            lock_aspect: true,
            task_ids: Vec::new(),
            scroll_handle: ScrollHandle::new(),
        }
    }
}

/// What a dialog slider edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExportSlider {
    Quality(QualityTarget),
    /// Index into the encoder's `speed_presets`.
    Speed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExportSliderDrag {
    slider: ExportSlider,
    min: u32,
    max: u32,
}

struct ExportSliderDragPreview;

impl Render for ExportSliderDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn apply_export_slider(config: &mut ConversionConfig, slider: ExportSlider, value: u32) -> bool {
    match slider {
        ExportSlider::Quality(QualityTarget::Crf) => apply_crf(config, u32_to_u8(value)),
        ExportSlider::Quality(QualityTarget::Quality) => apply_quality(config, value),
        ExportSlider::Quality(QualityTarget::Bitrate) => {
            apply_video_bitrate(config, &(value * 1_000).to_string())
        }
        ExportSlider::Speed => speed_presets(&config.video_codec)
            .get(value as usize)
            .is_some_and(|(preset, _)| apply_video_preset(config, preset)),
    }
}

impl FrameRoot {
    pub(super) fn open_export_dialog(&mut self) -> bool {
        if self.file_queue.selected_file().is_none() {
            return false;
        }
        // Reopening after a finished export starts over, so its settings unlock.
        if !matches!(self.export_phase(), ExportPhase::Running { .. }) {
            self.export_ui.task_ids.clear();
            if let Some(id) = self
                .file_queue
                .selected_file()
                .filter(|file| file.status == FileStatus::Completed)
                .map(|file| file.id.clone())
            {
                self.prepare_file_for_reconversion(&id);
            }
        }
        self.export_ui.open = true;
        self.export_ui.present = true;
        true
    }

    pub(super) const fn close_export_dialog(&mut self) -> bool {
        let changed = self.export_ui.open;
        self.export_ui.open = false;
        changed
    }

    const fn finish_export_dialog_close(&mut self) -> bool {
        if self.export_ui.open || !self.export_ui.present {
            return false;
        }
        self.export_ui.present = false;
        true
    }

    fn export_phase(&self) -> ExportPhase {
        export_phase(self.file_queue.files(), &self.export_ui.task_ids)
    }

    fn start_export(&mut self, cx: &mut Context<Self>) {
        let ready = self.file_queue.selected_file().is_some_and(|file| {
            !file.output_name.trim().is_empty() && custom_dimensions_error(&file.config).is_none()
        });
        if !ready {
            return;
        }
        self.start_selected_conversions(cx);
        self.export_ui
            .task_ids
            .clone_from(&self.active_conversion_task_ids);
    }

    fn cancel_export(&mut self) {
        for id in std::mem::take(&mut self.export_ui.task_ids) {
            self.cancel_conversion_task(&id);
        }
    }

    /// Leaves the finished state and unlocks the exported files for another export.
    fn edit_export_settings(&mut self) {
        for id in std::mem::take(&mut self.export_ui.task_ids) {
            self.prepare_file_for_reconversion(&id);
        }
    }

    fn reveal_export(&self, cx: &App) {
        let Some(directory) = self.default_output_directory.as_ref() else {
            return;
        };
        let output = self
            .export_ui
            .task_ids
            .iter()
            .filter_map(|id| self.file_queue.file_by_id(id))
            .find(|file| file.status == FileStatus::Completed)
            .map(|file| {
                PathBuf::from(frame_core::args::build_output_path(
                    &directory.to_string_lossy(),
                    &file.config.container,
                    Some(&file.output_name),
                ))
            });
        match output {
            Some(path) if path.exists() => cx.reveal_path(&path),
            _ => cx.open_with_system(directory),
        }
    }

    /// Keeps custom width and height at the source aspect while the dialog's link is on.
    pub(super) fn sync_export_linked_dimension(&mut self, edited: FrameTextInputKind) {
        if !self.export_ui.open || !self.export_ui.lock_aspect {
            return;
        }
        let metadata = self.selected_source_metadata();
        let Some(file) = self.file_queue.selected_file_mut() else {
            return;
        };
        let config = &mut file.config;
        let Some((source_width, source_height)) = source_dimensions(config, metadata.as_ref())
        else {
            return;
        };
        match edited {
            FrameTextInputKind::VideoCustomWidth => {
                if let Some(width) = parse_dimension(config.custom_width.as_deref()) {
                    config.custom_height =
                        Some(scaled_even(width, source_width, source_height).to_string());
                }
            }
            FrameTextInputKind::VideoCustomHeight => {
                if let Some(height) = parse_dimension(config.custom_height.as_deref()) {
                    config.custom_width =
                        Some(scaled_even(height, source_height, source_width).to_string());
                }
            }
            _ => {}
        }
    }

    pub(super) fn render_export_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let scheme = material::scheme(self.appearance.color_theme);
        let is_open = self.export_ui.open;
        let transition = window
            .use_keyed_transition(
                "export-dialog-motion",
                cx,
                SURFACE_MOTION_DURATION,
                |_window, _cx| 0.0_f32,
            )
            .with_easing(ease_in_out);
        set_motion_target(&transition, motion_target(is_open), cx);
        let visibility = *transition.evaluate(window, cx);
        if !is_open && motion_is_hidden(visibility) {
            cx.defer_in(window, |root, _window, cx| {
                if root.finish_export_dialog_close() {
                    cx.notify();
                }
            });
        }

        let view = self.export_view(scheme);
        let phase = self.export_phase();
        let panel_focus = self.ensure_focus(
            FrameFocusKey::Control("export-dialog-panel".to_string()),
            false,
            cx,
        );
        let close_focus = self.ensure_focus(
            FrameFocusKey::Control("export-dialog-close".to_string()),
            true,
            cx,
        );
        let last_focus = self.ensure_focus(
            FrameFocusKey::Control("export-dialog-last".to_string()),
            true,
            cx,
        );
        if is_open && !panel_focus.contains_focused(window, cx) {
            close_focus.focus(window, cx);
        }

        let body: Vec<gpui::AnyElement> = match phase {
            ExportPhase::Idle => {
                let focuses = ExportInputFocuses {
                    width: self.ensure_text_input_focus(FrameTextInputKind::VideoCustomWidth, cx),
                    height: self.ensure_text_input_focus(FrameTextInputKind::VideoCustomHeight, cx),
                    name: self.ensure_text_input_focus(FrameTextInputKind::ExportName, cx),
                    gif_loop: self.ensure_text_input_focus(FrameTextInputKind::GifLoop, cx),
                };
                vec![
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .border_t_1()
                        .border_b_1()
                        .border_color(color(scheme.outline_variant))
                        .child(Self::export_settings_column(&view, &focuses, window, cx))
                        .child(export_side_panel(&view, &focuses, window, cx))
                        .into_any_element(),
                    export_footer(&view, &last_focus, window, cx).into_any_element(),
                ]
            }
            ExportPhase::Running { .. } | ExportPhase::Done { .. } => {
                vec![Self::export_status(&view, phase, &last_focus, window, cx).into_any_element()]
            }
        };

        let dialog = m3::Dialog::new("export-dialog", view.title)
            .build(scheme)
            .track_focus(&panel_focus)
            .tab_stop(false)
            .w(theme::ui_rem(DIALOG_WIDTH))
            .max_w(relative(1.0))
            .h(theme::ui_rem(DIALOG_HEIGHT))
            .max_h(relative(1.0))
            .mt(theme::ui_rem(
                (1.0 - visibility.clamp(0.0, 1.0)) * DIALOG_ENTER_OFFSET,
            ))
            .child(export_header(&view, &close_focus, window, cx))
            .children(body);

        m3::scrim("export-dialog-scrim", visibility, scheme)
            .on_key_down(
                cx.listener(move |root, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        if root.close_export_dialog() {
                            cx.notify();
                        }
                        return;
                    }
                    handle_modal_tab_navigation(event, &close_focus, &last_focus, window, cx);
                }),
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                cx.stop_propagation();
                if root.close_export_dialog() {
                    cx.notify();
                }
            }))
            .child(dialog)
    }

    fn export_view(&self, scheme: &'static ColorScheme) -> ExportView {
        let metadata = self.selected_source_metadata();
        let file = self.file_queue.selected_file();
        let config = file.map(|file| file.config.clone()).unwrap_or_default();
        let kind = source_kind_for(metadata.as_ref());
        let video_controls = kind == SourceKind::Video
            && config.processing_mode != ProcessingMode::Copy
            && !is_audio_only_container(&config.container);
        let encoders = video_codec_options(&config, &self.available_encoders, false)
            .into_iter()
            .filter(|option| !option.is_disabled)
            .collect();
        let duration =
            output_duration_seconds(&config, preview_duration_seconds(metadata.as_ref()));
        ExportView {
            scheme,
            title: match kind {
                SourceKind::Audio => "Export audio",
                SourceKind::Image => "Export image",
                SourceKind::Video => "Export video",
            },
            source_name: file.map(|file| file.name.clone()).unwrap_or_default(),
            output_name: file
                .map(|file| file.output_name.clone())
                .unwrap_or_default(),
            disabled: self.update_installation_in_progress()
                || self.file_queue.selected_file_locked(),
            kind,
            video_controls,
            encoders,
            duration,
            queued: self
                .file_queue
                .files()
                .iter()
                .filter(|file| {
                    file.is_selected_for_conversion && file.status.is_actionable_for_conversion()
                })
                .count(),
            has_output_directory: self.default_output_directory.is_some(),
            output_directory: self
                .default_output_directory
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned()),
            can_start: self.app_state().can_start_conversion(),
            advanced: self.export_ui.advanced,
            lock_aspect: self.export_ui.lock_aspect,
            scroll_handle: self.export_ui.scroll_handle.clone(),
            metadata,
            config,
            tasks: self
                .export_ui
                .task_ids
                .iter()
                .filter_map(|id| self.file_queue.file_by_id(id))
                .map(|file| {
                    (
                        file.output_name.clone(),
                        file.config.container.clone(),
                        file.conversion_error.clone(),
                    )
                })
                .collect(),
        }
    }

    fn export_settings_column(
        view: &ExportView,
        focuses: &ExportInputFocuses,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let scheme = view.scheme;
        let mut column = div()
            .id("export-dialog-settings")
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(&view.scroll_handle)
            .flex()
            .flex_col()
            .gap(theme::ui_rem(SECTION_GAP))
            .px(theme::ui_rem(24.0))
            .pt(theme::ui_rem(20.0))
            .pb(theme::ui_rem(28.0))
            .text_color(color(scheme.on_surface));

        if view.kind != SourceKind::Video {
            return column.child(info_note(
                match view.kind {
                    SourceKind::Audio => {
                        "Choose the audio format and quality in the Output and Audio settings."
                    }
                    _ => "Choose the image format and size in the Output and Images settings.",
                },
                scheme,
            ));
        }

        column = column.child(format_section(view, window, cx));
        if !view.video_controls {
            return column.child(info_note(
                "Stream copy keeps the source encoding, so size, frame rate and quality can't \
                 change. Switch to re-encoding in the Output settings to adjust them.",
                scheme,
            ));
        }
        column
            .child(resolution_section(view, focuses, window, cx))
            .child(fps_section(view, window, cx))
            .child(quality_section(view, window, cx))
            .child(advanced_section(view, focuses, window, cx))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Progress, success and failure share one layout and differ only in content."
    )]
    fn export_status(
        view: &ExportView,
        phase: ExportPhase,
        last_focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let scheme = view.scheme;
        let count = view.tasks.len();
        let (symbol, icon_color, title, detail) = match phase {
            ExportPhase::Running { progress } => (
                Symbol::Movie,
                scheme.on_surface_variant,
                format!(
                    "Exporting{}… {:.0}%",
                    if count > 1 {
                        format!(" {count} files")
                    } else {
                        String::new()
                    },
                    progress * 100.0
                ),
                "You can close this and keep editing.".to_string(),
            ),
            ExportPhase::Done {
                completed,
                failed: 0,
            } => (
                Symbol::CheckCircle,
                scheme.primary,
                "Export complete".to_string(),
                done_detail(view, completed),
            ),
            ExportPhase::Done { completed, failed } => (
                Symbol::Error,
                scheme.error,
                if completed == 0 {
                    "Export failed".to_string()
                } else {
                    format!("{failed} of {} exports failed", completed + failed)
                },
                view.tasks
                    .iter()
                    .find_map(|(_, _, error)| error.clone())
                    .unwrap_or_else(|| "Check the logs for details.".to_string()),
            ),
            ExportPhase::Idle => unreachable!("the status view only renders for a started export"),
        };

        let mut actions = div().flex().gap(theme::ui_rem(8.0)).pt(theme::ui_rem(16.0));
        match phase {
            ExportPhase::Running { .. } => {
                actions = actions.child(
                    m3::Button::new(
                        "export-cancel",
                        "Cancel export",
                        m3::ButtonVariant::Outlined,
                    )
                    .icon(Symbol::Stop)
                    .focus(last_focus)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(
                        |root, _: &ClickEvent, _window, cx| {
                            root.cancel_export();
                            cx.notify();
                        },
                    )),
                );
            }
            ExportPhase::Done { completed, .. } => {
                actions = actions
                    .child(
                        m3::Button::new(
                            "export-change-settings",
                            "Change settings",
                            m3::ButtonVariant::Outlined,
                        )
                        .icon(Symbol::Tune)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(
                            |root, _: &ClickEvent, _window, cx| {
                                root.edit_export_settings();
                                cx.notify();
                            },
                        )),
                    )
                    .when(completed > 0 && view.has_output_directory, |this| {
                        this.child(
                            m3::Button::new(
                                "export-show-in-folder",
                                "Show in folder",
                                m3::ButtonVariant::Tonal,
                            )
                            .icon(Symbol::FolderOpen)
                            .build(scheme, window, cx)
                            .on_click(cx.listener(
                                |root, _: &ClickEvent, _window, cx| {
                                    root.reveal_export(cx);
                                },
                            )),
                        )
                    })
                    .child(
                        m3::Button::new("export-done", "Done", m3::ButtonVariant::Filled)
                            .focus(last_focus)
                            .build(scheme, window, cx)
                            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                                if root.close_export_dialog() {
                                    cx.notify();
                                }
                            })),
                    );
            }
            ExportPhase::Idle => {}
        }

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(theme::ui_rem(16.0))
            .px(theme::ui_rem(48.0))
            .py(theme::ui_rem(24.0))
            .border_t_1()
            .border_color(color(scheme.outline_variant))
            .text_center()
            .text_color(color(scheme.on_surface))
            .child(m3::m3_icon(
                symbol,
                false,
                STATUS_ICON_SIZE,
                color(icon_color),
            ))
            .child(div().m3_type(typography::HEADLINE_SMALL).child(title))
            .child(
                div()
                    .m3_type(typography::BODY_MEDIUM)
                    .text_color(color(scheme.on_surface_variant))
                    .child(detail),
            )
            .when_some(
                match phase {
                    ExportPhase::Running { progress } => Some(progress),
                    _ => None,
                },
                |this, progress| {
                    this.child(
                        div()
                            .w(theme::ui_rem(PROGRESS_WIDTH))
                            .max_w_full()
                            .pt(theme::ui_rem(8.0))
                            .child(
                                m3::LinearProgress::new(
                                    "export-progress",
                                    "Export progress",
                                    progress,
                                )
                                .build(scheme),
                            ),
                    )
                },
            )
            .child(actions)
    }
}

/// A snapshot of everything the dialog shows, taken once per render.
#[expect(
    clippy::struct_excessive_bools,
    reason = "The snapshot mirrors independent facts about the selection and app state."
)]
struct ExportView {
    scheme: &'static ColorScheme,
    title: &'static str,
    config: ConversionConfig,
    metadata: Option<SourceMetadata>,
    source_name: String,
    output_name: String,
    disabled: bool,
    kind: SourceKind,
    /// The source is video being re-encoded, so size, rate and quality apply.
    video_controls: bool,
    encoders: Vec<VideoCodecOption>,
    /// Exported length in seconds, after trims and cuts.
    duration: f64,
    /// Files the Export button will start.
    queued: usize,
    has_output_directory: bool,
    output_directory: Option<String>,
    can_start: bool,
    advanced: bool,
    lock_aspect: bool,
    scroll_handle: ScrollHandle,
    /// Output name, container and error of each started file.
    tasks: Vec<(String, String, Option<String>)>,
}

impl ExportView {
    const fn metadata(&self) -> Option<&SourceMetadata> {
        self.metadata.as_ref()
    }

    fn size_estimate(&self) -> Option<String> {
        estimate_size_bytes(&self.config, self.metadata(), self.duration).map(format_size)
    }

    fn name_error(&self) -> Option<&'static str> {
        self.output_name
            .trim()
            .is_empty()
            .then_some("Enter a file name")
    }

    fn encoder_label(&self) -> &'static str {
        self.encoders
            .iter()
            .find(|option| option.is_selected)
            .map_or("Default encoder", |option| option.label)
    }

    fn export_enabled(&self) -> bool {
        self.can_start
            && self.name_error().is_none()
            && (!self.video_controls || custom_dimensions_error(&self.config).is_none())
    }
}

struct ExportInputFocuses {
    width: FocusHandle,
    height: FocusHandle,
    name: FocusHandle,
    gif_loop: FocusHandle,
}

fn done_detail(view: &ExportView, completed: usize) -> String {
    let folder = view
        .output_directory
        .as_deref()
        .map_or_else(String::new, |folder| format!(" · saved to {folder}"));
    match view.tasks.as_slice() {
        [(name, container, _)] => format!("{name}.{container}{folder}"),
        _ => format!("{completed} files{folder}"),
    }
}

fn export_header(
    view: &ExportView,
    close_focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let mut subtitle = vec![view.source_name.clone()];
    if view.duration > 0.0 {
        subtitle.push(format_clock(view.duration));
    }
    if let Some((width, height)) = source_dimensions(&view.config, view.metadata()) {
        let rate = view
            .metadata()
            .and_then(|metadata| metadata.frame_rate)
            .map_or_else(String::new, |rate| format!(", {} fps", format_rate(rate)));
        subtitle.push(format!("source {width} × {height}{rate}"));
    }

    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(theme::ui_rem(8.0))
        .pt(theme::ui_rem(20.0))
        .pr(theme::ui_rem(16.0))
        .pb(theme::ui_rem(12.0))
        .pl(theme::ui_rem(24.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(2.0))
                .child(
                    div()
                        .m3_type(typography::HEADLINE_SMALL)
                        .text_color(color(scheme.on_surface))
                        .child(view.title),
                )
                .child(
                    div()
                        .m3_type(typography::BODY_MEDIUM)
                        .truncate()
                        .child(subtitle.join(" · ")),
                ),
        )
        .child(
            m3::IconButton::new("export-dialog-close", Symbol::Close, "Close")
                .focus(close_focus)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                    cx.stop_propagation();
                    if root.close_export_dialog() {
                        cx.notify();
                    }
                })),
        )
}

fn section(title: &'static str, scheme: &'static ColorScheme) -> gpui::Div {
    div().flex().flex_col().gap(theme::ui_rem(12.0)).child(
        div()
            .m3_type(typography::TITLE_SMALL)
            .text_color(color(scheme.on_surface))
            .child(title),
    )
}

/// A section whose heading has a short note at the trailing edge.
fn noted_section(
    title: &'static str,
    note: impl Into<SharedString>,
    note_color: crate::theme::RgbaToken,
    scheme: &'static ColorScheme,
) -> gpui::Div {
    div().flex().flex_col().gap(theme::ui_rem(12.0)).child(
        div()
            .flex()
            .items_baseline()
            .justify_between()
            .gap(theme::ui_rem(12.0))
            .child(
                div()
                    .flex_none()
                    .m3_type(typography::TITLE_SMALL)
                    .text_color(color(scheme.on_surface))
                    .child(title),
            )
            .child(
                div()
                    .min_w_0()
                    .text_right()
                    .m3_type(typography::BODY_SMALL)
                    .text_color(color(note_color))
                    .child(note.into()),
            ),
    )
}

fn subsection_label(label: &'static str, scheme: &'static ColorScheme) -> gpui::Div {
    div()
        .m3_type(typography::LABEL_LARGE)
        .text_color(color(scheme.on_surface_variant))
        .child(label)
}

fn info_note(text: &'static str, scheme: &'static ColorScheme) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .px(theme::ui_rem(16.0))
        .py(theme::ui_rem(12.0))
        .rounded(theme::ui_rem(shape::CORNER_MEDIUM))
        .bg(color(scheme.surface_container_highest))
        .text_color(color(scheme.on_surface_variant))
        .m3_type(typography::BODY_MEDIUM)
        .child(m3::m3_icon(
            Symbol::Info,
            false,
            24.0,
            color(scheme.on_surface_variant),
        ))
        .child(div().flex_1().min_w_0().child(text))
}

fn value_badge(text: String, scheme: &'static ColorScheme) -> gpui::Div {
    div()
        .flex_none()
        .px(theme::ui_rem(10.0))
        .py(theme::ui_rem(4.0))
        .rounded(theme::ui_rem(shape::CORNER_SMALL))
        .bg(color(scheme.secondary_container))
        .text_color(color(scheme.on_secondary_container))
        .m3_type(typography::LABEL_LARGE)
        .child(text)
}

/// Labels under a slider, one at each end.
fn range_labels(
    lower: &'static str,
    upper: &'static str,
    scheme: &'static ColorScheme,
) -> gpui::Div {
    div()
        .flex()
        .justify_between()
        .m3_type(typography::LABEL_MEDIUM)
        .text_color(color(scheme.on_surface_variant))
        .child(lower)
        .child(upper)
}

const fn text_input_colors(
    focused: bool,
    error: bool,
    scheme: &ColorScheme,
) -> FrameTextInputColors {
    let colors = m3::text_field_colors(focused, error, scheme);
    FrameTextInputColors {
        text: colors.input,
        placeholder: colors.affix,
        caret: colors.caret,
        selection: colors.selection,
    }
}

struct ExportTextField {
    id: &'static str,
    input_id: &'static str,
    label: &'static str,
    value: String,
    kind: FrameTextInputKind,
    suffix: Option<String>,
    supporting: Option<&'static str>,
    error: Option<&'static str>,
    container: crate::theme::RgbaToken,
}

fn export_text_field(
    field: ExportTextField,
    focus: &FocusHandle,
    disabled: bool,
    scheme: &'static ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let focused = focus.is_focused(window);
    let error = field.error.is_some();
    let input = m3_text_input(
        FrameTextInputSpec {
            id: field.input_id,
            value: &field.value,
            placeholder: "",
            disabled,
            focus: Some(focus),
            kind: field.kind,
        },
        text_input_colors(focused, error, scheme),
        cx,
    );
    let mut text_field = m3::TextField::new(field.id, field.label)
        .input(input)
        .populated(!field.value.is_empty())
        .focused(focused)
        .enabled(!disabled)
        .error(field.error)
        .container(field.container);
    if let Some(suffix) = field.suffix {
        text_field = text_field.suffix(suffix);
    }
    if let Some(supporting) = field.supporting {
        text_field = text_field.supporting_text(supporting);
    }
    text_field.build(scheme, window, cx)
}

#[expect(
    clippy::too_many_arguments,
    reason = "A dialog slider ties its value, range, readout and edit target together in one call."
)]
fn export_slider(
    id: &'static str,
    label: &'static str,
    value: u32,
    min: u32,
    max: u32,
    value_text: String,
    slider: ExportSlider,
    enabled: bool,
    scheme: &'static ColorScheme,
    cx: &Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let drag = ExportSliderDrag { slider, min, max };
    m3::Slider::new(id, label, f64::from(value), f64::from(min), f64::from(max))
        .value_text(value_text)
        .enabled(enabled)
        .build(scheme)
        .when(enabled, |this| {
            this.on_drag(drag, |_drag, _position, _window, cx| {
                cx.new(|_| ExportSliderDragPreview)
            })
            .on_drag_move(cx.listener(
                move |root, event: &DragMoveEvent<ExportSliderDrag>, _window, cx| {
                    // Every slider sees every drag of this type; act only on our own.
                    let drag = *event.drag(cx);
                    if drag.slider != slider {
                        return;
                    }
                    let fraction =
                        timeline_slider_percent_from_bounds(event.event.position, event.bounds);
                    let value = range_value_from_fraction(fraction, drag.min, drag.max);
                    if root.update_selected_config(|config| {
                        apply_export_slider(config, drag.slider, value)
                    }) {
                        cx.notify();
                    }
                },
            ))
            .on_key_down(cx.listener(
                move |root, event: &gpui::KeyDownEvent, _window, cx| {
                    let Some(value) =
                        range_value_for_key(value, min, max, event.keystroke.key.as_str())
                    else {
                        return;
                    };
                    cx.stop_propagation();
                    if root
                        .update_selected_config(|config| apply_export_slider(config, slider, value))
                    {
                        cx.notify();
                    }
                },
            ))
        })
}

fn format_section(
    view: &ExportView,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let available = output_container_options(&view.config, view.metadata(), view.disabled);
    let cards: Vec<_> = EXPORT_FORMATS
        .iter()
        .map(|format| {
            let container = format.container;
            let enabled = available
                .iter()
                .any(|option| option.container == container && !option.is_disabled);
            m3::SelectCard::new(
                format!("export-format-{container}"),
                format.label,
                view.config.container == container,
            )
            .icon(format.symbol)
            .supporting(format.description)
            .enabled(enabled)
            .build(scheme, window, cx)
            .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                if root.update_selected_config(|config| apply_export_format(config, container)) {
                    cx.notify();
                }
            }))
        })
        .collect();
    section("Format", scheme).child(
        div()
            .id("export-formats")
            .role(gpui::Role::RadioGroup)
            .aria_label("Format")
            .grid()
            .grid_cols(4)
            .gap(theme::ui_rem(8.0))
            .children(cards),
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "The resolution choice and its custom size row read top to bottom like the UI."
)]
fn resolution_section(
    view: &ExportView,
    focuses: &ExportInputFocuses,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let config = &view.config;
    let error = custom_dimensions_error(config);
    let fill = output_dimensions(config, view.metadata());
    let selected = RESOLUTION_OPTIONS
        .iter()
        .position(|(value, _)| *value == config.resolution);
    let mut section = noted_section(
        "Resolution",
        resolution_note(config, view.metadata()),
        if error.is_some() {
            scheme.error
        } else {
            scheme.on_surface_variant
        },
        scheme,
    )
    .child(
        m3::SegmentedButton::new(
            "export-resolution",
            "Resolution",
            RESOLUTION_OPTIONS.iter().map(|(_, label)| *label),
        )
        .selected(selected)
        .enabled(!view.disabled)
        .on_select(cx.listener(move |root, index: &usize, _window, cx| {
            let Some((resolution, _)) = RESOLUTION_OPTIONS.get(*index) else {
                return;
            };
            if root.update_selected_config(|config| {
                let changed = apply_resolution(config, resolution);
                // Start a custom size from the current one rather than blank fields.
                let prefill = (*resolution == "custom"
                    && config.custom_width.is_none()
                    && config.custom_height.is_none())
                .then_some(fill)
                .flatten();
                if let Some((width, height)) = prefill {
                    config.custom_width = Some(width.to_string());
                    config.custom_height = Some(height.to_string());
                }
                changed || prefill.is_some()
            }) {
                cx.notify();
            }
        }))
        .build(scheme, window, cx),
    );

    if config.resolution == "custom" {
        let container = scheme.surface_container_high;
        section = section.child(
            div()
                .flex()
                .items_start()
                .gap(theme::ui_rem(12.0))
                .child(div().flex_1().min_w_0().child(export_text_field(
                    ExportTextField {
                        id: "export-width",
                        input_id: "export-width-input",
                        label: "Width",
                        value: config.custom_width.clone().unwrap_or_default(),
                        kind: FrameTextInputKind::VideoCustomWidth,
                        suffix: Some("px".to_string()),
                        supporting: None,
                        error: None,
                        container,
                    },
                    &focuses.width,
                    view.disabled,
                    scheme,
                    window,
                    cx,
                )))
                .child(
                    div().pt(theme::ui_rem(8.0)).child(
                        m3::IconButton::new(
                            "export-lock-aspect",
                            if view.lock_aspect {
                                Symbol::Link
                            } else {
                                Symbol::LinkOff
                            },
                            "Keep aspect ratio",
                        )
                        .toggle(view.lock_aspect)
                        .enabled(!view.disabled)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(
                            |root, _: &ClickEvent, _window, cx| {
                                root.export_ui.lock_aspect = !root.export_ui.lock_aspect;
                                cx.notify();
                            },
                        )),
                    ),
                )
                .child(div().flex_1().min_w_0().child(export_text_field(
                    ExportTextField {
                        id: "export-height",
                        input_id: "export-height-input",
                        label: "Height",
                        value: config.custom_height.clone().unwrap_or_default(),
                        kind: FrameTextInputKind::VideoCustomHeight,
                        suffix: Some("px".to_string()),
                        supporting: None,
                        error: None,
                        container,
                    },
                    &focuses.height,
                    view.disabled,
                    scheme,
                    window,
                    cx,
                ))),
        );
    }
    section
}

fn fps_section(view: &ExportView, window: &mut Window, cx: &mut Context<FrameRoot>) -> gpui::Div {
    let scheme = view.scheme;
    let config = &view.config;
    let options = fps_options(&config.container);
    let source_fps = view.metadata().and_then(|metadata| metadata.frame_rate);
    let (note, warn) = fps_note(config, view.metadata());
    noted_section(
        "Frame rate",
        note,
        if warn {
            scheme.tertiary
        } else {
            scheme.on_surface_variant
        },
        scheme,
    )
    .child(
        m3::SegmentedButton::new(
            "export-fps",
            "Frame rate",
            options.iter().map(|fps| fps_option_label(fps, source_fps)),
        )
        .selected(options.iter().position(|fps| *fps == config.fps))
        .enabled(!view.disabled)
        .on_select(cx.listener(move |root, index: &usize, _window, cx| {
            if let Some(fps) = options.get(*index)
                && root.update_selected_config(|config| apply_fps(config, fps))
            {
                cx.notify();
            }
        }))
        .build(scheme, window, cx),
    )
}

fn quality_section(
    view: &ExportView,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let config = &view.config;
    let section = section("Quality", scheme);
    if is_gif_container(&config.container) {
        return section.child(info_note(
            "GIFs use a palette of up to 256 colors and have no sound. Lower the resolution or \
             frame rate to shrink the file.",
            scheme,
        ));
    }

    let control = quality_control(config);
    let enabled = !view.disabled;
    section
        .when(control.mode_selectable, |this| {
            this.child(
                m3::SegmentedButton::new(
                    "export-quality-mode",
                    "Quality mode",
                    ["Constant quality", "Target bitrate"],
                )
                .selected(Some(usize::from(control.target == QualityTarget::Bitrate)))
                .enabled(enabled)
                .on_select(cx.listener(|root, index: &usize, _window, cx| {
                    let mode = if *index == 0 { "crf" } else { "bitrate" };
                    if root.update_selected_config(|config| apply_video_bitrate_mode(config, mode))
                    {
                        cx.notify();
                    }
                }))
                .build(scheme, window, cx),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(theme::ui_rem(12.0))
                .child(
                    div()
                        .min_w_0()
                        .m3_type(typography::BODY_MEDIUM)
                        .text_color(color(scheme.on_surface_variant))
                        .child(control.hint),
                )
                .child(value_badge(control.badge.clone(), scheme)),
        )
        .child(export_slider(
            "export-quality-slider",
            "Quality",
            control.value,
            control.min,
            control.max,
            control.badge,
            ExportSlider::Quality(control.target),
            enabled,
            scheme,
            cx,
        ))
        .child(range_labels(control.lower, control.upper, scheme))
}

fn advanced_section(
    view: &ExportView,
    focuses: &ExportInputFocuses,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let config = &view.config;
    let hover = state::state_layer(scheme.transparent, scheme.on_surface, state::HOVER_OPACITY);
    let toggle = div()
        .id("export-advanced-toggle")
        .role(gpui::Role::Button)
        .aria_label("Advanced encoding")
        .aria_expanded(view.advanced)
        .focusable()
        .tab_stop(true)
        .focus_visible(move |style| {
            style
                .ring_width(px(state::FOCUS_INDICATOR_THICKNESS))
                .ring_color(color(scheme.secondary))
        })
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .min_h(theme::ui_rem(56.0))
        .px(theme::ui_rem(8.0))
        .mx(theme::ui_rem(-8.0))
        .rounded(theme::ui_rem(shape::CORNER_MEDIUM))
        .hover(move |style| style.bg(color(hover)))
        .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
            root.export_ui.advanced = !root.export_ui.advanced;
            cx.notify();
        }))
        .on_key_down(
            cx.listener(|root, event: &gpui::KeyDownEvent, _window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    root.export_ui.advanced = !root.export_ui.advanced;
                    cx.notify();
                }
            }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .m3_type(typography::TITLE_SMALL)
                        .child("Advanced encoding"),
                )
                .child(
                    div()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .truncate()
                        .child(advanced_summary(config, view.encoder_label())),
                ),
        )
        .child(m3::m3_icon(
            if view.advanced {
                Symbol::ExpandLess
            } else {
                Symbol::ExpandMore
            },
            false,
            24.0,
            color(scheme.on_surface_variant),
        ));

    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(4.0))
        .pt(theme::ui_rem(8.0))
        .border_t_1()
        .border_color(color(scheme.outline_variant))
        .child(toggle)
        .when(view.advanced, |this| {
            this.child(advanced_options(view, focuses, window, cx))
        })
}

#[expect(
    clippy::too_many_lines,
    reason = "The advanced options keep encoder, speed, pixel format and scaling in UI order."
)]
fn advanced_options(
    view: &ExportView,
    focuses: &ExportInputFocuses,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let config = &view.config;
    let enabled = !view.disabled;
    let is_gif = is_gif_container(&config.container);

    let encoders: Vec<_> = view
        .encoders
        .iter()
        .map(|option| {
            let codec = option.codec;
            m3::RadioItem::new(
                format!("export-encoder-{codec}"),
                option.label,
                option.is_selected,
            )
            .supporting(encoder_description(codec))
            .enabled(enabled)
            .build(scheme, window, cx)
            .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                if root.update_selected_config(|config| apply_video_codec(config, codec)) {
                    cx.notify();
                }
            }))
        })
        .collect();
    let mut options = div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(24.0))
        .pt(theme::ui_rem(8.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(4.0))
                .child(subsection_label("Encoder", scheme).pb(theme::ui_rem(4.0)))
                .children(encoders),
        );

    let presets = speed_presets(&config.video_codec);
    if let Some(index) = presets
        .iter()
        .position(|(preset, _)| *preset == config.preset)
    {
        let label = presets[index].1;
        let default = config.preset == crate::settings::DEFAULT_PRESET;
        let max = u32::try_from(presets.len() - 1).unwrap_or(0);
        options = options.child(
            div()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(8.0))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .gap(theme::ui_rem(12.0))
                        .child(subsection_label("Encoding speed", scheme))
                        .child(div().m3_type(typography::BODY_MEDIUM).child(if default {
                            format!("{label} · default")
                        } else {
                            label.to_string()
                        })),
                )
                .child(export_slider(
                    "export-speed-slider",
                    "Encoding speed",
                    u32::try_from(index).unwrap_or(0),
                    0,
                    max,
                    label.to_string(),
                    ExportSlider::Speed,
                    enabled,
                    scheme,
                    cx,
                ))
                .child(range_labels(
                    "Faster, larger file",
                    "Slower, smaller file",
                    scheme,
                )),
        );
    }

    if !is_gif {
        let pixel_formats: Vec<_> = video_pixel_format_options(config)
            .into_iter()
            .filter(|option| !option.is_disabled)
            .map(|option| {
                let id = option.id;
                let (label, description) = pixel_format_text(id);
                m3::RadioItem::new(
                    format!("export-pixel-format-{id}"),
                    label,
                    option.is_selected,
                )
                .supporting(description)
                .enabled(enabled)
                .build(scheme, window, cx)
                .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                    if root.update_selected_config(|config| apply_pixel_format(config, id)) {
                        cx.notify();
                    }
                }))
            })
            .collect();
        options = options.child(
            div()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(4.0))
                .child(subsection_label("Pixel format", scheme).pb(theme::ui_rem(4.0)))
                .children(pixel_formats),
        );
    }

    options = options.child(
        div()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(12.0))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap(theme::ui_rem(12.0))
                    .child(subsection_label("Scaling algorithm", scheme))
                    .child(
                        div()
                            .m3_type(typography::BODY_SMALL)
                            .text_color(color(scheme.on_surface_variant))
                            .child(scaling_note(config, view.metadata())),
                    ),
            )
            .child(
                m3::SegmentedButton::new(
                    "export-scaling",
                    "Scaling algorithm",
                    SCALING_OPTIONS.iter().map(|(_, label)| *label),
                )
                .selected(
                    SCALING_OPTIONS
                        .iter()
                        .position(|(id, _)| *id == config.scaling_algorithm),
                )
                .enabled(enabled)
                .on_select(cx.listener(|root, index: &usize, _window, cx| {
                    if let Some((algorithm, _)) = SCALING_OPTIONS.get(*index)
                        && root.update_selected_config(|config| {
                            apply_scaling_algorithm(config, algorithm)
                        })
                    {
                        cx.notify();
                    }
                }))
                .build(scheme, window, cx),
            ),
    );

    if is_gif {
        options = options.child(gif_options(view, focuses, window, cx));
    }
    options.child(hardware_options(view, window, cx))
}

fn gif_options(
    view: &ExportView,
    focuses: &ExportInputFocuses,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let config = &view.config;
    let enabled = !view.disabled;
    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(24.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(12.0))
                .child(subsection_label("Palette colors", scheme))
                .child(
                    m3::SegmentedButton::new(
                        "export-gif-colors",
                        "Palette colors",
                        GIF_COLOR_OPTIONS.iter().map(ToString::to_string),
                    )
                    .selected(
                        GIF_COLOR_OPTIONS
                            .iter()
                            .position(|colors| *colors == config.gif_colors),
                    )
                    .enabled(enabled)
                    .on_select(cx.listener(|root, index: &usize, _window, cx| {
                        if let Some(colors) = GIF_COLOR_OPTIONS.get(*index)
                            && root
                                .update_selected_config(|config| apply_gif_colors(config, *colors))
                        {
                            cx.notify();
                        }
                    }))
                    .build(scheme, window, cx),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(12.0))
                .child(subsection_label("Dithering", scheme))
                .child(
                    m3::SegmentedButton::new(
                        "export-gif-dither",
                        "Dithering",
                        GIF_DITHER_OPTIONS.iter().map(|(_, label)| *label),
                    )
                    .selected(
                        GIF_DITHER_OPTIONS
                            .iter()
                            .position(|(id, _)| *id == config.gif_dither),
                    )
                    .enabled(enabled)
                    .on_select(cx.listener(|root, index: &usize, _window, cx| {
                        if let Some((dither, _)) = GIF_DITHER_OPTIONS.get(*index)
                            && root
                                .update_selected_config(|config| apply_gif_dither(config, dither))
                        {
                            cx.notify();
                        }
                    }))
                    .build(scheme, window, cx),
                ),
        )
        .child(export_text_field(
            ExportTextField {
                id: "export-gif-loop",
                input_id: "export-gif-loop-input",
                label: "Loop count",
                value: config.gif_loop.to_string(),
                kind: FrameTextInputKind::GifLoop,
                suffix: None,
                supporting: Some("0 loops forever"),
                error: None,
                container: scheme.surface_container_high,
            },
            &focuses.gif_loop,
            view.disabled,
            scheme,
            window,
            cx,
        ))
}

/// Encoder-specific switches for hardware encoders. Empty for software encoders.
fn hardware_options(
    view: &ExportView,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let config = &view.config;
    let enabled = !view.disabled;
    let codec = config.video_codec.as_str();
    let mut rows = div().flex().flex_col().gap(theme::ui_rem(4.0));
    if is_nvenc_video_codec(codec) {
        rows = rows
            .child(switch_row(
                SwitchRow {
                    id: "export-nvenc-spatial-aq",
                    label: "Spatial AQ",
                    detail: "More detail in complex scenes",
                    checked: config.nvenc_spatial_aq,
                    enabled,
                    toggle: |config| apply_nvenc_spatial_aq(config, !config.nvenc_spatial_aq),
                },
                scheme,
                window,
                cx,
            ))
            .child(switch_row(
                SwitchRow {
                    id: "export-nvenc-temporal-aq",
                    label: "Temporal AQ",
                    detail: "Steadier quality between frames",
                    checked: config.nvenc_temporal_aq,
                    enabled,
                    toggle: |config| apply_nvenc_temporal_aq(config, !config.nvenc_temporal_aq),
                },
                scheme,
                window,
                cx,
            ));
    }
    if is_videotoolbox_video_codec(codec) {
        rows = rows.child(switch_row(
            SwitchRow {
                id: "export-videotoolbox-allow-sw",
                label: "Allow software fallback",
                detail: "Use the CPU if hardware encoding fails",
                checked: config.videotoolbox_allow_sw,
                enabled,
                toggle: |config| apply_videotoolbox_allow_sw(config, !config.videotoolbox_allow_sw),
            },
            scheme,
            window,
            cx,
        ));
    }
    if is_hardware_video_codec(codec) {
        rows = rows.child(switch_row(
            SwitchRow {
                id: "export-hw-decode",
                label: "Hardware decoding",
                detail: "Decode the source on the GPU too",
                checked: config.hw_decode,
                enabled,
                toggle: |config| apply_hw_decode(config, !config.hw_decode),
            },
            scheme,
            window,
            cx,
        ));
    }
    rows
}

#[derive(Clone, Copy)]
struct SwitchRow {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    checked: bool,
    enabled: bool,
    toggle: fn(&mut ConversionConfig) -> bool,
}

fn switch_row(
    row: SwitchRow,
    scheme: &'static ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let toggle = row.toggle;
    div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(16.0))
        .min_h(theme::ui_rem(56.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .m3_type(typography::BODY_LARGE)
                        .text_color(color(scheme.on_surface))
                        .child(row.label),
                )
                .child(
                    div()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .child(row.detail),
                ),
        )
        .child(
            m3::Switch::new(row.id, row.label, row.checked)
                .enabled(row.enabled)
                .build(scheme, window, cx)
                .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                    if root.update_selected_config(toggle) {
                        cx.notify();
                    }
                })),
        )
}

fn export_side_panel(
    view: &ExportView,
    focuses: &ExportInputFocuses,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let scheme = view.scheme;
    let config = &view.config;
    let dimensions = if view.kind == SourceKind::Audio {
        None
    } else {
        output_dimensions(config, view.metadata())
    };
    div()
        .id("export-dialog-summary")
        .w(theme::ui_rem(SIDE_PANEL_WIDTH))
        .flex_none()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(16.0))
        .p(theme::ui_rem(SIDE_PANEL_PADDING))
        .bg(color(scheme.surface_container))
        .text_color(color(scheme.on_surface))
        .child(preview_frame(view.kind, dimensions, scheme))
        .child(export_text_field(
            ExportTextField {
                id: "export-name",
                input_id: "export-name-input",
                label: "File name",
                value: view.output_name.clone(),
                kind: FrameTextInputKind::ExportName,
                suffix: Some(format!(".{}", config.container)),
                supporting: None,
                error: view.name_error(),
                container: scheme.surface_container,
            },
            &focuses.name,
            view.disabled,
            scheme,
            window,
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .children(
                    summary_rows(view, dimensions)
                        .into_iter()
                        .map(|(key, value)| {
                            div()
                                .min_h(theme::ui_rem(SUMMARY_ROW_HEIGHT))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(theme::ui_rem(12.0))
                                .m3_type(typography::BODY_MEDIUM)
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(color(scheme.on_surface_variant))
                                        .child(key),
                                )
                                .child(div().min_w_0().text_right().child(value))
                        }),
                ),
        )
        .child(div().flex_1())
        .when(view.kind == SourceKind::Video, |this| {
            this.child(
                div().flex().child(
                    m3::Button::new(
                        "export-reset",
                        "Reset to recommended",
                        m3::ButtonVariant::Text,
                    )
                    .icon(Symbol::RestartAlt)
                    .enabled(!view.disabled && view.video_controls && !is_recommended(config))
                    .build(scheme, window, cx)
                    .on_click(cx.listener(
                        |root, _: &ClickEvent, _window, cx| {
                            if root.update_selected_config(reset_to_recommended) {
                                cx.notify();
                            }
                        },
                    )),
                ),
            )
        })
}

/// A placeholder frame drawn at the output aspect ratio, labelled with the output size.
fn preview_frame(
    kind: SourceKind,
    dimensions: Option<(u32, u32)>,
    scheme: &'static ColorScheme,
) -> gpui::Div {
    let inner_width = SIDE_PANEL_PADDING.mul_add(-2.0, SIDE_PANEL_WIDTH);
    let (width, height) = dimensions.map_or((16.0, 9.0), |(width, height)| {
        (f64::from(width), f64::from(height))
    });
    let aspect = crate::numeric::f64_to_f32(height / width);
    let frame_height = (inner_width * aspect).min(PREVIEW_MAX_HEIGHT);
    let frame_width = frame_height / aspect;
    div().flex().justify_center().child(
        m3::m3_surface(scheme.surface_container_highest, shape::CORNER_MEDIUM)
            .w(theme::ui_rem(frame_width))
            .h(theme::ui_rem(frame_height))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(theme::ui_rem(4.0))
            .text_color(color(scheme.on_surface_variant))
            .child(m3::m3_icon(
                match kind {
                    SourceKind::Audio => Symbol::MusicNote,
                    SourceKind::Image => Symbol::Image,
                    SourceKind::Video => Symbol::Movie,
                },
                false,
                32.0,
                color(scheme.on_surface_variant),
            ))
            .when_some(dimensions, |this, dimensions| {
                this.child(
                    div()
                        .m3_type(typography::LABEL_MEDIUM)
                        .child(format_dimensions(Some(dimensions))),
                )
            }),
    )
}

fn summary_rows(view: &ExportView, dimensions: Option<(u32, u32)>) -> Vec<(&'static str, String)> {
    let config = &view.config;
    let mut rows = vec![("Format", config.container.to_uppercase())];
    if view.kind == SourceKind::Video {
        rows.push((
            "Video",
            if view.video_controls {
                if config.pixel_format == "auto" || is_gif_container(&config.container) {
                    view.encoder_label().to_string()
                } else {
                    format!(
                        "{} · {}",
                        view.encoder_label(),
                        pixel_format_text(&config.pixel_format).0
                    )
                }
            } else {
                "Copied from source".to_string()
            },
        ));
    }
    if view.kind != SourceKind::Audio {
        rows.push(("Size", format_dimensions(dimensions)));
    }
    if view.kind == SourceKind::Video {
        rows.push((
            "Frame rate",
            output_fps(config, view.metadata()).map_or_else(
                || "Source".to_string(),
                |fps| format!("{} fps", format_rate(fps)),
            ),
        ));
    }
    if view.kind != SourceKind::Image {
        rows.push(("Audio", audio_summary(config, view.metadata())));
    }
    if let Some(size) = view.size_estimate() {
        rows.push(("Est. file size", format!("≈ {size}")));
    }
    rows
}

fn export_footer(
    view: &ExportView,
    last_focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = view.scheme;
    let note = if view.has_output_directory {
        let mut parts = Vec::new();
        if let Some(size) = view.size_estimate() {
            parts.push(format!("About {size}"));
        }
        if view.queued > 1 {
            parts.push(format!("exports {} files", view.queued));
        }
        let note = parts.join(" · ");
        let mut chars = note.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    } else {
        "Choose an output folder to export to.".to_string()
    };

    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(theme::ui_rem(8.0))
        .px(theme::ui_rem(24.0))
        .py(theme::ui_rem(16.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .m3_type(typography::BODY_MEDIUM)
                .text_color(color(scheme.on_surface_variant))
                .child(note),
        )
        .when(!view.has_output_directory, |this| {
            this.child(
                m3::Button::new(
                    "export-choose-folder",
                    "Choose folder",
                    m3::ButtonVariant::Text,
                )
                .icon(Symbol::FolderOpen)
                .build(scheme, window, cx)
                .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                    FrameRoot::prompt_default_output_folder(window, cx);
                })),
            )
        })
        .child(
            m3::Button::new("export-cancel-dialog", "Cancel", m3::ButtonVariant::Text)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                    if root.close_export_dialog() {
                        cx.notify();
                    }
                })),
        )
        .child(
            m3::Button::new("export-start", "Export", m3::ButtonVariant::Filled)
                .icon(Symbol::FileExport)
                .enabled(view.export_enabled())
                .focus(last_focus)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                    root.start_export(cx);
                    cx.notify();
                })),
        )
}
