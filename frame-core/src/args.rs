use std::path::Path;

use crate::codec::{
    add_audio_codec_args, add_fps_args, add_stream_copy_video_tag_args, add_video_codec_args,
    audio_codec_supports_vbr,
};
use crate::container::{is_transport_stream_container, transport_stream_profile};
use crate::cuts::{
    CutRange, build_audio_cut_filters, build_video_cut_filters, output_cut_ranges,
    output_duration_seconds, validate_cut_segments,
};
use crate::error::ConversionError;
use crate::filters::{
    build_audio_filters, build_encode_overlay_filter_complex, build_encode_video_filters,
    build_overlay_filter_complex, build_video_filters, has_overlay,
};
use crate::media_filters::validate_media_filters;
use crate::media_rules::{
    all_containers, container_supports_audio, is_audio_codec_allowed,
    is_audio_stream_codec_allowed, is_image_container, is_video_codec_allowed,
    is_video_only_container, is_video_pixel_format_allowed, is_video_stream_codec_allowed,
};
use crate::types::{
    AudioTrack, ConversionConfig, MetadataConfig, MetadataMode, ProbeMetadata, VOLUME_EPSILON,
};
use crate::utils::{get_hwaccel_args, is_audio_only_container, parse_time};

fn build_project_args(
    output: &str,
    config: &ConversionConfig,
    probe: &ProbeMetadata,
) -> Result<Vec<String>, ConversionError> {
    crate::project_timeline::validate(config)?;
    let audio_only = is_audio_only_container(&config.container);
    if is_image_container(&config.container) || config.container == "gif" {
        return Err(ConversionError::InvalidInput(
            "Export a project to a video or audio container".to_string(),
        ));
    }
    let (mut width, mut height) = (probe.width.unwrap_or(1280), probe.height.unwrap_or(720));
    match config.resolution.as_str() {
        "custom" => {
            width = config
                .custom_width
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(width);
            height = config
                .custom_height
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(height);
        }
        "1080p" => {
            width = 1920;
            height = 1080;
        }
        "720p" => {
            width = 1280;
            height = 720;
        }
        "480p" => {
            width = 854;
            height = 480;
        }
        _ => {}
    }
    width = width.max(2) / 2 * 2;
    height = height.max(2) / 2 * 2;
    let fps = config.fps.parse::<u32>().unwrap_or(30).clamp(1, 120);
    let duration = crate::project_timeline::duration_seconds(&config.timeline_sources);
    let audio = container_supports_audio(&config.container) && config.audio_codec != "none";
    let mut args = vec!["-filter_complex_threads".to_string(), "1".to_string()];
    crate::project_timeline::add_graph(
        &mut args,
        &config.timeline_sources,
        &crate::project_timeline::RenderWindow {
            width,
            height,
            fps,
            start: 0.0,
            end: duration,
            video: !audio_only,
            audio,
            realtime: false,
        },
    );
    if !audio_only {
        args.extend(["-map".to_string(), "[project_video_out]".to_string()]);
        add_video_codec_args(&mut args, config);
        args.extend([
            "-pix_fmt".to_string(),
            if has_custom_pixel_format(config) {
                config.pixel_format.clone()
            } else {
                "yuv420p".to_string()
            },
        ]);
    }
    if audio {
        args.extend(["-map".to_string(), "[project_audio_out]".to_string()]);
        add_audio_codec_args(&mut args, config);
    }
    add_container_metadata_flags(&mut args, config, probe);
    args.push("-sn".to_string());
    add_container_output_args(&mut args, &config.container);
    args.extend([
        "-t".to_string(),
        format!("{duration:.6}"),
        "-n".to_string(),
        output.to_string(),
    ]);
    Ok(args)
}

fn is_copy_mode(config: &ConversionConfig) -> bool {
    config.processing_mode == "copy"
}

fn has_custom_pixel_format(config: &ConversionConfig) -> bool {
    let pixel_format = config.pixel_format.trim();
    !pixel_format.is_empty() && pixel_format != "auto"
}

fn collect_selected_audio_tracks<'a>(
    config: &ConversionConfig,
    probe: &'a ProbeMetadata,
) -> Result<Vec<&'a AudioTrack>, ConversionError> {
    config
        .selected_audio_tracks
        .iter()
        .map(|index| {
            probe
                .audio_tracks
                .iter()
                .find(|track| track.index == *index)
                .ok_or_else(|| {
                    ConversionError::InvalidInput(format!(
                        "Selected audio track #{index} was not found in source"
                    ))
                })
        })
        .collect()
}

fn add_track_maps<T>(args: &mut Vec<String>, tracks: &[&T], index: impl Fn(&T) -> u32) {
    for track in tracks {
        args.push("-map".to_string());
        args.push(format!("0:{}", index(track)));
    }
}

/// Validates whether stream-copy mode can preserve the selected source streams.
///
/// # Errors
///
/// Returns [`ConversionError`] when the selected source streams are missing or
/// incompatible with the requested output container.
pub fn validate_stream_copy_compatibility(
    config: &ConversionConfig,
    probe: &ProbeMetadata,
) -> Result<(), ConversionError> {
    if !is_copy_mode(config) {
        return Ok(());
    }

    let is_audio_only = is_audio_only_container(&config.container);

    if is_audio_only {
        let selected_audio = collect_selected_audio_tracks(config, probe)?;
        if selected_audio.is_empty() {
            return Err(ConversionError::InvalidInput(
                "Select at least one audio track for an audio-only output".to_string(),
            ));
        }
        for track in selected_audio {
            if !is_audio_stream_codec_allowed(&config.container, &track.codec) {
                return Err(ConversionError::InvalidInput(format!(
                    "Audio codec '{}' from source track #{} is incompatible with container '{}'",
                    track.codec, track.index, config.container
                )));
            }
        }
        return Ok(());
    }

    let video_codec = probe.video_codec.as_deref().ok_or_else(|| {
        ConversionError::InvalidInput(
            "Source has no video stream; choose an audio container for stream copy".to_string(),
        )
    })?;
    if !is_video_stream_codec_allowed(&config.container, video_codec) {
        return Err(ConversionError::InvalidInput(format!(
            "Video codec '{}' is incompatible with container '{}'",
            video_codec, config.container
        )));
    }

    if container_supports_audio(&config.container) {
        for track in collect_selected_audio_tracks(config, probe)? {
            if !is_audio_stream_codec_allowed(&config.container, &track.codec) {
                return Err(ConversionError::InvalidInput(format!(
                    "Audio codec '{}' from source track #{} is incompatible with container '{}'",
                    track.codec, track.index, config.container
                )));
            }
        }
    }

    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "FFmpeg command assembly stays in one place to keep ordering guarantees explicit"
)]
/// Builds probe-aware `FFmpeg` arguments for one conversion.
///
/// # Errors
///
/// Returns [`ConversionError`] when a selected source stream is missing or
/// cannot be represented by the requested output configuration.
pub fn build_ffmpeg_args(
    input: &str,
    output: &str,
    config: &ConversionConfig,
    probe: &ProbeMetadata,
) -> Result<Vec<String>, ConversionError> {
    if !config.timeline_sources.is_empty() {
        return build_project_args(output, config, probe);
    }
    let external_audio = crate::audio_timeline::has_external_audio(config);
    if external_audio && (!container_supports_audio(&config.container) || is_copy_mode(config)) {
        return Err(ConversionError::InvalidInput(
            "Added audio requires re-encoding to an audio-capable container, such as MP4"
                .to_string(),
        ));
    }
    let mut args = Vec::new();

    // Hardware decode acceleration (must be before -i)
    if config.hw_decode {
        args.extend(get_hwaccel_args(&config.video_codec));
    }

    if let Some(start) = &config.start_time
        && !start.is_empty()
    {
        args.push("-ss".to_string());
        args.push(start.clone());
    }

    args.push("-i".to_string());
    args.push(input.to_string());

    if has_overlay(config)
        && let Some(overlay) = &config.overlay
    {
        args.push("-i".to_string());
        args.push(overlay.path.clone());
    }

    let first_audio_input = 1 + usize::from(has_overlay(config));
    crate::audio_timeline::add_inputs(&mut args, config);

    let cut_ranges = output_cut_ranges(config);
    if let Some(end_str) = &config.end_time
        && !end_str.is_empty()
    {
        if !cut_ranges.is_empty() {
            // Cuts shorten the output timeline, so `-to` would overshoot the trim end.
            if let Some(duration) = output_duration_seconds(config)
                && duration > 0.0
            {
                args.push("-t".to_string());
                args.push(format!("{duration:.3}"));
            }
        } else if let Some(start_str) = &config.start_time {
            if start_str.is_empty() {
                args.push("-to".to_string());
                args.push(end_str.clone());
            } else if let (Some(start_t), Some(end_t)) =
                (parse_time(start_str), parse_time(end_str))
            {
                let duration = end_t - start_t;
                if duration > 0.0 {
                    args.push("-t".to_string());
                    args.push(format!("{duration:.3}"));
                }
            }
        } else {
            args.push("-to".to_string());
            args.push(end_str.clone());
        }
    }

    add_container_metadata_flags(&mut args, config, probe);
    args.push("-sn".to_string());

    let is_audio_only = is_audio_only_container(&config.container);
    let is_video_only = is_video_only_container(&config.container);
    let is_image_output = is_image_container(&config.container);
    let is_gif_output = config.container.eq_ignore_ascii_case("gif");
    let use_overlay = has_overlay(config) && !is_audio_only && !is_gif_output;
    if is_copy_mode(config) {
        validate_stream_copy_compatibility(config, probe)?;
        if !cut_ranges.is_empty() {
            return Err(ConversionError::InvalidInput(
                "Removing segments requires re-encoding".to_string(),
            ));
        }

        args.extend(["-c".to_string(), "copy".to_string()]);
        if !is_audio_only {
            args.push("-map".to_string());
            args.push(
                probe
                    .video_stream_index
                    .map_or_else(|| "0:v:0?".to_string(), |index| format!("0:{index}")),
            );
        }

        if container_supports_audio(&config.container) {
            let audio_tracks = collect_selected_audio_tracks(config, probe)?;
            add_track_maps(&mut args, &audio_tracks, |track| track.index);
            if audio_tracks.is_empty() {
                args.push("-an".to_string());
            }
        }

        if !is_audio_only {
            add_stream_copy_video_tag_args(
                &mut args,
                &config.container,
                probe.video_codec.as_deref(),
            );
        }
        args.push("-dn".to_string());
        add_container_output_args(&mut args, &config.container);
        args.push("-n".to_string());
        args.push(output.to_string());
        return Ok(args);
    }

    if is_audio_only {
        args.push("-vn".to_string());

        let audio_tracks = collect_selected_audio_tracks(config, probe)?;
        if audio_tracks.is_empty() && !external_audio {
            return Err(ConversionError::InvalidInput(
                "Select at least one audio track for an audio-only output".to_string(),
            ));
        }
        if !external_audio {
            add_track_maps(&mut args, &audio_tracks, |track| track.index);
        }

        add_audio_codec_args(&mut args, config);
    } else if is_video_only && is_gif_output {
        args.push("-filter_complex".to_string());
        args.push(build_gif_filter_complex(config, &cut_ranges));

        args.push("-map".to_string());
        args.push("[gif_out]".to_string());
        args.push("-an".to_string());

        args.push("-c:v".to_string());
        args.push("gif".to_string());

        args.push("-loop".to_string());
        args.push(config.gif_loop.to_string());
        args.push("-f".to_string());
        args.push("gif".to_string());
    } else if is_image_output {
        add_video_codec_args(&mut args, config);
        if has_custom_pixel_format(config) {
            args.push("-pix_fmt".to_string());
            args.push(config.pixel_format.trim().to_string());
        }

        if use_overlay {
            args.push("-filter_complex".to_string());
            args.push(build_overlay_filter_complex(config));
        } else {
            let video_filters = build_video_filters(config, true);
            if !video_filters.is_empty() {
                args.push("-vf".to_string());
                args.push(video_filters.join(","));
            }
        }

        args.push("-map".to_string());
        args.push(if use_overlay {
            "[vout]".to_string()
        } else {
            "0:v:0".to_string()
        });
        args.push("-frames:v".to_string());
        args.push("1".to_string());
        args.push("-update".to_string());
        args.push("1".to_string());
    } else {
        add_video_codec_args(&mut args, config);
        if has_custom_pixel_format(config) {
            args.push("-pix_fmt".to_string());
            args.push(config.pixel_format.trim().to_string());
        }

        let cut_filters = build_video_cut_filters(&cut_ranges);
        if use_overlay {
            args.push("-filter_complex".to_string());
            args.push(build_encode_overlay_filter_complex(config, &cut_filters));
        } else {
            let mut video_filters = build_encode_video_filters(config, true);
            video_filters.extend(cut_filters);
            if !video_filters.is_empty() {
                args.push("-vf".to_string());
                args.push(video_filters.join(","));
            }
        }

        add_fps_args(&mut args, config);
        args.push("-map".to_string());
        args.push(if use_overlay {
            "[vout]".to_string()
        } else {
            "0:v:0".to_string()
        });

        let audio_tracks = collect_selected_audio_tracks(config, probe)?;
        if !external_audio {
            add_track_maps(&mut args, &audio_tracks, |track| track.index);
        }

        if audio_tracks.is_empty() && !external_audio {
            args.push("-an".to_string());
        } else {
            add_audio_codec_args(&mut args, config);
        }
    }

    if external_audio {
        let start = config
            .start_time
            .as_deref()
            .and_then(parse_time)
            .unwrap_or(0.0);
        let end = config
            .end_time
            .as_deref()
            .and_then(parse_time)
            .or_else(|| probe.duration.as_deref().and_then(parse_time))
            .ok_or_else(|| {
                ConversionError::InvalidInput("Added audio requires a source duration".to_string())
            })?;
        let duration = end - start;
        if !duration.is_finite() || duration <= 0.0 {
            return Err(ConversionError::InvalidInput(
                "Audio timeline duration must be positive".to_string(),
            ));
        }
        collect_selected_audio_tracks(config, probe)?;
        let mut graph = crate::audio_timeline::mix_graph(
            config,
            first_audio_input,
            &config.selected_audio_tracks,
            start,
            duration,
        );
        let cuts = build_audio_cut_filters(&cut_ranges);
        if !cuts.is_empty() {
            graph = graph.replace(
                "[frame_audio_out]",
                &format!(",{}[frame_audio_out]", cuts.join(",")),
            );
        }
        crate::audio_timeline::add_graph(&mut args, graph);
    }
    if !external_audio
        && !is_video_only
        && !is_image_output
        && !config.selected_audio_tracks.is_empty()
    {
        let mut audio_filters = build_audio_cut_filters(&cut_ranges);
        audio_filters.extend(build_audio_filters(config));
        if !audio_filters.is_empty() {
            args.push("-af".to_string());
            args.push(audio_filters.join(","));
        }
    }

    args.push("-dn".to_string());
    add_container_output_args(&mut args, &config.container);
    args.push("-n".to_string());
    args.push(output.to_string());

    Ok(args)
}

fn add_container_output_args(args: &mut Vec<String>, container: &str) {
    let Some(profile) = transport_stream_profile(container) else {
        return;
    };
    args.push("-f".to_string());
    args.push("mpegts".to_string());
    args.push("-mpegts_m2ts_mode".to_string());
    args.push(profile.ffmpeg_m2ts_mode().to_string());
}

fn add_container_metadata_flags(
    args: &mut Vec<String>,
    config: &ConversionConfig,
    probe: &ProbeMetadata,
) {
    if !is_transport_stream_container(&config.container) {
        match config.metadata.mode {
            MetadataMode::Clean => {
                args.extend(["-map_metadata".to_string(), "-1".to_string()]);
            }
            MetadataMode::Replace => {
                args.extend(["-map_metadata".to_string(), "-1".to_string()]);
                add_metadata_flags(args, &config.metadata);
            }
            MetadataMode::Preserve => add_metadata_flags(args, &config.metadata),
        }
        return;
    }

    args.extend(["-map_metadata".to_string(), "-1".to_string()]);
    let source_transport = probe.transport_stream.as_ref();
    let source_tags = probe.tags.as_ref();
    let manual_name = non_empty(config.metadata.service_name.as_deref());
    let manual_provider = non_empty(config.metadata.service_provider.as_deref());
    let (service_name, service_provider) = match config.metadata.mode {
        MetadataMode::Preserve => (
            manual_name
                .or_else(|| {
                    source_transport.and_then(|value| non_empty(value.service_name.as_deref()))
                })
                .or_else(|| source_tags.and_then(|value| non_empty(value.title.as_deref())))
                .unwrap_or("Service01"),
            manual_provider
                .or_else(|| {
                    source_transport.and_then(|value| non_empty(value.service_provider.as_deref()))
                })
                .or_else(|| source_tags.and_then(|value| non_empty(value.artist.as_deref())))
                .unwrap_or("Frame"),
        ),
        MetadataMode::Replace => (
            manual_name.unwrap_or("Service01"),
            manual_provider.unwrap_or("Frame"),
        ),
        MetadataMode::Clean => ("Service01", "Frame"),
    };
    for value in [
        format!("service_name={service_name}"),
        format!("service_provider={service_provider}"),
    ] {
        args.push("-metadata".to_string());
        args.push(value);
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalize_gif_dither(dither: &str) -> &'static str {
    match dither {
        "none" => "none",
        "bayer" => "bayer",
        "floyd_steinberg" => "floyd_steinberg",
        _ => "sierra2_4a",
    }
}

fn build_gif_filter_complex(config: &ConversionConfig, cut_ranges: &[CutRange]) -> String {
    let mut filters = build_video_filters(config, true);
    filters.extend(build_video_cut_filters(cut_ranges));
    if config.fps != "original" {
        filters.push(format!("fps={}", config.fps));
    }

    let chain = if filters.is_empty() {
        "split[gif_src][gif_palette_src]".to_string()
    } else {
        format!("{},split[gif_src][gif_palette_src]", filters.join(","))
    };

    let colors = config.gif_colors.clamp(2, 256);
    let dither = normalize_gif_dither(&config.gif_dither);

    format!(
        "[0:v:0]{chain};[gif_palette_src]palettegen=max_colors={colors}:stats_mode=single[gif_palette];[gif_src][gif_palette]paletteuse=dither={dither}:new=1[gif_out]"
    )
}

pub fn add_metadata_flags(args: &mut Vec<String>, metadata: &MetadataConfig) {
    if let Some(v) = &metadata.title
        && !v.is_empty()
    {
        args.push("-metadata".to_string());
        args.push(format!("title={v}"));
    }
    if let Some(v) = &metadata.artist
        && !v.is_empty()
    {
        args.push("-metadata".to_string());
        args.push(format!("artist={v}"));
    }
    if let Some(v) = &metadata.album
        && !v.is_empty()
    {
        args.push("-metadata".to_string());
        args.push(format!("album={v}"));
    }
    if let Some(v) = &metadata.genre
        && !v.is_empty()
    {
        args.push("-metadata".to_string());
        args.push(format!("genre={v}"));
    }
    if let Some(v) = &metadata.date
        && !v.is_empty()
    {
        args.push("-metadata".to_string());
        args.push(format!("date={v}"));
    }
    if let Some(v) = &metadata.comment
        && !v.is_empty()
    {
        args.push("-metadata".to_string());
        args.push(format!("comment={v}"));
    }
}

fn sanitize_output_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = trimmed.rsplit(['/', '\\']).next().map_or("", str::trim);

    if candidate.is_empty() || candidate == "." || candidate == ".." {
        return None;
    }

    Some(candidate.to_string())
}

pub fn build_output_path(
    output_directory: &str,
    container: &str,
    output_name: Option<&str>,
) -> String {
    let output_name = output_name
        .and_then(sanitize_output_name)
        .unwrap_or_else(|| "output_converted".to_string());
    let output_stem = output_name
        .rsplit_once('.')
        .filter(|(stem, extension)| {
            !stem.is_empty()
                && all_containers()
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(extension))
        })
        .map_or(output_name.as_str(), |(stem, _)| stem);
    let separator = if output_directory.contains('\\') && !output_directory.contains('/') {
        "\\"
    } else {
        "/"
    };
    let directory = output_directory.trim_end_matches(['/', '\\']);

    format!("{directory}{separator}{output_stem}.{container}")
}

#[expect(
    clippy::too_many_lines,
    reason = "Validation intentionally mirrors UI options in one function for consistent backend guardrails"
)]
/// Validates a source path and conversion configuration before running `FFmpeg`.
///
/// # Errors
///
/// Returns [`ConversionError`] when the input path is invalid, trim bounds are
/// malformed, output settings are incompatible, or referenced sidecar assets do
/// not exist.
pub fn validate_task_input(
    file_path: &str,
    config: &ConversionConfig,
) -> Result<(), ConversionError> {
    if !config.timeline_sources.is_empty() {
        crate::project_timeline::validate(config)?;
        let mut output_config = config.clone();
        output_config.timeline_sources.clear();
        output_config.selected_audio_tracks.clear();
        return validate_task_input(file_path, &output_config);
    }
    let input_path = Path::new(file_path);
    if !input_path.exists() {
        return Err(ConversionError::InvalidInput(format!(
            "Input file does not exist: {file_path}"
        )));
    }
    if !input_path.is_file() {
        return Err(ConversionError::InvalidInput(format!(
            "Input path is not a file: {file_path}"
        )));
    }

    if crate::audio_timeline::has_external_audio(config)
        && (!container_supports_audio(&config.container) || is_copy_mode(config))
    {
        return Err(ConversionError::InvalidInput(
            "Added audio requires re-encoding to an audio-capable container, such as MP4"
                .to_string(),
        ));
    }
    for track in &config.external_audio_tracks {
        if !Path::new(&track.path).is_file() || track.duration_millis == 0 {
            return Err(ConversionError::InvalidInput(format!(
                "Audio clip is missing or has no duration: {}",
                track.path
            )));
        }
    }
    let start_time = config
        .start_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let end_time = config
        .end_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let processing_mode = config.processing_mode.trim();

    if processing_mode != "reencode" && processing_mode != "copy" {
        return Err(ConversionError::InvalidInput(format!(
            "Invalid processing mode: {processing_mode}"
        )));
    }
    validate_media_filters(config)?;
    let is_copy_mode = processing_mode == "copy";

    if let Some(start) = start_time
        && parse_time(start).is_none()
    {
        return Err(ConversionError::InvalidInput(format!(
            "Invalid start time: {start}"
        )));
    }

    if let Some(end) = end_time
        && parse_time(end).is_none()
    {
        return Err(ConversionError::InvalidInput(format!(
            "Invalid end time: {end}"
        )));
    }

    if let (Some(start), Some(end)) = (start_time, end_time)
        && let (Some(start_t), Some(end_t)) = (parse_time(start), parse_time(end))
        && end_t <= start_t
    {
        return Err(ConversionError::InvalidInput(
            "End time must be greater than start time".to_string(),
        ));
    }

    validate_cut_segments(config)?;
    if !output_cut_ranges(config).is_empty() {
        if is_copy_mode {
            return Err(ConversionError::InvalidInput(
                "Removing segments requires re-encoding".to_string(),
            ));
        }
        if is_image_container(&config.container) {
            return Err(ConversionError::InvalidInput(
                "Removing segments is not available for image outputs".to_string(),
            ));
        }
    }

    if !is_copy_mode && config.resolution == "custom" {
        let w_str = config.custom_width.as_deref().unwrap_or("-1");
        let h_str = config.custom_height.as_deref().unwrap_or("-1");

        let w = w_str
            .parse::<i32>()
            .map_err(|_| ConversionError::InvalidInput(format!("Invalid custom width: {w_str}")))?;
        let h = h_str.parse::<i32>().map_err(|_| {
            ConversionError::InvalidInput(format!("Invalid custom height: {h_str}"))
        })?;

        if w == 0 || h == 0 {
            return Err(ConversionError::InvalidInput(
                "Resolution dimensions cannot be zero".to_string(),
            ));
        }
        if w < -1 || h < -1 {
            return Err(ConversionError::InvalidInput(
                "Resolution dimensions cannot be negative (except -1 for auto)".to_string(),
            ));
        }
    }

    if !is_copy_mode
        && config.video_bitrate_mode == "bitrate"
        && !is_audio_only_container(&config.container)
        && !is_video_only_container(&config.container)
    {
        let bitrate = config.video_bitrate.parse::<f64>().map_err(|_| {
            ConversionError::InvalidInput(format!(
                "Invalid video bitrate: {}",
                config.video_bitrate
            ))
        })?;
        if bitrate <= 0.0 {
            return Err(ConversionError::InvalidInput(
                "Video bitrate must be positive".to_string(),
            ));
        }
    }

    let is_audio_only = is_audio_only_container(&config.container);
    let is_video_only = is_video_only_container(&config.container);
    let is_image_output = is_image_container(&config.container);
    let supports_audio = container_supports_audio(&config.container);
    if !is_copy_mode
        && !is_audio_only
        && !is_video_codec_allowed(&config.container, &config.video_codec)
    {
        return Err(ConversionError::InvalidInput(format!(
            "Video codec '{}' is not compatible with container '{}'",
            config.video_codec, config.container
        )));
    }

    if !is_copy_mode
        && supports_audio
        && !is_audio_codec_allowed(&config.container, &config.audio_codec)
    {
        return Err(ConversionError::InvalidInput(format!(
            "Audio codec '{}' is not compatible with container '{}'",
            config.audio_codec, config.container
        )));
    }

    if !is_copy_mode && supports_audio {
        let lossless_audio = ["flac", "alac", "pcm_s16le", "pcm_bluray"];
        let is_lossless = lossless_audio.contains(&config.audio_codec.as_str());
        match config.audio_bitrate_mode.as_str() {
            "bitrate" => {
                if !is_lossless {
                    let bitrate = config.audio_bitrate.parse::<f64>().map_err(|_| {
                        ConversionError::InvalidInput(format!(
                            "Invalid audio bitrate: {}",
                            config.audio_bitrate
                        ))
                    })?;
                    if bitrate <= 0.0 {
                        return Err(ConversionError::InvalidInput(
                            "Audio bitrate must be positive".to_string(),
                        ));
                    }
                }
            }
            "vbr" => {
                if is_lossless {
                    return Err(ConversionError::InvalidInput(
                        "VBR is not applicable to lossless audio codecs".to_string(),
                    ));
                }
                if !audio_codec_supports_vbr(&config.audio_codec) {
                    return Err(ConversionError::InvalidInput(format!(
                        "Audio codec '{}' does not support VBR",
                        config.audio_codec
                    )));
                }
                if config.audio_quality.trim().parse::<u8>().is_err() {
                    return Err(ConversionError::InvalidInput(format!(
                        "Invalid audio quality: {}",
                        config.audio_quality
                    )));
                }
            }
            other => {
                return Err(ConversionError::InvalidInput(format!(
                    "Invalid audio bitrate mode: {other}"
                )));
            }
        }
        if config.audio_codec == "mp2"
            && !matches!(
                config.audio_bitrate.as_str(),
                "64" | "96" | "112" | "128" | "160" | "192" | "224" | "256" | "320" | "384"
            )
        {
            return Err(ConversionError::InvalidInput(format!(
                "MP2 bitrate must be one of 64, 96, 112, 128, 160, 192, 224, 256, 320, or 384 kbps: {}",
                config.audio_bitrate
            )));
        }
    }

    if (is_audio_only || is_video_only) && has_custom_pixel_format(config) {
        return Err(ConversionError::InvalidInput(
            "Pixel format override is not available for this container".to_string(),
        ));
    }

    if let Some(overlay) = config
        .overlay
        .as_ref()
        .filter(|overlay| overlay.enabled && !overlay.path.trim().is_empty())
    {
        let overlay_path = Path::new(&overlay.path);
        if !overlay_path.exists() {
            return Err(ConversionError::InvalidInput(format!(
                "Overlay image does not exist: {}",
                overlay.path
            )));
        }

        if is_audio_only {
            return Err(ConversionError::InvalidInput(
                "Overlay is not available for audio-only outputs".to_string(),
            ));
        }

        if config.container.eq_ignore_ascii_case("gif") {
            return Err(ConversionError::InvalidInput(
                "Overlay is not available for GIF output yet".to_string(),
            ));
        }
    }

    if !is_copy_mode
        && has_custom_pixel_format(config)
        && !is_video_pixel_format_allowed(
            &config.container,
            &config.video_codec,
            &config.pixel_format,
        )
    {
        return Err(ConversionError::InvalidInput(format!(
            "Pixel format '{}' is not compatible with container '{}' and encoder '{}'",
            config.pixel_format, config.container, config.video_codec
        )));
    }

    if is_copy_mode {
        if is_video_only || is_image_output {
            return Err(ConversionError::InvalidInput(
                "Stream copy mode is not available for image/video-only containers".to_string(),
            ));
        }

        if has_custom_pixel_format(config) {
            return Err(ConversionError::InvalidInput(
                "Pixel format override requires re-encoding mode".to_string(),
            ));
        }

        if has_overlay(config) {
            return Err(ConversionError::InvalidInput(
                "Overlay requires re-encoding".to_string(),
            ));
        }

        if (config.audio_volume - 100.0).abs() > VOLUME_EPSILON {
            return Err(ConversionError::InvalidInput(
                "Audio volume adjustment requires re-encoding".to_string(),
            ));
        }

        if config.audio_normalize {
            return Err(ConversionError::InvalidInput(
                "Audio normalization requires re-encoding".to_string(),
            ));
        }

        if config.rotation != "0" || config.flip_horizontal || config.flip_vertical {
            return Err(ConversionError::InvalidInput(
                "Video transforms require re-encoding".to_string(),
            ));
        }

        if config.crop.as_ref().is_some_and(|crop| crop.enabled) {
            return Err(ConversionError::InvalidInput(
                "Cropping requires re-encoding".to_string(),
            ));
        }

        if config.resolution != "original" || config.fps != "original" {
            return Err(ConversionError::InvalidInput(
                "Resolution and FPS changes require re-encoding".to_string(),
            ));
        }

        if config.hw_decode {
            return Err(ConversionError::InvalidInput(
                "Hardware decoding is unavailable in stream copy mode".to_string(),
            ));
        }
    }

    if !supports_audio && !config.selected_audio_tracks.is_empty() {
        return Err(ConversionError::InvalidInput(
            "Audio track selection is not available for this container".to_string(),
        ));
    }

    if is_video_only && config.container.eq_ignore_ascii_case("gif") {
        if !(2..=256).contains(&config.gif_colors) {
            return Err(ConversionError::InvalidInput(format!(
                "GIF palette size must be between 2 and 256 colors: {}",
                config.gif_colors
            )));
        }

        if !matches!(
            config.gif_dither.as_str(),
            "none" | "bayer" | "floyd_steinberg" | "sierra2_4a"
        ) {
            return Err(ConversionError::InvalidInput(format!(
                "Invalid GIF dither mode: {}",
                config.gif_dither
            )));
        }
    }

    if is_image_output {
        validate_image_encoding_settings(config)?;
    }

    Ok(())
}

fn validate_image_encoding_settings(config: &ConversionConfig) -> Result<(), ConversionError> {
    match config.video_codec.as_str() {
        "mjpeg" => {
            if !(1..=100).contains(&config.image_jpeg_quality) {
                return Err(ConversionError::InvalidInput(format!(
                    "JPEG quality must be between 1 and 100: {}",
                    config.image_jpeg_quality
                )));
            }
            if !matches!(config.image_jpeg_huffman.as_str(), "default" | "optimal") {
                return Err(ConversionError::InvalidInput(format!(
                    "Invalid JPEG Huffman mode: {}",
                    config.image_jpeg_huffman
                )));
            }
        }
        "libwebp" => {
            if config.image_webp_quality > 100 {
                return Err(ConversionError::InvalidInput(format!(
                    "WebP quality must be between 0 and 100: {}",
                    config.image_webp_quality
                )));
            }
            if config.image_webp_compression > 6 {
                return Err(ConversionError::InvalidInput(format!(
                    "WebP compression effort must be between 0 and 6: {}",
                    config.image_webp_compression
                )));
            }
            if !matches!(
                config.image_webp_preset.as_str(),
                "default" | "picture" | "photo" | "drawing" | "icon" | "text"
            ) {
                return Err(ConversionError::InvalidInput(format!(
                    "Invalid WebP preset: {}",
                    config.image_webp_preset
                )));
            }
        }
        "png" => {
            if config.image_png_compression > 9 {
                return Err(ConversionError::InvalidInput(format!(
                    "PNG compression level must be between 0 and 9: {}",
                    config.image_png_compression
                )));
            }
            if !matches!(
                config.image_png_prediction.as_str(),
                "none" | "sub" | "up" | "avg" | "paeth" | "mixed"
            ) {
                return Err(ConversionError::InvalidInput(format!(
                    "Invalid PNG prediction mode: {}",
                    config.image_png_prediction
                )));
            }
        }
        "tiff"
            if !matches!(
                config.image_tiff_compression.as_str(),
                "packbits" | "raw" | "lzw" | "deflate"
            ) =>
        {
            return Err(ConversionError::InvalidInput(format!(
                "Invalid TIFF compression mode: {}",
                config.image_tiff_compression
            )));
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::EVEN_DIMENSIONS_FILTER;
    use crate::types::SubtitleTrack;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn sample_config(container: &str, video_codec: &str) -> ConversionConfig {
        ConversionConfig {
            processing_mode: "reencode".to_string(),
            container: container.to_string(),
            video_codec: video_codec.to_string(),
            video_bitrate_mode: "crf".to_string(),
            video_bitrate: "5000".to_string(),
            audio_codec: "aac".to_string(),
            audio_bitrate: "128".to_string(),
            audio_bitrate_mode: "bitrate".to_string(),
            audio_quality: "4".to_string(),
            audio_channels: "original".to_string(),
            audio_volume: 100.0,
            audio_normalize: false,
            video_filters: crate::types::VideoFiltersConfig::default(),
            audio_filters: crate::types::AudioFiltersConfig::default(),
            external_audio_tracks: vec![],
            timeline_sources: Vec::new(),
            selected_audio_tracks: vec![],
            resolution: "original".to_string(),
            custom_width: None,
            custom_height: None,
            scaling_algorithm: "bicubic".to_string(),
            fps: "original".to_string(),
            crf: 23,
            quality: 50,
            preset: "medium".to_string(),
            start_time: None,
            end_time: None,
            cut_segments: Vec::new(),
            metadata: MetadataConfig::default(),
            rotation: "0".to_string(),
            flip_horizontal: false,
            flip_vertical: false,
            crop: None,
            overlay: None,
            nvenc_spatial_aq: false,
            nvenc_temporal_aq: false,
            videotoolbox_allow_sw: false,
            hw_decode: false,
            pixel_format: "auto".to_string(),
            image_jpeg_quality: 85,
            image_jpeg_huffman: "optimal".to_string(),
            image_webp_lossless: false,
            image_webp_quality: 75,
            image_webp_compression: 4,
            image_webp_preset: "default".to_string(),
            image_png_compression: 9,
            image_png_prediction: "paeth".to_string(),
            image_tiff_compression: "packbits".to_string(),
            gif_colors: 256,
            gif_dither: "sierra2_4a".to_string(),
            gif_loop: 0,
        }
    }

    fn sample_probe() -> ProbeMetadata {
        ProbeMetadata {
            media_kind: "video".to_string(),
            video_codec: Some("h264".to_string()),
            audio_tracks: vec![AudioTrack {
                index: 1,
                codec: "aac".to_string(),
                channels: "2".to_string(),
                ..AudioTrack::default()
            }],
            ..ProbeMetadata::default()
        }
    }

    #[test]
    fn added_audio_rejects_silent_containers_and_copy_mode() {
        for (container, mode) in [("gif", "reencode"), ("mp4", "copy")] {
            let mut config = sample_config(container, "libx264");
            config.processing_mode = mode.to_string();
            config
                .external_audio_tracks
                .push(crate::types::ExternalAudioTrack {
                    path: "music.wav".to_string(),
                    duration_millis: 1000,
                    ..Default::default()
                });
            assert!(build_ffmpeg_args("source.gif", "out.mp4", &config, &sample_probe()).is_err());
        }
    }

    #[test]
    fn build_ffmpeg_args_adds_even_dimensions_guard_for_default_video_reencode() {
        let config = sample_config("mp4", "libx264");

        let args = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect("arguments should build");

        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        assert_eq!(args[vf_index + 1], EVEN_DIMENSIONS_FILTER);
    }

    #[test]
    fn build_ffmpeg_args_does_not_add_even_dimensions_guard_for_image_output() {
        let config = sample_config("png", "png");

        let args = build_ffmpeg_args("input.mov", "output.png", &config, &sample_probe())
            .expect("arguments should build");

        assert!(!args.iter().any(|arg| arg == EVEN_DIMENSIONS_FILTER));
    }

    #[test]
    fn build_output_path_preserves_periods_in_output_name_on_unc_share() {
        let output = build_output_path(
            r"\\myserver.domain.com\share\movies\Really Funny Home Video Vol.1 (2026)",
            "mp4",
            Some("Really Funny Home Video Vol.1 (2026)"),
        );

        assert_eq!(
            output,
            r"\\myserver.domain.com\share\movies\Really Funny Home Video Vol.1 (2026)\Really Funny Home Video Vol.1 (2026).mp4"
        );
    }

    #[test]
    fn build_output_path_replaces_known_container_extension() {
        let output = build_output_path("/tmp", "mp4", Some("render.mov"));

        assert_eq!(output, "/tmp/render.mp4");
    }

    #[test]
    fn build_output_path_uses_selected_output_directory() {
        let output = build_output_path("/exports", "mp4", Some("render"));

        assert_eq!(output, "/exports/render.mp4");
    }

    #[test]
    fn transport_stream_output_paths_preserve_public_suffixes() {
        for container in ["m2t", "mts", "m2ts"] {
            assert_eq!(
                build_output_path("/exports", container, Some("camera.M2T")),
                format!("/exports/camera.{container}")
            );
        }
    }

    #[test]
    fn transport_stream_profiles_emit_exact_muxer_tail_for_reencode_and_copy() {
        for (container, mode) in [("m2t", "0"), ("mts", "1"), ("m2ts", "1")] {
            for processing_mode in ["reencode", "copy"] {
                let mut config = sample_config(container, "libx264");
                config.processing_mode = processing_mode.to_string();
                config.audio_codec = if container == "m2t" { "mp2" } else { "ac3" }.to_string();
                let mut probe = sample_probe();
                probe.audio_tracks[0].codec =
                    if container == "m2t" { "mp2" } else { "ac3" }.to_string();
                let args =
                    build_ffmpeg_args("input.ts", &format!("output.{container}"), &config, &probe)
                        .unwrap();
                let expected = [
                    "-f".to_string(),
                    "mpegts".to_string(),
                    "-mpegts_m2ts_mode".to_string(),
                    mode.to_string(),
                    "-n".to_string(),
                    format!("output.{container}"),
                ];
                assert_eq!(&args[args.len() - 6..], expected);
            }
        }
    }

    #[test]
    fn stream_copy_maps_only_the_video_selected_during_probe() {
        let mut config = sample_config("m2t", "libx264");
        config.processing_mode = "copy".to_string();
        config.selected_audio_tracks = vec![1];
        let mut probe = sample_probe();
        probe.video_stream_index = Some(4);

        let args = build_ffmpeg_args("input.m2t", "output.m2t", &config, &probe).unwrap();

        assert!(args_contains_pair(&args, "-map", "0:4"));
        assert!(!args.iter().any(|arg| arg == "0:v?"));
    }

    #[test]
    fn hevc_in_mp4_and_mov_is_tagged_hvc1_for_apple_players() {
        for container in ["mp4", "mov"] {
            for codec in ["libx265", "hevc_nvenc", "hevc_videotoolbox"] {
                let config = sample_config(container, codec);
                let args =
                    build_ffmpeg_args("input.mkv", "output", &config, &sample_probe()).unwrap();
                assert!(
                    args_contains_pair(&args, "-tag:v", "hvc1"),
                    "{codec} in {container} should be tagged hvc1"
                );
            }
        }
    }

    #[test]
    fn hvc1_tag_is_limited_to_hevc_in_quicktime_family_containers() {
        for (container, codec) in [("mkv", "libx265"), ("mp4", "libx264"), ("mov", "libsvtav1")] {
            let config = sample_config(container, codec);
            let args = build_ffmpeg_args("input.mkv", "output", &config, &sample_probe()).unwrap();
            assert!(
                !args.iter().any(|arg| arg == "-tag:v"),
                "{codec} in {container} should keep the muxer default tag"
            );
        }
    }

    #[test]
    fn stream_copy_tags_hevc_sources_hvc1_in_mp4() {
        let mut config = sample_config("mp4", "libx264");
        config.processing_mode = "copy".to_string();
        let mut probe = sample_probe();
        probe.video_codec = Some("hevc".to_string());

        let args = build_ffmpeg_args("input.mkv", "output.mp4", &config, &probe).unwrap();
        assert!(args_contains_pair(&args, "-tag:v", "hvc1"));

        probe.video_codec = Some("h264".to_string());
        let args = build_ffmpeg_args("input.mkv", "output.mp4", &config, &probe).unwrap();
        assert!(!args.iter().any(|arg| arg == "-tag:v"));
    }

    #[test]
    fn mpeg2video_uses_bitrate_without_generic_crf_or_preset() {
        let mut config = sample_config("m2t", "mpeg2video");
        config.video_bitrate_mode = "bitrate".to_string();
        config.video_bitrate = "18000".to_string();
        config.audio_codec = "mp2".to_string();

        let args = build_ffmpeg_args("input.ts", "output.m2t", &config, &sample_probe()).unwrap();

        assert!(args_contains_pair(&args, "-b:v", "18000k"));
        assert!(!args.iter().any(|arg| arg == "-crf"));
        assert!(!args.iter().any(|arg| arg == "-preset"));
    }

    #[test]
    fn mpegts_metadata_preserve_prefers_service_tags_and_manual_overrides() {
        let mut config = sample_config("m2t", "mpeg2video");
        config.audio_codec = "mp2".to_string();
        config.metadata.service_name = Some("Manual".to_string());
        let mut probe = sample_probe();
        probe.transport_stream = Some(crate::types::TransportStreamMetadata {
            service_name: Some("Source".to_string()),
            service_provider: Some("Provider".to_string()),
            ..crate::types::TransportStreamMetadata::default()
        });

        let args = build_ffmpeg_args("input.m2t", "output.m2t", &config, &probe).unwrap();

        assert!(args_contains_pair(
            &args,
            "-metadata",
            "service_name=Manual"
        ));
        assert!(args_contains_pair(
            &args,
            "-metadata",
            "service_provider=Provider"
        ));
    }

    #[test]
    fn build_ffmpeg_args_disables_output_overwrite_for_reencode() {
        let config = sample_config("mp4", "libx264");

        let args = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect("re-encode arguments should build");

        assert_eq!(
            (
                args.iter().any(|arg| arg == "-n"),
                args.iter().any(|arg| arg == "-y")
            ),
            (true, false)
        );
    }

    #[test]
    fn build_ffmpeg_args_disables_output_overwrite_for_stream_copy() {
        let mut config = sample_config("mp4", "libx264");
        config.processing_mode = "copy".to_string();

        let args = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect("stream-copy arguments should build");

        assert_eq!(
            (
                args.iter().any(|arg| arg == "-n"),
                args.iter().any(|arg| arg == "-y")
            ),
            (true, false)
        );
    }

    #[test]
    fn build_ffmpeg_args_adds_png_compression_options() {
        let mut config = sample_config("png", "png");
        config.image_png_compression = 3;
        config.image_png_prediction = "mixed".to_string();

        let args = build_ffmpeg_args("input.mov", "output.png", &config, &sample_probe())
            .expect("arguments should build");

        assert!(args_contains_pair(&args, "-compression_level", "3"));
        assert!(args_contains_pair(&args, "-pred", "mixed"));
    }

    #[test]
    fn build_ffmpeg_args_adds_jpeg_quality_and_huffman_options() {
        let mut config = sample_config("jpg", "mjpeg");
        config.image_jpeg_quality = 100;
        config.image_jpeg_huffman = "default".to_string();

        let args = build_ffmpeg_args("input.mov", "output.jpg", &config, &sample_probe())
            .expect("arguments should build");

        assert!(args_contains_pair(&args, "-q:v", "2"));
        assert!(args_contains_pair(&args, "-huffman", "default"));
    }

    #[test]
    fn build_ffmpeg_args_adds_webp_quality_and_compression_options() {
        let mut config = sample_config("webp", "libwebp");
        config.image_webp_lossless = true;
        config.image_webp_quality = 88;
        config.image_webp_compression = 6;
        config.image_webp_preset = "photo".to_string();

        let args = build_ffmpeg_args("input.mov", "output.webp", &config, &sample_probe())
            .expect("arguments should build");

        assert!(args_contains_pair(&args, "-lossless", "1"));
        assert!(args_contains_pair(&args, "-quality", "88"));
        assert!(args_contains_pair(&args, "-compression_level", "6"));
        assert!(args_contains_pair(&args, "-preset", "photo"));
    }

    #[test]
    fn build_ffmpeg_args_adds_tiff_compression_option() {
        let mut config = sample_config("tiff", "tiff");
        config.image_tiff_compression = "deflate".to_string();

        let args = build_ffmpeg_args("input.mov", "output.tiff", &config, &sample_probe())
            .expect("arguments should build");

        assert!(args_contains_pair(&args, "-compression_algo", "deflate"));
    }

    #[test]
    fn build_ffmpeg_args_maps_only_audio_tracks_returned_by_probe() {
        let mut config = sample_config("mp4", "libx264");
        config.selected_audio_tracks = vec![1];
        let probe = sample_probe();

        let args = build_ffmpeg_args("spatial.mov", "output.mp4", &config, &probe)
            .expect("recognized AAC track should be mapped");

        assert!(args_contains_pair(&args, "-map", "0:1"));
        assert!(!args.iter().any(|arg| arg == "0:a?"));
        assert!(!args.iter().any(|arg| arg == "0:2"));
        assert!(args.iter().any(|arg| arg == "-dn"));
    }

    #[test]
    fn legacy_subtitle_settings_do_not_affect_exports() {
        let mut probe = sample_probe();
        probe.subtitle_tracks = vec![SubtitleTrack {
            index: 2,
            codec: "hdmv_pgs_subtitle".to_string(),
            ..SubtitleTrack::default()
        }];

        for mode in ["reencode", "copy"] {
            let mut saved = serde_json::to_value(sample_config("mp4", "libx264")).unwrap();
            saved["processingMode"] = serde_json::json!(mode);
            saved["selectedSubtitleTracks"] = serde_json::json!([2]);
            saved["externalSubtitleTracks"] = serde_json::json!([{
                "path": "/missing/captions.srt",
                "isDefault": true
            }]);
            saved["subtitleBurnPath"] = serde_json::json!("/missing/burn.ass");
            saved["subtitleFontName"] = serde_json::json!("Arial");
            let config: ConversionConfig = serde_json::from_value(saved).unwrap();
            let args = build_ffmpeg_args("pgs.mkv", "output.mp4", &config, &probe)
                .expect("legacy subtitle settings should be ignored");
            assert!(args.iter().any(|arg| arg == "-sn"));
            assert!(!args_contains_pair(&args, "-map", "0:2"));
            assert_eq!(args.iter().filter(|arg| *arg == "-i").count(), 1);
            assert!(!args.iter().any(|arg| arg.contains("subtitles=")));
            let saved = serde_json::to_value(config).unwrap();
            assert!(
                saved
                    .as_object()
                    .unwrap()
                    .keys()
                    .all(|key| { !key.to_ascii_lowercase().contains("subtitle") })
            );
        }
    }

    #[test]
    fn build_ffmpeg_args_omits_audio_when_no_track_is_selected() {
        let mut config = sample_config("mp4", "libx264");
        config.selected_audio_tracks.clear();

        let args = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect("video-only MP4 should build");

        assert!(args.iter().any(|arg| arg == "-an"));
        assert!(!args_contains_pair(&args, "-map", "0:1"));
    }

    #[test]
    fn build_ffmpeg_args_rejects_audio_only_output_without_selected_track() {
        let mut config = sample_config("mp3", "libx264");
        config.selected_audio_tracks.clear();
        config.audio_codec = "mp3".to_string();

        let error = build_ffmpeg_args("input.mov", "output.mp3", &config, &sample_probe())
            .expect_err("audio-only output requires an explicit audio selection");

        assert!(
            error
                .to_string()
                .contains("Select at least one audio track")
        );
    }

    #[test]
    fn validate_task_input_rejects_invalid_webp_compression_level() {
        let path = temporary_input_file("invalid-webp-compression");
        let mut config = sample_config("webp", "libwebp");
        config.image_webp_compression = 7;

        let error = validate_task_input(&path.to_string_lossy(), &config)
            .expect_err("invalid webp compression should be rejected");

        let _ = fs::remove_file(path);
        assert!(error.to_string().contains("WebP compression effort"));
    }

    fn cut(start: &str, end: &str) -> crate::types::CutSegment {
        crate::types::CutSegment {
            start_time: start.to_string(),
            end_time: end.to_string(),
        }
    }

    #[test]
    fn build_ffmpeg_args_removes_cut_segments_from_video_and_audio() {
        let mut config = sample_config("mp4", "libx264");
        config.selected_audio_tracks = vec![1];
        config.cut_segments = vec![cut("00:00:02.000", "00:00:04.000")];

        let args = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect("cut arguments should build");

        let video_filters = args
            .windows(2)
            .find(|window| window[0] == "-vf")
            .map(|window| window[1].clone())
            .expect("video filters should be present");
        assert!(video_filters.ends_with(
            "select='not(gte(t,2.000)*lt(t,4.000))',setpts='PTS-(clip(T-2.000,0,2.000))/TB'"
        ));
        assert!(args_contains_pair(
            &args,
            "-af",
            "aselect='not(gte(t,2.000)*lt(t,4.000))',asetpts=N/SR/TB"
        ));
    }

    #[test]
    fn build_ffmpeg_args_shortens_trim_duration_by_removed_time() {
        let mut config = sample_config("mp4", "libx264");
        config.start_time = Some("00:00:10.000".to_string());
        config.end_time = Some("00:00:30.000".to_string());
        config.cut_segments = vec![
            cut("00:00:05.000", "00:00:12.000"),
            cut("00:00:20.000", "00:00:25.000"),
        ];

        let args = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect("trimmed cut arguments should build");

        assert!(args_contains_pair(&args, "-ss", "00:00:10.000"));
        assert!(args_contains_pair(&args, "-t", "13.000"));
        assert!(args.iter().any(|arg| {
            arg.contains("select='not(gte(t,0.000)*lt(t,2.000)+gte(t,10.000)*lt(t,15.000))'")
        }));
    }

    #[test]
    fn build_ffmpeg_args_applies_cuts_after_overlay_composition() {
        let mut config = sample_config("mp4", "libx264");
        config.overlay = Some(crate::types::OverlayConfig {
            enabled: true,
            path: "logo.png".to_string(),
            ..crate::types::OverlayConfig::default()
        });
        config.cut_segments = vec![cut("00:00:01.000", "00:00:02.000")];

        let args = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect("overlay cut arguments should build");
        let graph = args
            .windows(2)
            .find(|window| window[0] == "-filter_complex")
            .map(|window| window[1].clone())
            .expect("overlay graph should be present");

        assert!(graph.contains("[overlay_out];[overlay_out]select="));
        assert!(graph.ends_with("/TB'[vout]"));
    }

    #[test]
    fn build_ffmpeg_args_removes_cut_segments_from_audio_only_output() {
        let mut config = sample_config("mp3", "libx264");
        config.audio_codec = "mp3".to_string();
        config.selected_audio_tracks = vec![1];
        config.cut_segments = vec![cut("00:00:01.500", "00:00:03.000")];

        let args = build_ffmpeg_args("input.mov", "output.mp3", &config, &sample_probe())
            .expect("audio cut arguments should build");

        assert!(!args.iter().any(|arg| arg == "-vf"));
        assert!(args_contains_pair(
            &args,
            "-af",
            "aselect='not(gte(t,1.500)*lt(t,3.000))',asetpts=N/SR/TB"
        ));
    }

    #[test]
    fn build_ffmpeg_args_rejects_cuts_in_stream_copy_mode() {
        let mut config = sample_config("mp4", "libx264");
        config.processing_mode = "copy".to_string();
        config.cut_segments = vec![cut("00:00:01.000", "00:00:02.000")];

        let error = build_ffmpeg_args("input.mov", "output.mp4", &config, &sample_probe())
            .expect_err("stream copy cannot drop arbitrary frames");

        assert!(error.to_string().contains("requires re-encoding"));
    }

    fn args_contains_pair(args: &[String], key: &str, value: &str) -> bool {
        args.windows(2)
            .any(|window| window[0] == key && window[1] == value)
    }

    fn temporary_input_file(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "frame-core-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ));
        fs::write(&path, b"").expect("temporary input should be written");
        path
    }
}
