//! The project timeline selects and moves instances, never library sources.
//!
//! Layout: a toolbar, then a fixed header column beside a ruler and one lane per track. The
//! ruler and lanes share a width, so a project time maps to the same x in each.
use super::m3::{self, M3Text};
use super::project::ClipPlacement;
use super::project_tracks::{TrackKind, TrackToggle, Tracks};
use super::*;
use crate::material::{self, Symbol, shape, typography};
use frame_core::types::TimelineSource;
use std::{cell::Cell, rc::Rc};

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
        self.span = (self.span / factor).clamp(MIN_SPAN, MAX_SPAN);
    }
}

/// The narrowest and widest visible spans, in seconds.
const MIN_SPAN: f64 = 0.1;
const MAX_SPAN: f64 = 86400.0 * 7.0;
/// The zoom slider runs from the fitted span down to this many seconds.
const SLIDER_MIN_SPAN: f64 = 1.0;

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

    fn pan_project_timeline(&mut self, start: f64) {
        let mut view = self.project_timeline_view();
        view.start = start.max(0.0);
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
            let view = self.project_timeline_view();
            let fraction =
                f64::from(horizontal.as_f32()) / f64::from(bounds.size.width.as_f32().max(1.0));
            self.pan_project_timeline(fraction.mul_add(-view.span, view.start));
        }
        true
    }

    fn seek_project_timeline(
        &mut self,
        position: Point<Pixels>,
        track: Option<usize>,
        cx: &Context<Self>,
    ) -> bool {
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
        if !self.commit_preview_timeline_seek_at_position_with_context(position, cx) {
            return false;
        }
        // Ruler clicks select the highest video clip covering the playhead, skipping
        // empty tracks and gaps. Lane clicks select only within the clicked lane.
        let time = self.preview_ui.playback.current_time();
        let selected = self
            .project_timeline_clips()
            .into_iter()
            .filter(|(placement, source)| {
                let (start, length) = clip_extent(source);
                let eligible = track.map_or_else(
                    || self.project.tracks[placement.track].kind == TrackKind::Video,
                    |track| placement.track == track,
                );
                eligible && time >= start && time < start + length
            })
            .max_by_key(|(placement, _)| placement.track)
            .map(|(placement, _)| placement.id);
        if let Some(id) = selected {
            self.select_project_clip(&id);
        }
        true
    }

    fn drop_library_source(&mut self, source_id: &str, position: Point<Pixels>) -> bool {
        self.drop_library_source_on(source_id, position, None)
    }

    /// Adds a library source where it was dropped: at the pointer's time, on `track` when it
    /// holds the source's kind, otherwise on the default track.
    fn drop_library_source_on(
        &mut self,
        source_id: &str,
        position: Point<Pixels>,
        track: Option<usize>,
    ) -> bool {
        let Some(bounds) = self.preview_ui.timeline_track_bounds else {
            return false;
        };
        if bounds.size.width <= px(0.0) {
            return false;
        }
        let view = self.project_timeline_view();
        let time = view.time(timeline_slider_percent_from_bounds(position, bounds));
        let offset = crate::numeric::rounded_f64_to_u64(time * 1000.0);
        let first_clip = !self.preview_ui.first_clip_zoomed;
        let added = match track {
            Some(track) => self.add_library_clip_on_track(source_id, offset, track),
            None => self.add_library_clip_at(source_id, offset),
        };
        if !added {
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

    pub(super) fn remove_selected_project_clip(&mut self) -> bool {
        let Some(id) = self.file_queue.selected_file_id().map(str::to_string) else {
            return false;
        };
        self.remove_project_clip(&id)
    }

    /// Runs one step of a drag gesture, recording history before its first change only.
    fn gesture_edit(
        &mut self,
        recorded: &Cell<bool>,
        edit: impl FnOnce(&mut Self) -> bool,
    ) -> bool {
        let before = (!recorded.get()).then(|| self.timeline_snapshot());
        let changed = edit(self);
        if changed && let Some(before) = before {
            self.project.history.push(before);
            recorded.set(true);
        }
        changed
    }

    /// Clip edges, the playhead, and the project start: where moves and trims snap.
    fn snap_targets(&self, exclude: &str) -> Vec<f64> {
        let mut targets = vec![0.0, self.preview_ui.playback.current_time()];
        for (placement, source) in self.project_timeline_clips() {
            if placement.id != exclude {
                let (start, length) = clip_extent(&source);
                targets.extend([start, start + length]);
            }
        }
        targets
    }

    /// How close, in seconds, an edge must come to a target to snap. `None` with snapping off.
    fn snap_threshold(&self, view: TimelineView, width: f64) -> Option<f64> {
        (self.preview_ui.timeline_snap && width > 0.0)
            .then(|| f64::from(SNAP_DISTANCE) * view.span / width)
    }

    fn drag_project_clip(
        &mut self,
        drag: &ClipDrag,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let width = f64::from(bounds.size.width.as_f32());
        if width <= 0.0 {
            return false;
        }
        let x = position.x - bounds.origin.x - drag.grab_x.get();
        let mut start = drag.view.time(f64::from(x.as_f32()) / width);
        if let Some(threshold) = self.snap_threshold(drag.view, width) {
            start += snap_shift(
                &[start, start + drag.length],
                &self.snap_targets(&drag.id),
                threshold,
            );
        }
        self.preview_ui.project_timeline_view = Some(drag.view);
        let offset = crate::numeric::rounded_f64_to_u64(start.max(0.0) * 1000.0);
        self.gesture_edit(&drag.recorded, |root| {
            root.move_project_clip(&drag.id, offset)
        })
    }

    fn drag_project_trim(
        &mut self,
        drag: &ClipTrimDrag,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let width = f64::from(bounds.size.width.as_f32());
        if width <= 0.0 {
            return false;
        }
        let mut time = drag
            .view
            .time(f64::from((position.x - bounds.origin.x).as_f32()) / width);
        if let Some(threshold) = self.snap_threshold(drag.view, width) {
            time += snap_shift(&[time], &self.snap_targets(&drag.id), threshold);
        }
        self.preview_ui.project_timeline_view = Some(drag.view);
        self.gesture_edit(&drag.recorded, |root| {
            root.trim_project_clip(
                &drag.id,
                drag.start,
                time,
                drag.initial_start,
                drag.initial_offset,
            )
        })
    }

    fn toggle_timeline_menu(
        &mut self,
        target: MenuTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .preview_ui
            .timeline_menu
            .as_ref()
            .is_some_and(|menu| menu.target == target)
        {
            self.preview_ui.timeline_menu = None;
            return;
        }
        let count = match target {
            MenuTarget::AddTrack => 2,
            MenuTarget::Track(_) => 1,
        };
        let focuses: Vec<_> = (0..count).map(|_| cx.focus_handle()).collect();
        focuses[0].focus(window, cx);
        self.preview_ui.timeline_menu = Some(TimelineMenu { target, focuses });
    }
}

/// The visible span after the first clip is added, as a multiple of the project length.
const FIRST_CLIP_TRACK_SPAN: f64 = 1.1;
/// Pixels within which a dragged edge snaps to a target.
const SNAP_DISTANCE: f32 = 8.0;

/// The shift that moves the closest of `edges` onto a target within `threshold`, or zero.
fn snap_shift(edges: &[f64], targets: &[f64], threshold: f64) -> f64 {
    let mut best: Option<f64> = None;
    for edge in edges {
        for target in targets {
            let shift = target - edge;
            if shift.abs() <= threshold && best.is_none_or(|best| shift.abs() < best.abs()) {
                best = Some(shift);
            }
        }
    }
    best.unwrap_or(0.0)
}

/// A clip's start and length in project seconds.
fn clip_extent(source: &TimelineSource) -> (f64, f64) {
    (
        std::time::Duration::from_millis(source.offset_millis).as_secs_f64(),
        std::time::Duration::from_millis(frame_core::project_timeline::clip_duration_millis(
            source,
        ))
        .as_secs_f64(),
    )
}

/// Ruler steps in seconds, from a tenth of a second to half a day.
const TICK_STEPS: [f64; 19] = [
    0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0,
    3600.0, 7200.0, 18000.0, 43200.0,
];
/// Labeled ticks are at least this many pixels apart.
const MIN_MAJOR_TICK_DISTANCE: f32 = 96.0;
/// Unlabeled ticks between two labeled ones, plus one.
const MINOR_TICK_DIVISIONS: u32 = 5;

/// The smallest step whose labeled ticks are far enough apart to read.
fn major_tick_step(span: f64, width: f32) -> f64 {
    let min_step = span * f64::from(MIN_MAJOR_TICK_DISTANCE) / f64::from(width.max(1.0));
    TICK_STEPS
        .into_iter()
        .find(|step| *step >= min_step)
        .unwrap_or(TICK_STEPS[TICK_STEPS.len() - 1])
}

/// Times of the ticks inside `view`, each with whether it is labeled.
fn ruler_ticks(view: TimelineView, step: f64) -> Vec<(f64, bool)> {
    let minor = step / f64::from(MINOR_TICK_DIVISIONS);
    let end = view.start + view.span;
    let mut ticks = Vec::new();
    let mut index = (view.start / minor).ceil();
    loop {
        let time = index * minor;
        if time > minor.mul_add(1e-6, end) {
            break;
        }
        let major = (index / f64::from(MINOR_TICK_DIVISIONS)).fract().abs() < 1e-6;
        ticks.push((time, major));
        index += 1.0;
    }
    ticks
}

fn tenths(seconds: f64) -> (u64, u64) {
    let tenths = crate::numeric::rounded_f64_to_u64(seconds.max(0.0) * 10.0);
    (tenths / 10, tenths % 10)
}

fn clock(whole: u64) -> String {
    format!("{}:{:02}:{:02}", whole / 3600, whole / 60 % 60, whole % 60)
}

/// `H:MM:SS.d`, as in the toolbar and status line.
fn format_timecode(seconds: f64) -> String {
    let (whole, tenth) = tenths(seconds);
    format!("{}.{tenth}", clock(whole))
}

/// Ruler labels drop the tenths unless ticks are closer than a second.
fn format_ruler_time(seconds: f64, step: f64) -> String {
    if step < 1.0 {
        format_timecode(seconds)
    } else {
        clock(crate::numeric::rounded_f64_to_u64(seconds.max(0.0)))
    }
}

/// Maps a visible span to the zoom slider, where 0 is the fitted view and 1 is zoomed in.
fn zoom_fraction(span: f64, fit_span: f64) -> f64 {
    let range = (fit_span / SLIDER_MIN_SPAN).ln();
    if range <= 0.0 {
        return 0.0;
    }
    ((fit_span / span).ln() / range).clamp(0.0, 1.0)
}

fn span_from_zoom_fraction(fraction: f64, fit_span: f64) -> f64 {
    let fit_span = fit_span.max(SLIDER_MIN_SPAN);
    (fit_span * (SLIDER_MIN_SPAN / fit_span).powf(fraction.clamp(0.0, 1.0)))
        .clamp(MIN_SPAN, MAX_SPAN)
}

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
    grab_x: Rc<Cell<Pixels>>,
    /// Project seconds, for snapping the trailing edge.
    length: f64,
    view: TimelineView,
    recorded: Rc<Cell<bool>>,
}

#[derive(Clone)]
struct ClipTrimDrag {
    id: String,
    start: bool,
    initial_start: f64,
    initial_offset: u64,
    view: TimelineView,
    recorded: Rc<Cell<bool>>,
}

#[derive(Clone, Copy)]
struct PlayheadDrag;

#[derive(Clone)]
struct ScrollbarDrag {
    grab_x: Rc<Cell<Pixels>>,
    /// Seconds the whole scrollbar track stands for.
    total: f64,
    span: f64,
}

#[derive(Clone)]
struct ZoomDrag {
    fit_span: f64,
}

pub(super) struct MediaDragPreview;
impl Render for MediaDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(theme::ui_rem(1.0))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MenuTarget {
    AddTrack,
    Track(usize),
}

#[derive(Clone)]
pub(super) struct TimelineMenu {
    pub(super) target: MenuTarget,
    pub(super) focuses: Vec<FocusHandle>,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "The timeline renders from independent editing, history, and playback flags."
)]
pub(super) struct ProjectTimelineState {
    pub(super) scheme: material::ColorScheme,
    /// Every placed clip with its rendered source, in clip order.
    pub(super) clips: Vec<(ClipPlacement, TimelineSource)>,
    pub(super) tracks: Tracks,
    pub(super) files: Vec<FileItem>,
    pub(super) selected: Option<String>,
    pub(super) duration: f64,
    pub(super) current: f64,
    pub(super) view: TimelineView,
    pub(super) enabled: bool,
    pub(super) snap: bool,
    pub(super) can_undo: bool,
    pub(super) can_redo: bool,
    pub(super) playing: bool,
    pub(super) menu: Option<TimelineMenu>,
    /// The ruler's width from the last layout, to space its ticks.
    pub(super) ruler_width: Option<f32>,
}

impl ProjectTimelineState {
    fn selected_clip(&self) -> Option<&(ClipPlacement, TimelineSource)> {
        let selected = self.selected.as_deref()?;
        self.clips
            .iter()
            .find(|(placement, _)| placement.id == selected)
    }

    fn selected_editable(&self) -> bool {
        self.enabled
            && self
                .selected_clip()
                .is_some_and(|(placement, _)| !self.tracks[placement.track].locked)
    }

    /// Whether the playhead is far enough inside the selected clip to split it.
    fn can_split(&self) -> bool {
        self.selected_editable()
            && self.selected_clip().is_some_and(|(_, source)| {
                let (start, length) = clip_extent(source);
                self.current - start >= 0.05 && start + length - self.current >= 0.05
            })
    }

    /// Whether the playhead is far enough inside a clip on the selected clip's track to cut
    /// that clip to its beginning or end.
    fn can_cut_track(&self) -> bool {
        self.selected_editable()
            && self.selected_clip().is_some_and(|(selected, _)| {
                self.clips.iter().any(|(placement, source)| {
                    let (start, length) = clip_extent(source);
                    placement.track == selected.track
                        && self.current - start >= 0.05
                        && start + length - self.current >= 0.05
                })
            })
    }
}

/// The track header column, beside the ruler and every lane.
const HEADER_WIDTH: f32 = 184.0;
const RULER_HEIGHT: f32 = 28.0;
const VIDEO_LANE_HEIGHT: f32 = 72.0;
const AUDIO_LANE_HEIGHT: f32 = 48.0;
const LANE_GAP: f32 = 4.0;
/// Clips sit this far inside their lane.
const CLIP_INSET: f32 = 3.0;
const CLIP_HEADER_HEIGHT: f32 = 18.0;
/// Video clips end with a waveform band for the audio they export.
const VIDEO_WAVEFORM_HEIGHT: f32 = 14.0;
/// Audio clips fill the space under their header with the waveform.
const AUDIO_WAVEFORM_HEIGHT: f32 = AUDIO_LANE_HEIGHT - CLIP_HEADER_HEIGHT - 2.0 * CLIP_INSET;
const TRIM_HANDLE_WIDTH: f32 = 8.0;
/// Lanes scroll vertically past this height.
const MAX_LANES_HEIGHT: f32 = 248.0;
/// Room beside the scrollbar for each scroll arrow. The ruler and lanes leave the same room
/// on their right, so all three share their left edge and width.
const SCROLL_ARROW_WIDTH: f32 = 20.0;
/// Opacity of clips on hidden tracks.
const HIDDEN_CLIP_OPACITY: f32 = 0.38;

const fn lane_height(kind: TrackKind) -> f32 {
    match kind {
        TrackKind::Video => VIDEO_LANE_HEIGHT,
        TrackKind::Audio => AUDIO_LANE_HEIGHT,
    }
}

pub(super) fn project_timeline(
    state: &ProjectTimelineState,
    focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let scheme = &state.scheme;
    let order = state.tracks.display_order();
    m3::m3_surface(scheme.surface_container_low, shape::CORNER_EXTRA_LARGE)
        .id("project-timeline")
        .track_focus(focus)
        .tab_stop(false)
        .role(gpui::Role::Group)
        .aria_label("Project timeline")
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
        .flex_none()
        .min_w_0()
        .w_full()
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
        // Drops outside a lane land on the default track.
        .on_drop(cx.listener(|root, source: &LibrarySourceDrag, window, cx| {
            cx.stop_propagation();
            if root.drop_library_source(&source.0, window.mouse_position()) {
                cx.notify();
            }
        }))
        .child(toolbar(state, window, cx))
        .child(
            div()
                .flex()
                .items_end()
                .pr(theme::ui_rem(SCROLL_ARROW_WIDTH))
                .child(
                    div()
                        .w(theme::ui_rem(HEADER_WIDTH))
                        .flex_none()
                        .child(add_track_button(state, window, cx)),
                )
                .child(ruler(state, cx)),
        )
        .child(
            div()
                .id("project-timeline-lanes")
                .max_h(theme::ui_rem(MAX_LANES_HEIGHT))
                .overflow_y_scroll()
                .flex()
                .pr(theme::ui_rem(SCROLL_ARROW_WIDTH))
                .child(
                    div()
                        .w(theme::ui_rem(HEADER_WIDTH))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(theme::ui_rem(LANE_GAP))
                        .children(
                            order
                                .iter()
                                .map(|&index| track_header(state, index, window, cx)),
                        ),
                )
                .child(lanes(state, &order, window, cx)),
        )
        .child(scrollbar(state, cx))
        .child(status_line(state))
}

#[expect(
    clippy::too_many_lines,
    reason = "The toolbar lays out transport, history, edit, snap, and zoom controls."
)]
fn toolbar(
    state: &ProjectTimelineState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = &state.scheme;
    let fit_span = state.duration.max(1.0);
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(theme::ui_rem(8.0))
        .child(
            m3::IconButton::new(
                "timeline-play",
                if state.playing {
                    Symbol::Pause
                } else {
                    Symbol::PlayArrow
                },
                if state.playing { "Pause" } else { "Play" },
            )
            .variant(m3::IconButtonVariant::Filled)
            .enabled(!state.clips.is_empty())
            .build(scheme, window, cx)
            .on_click(cx.listener(|root, _, _, cx| {
                if root.toggle_preview_playback_with_context(cx) {
                    cx.notify();
                }
            })),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .child(
                    div()
                        .m3_type(typography::TITLE_SMALL)
                        .child(format_timecode(state.current)),
                )
                .child(
                    div()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .child(format!(
                            "of {} · {} tracks",
                            format_timecode(state.duration),
                            state.tracks.len()
                        )),
                ),
        )
        .child(
            div()
                .flex_none()
                .w(theme::ui_rem(1.0))
                .h(theme::ui_rem(24.0))
                .mx(theme::ui_rem(8.0))
                .bg(color(scheme.outline_variant)),
        )
        .child(
            m3::IconButton::new("timeline-undo", Symbol::Undo, "Undo")
                .size(m3::IconButtonSize::ExtraSmall)
                .enabled(state.enabled && state.can_undo)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    if root.undo_timeline_edit() {
                        cx.notify();
                    }
                })),
        )
        .child(
            m3::IconButton::new("timeline-redo", Symbol::Redo, "Redo")
                .size(m3::IconButtonSize::ExtraSmall)
                .enabled(state.enabled && state.can_redo)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    if root.redo_timeline_edit() {
                        cx.notify();
                    }
                })),
        )
        .child(
            m3::Button::new(
                "timeline-split",
                "Split at playhead",
                m3::ButtonVariant::Tonal,
            )
            .icon(Symbol::ContentCut)
            .size(m3::ButtonSize::ExtraSmall)
            .enabled(state.can_split())
            .build(scheme, window, cx)
            .on_click(cx.listener(|root, _, _, cx| {
                if root.split_project_clip_at_playhead() {
                    cx.notify();
                }
            })),
        )
        .child(
            m3::IconButton::new(
                "timeline-cut-to-beginning",
                Symbol::FirstPage,
                "Cut to beginning",
            )
            .size(m3::IconButtonSize::ExtraSmall)
            .enabled(state.can_cut_track())
            .build(scheme, window, cx)
            .on_click(cx.listener(|root, _, _, cx| {
                if root.cut_project_track_at_playhead(true) {
                    cx.notify();
                }
            })),
        )
        .child(
            m3::IconButton::new("timeline-cut-to-end", Symbol::LastPage, "Cut to end")
                .size(m3::IconButtonSize::ExtraSmall)
                .enabled(state.can_cut_track())
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    if root.cut_project_track_at_playhead(false) {
                        cx.notify();
                    }
                })),
        )
        .child(
            m3::IconButton::new("timeline-delete", Symbol::Delete, "Delete clip")
                .size(m3::IconButtonSize::ExtraSmall)
                .enabled(state.selected_editable())
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    if root.remove_selected_project_clip() {
                        cx.notify();
                    }
                })),
        )
        .child(
            m3::Chip::new("timeline-snap", "Snap", m3::ChipKind::Filter)
                .selected(state.snap)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    root.preview_ui.timeline_snap = !root.preview_ui.timeline_snap;
                    cx.notify();
                })),
        )
        // Zoom controls keep to the right, and wrap below on narrow panels.
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_end()
                .gap(theme::ui_rem(4.0))
                .child(
                    m3::IconButton::new("timeline-zoom-out", Symbol::ZoomOut, "Zoom out timeline")
                        .size(m3::IconButtonSize::ExtraSmall)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, _, cx| {
                            root.zoom_project_timeline(0.8);
                            cx.notify();
                        })),
                )
                .child(zoom_slider(state, fit_span, cx))
                .child(
                    m3::IconButton::new("timeline-zoom-in", Symbol::ZoomIn, "Zoom in timeline")
                        .size(m3::IconButtonSize::ExtraSmall)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, _, cx| {
                            root.zoom_project_timeline(1.25);
                            cx.notify();
                        })),
                )
                .child(
                    m3::Button::new("timeline-fit", "Fit", m3::ButtonVariant::Text)
                        .icon(Symbol::FitScreen)
                        .size(m3::ButtonSize::ExtraSmall)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, _, cx| {
                            root.preview_ui.project_timeline_view =
                                Some(TimelineView::fit(root.project_duration()));
                            cx.notify();
                        })),
                ),
        )
}

fn zoom_slider(
    state: &ProjectTimelineState,
    fit_span: f64,
    cx: &Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let fraction = zoom_fraction(state.view.span, fit_span);
    let step = |delta: f64| {
        move |root: &mut FrameRoot,
              _: &gpui::KeyDownEvent,
              _: &mut Window,
              cx: &mut Context<FrameRoot>| {
            let span = span_from_zoom_fraction(fraction + delta, fit_span);
            let mut view = root.project_timeline_view();
            view.span = span;
            root.preview_ui.project_timeline_view = Some(view);
            cx.stop_propagation();
            cx.notify();
        }
    };
    let (decrease, increase) = (step(-0.1), step(0.1));
    m3::Slider::new("timeline-zoom", "Timeline zoom", fraction, 0.0, 1.0)
        .value_text(format!("{:.0}%", fraction * 100.0))
        .build(&state.scheme)
        .flex_none()
        .w(theme::ui_rem(128.0))
        .on_key_down(
            cx.listener(move |root, event: &gpui::KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "left" | "down" => decrease(root, event, window, cx),
                    "right" | "up" => increase(root, event, window, cx),
                    _ => {}
                }
            }),
        )
        .on_drag(ZoomDrag { fit_span }, |_, _, _, cx| {
            cx.new(|_| MediaDragPreview)
        })
        .on_drag_move(cx.listener(|root, event: &DragMoveEvent<ZoomDrag>, _, cx| {
            let fit_span = event.drag(cx).fit_span;
            let fraction = timeline_slider_percent_from_bounds(event.event.position, event.bounds);
            let mut view = root.project_timeline_view();
            view.span = span_from_zoom_fraction(fraction, fit_span);
            root.preview_ui.project_timeline_view = Some(view);
            cx.notify();
        }))
}

fn timeline_menu(
    state: &ProjectTimelineState,
    target: MenuTarget,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> Option<gpui::Div> {
    let menu = state.menu.as_ref().filter(|menu| menu.target == target)?;
    let items = match target {
        MenuTarget::AddTrack => vec![
            m3::MenuItem::new("Video track", Symbol::Movie).enabled(state.enabled),
            m3::MenuItem::new("Audio track", Symbol::MusicNote).enabled(state.enabled),
        ],
        MenuTarget::Track(index) => {
            let track = state.tracks[index];
            vec![
                m3::MenuItem::new("Delete track", Symbol::Delete)
                    .enabled(state.enabled && !track.locked && state.tracks.count(track.kind) > 1),
            ]
        }
    };
    let items = items
        .into_iter()
        .zip(&menu.focuses)
        .map(|(item, focus)| item.focus(focus))
        .collect();
    let id = match target {
        MenuTarget::AddTrack => "timeline-add-track-menu".to_string(),
        MenuTarget::Track(index) => format!("timeline-track-menu-{index}"),
    };
    let menu = m3::Menu::new(id, items)
        .on_select(cx.listener(move |root, index: &usize, _, cx| {
            root.preview_ui.timeline_menu = None;
            match target {
                MenuTarget::AddTrack => {
                    root.add_project_track(if *index == 0 {
                        TrackKind::Video
                    } else {
                        TrackKind::Audio
                    });
                }
                MenuTarget::Track(track) => {
                    root.remove_project_track(track);
                }
            }
            cx.notify();
        }))
        .build(&state.scheme, window, cx)
        .shadow(m3::elevation_shadows(
            material::Elevation::Level2,
            &state.scheme,
        ))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down_out(cx.listener(|root, _, _, cx| {
            root.preview_ui.timeline_menu = None;
            cx.notify();
        }))
        .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "escape" | "tab") {
                cx.stop_propagation();
                root.preview_ui.timeline_menu = None;
                cx.notify();
            }
        }));
    // A zero-size anchor keeps the menu out of the trigger's layout.
    Some(
        div()
            .absolute()
            .size_0()
            .left_0()
            .top(theme::ui_rem(36.0))
            .child(
                gpui::deferred(
                    gpui::anchored()
                        .anchor(gpui::Anchor::TopLeft)
                        .snap_to_window()
                        .child(menu),
                )
                .with_priority(20),
            ),
    )
}

fn add_track_button(
    state: &ProjectTimelineState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    div()
        .relative()
        .child(
            m3::Button::new("timeline-add-track", "Add track", m3::ButtonVariant::Text)
                .icon(Symbol::Add)
                .size(m3::ButtonSize::ExtraSmall)
                .enabled(state.enabled)
                .build(&state.scheme, window, cx)
                .on_click(cx.listener(|root, _, window, cx| {
                    root.toggle_timeline_menu(MenuTarget::AddTrack, window, cx);
                    cx.notify();
                })),
        )
        .children(timeline_menu(state, MenuTarget::AddTrack, window, cx))
}

fn ruler(state: &ProjectTimelineState, cx: &Context<FrameRoot>) -> gpui::Stateful<gpui::Div> {
    let scheme = &state.scheme;
    let view = state.view;
    let step = major_tick_step(view.span, state.ruler_width.unwrap_or(800.0));
    div()
        .id("project-ruler")
        .focusable()
        .tab_stop(true)
        .role(gpui::Role::Slider)
        .aria_label("Project playhead")
        .aria_numeric_value(state.current)
        .aria_min_numeric_value(0.0)
        .aria_max_numeric_value(state.duration.max(1.0))
        .aria_value(format_timecode(state.current))
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
        .flex_1()
        .min_w_0()
        .h(theme::ui_rem(RULER_HEIGHT))
        .relative()
        .overflow_hidden()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|root, event: &MouseDownEvent, _, cx| {
                if root.seek_project_timeline(event.position, None, cx) {
                    cx.notify();
                }
            }),
        )
        .on_drag(PlayheadDrag, |_, _, _, cx| cx.new(|_| MediaDragPreview))
        .on_drag_move(
            cx.listener(|root, event: &DragMoveEvent<PlayheadDrag>, _, cx| {
                if root.seek_project_timeline(event.event.position, None, cx) {
                    cx.notify();
                }
            }),
        )
        .child(
            super::preview_panel::timeline::PreviewTimelineTrackBoundsProbe { owner: cx.entity() },
        )
        .children(ruler_ticks(view, step).into_iter().map(|(time, major)| {
            div()
                .absolute()
                .left(relative(view.fraction(time)))
                .bottom_0()
                .h(theme::ui_rem(if major { 12.0 } else { 5.0 }))
                .border_l_1()
                .border_color(color(if major {
                    scheme.outline
                } else {
                    scheme.outline_variant
                }))
                .when(major, |tick| {
                    tick.child(
                        div()
                            .absolute()
                            .bottom(theme::ui_rem(2.0))
                            .left(theme::ui_rem(3.0))
                            .whitespace_nowrap()
                            .m3_type(typography::LABEL_SMALL)
                            .text_color(color(scheme.on_surface_variant))
                            .child(format_ruler_time(time, step)),
                    )
                })
        }))
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(view.fraction(state.current)))
                .w(theme::ui_rem(2.0))
                .ml(theme::ui_rem(-1.0))
                .bg(color(scheme.primary))
                .child(
                    div()
                        .absolute()
                        .top(theme::ui_rem(4.0))
                        .left(theme::ui_rem(-5.0))
                        .w(theme::ui_rem(12.0))
                        .h(theme::ui_rem(16.0))
                        .rounded(theme::ui_rem(shape::CORNER_EXTRA_SMALL))
                        .bg(color(scheme.primary)),
                ),
        )
}

fn track_header(
    state: &ProjectTimelineState,
    index: usize,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = &state.scheme;
    let track = state.tracks[index];
    let name = state.tracks.name(index);
    let (badge, on_badge) = match track.kind {
        TrackKind::Video => (scheme.primary_container, scheme.on_primary_container),
        TrackKind::Audio => (scheme.tertiary_container, scheme.on_tertiary_container),
    };
    let mut toggle = |id: &str, symbol: Symbol, label: String, value: TrackToggle| {
        m3::IconButton::new(format!("track-{id}-{index}"), symbol, label)
            .size(m3::IconButtonSize::ExtraSmall)
            .enabled(state.enabled)
            .build(scheme, window, cx)
            .on_click(cx.listener(move |root, _, _, cx| {
                if root.toggle_project_track(index, value) {
                    cx.notify();
                }
            }))
    };
    let visibility = (track.kind == TrackKind::Video).then(|| {
        toggle(
            "visibility",
            if track.hidden {
                Symbol::VisibilityOff
            } else {
                Symbol::Visibility
            },
            format!("{} {name}", if track.hidden { "Show" } else { "Hide" }),
            TrackToggle::Hidden,
        )
    });
    let mute = toggle(
        "mute",
        if track.muted {
            Symbol::VolumeOff
        } else {
            Symbol::VolumeUp
        },
        format!("{} {name}", if track.muted { "Unmute" } else { "Mute" }),
        TrackToggle::Muted,
    );
    let lock = toggle(
        "lock",
        if track.locked {
            Symbol::Lock
        } else {
            Symbol::LockOpen
        },
        format!("{} {name}", if track.locked { "Unlock" } else { "Lock" }),
        TrackToggle::Locked,
    );
    let more = m3::IconButton::new(
        format!("track-more-{index}"),
        Symbol::MoreVert,
        format!("{name} options"),
    )
    .size(m3::IconButtonSize::ExtraSmall)
    .enabled(state.enabled)
    .build(scheme, window, cx)
    .on_click(cx.listener(move |root, _, window, cx| {
        root.toggle_timeline_menu(MenuTarget::Track(index), window, cx);
        cx.notify();
    }));
    div()
        .h(theme::ui_rem(lane_height(track.kind)))
        .flex()
        .items_center()
        .gap(theme::ui_rem(2.0))
        .pl(theme::ui_rem(8.0))
        .rounded_l(theme::ui_rem(shape::CORNER_SMALL))
        .bg(color(scheme.surface_container))
        .child(
            div()
                .flex_none()
                .min_w(theme::ui_rem(28.0))
                .mr(theme::ui_rem(6.0))
                .px(theme::ui_rem(4.0))
                .rounded(theme::ui_rem(shape::CORNER_EXTRA_SMALL))
                .bg(color(badge))
                .text_color(color(on_badge))
                .m3_type(typography::LABEL_MEDIUM)
                .flex()
                .justify_center()
                .child(state.tracks.label(index)),
        )
        .children(visibility)
        .child(mute)
        .child(lock)
        .child(div().relative().child(more).children(timeline_menu(
            state,
            MenuTarget::Track(index),
            window,
            cx,
        )))
}

fn lanes(
    state: &ProjectTimelineState,
    order: &[usize],
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id("project-lanes")
        .flex_1()
        .min_w_0()
        .relative()
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(LANE_GAP))
        .on_drag_move(cx.listener(|root, event: &DragMoveEvent<ClipDrag>, _, cx| {
            let drag = event.drag(cx).clone();
            if root.drag_project_clip(&drag, event.event.position, event.bounds) {
                cx.notify();
            }
        }))
        .on_drag_move(
            cx.listener(|root, event: &DragMoveEvent<ClipTrimDrag>, _, cx| {
                let drag = event.drag(cx).clone();
                if root.drag_project_trim(&drag, event.event.position, event.bounds) {
                    cx.notify();
                }
            }),
        )
        .children(order.iter().map(|&index| lane(state, index, window, cx)))
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .w(theme::ui_rem(2.0))
                .ml(theme::ui_rem(-1.0))
                .left(relative(state.view.fraction(state.current)))
                .bg(color(state.scheme.primary)),
        )
}

fn lane(
    state: &ProjectTimelineState,
    index: usize,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let scheme = &state.scheme;
    let track = state.tracks[index];
    let label = state.tracks.label(index);
    let highlight = scheme.surface_container_high;
    let enabled = state.enabled && !track.locked;
    div()
        .id(format!("project-lane-{index}"))
        .debug_selector(move || format!("project-lane-{label}"))
        .aria_label(format!("{} lane", state.tracks.name(index)))
        .h(theme::ui_rem(lane_height(track.kind)))
        .flex_none()
        .relative()
        .overflow_hidden()
        .rounded_r(theme::ui_rem(shape::CORNER_SMALL))
        .bg(color(scheme.surface_container))
        .cursor_pointer()
        .on_click(cx.listener(move |root, event: &gpui::ClickEvent, _, cx| {
            if let gpui::ClickEvent::Mouse(event) = event
                && root.seek_project_timeline(event.up.position, Some(index), cx)
            {
                cx.notify();
            }
        }))
        .when(enabled, |lane| {
            lane.drag_over::<LibrarySourceDrag>(move |style, _, _, _| style.bg(color(highlight)))
                .on_drop(
                    cx.listener(move |root, source: &LibrarySourceDrag, window, cx| {
                        cx.stop_propagation();
                        if root.drop_library_source_on(
                            &source.0,
                            window.mouse_position(),
                            Some(index),
                        ) {
                            cx.notify();
                        }
                    }),
                )
                // Dragging a clip over another lane of its kind moves it there.
                .on_drag_move(
                    cx.listener(move |root, event: &DragMoveEvent<ClipDrag>, _, cx| {
                        let y = event.event.position.y;
                        if y < event.bounds.top() || y >= event.bounds.bottom() {
                            return;
                        }
                        let drag = event.drag(cx).clone();
                        if root.gesture_edit(&drag.recorded, |root| {
                            root.move_project_clip_to_track(&drag.id, index)
                        }) {
                            cx.notify();
                        }
                    }),
                )
        })
        .children(
            state
                .clips
                .iter()
                .filter(|(placement, _)| placement.track == index)
                .filter_map(|(placement, source)| {
                    let file = state.files.iter().find(|file| file.id == placement.id)?;
                    Some(clip(state, placement, source, file, window, cx))
                }),
        )
}

#[expect(
    clippy::too_many_lines,
    reason = "A clip composes its header, filmstrip, waveform, and independent trim handles."
)]
fn clip(
    state: &ProjectTimelineState,
    placement: &ClipPlacement,
    source: &TimelineSource,
    file: &FileItem,
    _window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let scheme = &state.scheme;
    let view = state.view;
    let track = state.tracks[placement.track];
    let selected = state.selected.as_deref() == Some(&placement.id);
    let enabled = state.enabled && !track.locked;
    let id = placement.id.clone();
    let (start, length) = clip_extent(source);
    let video = placement.kind == SourceKind::Video;
    let (container, on_container) = match placement.kind {
        SourceKind::Audio => (scheme.tertiary_container, scheme.on_tertiary_container),
        SourceKind::Video | SourceKind::Image => {
            (scheme.primary_container, scheme.on_primary_container)
        }
    };
    let symbol = match placement.kind {
        SourceKind::Audio => Symbol::MusicNote,
        SourceKind::Image => Symbol::Image,
        SourceKind::Video => Symbol::Movie,
    };
    let source_name = file.name.clone();
    let source_duration = std::time::Duration::from_millis(source.duration_millis).as_secs_f64();
    let filmstrip = video
        .then(|| super::filmstrip::request(&source.path, source_duration, cx))
        .flatten();
    // Audio sources fall back to their first stream. A video shows the audio it exports,
    // so one without a selected track has no waveform band.
    let waveform_stream = match placement.kind {
        SourceKind::Audio => Some(source.config.selected_audio_tracks.first().copied()),
        SourceKind::Video => source
            .config
            .selected_audio_tracks
            .first()
            .copied()
            .map(Some),
        SourceKind::Image => None,
    };
    let waveform = waveform_stream
        .and_then(|stream| super::waveform::request(&source.path, stream, source_duration, cx));
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
        .unwrap_or(source_duration);
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
    let waveform_height = if video {
        VIDEO_WAVEFORM_HEIGHT
    } else {
        AUDIO_WAVEFORM_HEIGHT
    };
    let grab = Rc::new(Cell::new(px(0.0)));
    let debug_id = id.clone();

    div()
        .id(format!("project-clip-{id}"))
        .debug_selector(move || format!("project-clip-{debug_id}"))
        .absolute()
        .top(theme::ui_rem(CLIP_INSET))
        .bottom(theme::ui_rem(CLIP_INSET))
        .left(relative(view.fraction(start)))
        .w(relative(crate::numeric::f64_to_f32(length / view.span)))
        .min_w(theme::ui_rem(8.0))
        .rounded(theme::ui_rem(shape::CORNER_SMALL))
        .overflow_hidden()
        .bg(color(if video {
            scheme.surface_container_highest
        } else {
            container
        }))
        .when(selected, |clip| {
            clip.border_2().border_color(color(scheme.primary))
        })
        .when(track.hidden, |clip| clip.opacity(HIDDEN_CLIP_OPACITY))
        .text_color(color(on_container))
        .role(gpui::Role::Button)
        .aria_label(format!("Select {source_name} clip"))
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
                let key = event.keystroke.key.as_str();
                if matches!(key, "enter" | "space") {
                    root.select_project_clip(&id);
                    cx.stop_propagation();
                    cx.notify();
                }
                if matches!(key, "left" | "right") {
                    let offset = root
                        .project
                        .clips
                        .iter()
                        .find(|clip| clip.id == id)
                        .map_or(0, |clip| clip.offset_millis);
                    let offset = if key == "left" {
                        offset.saturating_sub(100)
                    } else {
                        offset.saturating_add(100)
                    };
                    root.gesture_edit(&Cell::new(false), |root| {
                        root.move_project_clip(&id, offset)
                    });
                    cx.stop_propagation();
                    cx.notify();
                }
            }
        }))
        .when(enabled, |clip| {
            clip.cursor_grab().on_drag(
                ClipDrag {
                    id: id.clone(),
                    grab_x: grab.clone(),
                    length,
                    view,
                    recorded: Rc::default(),
                },
                move |drag, position, _, cx| {
                    drag.grab_x.set(position.x);
                    cx.new(|_| MediaDragPreview)
                },
            )
        })
        .when_some(filmstrip, |clip, filmstrip| {
            clip.child(
                super::filmstrip::element(filmstrip, ranges.clone())
                    .top(theme::ui_rem(CLIP_HEADER_HEIGHT))
                    .bottom(theme::ui_rem(if waveform_stream.is_some() {
                        VIDEO_WAVEFORM_HEIGHT
                    } else {
                        0.0
                    })),
            )
        })
        .when(video && waveform_stream.is_some(), |clip| {
            clip.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(theme::ui_rem(VIDEO_WAVEFORM_HEIGHT))
                    .bg(color(container)),
            )
        })
        .when_some(waveform, |clip, waveform| {
            clip.child(
                super::waveform::element(waveform, ranges, color(on_container))
                    .bottom(theme::ui_rem(2.0))
                    .h(theme::ui_rem(waveform_height - 2.0)),
            )
        })
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(theme::ui_rem(CLIP_HEADER_HEIGHT))
                .px(theme::ui_rem(6.0))
                .flex()
                .items_center()
                .gap(theme::ui_rem(4.0))
                .overflow_hidden()
                .bg(color(container))
                .m3_type(typography::LABEL_SMALL)
                .child(m3::m3_icon(symbol, false, 12.0, color(on_container)))
                .child(div().flex_none().truncate().child(source_name))
                .child(
                    div()
                        .flex_none()
                        .opacity(0.72)
                        .child(format_timecode(length)),
                ),
        )
        .children([true, false].into_iter().map(|leading| {
            div()
                .id(format!("project-trim-{id}-{leading}"))
                .absolute()
                .top_0()
                .bottom_0()
                .w(theme::ui_rem(TRIM_HANDLE_WIDTH))
                .when(leading, gpui::Styled::left_0)
                .when(!leading, gpui::Styled::right_0)
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |handle| {
                    handle.child(
                        div()
                            .w(theme::ui_rem(2.0))
                            .h(theme::ui_rem(20.0))
                            .rounded(theme::ui_rem(1.0))
                            .bg(color(scheme.primary)),
                    )
                })
                .on_click(|_, _, cx| cx.stop_propagation())
                .when(enabled, |handle| {
                    handle.cursor_ew_resize().on_drag(
                        ClipTrimDrag {
                            id: id.clone(),
                            start: leading,
                            view,
                            initial_offset: placement.offset_millis,
                            initial_start: file
                                .config
                                .start_time
                                .as_deref()
                                .map_or(0.0, parse_time_to_seconds),
                            recorded: Rc::default(),
                        },
                        |_, _, _, cx| cx.new(|_| MediaDragPreview),
                    )
                })
        }))
}

/// A horizontal scrollbar over the project plus some room after it. The track lines up with
/// the ruler, so clicks reuse the ruler's bounds.
#[expect(
    clippy::too_many_lines,
    reason = "The scrollbar handles arrows, paging clicks, and thumb dragging."
)]
fn scrollbar(state: &ProjectTimelineState, cx: &Context<FrameRoot>) -> gpui::Div {
    let scheme = &state.scheme;
    let view = state.view;
    let total = (state.duration.max(1.0) * FIRST_CLIP_TRACK_SPAN).max(view.start + view.span);
    let thumb_start = crate::numeric::f64_to_f32(view.start / total);
    let thumb_width = crate::numeric::f64_to_f32(view.span / total);
    let grab = Rc::new(Cell::new(px(0.0)));
    let arrow = |id: &'static str, symbol: Symbol, label: &'static str, direction: f64| {
        div()
            .id(id)
            .flex_none()
            .w(theme::ui_rem(SCROLL_ARROW_WIDTH))
            .flex()
            .justify_center()
            .role(gpui::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .child(m3::m3_icon(
                symbol,
                false,
                16.0,
                color(scheme.on_surface_variant),
            ))
            .on_click(cx.listener(move |root, _, _, cx| {
                let view = root.project_timeline_view();
                root.pan_project_timeline((direction * 0.1).mul_add(view.span, view.start));
                cx.notify();
            }))
    };
    div()
        .flex()
        .items_center()
        .child(
            div()
                .w(theme::ui_rem(HEADER_WIDTH))
                .flex_none()
                .flex()
                .justify_end()
                .child(arrow(
                    "timeline-scroll-left",
                    Symbol::ArrowLeft,
                    "Scroll timeline left",
                    -1.0,
                )),
        )
        .child(
            div()
                .id("timeline-scrollbar")
                .flex_1()
                .min_w_0()
                .h(theme::ui_rem(8.0))
                .relative()
                .rounded(theme::ui_rem(shape::CORNER_FULL))
                .bg(color(scheme.surface_container_highest))
                .on_click(cx.listener(move |root, event: &gpui::ClickEvent, _, cx| {
                    if let gpui::ClickEvent::Mouse(event) = event
                        && let Some(bounds) = root.preview_ui.timeline_track_bounds
                    {
                        let fraction =
                            timeline_slider_percent_from_bounds(event.up.position, bounds);
                        let view = root.project_timeline_view();
                        root.pan_project_timeline(fraction.mul_add(total, -view.span / 2.0));
                        cx.notify();
                    }
                }))
                .on_drag_move(
                    cx.listener(|root, event: &DragMoveEvent<ScrollbarDrag>, _, cx| {
                        let drag = event.drag(cx).clone();
                        let width = f64::from(event.bounds.size.width.as_f32());
                        if width <= 0.0 {
                            return;
                        }
                        let x = event.event.position.x - event.bounds.origin.x - drag.grab_x.get();
                        let start = f64::from(x.as_f32()) / width * drag.total;
                        root.pan_project_timeline(start.min(drag.total - drag.span));
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("timeline-scrollbar-thumb")
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(thumb_start))
                        .w(relative(thumb_width))
                        .min_w(theme::ui_rem(24.0))
                        .rounded(theme::ui_rem(shape::CORNER_FULL))
                        .bg(color(scheme.outline))
                        .cursor_grab()
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .on_drag(
                            ScrollbarDrag {
                                grab_x: grab,
                                total,
                                span: view.span,
                            },
                            |drag, position, _, cx| {
                                drag.grab_x.set(position.x);
                                cx.new(|_| MediaDragPreview)
                            },
                        ),
                ),
        )
        .child(arrow(
            "timeline-scroll-right",
            Symbol::ArrowRight,
            "Scroll timeline right",
            1.0,
        ))
}

fn status_line(state: &ProjectTimelineState) -> gpui::Div {
    let text = state.selected_clip().map_or_else(
        || {
            if state.clips.is_empty() {
                "Drag media from the library onto a track".to_string()
            } else {
                format!("{} clips", state.clips.len())
            }
        },
        |(placement, source)| {
            let (start, length) = clip_extent(source);
            let name = state
                .files
                .iter()
                .find(|file| file.id == placement.id)
                .map_or("", |file| file.name.as_str());
            format!(
                "{name} · {} · {} – {} · {}",
                state.tracks.name(placement.track),
                format_timecode(start),
                format_timecode(start + length),
                format_timecode(length),
            )
        },
    );
    div()
        .min_w_0()
        .truncate()
        .m3_type(typography::BODY_SMALL)
        .text_color(color(state.scheme.on_surface_variant))
        .child(text)
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
                let seek = root.ensure_focus(
                    super::super::FrameFocusKey::Control(
                        super::super::hotkeys::PREVIEW_SEEK_FOCUS_ID.to_string(),
                    ),
                    true,
                    cx,
                );
                let focus = self.focus.clone();
                div()
                    .track_focus(&focus)
                    .child(project_timeline(&state.timeline, &seek, window, cx))
                    .on_key_down(cx.listener(move |root, event, window, cx| {
                        root.handle_preview_shortcut(event, &focus, window, cx);
                    }))
            })
        }
    }

    #[test]
    fn x_splits_after_clicking_a_clip_body() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|window, cx| {
            window.activate_window();
            let root = cx.new(|_| {
                let mut root = crate::app::project::tests::project();
                root.active_view = ActiveView::Workspace;
                root.add_library_clip("source");
                root.preview_ui.playback =
                    preview_playback_state(PreviewMediaKind::Video, 10.0, None, None);
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
        cx.simulate_click(
            point(
                bounds.left() + bounds.size.width * 0.25,
                bounds.bottom() + px(28.0),
            ),
            Modifiers::default(),
        );
        cx.simulate_keystrokes("x");
        root.read_with(cx, |root, _| {
            assert_eq!(root.project.clips.len(), 2);
            assert_eq!(root.project.clips[1].offset_millis, 2_750);
        });
    }

    #[test]
    fn q_and_e_cut_the_clip_to_its_beginning_and_end_after_clicking_it() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|window, cx| {
            window.activate_window();
            let root = cx.new(|_| {
                let mut root = crate::app::project::tests::project();
                root.active_view = ActiveView::Workspace;
                root.add_library_clip("source");
                root.preview_ui.playback =
                    preview_playback_state(PreviewMediaKind::Video, 10.0, None, None);
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
        let click = |cx: &mut gpui::VisualTestContext, fraction: f32| {
            cx.simulate_click(
                point(
                    bounds.left() + bounds.size.width * fraction,
                    bounds.bottom() + px(28.0),
                ),
                Modifiers::default(),
            );
        };
        click(cx, 0.25);
        cx.simulate_keystrokes("q");
        click(cx, 0.75);
        cx.simulate_keystrokes("e");
        root.read_with(cx, |root, _| {
            assert_eq!(root.project.clips.len(), 1);
            assert_eq!(root.project.clips[0].offset_millis, 2_750);
            let config = &root.file_queue.files()[0].config;
            assert_eq!(config.start_time.as_deref(), Some("00:00:02.750"));
            assert_eq!(config.end_time.as_deref(), Some("00:00:08.250"));
        });
    }

    fn lane_center(cx: &mut gpui::VisualTestContext, selector: &'static str) -> Pixels {
        cx.debug_bounds(selector).unwrap().center().y
    }

    #[test]
    fn tracks_remain_after_their_last_clips_are_removed() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                root
            });
            TimelineTestView {
                root,
                focus: cx.focus_handle(),
            }
        });
        let root = view.read_with(cx, |view, _| view.root.clone());
        cx.update(|window, cx| window.draw(cx).clear());
        let ids: Vec<_> = root.read_with(cx, |root, _| {
            root.project
                .clips
                .iter()
                .map(|clip| clip.id.clone())
                .collect()
        });
        root.update(cx, |root, _| {
            for id in &ids {
                assert!(root.remove_project_clip(id));
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("project-lane-V1").is_some());
        assert!(cx.debug_bounds("project-lane-A1").is_some());
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
    fn snapping_picks_the_closest_target_within_the_threshold() {
        // The trailing edge (9.8) is closer to 10 than the leading edge (2.3) is to 2.
        assert!((snap_shift(&[2.3, 9.8], &[2.0, 10.0], 0.5) - 0.2).abs() < 1e-9);
        assert!((snap_shift(&[2.3], &[3.0], 0.5)).abs() < f64::EPSILON);
        assert!((snap_shift(&[5.0], &[], 0.5)).abs() < f64::EPSILON);
    }

    #[test]
    fn ruler_steps_keep_labels_apart_and_ticks_inside_the_view() {
        assert!((major_tick_step(6840.0, 1700.0) - 600.0).abs() < f64::EPSILON);
        assert!((major_tick_step(1.0, 1000.0) - 0.1).abs() < f64::EPSILON);
        assert!((major_tick_step(1e9, 100.0) - 43200.0).abs() < f64::EPSILON);
        let ticks = ruler_ticks(
            TimelineView {
                start: 3.0,
                span: 20.0,
            },
            10.0,
        );
        assert_eq!(ticks.first(), Some(&(4.0, false)));
        assert!(ticks.contains(&(10.0, true)));
        assert!(ticks.contains(&(20.0, true)));
        assert_eq!(ticks.iter().filter(|(_, major)| *major).count(), 2);
        assert!(ticks.iter().all(|(time, _)| (3.0..=23.0).contains(time)));
    }

    #[test]
    fn timecodes_show_hours_and_tenths() {
        assert_eq!(format_timecode(993.0), "0:16:33.0");
        assert_eq!(format_timecode(6837.29), "1:53:57.3");
        assert_eq!(format_timecode(-1.0), "0:00:00.0");
        assert_eq!(format_ruler_time(600.0, 600.0), "0:10:00");
        assert_eq!(format_ruler_time(1.5, 0.5), "0:00:01.5");
    }

    #[test]
    fn zoom_slider_maps_the_fitted_view_to_zero_and_round_trips() {
        assert!(zoom_fraction(100.0, 100.0).abs() < f64::EPSILON);
        assert!((zoom_fraction(SLIDER_MIN_SPAN, 100.0) - 1.0).abs() < f64::EPSILON);
        for fraction in [0.0, 0.3, 1.0] {
            let span = span_from_zoom_fraction(fraction, 100.0);
            assert!((zoom_fraction(span, 100.0) - fraction).abs() < 1e-9);
        }
        // A project shorter than the slider's range can't zoom further.
        assert!(zoom_fraction(0.5, 0.5).abs() < f64::EPSILON);
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
                    preview_playback_state(PreviewMediaKind::Video, 6.0, None, None);
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
        let video = lane_center(cx, "project-lane-V1");
        let audio = lane_center(cx, "project-lane-A1");
        // The ruler, the first clip's body, and the empty end of the audio lane.
        for (fraction, y) in [(0.75, bounds.center().y), (0.25, video), (0.75, audio)] {
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
    fn ruler_selects_first_available_video_at_playhead_and_lane_clicks_override_it() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = crate::app::project::tests::project();
                assert!(root.add_library_clip_on_track("source", 0, 0));
                assert!(root.add_project_track(TrackKind::Video));
                assert!(root.add_library_clip_on_track("source", 0, 2));
                assert!(root.add_library_clip_on_track("source", 10_000, 2));
                assert!(root.add_library_clip_on_track("source", 20_000, 0));
                // An empty track above both populated tracks must also be skipped.
                assert!(root.add_project_track(TrackKind::Video));
                let lower = root.project.clips[0].id.clone();
                assert!(root.select_project_clip(&lower));
                root.preview_ui.playback =
                    preview_playback_state(PreviewMediaKind::Video, 30.0, None, None);
                root.preview_ui.project_timeline_view = Some(TimelineView::fit(30.0));
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
        let lower = lane_center(cx, "project-lane-V1");
        let upper = lane_center(cx, "project-lane-V2");
        // Selecting a different lane is explicit; the next ruler click selects the
        // top video clip again. At the shared boundary, select the incoming clip.
        for (time, y, selected) in [
            (2.0, bounds.center().y, 1),
            (2.0, lower, 0),
            (2.0, bounds.center().y, 1),
            (10.0, bounds.center().y, 2),
            (3.0, lower, 0),
            (3.0, upper, 1),
            (20.0, bounds.center().y, 3),
            (25.0, bounds.center().y, 3),
            (3.0, bounds.center().y, 1),
        ] {
            cx.simulate_click(
                point(bounds.left() + bounds.size.width * (time / 30.0), y),
                Modifiers::default(),
            );
            root.read_with(cx, |root, _| {
                assert_eq!(
                    root.file_queue.selected_file_id(),
                    Some(root.project.clips[selected].id.as_str()),
                    "selection at {time}s"
                );
                assert!((root.preview_ui.playback.current_time() - f64::from(time)).abs() < 0.01);
            });
        }
    }

    #[test]
    fn dragging_and_trimming_use_the_panned_zoom_scale() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                root.preview_ui.timeline_snap = false;
                // Keep the second clip clear of the first after it moves.
                let second = root.project.clips[1].id.clone();
                assert!(root.move_project_clip(&second, 8_000));
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
        let y = lane_center(cx, "project-lane-V1");
        let start = point(bounds.left() + bounds.size.width * 0.125, y);
        // GPUI starts the drag after crossing its threshold and captures that grab point.
        let end = point(start.x + px(8.0) + bounds.size.width * 0.125, start.y);
        drag(cx, start, end);
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
        let start = point(bounds.left() + bounds.size.width * 0.375 - px(3.0), y);
        let end = point(bounds.left() + bounds.size.width * 0.25, y);
        drag(cx, start, end);
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

        // Each gesture is one undo step.
        root.update(cx, |root, _| {
            assert!(root.undo_timeline_edit());
            assert!(root.undo_timeline_edit());
            assert_eq!(root.project.clips[0].offset_millis, 0);
        });
    }

    fn drag(cx: &mut gpui::VisualTestContext, start: Point<Pixels>, end: Point<Pixels>) {
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(start.x + px(8.0), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.update(|window, cx| window.draw(cx).clear());
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    }

    #[test]
    fn dragging_a_clip_onto_another_video_lane_moves_it_there() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
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
        let x = bounds.left() + bounds.size.width * 0.1;
        let (v1, v2, a1) = (
            lane_center(cx, "project-lane-V1"),
            lane_center(cx, "project-lane-V2"),
            lane_center(cx, "project-lane-A1"),
        );
        assert!(v2 < v1 && v1 < a1, "V2 sits above V1, audio below");
        drag(cx, point(x, v1), point(x, v2));
        root.read_with(cx, |root, _| {
            assert_eq!(root.project.clips[0].track, 2);
        });
        // Audio lanes don't take video clips.
        cx.update(|window, cx| window.draw(cx).clear());
        let (v2, a1) = (
            lane_center(cx, "project-lane-V2"),
            lane_center(cx, "project-lane-A1"),
        );
        drag(cx, point(x, v2), point(x, a1));
        root.read_with(cx, |root, _| {
            assert_eq!(root.project.clips[0].track, 2);
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
