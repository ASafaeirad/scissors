//! The project timeline selects and moves instances, never library sources.
use super::m3::{self, M3Text};
use super::*;
use crate::material::{self, Symbol, shape, typography};

#[derive(Clone)]
pub(super) struct LibrarySourceDrag(pub(super) String);

#[derive(Clone)]
struct ClipDrag {
    id: String,
    grab_x: std::rc::Rc<std::cell::Cell<Pixels>>,
    duration: f64,
}

#[derive(Clone)]
struct ClipTrimDrag {
    id: String,
    start: bool,
    initial_start: f64,
    initial_offset: u64,
    duration: f64,
}

pub(super) struct MediaDragPreview;
impl Render for MediaDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(theme::ui_rem(1.0))
    }
}

pub(super) struct ProjectTimelineState {
    pub(super) scheme: &'static material::ColorScheme,
    pub(super) clips: Vec<super::project::ClipPlacement>,
    pub(super) sources: Vec<frame_core::types::TimelineSource>,
    pub(super) files: Vec<FileItem>,
    pub(super) selected: Option<String>,
    pub(super) duration: f64,
    pub(super) current: f64,
    pub(super) enabled: bool,
}

pub(super) fn project_timeline(
    state: &ProjectTimelineState,
    focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let ProjectTimelineState {
        scheme,
        clips,
        sources,
        files,
        selected,
        current,
        enabled,
        ..
    } = state;
    let scheme = *scheme;
    let current = *current;
    let enabled = *enabled;
    let duration = state.duration.max(1.0);
    let count = clips.len();
    m3::m3_surface(scheme.surface_container_low, shape::CORNER_EXTRA_LARGE)
        .id("project-timeline")
        .flex_none()
        .min_w_0()
        .w_full()
        .max_h(theme::ui_rem(320.0))
        .overflow_y_scroll()
        .p(theme::ui_rem(16.0))
        .flex().flex_col().gap(theme::ui_rem(8.0))
        .on_drop(cx.listener(|root, source: &LibrarySourceDrag, _, cx| {
            cx.stop_propagation();
            if root.add_library_clip(&source.0) { cx.notify(); }
        }))
        .child(div().w_full().min_w_0().flex().items_center().gap(theme::ui_rem(12.0))
            .child(div().m3_type(typography::TITLE_SMALL).child("Project timeline"))
            .child(div().flex_1().min_w_0().truncate().m3_type(typography::BODY_SMALL).text_color(color(scheme.on_surface_variant))
                .child(format!("{count} tracks • Drag media here. Select a track to edit its properties."))))
        .child(div().id("project-ruler").track_focus(focus).tab_stop(true).role(gpui::Role::Slider).aria_label("Project playhead").aria_numeric_value(current).aria_min_numeric_value(0.0).aria_max_numeric_value(duration).aria_value(current.to_string())
                .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, _, cx| {
                    if root.adjust_preview_timeline_from_keyboard_with_context(TimelineDragTarget::Scrub, &event.keystroke.key, cx) { cx.stop_propagation(); cx.notify(); }
                })).h(theme::ui_rem(24.0)).ml(theme::ui_rem(212.0))
            .relative().cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |root, event: &MouseDownEvent, _, cx| {
                if root.commit_preview_timeline_seek_at_position_with_context(event.position, cx) { cx.notify(); }
            }))
            .child(super::preview_panel::timeline::PreviewTimelineTrackBoundsProbe { owner: cx.entity() })
            .children((0_u8..=4).map(|step| div().absolute().left(relative(f32::from(step) / 4.0)).m3_type(typography::LABEL_SMALL)
                .text_color(color(scheme.on_surface_variant)).child(format!("{:.1}s", f64::from(step) * duration / 4.0)))))
        .when(clips.is_empty(), |panel| panel.child(div().p(theme::ui_rem(24.0)).text_color(color(scheme.on_surface_variant)).child("Add sources from the library to start editing.")))
        .children(clips.iter().zip(sources).filter_map(|(placement, source)| {
            let file = files.iter().find(|file| file.id == placement.id)?;
            Some(track_row(placement, source, file, selected.as_deref() == Some(&placement.id), duration, current, enabled, scheme, window, cx))
        }))
}

#[expect(
    clippy::too_many_arguments,
    reason = "A track row has its source, instance, selection, ruler, and render context."
)]
#[expect(
    clippy::too_many_lines,
    reason = "A row composes its property controls, movable clip, and independent trim handles."
)]
fn track_row(
    placement: &super::project::ClipPlacement,
    source: &frame_core::types::TimelineSource,
    file: &FileItem,
    selected: bool,
    duration: f64,
    current: f64,
    enabled: bool,
    scheme: &'static material::ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let id = placement.id.clone();
    let start = std::time::Duration::from_millis(placement.offset_millis).as_secs_f64();
    let length = std::time::Duration::from_millis(
        frame_core::project_timeline::clip_duration_millis(source),
    )
    .as_secs_f64();
    let symbol = match placement.kind {
        SourceKind::Audio => Symbol::MusicNote,
        SourceKind::Image => Symbol::Image,
        SourceKind::Video => Symbol::Movie,
    };
    let grab = std::rc::Rc::new(std::cell::Cell::new(px(0.0)));
    let source_name = file.name.clone();
    div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .child(
            div()
                .w(theme::ui_rem(200.0))
                .flex_none()
                .flex()
                .items_center()
                .gap(theme::ui_rem(4.0))
                .child(
                    m3::Button::new(
                        format!("project-select-{id}"),
                        match placement.kind {
                            SourceKind::Audio => "Audio",
                            SourceKind::Image => "Image",
                            SourceKind::Video => "Video",
                        },
                        m3::ButtonVariant::Text,
                    )
                    .icon(symbol)
                    .size(m3::ButtonSize::ExtraSmall)
                    .enabled(enabled)
                    .build(scheme, window, cx)
                    .flex_1()
                    .min_w_0()
                    .aria_label(format!("Edit {source_name} track properties"))
                    .aria_selected(selected)
                    .on_click(cx.listener({
                        let id = id.clone();
                        move |root, _, _, cx| {
                            if root.select_project_clip(&id) {
                                cx.notify();
                            }
                        }
                    })),
                )
                .child(
                    m3::IconButton::new(
                        format!("project-mute-{id}"),
                        if placement.muted {
                            Symbol::VolumeOff
                        } else {
                            Symbol::VolumeUp
                        },
                        format!(
                            "{} {}",
                            if placement.muted { "Unmute" } else { "Mute" },
                            source_name
                        ),
                    )
                    .size(m3::IconButtonSize::ExtraSmall)
                    .enabled(enabled)
                    .build(scheme, window, cx)
                    .on_click(cx.listener({
                        let id = id.clone();
                        move |root, _, _, cx| {
                            if !root.is_processing
                                && let Some(clip) =
                                    root.project.clips.iter_mut().find(|clip| clip.id == id)
                            {
                                clip.muted = !clip.muted;
                                cx.notify();
                            }
                        }
                    })),
                )
                .child(
                    m3::IconButton::new(
                        format!("project-remove-{id}"),
                        Symbol::Delete,
                        format!("Remove {source_name} from timeline"),
                    )
                    .size(m3::IconButtonSize::ExtraSmall)
                    .enabled(enabled)
                    .build(scheme, window, cx)
                    .on_click(cx.listener({
                        let id = id.clone();
                        move |root, _, _, cx| {
                            if root.remove_project_clip(&id) {
                                cx.notify();
                            }
                        }
                    })),
                ),
        )
        .child(
            div()
                .id(format!("project-lane-{id}"))
                .flex_1()
                .min_w_0()
                .h(theme::ui_rem(40.0))
                .relative()
                .rounded(theme::ui_rem(shape::CORNER_SMALL))
                .bg(color(scheme.surface_container_highest))
                .cursor_pointer()
                .on_click(cx.listener(|root, event: &gpui::ClickEvent, _, cx| {
                    if let gpui::ClickEvent::Mouse(event) = event
                        && root.commit_preview_timeline_seek_at_position_with_context(
                            event.up.position,
                            cx,
                        )
                    {
                        cx.notify();
                    }
                }))
                .on_drag_move(
                    cx.listener(move |root, event: &DragMoveEvent<ClipDrag>, _, cx| {
                        let drag = event.drag(cx).clone();
                        let width = f64::from(event.bounds.size.width.as_f32());
                        if width <= 0.0 {
                            return;
                        }
                        let x = event.event.position.x - event.bounds.origin.x - drag.grab_x.get();
                        let offset =
                            (f64::from(x.as_f32()) / width * drag.duration * 1000.0).max(0.0);
                        if root
                            .move_project_clip(&drag.id, crate::numeric::rounded_f64_to_u64(offset))
                        {
                            cx.notify();
                        }
                    }),
                )
                .on_drag_move(cx.listener(
                    move |root, event: &DragMoveEvent<ClipTrimDrag>, _, cx| {
                        let drag = event.drag(cx).clone();
                        let width = f64::from(event.bounds.size.width.as_f32());
                        if width <= 0.0 {
                            return;
                        }
                        let time =
                            f64::from((event.event.position.x - event.bounds.origin.x).as_f32())
                                / width
                                * drag.duration;
                        root.trim_project_clip(
                            &drag.id,
                            drag.start,
                            time,
                            drag.initial_start,
                            drag.initial_offset,
                        );
                        cx.notify();
                    },
                ))
                .child(
                    div()
                        .id(format!("project-clip-{id}"))
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(crate::numeric::unit_f64_to_f32(start / duration)))
                        .w(relative(crate::numeric::unit_f64_to_f32(length / duration)))
                        .min_w(theme::ui_rem(16.0))
                        .rounded(theme::ui_rem(shape::CORNER_SMALL))
                        .bg(color(if selected {
                            scheme.primary_container
                        } else {
                            scheme.secondary_container
                        }))
                        .text_color(color(if selected {
                            scheme.on_primary_container
                        } else {
                            scheme.on_secondary_container
                        }))
                        .m3_type(typography::LABEL_MEDIUM)
                        .flex()
                        .items_center()
                        .px(theme::ui_rem(10.0))
                        .overflow_hidden()
                        .role(gpui::Role::Button)
                        .aria_label(format!("Select {source_name} track"))
                        .aria_selected(selected)
                        .focusable()
                        .tab_stop(true)
                        .on_click(cx.listener({
                            let id = id.clone();
                            move |root, _, _, cx| {
                                if root.select_project_clip(&id) {
                                    cx.notify();
                                }
                            }
                        }))
                        .on_key_down(cx.listener({
                            let id = id.clone();
                            move |root, event: &gpui::KeyDownEvent, _, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    root.select_project_clip(&id);
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                                if matches!(event.keystroke.key.as_str(), "left" | "right") {
                                    let offset = root
                                        .project
                                        .clips
                                        .iter()
                                        .find(|clip| clip.id == id)
                                        .map_or(0, |clip| clip.offset_millis);
                                    root.move_project_clip(
                                        &id,
                                        if event.keystroke.key == "left" {
                                            offset.saturating_sub(100)
                                        } else {
                                            offset.saturating_add(100)
                                        },
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                            }
                        }))
                        .when(enabled, |clip| {
                            clip.on_drag(
                                ClipDrag {
                                    id: id.clone(),
                                    grab_x: grab.clone(),
                                    duration,
                                },
                                move |drag, position, _, cx| {
                                    drag.grab_x.set(position.x);
                                    cx.new(|_| MediaDragPreview)
                                },
                            )
                        })
                        .child(
                            div()
                                .truncate()
                                .child(format!("{source_name} • {length:.1}s")),
                        )
                        .children([true, false].into_iter().map(|start| {
                            div()
                                .id(format!("project-trim-{id}-{start}"))
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .w(theme::ui_rem(6.0))
                                .when(start, gpui::Styled::left_0)
                                .when(!start, gpui::Styled::right_0)
                                .bg(color(scheme.outline))
                                .cursor_ew_resize()
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .when(enabled, |handle| {
                                    handle.on_drag(
                                        ClipTrimDrag {
                                            id: id.clone(),
                                            start,
                                            duration,
                                            initial_offset: placement.offset_millis,
                                            initial_start: file
                                                .config
                                                .start_time
                                                .as_deref()
                                                .map_or(0.0, parse_time_to_seconds),
                                        },
                                        |_, _, _, cx| cx.new(|_| MediaDragPreview),
                                    )
                                })
                        })),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .w(theme::ui_rem(2.0))
                        .left(relative(crate::numeric::unit_f64_to_f32(
                            current / duration,
                        )))
                        .bg(color(scheme.on_surface)),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, Modifiers, TestAppContext};

    struct TimelineTestView {
        root: Entity<FrameRoot>,
        focus: FocusHandle,
    }

    impl Render for TimelineTestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.root.update(cx, |root, cx| {
                let state = root.project_render_state().unwrap();
                project_timeline(&state.timeline, &self.focus, window, cx)
            })
        }
    }

    #[test]
    fn clicking_ruler_clip_and_empty_lane_seeks_without_moving_clips() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                root.preview_ui.playback =
                    preview_playback_state(PreviewMediaKind::Video, 8.0, None, None);
                root
            });
            TimelineTestView {
                root,
                focus: cx.focus_handle(),
            }
        });
        let root = view.read_with(cx, |view, _| view.root.clone());
        cx.update(|window, cx| window.draw(cx).clear());
        let bounds = root.read_with(cx, |root, _| root.preview_ui.timeline_track_bounds.unwrap());
        let original = root.read_with(cx, |root, _| {
            root.project
                .clips
                .iter()
                .map(|clip| clip.offset_millis)
                .collect::<Vec<_>>()
        });
        // Ruler, a clip body, and the empty half of that clip's lane.
        for (fraction, y) in [
            (0.75, bounds.center().y),
            (0.25, bounds.bottom() + px(28.0)),
            (0.75, bounds.bottom() + px(28.0)),
        ] {
            cx.simulate_click(
                point(bounds.left() + bounds.size.width * fraction, y),
                Modifiers::default(),
            );
            let time = root.read_with(cx, |root, _| root.preview_ui.playback.current_time());
            assert!(
                f64::from(fraction).mul_add(-8.0, time).abs() < 0.01,
                "playhead was {time}"
            );
        }
        root.read_with(cx, |root, _| {
            assert_eq!(
                root.file_queue.selected_file_id(),
                Some(root.project.clips[0].id.as_str())
            );
            assert_eq!(
                root.project
                    .clips
                    .iter()
                    .map(|clip| clip.offset_millis)
                    .collect::<Vec<_>>(),
                original
            );
        });
    }
}
