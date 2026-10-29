use super::{ParentElement, Styled, assets, color, div, icon_svg, input_highlight_shadows, theme};

pub(in crate::app) const FRAME_CHECKBOX_SIZE: f32 = 16.0;
pub(in crate::app) const FRAME_CHECK_ICON_SIZE: f32 = 14.0;
const FRAME_CHECKBOX_MARK_SIZE: f32 = 8.0;

pub(in crate::app) fn frame_checkbox_indicator(
    checked: bool,
    indeterminate: bool,
    disabled: bool,
    palette: &theme::ThemePalette,
) -> gpui::Div {
    let active = checked || indeterminate;
    let mut mark = div()
        .w(theme::ui_rem(FRAME_CHECKBOX_SIZE))
        .h(theme::ui_rem(FRAME_CHECKBOX_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::ui_rem(theme::RADIUS_XS))
        .bg(if active {
            color(palette.border_subtle)
        } else {
            color(palette.transparent)
        });

    if indeterminate {
        mark = mark.child(
            div()
                .w(theme::ui_rem(FRAME_CHECKBOX_MARK_SIZE))
                .h(theme::ui_rem(2.0))
                .rounded(theme::ui_rem(theme::RADIUS_XS))
                .bg(color(palette.text_primary)),
        );
    } else if checked {
        mark = mark.child(icon_svg(
            assets::ICON_CHECK,
            FRAME_CHECK_ICON_SIZE,
            color(palette.text_primary),
        ));
    }

    div()
        .w(theme::ui_rem(FRAME_CHECKBOX_SIZE))
        .h(theme::ui_rem(FRAME_CHECKBOX_SIZE))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::ui_rem(theme::RADIUS_XS))
        .bg(color(palette.canvas))
        .opacity(if disabled { 0.5 } else { 1.0 })
        .shadow(input_highlight_shadows(palette))
        .child(mark)
}
