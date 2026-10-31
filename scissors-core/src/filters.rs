use crate::{
    media_filters::{
        build_audio_effect_filters, build_video_post_scale_filters, build_video_pre_scale_filters,
    },
    media_rules::is_image_container,
    types::ConversionConfig,
};

pub const EVEN_DIMENSIONS_FILTER: &str = "pad=ceil(iw/2)*2:ceil(ih/2)*2:0:0";
pub const PREVIEW_OUTPUT_LABEL: &str = "preview_v";
pub const VIDEO_OUTPUT_LABEL: &str = "vout";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualFilterBase {
    Video,
    Image,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualFilterProfile {
    ExportVideo,
    ExportImage,
    PreviewLowRes {
        base: VisualFilterBase,
        width: u32,
        height: u32,
        fps: u32,
    },
}

fn rounded_i32(value: f64, min_value: f64) -> i32 {
    let clamped = value
        .max(min_value)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "value is rounded and clamped into i32 range first"
    )]
    let converted = clamped as i32;
    converted
}

#[must_use]
pub fn build_video_filters(config: &ConversionConfig, include_scale: bool) -> Vec<String> {
    let mut filters = Vec::new();
    let is_image = is_image_container(&config.container);

    if config.flip_horizontal {
        filters.push("hflip".to_string());
    }
    if config.flip_vertical {
        filters.push("vflip".to_string());
    }

    match config.rotation.as_str() {
        "90" => filters.push("transpose=1".to_string()),
        "180" => filters.push("transpose=1,transpose=1".to_string()),
        "270" => filters.push("transpose=2".to_string()),
        _ => {}
    }

    if let Some(crop) = &config.crop
        && crop.enabled
    {
        let crop_width = rounded_i32(crop.width, 1.0);
        let crop_height = rounded_i32(crop.height, 1.0);
        let crop_x = rounded_i32(crop.x, 0.0);
        let crop_y = rounded_i32(crop.y, 0.0);
        filters.push(format!("crop={crop_width}:{crop_height}:{crop_x}:{crop_y}"));
    }

    filters.extend(build_video_pre_scale_filters(
        &config.video_filters,
        is_image,
    ));

    if include_scale && (config.resolution != "original" || config.resolution == "custom") {
        filters.push(build_resolution_scale_filter(config));
    }

    filters.extend(build_video_post_scale_filters(&config.video_filters));

    filters
}

fn build_resolution_scale_filter(config: &ConversionConfig) -> String {
    let algorithm = match config.scaling_algorithm.as_str() {
        "lanczos" => ":flags=lanczos",
        "bilinear" => ":flags=bilinear",
        "nearest" => ":flags=neighbor",
        "bicubic" => ":flags=bicubic",
        _ => "",
    };

    if config.resolution == "custom" {
        let width = config.custom_width.as_deref().unwrap_or("-1");
        let height = config.custom_height.as_deref().unwrap_or("-1");
        return custom_resolution_scale_filter(width, height, algorithm);
    }

    match config.resolution.as_str() {
        "1080p" => format!("scale=-2:1080{algorithm}"),
        "720p" => format!("scale=-2:720{algorithm}"),
        "480p" => format!("scale=-2:480{algorithm}"),
        _ => "scale=-1:-1".to_string(),
    }
}

fn custom_resolution_scale_filter(width: &str, height: &str, algorithm: &str) -> String {
    if width != "-1" && height != "-1" {
        format!(
            "scale={width}:{height}:force_original_aspect_ratio=decrease{algorithm},pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"
        )
    } else if width == "-1" && height == "-1" {
        "scale=-1:-1".to_string()
    } else {
        format!("scale={width}:{height}{algorithm}")
    }
}

#[must_use]
pub fn build_encode_video_filters(config: &ConversionConfig, include_scale: bool) -> Vec<String> {
    let mut filters = build_video_filters(config, include_scale);
    filters.push(EVEN_DIMENSIONS_FILTER.to_string());
    filters
}

#[must_use]
pub fn build_visual_filter_chain(
    config: &ConversionConfig,
    profile: VisualFilterProfile,
) -> Vec<String> {
    match profile {
        VisualFilterProfile::ExportVideo => build_encode_video_filters(config, true),
        VisualFilterProfile::ExportImage => build_video_filters(config, true),
        VisualFilterProfile::PreviewLowRes {
            base,
            width,
            height,
            fps,
        } => {
            let mut filters = match base {
                VisualFilterBase::Video => build_encode_video_filters(config, true),
                VisualFilterBase::Image => build_video_filters(config, true),
            };
            filters.extend(preview_low_res_filters(width, height, fps));
            filters
        }
    }
}

#[must_use]
pub fn build_visual_filter_complex(
    config: &ConversionConfig,
    profile: VisualFilterProfile,
) -> String {
    match profile {
        VisualFilterProfile::ExportVideo | VisualFilterProfile::ExportImage => {
            build_export_filter_complex(
                config,
                &build_visual_filter_chain(config, profile),
                VIDEO_OUTPUT_LABEL,
            )
        }
        VisualFilterProfile::PreviewLowRes {
            base,
            width,
            height,
            fps,
        } => build_preview_filter_complex(config, base, width, height, fps),
    }
}

#[must_use]
pub fn has_overlay(config: &ConversionConfig) -> bool {
    config
        .overlay
        .as_ref()
        .is_some_and(|overlay| overlay.enabled && !overlay.path.trim().is_empty())
}

#[must_use]
pub fn build_overlay_filter_complex(config: &ConversionConfig) -> String {
    let filters = build_video_filters(config, true);
    build_overlay_filter_complex_with_filters(config, &filters, VIDEO_OUTPUT_LABEL)
}

/// Builds the export overlay graph; `post_filters` run on the composited frames.
#[must_use]
pub fn build_encode_overlay_filter_complex(
    config: &ConversionConfig,
    post_filters: &[String],
) -> String {
    let filters = build_encode_video_filters(config, true);
    if post_filters.is_empty() {
        return build_overlay_filter_complex_with_filters(config, &filters, VIDEO_OUTPUT_LABEL);
    }

    let graph = build_overlay_filter_complex_with_filters(config, &filters, "overlay_out");
    format!(
        "{graph};[overlay_out]{}[{VIDEO_OUTPUT_LABEL}]",
        post_filters.join(",")
    )
}

fn build_overlay_filter_complex_with_filters(
    config: &ConversionConfig,
    filters: &[String],
    output_label: &str,
) -> String {
    let Some(overlay) = &config.overlay else {
        return labeled_filter_chain(filters, output_label);
    };

    let base_chain = labeled_filter_chain(filters, "base");
    let x = overlay.x.clamp(0.0, 1.0);
    let y = overlay.y.clamp(0.0, 1.0);
    let width = overlay.width.clamp(0.03, 0.8);
    let opacity = overlay.opacity.clamp(0.0, 1.0);

    format!(
        "{base_chain};[base]split[base_ref][base_out];[1:v:0]format=rgba,colorchannelmixer=aa={opacity:.3}[overlay_src];[overlay_src][base_ref]scale=w='min(rw*{width:.6},rh*iw/ih)':h=-1[overlay_scaled];[base_out][overlay_scaled]overlay=x='min(max(main_w*{x:.6}-overlay_w/2,0),main_w-overlay_w)':y='min(max(main_h*{y:.6}-overlay_h/2,0),main_h-overlay_h)':format=auto[{output_label}]"
    )
}

fn build_export_filter_complex(
    config: &ConversionConfig,
    filters: &[String],
    output_label: &str,
) -> String {
    if has_overlay(config) {
        build_overlay_filter_complex_with_filters(config, filters, output_label)
    } else {
        labeled_filter_chain(filters, output_label)
    }
}

fn build_preview_filter_complex(
    config: &ConversionConfig,
    base: VisualFilterBase,
    width: u32,
    height: u32,
    fps: u32,
) -> String {
    let base_filters = match base {
        VisualFilterBase::Video => build_encode_video_filters(config, true),
        VisualFilterBase::Image => build_video_filters(config, true),
    };
    let preview_filters = preview_low_res_filters(width, height, fps);
    let preview_chain = preview_filters.join(",");

    if has_overlay(config) {
        let graph =
            build_overlay_filter_complex_with_filters(config, &base_filters, "preview_export");
        format!("{graph};[preview_export]{preview_chain}[{PREVIEW_OUTPUT_LABEL}]")
    } else {
        let mut filters = base_filters;
        filters.extend(preview_filters);
        labeled_filter_chain(&filters, PREVIEW_OUTPUT_LABEL)
    }
}

fn labeled_filter_chain(filters: &[String], output_label: &str) -> String {
    if filters.is_empty() {
        format!("[0:v:0]null[{output_label}]")
    } else {
        format!("[0:v:0]{}[{output_label}]", filters.join(","))
    }
}

fn preview_low_res_filters(width: u32, height: u32, fps: u32) -> Vec<String> {
    vec![
        format!("fps={fps}"),
        format!("scale={width}:{height}"),
        "setsar=1".to_string(),
        "format=bgra".to_string(),
    ]
}

#[must_use]
pub fn build_audio_filters(config: &ConversionConfig) -> Vec<String> {
    build_audio_effect_filters(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CropConfig, MetadataConfig, OverlayConfig};

    fn default_config() -> ConversionConfig {
        ConversionConfig {
            processing_mode: "reencode".to_string(),
            container: "mp4".to_string(),
            video_codec: "libx264".to_string(),
            video_bitrate_mode: "crf".to_string(),
            video_bitrate: "5000".to_string(),
            audio_codec: "aac".to_string(),
            audio_bitrate: "192".to_string(),
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
            scaling_algorithm: "lanczos".to_string(),
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

    #[test]
    fn test_empty_video_filters() {
        let config = default_config();
        let filters = build_video_filters(&config, true);
        assert!(filters.is_empty());
    }

    #[test]
    fn encode_video_filters_add_even_dimensions_guard_for_original_resolution() {
        let config = default_config();
        let filters = build_encode_video_filters(&config, true);
        assert_eq!(filters, vec![EVEN_DIMENSIONS_FILTER]);
    }

    #[test]
    fn test_flip_filters() {
        let mut config = default_config();
        config.flip_horizontal = true;
        config.flip_vertical = true;
        let filters = build_video_filters(&config, true);
        assert_eq!(filters, vec!["hflip", "vflip"]);
    }

    #[test]
    fn test_rotation_filter() {
        let mut config = default_config();
        config.rotation = "90".to_string();
        let filters = build_video_filters(&config, true);
        assert_eq!(filters, vec!["transpose=1"]);
    }

    #[test]
    fn test_crop_filter() {
        let mut config = default_config();
        config.crop = Some(CropConfig {
            enabled: true,
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 200.0,
            source_width: None,
            source_height: None,
            aspect_ratio: None,
        });
        let filters = build_video_filters(&config, true);
        assert_eq!(filters, vec!["crop=100:200:10:20"]);
    }

    #[test]
    fn encode_video_filters_append_even_dimensions_guard_after_user_filters() {
        let mut config = default_config();
        config.crop = Some(CropConfig {
            enabled: true,
            x: 10.0,
            y: 20.0,
            width: 101.0,
            height: 201.0,
            source_width: None,
            source_height: None,
            aspect_ratio: None,
        });

        let filters = build_encode_video_filters(&config, true);

        assert_eq!(filters, vec!["crop=101:201:10:20", EVEN_DIMENSIONS_FILTER]);
    }

    #[test]
    fn test_overlay_filter_complex() {
        let mut config = default_config();
        config.resolution = "720p".to_string();
        config.overlay = Some(OverlayConfig {
            enabled: true,
            path: "/tmp/logo.png".to_string(),
            x: 0.9,
            y: 0.85,
            width: 0.2,
            opacity: 0.75,
            anchor: "custom".to_string(),
        });

        let filter = build_overlay_filter_complex(&config);

        assert!(filter.contains("[0:v:0]scale=-2:720:flags=lanczos[base]"));
        assert!(filter.contains("[1:v:0]format=rgba,colorchannelmixer=aa=0.750"));
        assert!(filter.contains("[base]split[base_ref][base_out]"));
        assert!(filter.contains("[overlay_src][base_ref]scale=w='min(rw*0.200000,rh*iw/ih)':h=-1"));
        assert!(
            filter.contains(
                "overlay=x='min(max(main_w*0.900000-overlay_w/2,0),main_w-overlay_w)':y='min(max(main_h*0.850000-overlay_h/2,0),main_h-overlay_h)'"
            )
        );
        assert!(filter.ends_with("[vout]"));
    }

    #[test]
    fn encode_overlay_filter_complex_pads_base_before_overlay() {
        let mut config = default_config();
        config.overlay = Some(OverlayConfig {
            enabled: true,
            path: "/tmp/logo.png".to_string(),
            x: 0.5,
            y: 0.5,
            width: 0.2,
            opacity: 1.0,
            anchor: "custom".to_string(),
        });

        let filter = build_encode_overlay_filter_complex(&config, &[]);

        assert!(filter.contains("[0:v:0]pad=ceil(iw/2)*2:ceil(ih/2)*2:0:0[base]"));
    }

    #[test]
    fn preview_low_res_filter_complex_keeps_overlay_before_fps_and_scale() {
        let mut config = default_config();
        config.overlay = Some(OverlayConfig {
            enabled: true,
            path: "/tmp/logo.png".to_string(),
            x: 0.5,
            y: 0.5,
            width: 0.2,
            opacity: 1.0,
            anchor: "custom".to_string(),
        });

        let filter = build_visual_filter_complex(
            &config,
            VisualFilterProfile::PreviewLowRes {
                base: VisualFilterBase::Video,
                width: 640,
                height: 360,
                fps: 24,
            },
        );

        assert!(filter.contains("[preview_export]fps=24,scale=640:360"));
    }

    #[test]
    fn test_audio_normalize_filter() {
        let mut config = default_config();
        config.audio_normalize = true;
        let filters = build_audio_filters(&config);
        assert_eq!(filters, vec!["loudnorm=I=-16:TP=-1.5:LRA=11"]);
    }

    #[test]
    fn test_audio_volume_filter() {
        let mut config = default_config();
        config.audio_volume = 150.0;
        let filters = build_audio_filters(&config);
        assert_eq!(filters, vec!["volume=1.500"]);
    }
}
