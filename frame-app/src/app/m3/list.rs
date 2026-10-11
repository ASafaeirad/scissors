use super::{ColorScheme, M3Text, Styled, color, div, theme};
use crate::material::{spacing, typography};
use gpui::{ParentElement, SharedString};

const LIST_ITEM_HEIGHT: f32 = 48.0;

/// Colors for a read-only detail list: a `primary` subheader, then rows that pair a muted
/// label with the value it describes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct ListColors {
    pub(in crate::app) subheader: crate::theme::RgbaToken,
    pub(in crate::app) label: crate::theme::RgbaToken,
    pub(in crate::app) value: crate::theme::RgbaToken,
}

/// The value carries the information, so it gets the stronger `on_surface` role.
pub(in crate::app) const fn list_colors(scheme: &ColorScheme) -> ListColors {
    ListColors {
        subheader: scheme.primary,
        label: scheme.on_surface_variant,
        value: scheme.on_surface,
    }
}

/// Section title above a group of list items, e.g. "Video stream".
pub(in crate::app) fn list_subheader(
    text: impl Into<SharedString>,
    scheme: &ColorScheme,
) -> gpui::Div {
    div()
        .px(theme::ui_rem(spacing::SPACE_2))
        .pt(theme::ui_rem(spacing::SPACE_4))
        .pb(theme::ui_rem(spacing::SPACE_2))
        .m3_type(typography::TITLE_SMALL)
        .text_color(color(list_colors(scheme).subheader))
        .child(text.into())
}

/// A one-line list item with a label and trailing value. Long values truncate rather than wrap.
pub(in crate::app) struct ListItem {
    label: SharedString,
    value: Option<SharedString>,
}

impl ListItem {
    pub(in crate::app) fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: None,
        }
    }

    pub(in crate::app) fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub(in crate::app) fn build(self, scheme: &ColorScheme) -> gpui::Div {
        let colors = list_colors(scheme);
        let row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(theme::ui_rem(spacing::SPACE_4))
            .min_h(theme::ui_rem(LIST_ITEM_HEIGHT))
            .px(theme::ui_rem(spacing::SPACE_2))
            .m3_type(typography::BODY_LARGE)
            .child(
                div()
                    .flex_none()
                    .text_color(color(colors.label))
                    .child(self.label),
            );
        match self.value {
            Some(value) => row.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(color(colors.value))
                    .child(value),
            ),
            None => row,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn values_outrank_their_labels() {
        for theme in [ColorTheme::Light, ColorTheme::Dark] {
            let scheme = material::scheme(theme);
            let colors = list_colors(scheme);
            assert_eq!(colors.subheader, scheme.primary);
            assert_eq!(colors.label, scheme.on_surface_variant);
            assert_eq!(colors.value, scheme.on_surface);
        }
    }
}
