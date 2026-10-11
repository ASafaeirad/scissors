//! Pure state for the export dialog: options, labels, output geometry and the size estimate.
//! Rendering lives in `mod.rs`.

use super::super::settings_panel::is_lossless_audio_codec;
use crate::file_queue::{FileItem, FileStatus};
use crate::material::Symbol;
use crate::numeric::{
    rounded_f64_to_u32, rounded_f64_to_u64, u32_to_f32, u64_to_f64, usize_to_f32,
};
use crate::preview::parse_time_to_seconds;
use crate::settings::{
    AudioQualityRange, ConversionConfig, DEFAULT_AUDIO_BITRATE, DEFAULT_AUDIO_BITRATE_MODE,
    DEFAULT_AUDIO_CHANNELS, DEFAULT_AUDIO_QUALITY, DEFAULT_CRF, DEFAULT_FPS, DEFAULT_PIXEL_FORMAT,
    DEFAULT_PRESET, DEFAULT_QUALITY, DEFAULT_RESOLUTION, DEFAULT_SCALING_ALGORITHM,
    DEFAULT_VIDEO_BITRATE, DEFAULT_VIDEO_BITRATE_MODE, ProcessingMode, SourceMetadata,
    audio_codec_supports_vbr, audio_quality_range, container_supports_audio,
    default_audio_codec_for_container, first_allowed_video_codec, is_gif_container,
    is_hardware_video_codec, is_video_preset_allowed, mp2_original_channels_are_unsupported,
};

/// One of the format cards at the top of the dialog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExportFormat {
    pub(super) container: &'static str,
    pub(super) label: &'static str,
    pub(super) symbol: Symbol,
    pub(super) description: &'static str,
}

pub(super) const EXPORT_FORMATS: [ExportFormat; 4] = [
    ExportFormat {
        container: "mp4",
        label: "MP4",
        symbol: Symbol::Movie,
        description: "Share anywhere",
    },
    ExportFormat {
        container: "webm",
        label: "WebM",
        symbol: Symbol::Public,
        description: "Web and browsers",
    },
    ExportFormat {
        container: "mov",
        label: "MOV",
        symbol: Symbol::VideoFile,
        description: "Keep editing",
    },
    ExportFormat {
        container: "gif",
        label: "GIF",
        symbol: Symbol::GifBox,
        description: "Short loops",
    },
];

pub(super) const RESOLUTION_OPTIONS: [(&str, &str); 5] = [
    ("original", "Original"),
    ("1080p", "1080p"),
    ("720p", "720p"),
    ("480p", "480p"),
    ("custom", "Custom"),
];
/// Frame rates offered in the dialog. GIFs get lower rates; both lists are subsets of the
/// rates `apply_fps` accepts for the container.
pub(super) const VIDEO_FPS_OPTIONS: [&str; 4] = ["original", "24", "30", "60"];
pub(super) const GIF_FPS_OPTIONS: [&str; 5] = ["original", "10", "15", "24", "30"];
pub(super) const SCALING_OPTIONS: [(&str, &str); 4] = [
    ("bicubic", "Bicubic"),
    ("lanczos", "Lanczos"),
    ("bilinear", "Bilinear"),
    ("nearest", "Nearest"),
];
pub(super) const GIF_COLOR_OPTIONS: [u16; 4] = [32, 64, 128, 256];
pub(super) const GIF_DITHER_OPTIONS: [(&str, &str); 4] = [
    ("sierra2_4a", "Sierra"),
    ("floyd_steinberg", "Floyd-Steinberg"),
    ("bayer", "Bayer"),
    ("none", "None"),
];
pub(super) const SPEED_PRESETS: [(&str, &str); 9] = [
    ("ultrafast", "Ultrafast"),
    ("superfast", "Superfast"),
    ("veryfast", "Very fast"),
    ("faster", "Faster"),
    ("fast", "Fast"),
    ("medium", "Medium"),
    ("slow", "Slow"),
    ("slower", "Slower"),
    ("veryslow", "Very slow"),
];
/// Custom dimensions must be even (4:2:0 chroma) and at least this large.
const MIN_CUSTOM_DIMENSION: u32 = 16;
pub(super) const MIN_BITRATE_MBPS: u32 = 1;
pub(super) const MAX_BITRATE_MBPS: u32 = 50;

pub(super) const fn fps_options(container: &str) -> &'static [&'static str] {
    if is_gif_container(container) {
        &GIF_FPS_OPTIONS
    } else {
        &VIDEO_FPS_OPTIONS
    }
}

pub(super) fn fps_option_label(fps: &str, source_fps: Option<f64>) -> String {
    if fps == "original" {
        source_fps.map_or_else(
            || "Source".to_string(),
            |rate| format!("Source ({})", format_rate(rate)),
        )
    } else {
        format!("{fps} fps")
    }
}

/// `30`, `29.97`, `23.976`: whole rates without decimals, others to at most three places.
pub(super) fn format_rate(rate: f64) -> String {
    if (rate - rate.round()).abs() < 0.005 {
        format!("{rate:.0}")
    } else {
        let text = format!("{rate:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub(super) fn output_fps(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> Option<f64> {
    if config.fps == "original" {
        metadata.and_then(|metadata| metadata.frame_rate)
    } else {
        config.fps.parse().ok()
    }
}

/// Note next to the frame rate heading and whether it should draw attention.
pub(super) fn fps_note(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> (&'static str, bool) {
    let source = metadata.and_then(|metadata| metadata.frame_rate);
    let (Some(source), Some(output)) = (source, output_fps(config, metadata)) else {
        return ("", false);
    };
    if output > source + 0.01 {
        ("Higher than source. Frames will be duplicated.", true)
    } else if output < source - 0.01 {
        ("Lower than source. Motion may look less smooth.", false)
    } else {
        ("Matches source", false)
    }
}

/// Source frame size after rotation and crop, the frame the resolution setting scales.
pub(super) fn source_dimensions(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> Option<(u32, u32)> {
    let metadata = metadata?;
    let (mut width, mut height) = (metadata.width?, metadata.height?);
    if width == 0 || height == 0 {
        return None;
    }
    if matches!(config.rotation.as_str(), "90" | "270") {
        std::mem::swap(&mut width, &mut height);
    }
    if let Some(crop) = config.crop.as_ref().filter(|crop| crop.enabled) {
        width = crop.width.max(1);
        height = crop.height.max(1);
    }
    Some((width, height))
}

pub(super) fn parse_dimension(value: Option<&str>) -> Option<u32> {
    value
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
}

/// Scales `other` so `changed / other` keeps the source aspect, rounded to an even number the way
/// a `-2` scale argument does.
pub(super) fn scaled_even(changed: u32, from: u32, other: u32) -> u32 {
    if from == 0 {
        return other;
    }
    let scaled = f64::from(other) * f64::from(changed) / f64::from(from);
    (rounded_f64_to_u32(scaled / 2.0) * 2).max(2)
}

/// The exported frame size, if it is known.
pub(super) fn output_dimensions(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> Option<(u32, u32)> {
    let source = source_dimensions(config, metadata);
    match config.resolution.as_str() {
        "custom" => {
            let width = parse_dimension(config.custom_width.as_deref());
            let height = parse_dimension(config.custom_height.as_deref());
            match (width, height, source) {
                (Some(width), Some(height), _) => Some((width, height)),
                (Some(width), None, Some((sw, sh))) => Some((width, scaled_even(width, sw, sh))),
                (None, Some(height), Some((sw, sh))) => Some((scaled_even(height, sh, sw), height)),
                (None, None, source) => source,
                _ => None,
            }
        }
        "1080p" | "720p" | "480p" => {
            let height = config.resolution.trim_end_matches('p').parse().ok()?;
            let (sw, sh) = source?;
            Some((scaled_even(height, sh, sw), height))
        }
        _ => source,
    }
}

/// Why the custom size can't be exported, if it can't.
pub(super) fn custom_dimensions_error(config: &ConversionConfig) -> Option<&'static str> {
    if config.resolution != "custom" {
        return None;
    }
    let width = parse_dimension(config.custom_width.as_deref());
    let height = parse_dimension(config.custom_height.as_deref());
    if width.is_none() && height.is_none() {
        return Some("Enter a width or height");
    }
    let invalid = |value: Option<u32>| {
        value.is_some_and(|value| value < MIN_CUSTOM_DIMENSION || value % 2 != 0)
    };
    (invalid(width) || invalid(height)).then_some("Use even numbers, 16 px or more")
}

pub(super) fn format_dimensions(dimensions: Option<(u32, u32)>) -> String {
    dimensions.map_or_else(
        || "Size unknown".to_string(),
        |(width, height)| format!("{width} × {height}"),
    )
}

pub(super) fn resolution_note(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> String {
    if let Some(error) = custom_dimensions_error(config) {
        return error.to_string();
    }
    let dimensions = output_dimensions(config, metadata);
    let text = format_dimensions(dimensions);
    if dimensions.is_some() && dimensions == source_dimensions(config, metadata) {
        format!("{text} · same as source")
    } else {
        text
    }
}

pub(super) fn scaling_note(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> &'static str {
    let resizing = config.resolution != "original"
        && output_dimensions(config, metadata) != source_dimensions(config, metadata);
    if !resizing {
        return "Only used when resizing";
    }
    match config.scaling_algorithm.as_str() {
        "lanczos" => "Sharpest, slower",
        "bilinear" => "Softer, fast",
        "nearest" => "Pixel art, no smoothing",
        _ => "Sharp, a good default",
    }
}

/// What the quality slider edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QualityTarget {
    /// Software encoders: constant rate factor, 0 (lossless) to 51.
    Crf,
    /// Hardware encoders: encoder quality, 1 to 100.
    Quality,
    /// Target bitrate in Mb/s.
    Bitrate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct QualityControl {
    pub(super) target: QualityTarget,
    pub(super) value: u32,
    pub(super) min: u32,
    pub(super) max: u32,
    pub(super) badge: String,
    pub(super) hint: &'static str,
    pub(super) lower: &'static str,
    pub(super) upper: &'static str,
    /// MPEG-2 only encodes at a target bitrate, so it hides the mode choice.
    pub(super) mode_selectable: bool,
}

pub(super) fn bitrate_mbps(config: &ConversionConfig) -> u32 {
    let kbps = config
        .video_bitrate
        .parse::<u32>()
        .or_else(|_| DEFAULT_VIDEO_BITRATE.parse())
        .unwrap_or(5_000);
    ((kbps + 500) / 1_000).clamp(MIN_BITRATE_MBPS, MAX_BITRATE_MBPS)
}

pub(super) const fn crf_word(crf: u8) -> &'static str {
    match crf {
        0..=17 => "Visually lossless",
        18..=22 => "High",
        23..=27 => "Balanced",
        28..=33 => "Small file",
        _ => "Low",
    }
}

const CONSTANT_QUALITY_HINT: &str =
    "Keeps quality steady; file size varies with motion. 18–28 suits most videos.";

pub(super) fn quality_control(config: &ConversionConfig) -> QualityControl {
    let mode_selectable = config.video_codec != "mpeg2video";
    if config.video_bitrate_mode == "bitrate" || !mode_selectable {
        let value = bitrate_mbps(config);
        return QualityControl {
            target: QualityTarget::Bitrate,
            value,
            min: MIN_BITRATE_MBPS,
            max: MAX_BITRATE_MBPS,
            badge: format!("{value} Mb/s"),
            hint: "Keeps file size predictable; quality varies with motion.",
            lower: "1 Mb/s",
            upper: "50 Mb/s",
            mode_selectable,
        };
    }
    if is_hardware_video_codec(&config.video_codec) {
        return QualityControl {
            target: QualityTarget::Quality,
            value: config.quality,
            min: 1,
            max: 100,
            badge: format!("Quality {}", config.quality),
            hint: CONSTANT_QUALITY_HINT,
            lower: "Smallest",
            upper: "Best",
            mode_selectable,
        };
    }
    QualityControl {
        target: QualityTarget::Crf,
        value: u32::from(config.crf),
        min: 0,
        max: 51,
        badge: format!("{} · CRF {}", crf_word(config.crf), config.crf),
        hint: CONSTANT_QUALITY_HINT,
        lower: "Lossless",
        upper: "Smallest",
        mode_selectable,
    }
}

/// Encoding speed presets the codec accepts, slowest last. Empty when it has no presets.
pub(super) fn speed_presets(codec: &str) -> Vec<(&'static str, &'static str)> {
    if codec == "mpeg2video"
        || codec == "gif"
        || codec == "prores"
        || crate::settings::is_videotoolbox_video_codec(codec)
    {
        return Vec::new();
    }
    SPEED_PRESETS
        .into_iter()
        .filter(|(preset, _)| is_video_preset_allowed(codec, preset))
        .collect()
}

pub(super) fn encoder_description(codec: &str) -> &'static str {
    match codec {
        "libx264" => "Plays everywhere",
        "libx265" => "About 40% smaller, newer devices",
        "libsvtav1" => "Smallest, slow to encode",
        "vp9" => "Best browser support",
        "prores" => "For further editing, very large",
        "mpeg2video" => "Broadcast and DVD players",
        "gif" => "256 colors, no sound",
        "h264_videotoolbox" | "hevc_videotoolbox" => "Hardware, Apple silicon",
        "h264_nvenc" | "hevc_nvenc" | "av1_nvenc" => "Hardware, NVIDIA",
        _ => "",
    }
}

pub(super) fn pixel_format_text(id: &str) -> (&'static str, &'static str) {
    match id {
        "yuv420p" => ("4:2:0 · 8-bit", "Most compatible"),
        "yuv422p" => ("4:2:2 · 8-bit", "Sharper color edges"),
        "yuv444p" => ("4:4:4 · 8-bit", "Full color detail"),
        "yuv420p10le" => ("4:2:0 · 10-bit", "Smoother gradients, HDR"),
        "yuv422p10le" => ("4:2:2 · 10-bit", "Editing and broadcast"),
        "yuv444p10le" => ("4:4:4 · 10-bit", "Mastering"),
        _ => ("Automatic", "Recommended"),
    }
}

/// One line summing up the advanced section while it is collapsed.
pub(super) fn advanced_summary(config: &ConversionConfig, encoder_label: &str) -> String {
    let mut parts = vec![encoder_label.to_string()];
    if is_gif_container(&config.container) {
        parts.push(format!("{} colors", config.gif_colors));
    } else {
        parts.push(if config.pixel_format == "auto" {
            "auto pixel format".to_string()
        } else {
            pixel_format_text(&config.pixel_format).0.to_string()
        });
    }
    if let Some((_, label)) = speed_presets(&config.video_codec)
        .into_iter()
        .find(|(preset, _)| *preset == config.preset)
    {
        parts.push(format!("{} preset", label.to_lowercase()));
    }
    parts.join(" · ")
}

/// Whether the video and audio encoding settings the dialog edits are at their recommended values.
pub(super) fn is_recommended(config: &ConversionConfig) -> bool {
    config.container == "mp4"
        && config.video_codec == first_allowed_video_codec("mp4", None)
        && config.resolution == DEFAULT_RESOLUTION
        && config.fps == DEFAULT_FPS
        && config.video_bitrate_mode == DEFAULT_VIDEO_BITRATE_MODE
        && config.crf == DEFAULT_CRF
        && config.quality == DEFAULT_QUALITY
        && config.preset == DEFAULT_PRESET
        && config.pixel_format == DEFAULT_PIXEL_FORMAT
        && config.scaling_algorithm == DEFAULT_SCALING_ALGORITHM
        && config.audio_codec == default_audio_codec_for_container("mp4")
        && config.audio_channels == DEFAULT_AUDIO_CHANNELS
        && config.audio_bitrate_mode == DEFAULT_AUDIO_BITRATE_MODE
        && config.audio_bitrate == DEFAULT_AUDIO_BITRATE
}

/// Puts the video and audio encoding settings back to their recommended values. Returns whether anything changed.
pub(super) fn reset_to_recommended(config: &mut ConversionConfig) -> bool {
    let before = config.clone();
    crate::settings::apply_output_container(config, "mp4");
    config.video_codec = first_allowed_video_codec("mp4", None);
    config.resolution = DEFAULT_RESOLUTION.to_string();
    config.custom_width = None;
    config.custom_height = None;
    config.fps = DEFAULT_FPS.to_string();
    config.video_bitrate_mode = DEFAULT_VIDEO_BITRATE_MODE.to_string();
    config.video_bitrate = DEFAULT_VIDEO_BITRATE.to_string();
    config.crf = DEFAULT_CRF;
    config.quality = DEFAULT_QUALITY;
    config.preset = DEFAULT_PRESET.to_string();
    config.pixel_format = DEFAULT_PIXEL_FORMAT.to_string();
    config.scaling_algorithm = DEFAULT_SCALING_ALGORITHM.to_string();
    config.audio_codec = default_audio_codec_for_container("mp4").to_string();
    config.audio_channels = DEFAULT_AUDIO_CHANNELS.to_string();
    config.audio_bitrate_mode = DEFAULT_AUDIO_BITRATE_MODE.to_string();
    config.audio_bitrate = DEFAULT_AUDIO_BITRATE.to_string();
    config.audio_quality = DEFAULT_AUDIO_QUALITY.to_string();
    crate::settings::normalize_video_config(config, None);
    before != *config
}

/// Switches the output format. Moving to GIF from full-size, source-rate video picks a smaller
/// size and rate, since full-size GIFs are huge. Frame rates the new format doesn't offer fall
/// back to the source rate.
pub(super) fn apply_export_format(config: &mut ConversionConfig, container: &str) -> bool {
    let before = config.clone();
    let to_gif = is_gif_container(container) && !is_gif_container(&config.container);
    crate::settings::apply_output_container(config, container);
    if to_gif {
        if config.resolution == "original" {
            config.resolution = "480p".to_string();
        }
        if config.fps == "original" {
            config.fps = "15".to_string();
        }
    }
    if !fps_options(&config.container).contains(&config.fps.as_str()) {
        config.fps = "original".to_string();
    }
    before != *config
}

/// Length of the exported media in seconds: the trimmed range minus any cuts.
pub(super) fn output_duration_seconds(config: &ConversionConfig, source_duration: f64) -> f64 {
    let bound = |value: Option<&String>| {
        value
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(parse_time_to_seconds)
    };
    let start = bound(config.start_time.as_ref())
        .unwrap_or(0.0)
        .clamp(0.0, source_duration);
    let end = bound(config.end_time.as_ref())
        .unwrap_or(source_duration)
        .clamp(start, source_duration);
    let removed: f64 = config
        .cut_segments
        .iter()
        .map(|cut| {
            let cut_start = parse_time_to_seconds(&cut.start_time).max(start);
            let cut_end = parse_time_to_seconds(&cut.end_time).min(end);
            (cut_end - cut_start).max(0.0)
        })
        .sum();
    (end - start - removed).max(0.0)
}

/// Whether the export carries an audio stream.
pub(super) fn exports_audio(config: &ConversionConfig, metadata: Option<&SourceMetadata>) -> bool {
    container_supports_audio(&config.container)
        && metadata.is_none_or(|metadata| {
            (!metadata.audio_tracks.is_empty() && !config.selected_audio_tracks.is_empty())
                || config
                    .external_audio_tracks
                    .iter()
                    .any(|track| !track.muted)
        })
}

pub(super) const AUDIO_CHANNEL_OPTIONS: [(&str, &str); 3] = [
    ("original", "Original"),
    ("stereo", "Stereo"),
    ("mono", "Mono"),
];
const MIN_AUDIO_BITRATE_KBPS: u32 = 32;
/// The only rates the MP2 encoder accepts, in kb/s.
const MP2_BITRATES: [&str; 10] = [
    "64", "96", "112", "128", "160", "192", "224", "256", "320", "384",
];
const VBR_LEVEL_WORDS: [&str; 5] = ["Low", "Medium", "Good", "High", "Very high"];

/// Whether the export re-encodes audio, so the audio section's settings apply.
pub(super) fn encodes_audio(config: &ConversionConfig) -> bool {
    config.processing_mode != ProcessingMode::Copy && container_supports_audio(&config.container)
}

pub(super) fn audio_codec_label(codec: &str) -> &'static str {
    match codec {
        "aac" => "AAC",
        "libfdk_aac" => "AAC (Fraunhofer FDK)",
        "ac3" => "Dolby Digital (AC-3)",
        "libopus" => "Opus",
        "mp3" => "MP3",
        "alac" => "Apple Lossless",
        "flac" => "FLAC",
        "pcm_s16le" => "PCM 16-bit",
        "pcm_bluray" => "Blu-ray PCM",
        "mp2" => "MPEG Layer II",
        _ => "Other codec",
    }
}

pub(super) fn audio_codec_description(codec: &str) -> &'static str {
    match codec {
        "aac" => "Plays everywhere",
        "libfdk_aac" => "Higher-quality AAC",
        "ac3" => "Surround, TVs and discs",
        "libopus" => "Best quality per bit",
        "mp3" => "Legacy players",
        "mp2" => "Broadcast",
        codec if is_lossless_audio_codec(codec) => "Lossless",
        _ => "",
    }
}

/// The most channels among the source audio tracks being exported.
fn source_audio_channels(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> Option<u16> {
    metadata?
        .audio_tracks
        .iter()
        .filter(|track| config.selected_audio_tracks.contains(&track.index))
        .filter_map(|track| track.channels.as_deref()?.parse::<u16>().ok())
        .max()
}

pub(super) fn audio_channels_note(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> String {
    match config.audio_channels.as_str() {
        "mono" => "Mixes down to 1 channel".to_string(),
        "stereo" => "2 channels".to_string(),
        _ if mp2_original_channels_are_unsupported(config, metadata) => {
            "MP2 keeps at most 2 channels".to_string()
        }
        _ => match source_audio_channels(config, metadata) {
            Some(1) => "Keeps 1 channel from source".to_string(),
            Some(channels) => format!("Keeps {channels} channels from source"),
            None => "Keeps the source channels".to_string(),
        },
    }
}

const fn max_audio_bitrate(codec: &str) -> u32 {
    match codec.as_bytes() {
        b"libopus" => 512,
        b"mp3" => 320,
        b"mp2" => 384,
        _ => 640,
    }
}

/// Whether the audio is encoded at a target bitrate, the only mode some codecs offer.
pub(super) fn uses_audio_bitrate(config: &ConversionConfig) -> bool {
    let vbr = config.audio_bitrate_mode == "vbr" && audio_codec_supports_vbr(&config.audio_codec);
    !(is_lossless_audio_codec(&config.audio_codec) || vbr)
}

/// Why the target audio bitrate can't be exported, if it can't.
pub(super) fn audio_bitrate_error(config: &ConversionConfig) -> Option<String> {
    if !encodes_audio(config) || !uses_audio_bitrate(config) {
        return None;
    }
    if config.audio_codec == "mp2" {
        return (!MP2_BITRATES.contains(&config.audio_bitrate.as_str()))
            .then(|| "Use a standard MP2 rate, like 192 kb/s".to_string());
    }
    let max = max_audio_bitrate(&config.audio_codec);
    let in_range = config
        .audio_bitrate
        .parse::<u32>()
        .is_ok_and(|kbps| (MIN_AUDIO_BITRATE_KBPS..=max).contains(&kbps));
    (!in_range).then(|| format!("Enter {MIN_AUDIO_BITRATE_KBPS}–{max} kb/s"))
}

/// Typical stereo bitrate of a VBR quality level, in kb/s.
fn vbr_kbps(codec: &str, quality: u32) -> Option<u32> {
    let table: &[u32] = match codec {
        // LAME -V0 (best) to -V9.
        "mp3" => &[245, 225, 190, 175, 165, 130, 115, 100, 85, 65],
        // FDK VBR modes 1 to 5.
        "libfdk_aac" => &[0, 56, 72, 112, 144, 224],
        _ => return None,
    };
    table.get(quality as usize).copied()
}

/// The VBR quality slider. Its position always runs from smaller file to higher quality, so
/// codecs where a lower number is better are flipped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AudioVbrControl {
    pub(super) position: u32,
    pub(super) min: u32,
    pub(super) max: u32,
    pub(super) badge: String,
}

/// The codec's VBR quality range and the configured level, clamped into it.
fn audio_vbr_quality(config: &ConversionConfig) -> Option<(AudioQualityRange, u32)> {
    let range = audio_quality_range(&config.audio_codec)?;
    let quality = config
        .audio_quality
        .trim()
        .parse::<u32>()
        .unwrap_or(range.default_value)
        .clamp(range.min, range.max);
    Some((range, quality))
}

fn vbr_estimate_kbps(config: &ConversionConfig) -> Option<u32> {
    let (_, quality) = audio_vbr_quality(config)?;
    vbr_kbps(&config.audio_codec, quality)
}

pub(super) fn audio_vbr_control(config: &ConversionConfig) -> Option<AudioVbrControl> {
    let (range, quality) = audio_vbr_quality(config)?;
    let position = if range.lower_is_better {
        range.max + range.min - quality
    } else {
        quality
    };
    let span = usize::try_from((range.max - range.min).max(1)).unwrap_or(1);
    let step = usize::try_from(position - range.min).unwrap_or(0);
    let word = VBR_LEVEL_WORDS[(step * (VBR_LEVEL_WORDS.len() - 1) + span / 2) / span];
    let badge = vbr_kbps(&config.audio_codec, quality).map_or_else(
        || word.to_string(),
        |kbps| format!("{word} · ≈ {kbps} kb/s"),
    );
    Some(AudioVbrControl {
        position,
        min: range.min,
        max: range.max,
        badge,
    })
}

/// The codec quality value for a VBR slider position.
pub(super) fn audio_quality_for_position(codec: &str, position: u32) -> Option<u32> {
    let range = audio_quality_range(codec)?;
    let position = position.clamp(range.min, range.max);
    Some(if range.lower_is_better {
        range.max + range.min - position
    } else {
        position
    })
}

/// One line summing up the audio section while it is collapsed.
pub(super) fn audio_section_summary(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> String {
    if !exports_audio(config, metadata) {
        return "No audio tracks selected".to_string();
    }
    let channels = match config.audio_channels.as_str() {
        "stereo" => "stereo",
        "mono" => "mono",
        _ => "original channels",
    };
    format!("{} · {channels}", audio_summary(config, metadata))
}

fn audio_kbps(config: &ConversionConfig) -> f64 {
    let mono = config.audio_channels == "mono";
    match config.audio_codec.as_str() {
        codec if codec.starts_with("pcm") => {
            if mono {
                768.0
            } else {
                1_536.0
            }
        }
        "flac" | "alac" => {
            if mono {
                450.0
            } else {
                900.0
            }
        }
        _ if !uses_audio_bitrate(config) => vbr_estimate_kbps(config).map_or(192.0, f64::from),
        _ => config.audio_bitrate.parse().unwrap_or(192.0),
    }
}

pub(super) fn audio_summary(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
) -> String {
    if config.processing_mode == ProcessingMode::Copy {
        return if exports_audio(config, metadata) {
            "Copied from source".to_string()
        } else {
            "None".to_string()
        };
    }
    if !exports_audio(config, metadata) {
        return "None".to_string();
    }
    let codec = match config.audio_codec.as_str() {
        "aac" | "libfdk_aac" => "AAC",
        "libopus" => "Opus",
        "mp3" => "MP3",
        "ac3" => "Dolby Digital",
        "flac" => "FLAC",
        "alac" => "ALAC",
        "mp2" => "MP2",
        codec if codec.starts_with("pcm") => "PCM",
        _ => return config.audio_codec.to_uppercase(),
    };
    if is_lossless_audio_codec(&config.audio_codec) {
        format!("{codec} · lossless")
    } else if uses_audio_bitrate(config) {
        format!("{codec} · {} kb/s", config.audio_bitrate)
    } else {
        vbr_estimate_kbps(config).map_or_else(
            || format!("{codec} · VBR"),
            |kbps| format!("{codec} · VBR ≈ {kbps} kb/s"),
        )
    }
}

/// Relative bitrate needed for the same quality, against H.264.
fn codec_efficiency(codec: &str) -> f64 {
    match codec {
        "libx265" | "hevc_nvenc" | "hevc_videotoolbox" => 0.6,
        "libsvtav1" | "av1_nvenc" => 0.5,
        "vp9" => 0.65,
        "mpeg2video" => 2.0,
        _ => 1.0,
    }
}

/// A rough output size in bytes. Constant-quality output depends on content, so this models a
/// typical screen recording: about 8 Mb/s for 1080p30 H.264 at CRF 23, halving every 6 CRF.
pub(super) fn estimate_size_bytes(
    config: &ConversionConfig,
    metadata: Option<&SourceMetadata>,
    duration: f64,
) -> Option<f64> {
    if duration <= 0.0 {
        return None;
    }
    let audio = if exports_audio(config, metadata) {
        audio_kbps(config)
    } else {
        0.0
    };
    if config.processing_mode == ProcessingMode::Copy {
        let video = metadata.and_then(|metadata| metadata.video_bitrate_kbps)?;
        return Some((video + audio) * 1_000.0 * duration / 8.0);
    }

    let (width, height) = output_dimensions(config, metadata)?;
    let pixels = f64::from(width) * f64::from(height) / (1_920.0 * 1_080.0);
    let fps = output_fps(config, metadata).unwrap_or(30.0) / 30.0;
    let video_kbps = if is_gif_container(&config.container) {
        6_000.0 * pixels * fps * f64::from(config.gif_colors) / 256.0
    } else if config.video_codec == "prores" {
        147_000.0 * pixels * fps
    } else if quality_control(config).target == QualityTarget::Bitrate {
        f64::from(bitrate_mbps(config)) * 1_000.0
    } else {
        let crf = if is_hardware_video_codec(&config.video_codec) {
            // Maps quality 1..=100 onto roughly the same CRF range the NVENC mapping uses.
            51.0 - f64::from(config.quality) / 2.0
        } else {
            f64::from(config.crf)
        };
        8_000.0 * pixels * fps * ((23.0 - crf) / 6.0).exp2() * codec_efficiency(&config.video_codec)
    };
    Some((video_kbps + audio) * 1_000.0 * duration / 8.0)
}

pub(super) fn format_size(bytes: f64) -> String {
    let megabytes = bytes / 1_000_000.0;
    if megabytes >= 1_000.0 {
        format!("{:.1} GB", megabytes / 1_000.0)
    } else if megabytes < 10.0 {
        format!("{megabytes:.1} MB")
    } else {
        format!("{megabytes:.0} MB")
    }
}

/// `00:24.0` under an hour, `1:02:05` past it.
pub(super) fn format_clock(seconds: f64) -> String {
    let tenths = rounded_f64_to_u64(seconds.max(0.0) * 10.0);
    let hours = tenths / 36_000;
    let minutes = (tenths / 600) % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{:02}", (tenths / 10) % 60)
    } else {
        format!("{minutes:02}:{:04.1}", u64_to_f64(tenths % 600) / 10.0)
    }
}

/// Where an export started from the dialog stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ExportPhase {
    /// Nothing exporting: the dialog shows the settings.
    Idle,
    /// `progress` is the mean completed fraction across the exported files.
    Running {
        progress: f32,
    },
    Done {
        completed: usize,
        failed: usize,
    },
}

pub(super) fn export_phase(files: &[FileItem], task_ids: &[String]) -> ExportPhase {
    let tasks: Vec<_> = files
        .iter()
        .filter(|file| task_ids.contains(&file.id))
        .collect();
    let running = tasks.iter().any(|file| {
        matches!(
            file.status,
            FileStatus::Queued
                | FileStatus::Converting
                | FileStatus::Paused
                | FileStatus::Cancelling
        )
    });
    if running {
        let total: u32 = tasks
            .iter()
            .map(|file| match file.status {
                FileStatus::Completed | FileStatus::Error => 100,
                _ => u32::from(file.progress_percent),
            })
            .sum();
        let progress = u32_to_f32(total) / 100.0 / usize_to_f32(tasks.len().max(1));
        return ExportPhase::Running { progress };
    }
    let completed = tasks
        .iter()
        .filter(|file| file.status == FileStatus::Completed)
        .count();
    let failed = tasks
        .iter()
        .filter(|file| file.status == FileStatus::Error)
        .count();
    if completed + failed == 0 {
        ExportPhase::Idle
    } else {
        ExportPhase::Done { completed, failed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{CropSettings, CutSegment};

    fn video_metadata(width: u32, height: u32, fps: f64) -> SourceMetadata {
        SourceMetadata {
            width: Some(width),
            height: Some(height),
            frame_rate: Some(fps),
            video_codec: Some("h264".to_string()),
            ..SourceMetadata::default()
        }
    }

    #[test]
    fn preset_resolutions_keep_aspect_with_even_width() {
        let metadata = video_metadata(1920, 1080, 30.0);
        let mut config = ConversionConfig::default();
        assert_eq!(
            output_dimensions(&config, Some(&metadata)),
            Some((1920, 1080))
        );
        config.resolution = "720p".to_string();
        assert_eq!(
            output_dimensions(&config, Some(&metadata)),
            Some((1280, 720))
        );
        config.resolution = "480p".to_string();
        assert_eq!(
            output_dimensions(&config, Some(&metadata)),
            Some((854, 480))
        );
    }

    #[test]
    fn rotation_and_crop_change_the_source_frame() {
        let metadata = video_metadata(1920, 1080, 30.0);
        let mut config = ConversionConfig {
            rotation: "90".to_string(),
            ..ConversionConfig::default()
        };
        assert_eq!(
            source_dimensions(&config, Some(&metadata)),
            Some((1080, 1920))
        );
        config.crop = Some(CropSettings {
            enabled: true,
            width: 640,
            height: 360,
            ..CropSettings::default()
        });
        assert_eq!(
            source_dimensions(&config, Some(&metadata)),
            Some((640, 360))
        );
    }

    #[test]
    fn custom_size_fills_a_missing_side_from_the_source_aspect() {
        let metadata = video_metadata(1920, 1080, 30.0);
        let config = ConversionConfig {
            resolution: "custom".to_string(),
            custom_width: Some("1280".to_string()),
            ..ConversionConfig::default()
        };
        assert_eq!(
            output_dimensions(&config, Some(&metadata)),
            Some((1280, 720))
        );
        assert_eq!(custom_dimensions_error(&config), None);
    }

    #[test]
    fn custom_size_rejects_odd_tiny_or_missing_values() {
        let mut config = ConversionConfig {
            resolution: "custom".to_string(),
            ..ConversionConfig::default()
        };
        assert!(custom_dimensions_error(&config).is_some());
        config.custom_width = Some("641".to_string());
        assert!(custom_dimensions_error(&config).is_some());
        config.custom_width = Some("8".to_string());
        assert!(custom_dimensions_error(&config).is_some());
        config.custom_width = Some("640".to_string());
        config.custom_height = Some("360".to_string());
        assert_eq!(custom_dimensions_error(&config), None);
    }

    #[test]
    fn scaled_even_rounds_to_even_values() {
        assert_eq!(scaled_even(480, 1080, 1920), 854);
        assert_eq!(scaled_even(1000, 1920, 1080), 562);
        assert_eq!(scaled_even(10, 0, 99), 99);
    }

    #[test]
    fn fps_note_compares_against_the_source() {
        let metadata = video_metadata(1920, 1080, 30.0);
        let mut config = ConversionConfig::default();
        assert_eq!(
            fps_note(&config, Some(&metadata)),
            ("Matches source", false)
        );
        config.fps = "60".to_string();
        assert!(fps_note(&config, Some(&metadata)).1);
        config.fps = "24".to_string();
        assert!(!fps_note(&config, Some(&metadata)).1);
        assert_eq!(fps_note(&config, None), ("", false));
    }

    #[test]
    fn rates_format_without_trailing_zeros() {
        assert_eq!(format_rate(30.0), "30");
        assert_eq!(format_rate(29.97), "29.97");
        assert_eq!(format_rate(23.976_023), "23.976");
        assert_eq!(fps_option_label("original", Some(30.0)), "Source (30)");
        assert_eq!(fps_option_label("original", None), "Source");
        assert_eq!(fps_option_label("24", None), "24 fps");
    }

    #[test]
    fn quality_control_follows_codec_and_mode() {
        let mut config = ConversionConfig::default();
        let crf = quality_control(&config);
        assert_eq!(crf.target, QualityTarget::Crf);
        assert_eq!(crf.badge, "Balanced · CRF 23");

        config.video_codec = "h264_nvenc".to_string();
        assert_eq!(quality_control(&config).target, QualityTarget::Quality);

        config.video_bitrate_mode = "bitrate".to_string();
        config.video_bitrate = "7500".to_string();
        let bitrate = quality_control(&config);
        assert_eq!(bitrate.target, QualityTarget::Bitrate);
        assert_eq!(bitrate.value, 8);

        config.video_bitrate_mode = "crf".to_string();
        config.video_codec = "mpeg2video".to_string();
        let mpeg2 = quality_control(&config);
        assert_eq!(mpeg2.target, QualityTarget::Bitrate);
        assert!(!mpeg2.mode_selectable);
    }

    #[test]
    fn speed_presets_respect_the_encoder() {
        assert_eq!(speed_presets("libx264").len(), 9);
        assert_eq!(
            speed_presets("h264_nvenc")
                .into_iter()
                .map(|(preset, _)| preset)
                .collect::<Vec<_>>(),
            vec!["fast", "medium", "slow"]
        );
        assert!(speed_presets("h264_videotoolbox").is_empty());
        assert!(speed_presets("gif").is_empty());
    }

    #[test]
    fn reset_restores_recommended_settings() {
        let mut config = ConversionConfig {
            container: "webm".to_string(),
            video_codec: "vp9".to_string(),
            resolution: "720p".to_string(),
            crf: 30,
            ..ConversionConfig::default()
        };
        assert!(!is_recommended(&config));
        assert!(reset_to_recommended(&mut config));
        assert!(is_recommended(&config));
        assert!(!reset_to_recommended(&mut config));
    }

    #[test]
    fn switching_to_gif_shrinks_full_size_exports() {
        let mut config = ConversionConfig::default();
        assert!(apply_export_format(&mut config, "gif"));
        assert_eq!(config.container, "gif");
        assert_eq!(config.resolution, "480p");
        assert_eq!(config.fps, "15");

        assert!(apply_export_format(&mut config, "mp4"));
        assert_eq!(config.fps, "original");
        assert_eq!(config.resolution, "480p");
    }

    #[test]
    fn webm_then_mov_swaps_opus_for_a_mov_audio_codec() {
        let mut config = ConversionConfig::default();
        apply_export_format(&mut config, "webm");
        assert_eq!(config.audio_codec, "libopus");
        apply_export_format(&mut config, "mov");
        assert_eq!(config.audio_codec, "aac");
    }

    #[test]
    fn saved_mov_with_opus_is_repaired_before_export() {
        let mut config = ConversionConfig {
            container: "mov".to_string(),
            audio_codec: "libopus".to_string(),
            ..ConversionConfig::default()
        };
        assert!(crate::settings::normalize_output_config(&mut config, None));
        assert_eq!(config.audio_codec, "aac");
    }

    #[test]
    fn output_duration_subtracts_trim_and_cuts() {
        let config = ConversionConfig {
            start_time: Some("00:00:02".to_string()),
            end_time: Some("00:00:20".to_string()),
            cut_segments: vec![CutSegment {
                start_time: "00:00:05".to_string(),
                end_time: "00:00:08".to_string(),
            }],
            ..ConversionConfig::default()
        };
        assert!((output_duration_seconds(&config, 30.0) - 15.0).abs() < 1e-9);
        assert!((output_duration_seconds(&ConversionConfig::default(), 24.0) - 24.0).abs() < 1e-9);
    }

    #[test]
    fn size_estimate_scales_with_quality_and_resolution() {
        let metadata = video_metadata(1920, 1080, 30.0);
        let mut config = ConversionConfig::default();
        let base = estimate_size_bytes(&config, Some(&metadata), 10.0).expect("estimate");
        config.crf = 29;
        let smaller = estimate_size_bytes(&config, Some(&metadata), 10.0).expect("estimate");
        assert!(smaller < base);
        config.crf = 23;
        config.resolution = "720p".to_string();
        let lower = estimate_size_bytes(&config, Some(&metadata), 10.0).expect("estimate");
        assert!(lower < base);
        assert_eq!(estimate_size_bytes(&config, Some(&metadata), 0.0), None);
        assert_eq!(estimate_size_bytes(&config, None, 10.0), None);
    }

    #[test]
    fn clock_switches_to_hours_past_an_hour() {
        assert_eq!(format_clock(24.0), "00:24.0");
        assert_eq!(format_clock(725.3), "12:05.3");
        assert_eq!(format_clock(3_725.0), "1:02:05");
    }

    #[test]
    fn sizes_format_by_magnitude() {
        assert_eq!(format_size(8_400_000.0), "8.4 MB");
        assert_eq!(format_size(54_000_000.0), "54 MB");
        assert_eq!(format_size(1_260_000_000.0), "1.3 GB");
    }

    fn file(id: &str, status: FileStatus, progress: u8) -> FileItem {
        FileItem {
            status,
            progress_percent: progress,
            ..FileItem::from_path(id, format!("/tmp/{id}.mp4"), 10)
        }
    }

    #[test]
    fn export_phase_tracks_the_started_tasks() {
        let ids = vec!["a".to_string(), "b".to_string()];
        assert_eq!(export_phase(&[], &[]), ExportPhase::Idle);

        let files = [
            file("a", FileStatus::Converting, 50),
            file("b", FileStatus::Completed, 100),
        ];
        let ExportPhase::Running { progress } = export_phase(&files, &ids) else {
            panic!("expected a running export");
        };
        assert!((progress - 0.75).abs() < 1e-6);

        let files = [
            file("a", FileStatus::Completed, 100),
            file("b", FileStatus::Error, 0),
        ];
        assert_eq!(
            export_phase(&files, &ids),
            ExportPhase::Done {
                completed: 1,
                failed: 1
            }
        );

        let files = [
            file("a", FileStatus::Idle, 0),
            file("b", FileStatus::Idle, 0),
        ];
        assert_eq!(export_phase(&files, &ids), ExportPhase::Idle);
    }

    #[test]
    fn audio_summary_reflects_codec_and_tracks() {
        let config = ConversionConfig::default();
        assert_eq!(
            audio_summary(&config, None),
            format!("AAC · {} kb/s", config.audio_bitrate)
        );
        let silent = SourceMetadata {
            audio_tracks: Vec::new(),
            ..video_metadata(1920, 1080, 30.0)
        };
        assert_eq!(audio_summary(&config, Some(&silent)), "None");
        let gif = ConversionConfig {
            container: "gif".to_string(),
            ..ConversionConfig::default()
        };
        assert_eq!(audio_summary(&gif, None), "None");
        let flac = ConversionConfig {
            container: "mkv".to_string(),
            audio_codec: "flac".to_string(),
            ..ConversionConfig::default()
        };
        assert_eq!(audio_summary(&flac, None), "FLAC · lossless");
        let vbr = ConversionConfig {
            audio_codec: "mp3".to_string(),
            audio_bitrate_mode: "vbr".to_string(),
            audio_quality: "4".to_string(),
            ..ConversionConfig::default()
        };
        assert_eq!(audio_summary(&vbr, None), "MP3 · VBR ≈ 165 kb/s");
        assert_eq!(
            audio_section_summary(&vbr, None),
            "MP3 · VBR ≈ 165 kb/s · original channels"
        );
    }

    #[test]
    fn audio_bitrate_is_checked_against_the_codec() {
        let mut config = ConversionConfig {
            audio_bitrate: "192".to_string(),
            ..ConversionConfig::default()
        };
        assert_eq!(audio_bitrate_error(&config), None);
        config.audio_bitrate = "16".to_string();
        assert_eq!(
            audio_bitrate_error(&config).as_deref(),
            Some("Enter 32–640 kb/s")
        );
        config.audio_bitrate = String::new();
        assert!(audio_bitrate_error(&config).is_some());

        config.audio_codec = "libopus".to_string();
        config.audio_bitrate = "600".to_string();
        assert_eq!(
            audio_bitrate_error(&config).as_deref(),
            Some("Enter 32–512 kb/s")
        );

        config.audio_codec = "mp2".to_string();
        config.audio_bitrate = "200".to_string();
        assert!(audio_bitrate_error(&config).is_some());
        config.audio_bitrate = "192".to_string();
        assert_eq!(audio_bitrate_error(&config), None);

        // Lossless and VBR audio have no bitrate to check.
        config.audio_codec = "alac".to_string();
        config.audio_bitrate = "0".to_string();
        assert_eq!(audio_bitrate_error(&config), None);
        config.audio_codec = "mp3".to_string();
        config.audio_bitrate_mode = "vbr".to_string();
        assert_eq!(audio_bitrate_error(&config), None);
        config.processing_mode = ProcessingMode::Copy;
        config.audio_bitrate_mode = "bitrate".to_string();
        assert_eq!(audio_bitrate_error(&config), None);
    }

    #[test]
    fn vbr_slider_runs_from_small_to_high_quality() {
        let mut config = ConversionConfig {
            audio_codec: "mp3".to_string(),
            audio_bitrate_mode: "vbr".to_string(),
            audio_quality: "0".to_string(),
            ..ConversionConfig::default()
        };
        let best = audio_vbr_control(&config).expect("mp3 has VBR");
        assert_eq!((best.position, best.min, best.max), (9, 0, 9));
        assert_eq!(best.badge, "Very high · ≈ 245 kb/s");
        assert_eq!(audio_quality_for_position("mp3", 9), Some(0));
        assert_eq!(audio_quality_for_position("mp3", 0), Some(9));

        config.audio_quality = "9".to_string();
        assert_eq!(
            audio_vbr_control(&config).expect("mp3 has VBR").badge,
            "Low · ≈ 65 kb/s"
        );

        config.audio_codec = "libfdk_aac".to_string();
        config.audio_quality = "4".to_string();
        let fdk = audio_vbr_control(&config).expect("FDK AAC has VBR");
        assert_eq!(fdk.position, 4);
        assert_eq!(fdk.badge, "High · ≈ 144 kb/s");
        assert_eq!(audio_quality_for_position("libfdk_aac", 4), Some(4));

        config.audio_codec = "aac".to_string();
        assert_eq!(audio_vbr_control(&config), None);
    }

    #[test]
    fn channel_note_reads_the_source_tracks() {
        let metadata = SourceMetadata {
            audio_tracks: vec![crate::settings::AudioTrack {
                index: 1,
                channels: Some("6".to_string()),
                ..crate::settings::AudioTrack::default()
            }],
            ..video_metadata(1920, 1080, 30.0)
        };
        let mut config = ConversionConfig {
            selected_audio_tracks: vec![1],
            ..ConversionConfig::default()
        };
        assert_eq!(
            audio_channels_note(&config, Some(&metadata)),
            "Keeps 6 channels from source"
        );
        assert_eq!(
            audio_channels_note(&config, None),
            "Keeps the source channels"
        );
        config.audio_codec = "mp2".to_string();
        assert_eq!(
            audio_channels_note(&config, Some(&metadata)),
            "MP2 keeps at most 2 channels"
        );
        config.audio_channels = "mono".to_string();
        assert_eq!(
            audio_channels_note(&config, Some(&metadata)),
            "Mixes down to 1 channel"
        );
    }

    #[test]
    fn reset_restores_recommended_audio() {
        let mut config = ConversionConfig {
            audio_codec: "mp3".to_string(),
            audio_channels: "mono".to_string(),
            audio_bitrate: "320".to_string(),
            ..ConversionConfig::default()
        };
        assert!(!is_recommended(&config));
        assert!(reset_to_recommended(&mut config));
        assert!(is_recommended(&config));
        assert_eq!(config.audio_codec, "aac");
        assert_eq!(config.audio_channels, "original");
    }
}
