//! The export dialog's encoder output: a switch under the progress that reveals the export's
//! log, kept scrolled to the newest line until the user scrolls up.

use super::{
    ClickEvent, ClipboardItem, Context, ExportPhase, FileStatus, FrameRoot, InteractiveElement,
    IntoElement, LogLine, M3Text, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement,
    ScrollStrategy, ScrollWheelEvent, StatefulInteractiveElement, Styled, UniformListScrollHandle,
    Window, color, div, m3, theme, uniform_list,
};
use crate::material::{ColorScheme, Symbol, shape, typography};
use crate::numeric::usize_to_f32;
use std::time::Duration;

const LOG_LINE_HEIGHT: f32 = 18.0;
const LOG_TEXT_SIZE: f32 = 12.0;
const LOG_HEADER_HEIGHT: f32 = 48.0;
const LOG_MIN_HEIGHT: f32 = 160.0;
const COPY_FEEDBACK_DURATION: Duration = Duration::from_millis(1_200);

pub(super) struct ExportLogState {
    scroll_handle: UniformListScrollHandle,
    /// Scrolls to each new line until the user scrolls away from the end.
    follow_tail: bool,
    /// Line count the list was last scrolled for, so new lines can be detected.
    scrolled_line_count: usize,
    copied: bool,
    copy_epoch: usize,
}

impl Default for ExportLogState {
    fn default() -> Self {
        Self {
            scroll_handle: UniformListScrollHandle::new(),
            follow_tail: true,
            scrolled_line_count: 0,
            copied: false,
            copy_epoch: 0,
        }
    }
}

/// What the log panel shows, taken once per render.
pub(super) struct ExportLogView {
    pub(super) shown: bool,
    task_id: Option<String>,
    line_count: usize,
    copied: bool,
    scroll_handle: UniformListScrollHandle,
}

impl FrameRoot {
    /// The started file whose output the panel shows: the one still running, else the last one.
    fn export_log_task_id(&self) -> Option<&str> {
        let ids = &self.export_ui.task_ids;
        ids.iter()
            .find(|id| {
                self.file_queue
                    .file_by_id(id)
                    .is_some_and(|file| is_running(file.status))
            })
            .or_else(|| ids.last())
            .map(String::as_str)
    }

    pub(super) fn export_log_view(&self) -> ExportLogView {
        let task_id = self.export_log_task_id();
        ExportLogView {
            shown: self.export_ui.show_log,
            line_count: task_id.map_or(0, |id| self.conversion_events.logs_for(id).len()),
            task_id: task_id.map(str::to_string),
            copied: self.export_ui.log.copied,
            scroll_handle: self.export_ui.log.scroll_handle.clone(),
        }
    }

    pub(super) const fn toggle_export_log(&mut self) {
        self.export_ui.show_log = !self.export_ui.show_log;
        // Opening the panel starts at the newest line.
        self.export_ui.log.follow_tail = true;
        self.export_ui.log.scrolled_line_count = 0;
    }

    /// Keeps the list on its last line as output arrives, unless the user scrolled up.
    pub(super) fn sync_export_log_scroll(&mut self) {
        if !self.export_ui.show_log {
            return;
        }
        let line_count = self
            .export_log_task_id()
            .map_or(0, |id| self.conversion_events.logs_for(id).len());
        let log = &mut self.export_ui.log;
        if line_count < log.scrolled_line_count {
            // A new export replaced the old output.
            log.follow_tail = true;
        }
        // Also asks again when the list isn't at its end, since a scroll requested before the
        // panel's final layout can land short of the last line.
        let behind = line_count != log.scrolled_line_count
            || log.scroll_handle.is_scrolled_to_end() == Some(false);
        if log.follow_tail && line_count > 0 && behind {
            log.scroll_handle
                .scroll_to_item_strict(line_count - 1, ScrollStrategy::Bottom);
        }
        log.scrolled_line_count = line_count;
    }

    fn sync_export_log_follow_after_scroll(&mut self) -> bool {
        let log = &mut self.export_ui.log;
        let follow_tail = log.scroll_handle.is_scrolled_to_end().unwrap_or(true);
        if log.follow_tail == follow_tail {
            return false;
        }
        log.follow_tail = follow_tail;
        true
    }

    fn copy_export_log(&mut self, cx: &Context<Self>) -> bool {
        let Some(id) = self.export_log_task_id() else {
            return false;
        };
        let lines = self.conversion_events.logs_for(id);
        if lines.is_empty() {
            return false;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(lines.join("\n")));
        let log = &mut self.export_ui.log;
        log.copied = true;
        log.copy_epoch = log.copy_epoch.wrapping_add(1);
        let epoch = log.copy_epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPY_FEEDBACK_DURATION).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |root, cx| {
                    if root.export_ui.log.copy_epoch == epoch {
                        root.export_ui.log.copied = false;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
        true
    }
}

const fn is_running(status: FileStatus) -> bool {
    matches!(
        status,
        FileStatus::Queued | FileStatus::Converting | FileStatus::Paused | FileStatus::Cancelling
    )
}

/// The state word at the end of the panel header.
const fn log_state_label(phase: ExportPhase) -> &'static str {
    match phase {
        ExportPhase::Done {
            completed: 0,
            failed: 1..,
        } => "failed",
        ExportPhase::Done { .. } => "finished",
        ExportPhase::Running { .. } | ExportPhase::Idle => "running",
    }
}

/// Lines reported as errors by the encoder or Frame. These take the `error` role.
fn is_error_line(text: &str) -> bool {
    let lower = text.trim_start().to_ascii_lowercase();
    lower.starts_with("[error]")
        || lower.starts_with("error")
        || lower.contains(" error ")
        || lower.contains("failed")
        || lower.contains("invalid")
}

/// The label and switch that show or hide the output panel.
pub(super) fn export_log_toggle(
    shown: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .child(
            div()
                .m3_type(typography::BODY_MEDIUM)
                .text_color(color(scheme.on_surface))
                .child("Show FFmpeg output"),
        )
        .child(
            m3::Switch::new("export-log-switch", "Show FFmpeg output", shown)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                    root.toggle_export_log();
                    cx.notify();
                })),
        )
}

/// The output panel: a header with the export state and a copy button over the scrolling log.
pub(super) fn export_log_panel(
    log: &ExportLogView,
    phase: ExportPhase,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let header = div()
        .h(theme::ui_rem(LOG_HEADER_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .pl(theme::ui_rem(16.0))
        .pr(theme::ui_rem(4.0))
        .border_b_1()
        .border_color(color(scheme.outline_variant))
        .child(
            div()
                .m3_type(typography::TITLE_SMALL)
                .text_color(color(scheme.on_surface_variant))
                .child(format!("FFmpeg output · {}", log_state_label(phase))),
        )
        .child(
            m3::IconButton::new(
                "export-log-copy",
                if log.copied {
                    Symbol::Check
                } else {
                    Symbol::ContentCopy
                },
                if log.copied {
                    "Output copied"
                } else {
                    "Copy output"
                },
            )
            .enabled(log.line_count > 0)
            .build(scheme, window, cx)
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                if root.copy_export_log(cx) {
                    cx.notify();
                }
            })),
        );

    let body = match log.task_id.as_deref() {
        Some(task_id) if log.line_count > 0 => {
            log_lines(task_id, log, scheme, cx).into_any_element()
        }
        _ => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .m3_type(typography::BODY_MEDIUM)
            .text_color(color(scheme.on_surface_variant))
            .child("Waiting for FFmpeg output…")
            .into_any_element(),
    };

    div()
        .w_full()
        .flex_1()
        .min_h(theme::ui_rem(LOG_MIN_HEIGHT))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(theme::ui_rem(shape::CORNER_MEDIUM))
        .border_1()
        .border_color(color(scheme.outline_variant))
        .bg(color(scheme.surface_container_lowest))
        .text_left()
        .child(header)
        .child(body)
}

fn log_lines(
    task_id: &str,
    log: &ExportLogView,
    scheme: &ColorScheme,
    cx: &Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let id = task_id.to_string();
    let row_scheme = *scheme;
    let list = uniform_list(
        "export-log-lines",
        log.line_count,
        cx.processor(move |root, range, _window, _cx| {
            root.conversion_events
                .log_line_window_for(&id, range)
                .into_iter()
                .map(|line| log_line_row(line, &row_scheme))
                .collect()
        }),
    )
    .track_scroll(&log.scroll_handle)
    .on_scroll_wheel(cx.listener(|_root, _: &ScrollWheelEvent, window, cx| {
        cx.defer_in(window, |root, _window, cx| {
            if root.sync_export_log_follow_after_scroll() {
                cx.notify();
            }
        });
    }))
    .size_full()
    .py(theme::ui_rem(4.0))
    .font_family(typography::MONO_TYPEFACE)
    .text_size(theme::ui_rem(LOG_TEXT_SIZE))
    .line_height(theme::ui_rem(LOG_LINE_HEIGHT));

    div()
        .id("export-log")
        .role(gpui::Role::Log)
        .aria_label("FFmpeg output")
        .relative()
        .flex_1()
        .min_h_0()
        .child(list)
        .child(
            m3::uniform_list_scrollbar(
                "export-log-scrollbar",
                &log.scroll_handle,
                usize_to_f32(log.line_count) * LOG_LINE_HEIGHT,
                scheme,
            )
            // Dragging the thumb stops following, or every redraw would pull it back down.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|root, _: &MouseDownEvent, _window, _cx| {
                    root.export_ui.log.follow_tail = false;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|root, _: &MouseUpEvent, _window, cx| {
                    root.sync_export_log_follow_after_scroll();
                    cx.notify();
                }),
            ),
        )
}

fn log_line_row(line: LogLine, scheme: &ColorScheme) -> gpui::Div {
    div()
        .h(theme::ui_rem(LOG_LINE_HEIGHT))
        .w_full()
        .pl(theme::ui_rem(16.0))
        .pr(theme::ui_rem(24.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_color(color(if is_error_line(&line.text) {
            scheme.error
        } else {
            scheme.on_surface_variant
        }))
        .child(line.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_queue::FileItem;
    use frame_core::events::ConversionEvent;

    fn root_with_export_log(line_count: usize) -> FrameRoot {
        let mut root = FrameRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("project-export", "/tmp/source.mp4", 1));
        root.file_queue
            .update_status("project-export", FileStatus::Converting, 10);
        root.export_ui.task_ids = vec!["project-export".to_string()];
        for index in 0..line_count {
            root.conversion_events.apply_conversion_event(
                &mut root.file_queue,
                ConversionEvent::log("project-export", format!("frame= {index}")),
            );
        }
        root
    }

    #[test]
    fn export_log_view_counts_the_running_exports_lines() {
        let root = root_with_export_log(3);

        let view = root.export_log_view();

        assert_eq!(view.task_id.as_deref(), Some("project-export"));
        assert_eq!(view.line_count, 3);
        assert!(!view.shown);
    }

    #[test]
    fn export_log_task_id_prefers_the_running_file() {
        let mut root = root_with_export_log(0);
        root.file_queue
            .add_file(FileItem::from_path("done", "/tmp/done.mp4", 1));
        root.file_queue
            .update_status("done", FileStatus::Completed, 100);
        root.export_ui.task_ids = vec!["project-export".to_string(), "done".to_string()];

        assert_eq!(root.export_log_task_id(), Some("project-export"));
    }

    #[test]
    fn toggling_the_log_resumes_following_new_lines() {
        let mut root = root_with_export_log(2);
        root.export_ui.log.follow_tail = false;

        root.toggle_export_log();

        assert!(root.export_ui.show_log);
        assert!(root.export_ui.log.follow_tail);
    }

    #[test]
    fn sync_export_log_scroll_tracks_the_line_count_while_shown() {
        let mut root = root_with_export_log(4);
        root.export_ui.show_log = true;

        root.sync_export_log_scroll();

        assert_eq!(root.export_ui.log.scrolled_line_count, 4);
    }

    #[test]
    fn log_state_label_names_each_phase() {
        for (phase, expected) in [
            (ExportPhase::Running { progress: 0.3 }, "running"),
            (
                ExportPhase::Done {
                    completed: 1,
                    failed: 0,
                },
                "finished",
            ),
            (
                ExportPhase::Done {
                    completed: 0,
                    failed: 1,
                },
                "failed",
            ),
        ] {
            assert_eq!(log_state_label(phase), expected);
        }
    }

    #[test]
    fn is_error_line_marks_errors_but_not_progress() {
        assert!(is_error_line("[ERROR] ffmpeg failed"));
        assert!(is_error_line("Error opening output file"));
        assert!(!is_error_line(
            "frame=   30 fps= 130 q=28.0 size=     941KiB time=00:00:01.00 speed=4.32x"
        ));
        assert!(!is_error_line("ffmpeg version 7.1"));
    }
}
