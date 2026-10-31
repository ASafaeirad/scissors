//! Cached audio envelopes, decoded away from the UI thread.
use super::decode_cache::DecodeKey;
use super::*;
use std::{
    io::{BufReader, Read},
    process::{Command, Stdio},
};

const SAMPLE_RATE: u32 = 8_000;
const MAX_PEAKS: u32 = 32_768;

#[derive(Debug, PartialEq)]
pub(super) struct Waveform {
    peaks: Vec<f32>,
    seconds_per_peak: f64,
}

/// `None` stream selects the first audio stream; `Some` is an absolute stream index.
pub(super) fn request(
    path: &str,
    stream: Option<u32>,
    duration: f64,
    cx: &mut Context<ScissorsRoot>,
) -> Option<Arc<Waveform>> {
    super::decode_cache::request(path, stream, duration, 64, decode, cx)
}

fn decode(key: &DecodeKey) -> Option<Waveform> {
    let duration = key.duration();
    let samples_per_peak = crate::numeric::rounded_f64_to_u64(
        (duration * f64::from(SAMPLE_RATE) / f64::from(MAX_PEAKS))
            .ceil()
            .max(1.0),
    );
    let mut child = Command::new(crate::runtime_binaries::ffmpeg_executable())
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(&key.path)
        .args([
            "-map",
            &key.stream
                .map_or_else(|| "0:a:0".into(), |index| format!("0:{index}")),
            "-vn",
            "-t",
            &format!("{duration:.6}"),
            "-ac",
            "1",
            "-ar",
            &SAMPLE_RATE.to_string(),
            "-f",
            "f32le",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let result = envelope(BufReader::new(child.stdout.take()?), samples_per_peak);
    if result.is_err() {
        let _ = child.kill();
    }
    let success = child.wait().ok()?.success();
    let peaks = result.ok()?;
    (success && !peaks.is_empty()).then_some(Waveform {
        peaks,
        seconds_per_peak: crate::numeric::u64_to_f64(samples_per_peak) / f64::from(SAMPLE_RATE),
    })
}

fn envelope(mut input: impl Read, samples_per_peak: u64) -> std::io::Result<Vec<f32>> {
    let mut peaks = Vec::new();
    let mut count = 0;
    let mut peak = 0.0_f32;
    let mut sample = [0; 4];
    loop {
        match input.read_exact(&mut sample) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        }
        let value = f32::from_le_bytes(sample);
        if value.is_finite() {
            peak = peak.max(value.abs().min(1.0));
        }
        count += 1;
        if count == samples_per_peak {
            peaks.push(peak);
            count = 0;
            peak = 0.0;
        }
    }
    if count > 0 {
        peaks.push(peak);
    }
    Ok(peaks)
}

impl Waveform {
    fn peak(&self, start: f64, end: f64) -> f32 {
        let first = usize::try_from(crate::numeric::f64_to_u64(
            (start / self.seconds_per_peak).floor(),
        ))
        .unwrap_or(usize::MAX)
        .min(self.peaks.len());
        let last = usize::try_from(crate::numeric::f64_to_u64(
            (end / self.seconds_per_peak).ceil(),
        ))
        .unwrap_or(usize::MAX)
        .min(self.peaks.len());
        self.peaks
            .get(first..last)
            .unwrap_or_default()
            .iter()
            .copied()
            .fold(0.0, f32::max)
    }
}

/// Source ranges concatenate after cuts. Their lengths also account for playback speed.
/// The canvas is absolutely positioned across its parent; callers set its top and bottom.
pub(super) fn element(
    waveform: Arc<Waveform>,
    ranges: Vec<(f64, f64)>,
    tint: Rgba,
) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let total: f64 = ranges.iter().map(|(start, end)| end - start).sum();
            let width = bounds.size.width.as_f32();
            if total <= 0.0 || width <= 0.0 {
                return;
            }
            let scale = window.rem_size().as_f32() / crate::appearance::BASE_REM_PX;
            let step = 3.0 * scale;
            let visible = bounds.intersect(&window.content_mask().bounds);
            let first_bar = crate::numeric::f64_to_u32(f64::from(
                ((visible.left() - bounds.left()).as_f32() / step)
                    .floor()
                    .max(0.0),
            ));
            let last_bar = crate::numeric::f64_to_u32(f64::from(
                (width.min((visible.right() - bounds.left()).as_f32()) / step).ceil(),
            ));
            let height = bounds.size.height.as_f32();
            for bar in first_bar..last_bar {
                let x = crate::numeric::u32_to_f32(bar) * step;
                let mut offset = f64::from(x / width) * total;
                let interval = f64::from(step / width) * total;
                let mut amplitude = 0.0_f32;
                let mut remaining = interval;
                for &(start, end) in &ranges {
                    let length = end - start;
                    if offset >= length {
                        offset -= length;
                        continue;
                    }
                    let amount = remaining.min(length - offset);
                    amplitude =
                        amplitude.max(waveform.peak(start + offset, start + offset + amount));
                    remaining -= amount;
                    if remaining <= 0.0 {
                        break;
                    }
                    offset = 0.0;
                }
                let bar_height = (amplitude * height).max(scale).min(height);
                window.paint_quad(gpui::fill(
                    Bounds::new(
                        point(
                            bounds.left() + px(x),
                            bounds.top() + px((height - bar_height) / 2.0),
                        ),
                        size(px(scale), px(bar_height)),
                    ),
                    tint,
                ));
            }
        },
    )
    .absolute()
    .left_0()
    .right_0()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires FFmpeg; run with --ignored"]
    fn decodes_audio_stream_and_preserves_silent_intervals() {
        let path =
            std::env::temp_dir().join(format!("scissors-waveform-{}.wav", std::process::id()));
        let status = Command::new(crate::runtime_binaries::ffmpeg_executable())
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "aevalsrc=if(lt(t\\,0.5)\\,0\\,0.75*sin(2*PI*440*t)):s=8000:d=1",
            ])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let mut key = DecodeKey {
            path: path.to_string_lossy().into_owned(),
            stream: None,
            duration_millis: 1000,
        };
        let waveform = decode(&key).expect("decode the first audio stream");
        assert!(waveform.peak(0.0, 0.4) < 0.001);
        assert!(waveform.peak(0.6, 0.9) > 0.7);
        key.stream = Some(0);
        assert!(decode(&key).is_some());
        key.stream = Some(99);
        assert!(decode(&key).is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn envelope_preserves_transients_silence_and_partial_buckets() {
        let samples = [0.0_f32, -0.8, 0.2, 0.0, 0.0, 0.0, 0.4];
        let bytes: Vec<_> = samples.into_iter().flat_map(f32::to_le_bytes).collect();
        let peaks = envelope(bytes.as_slice(), 3).unwrap();
        assert_eq!(peaks, [0.8, 0.0, 0.4]);
        let waveform = Waveform {
            peaks,
            seconds_per_peak: 1.0,
        };
        assert!(waveform.peak(1.0, 2.0).abs() < f32::EPSILON);
        assert!((waveform.peak(1.0, 3.0) - 0.4).abs() < f32::EPSILON);
        assert!(waveform.peak(10.0, 12.0).abs() < f32::EPSILON);
    }
}
