<div align="center">
  <img src="./icon.png" width="256" height="256" alt="Scissors Icon" />
  <h1>Scissors</h1>
</div>

<div align="center">
  <img src="https://img.shields.io/badge/GPUI--CE-native-2d3748?style=flat-square" alt="GPUI-CE" />
  <img src="https://img.shields.io/badge/Rust-Edition_2024-black?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/FFmpeg-runtime-007808?style=flat-square&logo=ffmpeg" alt="FFmpeg" />
  <img src="https://img.shields.io/badge/license-GPL--3.0-green?style=flat-square" alt="License" />
  <a href="https://github.com/sponsors/66HEX">
    <img src="https://img.shields.io/badge/Sponsor-GitHub-pink?style=flat-square&logo=githubsponsors" alt="GitHub Sponsors" />
  </a>
</div>

**Scissors** is a native media conversion utility built in Rust. It provides a
desktop interface for FFmpeg operations, with granular control over video,
audio, image, and metadata settings. The application uses a GPUI-CE
front end and a reusable Rust conversion core for FFmpeg argument generation,
source probing, compatibility validation, task control, and progress parsing.

<br />
<div align="center">
  <img src="./preview.png" alt="Scissors Application Preview" width="800" />
</div>
<br />

## Features

### Media Conversion Core

- **Media Types:** Video, audio, and image sources.
- **Supported Source Files:**
  - **Video:** `mp4`, `mov`, `mkv`, `avi`, `webm`, `mts`, `m2t`, `m2ts`, `gif`
  - **Audio:** `mp3`, `m4a`, `wav`, `flac`
  - **Image:** `png`, `jpg`, `jpeg`, `webp`, `bmp`, `tif`, `tiff`, `avif`,
    `heic`, `heif`
- **Supported Output Formats:**
  - **Video:** `mp4`, `mkv`, `webm`, `mov`, `m2t` (188-byte MPEG-TS), `mts`
    and `m2ts` (192-byte M2TS), `gif`
  - **Audio:** `mp3`, `m4a`, `wav`, `flac`
  - **Image:** `png`, `jpg`, `webp`, `bmp`, `tiff`
- **Video Encoders:**
  - `libx264` (H.264 / AVC)
  - `libx265` (H.265 / HEVC)
  - `vp9` (Google VP9)
  - `prores` (Apple ProRes)
  - `libsvtav1` (SVT-AV1)
  - `mpeg2video` (MPEG-2 Video for transport streams)
  - `gif` palette output
  - **Hardware Acceleration:** macOS FFmpeg downloads include VideoToolbox encoders
    (`h264_videotoolbox`, `hevc_videotoolbox`); Windows FFmpeg downloads include NVIDIA
    NVENC encoders (`h264_nvenc`, `hevc_nvenc`, `av1_nvenc`). Linux FFmpeg downloads use
    software encoders.
- **Image Encoders:** `png`, `mjpeg` (JPEG), `libwebp` (WebP), `bmp`, `tiff`.
- **Audio Encoders:** `aac`, `ac3`, `libopus`, `mp3`, `mp2`, `alac`, `flac`,
  `pcm_s16le`, Blu-ray `pcm_bluray`, plus optional `libfdk_aac` when available.
- **Bitrate Control:** CRF, target bitrate, audio VBR where supported, and
  codec-specific presets.
- **Video Processing:** Resolution presets, custom dimensions, FPS conversion,
  pixel format selection, scaling, crop, rotate, flip, and image overlay.
- **Video Filters:** Brightness, contrast, saturation, gamma, hue, color
  temperature, sharpen, Gaussian blur, denoise, deband, vignette, grayscale,
  and deinterlace controls.
- **GIF Controls:** Frame rate, color count, dithering, and loop count.
- **Audio Controls:** Codec, bitrate, VBR quality, channel layout, volume, and
  explicit per-track selection; only checked source tracks are exported.
- **Audio Filters:** Normalize, limiter, compressor, bass, treble, high-pass,
  low-pass, noise reduction, de-esser, and stereo width controls.
- **Metadata:** Preserve, clean, or replace generic metadata fields, plus
  MPEG-TS/M2TS program service name and provider metadata read through
  `ffprobe -show_programs`.
- **Metadata Probing:** Automated source inspection through `ffprobe`.

### Architecture & Workflow

- **Native UI:** GPUI-CE application shell, custom titlebar, workspace view,
  export dialog with live FFmpeg output, settings panels, and shared UI
  primitives.
- **Shared Conversion Core:** `scissors-core` owns FFmpeg arguments, media rules,
  probing types, filter construction, and validation logic.
- **Single-project workflow:** Edit one timeline with multiple media clips, save or
  open a project, and export it with progress and cancellation controls.
- **Real-time Telemetry:** FFmpeg progress and log events are parsed and shown
  in the app while conversions run.
- **Runtime Binaries:** Local development uses platform-specific FFmpeg and
  FFprobe binaries under `scissors-app/resources/binaries/`. The app detects encoder
  capabilities from the selected FFmpeg at startup.

## Technical Stack

### Native Application

- **Language:** Rust Edition 2024.
- **UI:** GPUI-CE.
- **Native Dialogs:** `rfd`, with extension filtering for supported media files.
- **Assets:** Embedded SVG icons, bundled Overused Grotesk font, and native app
  icon resources for native builds.

### Conversion Core

- **Runtime Tools:** FFmpeg and FFprobe.
- **Serialization:** `serde`, `serde_json`.
- **Error Handling:** `thiserror`.
- **Media Rules:** Shared JSON compatibility matrix consumed by Rust code.

## Local setup

This checkout is built and run locally. Install Rust and the native development
libraries listed in [CONTRIBUTING.md](CONTRIBUTING.md), then run:

```bash
cargo xtask setup-ffmpeg
cargo xtask run
```

Build an optimized local executable with:

```bash
cargo xtask build --release
```

The executable is `target/release/scissors`, or `target/release/scissors.exe` on Windows.
FFmpeg and FFprobe remain under `scissors-app/resources/binaries/`. To use your own
installations, set `SCISSORS_FFMPEG_PATH` and `SCISSORS_FFPROBE_PATH`, or set
`SCISSORS_USE_SYSTEM_MEDIA_TOOLS=1` to use the tools on `PATH`.

Scissors saves projects with the `.scissors` extension and can open existing
`.frame` projects. It reads settings from the former Frame config directory
until settings are saved under Scissors. Environment variables now use the
`SCISSORS_` prefix.

## Development Checks

Run the main checks before submitting changes:

```bash
cargo xtask ci
```

## Star History

<a href="https://www.star-history.com/?repos=66HEX%2Fframe&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=66HEX/frame&type=date&theme=dark&legend=top-left&sealed_token=qQ6hXCFpFbLB0xQ3VJWBpqgd-1jWExYh5y8PjqpU_0neCRaXvspFp3hJVBWGu2bE8OFZNI3XkEOY_jXujwlg62x69RUZXxVTdCFMG-dc9-vEkFW5PmLD6yZ26l8w3zNRAyTRisu5PC1Bk5UY_k7P-YUkwvSwXbTQg8MXFhjeTGJS9FPo8OsSIlmS_6en" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=66HEX/frame&type=date&legend=top-left&sealed_token=qQ6hXCFpFbLB0xQ3VJWBpqgd-1jWExYh5y8PjqpU_0neCRaXvspFp3hJVBWGu2bE8OFZNI3XkEOY_jXujwlg62x69RUZXxVTdCFMG-dc9-vEkFW5PmLD6yZ26l8w3zNRAyTRisu5PC1Bk5UY_k7P-YUkwvSwXbTQg8MXFhjeTGJS9FPo8OsSIlmS_6en" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=66HEX/frame&type=date&legend=top-left&sealed_token=qQ6hXCFpFbLB0xQ3VJWBpqgd-1jWExYh5y8PjqpU_0neCRaXvspFp3hJVBWGu2bE8OFZNI3XkEOY_jXujwlg62x69RUZXxVTdCFMG-dc9-vEkFW5PmLD6yZ26l8w3zNRAyTRisu5PC1Bk5UY_k7P-YUkwvSwXbTQg8MXFhjeTGJS9FPo8OsSIlmS_6en" />
 </picture>
</a>

## Acknowledgments & Third-Party Code

- **FFmpeg**: Licensed under [GPLv3](https://www.ffmpeg.org/legal.html).
- **GPUI-CE**: Native Rust UI framework used by Scissors.

## License

GPLv3 License. See [LICENSE](LICENSE) for details.
