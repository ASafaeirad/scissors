//! Material 3 elevation levels as two-layer shadows (a 30% key light and a 15% ambient light).
//!
//! Prefer tonal `surface_container*` roles for depth. Use shadows sparingly: elevated buttons and
//! cards (level 1), menus (level 2), FABs, snackbars, dialogs and floating toolbars (level 3).

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Elevation {
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
    Level5,
}

/// One shadow layer in logical pixels. `alpha` scales the scheme's opaque `shadow` role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLayer {
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub alpha: f32,
}

const KEY_ALPHA: f32 = 0.30;
const AMBIENT_ALPHA: f32 = 0.15;

const fn pair(key: (f32, f32), ambient: (f32, f32, f32)) -> [ShadowLayer; 2] {
    [
        ShadowLayer {
            offset_y: key.0,
            blur: key.1,
            spread: 0.0,
            alpha: KEY_ALPHA,
        },
        ShadowLayer {
            offset_y: ambient.0,
            blur: ambient.1,
            spread: ambient.2,
            alpha: AMBIENT_ALPHA,
        },
    ]
}

impl Elevation {
    /// Shadow layers for this level, matching `--md-sys-elevation-level*`. Level 0 is flat.
    #[must_use]
    pub const fn layers(self) -> &'static [ShadowLayer] {
        const LEVEL1: [ShadowLayer; 2] = pair((1.0, 2.0), (1.0, 3.0, 1.0));
        const LEVEL2: [ShadowLayer; 2] = pair((1.0, 2.0), (2.0, 6.0, 2.0));
        const LEVEL3: [ShadowLayer; 2] = pair((1.0, 3.0), (4.0, 8.0, 3.0));
        const LEVEL4: [ShadowLayer; 2] = pair((2.0, 3.0), (6.0, 10.0, 4.0));
        const LEVEL5: [ShadowLayer; 2] = pair((4.0, 4.0), (8.0, 12.0, 6.0));
        match self {
            Self::Level0 => &[],
            Self::Level1 => &LEVEL1,
            Self::Level2 => &LEVEL2,
            Self::Level3 => &LEVEL3,
            Self::Level4 => &LEVEL4,
            Self::Level5 => &LEVEL5,
        }
    }

    /// The next level up, used for hover lift. Level 5 stays at level 5.
    #[must_use]
    pub const fn raised(self) -> Self {
        match self {
            Self::Level0 => Self::Level1,
            Self::Level1 => Self::Level2,
            Self::Level2 => Self::Level3,
            Self::Level3 => Self::Level4,
            Self::Level4 | Self::Level5 => Self::Level5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_zero_is_flat_and_other_levels_have_key_and_ambient_layers() {
        assert!(Elevation::Level0.layers().is_empty());
        for level in [
            Elevation::Level1,
            Elevation::Level2,
            Elevation::Level3,
            Elevation::Level4,
            Elevation::Level5,
        ] {
            let layers = level.layers();
            assert_eq!(layers.len(), 2, "{level:?}");
            assert!((layers[0].alpha - KEY_ALPHA).abs() < f32::EPSILON);
            assert!((layers[1].alpha - AMBIENT_ALPHA).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn ambient_blur_grows_with_level() {
        let blurs = [
            Elevation::Level1,
            Elevation::Level2,
            Elevation::Level3,
            Elevation::Level4,
            Elevation::Level5,
        ]
        .map(|level| level.layers()[1].blur);
        assert!(blurs.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn raised_caps_at_level_five() {
        assert_eq!(Elevation::Level0.raised(), Elevation::Level1);
        assert_eq!(Elevation::Level5.raised(), Elevation::Level5);
    }
}
