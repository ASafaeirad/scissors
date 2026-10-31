//! Material 3 type scale: 15 roles (display, headline, title, body, label × large, medium, small).
//!
//! Sizes are logical pixels at 100% UI scale. Convert them with [`crate::theme::ui_rem`] so they
//! follow the user's scale preset.

use gpui::FontWeight;

/// Brand and plain typeface family (`--md-ref-typeface-brand` / `--md-ref-typeface-plain`).
pub const TYPEFACE: &str = "Roboto";
/// Monospaced face for console output, such as encoder logs.
pub const MONO_TYPEFACE: &str = "Roboto Mono";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    pub size: f32,
    pub line_height: f32,
    /// Letter spacing in logical pixels. GPUI has no tracking control yet, so components don't
    /// apply this. It is kept so the tokens stay a complete copy of the spec.
    pub tracking: f32,
    pub weight: FontWeight,
}

const fn style(size: f32, line_height: f32, tracking: f32, weight: FontWeight) -> TypeStyle {
    TypeStyle {
        size,
        line_height,
        tracking,
        weight,
    }
}

const REGULAR: FontWeight = FontWeight::NORMAL;
const MEDIUM: FontWeight = FontWeight::MEDIUM;

pub const DISPLAY_LARGE: TypeStyle = style(57.0, 64.0, -0.25, REGULAR);
pub const DISPLAY_MEDIUM: TypeStyle = style(45.0, 52.0, 0.0, REGULAR);
pub const DISPLAY_SMALL: TypeStyle = style(36.0, 44.0, 0.0, REGULAR);
pub const HEADLINE_LARGE: TypeStyle = style(32.0, 40.0, 0.0, REGULAR);
pub const HEADLINE_MEDIUM: TypeStyle = style(28.0, 36.0, 0.0, REGULAR);
pub const HEADLINE_SMALL: TypeStyle = style(24.0, 32.0, 0.0, REGULAR);
pub const TITLE_LARGE: TypeStyle = style(22.0, 28.0, 0.0, REGULAR);
pub const TITLE_MEDIUM: TypeStyle = style(16.0, 24.0, 0.15, MEDIUM);
pub const TITLE_SMALL: TypeStyle = style(14.0, 20.0, 0.1, MEDIUM);
pub const BODY_LARGE: TypeStyle = style(16.0, 24.0, 0.5, REGULAR);
pub const BODY_MEDIUM: TypeStyle = style(14.0, 20.0, 0.25, REGULAR);
pub const BODY_SMALL: TypeStyle = style(12.0, 16.0, 0.4, REGULAR);
pub const LABEL_LARGE: TypeStyle = style(14.0, 20.0, 0.1, MEDIUM);
pub const LABEL_MEDIUM: TypeStyle = style(12.0, 16.0, 0.5, MEDIUM);
pub const LABEL_SMALL: TypeStyle = style(11.0, 16.0, 0.5, MEDIUM);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_small_titles_use_medium_weight() {
        for role in [
            TITLE_MEDIUM,
            TITLE_SMALL,
            LABEL_LARGE,
            LABEL_MEDIUM,
            LABEL_SMALL,
        ] {
            assert_eq!(role.weight, FontWeight::MEDIUM);
        }
    }

    #[test]
    fn every_role_has_a_line_height_taller_than_its_size() {
        for role in [
            DISPLAY_LARGE,
            DISPLAY_MEDIUM,
            DISPLAY_SMALL,
            HEADLINE_LARGE,
            HEADLINE_MEDIUM,
            HEADLINE_SMALL,
            TITLE_LARGE,
            TITLE_MEDIUM,
            TITLE_SMALL,
            BODY_LARGE,
            BODY_MEDIUM,
            BODY_SMALL,
            LABEL_LARGE,
            LABEL_MEDIUM,
            LABEL_SMALL,
        ] {
            assert!(role.line_height > role.size, "{role:?}");
        }
    }
}
