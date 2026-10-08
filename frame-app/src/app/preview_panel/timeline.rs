use super::*;
use crate::numeric::unit_f64_to_f32;
use crate::preview::CutRange;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreviewTimelineDrag {
    target: TimelineDragTarget,
}

pub(super) struct PreviewTimelineDragPreview;

impl Render for PreviewTimelineDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(theme::ui_rem(0.0)).h(theme::ui_rem(0.0))
    }
}

struct PreviewTimelineTrackBoundsProbe {
    owner: Entity<FrameRoot>,
}

impl IntoElement for PreviewTimelineTrackBoundsProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PreviewTimelineTrackBoundsProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            position: Position::Absolute,
            size: size(relative(1.0).into(), relative(1.0).into()),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Style::default()
        };

        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.owner.update(cx, |root, _cx| {
            root.set_preview_timeline_track_bounds(bounds);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

pub(in crate::app) fn preview_timeline(
    state: &PreviewShellState,
    inputs: PreviewTimecodeInputFocuses<'_>,
    seek_focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let palette = state.palette;
    let labels = preview_timeline_labels(state);
    let trim_enabled = preview_trim_enabled(state);

    div()
        .mt(theme::ui_rem(PREVIEW_TIMELINE_TOP_MARGIN))
        .px_2()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(preview_timecode_field(
                            PreviewTimecodeFieldSpec {
                                label: "Start time",
                                value: inputs.start_value.to_string(),
                                enabled: trim_enabled,
                                width: 128.0,
                                kind: Some(FrameTextInputKind::PreviewStartTime),
                                focus: inputs.start,
                            },
                            palette,
                            window,
                            cx,
                        ))
                        .child(preview_timecode_field(
                            PreviewTimecodeFieldSpec {
                                label: "End time",
                                value: inputs.end_value.to_string(),
                                enabled: trim_enabled,
                                width: 128.0,
                                kind: Some(FrameTextInputKind::PreviewEndTime),
                                focus: inputs.end,
                            },
                            palette,
                            window,
                            cx,
                        ))
                        .child(preview_timecode_field(
                            PreviewTimecodeFieldSpec {
                                label: "Duration",
                                value: labels.duration,
                                enabled: false,
                                width: 104.0,
                                kind: None,
                                focus: None,
                            },
                            palette,
                            window,
                            cx,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(theme::ui_rem(6.0))
                        .child(preview_timeline_label(" ", palette))
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(preview_cut_button(state, window, cx))
                                .child(preview_play_button(state, window, cx)),
                        ),
                ),
        )
        .child(
            div()
                .min_w_0()
                .w_full()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(6.0))
                .child(preview_timeline_label("Trim", palette))
                .child(preview_timeline_track(state, seek_focus, cx)),
        )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct PreviewTimelineLabels {
    pub(in crate::app) start: String,
    pub(in crate::app) end: String,
    pub(in crate::app) duration: String,
}

pub(in crate::app) fn preview_timeline_labels(state: &PreviewShellState) -> PreviewTimelineLabels {
    if state.availability.media_kind == PreviewMediaKind::Image
        || state.availability.media_kind == PreviewMediaKind::Unknown
        || state.duration_seconds <= 0.0
    {
        return PreviewTimelineLabels {
            start: "--:--:--.---".to_string(),
            end: "--:--:--.---".to_string(),
            duration: "--:--:--.---".to_string(),
        };
    }

    PreviewTimelineLabels {
        start: format_time(state.playback.start_value()),
        end: format_time(state.playback.end_value()),
        duration: format_time(state.playback.output_duration()),
    }
}

pub(in crate::app) fn preview_trim_enabled(state: &PreviewShellState) -> bool {
    !state.availability.trim_disabled && state.duration_seconds > 0.0
}

pub(in crate::app) struct PreviewTimecodeFieldSpec<'a> {
    label: &'static str,
    value: String,
    enabled: bool,
    width: f32,
    kind: Option<FrameTextInputKind>,
    focus: Option<&'a FocusHandle>,
}

pub(in crate::app) fn preview_timecode_field(
    spec: PreviewTimecodeFieldSpec<'_>,
    palette: &'static theme::ThemePalette,
    window: &Window,
    cx: &Context<FrameRoot>,
) -> gpui::Div {
    let PreviewTimecodeFieldSpec {
        label,
        value,
        enabled,
        width,
        kind,
        focus,
    } = spec;
    let field = if let (Some(kind), Some(focus)) = (kind, focus) {
        frame_text_input(
            FrameTextInputSpec {
                id: match kind {
                    FrameTextInputKind::PreviewStartTime => "preview-start-time",
                    FrameTextInputKind::PreviewEndTime => "preview-end-time",
                    _ => "preview-timecode",
                },
                value: &value,
                placeholder: "--:--:--.---",
                disabled: !enabled,
                focus: Some(focus),
                kind,
            },
            palette,
            window,
            cx,
        )
        .font_features(assets::frame_tabular_number_font_features())
        .into_any_element()
    } else {
        div()
            .w_full()
            .h(theme::ui_rem(PREVIEW_TIMELINE_CONTROL_HEIGHT))
            .flex()
            .items_center()
            .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
            .text_color(color(palette.text_primary))
            .font_features(assets::frame_tabular_number_font_features())
            .child(value)
            .into_any_element()
    };

    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(6.0))
        .child(preview_timeline_label(label, palette))
        .child(
            div()
                .w(theme::ui_rem(width))
                .h(theme::ui_rem(PREVIEW_TIMELINE_CONTROL_HEIGHT))
                .child(field),
        )
}

pub(in crate::app) fn preview_timeline_label(
    label: &'static str,
    palette: &'static theme::ThemePalette,
) -> gpui::Div {
    div()
        .h(theme::ui_rem(12.0))
        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
        .font_weight(theme::TEXT_WEIGHT_MEDIUM)
        .text_color(color(palette.text_muted))
        .child(theme::ui_text(label))
}

#[expect(
    clippy::too_many_lines,
    reason = "Timeline track composes scrub, trim handles, a11y metadata, and drag/key handling together."
)]
pub(in crate::app) fn preview_timeline_track(
    state: &PreviewShellState,
    focus: &FocusHandle,
    cx: &Context<FrameRoot>,
) -> impl IntoElement {
    let palette = state.palette;
    let enabled = preview_trim_enabled(state);
    let duration = state.playback.duration();
    let current_time = state.playback.current_time();
    let track_top = centered_offset(PREVIEW_TIMELINE_CONTROL_HEIGHT, PREVIEW_TRACK_HEIGHT);
    let playhead_top = centered_offset(PREVIEW_TIMELINE_CONTROL_HEIGHT, PREVIEW_PLAYHEAD_HEIGHT);
    let start_fraction = timeline_fraction_from_percent(
        state
            .playback
            .to_timeline_percent(state.playback.start_value()),
    );
    let end_fraction = timeline_fraction_from_percent(
        state
            .playback
            .to_timeline_percent(state.playback.end_value()),
    );
    let playhead_fraction = timeline_fraction_from_percent(
        state
            .playback
            .to_timeline_percent(state.playback.current_time()),
    );

    let track = div()
        .id("preview-timeline-track")
        .relative()
        .h(theme::ui_rem(PREVIEW_TIMELINE_CONTROL_HEIGHT))
        .w_full()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .opacity(if enabled { 1.0 } else { 0.5 })
        .when(enabled, gpui::Styled::cursor_pointer)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|root, event: &MouseDownEvent, _window, cx| {
                if root.commit_preview_timeline_seek_at_position_with_context(event.position, cx) {
                    cx.notify();
                }
            }),
        )
        .when(enabled, |this| {
            this.on_drag(
                PreviewTimelineDrag {
                    target: TimelineDragTarget::Scrub,
                },
                |_drag, _position, _window, cx| cx.new(|_| PreviewTimelineDragPreview),
            )
        })
        .on_drag_move(cx.listener(
            |root, event: &DragMoveEvent<PreviewTimelineDrag>, _window, cx| {
                let drag = *event.drag(cx);
                let percent =
                    timeline_slider_percent_from_bounds(event.event.position, event.bounds);
                if root.apply_preview_timeline_drag_with_context(drag.target, percent, cx) {
                    cx.notify();
                }
            },
        ))
        .capture_any_mouse_up(cx.listener(|root, _event: &MouseUpEvent, _window, cx| {
            if root.end_preview_timeline_drag_with_context(cx) {
                cx.notify();
            }
        }));

    let owner = cx.entity();
    let decrement_owner = owner.clone();

    super::super::accessibility::apply_accessible_slider_with_focus(
        track,
        "Preview position",
        enabled,
        current_time,
        0.0,
        duration,
        format_time(current_time),
        focus,
        palette,
    )
    .on_a11y_action(gpui::AccessibleAction::Increment, move |_, _window, cx| {
        owner.update(cx, |root, cx| {
            if root.adjust_preview_timeline_from_keyboard_with_context(
                TimelineDragTarget::Scrub,
                "right",
                cx,
            ) {
                cx.notify();
            }
        });
    })
    .on_a11y_action(gpui::AccessibleAction::Decrement, move |_, _window, cx| {
        decrement_owner.update(cx, |root, cx| {
            if root.adjust_preview_timeline_from_keyboard_with_context(
                TimelineDragTarget::Scrub,
                "left",
                cx,
            ) {
                cx.notify();
            }
        });
    })
    .on_key_down(
        cx.listener(|root, event: &gpui::KeyDownEvent, _window, cx| {
            if !timeline_keyboard_key_is_handled(event.keystroke.key.as_str()) {
                return;
            }
            if root.adjust_preview_timeline_from_keyboard_with_context(
                TimelineDragTarget::Scrub,
                event.keystroke.key.as_str(),
                cx,
            ) {
                cx.notify();
            }
            cx.stop_propagation();
        }),
    )
    .child(
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(theme::ui_rem(track_top))
            .h(theme::ui_rem(PREVIEW_TRACK_HEIGHT))
            .rounded(theme::ui_rem(1.5))
            .bg(color(palette.fill_subtle)),
    )
    .child(
        div()
            .absolute()
            .left(relative(start_fraction))
            .right(relative((1.0 - end_fraction).max(0.0)))
            .top(theme::ui_rem(track_top))
            .h(theme::ui_rem(PREVIEW_TRACK_HEIGHT))
            .rounded(theme::ui_rem(1.0))
            .bg(color(palette.text_primary)),
    )
    .children(
        state
            .playback
            .cuts()
            .iter()
            .enumerate()
            .map(|(index, cut)| {
                preview_timeline_cut_region(state, index, cut, playhead_top, enabled)
            }),
    )
    .child(PreviewTimelineTrackBoundsProbe { owner: cx.entity() })
    .child(
        div()
            .absolute()
            .left(relative(playhead_fraction))
            .ml(px(-0.5))
            .top(theme::ui_rem(playhead_top))
            .h(theme::ui_rem(PREVIEW_PLAYHEAD_HEIGHT))
            .w(px(1.0))
            .bg(color(palette.text_primary)),
    )
    .child(preview_timeline_handle(
        TimelineDragTarget::Start,
        start_fraction,
        enabled,
        state.playback.start_value(),
        duration,
        palette,
        cx,
    ))
    .child(preview_timeline_handle(
        TimelineDragTarget::End,
        end_fraction,
        enabled,
        state.playback.end_value(),
        duration,
        palette,
        cx,
    ))
    .children(
        state
            .playback
            .cuts()
            .iter()
            .enumerate()
            .flat_map(|(index, cut)| {
                [
                    (TimelineDragTarget::CutStart(index), cut.start),
                    (TimelineDragTarget::CutEnd(index), cut.end),
                ]
            })
            .map(|(target, value)| {
                preview_timeline_handle(
                    target,
                    timeline_fraction_from_percent(state.playback.to_timeline_percent(value)),
                    enabled,
                    value,
                    duration,
                    palette,
                    cx,
                )
            }),
    )
}

fn preview_timeline_cut_region(
    state: &PreviewShellState,
    index: usize,
    cut: &CutRange,
    top: f32,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    let palette = state.palette;
    let start_fraction =
        timeline_fraction_from_percent(state.playback.to_timeline_percent(cut.start));
    let end_fraction = timeline_fraction_from_percent(state.playback.to_timeline_percent(cut.end));

    // Mouse down is left to the track so the playhead lands on the grab point,
    // which keeps the region anchored under the pointer while it moves.
    let region = div()
        .id(("preview-timeline-cut-region", index))
        .absolute()
        .left(relative(start_fraction))
        .right(relative((1.0 - end_fraction).max(0.0)))
        .top(theme::ui_rem(top))
        .h(theme::ui_rem(PREVIEW_PLAYHEAD_HEIGHT))
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .border_1()
        .border_color(color(palette.danger))
        .bg(color(palette.danger.with_alpha(0.3)));

    if !enabled {
        return region;
    }

    let dragging = state.playback.dragging() == Some(TimelineDragTarget::CutMove(index));
    region
        .when(dragging, gpui::Styled::cursor_grabbing)
        .when(!dragging, gpui::Styled::cursor_grab)
        .on_drag(
            PreviewTimelineDrag {
                target: TimelineDragTarget::CutMove(index),
            },
            |_drag, _position, _window, cx| cx.new(|_| PreviewTimelineDragPreview),
        )
}

pub(in crate::app) fn preview_cut_enabled(state: &PreviewShellState) -> bool {
    state.availability.cut_available && state.duration_seconds > 0.0
}

pub(in crate::app) fn preview_cut_button(
    state: &PreviewShellState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> impl IntoElement {
    let in_cut = state
        .playback
        .cut_index_at(state.playback.current_time())
        .is_some();

    preview_tool_button(
        "preview-cut-toggle",
        assets::ICON_SCISSORS,
        if in_cut {
            "Restore removed segment"
        } else {
            "Remove segment from playhead"
        },
        in_cut,
        // Existing cuts stay removable even when new ones are unavailable (e.g. stream copy).
        if in_cut {
            preview_trim_enabled(state)
        } else {
            preview_cut_enabled(state)
        },
        state.tooltip_visible_id.as_deref(),
        super::super::components::TooltipPlacement::Above,
        state.palette,
        window,
        cx,
    )
    .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
        if root.toggle_preview_cut_at_playhead() {
            cx.notify();
        }
    }))
}

pub(in crate::app) fn preview_timeline_handle(
    target: TimelineDragTarget,
    fraction: f32,
    enabled: bool,
    value: f64,
    duration: f64,
    palette: &'static theme::ThemePalette,
    cx: &Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let (handle_id, label): (ElementId, SharedString) = match target {
        TimelineDragTarget::Start => ("preview-timeline-start-handle".into(), "Trim start".into()),
        TimelineDragTarget::End => ("preview-timeline-end-handle".into(), "Trim end".into()),
        TimelineDragTarget::Scrub => (
            "preview-timeline-scrub-handle".into(),
            "Preview position".into(),
        ),
        TimelineDragTarget::CutStart(index) => (
            ("preview-timeline-cut-start-handle", index).into(),
            format!("Removed segment {} start", index + 1).into(),
        ),
        TimelineDragTarget::CutEnd(index) => (
            ("preview-timeline-cut-end-handle", index).into(),
            format!("Removed segment {} end", index + 1).into(),
        ),
        TimelineDragTarget::CutMove(index) => (
            ("preview-timeline-cut-region", index).into(),
            format!("Removed segment {}", index + 1).into(),
        ),
    };
    // Cut edges sit inside the trim range, so narrower hit areas keep short cuts grabbable.
    let handle_width = if target.is_cut() {
        PREVIEW_TIMELINE_HANDLE_WIDTH / 2.0
    } else {
        PREVIEW_TIMELINE_HANDLE_WIDTH
    };

    let handle = div()
        .id(handle_id)
        .absolute()
        .top_0()
        .left(relative(fraction))
        .ml(theme::ui_rem(-(handle_width / 2.0)))
        .h(theme::ui_rem(PREVIEW_TIMELINE_CONTROL_HEIGHT))
        .w(theme::ui_rem(handle_width))
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
            cx.stop_propagation();
        })
        .when(enabled, gpui::Styled::cursor_ew_resize);

    let handle = if enabled {
        handle.on_drag(
            PreviewTimelineDrag { target },
            |_drag, _position, _window, cx| cx.new(|_| PreviewTimelineDragPreview),
        )
    } else {
        handle
    };

    let owner = cx.entity();
    let decrement_owner = owner.clone();

    apply_accessible_slider(
        handle,
        label,
        enabled,
        value,
        0.0,
        duration,
        format_time(value),
        palette,
    )
    .on_a11y_action(gpui::AccessibleAction::Increment, move |_, _window, cx| {
        owner.update(cx, |root, cx| {
            if root.adjust_preview_timeline_from_keyboard_with_context(target, "right", cx) {
                cx.notify();
            }
        });
    })
    .on_a11y_action(gpui::AccessibleAction::Decrement, move |_, _window, cx| {
        decrement_owner.update(cx, |root, cx| {
            if root.adjust_preview_timeline_from_keyboard_with_context(target, "left", cx) {
                cx.notify();
            }
        });
    })
    .on_key_down(
        cx.listener(move |root, event: &gpui::KeyDownEvent, _window, cx| {
            if !timeline_keyboard_key_is_handled(event.keystroke.key.as_str()) {
                return;
            }
            if root.adjust_preview_timeline_from_keyboard_with_context(
                target,
                event.keystroke.key.as_str(),
                cx,
            ) {
                cx.notify();
            }
            cx.stop_propagation();
        }),
    )
}

pub(in crate::app) fn preview_play_button(
    state: &PreviewShellState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> impl IntoElement {
    let enabled = preview_trim_enabled(state);
    let icon = if state.playback.is_playing() {
        assets::ICON_PAUSE
    } else {
        assets::ICON_PLAY
    };

    preview_tool_button(
        "preview-playback-toggle",
        icon,
        if state.playback.is_playing() {
            "Pause preview"
        } else {
            "Play preview"
        },
        false,
        enabled,
        state.tooltip_visible_id.as_deref(),
        super::super::components::TooltipPlacement::Above,
        state.palette,
        window,
        cx,
    )
    .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
        if root.toggle_preview_playback_with_context(cx) {
            cx.notify();
        }
    }))
}

pub(in crate::app) fn centered_offset(container: f32, child: f32) -> f32 {
    ((container - child) / 2.0).max(0.0)
}

pub(in crate::app) fn timeline_fraction_from_percent(percent: f64) -> f32 {
    unit_f64_to_f32(percent / 100.0)
}

pub(in crate::app) fn timeline_slider_percent_from_bounds(
    position: gpui::Point<Pixels>,
    bounds: Bounds<Pixels>,
) -> f64 {
    let width = bounds.size.width.as_f32();
    if width <= 0.0 {
        return 0.0;
    }

    let x = (position.x - bounds.origin.x).as_f32();
    f64::from((x / width).clamp(0.0, 1.0))
}

pub(in crate::app) fn timeline_keyboard_time_for_key(
    current_time: f64,
    duration: f64,
    key: &str,
) -> Option<f64> {
    if !duration.is_finite() || duration <= 0.0 {
        return None;
    }

    let small_step = (duration / 100.0).clamp(0.1, 1.0);
    let large_step = (duration / 10.0).clamp(1.0, 10.0);
    let next = match key {
        "left" | "down" => current_time - small_step,
        "right" | "up" => current_time + small_step,
        "pageup" => current_time - large_step,
        "pagedown" => current_time + large_step,
        "home" => 0.0,
        "end" => duration,
        _ => return None,
    };

    Some(next.clamp(0.0, duration))
}

pub(in crate::app) fn timeline_keyboard_key_is_handled(key: &str) -> bool {
    matches!(
        key,
        "left" | "down" | "right" | "up" | "pageup" | "pagedown" | "home" | "end"
    )
}
