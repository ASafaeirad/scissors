//! Black-bar detection with `FFmpeg`'s `cropdetect` filter.
//!
//! Scissors samples a few short stretches spread across the source and keeps the union of what each
//! one detects, so a dark scene in one sample can't crop away picture that another sample shows.

/// Frames decoded per sample.
const SAMPLE_FRAMES: u32 = 8;
/// Points across the source to sample, as fractions of its duration.
const SAMPLE_POSITIONS: [f64; 5] = [0.1, 0.3, 0.5, 0.7, 0.9];

/// A crop rectangle in source pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectedCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl DetectedCrop {
    /// The smallest rectangle containing both crops.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
}

/// Seek positions, in seconds, to sample. Sources without a known duration are sampled from the
/// start.
#[must_use]
pub fn sample_times(duration_seconds: Option<f64>) -> Vec<f64> {
    match duration_seconds {
        Some(duration) if duration.is_finite() && duration > 0.0 => SAMPLE_POSITIONS
            .iter()
            .map(|position| duration * position)
            .collect(),
        _ => vec![0.0],
    }
}

/// `FFmpeg` arguments that run `cropdetect` over a few frames starting at `seek_seconds`.
#[must_use]
pub fn cropdetect_args(input: &str, seek_seconds: f64) -> Vec<String> {
    let mut args = vec!["-hide_banner".to_string(), "-nostats".to_string()];
    if seek_seconds > 0.0 {
        args.extend(["-ss".to_string(), format!("{seek_seconds:.3}")]);
    }
    args.extend([
        "-i".to_string(),
        input.to_string(),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-frames:v".to_string(),
        SAMPLE_FRAMES.to_string(),
        "-vf".to_string(),
        // The default fractional limit scales with bit depth; an absolute one misses 10-bit
        // black. `skip=0` keeps single-frame images from producing no report.
        "cropdetect=round=2:reset=0:skip=0".to_string(),
        "-an".to_string(),
        "-sn".to_string(),
        "-dn".to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ]);
    args
}

/// The last crop `cropdetect` reported in `FFmpeg`'s stderr. With `reset=0` each report covers
/// every frame before it. All-black frames report a negative size and are ignored.
#[must_use]
pub fn parse_cropdetect_output(stderr: &str) -> Option<DetectedCrop> {
    stderr.lines().rev().find_map(parse_crop_line)
}

fn parse_crop_line(line: &str) -> Option<DetectedCrop> {
    let (_, value) = line.rsplit_once("crop=")?;
    let mut parts = value.split_whitespace().next()?.split(':');
    let mut next = || parts.next()?.parse::<i64>().ok();
    let (width, height, x, y) = (next()?, next()?, next()?, next()?);
    if width <= 0 || height <= 0 || x < 0 || y < 0 {
        return None;
    }
    Some(DetectedCrop {
        x: u32::try_from(x).ok()?,
        y: u32::try_from(y).ok()?,
        width: u32::try_from(width).ok()?,
        height: u32::try_from(height).ok()?,
    })
}

/// Combines the per-sample results. `None` when no sample found any picture.
#[must_use]
pub fn merge_samples(
    samples: impl IntoIterator<Item = Option<DetectedCrop>>,
) -> Option<DetectedCrop> {
    samples.into_iter().flatten().reduce(DetectedCrop::union)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LETTERBOXED: &str = "\
[Parsed_cropdetect_0 @ 0x1] x1:240 x2:1679 y1:140 y2:939 w:1440 h:800 x:240 y:140 pts:2048 t:0.166667 limit:24.000000 crop=1440:800:240:140
[Parsed_cropdetect_0 @ 0x1] x1:240 x2:1679 y1:132 y2:947 w:1440 h:816 x:240 y:132 pts:2560 t:0.208333 limit:24.000000 crop=1440:816:240:132
[out#0/null @ 0x2] video:1kB audio:0kB subtitle:0kB other streams:0kB global headers:0kB muxing overhead: unknown
";

    #[test]
    fn parses_the_last_reported_crop() {
        assert_eq!(
            parse_cropdetect_output(LETTERBOXED),
            Some(DetectedCrop {
                x: 240,
                y: 132,
                width: 1440,
                height: 816,
            })
        );
    }

    #[test]
    fn ignores_reports_from_all_black_frames() {
        let stderr = "[Parsed_cropdetect_0 @ 0x1] x1:639 x2:0 y1:359 y2:0 w:-638 h:-358 x:640 y:360 pts:1024 t:0.080000 limit:24.000000 crop=-638:-358:640:360\n";
        assert_eq!(parse_cropdetect_output(stderr), None);
        assert_eq!(parse_cropdetect_output(""), None);
    }

    #[test]
    fn union_keeps_picture_seen_by_either_sample() {
        let letterbox = DetectedCrop {
            x: 0,
            y: 132,
            width: 1920,
            height: 816,
        };
        let pillarbox = DetectedCrop {
            x: 240,
            y: 0,
            width: 1440,
            height: 1080,
        };
        assert_eq!(
            letterbox.union(pillarbox),
            DetectedCrop {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }
        );
    }

    #[test]
    fn merge_skips_samples_without_picture() {
        let crop = DetectedCrop {
            x: 0,
            y: 132,
            width: 1920,
            height: 816,
        };
        assert_eq!(merge_samples([None, Some(crop), None]), Some(crop));
        assert_eq!(merge_samples([None, None]), None);
    }

    #[test]
    fn samples_spread_across_known_durations() {
        assert_eq!(
            sample_times(Some(100.0)),
            vec![10.0, 30.0, 50.0, 70.0, 90.0]
        );
        assert_eq!(sample_times(None), vec![0.0]);
        assert_eq!(sample_times(Some(f64::NAN)), vec![0.0]);
    }

    #[test]
    fn args_seek_before_the_input_and_discard_output() {
        let args = cropdetect_args("in.mp4", 12.5);
        let input = args.iter().position(|arg| arg == "-i").unwrap();
        let seek = args.iter().position(|arg| arg == "-ss").unwrap();
        assert!(seek < input);
        assert_eq!(args[seek + 1], "12.500");
        assert!(args.ends_with(&["-f".to_string(), "null".to_string(), "-".to_string()]));
        assert!(!cropdetect_args("in.png", 0.0).contains(&"-ss".to_string()));
    }
}
