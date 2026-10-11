use super::{ColorScheme, M3Text, Styled, apply_focus, color, div, theme};
use crate::material::{shape, state, typography};
use gpui::{
    FocusHandle, InteractiveElement, Orientation, ParentElement, Role, SharedString,
    StatefulInteractiveElement, prelude::FluentBuilder, relative,
};

pub(in crate::app) const M3_SLIDER_HEIGHT: f32 = 44.0;
const TRACK_HEIGHT: f32 = 16.0;
const TRACK_TOP: f32 = (M3_SLIDER_HEIGHT - TRACK_HEIGHT) / 2.0;
const TRACK_OUTER_CORNER: f32 = TRACK_HEIGHT / 2.0;
const TRACK_INNER_CORNER: f32 = 2.0;
const HANDLE_WIDTH: f32 = 4.0;
/// Space between the handle and each track segment.
const HANDLE_GAP: f32 = 6.0;
const STOP_SIZE: f32 = 4.0;
const STOP_INSET: f32 = (TRACK_HEIGHT - STOP_SIZE) / 2.0;
const VALUE_LABEL_HEIGHT: f32 = 44.0;
const VALUE_LABEL_MIN_WIDTH: f32 = 48.0;
const VALUE_LABEL_GAP: f32 = 8.0;

/// An M3 slider: an active track, a 4px bar handle and an inactive track, separated by gaps.
///
/// Like the Frame slider, this draws state only. The caller owns pointer and keyboard input,
/// typically by converting mouse positions with the slider's bounds.
pub(in crate::app) struct Slider {
    id: SharedString,
    label: SharedString,
    value: f64,
    min: f64,
    max: f64,
    value_text: Option<String>,
    show_value_label: bool,
    enabled: bool,
    focus: Option<FocusHandle>,
}

impl Slider {
    /// `label` is announced to assistive tech. Values outside `min..=max` are clamped.
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: f64,
        min: f64,
        max: f64,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value,
            min,
            max,
            value_text: None,
            show_value_label: false,
            enabled: true,
            focus: None,
        }
    }

    /// Text announced for the current value, and shown in the value label. Defaults to the
    /// rounded number.
    pub(in crate::app) fn value_text(mut self, text: impl Into<String>) -> Self {
        self.value_text = Some(text.into());
        self
    }

    /// Shows the value in an `inverse_surface` bubble above the handle, e.g. while dragging.
    pub(in crate::app) const fn show_value_label(mut self, show: bool) -> Self {
        self.show_value_label = show;
        self
    }

    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(in crate::app) fn focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub(in crate::app) fn build(self, scheme: &ColorScheme) -> gpui::Stateful<gpui::Div> {
        let Self {
            id,
            label,
            value,
            min,
            max,
            value_text,
            show_value_label,
            enabled,
            focus,
        } = self;
        let fraction = slider_fraction(value, min, max);
        // `max`/`min` rather than `clamp`: clamp panics on NaN bounds.
        let value = value.max(min.min(max)).min(max.max(min));
        let value_text = value_text.unwrap_or_else(|| format!("{value:.0}"));

        let active = div()
            .absolute()
            .left_0()
            .top(theme::ui_rem(TRACK_TOP))
            .w(relative(fraction))
            .h(theme::ui_rem(TRACK_HEIGHT))
            .flex()
            .pr(theme::ui_rem(HANDLE_GAP))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .rounded_l(theme::ui_rem(TRACK_OUTER_CORNER))
                    .rounded_r(theme::ui_rem(TRACK_INNER_CORNER))
                    .bg(color(scheme.primary)),
            );
        let inactive = div()
            .absolute()
            .left(relative(fraction))
            .right_0()
            .top(theme::ui_rem(TRACK_TOP))
            .h(theme::ui_rem(TRACK_HEIGHT))
            .flex()
            .pl(theme::ui_rem(HANDLE_GAP))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .rounded_l(theme::ui_rem(TRACK_INNER_CORNER))
                    .rounded_r(theme::ui_rem(TRACK_OUTER_CORNER))
                    .bg(color(scheme.secondary_container))
                    // End stop: marks the maximum on the inactive track.
                    .child(
                        div()
                            .absolute()
                            .top(theme::ui_rem(STOP_INSET))
                            .right(theme::ui_rem(STOP_INSET))
                            .size(theme::ui_rem(STOP_SIZE))
                            .rounded(theme::ui_rem(shape::CORNER_FULL))
                            .bg(color(scheme.on_secondary_container)),
                    ),
            );
        let handle = div()
            .absolute()
            .top_0()
            .left(relative(fraction))
            .ml(theme::ui_rem(-HANDLE_WIDTH / 2.0))
            .w(theme::ui_rem(HANDLE_WIDTH))
            .h(theme::ui_rem(M3_SLIDER_HEIGHT))
            .rounded(theme::ui_rem(HANDLE_WIDTH / 2.0))
            .bg(color(scheme.primary));

        let slider = div()
            .id(id)
            .relative()
            .w_full()
            .h(theme::ui_rem(M3_SLIDER_HEIGHT))
            .when(!enabled, |this| {
                this.opacity(state::DISABLED_CONTENT_OPACITY)
            })
            .when(enabled, gpui::Styled::cursor_pointer)
            .child(active)
            .child(inactive)
            .child(handle)
            .when(show_value_label, |this| {
                this.child(value_label(value_text.clone(), fraction, scheme))
            });

        apply_focus(
            slider
                .role(Role::Slider)
                .aria_label(label)
                .aria_orientation(Orientation::Horizontal)
                .aria_numeric_value(value)
                .aria_min_numeric_value(min)
                .aria_max_numeric_value(max)
                .aria_value(value_text),
            enabled,
            focus.as_ref(),
            scheme,
        )
    }
}

/// Bubble centered over the handle. A zero-width anchor at the handle position centers the
/// bubble without knowing its rendered width.
fn value_label(text: String, fraction: f32, scheme: &ColorScheme) -> gpui::Div {
    div()
        .absolute()
        .left(relative(fraction))
        .bottom(theme::ui_rem(M3_SLIDER_HEIGHT + VALUE_LABEL_GAP))
        .w_0()
        .flex()
        .justify_center()
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .min_w(theme::ui_rem(VALUE_LABEL_MIN_WIDTH))
                .h(theme::ui_rem(VALUE_LABEL_HEIGHT))
                .px(theme::ui_rem(16.0))
                .rounded(theme::ui_rem(VALUE_LABEL_HEIGHT / 2.0))
                .bg(color(scheme.inverse_surface))
                .text_color(color(scheme.inverse_on_surface))
                .m3_type(typography::LABEL_LARGE)
                .whitespace_nowrap()
                .child(text),
        )
}

/// Position of `value` along the track, in `0..=1`. A zero-width range sits at the start.
#[expect(
    clippy::cast_possible_truncation,
    reason = "Slider fractions are layout ratios clamped to 0..=1 before narrowing."
)]
pub(in crate::app) fn slider_fraction(value: f64, min: f64, max: f64) -> f32 {
    let span = max - min;
    if !span.is_finite() || span.abs() <= f64::EPSILON || !value.is_finite() {
        return 0.0;
    }
    ((value - min) / span).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_maps_value_into_range() {
        assert!((slider_fraction(100.0, 0.0, 200.0) - 0.5).abs() < f32::EPSILON);
        assert!((slider_fraction(70.0, 0.0, 100.0) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn fraction_clamps_out_of_range_values() {
        assert!(slider_fraction(-5.0, 0.0, 10.0).abs() < f32::EPSILON);
        assert!((slider_fraction(15.0, 0.0, 10.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn degenerate_ranges_sit_at_the_start() {
        assert!(slider_fraction(5.0, 5.0, 5.0).abs() < f32::EPSILON);
        assert!(slider_fraction(f64::NAN, 0.0, 1.0).abs() < f32::EPSILON);
        assert!(slider_fraction(1.0, 0.0, f64::INFINITY).abs() < f32::EPSILON);
    }

    #[test]
    fn track_is_vertically_centered() {
        assert!((TRACK_TOP.mul_add(2.0, TRACK_HEIGHT) - M3_SLIDER_HEIGHT).abs() < f32::EPSILON);
    }
}
