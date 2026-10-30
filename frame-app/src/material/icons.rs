//! Material Symbols (Outlined, weight 400, 24px optical size) bundled as SVG assets.
//!
//! Each symbol ships outlined and filled. Filled means selected or active: the current navigation
//! destination, a toggled icon button. Use outlined otherwise.
//!
//! To add a symbol, download both styles into `assets/icons/material/` and add a line below:
//!
//! ```sh
//! base=https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsoutlined
//! curl -o assets/icons/material/NAME.svg      $base/NAME/default/24px.svg
//! curl -o assets/icons/material/NAME-fill.svg $base/NAME/fill1/24px.svg
//! ```

macro_rules! symbols {
    ($($variant:ident => $name:literal,)+) => {
        /// A bundled Material Symbol.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Symbol {
            $($variant,)+
        }

        impl Symbol {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// The symbol's ligature name, as used on fonts.google.com/icons.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            /// Asset path of the outlined style.
            #[must_use]
            pub const fn path(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!("icons/material/", $name, ".svg"),)+
                }
            }

            /// Asset path of the filled style.
            #[must_use]
            pub const fn filled_path(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!("icons/material/", $name, "-fill.svg"),)+
                }
            }

            /// Asset path of the requested style.
            #[must_use]
            pub const fn path_for(self, filled: bool) -> &'static str {
                if filled { self.filled_path() } else { self.path() }
            }
        }

        /// Returns the SVG bytes for a Material Symbol asset path.
        #[must_use]
        pub fn load(path: &str) -> Option<&'static [u8]> {
            match path {
                $(
                    concat!("icons/material/", $name, ".svg") => Some(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"), "/assets/icons/material/", $name, ".svg"
                    ))),
                    concat!("icons/material/", $name, "-fill.svg") => Some(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"), "/assets/icons/material/", $name, "-fill.svg"
                    ))),
                )+
                _ => None,
            }
        }
    };
}

symbols! {
    Add => "add",
    AddPhotoAlternate => "add_photo_alternate",
    ArrowLeft => "arrow_left",
    ArrowRight => "arrow_right",
    Check => "check",
    CheckCircle => "check_circle",
    Close => "close",
    ClosedCaption => "closed_caption",
    ContentCut => "content_cut",
    Crop => "crop",
    Delete => "delete",
    Error => "error",
    ExpandLess => "expand_less",
    ExpandMore => "expand_more",
    FileExport => "file_export",
    FitScreen => "fit_screen",
    Flip => "flip",
    FolderOpen => "folder_open",
    Forward5 => "forward_5",
    GifBox => "gif_box",
    Image => "image",
    Info => "info",
    Layers => "layers",
    Link => "link",
    LinkOff => "link_off",
    Lock => "lock",
    LockOpen => "lock_open",
    Menu => "menu",
    MoreVert => "more_vert",
    Movie => "movie",
    MusicNote => "music_note",
    Pause => "pause",
    PlayArrow => "play_arrow",
    Public => "public",
    Redo => "redo",
    Remove => "remove",
    Replay5 => "replay_5",
    RestartAlt => "restart_alt",
    Rotate90DegreesCcw => "rotate_90_degrees_ccw",
    Rotate90DegreesCw => "rotate_90_degrees_cw",
    Save => "save",
    Search => "search",
    Settings => "settings",
    Stop => "stop",
    Tune => "tune",
    Undo => "undo",
    Upload => "upload",
    UploadFile => "upload_file",
    VideoFile => "video_file",
    VideoLibrary => "video_library",
    Visibility => "visibility",
    VisibilityOff => "visibility_off",
    VolumeOff => "volume_off",
    VolumeUp => "volume_up",
    Warning => "warning",
    ZoomIn => "zoom_in",
    ZoomOut => "zoom_out",
}

/// Lists the file names under `icons/material`, both styles of every symbol.
#[must_use]
pub fn list() -> Vec<String> {
    Symbol::ALL
        .iter()
        .flat_map(|symbol| {
            [
                format!("{}.svg", symbol.name()),
                format!("{}-fill.svg", symbol.name()),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_symbol_loads_both_styles_as_svg() {
        for symbol in Symbol::ALL {
            for path in [symbol.path(), symbol.filled_path()] {
                let bytes = load(path).unwrap_or_else(|| panic!("{path} should load"));
                assert!(bytes.starts_with(b"<svg"), "{path} should be an svg");
            }
        }
    }

    #[test]
    fn outlined_and_filled_styles_differ() {
        // Line-only symbols (`add`, `crop`, `menu`) have no filled form and ship identical files,
        // so only check ones whose filled style is meaningfully different.
        for symbol in [
            Symbol::VideoLibrary,
            Symbol::ClosedCaption,
            Symbol::VolumeUp,
            Symbol::FileExport,
        ] {
            assert_ne!(
                load(symbol.path()),
                load(symbol.filled_path()),
                "{symbol:?}"
            );
        }
    }

    #[test]
    fn path_for_selects_style() {
        assert_eq!(Symbol::Menu.path_for(false), "icons/material/menu.svg");
        assert_eq!(Symbol::Menu.path_for(true), "icons/material/menu-fill.svg");
    }

    #[test]
    fn unknown_paths_do_not_load() {
        assert!(load("icons/material/missing.svg").is_none());
        assert!(load("icons/play.svg").is_none());
    }

    #[test]
    fn every_symbol_renders() {
        let renderer = gpui::SvgRenderer::new(std::sync::Arc::new(crate::assets::FrameAssets));
        for symbol in Symbol::ALL {
            for path in [symbol.path(), symbol.filled_path()] {
                renderer
                    .render_single_frame(load(path).expect("symbol should load"), 1.0)
                    .unwrap_or_else(|error| panic!("{path} should render: {error}"));
            }
        }
    }
}
