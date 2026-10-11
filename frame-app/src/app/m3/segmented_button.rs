use super::{
    App, ColorScheme, Interaction, InteractiveElement, M3Colors, M3Text, SelectHandler, Styled,
    Symbol, Window, apply_accessible, color, div, m3_icon, theme,
};
use crate::material::{state, typography};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement, Toggled,
    prelude::FluentBuilder, px,
};
use std::rc::Rc;

const SEGMENTED_HEIGHT: f32 = 40.0;
const SEGMENT_MIN_WIDTH: f32 = 48.0;
const SEGMENT_PADDING: f32 = 12.0;
const SEGMENT_EDGE_PADDING: f32 = 16.0;
const SEGMENT_GAP: f32 = 8.0;
const SEGMENT_ICON_SIZE: f32 = 18.0;
/// Outer corner of the end segments, inside the 1px container border.
const SEGMENT_END_CORNER: f32 = SEGMENTED_HEIGHT / 2.0 - 1.0;

pub(in crate::app) const fn segment_colors(
    selected: bool,
    enabled: bool,
    scheme: &ColorScheme,
) -> M3Colors {
    match (selected, enabled) {
        (_, false) => M3Colors::new(
            scheme.transparent,
            state::disabled_content(scheme.on_surface),
        ),
        (true, true) => M3Colors::new(scheme.secondary_container, scheme.on_secondary_container),
        (false, true) => M3Colors::new(scheme.transparent, scheme.on_surface),
    }
}

/// An M3 single-select segmented button: 2 to 5 outlined segments in a pill. The selected
/// segment fills with `secondary_container` and shows a check mark.
pub(in crate::app) struct SegmentedButton {
    id: SharedString,
    label: SharedString,
    options: Vec<SharedString>,
    selected: Option<usize>,
    enabled: bool,
    on_select: Option<SelectHandler>,
    focus: Vec<FocusHandle>,
}

impl SegmentedButton {
    /// `label` names the group for assistive tech.
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected: None,
            enabled: true,
            on_select: None,
            focus: Vec::new(),
        }
    }

    pub(in crate::app) const fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(in crate::app) fn on_select(
        mut self,
        handler: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Focus handles for the segments, in order. Segments without one get an anonymous handle.
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
            label,
            options,
            selected,
            enabled,
            on_select,
            focus,
        } = self;
        let count = options.len();
        let segments: Vec<_> = options
            .into_iter()
            .enumerate()
            .map(|(index, option)| {
                let segment = Segment {
                    id: SharedString::from(format!("{id}-segment-{index}")),
                    label: option,
                    index,
                    count,
                    selected: selected == Some(index),
                    enabled,
                };
                segment.build(on_select.clone(), focus.get(index), scheme, window, cx)
            })
            .collect();

        div()
            .id(id)
            .flex()
            .w_full()
            .h(theme::ui_rem(SEGMENTED_HEIGHT))
            .rounded(theme::ui_rem(SEGMENTED_HEIGHT / 2.0))
            .border_1()
            .border_color(color(if enabled {
                scheme.outline
            } else {
                state::disabled_container(scheme.on_surface)
            }))
            .role(Role::RadioGroup)
            .aria_label(label)
            .children(segments)
    }
}

struct Segment {
    id: SharedString,
    label: SharedString,
    index: usize,
    count: usize,
    selected: bool,
    enabled: bool,
}

impl Segment {
    fn build(
        self,
        on_select: Option<SelectHandler>,
        focus: Option<&FocusHandle>,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let Self {
            id,
            label,
            index,
            count,
            selected,
            enabled,
        } = self;
        let first = index == 0;
        let last = index + 1 == count;
        let colors = segment_colors(selected, enabled, scheme);
        let interaction = Interaction::new(&id, window, cx);
        let content = color(colors.content);
        let pressed = colors.pressed();

        let segment = div()
            .id(id)
            .flex_1()
            .min_w(theme::ui_rem(SEGMENT_MIN_WIDTH))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .gap(theme::ui_rem(SEGMENT_GAP))
            .pl(theme::ui_rem(if first {
                SEGMENT_EDGE_PADDING
            } else {
                SEGMENT_PADDING
            }))
            .pr(theme::ui_rem(if last {
                SEGMENT_EDGE_PADDING
            } else {
                SEGMENT_PADDING
            }))
            .when(first, |this| {
                this.rounded_l(theme::ui_rem(SEGMENT_END_CORNER))
            })
            .when(last, |this| {
                this.rounded_r(theme::ui_rem(SEGMENT_END_CORNER))
            })
            .when(!first, |this| {
                this.border_l(px(1.0)).border_color(color(if enabled {
                    scheme.outline
                } else {
                    state::disabled_container(scheme.on_surface)
                }))
            })
            .bg(interaction.container(colors, enabled))
            .text_color(content)
            .m3_type(typography::LABEL_LARGE)
            .whitespace_nowrap()
            .when(selected, |this| {
                this.child(m3_icon(Symbol::Check, false, SEGMENT_ICON_SIZE, content))
            })
            .child(label.clone())
            .when(enabled, |this| {
                this.cursor_pointer()
                    .active(move |style| style.bg(color(pressed)))
            })
            .when_some(on_select.filter(|_| enabled), |this, on_select| {
                this.on_click(move |_, window, cx| on_select(&index, window, cx))
            });

        let segment = interaction.apply(segment, enabled);
        apply_accessible(segment, Role::RadioButton, label, enabled, focus, scheme)
            .aria_toggled(Toggled::from(selected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn selected_segment_fills_with_secondary_container() {
        let scheme = material::scheme(ColorTheme::Dark);
        let on = segment_colors(true, true, scheme);
        let off = segment_colors(false, true, scheme);
        assert_eq!(on.container, scheme.secondary_container);
        assert_eq!(on.content, scheme.on_secondary_container);
        assert_eq!(off.container, scheme.transparent);
        assert_eq!(off.content, scheme.on_surface);
    }

    #[test]
    fn disabled_segments_dim_content_only() {
        let scheme = material::scheme(ColorTheme::Light);
        let disabled = segment_colors(true, false, scheme);
        assert_eq!(disabled.container, scheme.transparent);
        assert_eq!(disabled.content, scheme.on_surface.with_alpha(0.38));
    }
}
