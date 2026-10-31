use super::filter_controls::{filter_checkbox, filter_section, filter_value};
use super::*;
use crate::app::m3::{self, M3Text};
use crate::material::{ColorScheme, Symbol, spacing, typography};
use crate::settings::{
    DeinterlaceMode, FilterStrength, VideoScalarFilter, apply_video_deinterlace,
    apply_video_denoise, apply_video_grayscale, apply_video_scalar_filter, reset_video_filters,
};
use scissors_core::capabilities::AvailableFilters;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VideoFilterRangeTarget {
    Brightness,
    Contrast,
    Saturation,
    Gamma,
    Hue,
    Temperature,
    Sharpen,
    GaussianBlur,
    Deband,
    Vignette,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VideoFilterRangeDrag {
    target: VideoFilterRangeTarget,
    min: i32,
    max: i32,
}

struct VideoFilterRangeDragPreview;

impl Render for VideoFilterRangeDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(theme::ui_rem(0.0)).h(theme::ui_rem(0.0))
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "The tab render function keeps the approved filter section order visible."
)]
pub(in crate::app) fn settings_video_filters_tab(
    config: &ConversionConfig,
    settings_disabled: bool,
    is_image_source: bool,
    available_filters: &AvailableFilters,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let filters = config.video_filters;
    let mut content = div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(spacing::SPACE_6))
        .text_color(color(scheme.on_surface))
        .m3_type(typography::BODY_MEDIUM)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .m3_type(typography::TITLE_SMALL)
                        .child("Video filters"),
                )
                .child(settings_video_filters_reset_all(
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                )),
        )
        .child(
            filter_section("Color adjustments", scheme)
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Brightness,
                        filters.color.brightness.enabled,
                        filters.color.brightness.value,
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Contrast,
                        filters.color.contrast.enabled,
                        i32::try_from(filters.color.contrast.value).unwrap_or(100),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Saturation,
                        filters.color.saturation.enabled,
                        i32::try_from(filters.color.saturation.value).unwrap_or(100),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Gamma,
                        filters.color.gamma.enabled,
                        i32::try_from(filters.color.gamma.value).unwrap_or(100),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                )),
        )
        .child(
            filter_section("Tone", scheme)
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Hue,
                        filters.hue.enabled,
                        filters.hue.value,
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Temperature,
                        filters.temperature.enabled,
                        i32::try_from(filters.temperature.value).unwrap_or(6500),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                )),
        )
        .child(
            filter_section("Detail", scheme)
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Sharpen,
                        filters.sharpen.enabled,
                        i32::try_from(filters.sharpen.value).unwrap_or(25),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::GaussianBlur,
                        filters.gaussian_blur.enabled,
                        i32::try_from(filters.gaussian_blur.value).unwrap_or(20),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                )),
        )
        .child(
            filter_section("Cleanup", scheme)
                .child(settings_video_denoise_control(
                    filters.denoise_enabled,
                    filters.denoise_strength,
                    settings_disabled || !available_filters.hqdn3d,
                    scheme,
                    window,
                    cx,
                ))
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Deband,
                        filters.deband.enabled,
                        i32::try_from(filters.deband.value).unwrap_or(25),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                )),
        )
        .child(
            filter_section("Style", scheme)
                .child(settings_video_filter_range_field(
                    video_filter_spec(
                        VideoFilterRangeTarget::Vignette,
                        filters.vignette.enabled,
                        i32::try_from(filters.vignette.value).unwrap_or(35),
                        available_filters,
                    ),
                    settings_disabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(settings_video_grayscale_control(
                    filters.grayscale,
                    settings_disabled || !available_filters.hue,
                    scheme,
                    window,
                    cx,
                )),
        );

    if !is_image_source {
        content = content.child(filter_section("Interlace", scheme).child(
            settings_video_deinterlace_control(
                filters.deinterlace,
                settings_disabled || !available_filters.bwdif,
                scheme,
                window,
                cx,
            ),
        ));
    }

    content
}

struct VideoFilterRangeSpec {
    target: VideoFilterRangeTarget,
    label: &'static str,
    value_label: String,
    enabled: bool,
    available: bool,
    value: i32,
    default_value: i32,
    min: i32,
    max: i32,
}

fn video_filter_spec(
    target: VideoFilterRangeTarget,
    enabled: bool,
    value: i32,
    available_filters: &AvailableFilters,
) -> VideoFilterRangeSpec {
    let available = video_filter_available(target, available_filters);
    match target {
        VideoFilterRangeTarget::Brightness => video_spec(
            target,
            "Brightness",
            enabled,
            available,
            value,
            0,
            -100,
            100,
            "%",
        ),
        VideoFilterRangeTarget::Contrast => video_spec(
            target, "Contrast", enabled, available, value, 100, 0, 200, "%",
        ),
        VideoFilterRangeTarget::Saturation => video_spec(
            target,
            "Saturation",
            enabled,
            available,
            value,
            100,
            0,
            300,
            "%",
        ),
        VideoFilterRangeTarget::Gamma => video_spec(
            target, "Gamma", enabled, available, value, 100, 10, 300, "%",
        ),
        VideoFilterRangeTarget::Hue => {
            video_spec(target, "Hue", enabled, available, value, 0, -180, 180, "°")
        }
        VideoFilterRangeTarget::Temperature => video_spec(
            target,
            "Temperature",
            enabled,
            available,
            value,
            6500,
            2000,
            12_000,
            " K",
        ),
        VideoFilterRangeTarget::Sharpen => video_spec(
            target, "Sharpen", enabled, available, value, 25, 0, 100, "%",
        ),
        VideoFilterRangeTarget::GaussianBlur => video_spec(
            target,
            "Gaussian blur",
            enabled,
            available,
            value,
            20,
            0,
            100,
            "%",
        ),
        VideoFilterRangeTarget::Deband => {
            video_spec(target, "Deband", enabled, available, value, 25, 0, 100, "%")
        }
        VideoFilterRangeTarget::Vignette => video_spec(
            target, "Vignette", enabled, available, value, 35, 0, 100, "%",
        ),
    }
}

const fn video_filter_available(
    target: VideoFilterRangeTarget,
    available_filters: &AvailableFilters,
) -> bool {
    match target {
        VideoFilterRangeTarget::Brightness
        | VideoFilterRangeTarget::Contrast
        | VideoFilterRangeTarget::Saturation
        | VideoFilterRangeTarget::Gamma => available_filters.eq,
        VideoFilterRangeTarget::Hue => available_filters.hue,
        VideoFilterRangeTarget::Temperature => available_filters.colortemperature,
        VideoFilterRangeTarget::Sharpen => available_filters.unsharp,
        VideoFilterRangeTarget::GaussianBlur => available_filters.gblur,
        VideoFilterRangeTarget::Deband => available_filters.deband,
        VideoFilterRangeTarget::Vignette => available_filters.vignette,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Filter specs are compact data literals for the settings UI."
)]
fn video_spec(
    target: VideoFilterRangeTarget,
    label: &'static str,
    enabled: bool,
    available: bool,
    value: i32,
    default_value: i32,
    min: i32,
    max: i32,
    suffix: &'static str,
) -> VideoFilterRangeSpec {
    VideoFilterRangeSpec {
        target,
        label,
        value_label: format!("{value}{suffix}"),
        enabled,
        available,
        value,
        default_value,
        min,
        max,
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Range specs are short-lived UI values built inline at call sites."
)]
fn settings_video_filter_range_field(
    spec: VideoFilterRangeSpec,
    settings_disabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let unavailable = !spec.available;
    let control_disabled = settings_disabled || unavailable;
    let slider_disabled = control_disabled || !spec.enabled;

    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(spacing::SPACE_2))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(theme::ui_rem(spacing::SPACE_2))
                .child(
                    filter_checkbox(
                        format!(
                            "settings-video-filter-{}-toggle",
                            video_target_id(spec.target)
                        ),
                        spec.label,
                        if unavailable {
                            "This FFmpeg runtime does not provide the required filter."
                        } else {
                            ""
                        },
                        spec.enabled,
                        control_disabled,
                        scheme,
                        window,
                        cx,
                        move |root, _event, _window, cx| {
                            if control_disabled {
                                return;
                            }
                            if root.update_selected_config(|config| {
                                apply_video_filter_range(
                                    config,
                                    spec.target,
                                    !spec.enabled,
                                    spec.value,
                                )
                            }) {
                                cx.notify();
                            }
                        },
                    )
                    .flex_1(),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme::ui_rem(spacing::SPACE_1))
                        .child(filter_value(spec.value_label.clone(), scheme))
                        .child(settings_video_filter_reset(
                            spec.target,
                            spec.default_value,
                            control_disabled,
                            scheme,
                            window,
                            cx,
                        )),
                ),
        )
        .child(settings_video_filter_range_slider(
            spec.value,
            spec.min,
            spec.max,
            slider_disabled,
            spec.target,
            scheme,
            cx,
        ))
}

fn settings_video_filter_range_slider(
    value: i32,
    min: i32,
    max: i32,
    disabled: bool,
    target: VideoFilterRangeTarget,
    scheme: &ColorScheme,
    cx: &Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    let drag = VideoFilterRangeDrag { target, min, max };
    let owner = cx.entity();
    let decrement_owner = owner.clone();

    m3::Slider::new(
        video_slider_id(target),
        video_slider_label(target),
        f64::from(value),
        f64::from(min),
        f64::from(max),
    )
    .enabled(!disabled)
    .build(scheme)
    .on_a11y_action(gpui::AccessibleAction::Increment, move |_, _window, cx| {
        if disabled {
            return;
        }
        owner.update(cx, move |root, cx| {
            if let Some(next) = signed_range_value_for_key(value, min, max, "right")
                && root.update_selected_config(|config| {
                    apply_video_filter_range(config, target, true, next)
                })
            {
                root.defer_filter_preview_reconfigure(cx);
                cx.notify();
            }
        });
    })
    .on_a11y_action(gpui::AccessibleAction::Decrement, move |_, _window, cx| {
        if disabled {
            return;
        }
        decrement_owner.update(cx, move |root, cx| {
            if let Some(next) = signed_range_value_for_key(value, min, max, "left")
                && root.update_selected_config(|config| {
                    apply_video_filter_range(config, target, true, next)
                })
            {
                root.defer_filter_preview_reconfigure(cx);
                cx.notify();
            }
        });
    })
    .when(!disabled, |slider| {
        slider.on_drag(drag, |_drag, _position, _window, cx| {
            cx.new(|_| VideoFilterRangeDragPreview)
        })
    })
    .on_drag_move(cx.listener(
        move |root, event: &DragMoveEvent<VideoFilterRangeDrag>, _window, cx| {
            if disabled {
                return;
            }
            let drag = *event.drag(cx);
            let fraction = timeline_slider_percent_from_bounds(event.event.position, event.bounds);
            let value = signed_range_value_from_fraction(fraction, drag.min, drag.max);
            if root.update_selected_config(|config| {
                apply_video_filter_range(config, drag.target, true, value)
            }) {
                root.defer_filter_preview_reconfigure(cx);
                cx.notify();
            }
        },
    ))
    .on_key_down(
        cx.listener(move |root, event: &gpui::KeyDownEvent, _window, cx| {
            if disabled {
                return;
            }
            let Some(value) =
                signed_range_value_for_key(value, min, max, event.keystroke.key.as_str())
            else {
                return;
            };
            if root.update_selected_config(|config| {
                apply_video_filter_range(config, target, true, value)
            }) {
                root.defer_filter_preview_reconfigure(cx);
                cx.notify();
            }
            cx.stop_propagation();
        }),
    )
}

fn settings_video_filter_reset(
    target: VideoFilterRangeTarget,
    default_value: i32,
    disabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    m3::IconButton::new(
        format!("settings-video-filter-{}-reset", video_target_id(target)),
        Symbol::RestartAlt,
        "Reset filter",
    )
    .enabled(!disabled)
    .build(scheme, window, cx)
    .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
        if disabled {
            return;
        }
        if root.update_selected_config(|config| {
            apply_video_filter_range(config, target, false, default_value)
        }) {
            cx.notify();
        }
    }))
}

fn settings_video_denoise_control(
    enabled: bool,
    strength: FilterStrength,
    disabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let strengths = [
        FilterStrength::Low,
        FilterStrength::Medium,
        FilterStrength::High,
    ];
    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(spacing::SPACE_2))
        .child(filter_checkbox(
            "settings-video-denoise-toggle",
            "Denoise",
            "",
            enabled,
            disabled,
            scheme,
            window,
            cx,
            move |root, _, _, cx| {
                if !disabled
                    && root.update_selected_config(|config| {
                        apply_video_denoise(config, !enabled, strength)
                    })
                {
                    cx.notify();
                }
            },
        ))
        .child(
            m3::SegmentedButton::new(
                "settings-video-denoise",
                "Denoise strength",
                ["Low", "Medium", "High"],
            )
            .selected(
                strengths
                    .iter()
                    .position(|candidate| *candidate == strength),
            )
            .enabled(!disabled && enabled)
            .on_select(cx.listener(move |root, index: &usize, _, cx| {
                if !disabled
                    && enabled
                    && root.update_selected_config(|config| {
                        apply_video_denoise(config, true, strengths[*index])
                    })
                {
                    cx.notify();
                }
            }))
            .build(scheme, window, cx),
        )
}

fn settings_video_grayscale_control(
    checked: bool,
    disabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    filter_checkbox(
        "settings-video-grayscale-toggle",
        "Grayscale",
        "",
        checked,
        disabled,
        scheme,
        window,
        cx,
        move |root, _event, _window, cx| {
            if disabled {
                return;
            }
            if root.update_selected_config(|config| apply_video_grayscale(config, !checked)) {
                cx.notify();
            }
        },
    )
}

fn settings_video_deinterlace_control(
    mode: DeinterlaceMode,
    disabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let modes = [
        DeinterlaceMode::Off,
        DeinterlaceMode::Auto,
        DeinterlaceMode::On,
    ];
    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(spacing::SPACE_2))
        .child("Deinterlace")
        .child(
            m3::SegmentedButton::new(
                "settings-video-deinterlace",
                "Deinterlace",
                ["Off", "Auto", "On"],
            )
            .selected(modes.iter().position(|candidate| *candidate == mode))
            .enabled(!disabled)
            .on_select(cx.listener(move |root, index: &usize, _, cx| {
                if !disabled
                    && root.update_selected_config(|config| {
                        apply_video_deinterlace(config, modes[*index])
                    })
                {
                    cx.notify();
                }
            }))
            .build(scheme, window, cx),
        )
}

fn settings_video_filters_reset_all(
    disabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    m3::Button::new(
        "settings-video-filters-reset-all",
        "Reset all",
        m3::ButtonVariant::Text,
    )
    .icon(Symbol::RestartAlt)
    .enabled(!disabled)
    .build(scheme, window, cx)
    .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
        if disabled {
            return;
        }
        if root.update_selected_config(reset_video_filters) {
            cx.notify();
        }
    }))
}

fn apply_video_filter_range(
    config: &mut ConversionConfig,
    target: VideoFilterRangeTarget,
    enabled: bool,
    value: i32,
) -> bool {
    let filter = match target {
        VideoFilterRangeTarget::Brightness => VideoScalarFilter::Brightness,
        VideoFilterRangeTarget::Contrast => VideoScalarFilter::Contrast,
        VideoFilterRangeTarget::Saturation => VideoScalarFilter::Saturation,
        VideoFilterRangeTarget::Gamma => VideoScalarFilter::Gamma,
        VideoFilterRangeTarget::Hue => VideoScalarFilter::Hue,
        VideoFilterRangeTarget::Temperature => VideoScalarFilter::Temperature,
        VideoFilterRangeTarget::Sharpen => VideoScalarFilter::Sharpen,
        VideoFilterRangeTarget::GaussianBlur => VideoScalarFilter::GaussianBlur,
        VideoFilterRangeTarget::Deband => VideoScalarFilter::Deband,
        VideoFilterRangeTarget::Vignette => VideoScalarFilter::Vignette,
    };
    apply_video_scalar_filter(config, filter, enabled, value)
}

fn signed_range_value_from_fraction(fraction: f64, min: i32, max: i32) -> i32 {
    if max <= min {
        return min;
    }
    let span = f64::from(max - min);
    let value = fraction
        .clamp(0.0, 1.0)
        .mul_add(span, f64::from(min))
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Value is rounded and clamped into i32 range before casting."
    )]
    {
        value as i32
    }
}

fn signed_range_value_for_key(value: i32, min: i32, max: i32, key: &str) -> Option<i32> {
    let offset = u32::try_from(value.clamp(min, max) - min).ok()?;
    let span = u32::try_from(max - min).ok()?;
    let next = range_value_for_key(offset, 0, span, key)?;
    i32::try_from(next).ok().map(|next| next + min)
}

const fn video_target_id(target: VideoFilterRangeTarget) -> &'static str {
    match target {
        VideoFilterRangeTarget::Brightness => "brightness",
        VideoFilterRangeTarget::Contrast => "contrast",
        VideoFilterRangeTarget::Saturation => "saturation",
        VideoFilterRangeTarget::Gamma => "gamma",
        VideoFilterRangeTarget::Hue => "hue",
        VideoFilterRangeTarget::Temperature => "temperature",
        VideoFilterRangeTarget::Sharpen => "sharpen",
        VideoFilterRangeTarget::GaussianBlur => "gaussian-blur",
        VideoFilterRangeTarget::Deband => "deband",
        VideoFilterRangeTarget::Vignette => "vignette",
    }
}

const fn video_slider_id(target: VideoFilterRangeTarget) -> &'static str {
    match target {
        VideoFilterRangeTarget::Brightness => "settings-video-filter-brightness-slider",
        VideoFilterRangeTarget::Contrast => "settings-video-filter-contrast-slider",
        VideoFilterRangeTarget::Saturation => "settings-video-filter-saturation-slider",
        VideoFilterRangeTarget::Gamma => "settings-video-filter-gamma-slider",
        VideoFilterRangeTarget::Hue => "settings-video-filter-hue-slider",
        VideoFilterRangeTarget::Temperature => "settings-video-filter-temperature-slider",
        VideoFilterRangeTarget::Sharpen => "settings-video-filter-sharpen-slider",
        VideoFilterRangeTarget::GaussianBlur => "settings-video-filter-gaussian-blur-slider",
        VideoFilterRangeTarget::Deband => "settings-video-filter-deband-slider",
        VideoFilterRangeTarget::Vignette => "settings-video-filter-vignette-slider",
    }
}

const fn video_slider_label(target: VideoFilterRangeTarget) -> &'static str {
    match target {
        VideoFilterRangeTarget::Brightness => "Brightness",
        VideoFilterRangeTarget::Contrast => "Contrast",
        VideoFilterRangeTarget::Saturation => "Saturation",
        VideoFilterRangeTarget::Gamma => "Gamma",
        VideoFilterRangeTarget::Hue => "Hue",
        VideoFilterRangeTarget::Temperature => "Temperature",
        VideoFilterRangeTarget::Sharpen => "Sharpen",
        VideoFilterRangeTarget::GaussianBlur => "Gaussian blur",
        VideoFilterRangeTarget::Deband => "Deband",
        VideoFilterRangeTarget::Vignette => "Vignette",
    }
}
