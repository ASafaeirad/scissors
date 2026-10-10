use super::{ColorScheme, InteractiveElement, Styled, color, div, theme};
use crate::material::shape;
use gpui::{
    ParentElement, Role, SharedString, StatefulInteractiveElement, prelude::FluentBuilder, relative,
};

const TRACK_HEIGHT: f32 = 4.0;
/// Space between the active indicator and the track.
const TRACK_GAP: f32 = 4.0;
const STOP_SIZE: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct ProgressColors {
    pub(in crate::app) active: theme::RgbaToken,
    pub(in crate::app) track: theme::RgbaToken,
    pub(in crate::app) stop: theme::RgbaToken,
}

pub(in crate::app) const fn progress_colors(scheme: &ColorScheme) -> ProgressColors {
    ProgressColors {
        active: scheme.primary,
        track: scheme.secondary_container,
        stop: scheme.primary,
    }
}

/// An M3 determinate linear progress indicator: a `primary` bar, a gap, the
/// `secondary_container` track and a stop dot at the end.
pub(in crate::app) struct LinearProgress {
    id: SharedString,
    label: SharedString,
    value: f32,
}

impl LinearProgress {
    /// `value` is the completed fraction, clamped to `0..=1`. `label` is announced to assistive
    /// tech.
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: f32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value,
        }
    }

    pub(in crate::app) fn build(self, scheme: &'static ColorScheme) -> gpui::Stateful<gpui::Div> {
        let colors = progress_colors(scheme);
        let value = progress_fraction(self.value);
        let corner = theme::ui_rem(TRACK_HEIGHT / 2.0);
        div()
            .id(self.id)
            .role(Role::ProgressIndicator)
            .aria_label(self.label)
            .aria_numeric_value(f64::from(value) * 100.0)
            .aria_min_numeric_value(0.0)
            .aria_max_numeric_value(100.0)
            .relative()
            .w_full()
            .h(theme::ui_rem(TRACK_HEIGHT))
            .flex()
            .items_center()
            .when(value > 0.0, |this| {
                this.gap(theme::ui_rem(TRACK_GAP)).child(
                    div()
                        .flex_none()
                        .w(relative(value))
                        .h_full()
                        .rounded(corner)
                        .bg(color(colors.active)),
                )
            })
            .when(value < 1.0, |this| {
                this.child(
                    div()
                        .flex_1()
                        .h_full()
                        .rounded(corner)
                        .bg(color(colors.track)),
                )
                .child(
                    div()
                        .absolute()
                        .right_0()
                        .size(theme::ui_rem(STOP_SIZE))
                        .rounded(theme::ui_rem(shape::CORNER_FULL))
                        .bg(color(colors.stop)),
                )
            })
    }
}

/// The completed fraction in `0..=1`. Non-finite input reads as no progress.
pub(in crate::app) const fn progress_fraction(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn indicator_is_primary_on_secondary_container_track() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = progress_colors(scheme);
        assert_eq!(colors.active, scheme.primary);
        assert_eq!(colors.track, scheme.secondary_container);
        assert_eq!(colors.stop, scheme.primary);
    }

    #[test]
    fn fraction_is_clamped_and_ignores_nan() {
        assert!((progress_fraction(0.4) - 0.4).abs() < f32::EPSILON);
        assert!((progress_fraction(1.5) - 1.0).abs() < f32::EPSILON);
        assert!(progress_fraction(-1.0).abs() < f32::EPSILON);
        assert!(progress_fraction(f32::NAN).abs() < f32::EPSILON);
    }
}
