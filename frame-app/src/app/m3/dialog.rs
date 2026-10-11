use super::{ColorScheme, InteractiveElement, Styled, color, div, elevation_shadows, theme};
use crate::material::{Elevation, shape};
use gpui::{Role, SharedString, StatefulInteractiveElement};

/// Opacity of the `scrim` role behind a modal dialog.
const SCRIM_OPACITY: f32 = 0.32;
/// Dialogs keep this much room from the window edges.
pub(in crate::app) const DIALOG_MARGIN: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct DialogColors {
    pub(in crate::app) scrim: theme::RgbaToken,
    pub(in crate::app) container: theme::RgbaToken,
    pub(in crate::app) headline: theme::RgbaToken,
    pub(in crate::app) supporting: theme::RgbaToken,
}

/// Colors for a basic dialog. `visibility` fades the scrim in and out (0 hidden, 1 shown).
pub(in crate::app) fn dialog_colors(scheme: &ColorScheme, visibility: f32) -> DialogColors {
    DialogColors {
        scrim: scheme
            .scrim
            .with_alpha(SCRIM_OPACITY * visibility.clamp(0.0, 1.0)),
        container: scheme.surface_container_high,
        headline: scheme.on_surface,
        supporting: scheme.on_surface_variant,
    }
}

/// The full-window scrim a modal dialog sits on. Children are centered. The caller attaches
/// `.on_click(...)` to dismiss and `.on_key_down(...)` for Escape.
pub(in crate::app) fn scrim(
    id: impl Into<SharedString>,
    visibility: f32,
    scheme: &ColorScheme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(theme::ui_rem(DIALOG_MARGIN))
        .bg(color(dialog_colors(scheme, visibility).scrim))
        .opacity(visibility.clamp(0.0, 1.0))
        .occlude()
}

/// An M3 dialog container: `surface_container_high`, extra-large corners and level 3 elevation.
///
/// It has no padding so content can run edge to edge (dividers, side panels). Children lay out
/// in a column. Clicks inside don't reach the scrim.
pub(in crate::app) struct Dialog {
    id: SharedString,
    label: SharedString,
}

impl Dialog {
    /// `label` names the dialog for assistive tech, usually its headline.
    pub(in crate::app) fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    pub(in crate::app) fn build(self, scheme: &ColorScheme) -> gpui::Stateful<gpui::Div> {
        let colors = dialog_colors(scheme, 1.0);
        div()
            .id(self.id)
            .role(Role::Dialog)
            .aria_label(self.label)
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(theme::ui_rem(shape::CORNER_EXTRA_LARGE))
            .bg(color(colors.container))
            .text_color(color(colors.supporting))
            .shadow(elevation_shadows(Elevation::Level3, scheme))
            .occlude()
            .on_click(|_, _, cx| cx.stop_propagation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn dialog_uses_high_container_and_paired_text_roles() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = dialog_colors(scheme, 1.0);
        assert_eq!(colors.container, scheme.surface_container_high);
        assert_eq!(colors.headline, scheme.on_surface);
        assert_eq!(colors.supporting, scheme.on_surface_variant);
    }

    #[test]
    fn scrim_fades_with_visibility() {
        let scheme = material::scheme(ColorTheme::Light);
        assert!((dialog_colors(scheme, 1.0).scrim.alpha - SCRIM_OPACITY).abs() < 1e-6);
        assert!(dialog_colors(scheme, 0.0).scrim.alpha.abs() < f32::EPSILON);
        assert!((dialog_colors(scheme, 2.0).scrim.alpha - SCRIM_OPACITY).abs() < 1e-6);
    }
}
