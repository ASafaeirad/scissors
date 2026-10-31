//! Removal of user-selected source ranges ("cuts") from exported media.
//!
//! Cuts are applied in the filter graph: `select`/`aselect` drop the frames
//! inside each removed range and the following `setpts`/`asetpts` close the
//! resulting gaps so the output timeline stays continuous.

use crate::error::ConversionError;
use crate::types::ConversionConfig;
use crate::utils::parse_time;

/// Ranges shorter than this are ignored; they cannot remove a whole frame.
const MIN_CUT_SECONDS: f64 = 0.001;

/// A half-open `[start, end)` time range in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutRange {
    pub start: f64,
    pub end: f64,
}

impl CutRange {
    #[must_use]
    pub fn duration(self) -> f64 {
        self.end - self.start
    }
}

fn trim_bound(value: Option<&str>) -> Option<f64> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(parse_time)
}

/// Validates the configured cut segments.
///
/// # Errors
///
/// Returns [`ConversionError`] when a segment has malformed or inverted
/// timecodes, or when the segments remove the whole trimmed range.
pub fn validate_cut_segments(config: &ConversionConfig) -> Result<(), ConversionError> {
    for segment in &config.cut_segments {
        let start = parse_time(segment.start_time.trim()).filter(|value| *value >= 0.0);
        let end = parse_time(segment.end_time.trim()).filter(|value| *value >= 0.0);
        let (Some(start), Some(end)) = (start, end) else {
            return Err(ConversionError::InvalidInput(format!(
                "Invalid removed segment: {} - {}",
                segment.start_time, segment.end_time
            )));
        };
        if end <= start {
            return Err(ConversionError::InvalidInput(format!(
                "Removed segment end must be greater than its start: {} - {}",
                segment.start_time, segment.end_time
            )));
        }
    }

    if let Some(duration) = output_duration_seconds(config)
        && duration <= MIN_CUT_SECONDS
        && !output_cut_ranges(config).is_empty()
    {
        return Err(ConversionError::InvalidInput(
            "Removed segments cover the entire trimmed range".to_string(),
        ));
    }

    Ok(())
}

/// Returns the cut ranges on the timeline seen by the filter graph.
///
/// The input is seeked to `start_time`, so ranges are shifted by the trim
/// start, clipped to the trim window, sorted, and merged. Malformed segments
/// are skipped; [`validate_cut_segments`] reports them.
#[must_use]
pub fn output_cut_ranges(config: &ConversionConfig) -> Vec<CutRange> {
    let trim_start = trim_bound(config.start_time.as_deref()).unwrap_or(0.0);
    let trim_end = trim_bound(config.end_time.as_deref()).unwrap_or(f64::INFINITY);

    let mut ranges: Vec<CutRange> = config
        .cut_segments
        .iter()
        .filter_map(|segment| {
            let start = parse_time(segment.start_time.trim())?.max(trim_start);
            let end = parse_time(segment.end_time.trim())?.min(trim_end);
            (start.is_finite() && end - start >= MIN_CUT_SECONDS).then_some(CutRange {
                start: start - trim_start,
                end: end - trim_start,
            })
        })
        .collect();
    ranges.sort_by(|left, right| left.start.total_cmp(&right.start));

    let mut merged: Vec<CutRange> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// Total seconds removed from the trimmed range by cut segments.
#[must_use]
pub fn removed_seconds(config: &ConversionConfig) -> f64 {
    output_cut_ranges(config)
        .into_iter()
        .map(CutRange::duration)
        .sum()
}

/// Expected output duration when an end time is set, accounting for cuts.
#[must_use]
pub fn output_duration_seconds(config: &ConversionConfig) -> Option<f64> {
    let start = trim_bound(config.start_time.as_deref()).unwrap_or(0.0);
    let end = trim_bound(config.end_time.as_deref())?;
    Some((end - start - removed_seconds(config)).max(0.0))
}

fn format_seconds(seconds: f64) -> String {
    format!("{seconds:.3}")
}

fn keep_expression(ranges: &[CutRange]) -> String {
    let removed = ranges
        .iter()
        .map(|range| {
            format!(
                "gte(t,{})*lt(t,{})",
                format_seconds(range.start),
                format_seconds(range.end)
            )
        })
        .collect::<Vec<_>>()
        .join("+");
    format!("not({removed})")
}

/// Video filters that drop cut ranges and close the gaps they leave.
///
/// Timestamps are shifted by the removed time before each frame rather than
/// regenerated from the frame count, which keeps variable frame rate sources
/// intact.
#[must_use]
pub fn build_video_cut_filters(ranges: &[CutRange]) -> Vec<String> {
    if ranges.is_empty() {
        return Vec::new();
    }

    let shift = ranges
        .iter()
        .map(|range| {
            format!(
                "clip(T-{},0,{})",
                format_seconds(range.start),
                format_seconds(range.duration())
            )
        })
        .collect::<Vec<_>>()
        .join("+");

    vec![
        format!("select='{}'", keep_expression(ranges)),
        format!("setpts='PTS-({shift})/TB'"),
    ]
}

/// Audio filters that drop cut ranges and re-time the remaining samples.
#[must_use]
pub fn build_audio_cut_filters(ranges: &[CutRange]) -> Vec<String> {
    if ranges.is_empty() {
        return Vec::new();
    }

    vec![
        format!("aselect='{}'", keep_expression(ranges)),
        "asetpts=N/SR/TB".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CutSegment;

    fn config_with(
        start: Option<&str>,
        end: Option<&str>,
        cuts: &[(&str, &str)],
    ) -> ConversionConfig {
        let mut config: ConversionConfig = serde_json::from_value(serde_json::json!({
            "container": "mp4",
            "videoCodec": "libx264",
            "videoBitrateMode": "crf",
            "videoBitrate": "5000",
            "audioCodec": "aac",
            "audioBitrate": "192",
            "audioChannels": "original",
            "selectedAudioTracks": [],
            "selectedSubtitleTracks": [],
            "subtitleBurnPath": null,
            "resolution": "original",
            "customWidth": null,
            "customHeight": null,
            "scalingAlgorithm": "bicubic",
            "fps": "original",
            "crf": 23,
            "preset": "medium",
            "startTime": null,
            "endTime": null,
        }))
        .expect("minimal config should deserialize");
        config.start_time = start.map(str::to_string);
        config.end_time = end.map(str::to_string);
        config.cut_segments = cuts
            .iter()
            .map(|(start, end)| CutSegment {
                start_time: (*start).to_string(),
                end_time: (*end).to_string(),
            })
            .collect();
        config
    }

    #[test]
    fn output_cut_ranges_merge_overlaps_and_shift_by_trim_start() {
        let config = config_with(
            Some("00:00:10.000"),
            Some("00:01:00.000"),
            &[
                ("00:00:30.000", "00:00:40.000"),
                ("00:00:05.000", "00:00:12.000"),
                ("00:00:35.000", "00:00:45.000"),
                ("00:00:55.000", "00:01:10.000"),
            ],
        );

        assert_eq!(
            output_cut_ranges(&config),
            vec![
                CutRange {
                    start: 0.0,
                    end: 2.0
                },
                CutRange {
                    start: 20.0,
                    end: 35.0
                },
                CutRange {
                    start: 45.0,
                    end: 50.0
                },
            ]
        );
        assert_eq!(output_duration_seconds(&config), Some(28.0));
    }

    #[test]
    fn output_cut_ranges_drop_segments_outside_the_trim_window() {
        let config = config_with(
            Some("00:00:10.000"),
            Some("00:00:20.000"),
            &[
                ("00:00:01.000", "00:00:05.000"),
                ("00:00:25.000", "00:00:30.000"),
            ],
        );

        assert!(output_cut_ranges(&config).is_empty());
    }

    #[test]
    fn video_cut_filters_select_kept_frames_and_close_gaps() {
        let ranges = [
            CutRange {
                start: 2.0,
                end: 4.5,
            },
            CutRange {
                start: 10.0,
                end: 11.0,
            },
        ];

        assert_eq!(
            build_video_cut_filters(&ranges),
            vec![
                "select='not(gte(t,2.000)*lt(t,4.500)+gte(t,10.000)*lt(t,11.000))'".to_string(),
                "setpts='PTS-(clip(T-2.000,0,2.500)+clip(T-10.000,0,1.000))/TB'".to_string(),
            ]
        );
        assert_eq!(
            build_audio_cut_filters(&ranges),
            vec![
                "aselect='not(gte(t,2.000)*lt(t,4.500)+gte(t,10.000)*lt(t,11.000))'".to_string(),
                "asetpts=N/SR/TB".to_string(),
            ]
        );
    }

    #[test]
    fn validate_cut_segments_rejects_inverted_and_full_coverage_cuts() {
        let inverted = config_with(None, None, &[("00:00:05.000", "00:00:02.000")]);
        assert!(validate_cut_segments(&inverted).is_err());

        let malformed = config_with(None, None, &[("soon", "00:00:02.000")]);
        assert!(validate_cut_segments(&malformed).is_err());

        let everything = config_with(
            Some("00:00:02.000"),
            Some("00:00:04.000"),
            &[("00:00:00.000", "00:00:10.000")],
        );
        assert!(validate_cut_segments(&everything).is_err());

        let valid = config_with(None, None, &[("00:00:01.000", "00:00:02.000")]);
        assert!(validate_cut_segments(&valid).is_ok());
    }
}
