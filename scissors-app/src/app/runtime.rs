use super::*;

pub fn init_app(cx: &mut App, name: impl Into<SharedString>) {
    cx.activate(true);
    cx.set_text_rendering_mode(TextRenderingMode::PlatformDefault);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new(
            "backspace",
            TextInputBackspace,
            Some(SCISSORS_TEXT_INPUT_CONTEXT),
        ),
        KeyBinding::new("delete", TextInputDelete, Some(SCISSORS_TEXT_INPUT_CONTEXT)),
        KeyBinding::new("left", TextInputLeft, Some(SCISSORS_TEXT_INPUT_CONTEXT)),
        KeyBinding::new("right", TextInputRight, Some(SCISSORS_TEXT_INPUT_CONTEXT)),
        KeyBinding::new(
            "shift-left",
            TextInputSelectLeft,
            Some(SCISSORS_TEXT_INPUT_CONTEXT),
        ),
        KeyBinding::new(
            "shift-right",
            TextInputSelectRight,
            Some(SCISSORS_TEXT_INPUT_CONTEXT),
        ),
        KeyBinding::new("home", TextInputHome, Some(SCISSORS_TEXT_INPUT_CONTEXT)),
        KeyBinding::new("end", TextInputEnd, Some(SCISSORS_TEXT_INPUT_CONTEXT)),
        KeyBinding::new("cmd-left", TextInputHome, Some(SCISSORS_TEXT_INPUT_CONTEXT)),
        KeyBinding::new("cmd-right", TextInputEnd, Some(SCISSORS_TEXT_INPUT_CONTEXT)),
        KeyBinding::new(
            "secondary-a",
            TextInputSelectAll,
            Some(SCISSORS_TEXT_INPUT_CONTEXT),
        ),
        KeyBinding::new(
            "secondary-c",
            TextInputCopy,
            Some(SCISSORS_TEXT_INPUT_CONTEXT),
        ),
        KeyBinding::new(
            "secondary-x",
            TextInputCut,
            Some(SCISSORS_TEXT_INPUT_CONTEXT),
        ),
        KeyBinding::new(
            "secondary-v",
            TextInputPaste,
            Some(SCISSORS_TEXT_INPUT_CONTEXT),
        ),
        KeyBinding::new(
            "enter",
            TextInputCommit,
            Some(SCISSORS_TIMECODE_INPUT_CONTEXT),
        ),
        KeyBinding::new(
            "escape",
            TextInputCancel,
            Some(SCISSORS_TIMECODE_INPUT_CONTEXT),
        ),
        KeyBinding::new("secondary-=", IncreaseUiScale, None),
        KeyBinding::new("secondary-+", IncreaseUiScale, None),
        KeyBinding::new("secondary-shift-=", IncreaseUiScale, None),
        KeyBinding::new("secondary--", DecreaseUiScale, None),
        KeyBinding::new("secondary-0", ResetUiScale, None),
        KeyBinding::new("f12", ToggleFpsMeter, None),
        KeyBinding::new("secondary-s", SaveProject, None),
        KeyBinding::new("secondary-shift-s", SaveProjectAs, None),
        KeyBinding::new("secondary-o", OpenProject, None),
        KeyBinding::new("ctrl-,", OpenSettings, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-,", OpenSettings, None),
    ]);
    cx.set_menus(vec![Menu {
        name: name.into(),
        items: vec![
            MenuItem::action("Open project…", OpenProject),
            MenuItem::action("Save project", SaveProject),
            MenuItem::action("Save project as…", SaveProjectAs),
            MenuItem::separator(),
            MenuItem::action("Show/hide FPS meter", ToggleFpsMeter),
            MenuItem::action("Quit", Quit),
        ],
        disabled: false,
    }]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

/// Opens Scissors' main application window.
///
/// # Panics
///
/// Panics when GPUI cannot create the main window.
pub fn open_scissors_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(WINDOW_MIN_WIDTH), px(WINDOW_MIN_HEIGHT)), cx);
    cx.open_window(scissors_window_options(bounds), |_, cx| {
        cx.new(|cx| {
            let mut root = ScissorsRoot::new_with_platform_persistence();
            root.load_runtime_capabilities(cx);
            root
        })
    })
    .expect("failed to open Scissors GPUI window");
}

#[must_use]
pub fn scissors_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Scissors".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(
                px(crate::TITLEBAR_MACOS_NATIVE_TRAFFIC_LIGHT_X),
                px(crate::TITLEBAR_MACOS_NATIVE_TRAFFIC_LIGHT_Y),
            )),
        }),
        window_min_size: Some(size(px(WINDOW_MIN_WIDTH), px(WINDOW_MIN_HEIGHT))),
        window_background: WindowBackgroundAppearance::Opaque,
        window_decorations: Some(WindowDecorations::Client),
        #[cfg(target_os = "linux")]
        client_side_frame: Some(gpui::ClientSideFrameOptions {
            corner_radius: px(theme::RADIUS_LG + LINUX_WINDOW_FRAME_INSET),
        }),
        #[cfg(not(target_os = "linux"))]
        client_side_frame: None,
        app_id: Some(SCISSORS_APP_ID.to_owned()),
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        icon: scissors_window_icon(),
        ..Default::default()
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn scissors_window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    use std::{io::Cursor, sync::LazyLock};

    static APP_ICON: LazyLock<Option<std::sync::Arc<image::RgbaImage>>> = LazyLock::new(|| {
        const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app_icon.png"));
        image::ImageReader::new(Cursor::new(BYTES))
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()
            .map(image::DynamicImage::into_rgba8)
            .map(std::sync::Arc::new)
    });

    APP_ICON.as_ref().cloned()
}

#[cfg(test)]
mod ui_scale_keybinding_tests {
    use gpui::Keystroke;

    #[test]
    fn primary_modifier_ui_scale_bindings_use_valid_gpui_syntax() {
        for binding in [
            "secondary-=",
            "secondary-+",
            "secondary-shift-=",
            "secondary--",
            "secondary-0",
        ] {
            assert!(
                Keystroke::parse(binding).is_ok(),
                "{binding} should be a valid GPUI keystroke"
            );
        }
    }

    #[test]
    fn secondary_uses_the_native_primary_modifier() {
        let keystroke = Keystroke::parse("secondary-0").expect("binding should parse");

        #[cfg(target_os = "macos")]
        assert!(keystroke.modifiers.platform && !keystroke.modifiers.control);

        #[cfg(not(target_os = "macos"))]
        assert!(keystroke.modifiers.control && !keystroke.modifiers.platform);
    }
}
