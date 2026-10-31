use super::{ColorScheme, theme};
use gpui::UniformListScrollHandle;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct ScrollbarColors {
    pub(in crate::app) track: theme::RgbaToken,
    pub(in crate::app) thumb: theme::RgbaToken,
}

pub(in crate::app) const fn scrollbar_colors(scheme: &ColorScheme) -> ScrollbarColors {
    ScrollbarColors {
        track: scheme.surface_container_highest,
        thumb: scheme.outline,
    }
}

/// A vertical scrollbar for a `uniform_list`, drawn over its parent's right edge. Place it in a
/// `relative` parent next to the list. `content_height` is the list's full height in logical
/// pixels, used until GPUI has measured the scroll extent.
pub(in crate::app) fn uniform_list_scrollbar(
    id: impl Into<String>,
    scroll_handle: &UniformListScrollHandle,
    content_height: f32,
    scheme: &ColorScheme,
) -> gpui::Stateful<gpui::Div> {
    let colors = scrollbar_colors(scheme);
    let base_handle = scroll_handle.0.borrow().base_handle.clone();
    crate::app::components::vertical_scrollbar(
        id,
        base_handle,
        content_height,
        colors.track,
        colors.thumb,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material::scheme;

    #[test]
    fn scrollbar_thumb_stands_out_from_its_track() {
        for theme in [ColorTheme::Light, ColorTheme::Dark] {
            let scheme = scheme(theme);
            let colors = scrollbar_colors(scheme);

            assert_eq!(colors.track, scheme.surface_container_highest);
            assert_eq!(colors.thumb, scheme.outline);
            assert_ne!(colors.track, colors.thumb);
        }
    }
}
