//! The track speed card shown above a selected clip's settings.
//!
//! The slider is discrete: its stops are spread evenly along the track, so the common slow
//! and near-original speeds get as much room as the large multipliers.

use super::{Context, InteractiveElement, ParentElement, ScissorsRoot, Styled, Window, color, div};
use super::{
    m3::{self, M3Text},
    theme,
};
use crate::material::{ColorScheme, Symbol, shape, spacing, typography};
use gpui::{AppContext, StatefulInteractiveElement, prelude::FluentBuilder};

/// Speeds the slider snaps to, in order. Must start at the minimum and end at the maximum
/// accepted by `set_project_clip_speed`.
const SPEED_STOPS: [f64; 16] = [
    0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 15.0, 20.0,
];
/// One-tap presets shown as chips under the slider.
const SPEED_PRESETS: [f64; 6] = [0.5, 1.0, 2.0, 4.0, 10.0, 20.0];
const PRESET_COLUMNS: u16 = 3;
const SPEED_EPSILON: f64 = 1e-6;
const READOUT_MIN_WIDTH: f32 = 80.0;
const READOUT_HEIGHT: f32 = 40.0;

#[derive(Clone)]
struct TrackSpeedDrag(String);

pub(super) fn track_speed_control(
    clip: &super::project::ClipPlacement,
    source: Option<&scissors_core::types::TimelineSource>,
    enabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let speed = clip.speed;
    let label = format_speed(speed);
    let summary = source.map_or_else(
        || speed_summary(speed).to_string(),
        |source| {
            let millis = scissors_core::project_timeline::clip_duration_millis(source);
            format!(
                "{} · {}",
                speed_summary(speed),
                format_duration(std::time::Duration::from_millis(millis).as_secs_f64())
            )
        },
    );

    let slider = speed_slider(clip, &label, enabled, scheme, cx);

    let reset_id = clip.id.clone();
    let header = div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(spacing::SPACE_2))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().m3_type(typography::TITLE_SMALL).child("Track speed"))
                .child(
                    div()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .truncate()
                        .child(summary),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .min_w(theme::ui_rem(READOUT_MIN_WIDTH))
                .h(theme::ui_rem(READOUT_HEIGHT))
                .px(theme::ui_rem(spacing::SPACE_4))
                .rounded(theme::ui_rem(shape::CORNER_SMALL))
                .bg(color(scheme.secondary_container))
                .text_color(color(scheme.on_secondary_container))
                .m3_type(typography::TITLE_MEDIUM)
                .child(label),
        )
        .child(
            m3::IconButton::new("track-speed-reset", Symbol::RestartAlt, "Reset speed")
                .enabled(enabled && !is_speed(speed, 1.0))
                .build(scheme, window, cx)
                .on_click(cx.listener(move |root, _, _, cx| {
                    if root.set_project_clip_speed(&reset_id, 1.0) {
                        cx.notify();
                    }
                })),
        );

    let range = div()
        .flex()
        .justify_between()
        .m3_type(typography::LABEL_SMALL)
        .text_color(color(scheme.on_surface_variant))
        .child(format_speed(SPEED_STOPS[0]))
        .child(format_speed(SPEED_STOPS[SPEED_STOPS.len() - 1]));

    let presets = speed_presets(clip, enabled, scheme, window, cx);

    m3::m3_surface(scheme.surface_container_highest, shape::CORNER_MEDIUM)
        .flex_none()
        .flex()
        .flex_col()
        .p(theme::ui_rem(spacing::SPACE_4))
        .mb(theme::ui_rem(spacing::SPACE_3))
        .text_color(color(scheme.on_surface))
        .child(header)
        .child(div().mt(theme::ui_rem(spacing::SPACE_1)).child(slider))
        // Tuck the range labels under the track, inside the slider's touch area.
        .child(range.mt(theme::ui_rem(-spacing::SPACE_2)))
        .child(presets.mt(theme::ui_rem(spacing::SPACE_3)))
}

/// Discrete speed slider. Dragging snaps to the nearest stop; arrow keys step between stops.
fn speed_slider(
    clip: &super::project::ClipPlacement,
    label: &str,
    enabled: bool,
    scheme: &ColorScheme,
    cx: &Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    let speed = clip.speed;
    let id = clip.id.clone();
    let key_id = id.clone();
    m3::Slider::new(
        "track-speed",
        "Track speed",
        speed_fraction(speed),
        0.0,
        1.0,
    )
    .value_text(label)
    .stops(stop_fractions())
    .enabled(enabled)
    .build(scheme)
    .when(enabled, |slider| {
        slider
            .on_drag(TrackSpeedDrag(id.clone()), |_, _, _, cx| {
                cx.new(|_| super::project_timeline::MediaDragPreview)
            })
            .on_drag_move(cx.listener(
                move |root, event: &gpui::DragMoveEvent<TrackSpeedDrag>, _, cx| {
                    if event.drag(cx).0 != id {
                        return;
                    }
                    let fraction = super::timeline_slider_percent_from_bounds(
                        event.event.position,
                        event.bounds,
                    );
                    if root.set_project_clip_speed(&id, speed_at_fraction(fraction)) {
                        cx.notify();
                    }
                },
            ))
            .on_key_down(cx.listener(move |root, event: &gpui::KeyDownEvent, _, cx| {
                let next = match event.keystroke.key.as_str() {
                    "left" | "down" => step_speed(speed, false),
                    "right" | "up" => step_speed(speed, true),
                    "home" => SPEED_STOPS[0],
                    "end" => SPEED_STOPS[SPEED_STOPS.len() - 1],
                    _ => return,
                };
                cx.stop_propagation();
                if root.set_project_clip_speed(&key_id, next) {
                    cx.notify();
                }
            }))
    })
}

fn speed_presets(
    clip: &super::project::ClipPlacement,
    enabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    // A grid rather than a wrapping row: Taffy measures a wrapped row's height as if every chip
    // sat on its own line, which left the card tall and mostly empty.
    div()
        .grid()
        .grid_cols(PRESET_COLUMNS)
        .gap(theme::ui_rem(spacing::SPACE_2))
        .children(SPEED_PRESETS.into_iter().map(|preset| {
            let id = clip.id.clone();
            m3::Chip::new(
                format!("track-speed-preset-{preset}"),
                format_speed(preset),
                m3::ChipKind::Filter,
            )
            .selected(is_speed(clip.speed, preset))
            .enabled(enabled)
            .build(scheme, window, cx)
            .justify_center()
            .on_click(cx.listener(move |root, _, _, cx| {
                if root.set_project_clip_speed(&id, preset) {
                    cx.notify();
                }
            }))
        }))
}

fn is_speed(speed: f64, target: f64) -> bool {
    (speed - target).abs() < SPEED_EPSILON
}

#[expect(
    clippy::cast_precision_loss,
    reason = "There are only a handful of speed stops."
)]
const fn last_stop_index() -> f64 {
    (SPEED_STOPS.len() - 1) as f64
}

/// Position of each stop along the slider track.
#[expect(
    clippy::cast_possible_truncation,
    reason = "Stop fractions are layout ratios in 0..=1."
)]
fn stop_fractions() -> impl Iterator<Item = f32> {
    SPEED_STOPS.iter().map(|&stop| speed_fraction(stop) as f32)
}

/// Slider position for `speed`, interpolating between the neighboring stops.
fn speed_fraction(speed: f64) -> f64 {
    if !speed.is_finite() || speed <= SPEED_STOPS[0] {
        return 0.0;
    }
    SPEED_STOPS
        .windows(2)
        .enumerate()
        .find(|(_, pair)| speed <= pair[1])
        .map_or(1.0, |(index, pair)| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "There are only a handful of speed stops."
            )]
            let index = index as f64;
            (index + (speed - pair[0]) / (pair[1] - pair[0])) / last_stop_index()
        })
}

/// The stop nearest to a slider position in `0..=1`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "The index is rounded from a fraction clamped to 0..=1."
)]
fn speed_at_fraction(fraction: f64) -> f64 {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    SPEED_STOPS[(fraction * last_stop_index()).round() as usize]
}

/// The next stop above (`faster`) or below `speed`, staying at the ends of the range.
fn step_speed(speed: f64, faster: bool) -> f64 {
    if faster {
        SPEED_STOPS
            .into_iter()
            .find(|&stop| stop > speed + SPEED_EPSILON)
            .unwrap_or(SPEED_STOPS[SPEED_STOPS.len() - 1])
    } else {
        SPEED_STOPS
            .into_iter()
            .rev()
            .find(|&stop| stop < speed - SPEED_EPSILON)
            .unwrap_or(SPEED_STOPS[0])
    }
}

/// `1×`, `0.75×`, `2.5×`: up to two decimals, without trailing zeros.
fn format_speed(speed: f64) -> String {
    let number = format!("{speed:.2}");
    let number = number.trim_end_matches('0').trim_end_matches('.');
    format!("{number}×")
}

const fn speed_summary(speed: f64) -> &'static str {
    if (speed - 1.0).abs() < SPEED_EPSILON {
        "Original speed"
    } else if speed > 1.0 {
        "Sped up"
    } else {
        "Slowed down"
    }
}

/// `HH:MM:SS`, rounded to the nearest second.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Clip durations are finite, non-negative and far below u64::MAX seconds."
)]
fn format_duration(seconds: f64) -> String {
    let total = if seconds.is_finite() {
        seconds.max(0.0).round() as u64
    } else {
        0
    };
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        total % 3600 / 60,
        total % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_are_evenly_spaced_and_cover_the_accepted_range() {
        assert!(is_speed(SPEED_STOPS[0], 0.5));
        assert!(is_speed(SPEED_STOPS[SPEED_STOPS.len() - 1], 20.0));
        assert!(SPEED_STOPS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(speed_fraction(0.5).abs() < f64::EPSILON);
        assert!((speed_fraction(20.0) - 1.0).abs() < f64::EPSILON);
        assert!((speed_fraction(1.0) - 2.0 / 15.0).abs() < 1e-9);
        assert!(
            SPEED_PRESETS
                .iter()
                .all(|preset| SPEED_STOPS.iter().any(|stop| is_speed(*stop, *preset)))
        );
    }

    #[test]
    fn speeds_between_stops_interpolate() {
        let halfway = f64::midpoint(speed_fraction(4.0), speed_fraction(5.0));
        assert!((speed_fraction(4.5) - halfway).abs() < 1e-9);
        assert!(speed_fraction(f64::NAN).abs() < f64::EPSILON);
        assert!((speed_fraction(40.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dragging_snaps_to_the_nearest_stop() {
        assert!(is_speed(speed_at_fraction(-1.0), 0.5));
        assert!(is_speed(speed_at_fraction(speed_fraction(1.0) + 0.01), 1.0));
        assert!(is_speed(speed_at_fraction(2.0), 20.0));
        assert!(is_speed(speed_at_fraction(f64::NAN), 0.5));
    }

    #[test]
    fn keys_step_between_stops() {
        assert!(is_speed(step_speed(1.0, true), 1.25));
        assert!(is_speed(step_speed(1.0, false), 0.75));
        assert!(is_speed(step_speed(1.1, false), 1.0));
        assert!(is_speed(step_speed(20.0, true), 20.0));
        assert!(is_speed(step_speed(0.5, false), 0.5));
    }

    #[test]
    fn labels_drop_trailing_zeros() {
        assert_eq!(format_speed(1.0), "1×");
        assert_eq!(format_speed(0.75), "0.75×");
        assert_eq!(format_speed(2.5), "2.5×");
        assert_eq!(speed_summary(1.0), "Original speed");
        assert_eq!(speed_summary(2.0), "Sped up");
        assert_eq!(speed_summary(0.5), "Slowed down");
        assert_eq!(format_duration(19.4), "00:00:19");
        assert_eq!(format_duration(3_725.0), "01:02:05");
    }
}
