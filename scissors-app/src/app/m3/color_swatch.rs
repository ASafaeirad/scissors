use super::{
    App, ColorScheme, Interaction, M3Colors, Styled, Symbol, Window, apply_accessible, color, div,
    m3_icon, theme,
};
use gpui::{
    FocusHandle, InteractiveElement, ParentElement, Role, SharedString, StatefulInteractiveElement,
    Toggled, prelude::FluentBuilder,
};

/// Accent choices preview the primary role and its paired content role.
pub(in crate::app) const fn color_swatch_colors(preview: &ColorScheme) -> M3Colors {
    M3Colors::new(preview.primary, preview.on_primary)
}

pub(in crate::app) struct ColorSwatch {
    id: SharedString,
    label: SharedString,
    selected: bool,
    colors: M3Colors,
    focus: Option<FocusHandle>,
}

impl ColorSwatch {
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        preview: &ColorScheme,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            colors: color_swatch_colors(preview),
            focus: None,
        }
    }

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
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = self.colors;
        let interaction = Interaction::new(&self.id, window, cx);
        let swatch = div()
            .id(self.id)
            .size(theme::ui_rem(48.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .border(theme::ui_rem(2.0))
            .border_color(color(if self.selected {
                scheme.primary
            } else {
                scheme.transparent
            }))
            .child(
                div()
                    .size(theme::ui_rem(36.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(interaction.container(colors, true))
                    .when(self.selected, |this| {
                        this.child(m3_icon(Symbol::Check, false, 20.0, color(colors.content)))
                    }),
            );
        let swatch = interaction.apply(swatch, true);
        apply_accessible(
            swatch,
            Role::RadioButton,
            self.label,
            true,
            self.focus.as_ref(),
            scheme,
        )
        .aria_toggled(Toggled::from(self.selected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::{AccentColor, ColorTheme};

    #[test]
    fn swatches_pair_each_preview_primary_with_its_content_role() {
        for theme in ColorTheme::ALL {
            for (_, accent) in AccentColor::PRESETS {
                let preview = crate::material::scheme_from_accent(theme, accent);
                let colors = color_swatch_colors(&preview);
                assert_eq!(colors.container, preview.primary);
                assert_eq!(colors.content, preview.on_primary);
            }
        }
    }
}
