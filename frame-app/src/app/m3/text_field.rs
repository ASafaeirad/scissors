use super::{
    App, ColorScheme, Interaction, InteractiveElement, M3Text, Styled, Window, color, div,
    mix_color, theme,
};
use crate::material::{shape, state, typography};
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, prelude::FluentBuilder, px};

const FIELD_HEIGHT: f32 = 56.0;
const FIELD_PADDING: f32 = 16.0;
/// The raised label sits on the outline, inset from the corner with a little padding so the
/// outline appears to break around it.
const RAISED_LABEL_LEFT: f32 = 12.0;
const RAISED_LABEL_PADDING: f32 = 4.0;
const RAISED_LABEL_TOP: f32 = -8.0;
const SUPPORTING_TOP: f32 = 4.0;
const OUTLINE: f32 = 1.0;
const OUTLINE_FOCUSED: f32 = 2.0;
/// Selected text is highlighted with the caret color at this opacity.
const SELECTION_OPACITY: f32 = 0.30;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct TextFieldColors {
    pub(in crate::app) outline: theme::RgbaToken,
    pub(in crate::app) outline_hovered: theme::RgbaToken,
    pub(in crate::app) label: theme::RgbaToken,
    pub(in crate::app) input: theme::RgbaToken,
    pub(in crate::app) affix: theme::RgbaToken,
    pub(in crate::app) supporting: theme::RgbaToken,
    pub(in crate::app) caret: theme::RgbaToken,
    pub(in crate::app) selection: theme::RgbaToken,
}

/// Colors for an outlined text field. Error wins over focus; focus draws in `primary`.
pub(in crate::app) const fn text_field_colors(
    focused: bool,
    error: bool,
    scheme: &ColorScheme,
) -> TextFieldColors {
    let accent = if error {
        Some(scheme.error)
    } else if focused {
        Some(scheme.primary)
    } else {
        None
    };
    let outline = match accent {
        Some(accent) => accent,
        None => scheme.outline,
    };
    TextFieldColors {
        outline,
        outline_hovered: match accent {
            Some(accent) => accent,
            None => scheme.on_surface,
        },
        label: match accent {
            Some(accent) => accent,
            None => scheme.on_surface_variant,
        },
        input: scheme.on_surface,
        affix: scheme.on_surface_variant,
        supporting: if error {
            scheme.error
        } else {
            scheme.on_surface_variant
        },
        caret: if error { scheme.error } else { scheme.primary },
        selection: if error { scheme.error } else { scheme.primary }.with_alpha(SELECTION_OPACITY),
    }
}

/// An M3 outlined text field: a 56px box with a 1px outline that thickens to 2px `primary` while
/// focused, a label that rises onto the outline once the field has a value or focus, an optional
/// suffix, and supporting or error text below.
///
/// The field only draws decoration. Pass the editable element with [`TextField::input`]; in
/// Frame that is `input::m3_text_input`, which owns focus, the caret and IME.
pub(in crate::app) struct TextField {
    id: SharedString,
    label: SharedString,
    input: Option<AnyElement>,
    populated: bool,
    focused: bool,
    enabled: bool,
    suffix: Option<SharedString>,
    supporting: Option<SharedString>,
    error: Option<SharedString>,
    container: Option<theme::RgbaToken>,
}

impl TextField {
    pub(in crate::app) fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            input: None,
            populated: false,
            focused: false,
            enabled: true,
            suffix: None,
            supporting: None,
            error: None,
            container: None,
        }
    }

    pub(in crate::app) fn input(mut self, input: impl IntoElement) -> Self {
        self.input = Some(input.into_any_element());
        self
    }

    /// Whether the field has a value. A populated or focused field raises its label.
    pub(in crate::app) const fn populated(mut self, populated: bool) -> Self {
        self.populated = populated;
        self
    }

    pub(in crate::app) const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Text after the value, such as a unit or file extension.
    pub(in crate::app) fn suffix(mut self, suffix: impl Into<SharedString>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    pub(in crate::app) fn supporting_text(mut self, text: impl Into<SharedString>) -> Self {
        self.supporting = Some(text.into());
        self
    }

    /// Shows `text` in place of the supporting text and switches the field to the error roles.
    pub(in crate::app) fn error(mut self, text: Option<impl Into<SharedString>>) -> Self {
        self.error = text.map(Into::into);
        self
    }

    /// The surface the field sits on. The raised label paints it behind itself to break the
    /// outline. Defaults to `surface`.
    pub(in crate::app) const fn container(mut self, container: theme::RgbaToken) -> Self {
        self.container = Some(container);
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
            input,
            populated,
            focused,
            enabled,
            suffix,
            supporting,
            error,
            container,
        } = self;
        let colors = text_field_colors(focused, error.is_some(), scheme);
        let interaction = Interaction::new(&id, window, cx);
        let hover = if enabled { interaction.hover } else { 0.0 };
        let outline = mix_color(colors.outline, colors.outline_hovered, hover);
        let outline_width = if focused { OUTLINE_FOCUSED } else { OUTLINE };
        let raised = populated || focused;
        let container = container.unwrap_or(scheme.surface);

        let label = if raised {
            div()
                .absolute()
                .top(theme::ui_rem(RAISED_LABEL_TOP))
                .left(theme::ui_rem(RAISED_LABEL_LEFT))
                .px(theme::ui_rem(RAISED_LABEL_PADDING))
                .bg(color(container))
                .m3_type(typography::BODY_SMALL)
                .text_color(color(colors.label))
                .whitespace_nowrap()
                .child(label)
        } else {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(theme::ui_rem(FIELD_PADDING))
                .flex()
                .items_center()
                .m3_type(typography::BODY_LARGE)
                .text_color(color(colors.label))
                .whitespace_nowrap()
                .child(label)
        };

        let field_box = div()
            .relative()
            .w_full()
            .h(theme::ui_rem(FIELD_HEIGHT))
            .flex()
            .items_center()
            .rounded(theme::ui_rem(shape::CORNER_EXTRA_SMALL))
            .border(px(outline_width))
            .border_color(outline)
            // Keep the content still when the outline thickens.
            .px(theme::ui_rem(FIELD_PADDING - (outline_width - OUTLINE)))
            .gap(theme::ui_rem(RAISED_LABEL_PADDING))
            .m3_type(typography::BODY_LARGE)
            .text_color(color(colors.input))
            .when_some(input, |this, input| {
                this.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .child(input),
                )
            })
            .when_some(suffix.filter(|_| raised), |this, suffix| {
                this.child(
                    div()
                        .flex_none()
                        .text_color(color(colors.affix))
                        .child(suffix),
                )
            });

        let supporting = error.or(supporting);
        let field = div()
            .id(id)
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .when(!enabled, |this| {
                this.opacity(state::DISABLED_CONTENT_OPACITY)
            })
            // GPUI paints a div's border after its children. Paint the floating label as a
            // sibling of the bordered box so its background actually cuts the outline.
            .child(div().relative().w_full().child(field_box).child(label))
            .when_some(supporting, |this, supporting| {
                this.child(
                    div()
                        .pt(theme::ui_rem(SUPPORTING_TOP))
                        .px(theme::ui_rem(FIELD_PADDING))
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(colors.supporting))
                        .child(supporting),
                )
            });
        interaction.apply(field, enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn resting_field_uses_outline_and_variant_label() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = text_field_colors(false, false, scheme);
        assert_eq!(colors.outline, scheme.outline);
        assert_eq!(colors.outline_hovered, scheme.on_surface);
        assert_eq!(colors.label, scheme.on_surface_variant);
        assert_eq!(colors.input, scheme.on_surface);
        assert_eq!(colors.caret, scheme.primary);
    }

    #[test]
    fn focus_draws_in_primary() {
        let scheme = material::scheme(ColorTheme::Light);
        let colors = text_field_colors(true, false, scheme);
        assert_eq!(colors.outline, scheme.primary);
        assert_eq!(colors.label, scheme.primary);
        assert_eq!(colors.supporting, scheme.on_surface_variant);
    }

    #[test]
    fn error_overrides_focus() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = text_field_colors(true, true, scheme);
        assert_eq!(colors.outline, scheme.error);
        assert_eq!(colors.outline_hovered, scheme.error);
        assert_eq!(colors.label, scheme.error);
        assert_eq!(colors.supporting, scheme.error);
        assert_eq!(colors.caret, scheme.error);
    }
}
