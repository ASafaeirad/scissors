//! The project timeline selects and moves instances, never library sources.
use super::m3::{self, M3Text};
use super::*;
use crate::material::{self, Symbol, shape, typography};

/// The visible time range is UI state, independent of the exported project duration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelineView {
    start: f64,
    span: f64,
}

impl TimelineView {
    pub(super) const fn fit(duration: f64) -> Self {
        Self {
            start: 0.0,
            span: duration.max(1.0),
        }
    }

    fn fraction(self, time: f64) -> f32 {
        crate::numeric::f64_to_f32((time - self.start) / self.span)
    }

    const fn time(self, fraction: f64) -> f64 {
        fraction.mul_add(self.span, self.start).max(0.0)
    }

    fn zoom(&mut self, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        self.span = (self.span / factor).clamp(0.1, 86400.0 * 7.0);
    }
}

impl FrameRoot {
    pub(super) fn project_timeline_view(&self) -> TimelineView {
        self.preview_ui
            .project_timeline_view
            .unwrap_or_else(|| TimelineView::fit(self.project_duration()))
    }

    fn zoom_project_timeline(&mut self, factor: f64) {
        let mut view = self.project_timeline_view();
        view.zoom(factor);
        self.preview_ui.project_timeline_view = Some(view);
    }

    fn scroll_project_timeline(&mut self, event: &ScrollWheelEvent) -> bool {
        let delta = event.delta.pixel_delta(px(20.0));
        if event.modifiers.control {
            self.zoom_project_timeline((f64::from(delta.y.as_f32()) * 0.01).exp());
        } else {
            // Linux and Windows already convert Shift+wheel to an X delta.
            // Use Y only when the platform leaves the wheel on its original axis.
            let horizontal = if delta.x == px(0.0) && event.modifiers.shift {
                delta.y
            } else {
                delta.x
            };
            if horizontal == px(0.0) {
                return false;
            }
            let Some(bounds) = self.preview_ui.timeline_track_bounds else {
                return false;
            };
            let mut view = self.project_timeline_view();
            let fraction =
                f64::from(horizontal.as_f32()) / f64::from(bounds.size.width.as_f32().max(1.0));
            view.start = fraction.mul_add(-view.span, view.start).max(0.0);
            self.preview_ui.project_timeline_view = Some(view);
        }
        true
    }

    fn seek_project_timeline(&mut self, position: Point<Pixels>, cx: &Context<Self>) -> bool {
        let Some(bounds) = self.preview_ui.timeline_track_bounds else {
            return false;
        };
        let fraction = timeline_slider_percent_from_bounds(position, bounds);
        let time = self.project_timeline_view().time(fraction);
        let position = point(
            bounds.left()
                + bounds.size.width
                    * crate::numeric::unit_f64_to_f32(
                        time / self.preview_ui.playback.duration().max(f64::EPSILON),
                    ),
            position.y,
        );
        self.commit_preview_timeline_seek_at_position_with_context(position, cx)
    }

    fn drop_library_source(&mut self, source_id: &str, position: Point<Pixels>) -> bool {
        let Some(bounds) = self.preview_ui.timeline_track_bounds else {
            return false;
        };
        if bounds.size.width <= px(0.0) {
            return false;
        }
        let view = self.project_timeline_view();
        let time = view.time(timeline_slider_percent_from_bounds(position, bounds));
        let first_clip = !self.preview_ui.first_clip_zoomed;
        if !self.add_library_clip_at(source_id, crate::numeric::rounded_f64_to_u64(time * 1000.0)) {
            return false;
        }
        // Keep the drop position stable when the new clip extends the project duration.
        // The first clip keeps the zoom it set in `zoom_to_first_project_clip`.
        if !first_clip {
            self.preview_ui.project_timeline_view = Some(view);
        }
        true
    }

    /// Zooms out once, when the first clip ever lands on the timeline, so the track
    /// spans 110% of the project length. Later clips keep whatever scale is in use.
    pub(super) fn zoom_to_first_project_clip(&mut self) {
        if std::mem::replace(&mut self.preview_ui.first_clip_zoomed, true) {
            return;
        }
        self.preview_ui.project_timeline_view = Some(TimelineView {
            start: 0.0,
            span: self.project_duration() * FIRST_CLIP_TRACK_SPAN,
        });
    }
}

/// The visible span after the first clip is added, as a multiple of the project length.
const FIRST_CLIP_TRACK_SPAN: f64 = 1.1;

#[derive(Clone)]
pub(super) struct LibrarySourceDrag(pub(super) String);

pub(super) struct LibraryDragPreview {
    pub(super) name: String,
    pub(super) scheme: material::ColorScheme,
}

impl Render for LibraryDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        m3::m3_surface(self.scheme.secondary_container, shape::CORNER_SMALL)
            .px(theme::ui_rem(12.0))
            .py(theme::ui_rem(8.0))
            .max_w(theme::ui_rem(240.0))
            .text_color(color(self.scheme.on_secondary_container))
            .m3_type(typography::LABEL_MEDIUM)
            .truncate()
            .child(self.name.clone())
    }
}

#[derive(Clone)]
struct ClipDrag {
    id: String,
    grab_x: std::rc::Rc<std::cell::Cell<Pixels>>,
    view: TimelineView,
}

#[derive(Clone)]
struct ClipTrimDrag {
    id: String,
    start: bool,
    initial_start: f64,
    initial_offset: u64,
    view: TimelineView,
}

pub(super) struct MediaDragPreview;
impl Render for MediaDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(theme::ui_rem(1.0))
    }
}

pub(super) struct ProjectTimelineState {
    pub(super) scheme: material::ColorScheme,
    pub(super) clips: Vec<super::project::ClipPlacement>,
    pub(super) sources: Vec<frame_core::types::TimelineSource>,
    pub(super) files: Vec<FileItem>,
    pub(super) selected: Option<String>,
    pub(super) duration: f64,
    pub(super) current: f64,
    pub(super) view: TimelineView,
    pub(super) enabled: bool,
}

#[expect(
    clippy::too_many_lines,
    reason = "The panel composes its viewport input, accessible ruler, and track rows."
)]
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
    let surface_container_high = scheme.surface_container_high;
    let current = *current;
    let enabled = *enabled;
    let duration = state.duration.max(1.0);
    let view = state.view;
    m3::m3_surface(scheme.surface_container_low, shape::CORNER_EXTRA_LARGE)
        .id("project-timeline")
        .flex_none()
        .min_w_0()
        .w_full()
        .max_h(theme::ui_rem(320.0))
        .overflow_y_scroll()
        .p(theme::ui_rem(16.0))
        .flex()
        .flex_col()
        .gap(theme::ui_rem(8.0))
        .text_color(color(scheme.on_surface))
        .on_pinch(cx.listener(|root, event: &PinchEvent, _, cx| {
            root.zoom_project_timeline(1.0 + f64::from(event.delta));
            cx.stop_propagation();
            cx.notify();
        }))
        .on_scroll_wheel(cx.listener(|root, event: &ScrollWheelEvent, _, cx| {
            if root.scroll_project_timeline(event) {
                cx.stop_propagation();
                cx.notify();
            }
        }))
        .drag_over::<LibrarySourceDrag>(move |style, _, _, _| {
            if enabled {
                style.bg(color(surface_container_high))
            } else {
                style
            }
        })
        .on_drop(cx.listener(|root, source: &LibrarySourceDrag, window, cx| {
            cx.stop_propagation();
            if root.drop_library_source(&source.0, window.mouse_position()) {
                cx.notify();
            }
        }))
        .child(project_timeline_toolbar(state, window, cx))
        .child(
            div()
                .id("project-ruler")
                .track_focus(focus)
                .tab_stop(true)
                .role(gpui::Role::Slider)
                .aria_label("Project playhead")
                .aria_numeric_value(current)
                .aria_min_numeric_value(0.0)
                .aria_max_numeric_value(duration)
                .aria_value(current.to_string())
                .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, _, cx| {
                    if root.adjust_preview_timeline_from_keyboard_with_context(
                        TimelineDragTarget::Scrub,
                        &event.keystroke.key,
                        cx,
                    ) {
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .h(theme::ui_rem(24.0))
                .ml(theme::ui_rem(212.0))
                .relative()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |root, event: &MouseDownEvent, _, cx| {
                        if root.seek_project_timeline(event.position, cx) {
                            cx.notify();
                        }
                    }),
                )
                .child(
                    super::preview_panel::timeline::PreviewTimelineTrackBoundsProbe {
                        owner: cx.entity(),
                    },
                )
                .children((0_u8..=4).map(|step| {
                    div()
                        .absolute()
                        .when(step < 4, |tick| tick.left(relative(f32::from(step) / 4.0)))
                        .when(step == 4, gpui::Styled::right_0)
                        .m3_type(typography::LABEL_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .child(format!("{:.1}s", view.time(f64::from(step) / 4.0)))
                })),
        )
        .children([false, true].into_iter().flat_map(|audio| {
            let rows: Vec<_> = clips
                .iter()
                .zip(sources)
                .filter(|(placement, _)| (placement.kind == SourceKind::Audio) == audio)
                .filter_map(|(placement, source)| {
                    let file = files.iter().find(|file| file.id == placement.id)?;
                    Some(track_row(
                        placement,
                        source,
                        file,
                        selected.as_deref() == Some(&placement.id),
                        view,
                        current,
                        enabled,
                        scheme,
                        window,
                        cx,
                    ))
                })
                .collect();
            if rows.is_empty() {
                vec![empty_track_row(audio, view, current, scheme, cx)]
            } else {
                rows
            }
        }))
}

fn empty_track_row(
    audio: bool,
    view: TimelineView,
    current: f64,
    scheme: &material::ColorScheme,
    cx: &Context<FrameRoot>,
) -> gpui::Div {
    let (id, label, symbol) = if audio {
        ("project-empty-audio", "Audio", Symbol::MusicNote)
    } else {
        ("project-empty-video", "Video", Symbol::Movie)
    };
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
                .gap(theme::ui_rem(8.0))
                .px(theme::ui_rem(12.0))
                .m3_type(typography::LABEL_LARGE)
                .child(m3::m3_icon(symbol, false, 18.0, color(scheme.on_surface)))
                .child(label),
        )
        .child(
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .aria_label(format!("Empty {label} track"))
                .flex_1()
                .min_w_0()
                .h(theme::ui_rem(40.0))
                .relative()
                .overflow_hidden()
                .rounded(theme::ui_rem(shape::CORNER_SMALL))
                .bg(color(scheme.surface_container_highest))
                .cursor_pointer()
                .on_click(cx.listener(|root, event: &gpui::ClickEvent, _, cx| {
                    if let gpui::ClickEvent::Mouse(event) = event
                        && root.seek_project_timeline(event.up.position, cx)
                    {
                        cx.notify();
                    }
                }))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .w(theme::ui_rem(2.0))
                        .left(relative(view.fraction(current)))
                        .bg(color(scheme.on_surface)),
                ),
        )
}

fn project_timeline_toolbar(
    state: &ProjectTimelineState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = &state.scheme;
    let audio_count = state
        .clips
        .iter()
        .filter(|clip| clip.kind == SourceKind::Audio)
        .count();
    let count = audio_count.max(1) + (state.clips.len() - audio_count).max(1);
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .child(
            div()
                .m3_type(typography::TITLE_SMALL)
                .child("Project timeline"),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .m3_type(typography::BODY_SMALL)
                .text_color(color(scheme.on_surface_variant))
                .child(format!("{count} tracks")),
        )
        .children(
            [
                (
                    "timeline-zoom-out",
                    Symbol::ZoomOut,
                    "Zoom out timeline",
                    0.8,
                ),
                ("timeline-zoom-in", Symbol::ZoomIn, "Zoom in timeline", 1.25),
            ]
            .into_iter()
            .map(|(id, symbol, label, factor)| {
                m3::IconButton::new(id, symbol, label)
                    .size(m3::IconButtonSize::ExtraSmall)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(move |root, _, _, cx| {
                        root.zoom_project_timeline(factor);
                        cx.notify();
                    }))
            }),
        )
        .child(
            m3::Button::new("timeline-fit", "Fit", m3::ButtonVariant::Text)
                .size(m3::ButtonSize::ExtraSmall)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    root.preview_ui.project_timeline_view =
                        Some(TimelineView::fit(root.project_duration()));
                    cx.notify();
                })),
        )
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
    view: TimelineView,
    current: f64,
    enabled: bool,
    scheme: &material::ColorScheme,
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
    let waveform = (placement.kind == SourceKind::Audio)
        .then(|| {
            super::waveform::request(
                &source.path,
                source.config.selected_audio_tracks.first().copied(),
                std::time::Duration::from_millis(source.duration_millis).as_secs_f64(),
                cx,
            )
        })
        .flatten();
    let trim_start = source
        .config
        .start_time
        .as_deref()
        .and_then(frame_core::utils::parse_time)
        .unwrap_or(0.0);
    let trim_end = source
        .config
        .end_time
        .as_deref()
        .and_then(frame_core::utils::parse_time)
        .unwrap_or_else(|| std::time::Duration::from_millis(source.duration_millis).as_secs_f64());
    let mut ranges = Vec::new();
    let mut cursor = trim_start;
    for cut in frame_core::cuts::output_cut_ranges(&source.config) {
        if trim_start + cut.start > cursor {
            ranges.push((cursor, trim_start + cut.start));
        }
        cursor = trim_start + cut.end;
    }
    if cursor < trim_end {
        ranges.push((cursor, trim_end));
    }

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
                .overflow_hidden()
                .rounded(theme::ui_rem(shape::CORNER_SMALL))
                .bg(color(scheme.surface_container_highest))
                .cursor_pointer()
                .on_click(cx.listener(|root, event: &gpui::ClickEvent, _, cx| {
                    if let gpui::ClickEvent::Mouse(event) = event
                        && root.seek_project_timeline(event.up.position, cx)
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
                        let offset = drag.view.time(f64::from(x.as_f32()) / width) * 1000.0;
                        root.preview_ui.project_timeline_view = Some(drag.view);
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
                        let time = drag.view.time(
                            f64::from((event.event.position.x - event.bounds.origin.x).as_f32())
                                / width,
                        );
                        root.preview_ui.project_timeline_view = Some(drag.view);
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
                        .left(relative(view.fraction(start)))
                        .w(relative(crate::numeric::f64_to_f32(length / view.span)))
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
                                    view,
                                },
                                move |drag, position, _, cx| {
                                    drag.grab_x.set(position.x);
                                    cx.new(|_| MediaDragPreview)
                                },
                            )
                        })
                        .when_some(waveform, |clip, waveform| {
                            clip.items_start().pt(theme::ui_rem(2.0)).child(
                                super::waveform::element(
                                    waveform,
                                    ranges,
                                    color(if selected {
                                        scheme.on_primary_container
                                    } else {
                                        scheme.on_secondary_container
                                    }),
                                ),
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
                                            view,
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
                        .left(relative(view.fraction(current)))
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

    #[test]
    fn drops_require_bounds_and_ready_editable_sources() {
        let mut root = FrameRoot::new();
        root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
        root.preview_ui.timeline_track_bounds = None;
        assert!(!root.drop_library_source("library-video", point(px(50.0), px(10.0))));
        root.preview_ui.timeline_track_bounds = Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(100.0), px(24.0)),
        ));
        let count = root.project.clips.len();
        assert!(!root.drop_library_source("missing", point(px(50.0), px(10.0))));
        root.project
            .library
            .add_file(FileItem::from_path("pending", "/tmp/pending.mp4", 1));
        assert!(!root.drop_library_source("pending", point(px(50.0), px(10.0))));
        root.is_processing = true;
        assert!(!root.drop_library_source("library-video", point(px(50.0), px(10.0))));
        assert_eq!(root.project.clips.len(), count);
        root.is_processing = false;
        for (source, x, offset) in [("library-audio", -10.0, 0), ("library-video", 150.0, 6000)] {
            assert!(root.drop_library_source(source, point(px(x), px(10.0))));
            assert_eq!(root.project.clips.last().unwrap().offset_millis, offset);
        }
    }

    impl Render for TimelineTestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.root.update(cx, |root, cx| {
                let state = root.project_render_state(cx).unwrap();
                project_timeline(&state.timeline, &self.focus, window, cx)
            })
        }
    }

    #[test]
    fn video_and_audio_rows_remain_after_their_last_clips_are_removed() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                let ids: Vec<_> = root
                    .project
                    .clips
                    .iter()
                    .map(|clip| clip.id.clone())
                    .collect();
                for id in ids {
                    assert!(root.remove_project_clip(&id));
                }
                root
            });
            TimelineTestView {
                root,
                focus: cx.focus_handle(),
            }
        });
        let root = view.read_with(cx, |view, _| view.root.clone());
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("project-empty-video").is_some());
        assert!(cx.debug_bounds("project-empty-audio").is_some());

        root.update(cx, |root, _| {
            assert!(root.add_library_clip("library-video"));
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("project-empty-video").is_none());
        assert!(cx.debug_bounds("project-empty-audio").is_some());

        root.update(cx, |root, _| {
            assert!(root.add_library_clip("library-audio"));
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("project-empty-video").is_none());
        assert!(cx.debug_bounds("project-empty-audio").is_none());

        root.update(cx, |root, _| {
            let ids: Vec<_> = root
                .project
                .clips
                .iter()
                .map(|clip| clip.id.clone())
                .collect();
            for id in ids {
                assert!(root.remove_project_clip(&id));
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("project-empty-video").is_some());
        assert!(cx.debug_bounds("project-empty-audio").is_some());
    }

    #[test]
    fn only_the_first_ever_clip_zooms_the_timeline_to_its_track_length() {
        let mut root = crate::app::project::tests::project();
        assert!(root.add_library_clip("source"));
        let duration = root.project_duration();
        assert_eq!(
            root.project_timeline_view(),
            TimelineView {
                start: 0.0,
                span: duration * FIRST_CLIP_TRACK_SPAN,
            }
        );

        let zoomed = TimelineView {
            start: 1.0,
            span: 50.0,
        };
        root.preview_ui.project_timeline_view = Some(zoomed);
        assert!(root.add_library_clip("source"));
        assert_eq!(root.project_timeline_view(), zoomed);

        for _ in 0..2 {
            let id = root.project.clips[0].id.clone();
            assert!(root.remove_project_clip(&id));
        }
        assert!(root.add_library_clip("source"));
        root.preview_ui.project_timeline_view = Some(zoomed);
        assert!(root.add_library_clip("source"));
        assert_eq!(root.project_timeline_view(), zoomed);
    }

    #[test]
    fn dropping_the_first_clip_also_zooms_the_timeline() {
        let mut root = crate::app::project::tests::project();
        root.preview_ui.timeline_track_bounds = Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(100.0), px(24.0)),
        ));
        assert!(root.drop_library_source("source", point(px(50.0), px(10.0))));
        assert_eq!(
            root.project_timeline_view(),
            TimelineView {
                start: 0.0,
                span: root.project_duration() * FIRST_CLIP_TRACK_SPAN,
            }
        );
    }

    #[test]
    fn shift_scroll_pans_with_original_and_platform_converted_axes() {
        let mut root = FrameRoot::new();
        let view = TimelineView {
            start: 10.0,
            span: 20.0,
        };
        root.preview_ui.timeline_track_bounds = Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(200.0), px(24.0)),
        ));
        for delta in [
            gpui::ScrollDelta::Pixels(point(px(0.0), px(-20.0))),
            gpui::ScrollDelta::Pixels(point(px(-20.0), px(0.0))),
            gpui::ScrollDelta::Lines(point(0.0, -1.0)),
            gpui::ScrollDelta::Lines(point(-1.0, 0.0)),
        ] {
            root.preview_ui.project_timeline_view = Some(view);
            assert!(root.scroll_project_timeline(&ScrollWheelEvent {
                delta,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                ..ScrollWheelEvent::default()
            }));
            assert_eq!(
                root.project_timeline_view(),
                TimelineView {
                    start: 12.0,
                    span: 20.0
                }
            );
        }
        root.preview_ui.project_timeline_view = Some(view);
        assert!(!root.scroll_project_timeline(&ScrollWheelEvent {
            delta: gpui::ScrollDelta::Lines(point(0.0, -1.0)),
            ..ScrollWheelEvent::default()
        }));
        assert_eq!(root.project_timeline_view(), view);
    }

    #[test]
    fn zoom_keeps_visible_start_fixed_and_limits_invalid_gestures() {
        for start in [0.0, 30.0] {
            let original = TimelineView { start, span: 100.0 };
            let mut view = original;
            view.zoom(4.0);
            assert_eq!(view, TimelineView { start, span: 25.0 });
            view.zoom(0.25);
            assert_eq!(view, original);
            for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
                view.zoom(factor);
                assert_eq!(view, original);
            }
            view.zoom(1e9);
            assert_eq!(view, TimelineView { start, span: 0.1 });
            view.zoom(1e-9);
            assert_eq!(
                view,
                TimelineView {
                    start,
                    span: 604_800.0
                }
            );
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
                f64::from(fraction)
                    .mul_add(-root.read_with(cx, |root, _| root.project_duration()), time)
                    .abs()
                    < 0.01,
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

    #[test]
    fn dragging_and_trimming_use_the_panned_zoom_scale() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                root.preview_ui.project_timeline_view = Some(TimelineView {
                    start: 2.0,
                    span: 8.0,
                });
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
        let id = root.read_with(cx, |root, _| root.project.clips[0].id.clone());
        let start = point(
            bounds.left() + bounds.size.width * 0.125,
            bounds.bottom() + px(28.0),
        );
        // GPUI starts the drag after crossing its threshold and captures that grab point.
        let end = point(start.x + px(8.0) + bounds.size.width * 0.125, start.y);
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(start.x + px(8.0), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.update(|window, cx| window.draw(cx).clear());
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
        root.read_with(cx, |root, _| {
            assert!(
                root.project.clips[0].offset_millis.abs_diff(1000) <= 1,
                "offset {}",
                root.project.clips[0].offset_millis
            );
            assert_eq!(
                root.project_timeline_view(),
                TimelineView {
                    start: 2.0,
                    span: 8.0
                }
            );
        });
        cx.update(|window, cx| window.draw(cx).clear());
        let start = point(bounds.left() + bounds.size.width * 0.375 - px(3.0), end.y);
        let end = point(bounds.left() + bounds.size.width * 0.25, end.y);
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(start.x + px(8.0), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.update(|window, cx| window.draw(cx).clear());
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
        root.read_with(cx, |root, _| {
            let trim_end = root
                .file_queue
                .file_by_id(&id)
                .unwrap()
                .config
                .end_time
                .as_deref()
                .map_or(0.0, parse_time_to_seconds);
            assert!((trim_end - 3.0).abs() < 0.01, "trim end was {trim_end}");
            assert_eq!(
                root.project_timeline_view(),
                TimelineView {
                    start: 2.0,
                    span: 8.0
                }
            );
        });
    }

    #[test]
    fn wheel_pinch_and_pan_preserve_output_and_seek_visible_time() {
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
        let output = root.read_with(cx, |root, _| root.project_config().unwrap().end_time);
        let duration = root.read_with(cx, |root, _| root.project_duration());
        let initial_view = TimelineView {
            start: 1.0,
            span: duration,
        };
        root.update(cx, |root, _| {
            root.preview_ui.project_timeline_view = Some(initial_view);
        });
        let position = bounds.center();
        cx.simulate_event(ScrollWheelEvent {
            position,
            delta: gpui::ScrollDelta::Lines(point(0.0, 1.0)),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
            ..ScrollWheelEvent::default()
        });
        let wheel_view = root.read_with(cx, |root, _| root.project_timeline_view());
        assert!(wheel_view.span < duration);
        assert!((wheel_view.start - initial_view.start).abs() < f64::EPSILON);
        cx.simulate_event(PinchEvent {
            position: point(bounds.left() + bounds.size.width * 0.75, position.y),
            delta: 1.0,
            ..PinchEvent::default()
        });
        let pinch_view = root.read_with(cx, |root, _| root.project_timeline_view());
        assert!((pinch_view.span - wheel_view.span / 2.0).abs() < 0.001);
        assert!((pinch_view.start - initial_view.start).abs() < f64::EPSILON);
        cx.simulate_click(position, Modifiers::default());
        root.read_with(cx, |root, _| {
            assert!(
                (root.preview_ui.playback.current_time() - pinch_view.time(0.5)).abs() < 0.01,
                "actual {}, expected {}, bounds {:?}",
                root.preview_ui.playback.current_time(),
                pinch_view.time(0.5),
                root.preview_ui.timeline_track_bounds
            );
            assert!((root.project_duration() - (duration)).abs() < f64::EPSILON);
            assert_eq!(root.project_config().unwrap().end_time, output);
            assert_eq!(
                root.project
                    .clips
                    .iter()
                    .map(|clip| clip.offset_millis)
                    .collect::<Vec<_>>(),
                original
            );
        });
        cx.simulate_event(ScrollWheelEvent {
            position,
            delta: gpui::ScrollDelta::Pixels(point(px(-100.0), px(0.0))),
            ..ScrollWheelEvent::default()
        });
        let panned_view = root.read_with(cx, |root, _| root.project_timeline_view());
        assert!(panned_view.start > pinch_view.start);
        assert!((panned_view.span - (pinch_view.span)).abs() < f64::EPSILON);
        root.update(cx, |root, _| {
            let id = root.project.clips[0].id.clone();
            assert!(root.move_project_clip(&id, 20_000));
            assert_eq!(root.project_timeline_view(), panned_view);
        });
    }
}
