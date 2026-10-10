use super::{ColorScheme, Elevation, Styled, color, div, elevation_shadows, theme};

const TOOLBAR_HEIGHT: f32 = 64.0;
const TOOLBAR_GAP: f32 = 4.0;
const FLOATING_PADDING: f32 = 8.0;
const DOCKED_PADDING: f32 = 16.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum ToolbarVariant {
    /// A pill that floats over content with a level 3 shadow.
    Floating,
    /// Full width, attached to an edge of its container.
    Docked,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) enum ToolbarColor {
    /// `surface_container` with `on_surface_variant` content.
    #[default]
    Standard,
    /// `primary_container` with `on_primary_container` content, for more emphasis.
    Vibrant,
}

pub(in crate::app) const fn toolbar_colors(
    tone: ToolbarColor,
    scheme: &ColorScheme,
) -> (crate::theme::RgbaToken, crate::theme::RgbaToken) {
    match tone {
        ToolbarColor::Standard => (scheme.surface_container, scheme.on_surface_variant),
        ToolbarColor::Vibrant => (scheme.primary_container, scheme.on_primary_container),
    }
}

/// An M3 toolbar container. Add icon buttons and other controls as children.
///
/// Standard icon buttons pick their own content color, so in a vibrant toolbar pass a tonal or
/// filled variant for contrast.
pub(in crate::app) fn toolbar(
    variant: ToolbarVariant,
    tone: ToolbarColor,
    scheme: &ColorScheme,
) -> gpui::Div {
    let (container, content) = toolbar_colors(tone, scheme);
    let toolbar = div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(TOOLBAR_GAP))
        .h(theme::ui_rem(TOOLBAR_HEIGHT))
        .bg(color(container))
        .text_color(color(content));

    match variant {
        ToolbarVariant::Floating => toolbar
            .flex_none()
            .px(theme::ui_rem(FLOATING_PADDING))
            .rounded(theme::ui_rem(TOOLBAR_HEIGHT / 2.0))
            .shadow(elevation_shadows(Elevation::Level3, scheme)),
        ToolbarVariant::Docked => toolbar
            .w_full()
            .justify_between()
            .px(theme::ui_rem(DOCKED_PADDING)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn toolbar_tones_map_to_container_roles() {
        let scheme = material::scheme(ColorTheme::Dark);
        assert_eq!(
            toolbar_colors(ToolbarColor::Standard, scheme),
            (scheme.surface_container, scheme.on_surface_variant)
        );
        assert_eq!(
            toolbar_colors(ToolbarColor::Vibrant, scheme),
            (scheme.primary_container, scheme.on_primary_container)
        );
    }
}
