use super::{
    App, ColorScheme, Interaction, InteractiveElement, M3Colors, PRESSED_CORNER, Styled, Symbol,
    Window, apply_accessible, color, div, m3_icon, surface_shadows, theme,
};
use crate::material::{Elevation, shape, state};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement, Toggled,
    prelude::FluentBuilder,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum IconButtonVariant {
    Standard,
    Filled,
    Tonal,
    Outlined,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) enum IconButtonSize {
    /// 32px target, 20px icon.
    ExtraSmall,
    /// 40px target, 24px icon. The default.
    #[default]
    Small,
    /// 56px target, 24px icon.
    Medium,
}

const fn icon_button_metrics(size: IconButtonSize) -> (f32, f32) {
    match size {
        IconButtonSize::ExtraSmall => (32.0, 20.0),
        IconButtonSize::Small => (40.0, 24.0),
        IconButtonSize::Medium => (56.0, 24.0),
    }
}

/// Colors for an icon button. `toggle` is `None` for plain buttons, or the selected state for
/// toggle buttons.
pub(in crate::app) const fn icon_button_colors(
    variant: IconButtonVariant,
    toggle: Option<bool>,
    enabled: bool,
    scheme: &ColorScheme,
) -> M3Colors {
    if !enabled {
        let content = state::disabled_content(scheme.on_surface);
        return match variant {
            IconButtonVariant::Filled | IconButtonVariant::Tonal => {
                M3Colors::new(state::disabled_container(scheme.on_surface), content)
            }
            IconButtonVariant::Standard | IconButtonVariant::Outlined => {
                M3Colors::new(scheme.transparent, content)
            }
        };
    }

    match (variant, toggle) {
        (IconButtonVariant::Standard, Some(true)) => {
            M3Colors::new(scheme.transparent, scheme.primary)
        }
        (IconButtonVariant::Standard, _) => {
            M3Colors::new(scheme.transparent, scheme.on_surface_variant)
        }
        (IconButtonVariant::Filled, Some(false)) => {
            M3Colors::new(scheme.surface_container_highest, scheme.primary)
        }
        (IconButtonVariant::Filled, _) => M3Colors::new(scheme.primary, scheme.on_primary),
        (IconButtonVariant::Tonal, Some(false)) => {
            M3Colors::new(scheme.surface_container_highest, scheme.on_surface_variant)
        }
        (IconButtonVariant::Tonal, _) => {
            M3Colors::new(scheme.secondary_container, scheme.on_secondary_container)
        }
        (IconButtonVariant::Outlined, Some(true)) => {
            M3Colors::new(scheme.inverse_surface, scheme.inverse_on_surface)
        }
        (IconButtonVariant::Outlined, _) => {
            M3Colors::new(scheme.transparent, scheme.on_surface_variant)
                .outlined(scheme.outline_variant)
        }
    }
}

/// An M3 icon button. Needs an accessible label because it has no visible text.
///
/// Call [`IconButton::toggle`] to make it a toggle button: it then reports its pressed state to
/// assistive tech, fills its icon and squares its corners while selected.
pub(in crate::app) struct IconButton {
    id: SharedString,
    icon: Symbol,
    label: SharedString,
    variant: IconButtonVariant,
    size: IconButtonSize,
    toggle: Option<bool>,
    enabled: bool,
    focus: Option<FocusHandle>,
}

impl IconButton {
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        icon: Symbol,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            label: label.into(),
            variant: IconButtonVariant::Standard,
            size: IconButtonSize::default(),
            toggle: None,
            enabled: true,
            focus: None,
        }
    }

    pub(in crate::app) const fn variant(mut self, variant: IconButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub(in crate::app) const fn size(mut self, size: IconButtonSize) -> Self {
        self.size = size;
        self
    }

    pub(in crate::app) const fn toggle(mut self, selected: bool) -> Self {
        self.toggle = Some(selected);
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
            icon,
            label,
            variant,
            size,
            toggle,
            enabled,
            focus,
        } = self;
        let colors = icon_button_colors(variant, toggle, enabled, scheme);
        let (target, icon_size) = icon_button_metrics(size);
        let selected = toggle == Some(true);
        let corner = if selected {
            shape::CORNER_MEDIUM
        } else {
            target / 2.0
        };
        let interaction = Interaction::new(&id, window, cx);
        let content = color(colors.content);
        let pressed = colors.pressed();

        let button = div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(theme::ui_rem(target))
            .rounded(theme::ui_rem(corner))
            .bg(interaction.container(colors, enabled))
            .shadow(surface_shadows(Elevation::Level0, colors.outline, scheme))
            .text_color(content)
            .child(m3_icon(icon, selected, icon_size, content))
            .when(enabled, |this| {
                this.cursor_pointer().active(move |style| {
                    style
                        .bg(color(pressed))
                        .rounded(theme::ui_rem(PRESSED_CORNER))
                })
            });

        let button = interaction.apply(button, enabled);
        let button = apply_accessible(button, Role::Button, label, enabled, focus.as_ref(), scheme);
        match toggle {
            Some(selected) => button.aria_toggled(Toggled::from(selected)),
            None => button,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn standard_toggle_highlights_with_primary_when_selected() {
        let scheme = material::scheme(ColorTheme::Dark);
        let off = icon_button_colors(IconButtonVariant::Standard, Some(false), true, scheme);
        let on = icon_button_colors(IconButtonVariant::Standard, Some(true), true, scheme);
        assert_eq!(off.content, scheme.on_surface_variant);
        assert_eq!(on.content, scheme.primary);
        assert_eq!(on.container, scheme.transparent);
    }

    #[test]
    fn unselected_filled_and_tonal_toggles_fall_back_to_highest_container() {
        let scheme = material::scheme(ColorTheme::Light);
        for variant in [IconButtonVariant::Filled, IconButtonVariant::Tonal] {
            let colors = icon_button_colors(variant, Some(false), true, scheme);
            assert_eq!(colors.container, scheme.surface_container_highest);
        }
        assert_eq!(
            icon_button_colors(IconButtonVariant::Filled, None, true, scheme).container,
            scheme.primary
        );
    }

    #[test]
    fn outlined_icon_button_inverts_when_selected() {
        let scheme = material::scheme(ColorTheme::Light);
        let off = icon_button_colors(IconButtonVariant::Outlined, Some(false), true, scheme);
        let on = icon_button_colors(IconButtonVariant::Outlined, Some(true), true, scheme);
        assert_eq!(off.outline, Some(scheme.outline_variant));
        assert_eq!(on.container, scheme.inverse_surface);
        assert_eq!(on.outline, None);
    }

    #[test]
    fn disabled_icon_buttons_keep_a_container_only_when_filled_or_tonal() {
        let scheme = material::scheme(ColorTheme::Dark);
        let filled = icon_button_colors(IconButtonVariant::Filled, None, false, scheme);
        let standard = icon_button_colors(IconButtonVariant::Standard, None, false, scheme);
        assert_eq!(filled.container, scheme.on_surface.with_alpha(0.12));
        assert_eq!(standard.container, scheme.transparent);
        assert_eq!(standard.content, scheme.on_surface.with_alpha(0.38));
    }
}
