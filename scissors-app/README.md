# Scissors App

Native GPUI-CE application for Scissors.

This crate owns the application shell, GPUI views, GPUI-specific state, and bundled assets used by the native app. Shared conversion/probe logic is consumed through the `scissors-core` crate; GPUI app code and bundled assets stay in this directory.

The app intentionally stays self-contained here: local Scissors UI wrappers are built directly on GPUI-CE primitives, assets live under `scissors-app/assets/`, and no external GPUI component library is used.

## Source Layout

- `src/main.rs` is only the GPUI entrypoint.
- `src/app_info.rs` owns app identity constants used by the runtime.
- `src/app/` owns `ScissorsRoot`, window runtime, shell rendering, import/conversion/metadata workflows, and UI panels.
- `src/app/input/`, `src/app/preview_panel/`, and `src/app/settings_panel/` split the largest GPUI UI surfaces into focused submodules.
- `src/file_queue/`, `src/settings/`, `src/preview/`, and `src/conversion_runner/` contain tested domain logic outside rendering code.
- `src/assets/` embeds only files from `scissors-app/assets/`.
- `resources/app-icons/` contains the native app icons consumed by `build.rs`.
- `resources/binaries/` is an ignored local setup output for FFmpeg and
  FFprobe.

Build output stays under the workspace `target/` directory and is ignored by `.gitignore`. macOS-specific native-window glue is limited to hiding AppKit's standard titlebar buttons so the custom Scissors controls are the only visible traffic lights. Windows icons are embedded by `build.rs`. Run locally with `cargo xtask run`, or build an optimized executable with `cargo xtask build --release`.

Enable **Show FPS meter** under **Settings > Diagnostics** to toggle UI
performance diagnostics. You can also press `F12` or choose **Show/hide FPS meter**
in the app menu. The overlay updates every half second with UI redraw
FPS, average frame time, and the slowest frame in that sample. It requests
continuous redraws while enabled and pauses when the window loses focus. These
numbers measure UI responsiveness, independently of media playback FPS.
