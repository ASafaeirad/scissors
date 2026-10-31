use super::{
    ButtonColors, ButtonVariant, Context, FluentBuilder, FrameRoot, InteractiveElement,
    ParentElement, SETTINGS_CONTROL_HEIGHT, StatefulInteractiveElement, Styled, Window,
    animated_button_colors, apply_accessible_button, apply_accessible_button_with_focus,
    apply_accessible_toggle_button, apply_button_motion, button_colors, button_highlight_shadows,
    color, div, icon_svg, theme,
};
use gpui::FocusHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum FrameIconButtonVariant {
    DestructiveGhost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct FrameIconButtonSize {
    pub(in crate::app) button: f32,
    pub(in crate::app) icon: f32,
}

pub(in crate::app) fn frame_choice_button(
    id: impl Into<String>,
    label: impl Into<String>,
    selected: bool,
    enabled: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let label = label.into();
    apply_accessible_toggle_button(
        frame_text_button(
            id,
            label.clone(),
            ButtonVariant::Secondary,
            selected,
            enabled,
            palette,
            window,
            cx,
        )
        .w_full(),
        label,
        enabled,
        selected,
        palette,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "This shared button constructor exposes its semantic state, palette, and GPUI render context explicitly."
)]
pub(in crate::app) fn frame_text_button(
    id: impl Into<String>,
    label: impl Into<String>,
    variant: ButtonVariant,
    selected: bool,
    enabled: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    frame_text_button_inner(
        id, label, variant, selected, enabled, None, palette, window, cx,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "The shared implementation preserves the existing explicit button builder contract."
)]
fn frame_text_button_inner(
    id: impl Into<String>,
    label: impl Into<String>,
    variant: ButtonVariant,
    selected: bool,
    enabled: bool,
    focus: Option<&FocusHandle>,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let label = label.into();
    let display_label = theme::ui_text_owned(label.clone());
    let colors = button_colors(variant, selected, enabled, palette);
    let animated = animated_button_colors(id.clone(), colors, window, cx);
    let background = animated.background;
    let foreground = animated.foreground;
    let motion = animated.motion;
    let button = div()
        .id(id)
        .min_h(theme::ui_rem(SETTINGS_CONTROL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .px(theme::ui_rem(10.0))
        .bg(background)
        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
        .font_weight(theme::TEXT_WEIGHT_MEDIUM)
        .text_color(foreground)
        .opacity(colors.opacity)
        .when(text_button_uses_highlight(variant, selected), |this| {
            this.shadow(button_highlight_shadows(palette))
        })
        .when(enabled, |this| {
            this.hover(gpui::Styled::cursor_pointer)
                .active(move |style| style.bg(color(colors.active_background)))
        })
        .when(!enabled, gpui::Styled::cursor_not_allowed)
        .child(display_label);

    let button = apply_button_motion(button, motion, enabled);

    if let Some(focus) = focus {
        apply_accessible_button_with_focus(button, label, enabled, focus, palette)
    } else {
        apply_accessible_button(button, label, enabled, palette)
    }
}

const fn text_button_uses_highlight(variant: ButtonVariant, selected: bool) -> bool {
    !matches!(variant, ButtonVariant::Ghost) || selected
}

#[expect(
    clippy::too_many_arguments,
    reason = "Icon buttons need explicit a11y labels plus the existing visual button contract."
)]
pub(in crate::app) fn frame_icon_button(
    id: impl Into<String>,
    icon: &'static str,
    label: impl Into<String>,
    variant: FrameIconButtonVariant,
    enabled: bool,
    size: FrameIconButtonSize,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let label = label.into();
    frame_icon_button_inner(id, icon, label, variant, enabled, size, palette, window, cx)
}

#[expect(
    clippy::too_many_arguments,
    reason = "The shared implementation preserves the existing explicit icon button contract."
)]
fn frame_icon_button_inner(
    id: String,
    icon: &'static str,
    label: String,
    variant: FrameIconButtonVariant,
    enabled: bool,
    size: FrameIconButtonSize,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let (background, hover_background, active_background, foreground, hover_foreground, opacity) =
        match (variant, enabled) {
            (FrameIconButtonVariant::DestructiveGhost, false) => (
                palette.fill_subtle,
                palette.fill_subtle,
                palette.fill_subtle,
                palette.danger.with_alpha(0.5),
                palette.danger.with_alpha(0.5),
                1.0,
            ),
            (FrameIconButtonVariant::DestructiveGhost, true) => (
                palette.transparent,
                palette.fill_subtle,
                palette.fill_selected,
                palette.danger,
                palette.danger,
                1.0,
            ),
        };
    let animated = animated_button_colors(
        id.clone(),
        ButtonColors {
            background,
            hover_background,
            active_background,
            foreground,
            hover_foreground,
            opacity,
        },
        window,
        cx,
    );
    let animated_background = animated.background;
    let animated_foreground = animated.foreground;
    let motion = animated.motion;
    let icon = icon_svg(icon, size.icon, animated_foreground);

    let button = div()
        .id(id.clone())
        .group(id)
        .w(theme::ui_rem(size.button))
        .h(theme::ui_rem(size.button))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .bg(animated_background)
        .text_color(animated_foreground)
        .opacity(opacity)
        .when(enabled, |this| {
            this.hover(gpui::Styled::cursor_pointer)
                .active(move |style| style.bg(color(active_background)))
        })
        .when(!enabled, gpui::Styled::cursor_not_allowed)
        .child(icon);

    let button = apply_button_motion(button, motion, enabled);

    apply_accessible_button(button, label, enabled, palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_button_highlight_follows_variant_and_selection() {
        let cases = [
            ("ghost-unselected", ButtonVariant::Ghost, false, false),
            ("ghost-selected", ButtonVariant::Ghost, true, true),
            ("secondary", ButtonVariant::Secondary, false, true),
        ];

        for (name, variant, selected, expected) in cases {
            assert_eq!(
                text_button_uses_highlight(variant, selected),
                expected,
                "highlight mismatch for case={name}, selected={selected}"
            );
        }
    }
}
