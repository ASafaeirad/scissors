use super::{
    Context, FluentBuilder, InteractiveElement, ParentElement, SETTINGS_CONTROL_HEIGHT,
    ScissorsRoot, Styled, Window, apply_accessible_toggle_button, apply_button_motion,
    button_motion, color, div, mix_color, mix_scalar, selected_motion, theme,
};

pub(in crate::app) fn scissors_list_item(
    id: impl Into<String>,
    label: impl Into<String>,
    selected: bool,
    enabled: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let label = label.into();
    let selected_progress = selected_motion(format!("{id}-selected"), selected, window, cx);
    let motion = button_motion(format!("{id}-hover"), window, cx);
    let hover_progress = *motion.hover_transition.evaluate(window, cx);
    let emphasis_progress = selected_progress.max(hover_progress);

    let item = div()
        .id(id)
        .min_h(theme::ui_rem(SETTINGS_CONTROL_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .border_l(theme::ui_rem(2.0))
        .border_color(mix_color(
            palette.transparent,
            palette.text_muted,
            selected_progress,
        ))
        .bg(mix_color(
            palette.transparent,
            palette.fill_subtle,
            selected_progress,
        ))
        .pl(theme::ui_rem(mix_scalar(8.0, 12.0, selected_progress)))
        .pr(theme::ui_rem(12.0))
        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
        .font_weight(theme::TEXT_WEIGHT_MEDIUM)
        .text_color(mix_color(
            palette.text_muted,
            palette.text_primary,
            emphasis_progress,
        ))
        .opacity(if enabled { 1.0 } else { 0.5 })
        .when(enabled, |this| this.hover(gpui::Styled::cursor_pointer))
        .when(!enabled, gpui::Styled::cursor_not_allowed);

    let item = apply_button_motion(item, motion, enabled);

    apply_accessible_toggle_button(item, label, enabled, selected, palette)
}

#[expect(
    clippy::too_many_arguments,
    reason = "The list-item builder keeps content, interaction state, palette, and render context explicit."
)]
pub(in crate::app) fn scissors_list_item_with_caption(
    id: impl Into<String>,
    title: impl Into<String>,
    caption: impl Into<String>,
    selected: bool,
    enabled: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    let title = title.into();
    let display_title = theme::ui_text_owned(title.clone());
    let caption = theme::ui_text_owned(caption.into());

    scissors_list_item(id, title, selected, enabled, palette, window, cx)
        .gap_3()
        .child(
            div()
                .flex_none()
                .text_color(color(palette.text_primary))
                .child(display_title),
        )
        .child(
            div()
                .min_w_0()
                .flex_auto()
                .truncate()
                .text_right()
                .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                .font_weight(theme::TEXT_WEIGHT_REGULAR)
                .text_color(color(palette.text_muted))
                .child(caption),
        )
}
