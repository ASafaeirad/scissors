use super::{
    App, ColorScheme, Interaction, InteractiveElement, M3Colors, M3Text, Styled, Symbol, Window,
    apply_accessible, color, div, m3_icon, surface_shadows, theme,
};
use crate::material::{Elevation, shape, state, typography};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement, Toggled,
    prelude::FluentBuilder,
};

const CHIP_HEIGHT: f32 = 32.0;
const CHIP_ICON_SIZE: f32 = 18.0;
const CHIP_PADDING: f32 = 16.0;
const CHIP_PADDING_WITH_ICON: f32 = 8.0;
const CHIP_GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum ChipKind {
    /// A smart action related to the content, such as "Add to calendar".
    Assist,
    /// A toggleable filter. Shows a check mark while selected.
    Filter,
    /// A suggested response or query.
    Suggestion,
}

/// Colors for a chip. Disabled chips keep these colors and render at 38% opacity, matching the
/// design system.
pub(in crate::app) const fn chip_colors(
    kind: ChipKind,
    selected: bool,
    elevated: bool,
    scheme: &ColorScheme,
) -> M3Colors {
    if selected {
        return M3Colors::new(scheme.secondary_container, scheme.on_secondary_container);
    }
    let content = match kind {
        ChipKind::Assist => scheme.on_surface,
        ChipKind::Filter | ChipKind::Suggestion => scheme.on_surface_variant,
    };
    if elevated {
        M3Colors::new(scheme.surface_container_low, content)
    } else {
        M3Colors::new(scheme.transparent, content).outlined(scheme.outline_variant)
    }
}

/// Assist chips tint their leading icon with `primary`. Other chips use the label color.
const fn chip_icon_color(
    kind: ChipKind,
    selected: bool,
    colors: M3Colors,
    scheme: &ColorScheme,
) -> crate::theme::RgbaToken {
    match (kind, selected) {
        (ChipKind::Assist, false) => scheme.primary,
        _ => colors.content,
    }
}

/// An M3 chip: a compact 32px element with an 8px corner.
pub(in crate::app) struct Chip {
    id: SharedString,
    label: SharedString,
    kind: ChipKind,
    icon: Option<Symbol>,
    selected: bool,
    elevated: bool,
    enabled: bool,
    focus: Option<FocusHandle>,
}

impl Chip {
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        kind: ChipKind,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            icon: None,
            selected: false,
            elevated: false,
            enabled: true,
            focus: None,
        }
    }

    /// Leading icon. Selected filter chips show a check mark instead.
    pub(in crate::app) const fn icon(mut self, icon: Symbol) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(in crate::app) const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Elevated chips sit on `surface_container_low` with a level 1 shadow instead of an outline.
    pub(in crate::app) const fn elevated(mut self, elevated: bool) -> Self {
        self.elevated = elevated;
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
            kind,
            icon,
            selected,
            elevated,
            enabled,
            focus,
        } = self;
        let selected = selected && matches!(kind, ChipKind::Filter);
        let colors = chip_colors(kind, selected, elevated, scheme);
        let icon_color = color(chip_icon_color(kind, selected, colors, scheme));
        let leading = if selected { Some(Symbol::Check) } else { icon };
        let elevation = if elevated && !selected {
            Elevation::Level1
        } else {
            Elevation::Level0
        };
        let interaction = Interaction::new(&id, window, cx);
        let pressed = colors.pressed();

        let chip = div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .gap(theme::ui_rem(CHIP_GAP))
            .h(theme::ui_rem(CHIP_HEIGHT))
            .pl(theme::ui_rem(if leading.is_some() {
                CHIP_PADDING_WITH_ICON
            } else {
                CHIP_PADDING
            }))
            .pr(theme::ui_rem(CHIP_PADDING))
            .rounded(theme::ui_rem(shape::CORNER_SMALL))
            .bg(interaction.container(colors, enabled))
            .shadow(surface_shadows(elevation, colors.outline, scheme))
            .text_color(color(colors.content))
            .m3_type(typography::LABEL_LARGE)
            .whitespace_nowrap()
            .when(!enabled, |this| {
                this.opacity(state::DISABLED_CONTENT_OPACITY)
            })
            .when_some(leading, |this, leading| {
                this.child(m3_icon(leading, false, CHIP_ICON_SIZE, icon_color))
            })
            .child(label.clone())
            .when(enabled, |this| {
                this.cursor_pointer()
                    .active(move |style| style.bg(color(pressed)))
            });

        let chip = interaction.apply(chip, enabled);
        let chip = apply_accessible(chip, Role::Button, label, enabled, focus.as_ref(), scheme);
        if matches!(kind, ChipKind::Filter) {
            chip.aria_toggled(Toggled::from(selected))
        } else {
            chip
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn selected_chips_use_secondary_container_without_outline() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = chip_colors(ChipKind::Filter, true, false, scheme);
        assert_eq!(colors.container, scheme.secondary_container);
        assert_eq!(colors.content, scheme.on_secondary_container);
        assert_eq!(colors.outline, None);
    }

    #[test]
    fn unselected_chips_are_outlined_unless_elevated() {
        let scheme = material::scheme(ColorTheme::Light);
        let flat = chip_colors(ChipKind::Filter, false, false, scheme);
        let elevated = chip_colors(ChipKind::Filter, false, true, scheme);
        assert_eq!(flat.outline, Some(scheme.outline_variant));
        assert_eq!(elevated.outline, None);
        assert_eq!(elevated.container, scheme.surface_container_low);
    }

    #[test]
    fn assist_chips_tint_their_icon_with_primary() {
        let scheme = material::scheme(ColorTheme::Light);
        let colors = chip_colors(ChipKind::Assist, false, false, scheme);
        assert_eq!(colors.content, scheme.on_surface);
        assert_eq!(
            chip_icon_color(ChipKind::Assist, false, colors, scheme),
            scheme.primary
        );
        let filter = chip_colors(ChipKind::Filter, false, false, scheme);
        assert_eq!(
            chip_icon_color(ChipKind::Filter, false, filter, scheme),
            scheme.on_surface_variant
        );
    }
}
