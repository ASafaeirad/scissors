//! Material Design 3 components for GPUI, built on the tokens in [`crate::material`].
//!
//! These live alongside the Scissors components in `app::components` so screens can migrate one
//! control at a time. Each component takes the M3 [`ColorScheme`] for the active theme
//! (`m3::scheme(color_theme, cx)` for the active accent).
//!
//! Single-target controls (buttons, chips, switches) return a `Stateful<Div>` and the caller
//! attaches `.on_click(...)`, the same way as the Scissors components. Multi-target controls
//! (segmented buttons, navigation rail) take an `on_select` handler that receives the chosen
//! index. Its signature matches `cx.listener(|this, index: &usize, window, cx| ...)`.
//!
//! What is not ported: ripples (M3 allows state layers alone), letter spacing (GPUI has no
//! tracking control), and animated shape morphs. Pressed corners switch without a transition.
#![expect(
    dead_code,
    reason = "The M3 component API is broader than its current callers; screens adopt variants and builder options as they migrate."
)]

use super::{
    App, BoxShadow, InteractiveElement, Rgba, StatefulInteractiveElement, Styled, Window, color,
    div, mix_color, motion_target, point, primitives, px, set_motion_target, svg, theme,
};
use crate::material::{
    ColorScheme, Elevation, Symbol, TypeStyle, motion::CubicBezier, state, typography,
};
use gpui::{FocusHandle, Role, SharedString, Svg};
use std::time::Duration;

mod button;
mod card;
mod checkbox;
mod chip;
mod color_swatch;
mod dialog;
mod gallery;
mod icon_button;
mod list;
mod menu;
mod navigation_rail;
mod progress;
mod radio;
mod scrollbar;
mod segmented_button;
mod slider;
mod switch;
mod text_field;
mod toolbar;

pub(in crate::app) use button::*;
pub(in crate::app) use card::*;
pub(in crate::app) use checkbox::*;
pub(in crate::app) use chip::*;
pub(in crate::app) use color_swatch::*;
pub(in crate::app) use dialog::*;
pub(in crate::app) use gallery::M3GalleryState;
pub(in crate::app) use icon_button::*;
pub(in crate::app) use list::*;
pub(in crate::app) use menu::*;
pub(in crate::app) use navigation_rail::*;
pub(in crate::app) use progress::*;
pub(in crate::app) use radio::*;
pub(in crate::app) use scrollbar::*;
pub(in crate::app) use segmented_button::*;
pub(in crate::app) use slider::*;
pub(in crate::app) use switch::*;
pub(in crate::app) use text_field::*;
pub(in crate::app) use toolbar::*;

/// Handler for multi-target controls. Receives the selected item's index.
pub(in crate::app) type SelectHandler = std::rc::Rc<dyn Fn(&usize, &mut Window, &mut App)>;
/// Click handler stored by a component for one of its inner targets.
pub(in crate::app) type ClickHandler =
    std::rc::Rc<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)>;

/// Pressed buttons and icon buttons square up to this radius (M3 Expressive shape morph).
const PRESSED_CORNER: f32 = crate::material::shape::CORNER_SMALL;

/// Container and content colors for one component state, plus an optional 1px outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct M3Colors {
    pub(in crate::app) container: theme::RgbaToken,
    pub(in crate::app) content: theme::RgbaToken,
    pub(in crate::app) outline: Option<theme::RgbaToken>,
}

impl M3Colors {
    const fn new(container: theme::RgbaToken, content: theme::RgbaToken) -> Self {
        Self {
            container,
            content,
            outline: None,
        }
    }

    const fn outlined(mut self, outline: theme::RgbaToken) -> Self {
        self.outline = Some(outline);
        self
    }

    fn hovered(self) -> theme::RgbaToken {
        state::state_layer(self.container, self.content, state::HOVER_OPACITY)
    }

    fn pressed(self) -> theme::RgbaToken {
        state::state_layer(self.container, self.content, state::PRESSED_OPACITY)
    }
}

/// Cached color roles shared by views in the GPUI application.
struct ActiveColorSchemes {
    accent: crate::appearance::AccentColor,
    light: ColorScheme,
    dark: ColorScheme,
}

impl gpui::Global for ActiveColorSchemes {}

pub(in crate::app) fn update_color_schemes(accent: crate::appearance::AccentColor, cx: &mut App) {
    if cx
        .try_global::<ActiveColorSchemes>()
        .is_some_and(|current| current.accent == accent)
    {
        return;
    }
    cx.set_global(ActiveColorSchemes {
        accent,
        light: crate::material::scheme_from_accent(crate::appearance::ColorTheme::Light, accent),
        dark: crate::material::scheme_from_accent(crate::appearance::ColorTheme::Dark, accent),
    });
}

pub(in crate::app) fn scheme(theme: crate::appearance::ColorTheme, cx: &App) -> ColorScheme {
    cx.try_global::<ActiveColorSchemes>().map_or_else(
        || *crate::material::scheme(theme),
        |schemes| match theme {
            crate::appearance::ColorTheme::Light => schemes.light,
            crate::appearance::ColorTheme::Dark => schemes.dark,
        },
    )
}

/// Sets the M3 typeface, size, line height and weight for a type scale role.
pub(in crate::app) trait M3Text: Styled + Sized {
    fn m3_type(self, style: TypeStyle) -> Self {
        self.font_family(typography::TYPEFACE)
            .text_size(theme::ui_rem(style.size))
            .line_height(theme::ui_rem(style.line_height))
            .font_weight(style.weight)
    }
}

impl<T: Styled> M3Text for T {}

/// A Material Symbol tinted with `icon_color`. `size` is in logical pixels.
pub(in crate::app) fn m3_icon(symbol: Symbol, filled: bool, size: f32, icon_color: Rgba) -> Svg {
    svg()
        .path(symbol.path_for(filled))
        .flex_none()
        .w(theme::ui_rem(size))
        .h(theme::ui_rem(size))
        .text_color(icon_color)
}

/// Shadows for an elevation level, drawn in the scheme's `shadow` role.
pub(in crate::app) fn elevation_shadows(level: Elevation, scheme: &ColorScheme) -> Vec<BoxShadow> {
    level
        .layers()
        .iter()
        .map(|layer| BoxShadow {
            color: color(scheme.shadow.with_alpha(layer.alpha)).into(),
            offset: point(px(0.0), px(layer.offset_y)),
            blur_radius: px(layer.blur),
            spread_radius: px(layer.spread),
            inset: false,
        })
        .collect()
}

/// A 1px inset outline. Drawn as a shadow so it doesn't take layout space, like the CSS
/// `box-shadow: inset 0 0 0 1px` the design system uses.
fn outline_shadow(outline: theme::RgbaToken) -> BoxShadow {
    BoxShadow {
        color: color(outline).into(),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(1.0),
        inset: true,
    }
}

fn surface_shadows(
    level: Elevation,
    outline: Option<theme::RgbaToken>,
    scheme: &ColorScheme,
) -> Vec<BoxShadow> {
    let mut shadows = elevation_shadows(level, scheme);
    shadows.extend(outline.map(outline_shadow));
    shadows
}

/// Hover progress plus the press tracking shared with the Scissors buttons.
struct Interaction {
    hover: f32,
    motion: primitives::ButtonMotion,
}

impl Interaction {
    fn new(id: &str, window: &mut Window, cx: &mut App) -> Self {
        let motion = primitives::button_motion(id, window, cx);
        let hover = *motion.hover_transition.evaluate(window, cx);
        Self { hover, motion }
    }

    /// The container with the hover state layer faded in.
    fn container(&self, colors: M3Colors, enabled: bool) -> Rgba {
        let target = if enabled {
            colors.hovered()
        } else {
            colors.container
        };
        mix_color(colors.container, target, self.hover)
    }

    fn apply(self, element: gpui::Stateful<gpui::Div>, enabled: bool) -> gpui::Stateful<gpui::Div> {
        primitives::apply_button_motion(element, self.motion, enabled)
    }
}

/// Keyboard focus and disabled semantics with the M3 focus indicator: a 3px `secondary` ring.
fn apply_focus(
    element: gpui::Stateful<gpui::Div>,
    enabled: bool,
    focus: Option<&FocusHandle>,
    scheme: &ColorScheme,
) -> gpui::Stateful<gpui::Div> {
    let element = match focus {
        Some(focus) => element.track_focus(focus),
        None if enabled => element.focusable(),
        None => element,
    };
    let secondary = scheme.secondary;
    if enabled {
        element.tab_stop(true).focus_visible(move |style| {
            style
                .ring_width(px(state::FOCUS_INDICATOR_THICKNESS))
                .ring_color(color(secondary))
        })
    } else {
        element.tab_stop(false).aria_disabled(true)
    }
}

fn apply_accessible(
    element: gpui::Stateful<gpui::Div>,
    role: Role,
    label: impl Into<SharedString>,
    enabled: bool,
    focus: Option<&FocusHandle>,
    scheme: &ColorScheme,
) -> gpui::Stateful<gpui::Div> {
    apply_focus(element.role(role).aria_label(label), enabled, focus, scheme)
}

/// Animates toward `on` with an M3 duration and easing, starting settled on the first render.
/// The result may overshoot `0..=1` when `easing` is a spring.
fn m3_transition(
    key: impl Into<gpui::ElementId>,
    on: bool,
    duration: Duration,
    easing: CubicBezier,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let target = motion_target(on);
    let transition = window
        .use_keyed_transition(key, cx, duration, move |_window, _cx| target)
        .with_easing(move |progress| easing.ease(progress));
    set_motion_target(&transition, target, cx);
    *transition.evaluate(window, cx)
}

/// Linear interpolation that, unlike `mix_scalar`, keeps spring overshoot.
fn lerp_unclamped(from: f32, to: f32, progress: f32) -> f32 {
    (to - from).mul_add(progress, from)
}

/// An inert block of M3 surface color, for panels and cards that only need the container role.
pub(in crate::app) fn m3_surface(container: theme::RgbaToken, corner: f32) -> gpui::Div {
    div().bg(color(container)).rounded(theme::ui_rem(corner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::ColorTheme;
    use crate::material;

    #[test]
    fn hover_and_pressed_layers_tint_toward_content() {
        let scheme = material::scheme(ColorTheme::Dark);
        let colors = M3Colors::new(scheme.primary, scheme.on_primary);
        assert_ne!(colors.hovered(), colors.container);
        assert_ne!(colors.pressed(), colors.container);
        assert!((colors.hovered().alpha - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn elevation_shadows_follow_level_layers() {
        let scheme = material::scheme(ColorTheme::Light);
        assert!(elevation_shadows(Elevation::Level0, scheme).is_empty());
        assert_eq!(elevation_shadows(Elevation::Level3, scheme).len(), 2);
    }

    #[test]
    fn surface_shadows_append_the_outline_last() {
        let scheme = material::scheme(ColorTheme::Light);
        let shadows = surface_shadows(Elevation::Level1, Some(scheme.outline), scheme);
        assert_eq!(shadows.len(), 3);
        assert!(shadows[2].inset);
    }

    #[test]
    fn unclamped_lerp_keeps_overshoot() {
        assert!((lerp_unclamped(0.0, 10.0, 1.1) - 11.0).abs() < 1e-5);
    }
}
