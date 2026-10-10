use super::{
    App, ColorScheme, Interaction, InteractiveElement, M3Colors, M3Text, Styled, Window,
    apply_accessible, color, div, theme,
};
use crate::material::{shape, state, typography};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement, Toggled,
    prelude::FluentBuilder,
};

/// Touch target around the radio ring, also the size of its hover state layer.
const TARGET: f32 = 40.0;
const RING: f32 = 20.0;
const RING_BORDER: f32 = 2.0;
const DOT: f32 = 10.0;
const ITEM_MIN_HEIGHT: f32 = 48.0;
const ITEM_TRAILING_PADDING: f32 = 12.0;
const ITEM_GAP: f32 = 8.0;

/// Ring and dot color (`content`) plus the row fill (`container`) used by [`RadioItem`].
pub(in crate::app) const fn radio_colors(
    selected: bool,
    enabled: bool,
    scheme: &ColorScheme,
) -> M3Colors {
    match (selected, enabled) {
        (_, false) => M3Colors::new(
            scheme.transparent,
            state::disabled_content(scheme.on_surface),
        ),
        (true, true) => M3Colors::new(scheme.secondary_container, scheme.primary),
        (false, true) => M3Colors::new(scheme.transparent, scheme.on_surface_variant),
    }
}

/// The ring and dot, centered in a 40px target. `hover` tints the target with a state layer.
fn radio_indicator(selected: bool, colors: M3Colors, hover: f32) -> gpui::Div {
    let content = colors.content;
    div()
        .flex_none()
        .size(theme::ui_rem(TARGET))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::ui_rem(shape::CORNER_FULL))
        .bg(color(
            content.with_alpha(content.alpha * state::HOVER_OPACITY * hover),
        ))
        .child(
            div()
                .size(theme::ui_rem(RING))
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::ui_rem(shape::CORNER_FULL))
                .border(theme::ui_rem(RING_BORDER))
                .border_color(color(content))
                .when(selected, |ring| {
                    ring.child(
                        div()
                            .size(theme::ui_rem(DOT))
                            .rounded(theme::ui_rem(shape::CORNER_FULL))
                            .bg(color(content)),
                    )
                }),
        )
}

/// A standalone M3 radio button. Put a visible label next to it, or use [`RadioItem`] for a
/// whole clickable row.
pub(in crate::app) struct Radio {
    id: SharedString,
    label: SharedString,
    selected: bool,
    enabled: bool,
    focus: Option<FocusHandle>,
}

impl Radio {
    /// `label` is announced to assistive tech.
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        selected: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected,
            enabled: true,
            focus: None,
        }
    }

    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(in crate::app) fn focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub(in crate::app) fn build(
        self,
        scheme: &'static ColorScheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let Self {
            id,
            label,
            selected,
            enabled,
            focus,
        } = self;
        let colors = radio_colors(selected, enabled, scheme);
        let interaction = Interaction::new(&id, window, cx);
        let hover = if enabled { interaction.hover } else { 0.0 };
        let radio = div()
            .id(id)
            .flex_none()
            .rounded(theme::ui_rem(shape::CORNER_FULL))
            .when(enabled, gpui::Styled::cursor_pointer)
            .child(radio_indicator(selected, colors, hover));
        let radio = interaction.apply(radio, enabled);
        apply_accessible(
            radio,
            Role::RadioButton,
            label,
            enabled,
            focus.as_ref(),
            scheme,
        )
        .aria_toggled(Toggled::from(selected))
    }
}

/// A list row with a leading radio, a label and optional trailing supporting text. The selected
/// row fills with `secondary_container`. The whole row is the click target.
pub(in crate::app) struct RadioItem {
    id: SharedString,
    label: SharedString,
    supporting: Option<SharedString>,
    selected: bool,
    enabled: bool,
    focus: Option<FocusHandle>,
}

impl RadioItem {
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        selected: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            supporting: None,
            selected,
            enabled: true,
            focus: None,
        }
    }

    /// Short text shown at the trailing edge, e.g. what the option is for.
    pub(in crate::app) fn supporting(mut self, text: impl Into<SharedString>) -> Self {
        self.supporting = Some(text.into());
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

    pub(in crate::app) fn build(
        self,
        scheme: &'static ColorScheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let Self {
            id,
            label,
            supporting,
            selected,
            enabled,
            focus,
        } = self;
        let colors = radio_colors(selected, enabled, scheme);
        // The row's state layer tints with `on_surface`, as in the design.
        let row = M3Colors::new(colors.container, scheme.on_surface);
        let interaction = Interaction::new(&id, window, cx);
        let label_color = if enabled {
            scheme.on_surface
        } else {
            state::disabled_content(scheme.on_surface)
        };
        let supporting_color = if enabled {
            scheme.on_surface_variant
        } else {
            state::disabled_content(scheme.on_surface)
        };
        let pressed = row.pressed();

        let item = div()
            .id(id)
            .w_full()
            .min_h(theme::ui_rem(ITEM_MIN_HEIGHT))
            .flex()
            .items_center()
            .gap(theme::ui_rem(ITEM_GAP))
            .pr(theme::ui_rem(ITEM_TRAILING_PADDING))
            .rounded(theme::ui_rem(shape::CORNER_MEDIUM))
            .bg(interaction.container(row, enabled))
            .child(radio_indicator(selected, colors, 0.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .m3_type(typography::BODY_LARGE)
                    .text_color(color(label_color))
                    .child(label.clone()),
            )
            .when_some(supporting, |this, supporting| {
                this.child(
                    div()
                        .flex_none()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(supporting_color))
                        .text_right()
                        .child(supporting),
                )
            })
            .when(enabled, |this| {
                this.cursor_pointer()
                    .active(move |style| style.bg(color(pressed)))
            });
        let item = interaction.apply(item, enabled);
        apply_accessible(
            item,
            Role::RadioButton,
            label,
            enabled,
            focus.as_ref(),
            scheme,
        )
        .aria_toggled(Toggled::from(selected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn selected_radio_is_primary_on_secondary_container() {
        let scheme = material::scheme(ColorTheme::Dark);
        let on = radio_colors(true, true, scheme);
        assert_eq!(on.content, scheme.primary);
        assert_eq!(on.container, scheme.secondary_container);
        let off = radio_colors(false, true, scheme);
        assert_eq!(off.content, scheme.on_surface_variant);
        assert_eq!(off.container, scheme.transparent);
    }

    #[test]
    fn disabled_radio_dims_content_and_drops_fill() {
        let scheme = material::scheme(ColorTheme::Light);
        let disabled = radio_colors(true, false, scheme);
        assert_eq!(disabled.content, scheme.on_surface.with_alpha(0.38));
        assert_eq!(disabled.container, scheme.transparent);
    }

    #[test]
    fn dot_fits_inside_ring_border() {
        assert!(RING_BORDER.mul_add(2.0, DOT) <= RING);
    }
}
