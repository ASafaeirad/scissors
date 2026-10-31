//! Material Design 3 tokens, ported from the claude.ai/design "Material 3 Design System" project
//! (baseline scheme, source color `#6750A4`).
//!
//! This module holds data only: color schemes, type scale, shape, elevation, state layers, motion,
//! spacing and the bundled Material Symbols. The GPUI components that consume it live in
//! `app::m3`.
//!
//! Values are logical pixels at 100% UI scale. Convert lengths with [`crate::theme::ui_rem`].

pub mod color;
pub mod elevation;
pub mod icons;
pub mod motion;
pub mod shape;
pub mod spacing;
pub mod state;
pub mod typography;

pub use color::{ColorScheme, accent_preset_schemes, scheme, scheme_from_accent};
pub use elevation::Elevation;
pub use icons::Symbol;
pub use typography::TypeStyle;
