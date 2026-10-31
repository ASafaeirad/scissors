# Contributing to Scissors

Thank you for your interest in contributing to **Scissors**. This document covers
the current project structure, local setup, checks, and pull request standards.

## Technical Stack

- **Application:** Rust native desktop app built with GPUI-CE.
- **Core Engine:** FFmpeg and FFprobe runtime binaries.
- **Shared Logic:** `scissors-core` for conversion arguments, probing data,
  compatibility rules, filters, and validation.
- **Native UI:** `scissors-app` for the application shell, GPUI views, app state,
  dialogs, bundled assets, and runtime integration.
- **Automation:** `cargo xtask` for local builds, running the app, FFmpeg setup,
  and checks.

## Getting Started

### Prerequisites

To build and run Scissors locally, you will need:

1. **Rust:** [Install Rust](https://www.rust-lang.org/tools/install)
2. **Platform toolchain:** install the C/C++ build tools and native desktop
   libraries required by Rust and GPUI-CE for your operating system.

### System Dependencies

Install the development dependencies for your operating system.

#### macOS

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

Scissors's GPUI shader build also requires the Metal compiler from a full Xcode
installation. Ensure Xcode is the active developer directory, then verify the
compiler is available:

```bash
xcrun metal --version
```

If that command reports a missing Metal Toolchain, download the component and
verify it again:

```bash
xcodebuild -downloadComponent MetalToolchain
xcrun metal --version
```

No additional Homebrew packages are required for normal local development.
`cargo xtask setup-ffmpeg` downloads the FFmpeg and FFprobe binaries used by the
app during development.

#### Linux

Linux builds need a C/C++ toolchain plus native desktop, font, X11/XKB, Wayland,
and audio development libraries. The package names differ by distribution.

Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y build-essential clang pkg-config \
  libfontconfig1-dev libfreetype6-dev libx11-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libwayland-dev libasound2-dev libpulse-dev libdrm-dev
```

Fedora:

```bash
sudo dnf install gcc gcc-c++ clang pkg-config \
  fontconfig-devel freetype-devel libX11-devel libxkbcommon-devel \
  libxkbcommon-x11-devel libxcb-devel wayland-devel alsa-lib-devel \
  pulseaudio-libs-devel libdrm-devel
```

Arch Linux:

```bash
sudo pacman -S --needed base-devel clang pkgconf \
  fontconfig freetype2 libx11 libxkbcommon libxkbcommon-x11 libxcb wayland \
  alsa-lib libpulse libdrm
```

#### Windows

Install:

1. Rust with the MSVC toolchain.
2. Visual Studio 2022 Build Tools with the **Desktop development with C++**
   workload, including MSVC and the Windows SDK.

### Local Setup

1. **Set up FFmpeg binaries:**

   The application looks for FFmpeg and FFprobe in `scissors-app/resources/binaries/`
   during local development. Download the platform-specific tools with:

   ```bash
   cargo xtask setup-ffmpeg
   ```

2. **Run in development mode:**

   ```bash
   cargo xtask run
   ```

3. **Build an optimized local binary:**

   ```bash
   cargo xtask build --release
   ```

The executable is `target/release/scissors`, or `target/release/scissors.exe` on Windows.
To use an existing FFmpeg installation, set `SCISSORS_FFMPEG_PATH` and
`SCISSORS_FFPROBE_PATH`, or set `SCISSORS_USE_SYSTEM_MEDIA_TOOLS=1` to resolve both tools
from `PATH`.

## Development Workflow

### Project Structure

- `scissors-app/`: native GPUI-CE application, views, app state, dialogs, runtime
  binary lookup, bundled assets, native app identity, and app icons.
- `scissors-app/src/app/`: `ScissorsRoot`, workspace/logs rendering, settings panels,
  preview shell, import flow, conversion actions, and UI primitives.
- `scissors-app/src/file_queue/`, `scissors-app/src/settings/`, `scissors-app/src/preview/`,
  and `scissors-app/src/conversion_runner/`: tested domain logic kept outside
  rendering code where possible.
- `scissors-core/`: shared conversion/probe logic, FFmpeg argument construction,
  filters, media compatibility rules, and conversion event types.
- `scissors-core/media-rules.json`: source of truth for container, codec, stream,
  and pixel format compatibility.
- `tooling/xtask/`: local run/build commands, FFmpeg setup, and quality checks.
- `CHANGELOG.md`: historical product changes.

### Coding Standards

- **Rust formatting:** run `cargo fmt` for touched crates before submitting.
- **Rust linting:** keep `cargo clippy --all-targets -- -D warnings` clean.
- **Architecture:** use Material 3 tokens in `scissors-app/src/material/` and
  components in `scissors-app/src/app/m3/`.
- **Conversion Logic:** add FFmpeg behavior in `scissors-core` first, then bridge it
  into `scissors-app` through typed settings/configuration code.
- **Media Compatibility:** update `scissors-core/media-rules.json` and focused tests
  when adding formats, codecs, stream-copy rules, or pixel formats.
- **Runtime Binaries:** do not commit downloaded files from
  `scissors-app/resources/binaries/`.
- **Application Icons:** keep native icons under `scissors-app/resources/app-icons/`.
  `scissors-app/build.rs` prepares Linux icons and embeds Windows resources.

### Testing & Quality Control

Before submitting a PR, run the relevant checks:

```bash
cargo xtask ci
```

For UI changes, add or update focused GPUI tests where practical. Visual parity
fixtures live in the GPUI test modules. Runtime visual fixture names and review
scenarios are listed in [Visual Fixtures](CONTRIBUTING_VISUAL_FIXTURES.md).

## Pull Request Process

1. Create a new branch for your feature or bugfix:

   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make focused commits with descriptive messages.
3. Include tests or explain why a change is not practical to test.
4. Push to your fork and submit a pull request.
5. Provide a clear description of the behavior change, screenshots for UI work,
   and any relevant issue numbers.

## Reporting Issues

If you find a bug or have a feature request, please
[open an issue](https://github.com/66HEX/frame/issues). Include your operating
system, source media details when relevant, reproduction steps, and FFmpeg logs
from the Logs view when conversion behavior is involved.

## Financial Support

If you want to support the long-term maintenance and development of Scissors, use
[GitHub Sponsors](https://github.com/sponsors/66HEX).

---

By contributing to this project, you agree that your contributions will be
licensed under the project's [LICENSE](LICENSE).
