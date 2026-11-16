//! Project seconds to Source seconds, shared by editing and rendering.
//!
//! No eligibility tolerance or render-window intersection belongs here. Keep full
//! precision until a caller serializes an edit (millisecond time strings).
use crate::{cuts::output_cut_ranges, types::TimelineSource, utils::parse_time};

/// The Source instant at a project time.
///
/// At a cut edge, use the first retained instant after the removed segment,
/// matching the renderer's seek convention.
#[must_use]
pub fn source_time(clip: &TimelineSource, project_time: f64) -> f64 {
    let offset = std::time::Duration::from_millis(clip.offset_millis).as_secs_f64();
    let trim = clip
        .config
        .start_time
        .as_deref()
        .and_then(parse_time)
        .unwrap_or(0.0);
    let mut elapsed = (project_time - offset).max(0.0) * clip.speed;
    for cut in output_cut_ranges(&clip.config) {
        // Project offset subtraction and speed multiplication can land a few
        // ulps before an exact edge. This tolerance is far below edit precision.
        if cut.start - elapsed > 1e-9 {
            break;
        }
        elapsed += cut.duration();
    }
    trim + elapsed
}

/// Full-precision edited project duration.
///
/// Editing applies its own minimum-length tolerance; composition extents use
/// `duration_millis` to retain their existing millisecond truncation.
#[must_use]
pub fn duration_seconds(clip: &TimelineSource) -> f64 {
    let seconds = crate::cuts::output_duration_seconds(&clip.config).unwrap_or_else(|| {
        std::time::Duration::from_millis(clip.duration_millis).as_secs_f64()
            - clip
                .config
                .start_time
                .as_deref()
                .and_then(parse_time)
                .unwrap_or(0.0)
    });
    (seconds / clip.speed).max(0.0)
}

#[must_use]
pub fn duration_millis(clip: &TimelineSource) -> u64 {
    std::time::Duration::from_secs_f64(duration_seconds(clip))
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
