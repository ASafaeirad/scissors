use super::{
    App, ColorScheme, Interaction, InteractiveElement, M3Colors, M3Text, Styled, Symbol, Window,
    apply_accessible, color, div, m3_icon, theme,
};
use crate::material::{shape, spacing, state, typography};
use gpui::{
    ParentElement, Role, SharedString, StatefulInteractiveElement, Toggled, prelude::FluentBuilder,
};

const fn checkbox_colors(checked: bool, enabled: bool, scheme: &ColorScheme) -> M3Colors {
    if !enabled {
        return M3Colors::new(
            scheme.transparent,
            state::disabled_content(scheme.on_surface),
        )
        .outlined(state::disabled_content(scheme.on_surface));
    }
    if checked {
        M3Colors::new(scheme.primary, scheme.on_primary)
    } else {
        M3Colors::new(scheme.transparent, scheme.on_surface_variant)
            .outlined(scheme.on_surface_variant)
    }
}

/// A labeled M3 checkbox with a 48px touch target.
pub(in crate::app) struct Checkbox {
    id: SharedString,
    label: SharedString,
    checked: bool,
    enabled: bool,
}

impl Checkbox {
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
            enabled: true,
        }
    }

    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(in crate::app) fn build(
        self,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = checkbox_colors(self.checked, self.enabled, scheme);
        let interaction = Interaction::new(&self.id, window, cx);
        let mark = div()
            .flex_none()
            .size(theme::ui_rem(18.0))
            .rounded(theme::ui_rem(shape::CORNER_EXTRA_SMALL))
            .bg(color(colors.container))
            .when_some(colors.outline, |mark, outline| {
                mark.border(theme::ui_rem(2.0)).border_color(color(outline))
            })
            .when(self.checked, |mark| {
                mark.child(m3_icon(Symbol::Check, false, 18.0, color(colors.content)))
            });
        let row = div()
            .id(self.id)
            .flex()
            .items_center()
            .min_h(theme::ui_rem(48.0))
            .gap(theme::ui_rem(spacing::SPACE_3))
            .m3_type(typography::BODY_MEDIUM)
            .text_color(color(if self.enabled {
                scheme.on_surface
            } else {
                state::disabled_content(scheme.on_surface)
            }))
            .when(self.enabled, gpui::Styled::cursor_pointer)
            .child(mark)
            .child(self.label.clone());
        let row = interaction.apply(row, self.enabled);
        apply_accessible(row, Role::CheckBox, self.label, self.enabled, None, scheme)
            .aria_toggled(Toggled::from(self.checked))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;

    #[test]
    fn checkbox_states_use_paired_roles_in_both_themes() {
        for theme in [ColorTheme::Light, ColorTheme::Dark] {
            let scheme = crate::material::scheme(theme);
            let checked = checkbox_colors(true, true, scheme);
            assert_eq!(checked.container, scheme.primary);
            assert_eq!(checked.content, scheme.on_primary);
            assert_eq!(
                checkbox_colors(false, true, scheme).outline,
                Some(scheme.on_surface_variant)
            );
            assert_eq!(
                checkbox_colors(true, false, scheme).content,
                state::disabled_content(scheme.on_surface)
            );
        }
    }
}
