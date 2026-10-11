//! A finite project composition shared by preview and export.
use crate::{
    cuts::{
        build_audio_cut_filters, build_video_cut_filters, output_cut_ranges,
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
    std::time::Duration::from_secs_f64(seconds.max(0.0))
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

/// Add one input for every clip instance, then any clip overlays.
pub fn add_inputs(args: &mut Vec<String>, clips: &[TimelineSource], fps: u32) {
    for clip in clips {
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
    for clip in clips {
        if let Some(overlay) = &clip.config.overlay
            && overlay.enabled
        {
            args.extend(["-i".to_string(), overlay.path.clone()]);
        }
    }
}

/// Build only the requested output streams. Times are project seconds.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "The graph has explicit stream, geometry, and time bounds."
)]
#[expect(
    clippy::too_many_lines,
    reason = "Each clip supplies visual and audio chains to one shared composition graph."
)]
pub fn graph(
    clips: &[TimelineSource],
    width: u32,
    height: u32,
    fps: u32,
    start: f64,
    end: f64,
    video: bool,
    audio: bool,
    realtime: bool,
) -> String {
    let duration = duration_seconds(clips);
    let mut chains = Vec::new();
    let mut audio_labels = Vec::new();
    let mut visual_label = "project_canvas".to_string();
    if video {
        chains.push(format!(
            "color=c=black:s={width}x{height}:r={fps}:d={duration:.6}[{visual_label}]"
        ));
    }
    let mut overlay_input = clips.len();
    for (index, clip) in clips.iter().enumerate() {
        let config = &clip.config;
        let trim_start = config
            .start_time
            .as_deref()
            .and_then(parse_time)
            .unwrap_or(0.0);
        let trim_end = config
            .end_time
            .as_deref()
            .and_then(parse_time)
            .unwrap_or_else(|| {
                std::time::Duration::from_millis(clip.duration_millis).as_secs_f64()
            });
        let offset = std::time::Duration::from_millis(clip.offset_millis).as_secs_f64();
        let clip_duration =
            std::time::Duration::from_millis(clip_duration_millis(clip)).as_secs_f64();
        let cuts = output_cut_ranges(config);
        if video && clip.kind != "audio" {
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
                format!("trim=start={trim_start:.6}:end={trim_end:.6}"),
                "setpts=PTS-STARTPTS".to_string(),
            ];
            filters.extend(build_video_cut_filters(&cuts));
            if !has_overlay(config) {
                filters.extend(build_video_filters(config, false));
            }
            filters.extend([
                format!("scale={width}:{height}:force_original_aspect_ratio=decrease"),
                format!("pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black"),
                "setsar=1".to_string(),
                format!("fps={fps}"),
                "format=yuv420p".to_string(),
                format!("setpts=PTS-STARTPTS+{offset:.6}/TB"),
            ]);
            chains.push(format!(
                "[{input_label}]{}[clip_v_{index}]",
                filters.join(",")
            ));
            let next = format!("project_canvas_{index}");
            let clip_end = offset + clip_duration;
            chains.push(format!("[{visual_label}][clip_v_{index}]overlay=eof_action=pass:repeatlast=0:enable='gte(t,{offset:.6})*lt(t,{clip_end:.6})'[{next}]"));
            visual_label = next;
        }
        if has_overlay(config) {
            overlay_input += 1;
        }
        if audio && !clip.muted {
            for track in &config.selected_audio_tracks {
                let label = format!("clip_a_{index}_{track}");
                let mut filters = vec![
                    format!("atrim=start={trim_start:.6}:end={trim_end:.6}"),
                    "asetpts=PTS-STARTPTS".to_string(),
                ];
                filters.extend(build_audio_cut_filters(&cuts));
                filters.extend(build_audio_filters(config));
                filters.extend([
                    "aresample=48000".to_string(),
                    "aformat=sample_fmts=fltp:channel_layouts=stereo".to_string(),
                    format!("adelay={}:all=1", clip.offset_millis),
                    "apad".to_string(),
                    format!("atrim=duration={duration:.6}"),
                ]);
                chains.push(format!("[{index}:{track}]{}[{label}]", filters.join(",")));
                audio_labels.push(format!("[{label}]"));
            }
        }
    }
    if video {
        chains.push(format!("[{visual_label}]trim=start={start:.6}:end={end:.6},setpts=PTS-STARTPTS{}[project_video_out]", if realtime { ",realtime" } else { "" }));
    }
    if audio {
        if audio_labels.is_empty() {
            chains.push(format!(
                "anullsrc=r=48000:cl=stereo,atrim=duration={duration:.6}[project_silence]"
            ));
            audio_labels.push("[project_silence]".to_string());
        }
        chains.push(format!("{}amix=inputs={}:duration=longest:normalize=0,atrim=start={start:.6}:end={end:.6},asetpts=PTS-STARTPTS{}[project_audio_out]", audio_labels.join(""), audio_labels.len(), if realtime { ",arealtime" } else { "" }));
    }
    chains.join(";")
}
