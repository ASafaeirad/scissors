use super::*;
use crate::app::m3::{self, M3Text};
use crate::material::{self, Symbol, shape, typography};
use crate::numeric::unit_f64_to_f32;
use crate::preview::CutRange;

#[derive(Clone, Debug)]
struct PreviewTimelineAudioDrag {
    index: usize,
    grab_x: std::rc::Rc<std::cell::Cell<Pixels>>,
}

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

pub(in crate::app) struct PreviewTimelineTrackBoundsProbe {
    pub(in crate::app) owner: Entity<FrameRoot>,
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
/// Audio clips share the source ruler and playhead.
pub(in crate::app) struct PreviewTimelineTrack {
    pub(in crate::app) label: &'static str,
    pub(in crate::app) icon: Symbol,
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

    let mut tracks = vec![PreviewTimelineTrack {
        label,
        icon: match state.availability.media_kind {
            PreviewMediaKind::Audio => Symbol::MusicNote,
            PreviewMediaKind::Image => Symbol::Image,
            _ => Symbol::VideoFile,
        },
        clips: state.playback.clips(),
    }];
    tracks.extend(state.audio_tracks.iter().map(|track| {
        let start = std::time::Duration::from_millis(track.offset_millis).as_secs_f64();
        let end = start + std::time::Duration::from_millis(track.duration_millis).as_secs_f64();
        PreviewTimelineTrack {
            label: "Audio",
            icon: Symbol::MusicNote,
            clips: vec![TimelineClip {
                start,
                end: end.min(state.playback.duration()),
                leading: TimelineDragTarget::Start,
                trailing: TimelineDragTarget::End,
            }],
        }
    }));
    tracks
}

pub(in crate::app) fn preview_timeline(
    state: &PreviewShellState,
    seek_focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let palette = state.palette;
    let scheme = &timeline_scheme(palette, cx);
    let tracks = preview_timeline_tracks(state);

    m3::m3_surface(scheme.surface_container_low, shape::CORNER_EXTRA_LARGE)
        .flex_none()
        .p(theme::ui_rem(16.0))
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
                        .m3_type(typography::TITLE_SMALL)
                        .text_color(color(scheme.on_surface))
                        .child(theme::ui_text("Timeline")),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .child(format!(
                            "{} · {}",
                            state
                                .selected_file_name
                                .as_deref()
                                .unwrap_or("No media selected"),
                            preview_timeline_info(&state.playback, &tracks)
                        )),
                )
                .child(
                    m3::Button::new("timeline-add-audio", "Add audio", m3::ButtonVariant::Tonal)
                        .size(m3::ButtonSize::ExtraSmall)
                        .icon(Symbol::Add)
                        .enabled(!state.controls_disabled && state.duration_seconds > 0.0)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, window, cx| {
                            root.prompt_timeline_audio(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .gap_3()
                .child(preview_timeline_track_labels(&tracks, state, window, cx))
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
    state: &PreviewShellState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = &timeline_scheme(state.palette, cx);
    div()
        .flex_none()
        .w(theme::ui_rem(if state.audio_tracks.is_empty() {
            PREVIEW_TIMELINE_LABEL_WIDTH
        } else {
            176.0
        }))
        .flex()
        .flex_col()
        .gap(theme::ui_rem(PREVIEW_TIMELINE_ROW_GAP))
        .child(div().h(theme::ui_rem(PREVIEW_TIMELINE_RULER_HEIGHT)))
        .children(tracks.iter().enumerate().map(|(index, track)| {
            div()
                .h(theme::ui_rem(PREVIEW_TIMELINE_LANE_HEIGHT))
                .pl_2()
                .flex()
                .items_center()
                .gap_2()
                .child(m3::m3_icon(
                    track.icon,
                    false,
                    20.0,
                    color(scheme.on_surface_variant),
                ))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .m3_type(typography::LABEL_LARGE)
                        .text_color(color(scheme.on_surface))
                        .child(theme::ui_text(track.label)),
                )
                .when(index > 0, |row| {
                    let audio_index = index - 1;
                    let muted = state.audio_tracks[audio_index].muted;
                    row.child(
                        m3::IconButton::new(
                            format!("audio-mute-{index}"),
                            if muted {
                                Symbol::VolumeOff
                            } else {
                                Symbol::VolumeUp
                            },
                            if muted {
                                "Unmute audio track"
                            } else {
                                "Mute audio track"
                            },
                        )
                        .size(m3::IconButtonSize::ExtraSmall)
                        .enabled(!state.controls_disabled)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(move |root, _, _, cx| {
                            root.update_selected_config(|config| {
                                let Some(track) = config.external_audio_tracks.get_mut(audio_index)
                                else {
                                    return false;
                                };
                                track.muted = !track.muted;
                                true
                            });
                            cx.notify();
                        })),
                    )
                    .child(
                        m3::IconButton::new(
                            format!("audio-remove-{index}"),
                            Symbol::Delete,
                            "Remove audio track",
                        )
                        .size(m3::IconButtonSize::ExtraSmall)
                        .enabled(!state.controls_disabled)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(move |root, _, _, cx| {
                            root.update_selected_config(|config| {
                                if audio_index >= config.external_audio_tracks.len() {
                                    return false;
                                }
                                config.external_audio_tracks.remove(audio_index);
                                true
                            });
                            cx.notify();
                        })),
                    )
                })
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
    let scheme = &timeline_scheme(palette, cx);
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
            move |root, event: &DragMoveEvent<PreviewTimelineAudioDrag>, _, cx| {
                let drag = event.drag(cx).clone();
                let width = f64::from(f32::from(event.bounds.size.width));
                if width <= 0.0 {
                    return;
                }
                let x = event.event.position.x - event.bounds.origin.x - drag.grab_x.get();
                let offset = (f64::from(f32::from(x)) / width * duration * 1000.0)
                    .clamp(0.0, duration.mul_add(1000.0, -1.0).max(0.0));
                let offset = crate::numeric::rounded_f64_to_u64(offset);
                root.update_selected_config(|config| {
                    let Some(track) = config.external_audio_tracks.get_mut(drag.index) else {
                        return false;
                    };
                    if track.offset_millis == offset {
                        return false;
                    }
                    track.offset_millis = offset;
                    true
                });
                cx.notify();
            },
        ))
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
    .child(preview_timeline_ruler(duration, palette, cx))
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
            .bg(color(scheme.on_surface))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(theme::ui_rem(-(PREVIEW_PLAYHEAD_KNOB_SIZE / 2.0)))
                    .ml(px(1.0))
                    .size(theme::ui_rem(PREVIEW_PLAYHEAD_KNOB_SIZE))
                    .rounded_full()
                    .bg(color(scheme.on_surface)),
            ),
    )
}

fn preview_timeline_ruler(
    duration: f64,
    palette: &'static theme::ThemePalette,
    cx: &gpui::App,
) -> gpui::Div {
    let scheme = &timeline_scheme(palette, cx);
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
                .border_color(color(scheme.outline_variant))
                .m3_type(typography::BODY_SMALL)
                .text_color(color(scheme.on_surface_variant))
                .child(format_timeline_ruler_time(time, duration))
        }))
}

#[expect(
    clippy::too_many_lines,
    reason = "A lane composes clip waveforms, audio movement, cut regions, and trim handles."
)]
fn preview_timeline_lane(
    state: &PreviewShellState,
    track_index: usize,
    track: &PreviewTimelineTrack,
    enabled: bool,
    cx: &Context<FrameRoot>,
) -> gpui::Div {
    let palette = state.palette;
    let scheme = &timeline_scheme(palette, cx);
    let current_time = state.playback.current_time();
    let active_clip = track
        .clips
        .iter()
        .copied()
        .find(|clip| clip.contains(current_time));
    let clip_name = track_index
        .checked_sub(1)
        .and_then(|index| state.audio_tracks.get(index))
        .and_then(|track| std::path::Path::new(&track.path).file_name())
        .map_or_else(
            || state.selected_file_name.clone().unwrap_or_default(),
            |name| name.to_string_lossy().into_owned(),
        );

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
        .rounded(theme::ui_rem(shape::CORNER_SMALL))
        .bg(color(scheme.surface_container))
        .opacity(
            if track_index > 0 && state.audio_tracks[track_index - 1].muted {
                0.4
            } else {
                1.0
            },
        )
        .children(track.clips.iter().enumerate().map(|(clip_index, clip)| {
            preview_timeline_clip(
                state,
                *clip,
                Some(*clip) == active_clip,
                &clip_name,
                track.icon,
                cx,
            )
            .when_some(
                state.waveforms.get(track_index).and_then(Clone::clone),
                |element, waveform| {
                    let source_start = if track_index == 0 { clip.start } else { 0.0 };
                    element.items_start().pt(theme::ui_rem(2.0)).child(
                        super::super::waveform::element(
                            waveform,
                            vec![(source_start, source_start + clip.duration())],
                            color(scheme.on_primary_container),
                        ),
                    )
                },
            )
            .id(format!("timeline-clip-{track_index}-{clip_index}"))
            .when(
                track_index > 0 && enabled && !state.controls_disabled,
                |clip| {
                    preview_audio_clip(
                        clip,
                        track_index - 1,
                        state.playback.duration(),
                        &clip_name,
                        cx,
                    )
                },
            )
        }))
        .when(edits_source, |lane| {
            lane.children(
                state
                    .playback
                    .cuts()
                    .iter()
                    .enumerate()
                    .map(|(index, cut)| {
                        preview_timeline_cut_region(state, index, cut, enabled, cx)
                    }),
            )
        })
        .children(edges.into_iter().map(|(target, value)| {
            let active =
                active_clip.is_some_and(|clip| clip.leading == target || clip.trailing == target);
            preview_timeline_handle(target, value, enabled, active, &state.playback, palette, cx)
        }))
}

fn preview_audio_clip(
    clip: gpui::Stateful<gpui::Div>,
    audio_index: usize,
    duration: f64,
    clip_name: &str,
    cx: &Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let duration_millis = crate::numeric::rounded_f64_to_u64(duration * 1000.0);
    clip.cursor_grab()
        .focusable()
        .tab_stop(true)
        .aria_label(format!(
            "Audio clip {clip_name}. Use left and right arrows to move."
        ))
        .on_key_down(cx.listener(move |root, event: &gpui::KeyDownEvent, _, cx| {
            let forward = match event.keystroke.key.as_str() {
                "left" => false,
                "right" => true,
                _ => return,
            };
            root.update_selected_config(|config| {
                let Some(track) = config.external_audio_tracks.get_mut(audio_index) else {
                    return false;
                };
                track.offset_millis = if forward {
                    track.offset_millis.saturating_add(100)
                } else {
                    track.offset_millis.saturating_sub(100)
                }
                .min(duration_millis.saturating_sub(1));
                true
            });
            cx.stop_propagation();
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_drag(
            PreviewTimelineAudioDrag {
                index: audio_index,
                grab_x: std::rc::Rc::new(std::cell::Cell::new(px(0.0))),
            },
            |drag, position, _, cx| {
                drag.grab_x.set(position.x);
                cx.new(|_| PreviewTimelineDragPreview)
            },
        )
}

fn preview_timeline_clip(
    state: &PreviewShellState,
    clip: TimelineClip,
    active: bool,
    name: &str,
    icon: Symbol,
    cx: &gpui::App,
) -> gpui::Div {
    let palette = state.palette;
    let scheme = &timeline_scheme(palette, cx);
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
        .rounded(theme::ui_rem(shape::CORNER_SMALL))
        .bg(color(scheme.primary_container))
        .border_color(color(if active {
            scheme.primary
        } else {
            scheme.outline_variant
        }))
        .when(active, gpui::Styled::border_2)
        .when(!active, gpui::Styled::border_1)
        .px(theme::ui_rem(PREVIEW_TIMELINE_CLIP_HANDLE_WIDTH + 4.0))
        .flex()
        .items_center()
        .gap(theme::ui_rem(6.0))
        .text_color(color(scheme.on_primary_container))
        .child(div().flex_none().child(m3::m3_icon(
            icon,
            false,
            16.0,
            color(scheme.on_primary_container),
        )))
        .child(
            div()
                .min_w_0()
                .truncate()
                .m3_type(typography::LABEL_MEDIUM)
                .child(name.to_string()),
        )
        .child(
            div()
                .flex_none()
                .m3_type(typography::BODY_SMALL)
                .text_color(color(scheme.on_primary_container))
                .child(format_clip_duration(clip.duration())),
        )
}

fn preview_timeline_cut_region(
    state: &PreviewShellState,
    index: usize,
    cut: &CutRange,
    enabled: bool,
    cx: &gpui::App,
) -> gpui::Stateful<gpui::Div> {
    let palette = state.palette;
    let scheme = &timeline_scheme(palette, cx);
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
        .rounded(theme::ui_rem(shape::CORNER_SMALL))
        .border_1()
        .border_dashed()
        .border_color(color(scheme.error.with_alpha(0.6)))
        .bg(color(scheme.error.with_alpha(0.12)));

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

    m3::Button::new(
        "preview-cut-toggle",
        if in_cut { "Restore" } else { "Remove segment" },
        m3::ButtonVariant::Tonal,
    )
    .icon(Symbol::ContentCut)
    .enabled(if in_cut {
        preview_trim_enabled(state)
    } else {
        preview_cut_enabled(state)
    })
    .build(&timeline_scheme(state.palette, cx), window, cx)
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
    let scheme = &timeline_scheme(palette, cx);
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
        .rounded(theme::ui_rem(shape::CORNER_SMALL))
        .when(active, |this| this.bg(color(scheme.primary)))
        .flex()
        .items_center()
        .justify_center()
        .child(preview_timeline_handle_grip(active, palette, cx))
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

fn preview_timeline_handle_grip(
    active: bool,
    palette: &'static theme::ThemePalette,
    cx: &gpui::App,
) -> gpui::Div {
    let scheme = &timeline_scheme(palette, cx);
    div()
        .w(px(2.0))
        .h(theme::ui_rem(14.0))
        .rounded(px(1.0))
        .bg(if active {
            color(scheme.on_primary)
        } else {
            color(scheme.on_surface.with_alpha(0.6))
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
    m3::IconButton::new(
        "preview-playback-toggle",
        if state.playback.is_playing() {
            Symbol::Pause
        } else {
            Symbol::PlayArrow
        },
        if state.playback.is_playing() {
            "Pause preview"
        } else {
            "Play preview"
        },
    )
    .variant(m3::IconButtonVariant::Filled)
    .enabled(enabled)
    .build(&timeline_scheme(state.palette, cx), window, cx)
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

fn timeline_scheme(palette: &theme::ThemePalette, cx: &gpui::App) -> material::ColorScheme {
    crate::app::m3::scheme(
        if palette.is_light() {
            ColorTheme::Light
        } else {
            ColorTheme::Dark
        },
        cx,
    )
}
