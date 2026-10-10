//! Material 3 color: baseline tonal palettes (source color `#6750A4`) and the light and dark
//! system color schemes built from them.
//!
//! Components read roles from [`ColorScheme`]. They never read the reference palettes directly.
//! Every fill role has a paired `on_*` role for content drawn on top of it.

use crate::appearance::ColorTheme;
use crate::theme::RgbaToken;

const fn rgb(red: u8, green: u8, blue: u8) -> RgbaToken {
    RgbaToken::from_rgb(red, green, blue)
}

/// Baseline reference palettes (`--md-ref-palette-*`). The tone number is the suffix.
pub mod palette {
    use super::{RgbaToken, rgb};

    pub const PRIMARY_10: RgbaToken = rgb(0x21, 0x00, 0x5D);
    pub const PRIMARY_20: RgbaToken = rgb(0x38, 0x1E, 0x72);
    pub const PRIMARY_30: RgbaToken = rgb(0x4F, 0x37, 0x8B);
    pub const PRIMARY_40: RgbaToken = rgb(0x67, 0x50, 0xA4);
    pub const PRIMARY_80: RgbaToken = rgb(0xD0, 0xBC, 0xFF);
    pub const PRIMARY_90: RgbaToken = rgb(0xEA, 0xDD, 0xFF);
    pub const PRIMARY_100: RgbaToken = rgb(0xFF, 0xFF, 0xFF);

    pub const SECONDARY_10: RgbaToken = rgb(0x1D, 0x19, 0x2B);
    pub const SECONDARY_20: RgbaToken = rgb(0x33, 0x2D, 0x41);
    pub const SECONDARY_30: RgbaToken = rgb(0x4A, 0x44, 0x58);
    pub const SECONDARY_40: RgbaToken = rgb(0x62, 0x5B, 0x71);
    pub const SECONDARY_80: RgbaToken = rgb(0xCC, 0xC2, 0xDC);
    pub const SECONDARY_90: RgbaToken = rgb(0xE8, 0xDE, 0xF8);
    pub const SECONDARY_100: RgbaToken = rgb(0xFF, 0xFF, 0xFF);

    pub const TERTIARY_10: RgbaToken = rgb(0x31, 0x11, 0x1D);
    pub const TERTIARY_20: RgbaToken = rgb(0x49, 0x25, 0x32);
    pub const TERTIARY_30: RgbaToken = rgb(0x63, 0x3B, 0x48);
    pub const TERTIARY_40: RgbaToken = rgb(0x7D, 0x52, 0x60);
    pub const TERTIARY_80: RgbaToken = rgb(0xEF, 0xB8, 0xC8);
    pub const TERTIARY_90: RgbaToken = rgb(0xFF, 0xD8, 0xE4);
    pub const TERTIARY_100: RgbaToken = rgb(0xFF, 0xFF, 0xFF);

    pub const ERROR_20: RgbaToken = rgb(0x60, 0x14, 0x10);
    pub const ERROR_30: RgbaToken = rgb(0x8C, 0x1D, 0x18);
    pub const ERROR_40: RgbaToken = rgb(0xB3, 0x26, 0x1E);
    pub const ERROR_80: RgbaToken = rgb(0xF2, 0xB8, 0xB5);
    pub const ERROR_90: RgbaToken = rgb(0xF9, 0xDE, 0xDC);
    pub const ERROR_100: RgbaToken = rgb(0xFF, 0xFF, 0xFF);

    pub const NEUTRAL_0: RgbaToken = rgb(0x00, 0x00, 0x00);
    pub const NEUTRAL_4: RgbaToken = rgb(0x0F, 0x0D, 0x13);
    pub const NEUTRAL_6: RgbaToken = rgb(0x14, 0x12, 0x18);
    pub const NEUTRAL_10: RgbaToken = rgb(0x1D, 0x1B, 0x20);
    pub const NEUTRAL_12: RgbaToken = rgb(0x21, 0x1F, 0x26);
    pub const NEUTRAL_17: RgbaToken = rgb(0x2B, 0x29, 0x30);
    pub const NEUTRAL_20: RgbaToken = rgb(0x32, 0x2F, 0x35);
    pub const NEUTRAL_22: RgbaToken = rgb(0x36, 0x34, 0x3B);
    pub const NEUTRAL_24: RgbaToken = rgb(0x3B, 0x38, 0x3E);
    pub const NEUTRAL_87: RgbaToken = rgb(0xDE, 0xD8, 0xE1);
    pub const NEUTRAL_90: RgbaToken = rgb(0xE6, 0xE0, 0xE9);
    pub const NEUTRAL_92: RgbaToken = rgb(0xEC, 0xE6, 0xF0);
    pub const NEUTRAL_94: RgbaToken = rgb(0xF3, 0xED, 0xF7);
    pub const NEUTRAL_95: RgbaToken = rgb(0xF5, 0xEF, 0xF7);
    pub const NEUTRAL_96: RgbaToken = rgb(0xF7, 0xF2, 0xFA);
    pub const NEUTRAL_98: RgbaToken = rgb(0xFE, 0xF7, 0xFF);
    pub const NEUTRAL_100: RgbaToken = rgb(0xFF, 0xFF, 0xFF);

    pub const NEUTRAL_VARIANT_30: RgbaToken = rgb(0x49, 0x45, 0x4F);
    pub const NEUTRAL_VARIANT_50: RgbaToken = rgb(0x79, 0x74, 0x7E);
    pub const NEUTRAL_VARIANT_60: RgbaToken = rgb(0x93, 0x8F, 0x99);
    pub const NEUTRAL_VARIANT_80: RgbaToken = rgb(0xCA, 0xC4, 0xD0);
    pub const NEUTRAL_VARIANT_90: RgbaToken = rgb(0xE7, 0xE0, 0xEC);
}

/// Resolved M3 system color roles (`--md-sys-color-*`).
#[derive(Debug, PartialEq)]
pub struct ColorScheme {
    color_theme: ColorTheme,
    pub primary: RgbaToken,
    pub on_primary: RgbaToken,
    pub primary_container: RgbaToken,
    pub on_primary_container: RgbaToken,
    pub secondary: RgbaToken,
    pub on_secondary: RgbaToken,
    pub secondary_container: RgbaToken,
    pub on_secondary_container: RgbaToken,
    pub tertiary: RgbaToken,
    pub on_tertiary: RgbaToken,
    pub tertiary_container: RgbaToken,
    pub on_tertiary_container: RgbaToken,
    pub error: RgbaToken,
    pub on_error: RgbaToken,
    pub error_container: RgbaToken,
    pub on_error_container: RgbaToken,
    pub primary_fixed: RgbaToken,
    pub primary_fixed_dim: RgbaToken,
    pub on_primary_fixed: RgbaToken,
    pub on_primary_fixed_variant: RgbaToken,
    pub secondary_fixed: RgbaToken,
    pub secondary_fixed_dim: RgbaToken,
    pub on_secondary_fixed: RgbaToken,
    pub on_secondary_fixed_variant: RgbaToken,
    pub tertiary_fixed: RgbaToken,
    pub tertiary_fixed_dim: RgbaToken,
    pub on_tertiary_fixed: RgbaToken,
    pub on_tertiary_fixed_variant: RgbaToken,
    pub background: RgbaToken,
    pub on_background: RgbaToken,
    pub surface: RgbaToken,
    pub surface_dim: RgbaToken,
    pub surface_bright: RgbaToken,
    pub surface_container_lowest: RgbaToken,
    pub surface_container_low: RgbaToken,
    pub surface_container: RgbaToken,
    pub surface_container_high: RgbaToken,
    pub surface_container_highest: RgbaToken,
    pub on_surface: RgbaToken,
    pub surface_variant: RgbaToken,
    pub on_surface_variant: RgbaToken,
    pub inverse_surface: RgbaToken,
    pub inverse_on_surface: RgbaToken,
    pub inverse_primary: RgbaToken,
    pub outline: RgbaToken,
    pub outline_variant: RgbaToken,
    pub surface_tint: RgbaToken,
    pub shadow: RgbaToken,
    pub scrim: RgbaToken,
    /// Fully transparent black, for containers that have no fill (text buttons, standard icon
    /// buttons). State layers composite onto it as pure content color.
    pub transparent: RgbaToken,
}

impl ColorScheme {
    /// Returns the user-facing theme this scheme was built for.
    #[must_use]
    pub const fn color_theme(&self) -> ColorTheme {
        self.color_theme
    }

    #[must_use]
    pub const fn is_dark(&self) -> bool {
        matches!(self.color_theme, ColorTheme::Dark)
    }
}

use palette as p;

pub static LIGHT: ColorScheme = ColorScheme {
    color_theme: ColorTheme::Light,
    primary: p::PRIMARY_40,
    on_primary: p::PRIMARY_100,
    primary_container: p::PRIMARY_90,
    on_primary_container: p::PRIMARY_30,
    secondary: p::SECONDARY_40,
    on_secondary: p::SECONDARY_100,
    secondary_container: p::SECONDARY_90,
    on_secondary_container: p::SECONDARY_30,
    tertiary: p::TERTIARY_40,
    on_tertiary: p::TERTIARY_100,
    tertiary_container: p::TERTIARY_90,
    on_tertiary_container: p::TERTIARY_30,
    error: p::ERROR_40,
    on_error: p::ERROR_100,
    error_container: p::ERROR_90,
    on_error_container: p::ERROR_30,
    primary_fixed: p::PRIMARY_90,
    primary_fixed_dim: p::PRIMARY_80,
    on_primary_fixed: p::PRIMARY_10,
    on_primary_fixed_variant: p::PRIMARY_30,
    secondary_fixed: p::SECONDARY_90,
    secondary_fixed_dim: p::SECONDARY_80,
    on_secondary_fixed: p::SECONDARY_10,
    on_secondary_fixed_variant: p::SECONDARY_30,
    tertiary_fixed: p::TERTIARY_90,
    tertiary_fixed_dim: p::TERTIARY_80,
    on_tertiary_fixed: p::TERTIARY_10,
    on_tertiary_fixed_variant: p::TERTIARY_30,
    background: p::NEUTRAL_98,
    on_background: p::NEUTRAL_10,
    surface: p::NEUTRAL_98,
    surface_dim: p::NEUTRAL_87,
    surface_bright: p::NEUTRAL_98,
    surface_container_lowest: p::NEUTRAL_100,
    surface_container_low: p::NEUTRAL_96,
    surface_container: p::NEUTRAL_94,
    surface_container_high: p::NEUTRAL_92,
    surface_container_highest: p::NEUTRAL_90,
    on_surface: p::NEUTRAL_10,
    surface_variant: p::NEUTRAL_VARIANT_90,
    on_surface_variant: p::NEUTRAL_VARIANT_30,
    inverse_surface: p::NEUTRAL_20,
    inverse_on_surface: p::NEUTRAL_95,
    inverse_primary: p::PRIMARY_80,
    outline: p::NEUTRAL_VARIANT_50,
    outline_variant: p::NEUTRAL_VARIANT_80,
    surface_tint: p::PRIMARY_40,
    shadow: p::NEUTRAL_0,
    scrim: p::NEUTRAL_0,
    transparent: p::NEUTRAL_0.with_alpha(0.0),
};

pub static DARK: ColorScheme = ColorScheme {
    color_theme: ColorTheme::Dark,
    primary: p::PRIMARY_80,
    on_primary: p::PRIMARY_20,
    primary_container: p::PRIMARY_30,
    on_primary_container: p::PRIMARY_90,
    secondary: p::SECONDARY_80,
    on_secondary: p::SECONDARY_20,
    secondary_container: p::SECONDARY_30,
    on_secondary_container: p::SECONDARY_90,
    tertiary: p::TERTIARY_80,
    on_tertiary: p::TERTIARY_20,
    tertiary_container: p::TERTIARY_30,
    on_tertiary_container: p::TERTIARY_90,
    error: p::ERROR_80,
    on_error: p::ERROR_20,
    error_container: p::ERROR_30,
    on_error_container: p::ERROR_90,
    primary_fixed: p::PRIMARY_90,
    primary_fixed_dim: p::PRIMARY_80,
    on_primary_fixed: p::PRIMARY_10,
    on_primary_fixed_variant: p::PRIMARY_30,
    secondary_fixed: p::SECONDARY_90,
    secondary_fixed_dim: p::SECONDARY_80,
    on_secondary_fixed: p::SECONDARY_10,
    on_secondary_fixed_variant: p::SECONDARY_30,
    tertiary_fixed: p::TERTIARY_90,
    tertiary_fixed_dim: p::TERTIARY_80,
    on_tertiary_fixed: p::TERTIARY_10,
    on_tertiary_fixed_variant: p::TERTIARY_30,
    background: p::NEUTRAL_6,
    on_background: p::NEUTRAL_90,
    surface: p::NEUTRAL_6,
    surface_dim: p::NEUTRAL_6,
    surface_bright: p::NEUTRAL_24,
    surface_container_lowest: p::NEUTRAL_4,
    surface_container_low: p::NEUTRAL_10,
    surface_container: p::NEUTRAL_12,
    surface_container_high: p::NEUTRAL_17,
    surface_container_highest: p::NEUTRAL_22,
    on_surface: p::NEUTRAL_90,
    surface_variant: p::NEUTRAL_VARIANT_30,
    on_surface_variant: p::NEUTRAL_VARIANT_80,
    inverse_surface: p::NEUTRAL_90,
    inverse_on_surface: p::NEUTRAL_20,
    inverse_primary: p::PRIMARY_40,
    outline: p::NEUTRAL_VARIANT_60,
    outline_variant: p::NEUTRAL_VARIANT_30,
    surface_tint: p::PRIMARY_80,
    shadow: p::NEUTRAL_0,
    scrim: p::NEUTRAL_0,
    transparent: p::NEUTRAL_0.with_alpha(0.0),
};

/// Returns the M3 color scheme for a user-selected theme.
#[must_use]
pub fn scheme(color_theme: ColorTheme) -> &'static ColorScheme {
    match color_theme {
        ColorTheme::Dark => &DARK,
        ColorTheme::Light => &LIGHT,
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::float_cmp,
        reason = "Color tests compare exact 8-bit channel conversions."
    )]
    #![expect(
        clippy::suboptimal_flops,
        reason = "Contrast helpers mirror the canonical WCAG formulas for auditability."
    )]

    use super::*;

    fn relative_luminance(token: RgbaToken) -> f32 {
        fn linear(channel: f32) -> f32 {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * linear(token.red) + 0.7152 * linear(token.green) + 0.0722 * linear(token.blue)
    }

    fn contrast_ratio(foreground: RgbaToken, background: RgbaToken) -> f32 {
        let foreground = relative_luminance(foreground);
        let background = relative_luminance(background);
        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
    }

    #[test]
    fn schemes_report_their_theme() {
        for color_theme in ColorTheme::ALL {
            assert_eq!(scheme(color_theme).color_theme(), color_theme);
        }
        assert!(DARK.is_dark());
        assert!(!LIGHT.is_dark());
    }

    #[test]
    fn baseline_key_roles_match_the_design_system() {
        assert_eq!(LIGHT.primary, RgbaToken::from_rgb(0x67, 0x50, 0xA4));
        assert_eq!(LIGHT.surface, RgbaToken::from_rgb(0xFE, 0xF7, 0xFF));
        assert_eq!(DARK.primary, RgbaToken::from_rgb(0xD0, 0xBC, 0xFF));
        assert_eq!(DARK.surface, RgbaToken::from_rgb(0x14, 0x12, 0x18));
        assert_eq!(
            DARK.surface_container_highest,
            RgbaToken::from_rgb(0x36, 0x34, 0x3B)
        );
    }

    #[test]
    fn content_roles_meet_wcag_aa_on_their_paired_containers() {
        for color_theme in ColorTheme::ALL {
            let s = scheme(color_theme);
            let pairs = [
                ("on_primary", s.on_primary, s.primary),
                (
                    "on_primary_container",
                    s.on_primary_container,
                    s.primary_container,
                ),
                ("on_secondary", s.on_secondary, s.secondary),
                (
                    "on_secondary_container",
                    s.on_secondary_container,
                    s.secondary_container,
                ),
                ("on_tertiary", s.on_tertiary, s.tertiary),
                (
                    "on_tertiary_container",
                    s.on_tertiary_container,
                    s.tertiary_container,
                ),
                ("on_error", s.on_error, s.error),
                (
                    "on_error_container",
                    s.on_error_container,
                    s.error_container,
                ),
                ("on_surface", s.on_surface, s.surface),
                (
                    "on_surface_variant",
                    s.on_surface_variant,
                    s.surface_container_highest,
                ),
                (
                    "inverse_on_surface",
                    s.inverse_on_surface,
                    s.inverse_surface,
                ),
            ];
            for (name, content, container) in pairs {
                let ratio = contrast_ratio(content, container);
                assert!(
                    ratio >= 4.5,
                    "{color_theme:?} {name} contrast was {ratio:.2}:1"
                );
            }
        }
    }

    #[test]
    fn surface_containers_step_away_from_the_surface_in_order() {
        for color_theme in ColorTheme::ALL {
            let s = scheme(color_theme);
            let steps = [
                s.surface_container_lowest,
                s.surface_container_low,
                s.surface_container,
                s.surface_container_high,
                s.surface_container_highest,
            ]
            .map(relative_luminance);
            let ordered = steps.windows(2).all(|pair| {
                if s.is_dark() {
                    pair[0] < pair[1]
                } else {
                    pair[0] > pair[1]
                }
            });
            assert!(
                ordered,
                "{color_theme:?} surface containers are out of order"
            );
        }
    }

    #[test]
    fn transparent_role_has_no_alpha() {
        for color_theme in ColorTheme::ALL {
            assert_eq!(scheme(color_theme).transparent.alpha, 0.0);
        }
    }
}
