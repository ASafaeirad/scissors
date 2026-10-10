use super::{
    App, ColorScheme, Interaction, InteractiveElement, Styled, Symbol, Window, apply_accessible,
    div, lerp_unclamped, m3_icon, m3_transition, mix_color, theme,
};
use crate::material::{motion, shape, state};
use gpui::{
    FocusHandle, ParentElement, Role, SharedString, StatefulInteractiveElement, Toggled,
    prelude::FluentBuilder,
};

const TRACK_WIDTH: f32 = 52.0;
const TRACK_HEIGHT: f32 = 32.0;
const TRACK_BORDER: f32 = 2.0;
const HANDLE_OFF: f32 = 16.0;
const HANDLE_ON: f32 = 24.0;
const HANDLE_PRESSED: f32 = 28.0;
const HANDLE_ICON: f32 = 16.0;
/// Hover state layer drawn around the handle.
const HANDLE_TARGET: f32 = 40.0;
/// Handle center measured from the track's outer left edge, off and on.
const HANDLE_CENTER_OFF: f32 = TRACK_HEIGHT / 2.0;
const HANDLE_CENTER_ON: f32 = TRACK_WIDTH - TRACK_HEIGHT / 2.0;
/// Absolute children are placed inside the border, so outer-edge offsets subtract it.
const HANDLE_TARGET_TOP: f32 = (TRACK_HEIGHT - HANDLE_TARGET) / 2.0 - TRACK_BORDER;

#[derive(Clone, Copy, Debug, PartialEq)]
struct SwitchColors {
    track: crate::theme::RgbaToken,
    track_outline: crate::theme::RgbaToken,
    handle: crate::theme::RgbaToken,
    icon: crate::theme::RgbaToken,
    state_layer: crate::theme::RgbaToken,
}

const fn switch_colors(checked: bool, scheme: &ColorScheme) -> SwitchColors {
    if checked {
        SwitchColors {
            track: scheme.primary,
            track_outline: scheme.primary,
            handle: scheme.on_primary,
            icon: scheme.on_primary_container,
            state_layer: scheme.primary,
        }
    } else {
        SwitchColors {
            track: scheme.surface_container_highest,
            track_outline: scheme.outline,
            handle: scheme.outline,
            icon: scheme.surface_container_highest,
            state_layer: scheme.on_surface,
        }
    }
}

/// An M3 switch: a 52×32 track whose handle grows and slides with a spring when toggled.
pub(in crate::app) struct Switch {
    id: SharedString,
    label: SharedString,
    checked: bool,
    icons: bool,
    enabled: bool,
    focus: Option<FocusHandle>,
}

impl Switch {
    /// `label` is announced to assistive tech. Render any visible label next to the switch.
    pub(in crate::app) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
            icons: false,
            enabled: true,
            focus: None,
        }
    }

    /// Shows a check (on) or close (off) icon in the handle. The off handle grows to 24px.
    pub(in crate::app) const fn icons(mut self, icons: bool) -> Self {
        self.icons = icons;
        self
    }

    pub(in crate::app) const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(in crate::app) fn focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub(in crate::app) fn build(
        self,
        scheme: &'static ColorScheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let Self {
            id,
            label,
            checked,
            icons,
            enabled,
            focus,
        } = self;
        let off = switch_colors(false, scheme);
        let on = switch_colors(true, scheme);
        let spatial = m3_transition(
            format!("{id}-handle-motion"),
            checked,
            motion::MEDIUM_2,
            motion::SPRING_FAST_SPATIAL,
            window,
            cx,
        );
        let fade = spatial.clamp(0.0, 1.0);
        let interaction = Interaction::new(&id, window, cx);
        let group = id.clone();

        let handle_off = if icons { HANDLE_ON } else { HANDLE_OFF };
        let handle_size = lerp_unclamped(handle_off, HANDLE_ON, spatial).max(0.0);
        let handle_center = lerp_unclamped(HANDLE_CENTER_OFF, HANDLE_CENTER_ON, spatial);
        let state_layer = mix_color(off.state_layer, on.state_layer, fade);
        let state_layer_alpha = state::HOVER_OPACITY * interaction.hover;
        let handle_icon = if checked {
            Symbol::Check
        } else {
            Symbol::Close
        };
        let icon_color = mix_color(off.icon, on.icon, fade);

        let handle = div()
            .absolute()
            .top(theme::ui_rem(HANDLE_TARGET_TOP))
            .left(theme::ui_rem(
                handle_center - HANDLE_TARGET / 2.0 - TRACK_BORDER,
            ))
            .size(theme::ui_rem(HANDLE_TARGET))
            .flex()
            .items_center()
            .justify_center()
            .rounded(theme::ui_rem(shape::CORNER_FULL))
            .bg(gpui::Rgba {
                a: state_layer.a * state_layer_alpha,
                ..state_layer
            })
            .child(
                div()
                    .id(format!("{group}-handle"))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(theme::ui_rem(handle_size))
                    .rounded(theme::ui_rem(shape::CORNER_FULL))
                    .bg(mix_color(off.handle, on.handle, fade))
                    .when(enabled, |this| {
                        this.group_active(group.clone(), |style| {
                            style.size(theme::ui_rem(HANDLE_PRESSED))
                        })
                    })
                    .when(icons, |this| {
                        this.child(m3_icon(handle_icon, false, HANDLE_ICON, icon_color))
                    }),
            );

        let switch = div()
            .id(id)
            .group(group)
            .relative()
            .flex_none()
            .w(theme::ui_rem(TRACK_WIDTH))
            .h(theme::ui_rem(TRACK_HEIGHT))
            .rounded(theme::ui_rem(TRACK_HEIGHT / 2.0))
            .bg(mix_color(off.track, on.track, fade))
            .border(theme::ui_rem(TRACK_BORDER))
            .border_color(mix_color(off.track_outline, on.track_outline, fade))
            .when(!enabled, |this| {
                this.opacity(state::DISABLED_CONTENT_OPACITY)
            })
            .when(enabled, gpui::Styled::cursor_pointer)
            .child(handle);

        let switch = interaction.apply(switch, enabled);
        apply_accessible(switch, Role::Switch, label, enabled, focus.as_ref(), scheme)
            .aria_toggled(Toggled::from(checked))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn checked_switch_fills_the_track_with_primary() {
        let scheme = material::scheme(ColorTheme::Dark);
        let on = switch_colors(true, scheme);
        assert_eq!(on.track, scheme.primary);
        assert_eq!(on.handle, scheme.on_primary);
    }

    #[test]
    fn unchecked_switch_uses_outline_handle_on_highest_container() {
        let scheme = material::scheme(ColorTheme::Light);
        let off = switch_colors(false, scheme);
        assert_eq!(off.track, scheme.surface_container_highest);
        assert_eq!(off.handle, scheme.outline);
        assert_eq!(off.track_outline, scheme.outline);
    }

    #[test]
    fn handle_travel_leaves_symmetric_insets() {
        let off_inset = HANDLE_CENTER_OFF - HANDLE_OFF / 2.0;
        let on_inset = TRACK_WIDTH - (HANDLE_CENTER_ON + HANDLE_ON / 2.0);
        assert!((off_inset - 8.0).abs() < f32::EPSILON);
        assert!((on_inset - 4.0).abs() < f32::EPSILON);
    }
}
