use super::*;
use crate::app::m3::{self, M3Text};
use crate::material::{ColorScheme, shape, spacing, typography};

pub(super) fn filter_section(label: &'static str, scheme: &ColorScheme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(spacing::SPACE_3))
        .child(
            div()
                .m3_type(typography::LABEL_MEDIUM)
                .text_color(color(scheme.primary))
                .child(label),
        )
}

pub(super) fn filter_value(value: String, scheme: &ColorScheme) -> gpui::Div {
    div()
        .px(theme::ui_rem(spacing::SPACE_2))
        .py(theme::ui_rem(spacing::SPACE_1))
        .rounded(theme::ui_rem(shape::CORNER_SMALL))
        .bg(color(scheme.surface_container_highest))
        .text_color(color(scheme.on_surface_variant))
        .m3_type(typography::LABEL_SMALL)
        .child(value)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Checkbox rows share the filter label, availability, and update handler."
)]
pub(super) fn filter_checkbox(
    id: impl Into<gpui::SharedString>,
    label: &'static str,
    description: &'static str,
    checked: bool,
    disabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
    handler: impl Fn(&mut FrameRoot, &ClickEvent, &mut Window, &mut Context<FrameRoot>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    m3::Checkbox::new(id, label, checked)
        .enabled(!disabled)
        .build(scheme, window, cx)
        .when(!description.is_empty(), |row| {
            row.flex_wrap().child(
                div()
                    .w_full()
                    .m3_type(typography::BODY_SMALL)
                    .child(description),
            )
        })
        .on_click(cx.listener(move |root, event, window, cx| {
            if !disabled {
                handler(root, event, window, cx);
            }
        }))
}
