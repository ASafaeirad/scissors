use super::{
    App, ColorScheme, Elevation, Interaction, InteractiveElement, M3Colors, M3Text, PRESSED_CORNER,
    Styled, Symbol, TypeStyle, Window, apply_accessible, color, div, m3_icon, surface_shadows,
    theme,
};
use crate::material::{shape, state, typography};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement,
    prelude::FluentBuilder,
};

/// M3 common button styles, highest to lowest emphasis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum ButtonVariant {
    Filled,
    Tonal,
    Elevated,
    Outlined,
    Text,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) enum ButtonSize {
    /// 32px.
    ExtraSmall,
    /// 40px, the default.
    #[default]
    Small,
    /// 56px.
    Medium,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ButtonMetrics {
    height: f32,
    gap: f32,
    icon: f32,
    type_style: TypeStyle,
}

const fn button_metrics(size: ButtonSize) -> ButtonMetrics {
    match size {
        ButtonSize::ExtraSmall => ButtonMetrics {
            height: 32.0,
            gap: 4.0,
            icon: 20.0,
            type_style: typography::LABEL_LARGE,
        },
        ButtonSize::Small => ButtonMetrics {
            height: 40.0,
            gap: 8.0,
            icon: 20.0,
            type_style: typography::LABEL_LARGE,
        },
        ButtonSize::Medium => ButtonMetrics {
            height: 56.0,
            gap: 8.0,
            icon: 24.0,
            type_style: typography::TITLE_MEDIUM,
        },
    }
}

/// Left and right padding. A leading icon tightens the left side on small buttons.
const fn button_padding(variant: ButtonVariant, size: ButtonSize, has_icon: bool) -> (f32, f32) {
    match (size, variant, has_icon) {
        (ButtonSize::ExtraSmall, _, _) | (ButtonSize::Small, ButtonVariant::Text, false) => {
            (12.0, 12.0)
        }
        (ButtonSize::Medium, _, _) | (ButtonSize::Small, _, false) => (24.0, 24.0),
        (ButtonSize::Small, ButtonVariant::Text, true) => (12.0, 16.0),
        (ButtonSize::Small, _, true) => (16.0, 24.0),
    }
}

pub(in crate::app) const fn button_colors(
    variant: ButtonVariant,
    enabled: bool,
    scheme: &ColorScheme,
) -> M3Colors {
    if !enabled {
        let content = state::disabled_content(scheme.on_surface);
        return match variant {
            ButtonVariant::Text => M3Colors::new(scheme.transparent, content),
            ButtonVariant::Outlined => M3Colors::new(scheme.transparent, content)
                .outlined(state::disabled_container(scheme.on_surface)),
            ButtonVariant::Filled | ButtonVariant::Tonal | ButtonVariant::Elevated => {
                M3Colors::new(state::disabled_container(scheme.on_surface), content)
            }
        };
    }

    match variant {
        ButtonVariant::Filled => M3Colors::new(scheme.primary, scheme.on_primary),
        ButtonVariant::Tonal => {
            M3Colors::new(scheme.secondary_container, scheme.on_secondary_container)
        }
        ButtonVariant::Elevated => M3Colors::new(scheme.surface_container_low, scheme.primary),
        ButtonVariant::Outlined => {
            M3Colors::new(scheme.transparent, scheme.primary).outlined(scheme.outline)
        }
        ButtonVariant::Text => M3Colors::new(scheme.transparent, scheme.primary),
    }
}

/// Resting and hovered elevation. Filled and tonal buttons lift on hover.
const fn button_elevation(variant: ButtonVariant, enabled: bool) -> (Elevation, Elevation) {
    match (variant, enabled) {
        (ButtonVariant::Filled | ButtonVariant::Tonal, true) => {
            (Elevation::Level0, Elevation::Level1)
        }
        (ButtonVariant::Elevated, true) => (Elevation::Level1, Elevation::Level2),
        _ => (Elevation::Level0, Elevation::Level0),
    }
}

/// An M3 common button: a pill with a label and an optional leading icon.
///
/// ```ignore
/// m3::Button::new("export", "Export", m3::ButtonVariant::Filled)
///     .icon(Symbol::FileExport)
///     .build(scheme, window, cx)
///     .on_click(cx.listener(|this, _, window, cx| this.export(window, cx)))
/// ```
pub(in crate::app) struct Button {
    id: SharedString,
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    icon: Option<Symbol>,
    enabled: bool,
    selected: bool,
    focus: Option<FocusHandle>,
}

impl Button {
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        variant: ButtonVariant,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant,
            size: ButtonSize::default(),
            icon: None,
            enabled: true,
            selected: false,
            focus: None,
        }
    }

    pub(in crate::app) const fn icon(mut self, icon: Symbol) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(in crate::app) const fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Toggle buttons square their corners while selected.
    pub(in crate::app) const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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
            variant,
            size,
            icon,
            enabled,
            selected,
            focus,
        } = self;
        let colors = button_colors(variant, enabled, scheme);
        let metrics = button_metrics(size);
        let (padding_left, padding_right) = button_padding(variant, size, icon.is_some());
        let (rest_elevation, hover_elevation) = button_elevation(variant, enabled);
        let corner = if selected {
            if matches!(size, ButtonSize::Medium) {
                shape::CORNER_LARGE
            } else {
                shape::CORNER_MEDIUM
            }
        } else {
            metrics.height / 2.0
        };
        let interaction = Interaction::new(&id, window, cx);
        let content = color(colors.content);
        let hover_shadows = surface_shadows(hover_elevation, colors.outline, scheme);
        let pressed = colors.pressed();

        let button = div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(theme::ui_rem(metrics.gap))
            .h(theme::ui_rem(metrics.height))
            .pl(theme::ui_rem(padding_left))
            .pr(theme::ui_rem(padding_right))
            .rounded(theme::ui_rem(corner))
            .bg(interaction.container(colors, enabled))
            .shadow(surface_shadows(rest_elevation, colors.outline, scheme))
            .text_color(content)
            .m3_type(metrics.type_style)
            .whitespace_nowrap()
            .when_some(icon, |this, icon| {
                this.child(m3_icon(icon, false, metrics.icon, content))
            })
            .child(label.clone())
            .when(enabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.shadow(hover_shadows))
                    .active(move |style| {
                        style
                            .bg(color(pressed))
                            .rounded(theme::ui_rem(PRESSED_CORNER))
                    })
            });

        let button = interaction.apply(button, enabled);
        apply_accessible(button, Role::Button, label, enabled, focus.as_ref(), scheme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn variants_map_to_their_container_roles() {
        let scheme = material::scheme(ColorTheme::Light);
        let cases = [
            (ButtonVariant::Filled, scheme.primary, scheme.on_primary),
            (
                ButtonVariant::Tonal,
                scheme.secondary_container,
                scheme.on_secondary_container,
            ),
            (
                ButtonVariant::Elevated,
                scheme.surface_container_low,
                scheme.primary,
            ),
            (ButtonVariant::Outlined, scheme.transparent, scheme.primary),
            (ButtonVariant::Text, scheme.transparent, scheme.primary),
        ];
        for (variant, container, content) in cases {
            let colors = button_colors(variant, true, scheme);
            assert_eq!(colors.container, container, "{variant:?}");
            assert_eq!(colors.content, content, "{variant:?}");
        }
        assert_eq!(
            button_colors(ButtonVariant::Outlined, true, scheme).outline,
            Some(scheme.outline)
        );
    }

    #[test]
    fn disabled_buttons_use_on_surface_opacities() {
        let scheme = material::scheme(ColorTheme::Dark);
        let filled = button_colors(ButtonVariant::Filled, false, scheme);
        assert_eq!(filled.container, scheme.on_surface.with_alpha(0.12));
        assert_eq!(filled.content, scheme.on_surface.with_alpha(0.38));

        let text = button_colors(ButtonVariant::Text, false, scheme);
        assert_eq!(text.container, scheme.transparent);
    }

    #[test]
    fn leading_icon_tightens_left_padding_on_small_buttons() {
        assert_eq!(
            button_padding(ButtonVariant::Filled, ButtonSize::Small, false),
            (24.0, 24.0)
        );
        assert_eq!(
            button_padding(ButtonVariant::Filled, ButtonSize::Small, true),
            (16.0, 24.0)
        );
        assert_eq!(
            button_padding(ButtonVariant::Text, ButtonSize::Small, true),
            (12.0, 16.0)
        );
        assert_eq!(
            button_padding(ButtonVariant::Tonal, ButtonSize::ExtraSmall, true),
            (12.0, 12.0)
        );
    }

    #[test]
    fn only_filled_tonal_and_elevated_buttons_cast_shadows() {
        assert_eq!(
            button_elevation(ButtonVariant::Elevated, true),
            (Elevation::Level1, Elevation::Level2)
        );
        assert_eq!(
            button_elevation(ButtonVariant::Filled, true),
            (Elevation::Level0, Elevation::Level1)
        );
        assert_eq!(
            button_elevation(ButtonVariant::Outlined, true),
            (Elevation::Level0, Elevation::Level0)
        );
        assert_eq!(
            button_elevation(ButtonVariant::Filled, false),
            (Elevation::Level0, Elevation::Level0)
        );
    }
}
