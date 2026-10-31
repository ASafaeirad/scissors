use super::{
    App, ColorScheme, Interaction, InteractiveElement, M3Colors, M3Text, Styled, Symbol, Window,
    apply_accessible, color, div, m3_icon, theme,
};
use crate::material::{shape, state, typography};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement, Toggled,
    prelude::FluentBuilder, px,
};

const CARD_PADDING: f32 = 12.0;
const CARD_GAP: f32 = 6.0;
const CARD_ICON_SIZE: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct SelectCardColors {
    pub(in crate::app) container: theme::RgbaToken,
    pub(in crate::app) outline: theme::RgbaToken,
    pub(in crate::app) icon: theme::RgbaToken,
    pub(in crate::app) label: theme::RgbaToken,
    pub(in crate::app) supporting: theme::RgbaToken,
}

/// Colors for a selectable outlined card. Selection fills with `secondary_container` and draws
/// the outline and icon in `primary`.
pub(in crate::app) const fn select_card_colors(
    selected: bool,
    enabled: bool,
    scheme: &ColorScheme,
) -> SelectCardColors {
    if !enabled {
        let content = state::disabled_content(scheme.on_surface);
        return SelectCardColors {
            container: scheme.transparent,
            outline: state::disabled_container(scheme.on_surface),
            icon: content,
            label: content,
            supporting: content,
        };
    }
    if selected {
        SelectCardColors {
            container: scheme.secondary_container,
            outline: scheme.primary,
            icon: scheme.primary,
            label: scheme.on_secondary_container,
            supporting: scheme.on_surface_variant,
        }
    } else {
        SelectCardColors {
            container: scheme.transparent,
            outline: scheme.outline_variant,
            icon: scheme.on_surface_variant,
            label: scheme.on_surface,
            supporting: scheme.on_surface_variant,
        }
    }
}

/// An outlined card that acts as one option of a single-select group: an icon, a label and a
/// line of supporting text. Lay several out in a grid inside a `RadioGroup`.
pub(in crate::app) struct SelectCard {
    id: SharedString,
    label: SharedString,
    icon: Option<Symbol>,
    supporting: Option<SharedString>,
    selected: bool,
    single_line: bool,
    enabled: bool,
    focus: Option<FocusHandle>,
}

impl SelectCard {
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        selected: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            supporting: None,
            selected,
            single_line: false,
            enabled: true,
            focus: None,
        }
    }

    pub(in crate::app) const fn icon(mut self, icon: Symbol) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(in crate::app) fn supporting(mut self, text: impl Into<SharedString>) -> Self {
        self.supporting = Some(text.into());
        self
    }

    /// Truncates the label and supporting text for compact list rows.
    pub(in crate::app) const fn single_line(mut self) -> Self {
        self.single_line = true;
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
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let Self {
            id,
            label,
            icon,
            supporting,
            selected,
            single_line,
            enabled,
            focus,
        } = self;
        let colors = select_card_colors(selected, enabled, scheme);
        let layer = M3Colors::new(colors.container, scheme.on_surface);
        let interaction = Interaction::new(&id, window, cx);
        let pressed = layer.pressed();

        let card = div()
            .id(id)
            .min_w_0()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(CARD_GAP))
            .p(theme::ui_rem(CARD_PADDING))
            .rounded(theme::ui_rem(shape::CORNER_MEDIUM))
            .border(px(1.0))
            .border_color(color(colors.outline))
            .bg(interaction.container(layer, enabled))
            .when_some(icon, |this, icon| {
                this.child(m3_icon(icon, selected, CARD_ICON_SIZE, color(colors.icon)))
            })
            .child(
                div()
                    .m3_type(typography::LABEL_LARGE)
                    .text_color(color(colors.label))
                    .when(single_line, Styled::truncate)
                    .child(label.clone()),
            )
            .when_some(supporting, |this, supporting| {
                this.child(
                    div()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(colors.supporting))
                        .when(single_line, Styled::truncate)
                        .child(supporting),
                )
            })
            .when(enabled, |this| {
                this.cursor_pointer()
                    .active(move |style| style.bg(color(pressed)))
            });
        let card = interaction.apply(card, enabled);
        apply_accessible(
            card,
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
    fn selected_card_fills_and_outlines_in_primary() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = select_card_colors(true, true, scheme);
        assert_eq!(colors.container, scheme.secondary_container);
        assert_eq!(colors.label, scheme.on_secondary_container);
        assert_eq!(colors.outline, scheme.primary);
        assert_eq!(colors.icon, scheme.primary);
    }

    #[test]
    fn resting_card_uses_outline_variant() {
        let scheme = material::scheme(ColorTheme::Light);
        let colors = select_card_colors(false, true, scheme);
        assert_eq!(colors.container, scheme.transparent);
        assert_eq!(colors.outline, scheme.outline_variant);
        assert_eq!(colors.label, scheme.on_surface);
    }

    #[test]
    fn disabled_card_dims_everything() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = select_card_colors(true, false, scheme);
        assert_eq!(colors.container, scheme.transparent);
        assert_eq!(colors.label, scheme.on_surface.with_alpha(0.38));
    }
}
