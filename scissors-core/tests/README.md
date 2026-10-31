# Scissors media integration tests

This directory contains integration tests that run real FFmpeg/FFprobe commands
against tiny synthetic media files. They are intentionally separate from the
unit tests that only validate argument construction.

Run the fast scissors-core suite:

```sh
cargo test --locked -p scissors-core
```

Run the real media suite:

```sh
cargo test --locked -p scissors-core --test media_integration -- --ignored --test-threads=1
```

Run the application runner smoke test:

```sh
cargo test --locked -p scissors-app \
  conversion_runner::tests::run_conversion_task_should_emit_completed_for_real_ffmpeg_job \
  -- --ignored --test-threads=1
```

The media suite discovers tools in this order:

1. `SCISSORS_TEST_FFMPEG` and `SCISSORS_TEST_FFPROBE`
2. bundled binaries under `scissors-app/resources/binaries`
3. `ffmpeg` and `ffprobe` on `PATH`

Set `SCISSORS_KEEP_MEDIA_TESTS=1` to keep generated files in the system temp
directory after a failure or local investigation.

Coverage currently includes:

- video re-encode outputs: H.264/MP4, H.265/MKV, VP9/WebM, SVT-AV1/MP4, ProRes/MOV
- audio-only outputs: MP3, M4A/AAC, WAV/PCM, FLAC
- still-image outputs: PNG, JPEG, WebP, BMP, TIFF
- GIF palette output
- pixel-format output checks for x264: `yuv420p`, `yuv422p`, `yuv444p`, `yuv420p10le`
- odd source dimensions padded before yuv420p/x264 encoding
- transforms verified by pixels: rotate, horizontal flip, vertical flip, crop, overlay
- custom resolution, trim timing, stream copy, selected audio track, subtitle stream, subtitle burn
- metadata replace and audio normalize/mono conversion
- application runner smoke coverage for `run_conversion_task` events and output creation

Optional encoder tests skip themselves when a local FFmpeg build lacks that
encoder, so a missing ProRes or SVT-AV1 encoder does not fail unrelated
environments.
