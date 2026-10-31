//! Material 3 motion: easing curves and duration tokens.
//!
//! The M3 Expressive spring tokens are approximated as overshooting cubic beziers, the same way
//! the design system's CSS does it. Pass an easing to GPUI with
//! `.with_easing(|t| motion::STANDARD.ease(t))`.

use std::time::Duration;

/// A CSS-style `cubic-bezier(x1, y1, x2, y2)` timing curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    #[must_use]
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Maps linear progress `t` in `0..=1` to eased progress. Spring curves overshoot past 1.
    #[must_use]
    pub fn ease(self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let parameter = self.parameter_for_x(t);
        bezier(parameter, self.y1, self.y2)
    }

    fn parameter_for_x(self, x: f32) -> f32 {
        // Newton-Raphson converges in a few steps for well-behaved curves. Bisection is the
        // fallback when the slope is too flat to trust.
        let mut parameter = x;
        for _ in 0..8 {
            let error = bezier(parameter, self.x1, self.x2) - x;
            if error.abs() < 1e-5 {
                return parameter;
            }
            let slope = bezier_slope(parameter, self.x1, self.x2);
            if slope.abs() < 1e-6 {
                break;
            }
            parameter -= error / slope;
        }

        let (mut low, mut high) = (0.0_f32, 1.0_f32);
        parameter = x;
        for _ in 0..32 {
            let value = bezier(parameter, self.x1, self.x2);
            if (value - x).abs() < 1e-5 {
                break;
            }
            if value < x {
                low = parameter;
            } else {
                high = parameter;
            }
            parameter = (low + high) * 0.5;
        }
        parameter
    }
}

/// One axis of a cubic bezier with endpoints fixed at 0 and 1.
fn bezier(t: f32, p1: f32, p2: f32) -> f32 {
    let inverse = 1.0 - t;
    (3.0 * inverse * inverse * t).mul_add(p1, (3.0 * inverse * t * t).mul_add(p2, t * t * t))
}

fn bezier_slope(t: f32, p1: f32, p2: f32) -> f32 {
    let inverse = 1.0 - t;
    (3.0 * inverse * inverse).mul_add(
        p1,
        (6.0 * inverse * t).mul_add(p2 - p1, 3.0 * t * t * (1.0 - p2)),
    )
}

pub const LINEAR: CubicBezier = CubicBezier::new(0.0, 0.0, 1.0, 1.0);
pub const STANDARD: CubicBezier = CubicBezier::new(0.2, 0.0, 0.0, 1.0);
pub const STANDARD_ACCELERATE: CubicBezier = CubicBezier::new(0.3, 0.0, 1.0, 1.0);
pub const STANDARD_DECELERATE: CubicBezier = CubicBezier::new(0.0, 0.0, 0.0, 1.0);
pub const EMPHASIZED: CubicBezier = CubicBezier::new(0.2, 0.0, 0.0, 1.0);
pub const EMPHASIZED_ACCELERATE: CubicBezier = CubicBezier::new(0.3, 0.0, 0.8, 0.15);
pub const EMPHASIZED_DECELERATE: CubicBezier = CubicBezier::new(0.05, 0.7, 0.1, 1.0);
pub const LEGACY: CubicBezier = CubicBezier::new(0.4, 0.0, 0.2, 1.0);
pub const SPRING_FAST_SPATIAL: CubicBezier = CubicBezier::new(0.42, 1.67, 0.21, 0.9);
pub const SPRING_DEFAULT_SPATIAL: CubicBezier = CubicBezier::new(0.38, 1.21, 0.22, 1.0);
pub const SPRING_FAST_EFFECTS: CubicBezier = CubicBezier::new(0.31, 0.94, 0.34, 1.0);
pub const SPRING_DEFAULT_EFFECTS: CubicBezier = CubicBezier::new(0.34, 0.8, 0.34, 1.0);

pub const SHORT_1: Duration = Duration::from_millis(50);
pub const SHORT_2: Duration = Duration::from_millis(100);
pub const SHORT_3: Duration = Duration::from_millis(150);
pub const SHORT_4: Duration = Duration::from_millis(200);
pub const MEDIUM_1: Duration = Duration::from_millis(250);
pub const MEDIUM_2: Duration = Duration::from_millis(300);
pub const MEDIUM_3: Duration = Duration::from_millis(350);
pub const MEDIUM_4: Duration = Duration::from_millis(400);
pub const LONG_1: Duration = Duration::from_millis(450);
pub const LONG_2: Duration = Duration::from_millis(500);
pub const LONG_3: Duration = Duration::from_millis(550);
pub const LONG_4: Duration = Duration::from_millis(600);
pub const EXTRA_LONG_1: Duration = Duration::from_millis(700);
pub const EXTRA_LONG_2: Duration = Duration::from_millis(800);
pub const EXTRA_LONG_3: Duration = Duration::from_millis(900);
pub const EXTRA_LONG_4: Duration = Duration::from_secs(1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curve_starts_at_zero_and_ends_at_one() {
        for curve in [
            LINEAR,
            STANDARD,
            STANDARD_ACCELERATE,
            STANDARD_DECELERATE,
            EMPHASIZED_ACCELERATE,
            EMPHASIZED_DECELERATE,
            LEGACY,
            SPRING_FAST_SPATIAL,
            SPRING_DEFAULT_SPATIAL,
            SPRING_FAST_EFFECTS,
            SPRING_DEFAULT_EFFECTS,
        ] {
            assert!(curve.ease(0.0).abs() < f32::EPSILON, "{curve:?}");
            assert!((curve.ease(1.0) - 1.0).abs() < f32::EPSILON, "{curve:?}");
        }
    }

    #[test]
    fn linear_curve_is_identity() {
        for step in 0_u8..=10 {
            let t = f32::from(step) / 10.0;
            assert!((LINEAR.ease(t) - t).abs() < 1e-4, "t={t}");
        }
    }

    #[test]
    fn standard_curve_front_loads_progress() {
        assert!(STANDARD.ease(0.5) > 0.5);
        assert!(STANDARD_ACCELERATE.ease(0.5) < 0.5);
    }

    #[test]
    fn spatial_spring_overshoots_before_settling() {
        let peak = (1..100)
            .map(|step: u8| SPRING_FAST_SPATIAL.ease(f32::from(step) / 100.0))
            .fold(f32::MIN, f32::max);
        assert!(peak > 1.0, "peak was {peak}");
    }

    #[test]
    fn curve_matches_a_known_css_sample() {
        // Browser cubic-bezier(.4,0,.2,1) at t=0.5 is ~0.7755.
        assert!((LEGACY.ease(0.5) - 0.7755).abs() < 2e-3);
    }
}
