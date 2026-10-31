//! Material 3 interaction states.
//!
//! An interaction is shown as a translucent layer of the *content* color over the container. The
//! container color itself is never darkened or lightened.

use crate::theme::RgbaToken;

pub const HOVER_OPACITY: f32 = 0.08;
pub const FOCUS_OPACITY: f32 = 0.10;
pub const PRESSED_OPACITY: f32 = 0.10;
pub const DRAGGED_OPACITY: f32 = 0.16;
pub const DISABLED_CONTENT_OPACITY: f32 = 0.38;
pub const DISABLED_CONTAINER_OPACITY: f32 = 0.12;
/// Focus indicator ring width in logical pixels, drawn in the `secondary` role.
pub const FOCUS_INDICATOR_THICKNESS: f32 = 3.0;

/// Composites `content` at `opacity` over `container` (source-over).
///
/// A transparent container yields the content color at `opacity`, so standard icon buttons and
/// text buttons get the same state layer as filled ones.
#[must_use]
pub fn state_layer(container: RgbaToken, content: RgbaToken, opacity: f32) -> RgbaToken {
    let source_alpha = (content.alpha * opacity).clamp(0.0, 1.0);
    let backdrop_alpha = container.alpha.clamp(0.0, 1.0);
    let alpha = backdrop_alpha.mul_add(1.0 - source_alpha, source_alpha);
    if alpha <= f32::EPSILON {
        return container;
    }

    let channel = |source: f32, backdrop: f32| {
        (backdrop * backdrop_alpha).mul_add(1.0 - source_alpha, source * source_alpha) / alpha
    };
    RgbaToken {
        red: channel(content.red, container.red),
        green: channel(content.green, container.green),
        blue: channel(content.blue, container.blue),
        alpha,
    }
}

/// Disabled container: `on_surface` at 12%.
#[must_use]
pub const fn disabled_container(on_surface: RgbaToken) -> RgbaToken {
    on_surface.with_alpha(DISABLED_CONTAINER_OPACITY)
}

/// Disabled content: `on_surface` at 38%.
#[must_use]
pub const fn disabled_content(on_surface: RgbaToken) -> RgbaToken {
    on_surface.with_alpha(DISABLED_CONTENT_OPACITY)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: RgbaToken, expected: RgbaToken) -> bool {
        let tolerance = 1e-5;
        (actual.red - expected.red).abs() < tolerance
            && (actual.green - expected.green).abs() < tolerance
            && (actual.blue - expected.blue).abs() < tolerance
            && (actual.alpha - expected.alpha).abs() < tolerance
    }

    #[test]
    fn zero_opacity_leaves_the_container_unchanged() {
        let container = RgbaToken::from_rgb(103, 80, 164);
        let content = RgbaToken::from_rgb(255, 255, 255);
        assert!(close(state_layer(container, content, 0.0), container));
    }

    #[test]
    fn opaque_container_stays_opaque_and_moves_toward_content() {
        let container = RgbaToken::from_rgb(0, 0, 0);
        let content = RgbaToken::from_rgb(255, 255, 255);
        let hovered = state_layer(container, content, HOVER_OPACITY);
        assert!(close(
            hovered,
            RgbaToken {
                red: HOVER_OPACITY,
                green: HOVER_OPACITY,
                blue: HOVER_OPACITY,
                alpha: 1.0,
            }
        ));
    }

    #[test]
    fn transparent_container_yields_translucent_content() {
        let container = RgbaToken::from_rgb(0, 0, 0).with_alpha(0.0);
        let content = RgbaToken::from_rgb(202, 196, 208);
        let pressed = state_layer(container, content, PRESSED_OPACITY);
        assert!(close(pressed, content.with_alpha(PRESSED_OPACITY)));
    }

    #[test]
    fn disabled_helpers_use_spec_opacities() {
        let on_surface = RgbaToken::from_rgb(230, 224, 233);
        assert!((disabled_container(on_surface).alpha - 0.12).abs() < f32::EPSILON);
        assert!((disabled_content(on_surface).alpha - 0.38).abs() < f32::EPSILON);
    }
}
