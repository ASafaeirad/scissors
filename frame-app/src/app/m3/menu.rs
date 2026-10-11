//! M3 menu container and keyboard-accessible action rows.
use super::{
    App, ColorScheme, InteractiveElement, M3Colors, M3Text, SelectHandler, Styled, Symbol, Window,
    apply_accessible, color, div, m3_icon, theme,
};
use crate::material::{shape, state, typography};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement,
    prelude::FluentBuilder,
};

pub(in crate::app) const fn menu_colors(enabled: bool, scheme: &ColorScheme) -> M3Colors {
    M3Colors::new(
        scheme.surface_container,
        if enabled {
            scheme.on_surface
        } else {
            state::disabled_content(scheme.on_surface)
        },
    )
}

pub(in crate::app) struct MenuItem {
    label: SharedString,
    icon: Symbol,
    enabled: bool,
    focus: Option<FocusHandle>,
}
impl MenuItem {
    pub(in crate::app) fn new(label: impl Into<SharedString>, icon: Symbol) -> Self {
        Self {
            label: label.into(),
            icon,
            enabled: true,
            focus: None,
        }
    }
    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub(in crate::app) fn focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
}

pub(in crate::app) struct Menu {
    id: SharedString,
    items: Vec<MenuItem>,
    on_select: Option<SelectHandler>,
}
impl Menu {
    pub(in crate::app) fn new(id: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        Self {
            id: id.into(),
            items,
            on_select: None,
        }
    }
    pub(in crate::app) fn on_select(
        mut self,
        handler: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(std::rc::Rc::new(handler));
        self
    }
    pub(in crate::app) fn build(
        self,
        scheme: &ColorScheme,
        _window: &mut Window,
        _cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = menu_colors(true, scheme);
        let focuses: Vec<_> = self
            .items
            .iter()
            .filter(|item| item.enabled)
            .filter_map(|item| item.focus.clone())
            .collect();
        let mut menu = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .w(theme::ui_rem(192.0))
            .py(theme::ui_rem(8.0))
            .rounded(theme::ui_rem(shape::CORNER_EXTRA_SMALL))
            .bg(color(colors.container))
            .text_color(color(colors.content));
        for (index, item) in self.items.into_iter().enumerate() {
            let colors = menu_colors(item.enabled, scheme);
            let select = self.on_select.clone();
            let key_select = select.clone();
            let item_focus = item.focus.clone();
            let key_focuses = focuses.clone();
            let row_id = format!("{}-{index}", self.id);
            let row = div()
                .id(row_id.clone())
                .debug_selector(move || row_id)
                .flex()
                .items_center()
                .h(theme::ui_rem(48.0))
                .px(theme::ui_rem(12.0))
                .gap(theme::ui_rem(12.0))
                .m3_type(typography::LABEL_LARGE)
                .text_color(color(colors.content))
                .child(m3_icon(item.icon, false, 24.0, color(colors.content)))
                .child(item.label.clone())
                .when(item.enabled, |row| {
                    row.cursor_pointer()
                        .hover(move |style| style.bg(color(colors.hovered())))
                        .active(move |style| style.bg(color(colors.pressed())))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            if let Some(select) = &select {
                                select(&index, window, cx);
                            }
                        })
                });
            let row = apply_accessible(
                row,
                Role::Button,
                item.label,
                item.enabled,
                item.focus.as_ref(),
                scheme,
            )
            .on_key_down(
                move |event, window, cx| match event.keystroke.key.as_str() {
                    "enter" | "space" if item.enabled => {
                        cx.stop_propagation();
                        if let Some(select) = &key_select {
                            select(&index, window, cx);
                        }
                    }
                    "up" | "down" | "home" | "end" if !key_focuses.is_empty() => {
                        cx.stop_propagation();
                        let current = key_focuses
                            .iter()
                            .position(|focus| Some(focus) == item_focus.as_ref())
                            .unwrap_or(0);
                        let count = key_focuses.len();
                        let target = match event.keystroke.key.as_str() {
                            "up" => (current + count - 1) % count,
                            "down" => (current + 1) % count,
                            "home" => 0,
                            _ => count - 1,
                        };
                        key_focuses[target].focus(window, cx);
                    }
                    _ => {}
                },
            );
            menu = menu.child(row);
        }
        menu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_uses_surface_roles_and_dims_disabled_actions() {
        for theme in [
            crate::appearance::ColorTheme::Light,
            crate::appearance::ColorTheme::Dark,
        ] {
            let scheme = crate::material::scheme(theme);
            let enabled = menu_colors(true, scheme);
            let disabled = menu_colors(false, scheme);
            assert_eq!(enabled.container, scheme.surface_container);
            assert_eq!(enabled.content, scheme.on_surface);
            assert_eq!(disabled.container, enabled.container);
            assert_eq!(disabled.content, state::disabled_content(scheme.on_surface));
        }
    }
}
