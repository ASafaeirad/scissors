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

/// One row of the timeline: a labelled lane holding the clips of a single source.
///
/// The selected source is the only track today. Additional tracks (overlays,
/// music) become extra rows that share the ruler and playhead.
pub(in crate::app) struct PreviewTimelineTrack {
    pub(in crate::app) label: &'static str,
    pub(in crate::app) icon: &'static str,
    pub(in crate::app) clips: Vec<TimelineClip>,
}

pub(in crate::app) fn preview_timeline_tracks(
    state: &PreviewShellState,
) -> Vec<PreviewTimelineTrack> {
    let label = match state.availability.media_kind {
        PreviewMediaKind::Audio => "Audio",
        PreviewMediaKind::Image => "Image",
        PreviewMediaKind::Video | PreviewMediaKind::Unknown => "Video",
    };

    vec![PreviewTimelineTrack {
        label,
        icon: preview_media_icon(state.availability.media_kind),
        clips: state.playback.clips(),
    }]
}

pub(in crate::app) fn preview_timeline(
    state: &PreviewShellState,
    seek_focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let palette = state.palette;
    let tracks = preview_timeline_tracks(state);

    div()
        .mt(theme::ui_rem(PREVIEW_TIMELINE_TOP_MARGIN))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .w_full()
                .min_w_0()
                .h(theme::ui_rem(PREVIEW_TIMELINE_CONTROL_HEIGHT))
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex_none()
                        .text_size(theme::ui_rem(theme::TEXT_ROW_BASE_SIZE))
                        .font_weight(theme::TEXT_WEIGHT_MEDIUM)
                        .text_color(color(palette.text_primary))
                        .child(theme::ui_text("Timeline")),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                        .text_color(color(palette.text_muted))
                        .child(theme::ui_text_owned(preview_timeline_info(
                            &state.playback,
                            &tracks,
                        ))),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(preview_cut_button(state, window, cx))
                        .child(preview_play_button(state, window, cx)),
                ),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .gap_3()
                .child(preview_timeline_track_labels(&tracks, palette))
                .child(preview_timeline_lanes(state, &tracks, seek_focus, cx)),
        )
}

/// Describes the clip under the playhead, or how to edit when there is none.
pub(in crate::app) fn preview_timeline_info(
    playback: &PreviewPlaybackState,
    tracks: &[PreviewTimelineTrack],
) -> String {
    let clips = tracks
        .first()
        .map_or(&[][..], |track| track.clips.as_slice());
    if clips.is_empty() {
        return "Select a video or audio source to edit its timeline.".to_string();
    }

    let time = playback.current_time();
    clips
        .iter()
        .position(|clip| clip.contains(time))
        .map_or_else(
            || "Drag clip edges to trim. Press X to remove a segment at the playhead.".to_string(),
            |index| {
                let clip = clips[index];
                format!(
                    "Clip {} of {} · {} – {}",
                    index + 1,
                    clips.len(),
                    format_timeline_time(clip.start, playback.duration()),
                    format_timeline_time(clip.end, playback.duration()),
                )
            },
        )
}

fn preview_timeline_track_labels(
    tracks: &[PreviewTimelineTrack],
    palette: &'static theme::ThemePalette,
) -> gpui::Div {
    div()
        .flex_none()
        .w(theme::ui_rem(PREVIEW_TIMELINE_LABEL_WIDTH))
        .flex()
        .flex_col()
        .gap(theme::ui_rem(PREVIEW_TIMELINE_ROW_GAP))
        .child(div().h(theme::ui_rem(PREVIEW_TIMELINE_RULER_HEIGHT)))
        .children(tracks.iter().map(|track| {
            div()
                .h(theme::ui_rem(PREVIEW_TIMELINE_LANE_HEIGHT))
                .pl_2()
                .flex()
                .items_center()
                .gap_2()
                .child(icon_svg(track.icon, 16.0, color(palette.text_muted)))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                        .font_weight(theme::TEXT_WEIGHT_MEDIUM)
                        .text_color(color(palette.text_primary))
                        .child(theme::ui_text(track.label)),
                )
        }))
}

pub(in crate::app) fn preview_trim_enabled(state: &PreviewShellState) -> bool {
    !state.availability.trim_disabled && state.duration_seconds > 0.0
}

/// The ruler and track lanes. The whole area scrubs, and every drag on it is
/// measured against its bounds, so lanes must span its full width.
#[expect(
    clippy::too_many_lines,
    reason = "Timeline lanes compose scrub, a11y metadata, and drag/key handling together."
)]
fn preview_timeline_lanes(
    state: &PreviewShellState,
    tracks: &[PreviewTimelineTrack],
    focus: &FocusHandle,
    cx: &Context<FrameRoot>,
) -> impl IntoElement {
    let palette = state.palette;
    let enabled = preview_trim_enabled(state);
    let duration = state.playback.duration();
    let current_time = state.playback.current_time();
    let playhead_fraction = timeline_fraction_from_percent(
        state
            .playback
            .to_timeline_percent(state.playback.current_time()),
    );

    let lanes = div()
        .id("preview-timeline-track")
        .relative()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(PREVIEW_TIMELINE_ROW_GAP))
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
        lanes,
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
    .child(preview_timeline_ruler(duration, palette))
    .children(
        tracks
            .iter()
            .enumerate()
            .map(|(index, track)| preview_timeline_lane(state, index, track, enabled, cx)),
    )
    .child(PreviewTimelineTrackBoundsProbe { owner: cx.entity() })
    .child(
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(playhead_fraction))
            .ml(px(-1.0))
            .w(px(2.0))
            .bg(color(palette.text_primary))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(theme::ui_rem(-(PREVIEW_PLAYHEAD_KNOB_SIZE / 2.0)))
                    .ml(px(1.0))
                    .size(theme::ui_rem(PREVIEW_PLAYHEAD_KNOB_SIZE))
                    .rounded_full()
                    .bg(color(palette.text_primary)),
            ),
    )
}

fn preview_timeline_ruler(duration: f64, palette: &'static theme::ThemePalette) -> gpui::Div {
    div()
        .relative()
        .w_full()
        .h(theme::ui_rem(PREVIEW_TIMELINE_RULER_HEIGHT))
        .overflow_hidden()
        .children(timeline_ruler_ticks(duration).into_iter().map(|time| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(unit_f64_to_f32(time / duration)))
                .pl_1()
                .border_l_1()
                .border_color(color(palette.border_subtle))
                .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE - 1.0))
                .text_color(color(palette.text_muted))
                .child(format_timeline_ruler_time(time, duration))
        }))
}

fn preview_timeline_lane(
    state: &PreviewShellState,
    track_index: usize,
    track: &PreviewTimelineTrack,
    enabled: bool,
    cx: &Context<FrameRoot>,
) -> gpui::Div {
    let palette = state.palette;
    let current_time = state.playback.current_time();
    let active_clip = track
        .clips
        .iter()
        .copied()
        .find(|clip| clip.contains(current_time));
    let clip_name = state.selected_file_name.clone().unwrap_or_default();

    // Removed segments and their edges belong to the source on the first track.
    let edits_source = track_index == 0;
    let edges: Vec<(TimelineDragTarget, f64)> = if edits_source {
        [
            (TimelineDragTarget::Start, state.playback.start_value()),
            (TimelineDragTarget::End, state.playback.end_value()),
        ]
        .into_iter()
        .chain(
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
                }),
        )
        .collect()
    } else {
        Vec::new()
    };

    div()
        .relative()
        .w_full()
        .h(theme::ui_rem(PREVIEW_TIMELINE_LANE_HEIGHT))
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .bg(color(palette.fill_subtle))
        .children(track.clips.iter().map(|clip| {
            preview_timeline_clip(
                state,
                *clip,
                Some(*clip) == active_clip,
                &clip_name,
                track.icon,
            )
        }))
        .when(edits_source, |lane| {
            lane.children(
                state
                    .playback
                    .cuts()
                    .iter()
                    .enumerate()
                    .map(|(index, cut)| preview_timeline_cut_region(state, index, cut, enabled)),
            )
        })
        .children(edges.into_iter().map(|(target, value)| {
            let active =
                active_clip.is_some_and(|clip| clip.leading == target || clip.trailing == target);
            preview_timeline_handle(target, value, enabled, active, &state.playback, palette, cx)
        }))
}

fn preview_timeline_clip(
    state: &PreviewShellState,
    clip: TimelineClip,
    active: bool,
    name: &str,
    icon: &'static str,
) -> gpui::Div {
    let palette = state.palette;
    let start_fraction =
        timeline_fraction_from_percent(state.playback.to_timeline_percent(clip.start));
    let end_fraction = timeline_fraction_from_percent(state.playback.to_timeline_percent(clip.end));

    div()
        .absolute()
        .top(theme::ui_rem(PREVIEW_TIMELINE_CLIP_INSET))
        .bottom(theme::ui_rem(PREVIEW_TIMELINE_CLIP_INSET))
        .left(relative(start_fraction))
        .right(relative((1.0 - end_fraction).max(0.0)))
        .overflow_hidden()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .bg(color(palette.accent.with_alpha(0.22)))
        .border_color(color(if active {
            palette.accent
        } else {
            palette.accent.with_alpha(0.35)
        }))
        .when(active, gpui::Styled::border_2)
        .when(!active, gpui::Styled::border_1)
        .px(theme::ui_rem(PREVIEW_TIMELINE_CLIP_HANDLE_WIDTH + 4.0))
        .flex()
        .items_center()
        .gap(theme::ui_rem(6.0))
        .text_color(color(palette.text_primary))
        .child(
            div()
                .flex_none()
                .child(icon_svg(icon, 14.0, color(palette.text_primary))),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                .font_weight(theme::TEXT_WEIGHT_MEDIUM)
                .child(name.to_string()),
        )
        .child(
            div()
                .flex_none()
                .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE - 1.0))
                .text_color(color(palette.text_muted))
                .child(format_clip_duration(clip.duration())),
        )
}

fn preview_timeline_cut_region(
    state: &PreviewShellState,
    index: usize,
    cut: &CutRange,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    let palette = state.palette;
    let start_fraction =
        timeline_fraction_from_percent(state.playback.to_timeline_percent(cut.start));
    let end_fraction = timeline_fraction_from_percent(state.playback.to_timeline_percent(cut.end));

    // Mouse down is left to the lanes so the playhead lands on the grab point,
    // which keeps the region anchored under the pointer while it moves.
    let region = div()
        .id(("preview-timeline-cut-region", index))
        .absolute()
        .left(relative(start_fraction))
        .right(relative((1.0 - end_fraction).max(0.0)))
        .top(theme::ui_rem(PREVIEW_TIMELINE_CLIP_INSET))
        .bottom(theme::ui_rem(PREVIEW_TIMELINE_CLIP_INSET))
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .border_1()
        .border_dashed()
        .border_color(color(palette.danger.with_alpha(0.6)))
        .bg(color(palette.danger.with_alpha(0.12)));

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

/// Whether a drag target is the leading (left) edge of the clip it bounds.
///
/// Leading grips sit just right of their edge and trailing grips just left,
/// so both stay inside the clip they resize.
const fn timeline_edge_is_leading(target: TimelineDragTarget) -> bool {
    matches!(
        target,
        TimelineDragTarget::Start | TimelineDragTarget::CutEnd(_)
    )
}

pub(in crate::app) fn preview_timeline_handle(
    target: TimelineDragTarget,
    value: f64,
    enabled: bool,
    active: bool,
    playback: &PreviewPlaybackState,
    palette: &'static theme::ThemePalette,
    cx: &Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let duration = playback.duration();
    let fraction = timeline_fraction_from_percent(playback.to_timeline_percent(value));
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
    let width = PREVIEW_TIMELINE_CLIP_HANDLE_WIDTH;
    let offset = if timeline_edge_is_leading(target) {
        0.0
    } else {
        -width
    };

    let handle = div()
        .id(handle_id)
        .absolute()
        .top(theme::ui_rem(PREVIEW_TIMELINE_CLIP_INSET))
        .bottom(theme::ui_rem(PREVIEW_TIMELINE_CLIP_INSET))
        .left(relative(fraction))
        .ml(theme::ui_rem(offset))
        .w(theme::ui_rem(width))
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .when(active, |this| this.bg(color(palette.accent)))
        .flex()
        .items_center()
        .justify_center()
        .child(preview_timeline_handle_grip(active, palette))
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

fn preview_timeline_handle_grip(active: bool, palette: &'static theme::ThemePalette) -> gpui::Div {
    div()
        .w(px(2.0))
        .h(theme::ui_rem(14.0))
        .rounded(px(1.0))
        .bg(if active {
            preview_selection_line_color()
        } else {
            color(palette.text_primary.with_alpha(0.6)).into()
        })
}

/// Ruler tick times: the smallest readable step that keeps at most eight ticks.
///
/// Ticks in the last few percent are dropped because their label would be clipped.
pub(in crate::app) fn timeline_ruler_ticks(duration: f64) -> Vec<f64> {
    const STEPS: [f64; 14] = [
        0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0,
    ];
    const MAX_TICKS: f64 = 8.0;
    const LABEL_END_MARGIN: f64 = 0.05;

    if !duration.is_finite() || duration <= 0.0 {
        return Vec::new();
    }

    let step = STEPS
        .into_iter()
        .find(|step| duration / step <= MAX_TICKS)
        .unwrap_or_else(|| (duration / MAX_TICKS / 3600.0).ceil() * 3600.0);
    std::iter::successors(Some(0.0), |time| {
        Some(time + step).filter(|next| *next < duration * (1.0 - LABEL_END_MARGIN))
    })
    .collect()
}

/// Ruler labels drop the hour for sources under an hour and add tenths for sub-second steps.
pub(in crate::app) fn format_timeline_ruler_time(seconds: f64, duration: f64) -> String {
    let whole = seconds.floor();
    let base = format_timeline_clock(whole, duration);
    let tenths = ((seconds - whole) * 10.0).round();
    if tenths > 0.0 {
        format!("{base}.{tenths:.0}")
    } else {
        base
    }
}

/// Precise clip boundary time, e.g. `00:05.3` or `1:02:05.3`.
pub(in crate::app) fn format_timeline_time(seconds: f64, duration: f64) -> String {
    let tenths_total = (seconds.max(0.0) * 10.0).round();
    let whole = (tenths_total / 10.0).floor();
    let tenths = tenths_total - whole * 10.0;
    format!("{}.{tenths:.0}", format_timeline_clock(whole, duration))
}

fn format_timeline_clock(whole_seconds: f64, duration: f64) -> String {
    let hours = (whole_seconds / 3600.0).floor();
    let minutes = ((whole_seconds % 3600.0) / 60.0).floor();
    let seconds = whole_seconds % 60.0;
    if duration >= 3600.0 {
        format!("{hours:.0}:{minutes:02.0}:{seconds:02.0}")
    } else {
        format!("{:02.0}:{seconds:02.0}", hours * 60.0 + minutes)
    }
}

/// Compact clip length shown inside a clip, e.g. `4.2 s` or `12:05`.
pub(in crate::app) fn format_clip_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{seconds:.1} s");
    }
    let whole = seconds.round();
    let hours = (whole / 3600.0).floor();
    let minutes = ((whole % 3600.0) / 60.0).floor();
    let rest = whole % 60.0;
    if hours > 0.0 {
        format!("{hours:.0}:{minutes:02.0}:{rest:02.0}")
    } else {
        format!("{minutes:.0}:{rest:02.0}")
    }
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
