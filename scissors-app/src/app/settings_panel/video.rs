use super::{
    ClickEvent, Context, ConversionConfig, FocusHandle, ParentElement, ScissorsRoot,
    ScissorsTextInputKind, ScissorsTextInputSpec, StatefulInteractiveElement, Styled, Window,
    apply_resolution, apply_scaling_algorithm, div, resolution_options, scaling_algorithm_options,
    scissors_choice_button, scissors_text_input, settings_field_label, settings_section, theme,
};

pub(in crate::app) fn settings_video_resolution_section(
    config: &ConversionConfig,
    settings_disabled: bool,
    video_width_focus: Option<&FocusHandle>,
    video_height_focus: Option<&FocusHandle>,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let mut section = settings_section("Resolution & framerate", palette).child(
        settings_resolution_grid(config, settings_disabled, palette, window, cx),
    );

    if config.resolution == "custom" {
        section = section.child(settings_custom_dimensions_grid(
            config,
            settings_disabled,
            video_width_focus,
            video_height_focus,
            palette,
            window,
            cx,
        ));
    }

    section
}

pub(in crate::app) fn settings_resolution_grid(
    config: &ConversionConfig,
    disabled: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let mut grid = div().grid().grid_cols(2).gap_2();
    for resolution in resolution_options() {
        let selected = config.resolution == *resolution;
        let label = resolution_label(resolution);
        grid = grid.child(
            scissors_choice_button(
                format!("video-resolution-{resolution}"),
                label,
                selected,
                !disabled,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                cx.stop_propagation();
                if disabled {
                    return;
                }
                if root.update_selected_config(|config| apply_resolution(config, resolution)) {
                    cx.notify();
                }
            })),
        );
    }
    grid
}

fn settings_custom_dimensions_grid(
    config: &ConversionConfig,
    disabled: bool,
    video_width_focus: Option<&FocusHandle>,
    video_height_focus: Option<&FocusHandle>,
    palette: &'static theme::ThemePalette,
    window: &Window,
    cx: &Context<ScissorsRoot>,
) -> gpui::Div {
    div()
        .grid()
        .grid_cols(2)
        .gap_2()
        .pt(theme::ui_rem(4.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(settings_field_label("Width", palette))
                .child(scissors_text_input(
                    ScissorsTextInputSpec {
                        id: "settings-video-width-field",
                        value: config.custom_width.as_deref().unwrap_or_default(),
                        placeholder: "1920",
                        disabled,
                        focus: video_width_focus,
                        kind: ScissorsTextInputKind::VideoCustomWidth,
                    },
                    palette,
                    window,
                    cx,
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(settings_field_label("Height", palette))
                .child(scissors_text_input(
                    ScissorsTextInputSpec {
                        id: "settings-video-height-field",
                        value: config.custom_height.as_deref().unwrap_or_default(),
                        placeholder: "1080",
                        disabled,
                        focus: video_height_focus,
                        kind: ScissorsTextInputKind::VideoCustomHeight,
                    },
                    palette,
                    window,
                    cx,
                )),
        )
}

pub(in crate::app) fn settings_video_scaling_section(
    config: &ConversionConfig,
    settings_disabled: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<ScissorsRoot>,
) -> gpui::Div {
    let disabled = settings_disabled || config.resolution == "original";
    let mut grid = div().grid().grid_cols(2).gap_2();
    for algorithm in scaling_algorithm_options() {
        grid = grid.child(
            scissors_choice_button(
                format!("video-scaling-{algorithm}"),
                scaling_algorithm_label(algorithm),
                config.scaling_algorithm == *algorithm,
                !disabled,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                cx.stop_propagation();
                if disabled {
                    return;
                }
                if root.update_selected_config(|config| apply_scaling_algorithm(config, algorithm))
                {
                    cx.notify();
                }
            })),
        );
    }

    settings_section("Scaling algorithm", palette).child(grid)
}

fn resolution_label(resolution: &str) -> &'static str {
    match resolution {
        "custom" => "Custom",
        "1080p" => "1080p",
        "720p" => "720p",
        "480p" => "480p",
        _ => "Original",
    }
}

fn scaling_algorithm_label(algorithm: &str) -> &'static str {
    match algorithm {
        "lanczos" => "Lanczos",
        "bilinear" => "Bilinear",
        "nearest" => "Nearest",
        _ => "Bicubic",
    }
}
