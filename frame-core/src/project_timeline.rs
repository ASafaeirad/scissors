//! A finite project composition shared by preview and export.
use crate::{
    cuts::{
        CutRange, build_audio_cut_filters, build_video_cut_filters, output_cut_ranges,
        output_duration_seconds,
    },
    error::ConversionError,
    filters::{
        build_audio_filters, build_overlay_filter_complex, build_video_filters, has_overlay,
    },
    types::{ConversionConfig, TimelineSource},
    utils::parse_time,
};

#[must_use]
pub fn clip_duration_millis(clip: &TimelineSource) -> u64 {
    let seconds = output_duration_seconds(&clip.config).unwrap_or_else(|| {
        std::time::Duration::from_millis(clip.duration_millis).as_secs_f64()
            - clip
                .config
                .start_time
                .as_deref()
                .and_then(parse_time)
                .unwrap_or(0.0)
    });
    std::time::Duration::from_secs_f64((seconds / clip.speed).max(0.0))
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[must_use]
pub fn duration_seconds(clips: &[TimelineSource]) -> f64 {
    std::time::Duration::from_millis(
        clips
            .iter()
            .map(|clip| {
                clip.offset_millis
                    .saturating_add(clip_duration_millis(clip))
            })
            .max()
            .unwrap_or(0),
    )
    .as_secs_f64()
}

/// Validate every referenced source before starting an export.
///
/// # Errors
/// Returns an error for absent sources, invalid trims, or nested compositions.
pub fn validate(config: &ConversionConfig) -> Result<(), ConversionError> {
    if config.processing_mode == "copy" {
        return Err(ConversionError::InvalidInput(
            "Project export requires re-encoding".to_string(),
        ));
    }
    for clip in &config.timeline_sources {
        if !clip.speed.is_finite() || !(0.5..=20.0).contains(&clip.speed) {
            return Err(ConversionError::InvalidInput(
                "Track speed must be between 0.5x and 20x".to_string(),
            ));
        }
        if !std::path::Path::new(&clip.path).is_file() || clip.duration_millis == 0 {
            return Err(ConversionError::InvalidInput(format!(
                "Project source is missing or has no duration: {}",
                clip.path
            )));
        }
        if !clip.config.timeline_sources.is_empty()
            || !matches!(clip.kind.as_str(), "audio" | "video" | "image")
        {
            return Err(ConversionError::InvalidInput(
                "Invalid project source kind or nested project".to_string(),
            ));
        }
        let start = clip
            .config
            .start_time
            .as_deref()
            .and_then(parse_time)
            .unwrap_or(0.0);
        let end = clip
            .config
            .end_time
            .as_deref()
            .and_then(parse_time)
            .unwrap_or_else(|| {
                std::time::Duration::from_millis(clip.duration_millis).as_secs_f64()
            });
        if start < 0.0
            || end <= start
            || !start.is_finite()
            || !end.is_finite()
            || end > std::time::Duration::from_millis(clip.duration_millis).as_secs_f64() + 0.001
        {
            return Err(ConversionError::InvalidInput(format!(
                "Invalid project clip trim: {}",
                clip.path
            )));
        }
        crate::cuts::validate_cut_segments(&clip.config)?;
        crate::media_filters::validate_media_filters(&clip.config)?;
    }
    Ok(())
}

/// The span of project time to render, and which streams to build.
#[derive(Clone, Copy, Debug)]
pub struct RenderWindow {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Project seconds.
    pub start: f64,
    pub end: f64,
    pub video: bool,
    pub audio: bool,
    pub realtime: bool,
}

/// The part of one clip that lands inside a render window.
#[derive(Clone, Debug, PartialEq)]
struct ClipSpan {
    /// Source seconds the input is seeked to.
    seek: f64,
    /// Source seconds read after `seek`.
    length: f64,
    /// Cut ranges relative to `seek`.
    cuts: Vec<CutRange>,
    /// Clip output seconds that fall before the window.
    skip: f64,
    /// Window seconds where the clip appears and disappears.
    from: f64,
    to: f64,
}

/// Where `clip` falls in `[start, end)`, or `None` when it is outside it.
fn clip_span(clip: &TimelineSource, start: f64, end: f64) -> Option<ClipSpan> {
    let config = &clip.config;
    let offset = std::time::Duration::from_millis(clip.offset_millis).as_secs_f64();
    let length = std::time::Duration::from_millis(clip_duration_millis(clip)).as_secs_f64();
    if length <= 0.0 || offset >= end || offset + length <= start {
        return None;
    }
    let trim_start = config
        .start_time
        .as_deref()
        .and_then(parse_time)
        .unwrap_or(0.0);
    let trim_end = config
        .end_time
        .as_deref()
        .and_then(parse_time)
        .unwrap_or_else(|| std::time::Duration::from_millis(clip.duration_millis).as_secs_f64());
    let skip = (start - offset).max(0.0);
    // Cuts before the window still consume source time.
    let cuts = output_cut_ranges(config);
    let mut source = skip * clip.speed;
    for cut in &cuts {
        if cut.start > source {
            break;
        }
        source += cut.duration();
    }
    Some(ClipSpan {
        seek: trim_start + source,
        length: (trim_end - trim_start - source).max(0.0),
        cuts: cuts
            .into_iter()
            .filter(|cut| cut.end > source)
            .map(|cut| CutRange {
                start: cut.start - source,
                end: cut.end - source,
            })
            .collect(),
        skip,
        from: (offset - start).max(0.0),
        to: (offset + length).min(end) - start,
    })
}

/// Add the inputs and filter graph that render `window` of the project.
///
/// Only clips inside the window are opened, each seeked to its first visible
/// frame, so rendering late in a long project does not decode everything
/// before it. Outputs are `[project_video_out]` and `[project_audio_out]`.
#[expect(
    clippy::too_many_lines,
    reason = "Each clip supplies visual and audio chains to one shared composition graph."
)]
pub fn add_graph(args: &mut Vec<String>, clips: &[TimelineSource], window: &RenderWindow) {
    let RenderWindow {
        width,
        height,
        fps,
        start,
        end,
        video,
        audio,
        realtime,
    } = *window;
    let duration = (end - start).max(0.0);
    let visible: Vec<_> = clips
        .iter()
        .filter_map(|clip| Some((clip, clip_span(clip, start, end)?)))
        .collect();
    for (clip, span) in &visible {
        if span.seek > 0.0 {
            args.extend(["-ss".to_string(), format!("{:.6}", span.seek)]);
        }
        if clip.kind == "image" {
            args.extend([
                "-loop".to_string(),
                "1".to_string(),
                "-framerate".to_string(),
                fps.to_string(),
            ]);
        }
        args.extend(["-i".to_string(), clip.path.clone()]);
    }
    for (clip, _) in &visible {
        if let Some(overlay) = &clip.config.overlay
            && overlay.enabled
        {
            args.extend(["-i".to_string(), overlay.path.clone()]);
        }
    }

    let mut chains = Vec::new();
    let mut audio_labels = Vec::new();
    let mut visual_label = "project_canvas".to_string();
    if video {
        chains.push(format!(
            "color=c=black:s={width}x{height}:r={fps}:d={duration:.6}[{visual_label}]"
        ));
    }
    let mut overlay_input = visible.len();
    for (index, (clip, span)) in visible.iter().enumerate() {
        let config = &clip.config;
        let length = span.length;
        if video && clip.kind != "audio" && !clip.hidden {
            let input_label = if has_overlay(config) {
                let mut overlay_config = (**config).clone();
                overlay_config.resolution = "original".to_string();
                let overlay_graph = build_overlay_filter_complex(&overlay_config)
                    .replace("[0:v:0]", &format!("[{index}:v:0]"))
                    .replace("[0:v]", &format!("[{index}:v:0]"))
                    .replace("[1:v:0]", &format!("[{overlay_input}:v:0]"));
                // Every intermediate label must be unique across instances.
                let mut overlay_graph = overlay_graph;
                for label in [
                    "base",
                    "base_ref",
                    "base_out",
                    "overlay_src",
                    "overlay_scaled",
                    "vout",
                ] {
                    overlay_graph = overlay_graph
                        .replace(&format!("[{label}]"), &format!("[clip_{index}_{label}]"));
                }
                chains.push(overlay_graph);
                format!("clip_{index}_vout")
            } else {
                format!("{index}:v:0")
            };
            let mut filters = vec![
                format!("trim=end={length:.6}"),
                "setpts=PTS-STARTPTS".to_string(),
            ];
            filters.extend(build_video_cut_filters(&span.cuts));
            if !has_overlay(config) {
                let clip_filters = build_video_filters(config, false);
                filters.extend(clip_filters);
            }
            if (clip.speed - 1.0).abs() > f64::EPSILON {
                filters.push(format!("setpts=(PTS-STARTPTS)/{:.6}", clip.speed));
            }
            filters.extend([
                format!("scale={width}:{height}:force_original_aspect_ratio=decrease"),
                format!("pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black"),
                "setsar=1".to_string(),
                format!("fps={fps}"),
                "format=yuv420p".to_string(),
                format!("setpts=PTS-STARTPTS+{:.6}/TB", span.from),
            ]);
            chains.push(format!(
                "[{input_label}]{}[clip_v_{index}]",
                filters.join(",")
            ));
            let next = format!("project_canvas_{index}");
            chains.push(format!(
                "[{visual_label}][clip_v_{index}]overlay=eof_action=pass:repeatlast=0:enable='gte(t,{:.6})*lt(t,{:.6})'[{next}]",
                span.from, span.to
            ));
            visual_label = next;
        }
        if has_overlay(config) {
            overlay_input += 1;
        }
        if audio && !clip.muted {
            for track in &config.selected_audio_tracks {
                let label = format!("clip_a_{index}_{track}");
                let mut filters = vec![
                    format!("atrim=end={length:.6}"),
                    "asetpts=PTS-STARTPTS".to_string(),
                ];
                filters.extend(build_audio_cut_filters(&span.cuts));
                filters.extend(build_audio_filters(config));
                filters.extend(speed_audio_filters(clip.speed));
                filters.extend([
                    "aresample=48000".to_string(),
                    "aformat=sample_fmts=fltp:channel_layouts=stereo".to_string(),
                ]);
                let fit = format!("apad,atrim=duration={duration:.6}");
                if span.from > 0.0 {
                    // Lead with real silence. `adelay` emits its padding without timestamps
                    // when the input is seeked, so later clips would play from the start.
                    chains.push(format!(
                        "[{index}:{track}]{}[{label}_clip]",
                        filters.join(",")
                    ));
                    chains.push(format!(
                        "anullsrc=r=48000:cl=stereo,atrim=duration={:.6}[{label}_lead]",
                        span.from
                    ));
                    chains.push(format!(
                        "[{label}_lead][{label}_clip]concat=n=2:v=0:a=1,{fit}[{label}]"
                    ));
                } else {
                    filters.push(fit);
                    chains.push(format!("[{index}:{track}]{}[{label}]", filters.join(",")));
                }
                audio_labels.push(format!("[{label}]"));
            }
        }
    }
    if video {
        chains.push(format!(
            "[{visual_label}]trim=end={duration:.6},setpts=PTS-STARTPTS{}[project_video_out]",
            if realtime { ",realtime" } else { "" }
        ));
    }
    if audio {
        if audio_labels.is_empty() {
            chains.push(format!(
                "anullsrc=r=48000:cl=stereo,atrim=duration={duration:.6}[project_silence]"
            ));
            audio_labels.push("[project_silence]".to_string());
        }
        chains.push(format!(
            "{}amix=inputs={}:duration=longest:normalize=0,atrim=end={duration:.6},asetpts=PTS-STARTPTS{}[project_audio_out]",
            audio_labels.join(""),
            audio_labels.len(),
            if realtime { ",arealtime" } else { "" }
        ));
    }
    args.extend(["-filter_complex".to_string(), chains.join(";")]);
}

/// Keep each tempo stage within 0.5..=2 to preserve audio quality at high rates.
fn speed_audio_filters(mut speed: f64) -> Vec<String> {
    let mut filters = Vec::new();
    loop {
        if speed <= 2.0 {
            break;
        }
        filters.push("atempo=2.000000".to_string());
        speed /= 2.0;
    }
    if (speed - 1.0).abs() > f64::EPSILON {
        filters.push(format!("atempo={speed:.6}"));
    }
    filters
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(
        offset_millis: u64,
        start: Option<&str>,
        end: &str,
        cuts: &[(&str, &str)],
    ) -> TimelineSource {
        let mut config: ConversionConfig = serde_json::from_value(serde_json::json!({
            "container": "mp4",
            "videoCodec": "libx264",
            "videoBitrateMode": "crf",
            "videoBitrate": "5000",
            "audioCodec": "aac",
            "audioBitrate": "192",
            "audioBitrateMode": "bitrate",
            "audioQuality": "4",
            "audioChannels": "original",
            "audioVolume": 100.0,
            "audioNormalize": false,
            "selectedAudioTracks": [1],
            "selectedSubtitleTracks": [],
            "resolution": "original",
            "scalingAlgorithm": "lanczos",
            "fps": "original",
            "crf": 23,
            "quality": 50,
            "preset": "medium",
            "metadata": {"mode": "preserve"},
            "rotation": "0",
            "flipHorizontal": false,
            "flipVertical": false,
        }))
        .unwrap();
        config.start_time = start.map(str::to_string);
        config.end_time = Some(end.to_string());
        config.cut_segments = cuts
            .iter()
            .map(|(start, end)| crate::types::CutSegment {
                start_time: (*start).to_string(),
                end_time: (*end).to_string(),
            })
            .collect();
        TimelineSource {
            path: format!("/media/{offset_millis}.mp4"),
            kind: "video".to_string(),
            offset_millis,
            duration_millis: 60_000,
            muted: false,
            speed: 1.0,
            hidden: false,
            config: Box::new(config),
        }
    }

    fn window(start: f64, end: f64) -> RenderWindow {
        RenderWindow {
            width: 64,
            height: 36,
            fps: 30,
            start,
            end,
            video: true,
            audio: true,
            realtime: false,
        }
    }

    #[test]
    fn spans_seek_each_clip_to_its_first_visible_frame() {
        let clip = clip(10_000, Some("5"), "25", &[]);
        assert_eq!(clip_span(&clip, 0.0, 9.0), None);
        assert_eq!(clip_span(&clip, 30.0, 40.0), None);
        assert_eq!(
            clip_span(&clip, 0.0, 40.0),
            Some(ClipSpan {
                seek: 5.0,
                length: 20.0,
                cuts: Vec::new(),
                skip: 0.0,
                from: 10.0,
                to: 30.0,
            })
        );
        assert_eq!(
            clip_span(&clip, 22.0, 26.0),
            Some(ClipSpan {
                seek: 17.0,
                length: 8.0,
                cuts: Vec::new(),
                skip: 12.0,
                from: 0.0,
                to: 4.0,
            })
        );
    }

    #[test]
    fn spans_skip_cuts_before_the_window_and_shift_later_ones() {
        // Output 0-2 is source 0-2, source 2-4 is cut, output 2-6 is source 4-8, 8-9 is cut.
        let clip = clip(0, None, "12", &[("2", "4"), ("8", "9")]);
        let span = clip_span(&clip, 3.0, 5.0).unwrap();
        assert!((span.seek - 5.0).abs() < 1e-9);
        assert!((span.length - 7.0).abs() < 1e-9);
        assert_eq!(span.cuts.len(), 1);
        assert!((span.cuts[0].start - 3.0).abs() < 1e-9);
        assert!((span.cuts[0].end - 4.0).abs() < 1e-9);
    }

    #[test]
    fn speed_changes_duration_and_seek_after_cuts() {
        let mut clip = clip(1_000, Some("5"), "25", &[("7", "9")]);
        clip.speed = 2.0;
        assert_eq!(clip_duration_millis(&clip), 9_000);
        let span = clip_span(&clip, 4.0, 6.0).unwrap();
        assert!((span.seek - 13.0).abs() < f64::EPSILON);
        assert!((span.length - 12.0).abs() < f64::EPSILON);
        assert!(span.cuts.is_empty());
        assert!((span.to - 2.0).abs() < f64::EPSILON);
        clip.speed = 0.5;
        assert_eq!(clip_duration_millis(&clip), 36_000);
    }

    #[test]
    fn speed_graph_retimes_video_and_preserves_audio_pitch() {
        for speed in [0.5, 2.0, 20.0] {
            let mut clip = clip(0, None, "20", &[]);
            clip.speed = speed;
            let mut args = Vec::new();
            add_graph(&mut args, &[clip], &window(0.0, 40.0));
            let graph = args.last().unwrap();
            assert!(graph.contains(&format!("setpts=(PTS-STARTPTS)/{speed:.6}")));
            assert!(graph.contains(&speed_audio_filters(speed).join(",")));
        }
        assert_eq!(
            speed_audio_filters(20.0),
            [
                "atempo=2.000000",
                "atempo=2.000000",
                "atempo=2.000000",
                "atempo=2.000000",
                "atempo=1.250000",
            ]
        );
        assert!(speed_audio_filters(1.0).is_empty());
    }

    #[test]
    fn legacy_timeline_sources_default_to_normal_speed() {
        let mut value = serde_json::to_value(clip(0, None, "20", &[])).unwrap();
        value.as_object_mut().unwrap().remove("speed");
        let restored: TimelineSource = serde_json::from_value(value).unwrap();
        assert!((restored.speed - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hidden_clips_keep_their_audio_but_leave_the_picture() {
        let mut clip = clip(0, None, "20", &[]);
        clip.hidden = true;
        let mut args = Vec::new();
        add_graph(&mut args, &[clip], &window(0.0, 20.0));
        let graph = args.last().unwrap();
        assert!(!graph.contains("[0:v:0]"));
        assert!(graph.contains("[0:1]atrim=end=20.000000,"));
    }

    #[test]
    fn invalid_speeds_are_rejected_before_rendering() {
        for speed in [0.0, 0.49, 20.1, f64::NAN, f64::INFINITY] {
            let mut clip = clip(0, None, "20", &[]);
            clip.speed = speed;
            let mut config = (*clip.config).clone();
            config.timeline_sources = vec![clip];
            assert!(
                validate(&config)
                    .unwrap_err()
                    .to_string()
                    .contains("Track speed")
            );
        }
    }

    #[test]
    fn graph_opens_only_clips_inside_the_window() {
        let clips = [clip(0, None, "10", &[]), clip(10_000, None, "10", &[])];
        let mut args = Vec::new();
        add_graph(&mut args, &clips, &window(12.0, 14.0));
        assert_eq!(
            args[..args.len() - 2],
            ["-ss", "2.000000", "-i", "/media/10000.mp4"]
        );
        let graph = args.last().unwrap();
        assert!(graph.contains("color=c=black:s=64x36:r=30:d=2.000000"));
        assert!(graph.contains("[0:v:0]trim=end=8.000000,"));
        assert!(graph.contains("[0:1]atrim=end=8.000000,"));
        assert!(!graph.contains("[1:"));

        let mut args = Vec::new();
        add_graph(&mut args, &clips, &window(0.0, 20.0));
        assert_eq!(
            args[..args.len() - 2],
            ["-i", "/media/0.mp4", "-i", "/media/10000.mp4"]
        );
        assert!(
            args.last()
                .unwrap()
                .contains("enable='gte(t,10.000000)*lt(t,20.000000)'")
        );
    }
}
