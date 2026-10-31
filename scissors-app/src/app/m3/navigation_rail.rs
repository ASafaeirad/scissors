use super::{
    App, ClickHandler, ColorScheme, IconButton, Interaction, InteractiveElement, M3Colors, M3Text,
    SelectHandler, Styled, Symbol, Window, apply_accessible, color, div, m3_icon, theme,
};
use crate::material::{shape, typography};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement,
    prelude::FluentBuilder,
};
use std::rc::Rc;

pub(in crate::app) const M3_NAVIGATION_RAIL_WIDTH: f32 = 80.0;
const RAIL_PADDING_TOP: f32 = 44.0;
const RAIL_PADDING_BOTTOM: f32 = 56.0;
const RAIL_HEADER_GAP: f32 = 40.0;
const RAIL_ITEM_GAP: f32 = 4.0;
const INDICATOR_WIDTH: f32 = 56.0;
const INDICATOR_HEIGHT: f32 = 32.0;
const RAIL_ICON_SIZE: f32 = 24.0;

#[derive(Clone, Debug)]
pub(in crate::app) struct NavigationItem {
    pub(in crate::app) icon: Symbol,
    pub(in crate::app) label: SharedString,
}

impl NavigationItem {
    pub(in crate::app) fn new(icon: Symbol, label: impl Into<SharedString>) -> Self {
        Self {
            icon,
            label: label.into(),
        }
    }
}

/// Indicator colors and label color for a destination.
pub(in crate::app) const fn navigation_item_colors(
    active: bool,
    scheme: &ColorScheme,
) -> (M3Colors, crate::theme::RgbaToken) {
    if active {
        (
            M3Colors::new(scheme.secondary_container, scheme.on_secondary_container),
            scheme.secondary,
        )
    } else {
        (
            M3Colors::new(scheme.transparent, scheme.on_surface_variant),
            scheme.on_surface_variant,
        )
    }
}

/// An M3 navigation rail: 3 to 7 destinations stacked down the side of the window, with an
/// optional menu button on top. The active destination gets a pill indicator and a filled icon.
pub(in crate::app) struct NavigationRail {
    id: SharedString,
    items: Vec<NavigationItem>,
    selected: usize,
    on_select: Option<SelectHandler>,
    menu: Option<ClickHandler>,
    focus: Vec<FocusHandle>,
}

impl NavigationRail {
    pub(in crate::app) fn new(id: impl Into<SharedString>, items: Vec<NavigationItem>) -> Self {
        Self {
            id: id.into(),
            items,
            selected: 0,
            on_select: None,
            menu: None,
            focus: Vec::new(),
        }
    }

    pub(in crate::app) const fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    pub(in crate::app) fn on_select(
        mut self,
        handler: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Shows a menu icon button above the destinations.
    pub(in crate::app) fn menu(
        mut self,
        handler: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.menu = Some(Rc::new(handler));
        self
    }

    /// Focus handles for the destinations, in order.
    pub(in crate::app) fn focus(mut self, handles: &[FocusHandle]) -> Self {
        self.focus = handles.to_vec();
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
            items,
            selected,
            on_select,
            menu,
            focus,
        } = self;
        let menu_button = menu.map(|handler| {
            IconButton::new(format!("{id}-menu"), Symbol::Menu, "Menu")
                .build(scheme, window, cx)
                .on_click(move |event, window, cx| handler(event, window, cx))
        });
        let destinations: Vec<_> = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                navigation_destination(
                    SharedString::from(format!("{id}-destination-{index}")),
                    item,
                    index,
                    index == selected,
                    on_select.clone(),
                    focus.get(index),
                    scheme,
                    window,
                    cx,
                )
            })
            .collect();

        let destinations_id = format!("{id}-destinations");
        div()
            .id(id)
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .w(theme::ui_rem(M3_NAVIGATION_RAIL_WIDTH))
            .h_full()
            .pt(theme::ui_rem(RAIL_PADDING_TOP))
            .pb(theme::ui_rem(RAIL_PADDING_BOTTOM))
            .bg(color(scheme.surface))
            .role(Role::Navigation)
            .when_some(menu_button, |this, menu_button| {
                this.child(div().mb(theme::ui_rem(RAIL_HEADER_GAP)).child(menu_button))
            })
            .child(
                div()
                    .id(destinations_id)
                    .flex()
                    .flex_col()
                    .gap(theme::ui_rem(RAIL_ITEM_GAP))
                    .role(Role::TabList)
                    .children(destinations),
            )
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Destinations take their item state plus the explicit GPUI render context."
)]
fn navigation_destination(
    id: SharedString,
    item: NavigationItem,
    index: usize,
    active: bool,
    on_select: Option<SelectHandler>,
    focus: Option<&FocusHandle>,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let (indicator, label_color) = navigation_item_colors(active, scheme);
    let interaction = Interaction::new(&id, window, cx);
    let icon_color = color(indicator.content);
    let pressed = indicator.pressed();
    let group = id.clone();
    let indicator_id = format!("{id}-indicator");

    let destination = div()
        .id(id)
        .group(group.clone())
        .flex()
        .flex_col()
        .items_center()
        .gap(theme::ui_rem(4.0))
        .w(theme::ui_rem(M3_NAVIGATION_RAIL_WIDTH))
        .pb(theme::ui_rem(4.0))
        .cursor_pointer()
        .child(
            div()
                .id(indicator_id)
                .flex()
                .items_center()
                .justify_center()
                .w(theme::ui_rem(INDICATOR_WIDTH))
                .h(theme::ui_rem(INDICATOR_HEIGHT))
                .rounded(theme::ui_rem(shape::CORNER_FULL))
                .bg(interaction.container(indicator, true))
                .group_active(group, move |style| style.bg(color(pressed)))
                .child(m3_icon(item.icon, active, RAIL_ICON_SIZE, icon_color)),
        )
        .child(
            div()
                .m3_type(typography::LABEL_MEDIUM)
                .text_color(color(label_color))
                .whitespace_nowrap()
                .child(item.label.clone()),
        )
        .when_some(on_select, |this, on_select| {
            this.on_click(move |_, window, cx| on_select(&index, window, cx))
        });

    let destination = interaction.apply(destination, true);
    apply_accessible(destination, Role::Tab, item.label, true, focus, scheme).aria_selected(active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn active_destination_gets_secondary_indicator_and_label() {
        let scheme = material::scheme(ColorTheme::Dark);
        let (indicator, label) = navigation_item_colors(true, scheme);
        assert_eq!(indicator.container, scheme.secondary_container);
        assert_eq!(indicator.content, scheme.on_secondary_container);
        assert_eq!(label, scheme.secondary);
    }

    #[test]
    fn inactive_destination_has_no_indicator() {
        let scheme = material::scheme(ColorTheme::Light);
        let (indicator, label) = navigation_item_colors(false, scheme);
        assert_eq!(indicator.container, scheme.transparent);
        assert_eq!(label, scheme.on_surface_variant);
    }

    #[test]
    fn indicator_fits_inside_the_rail() {
        const { assert!(INDICATOR_WIDTH < M3_NAVIGATION_RAIL_WIDTH) };
    }
}
