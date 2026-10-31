use super::accessibility::handle_modal_tab_navigation;
use super::m3::{self, M3Text};
use super::primitives::{card_surface_shadows, color};
use super::{
    AppearanceSettings, ClickEvent, ColorTheme, Context, ExternalPaths, FRAME_APP_VERSION,
    FluentBuilder, FocusHandle, FrameRoot, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SURFACE_MOTION_DURATION, ScalePreset, ScrollHandle, StatefulInteractiveElement,
    Styled, Window, WindowControlArea, div, ease_in_out, motion_is_hidden, motion_target, px,
    relative, set_motion_target, settings_sheet_right_inset, theme,
};
use crate::material::{ColorScheme, Symbol, typography};

pub(super) fn titlebar_drag_surface(cx: &Context<FrameRoot>) -> gpui::Div {
    div()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down_out(cx.listener(|root, _event, _window, _cx| {
            root.titlebar_drag.should_move = false;
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|root, _event, _window, _cx| {
                root.titlebar_drag.should_move = false;
            }),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|root, _event, _window, _cx| {
                root.titlebar_drag.should_move = true;
            }),
        )
        .on_mouse_move(cx.listener(|root, _event, window, _cx| {
            if root.titlebar_drag.should_move {
                root.titlebar_drag.should_move = false;
                window.start_window_move();
            }
        }))
}

#[derive(Clone, Copy)]
pub(super) struct AppSettingsSheetProps<'a> {
    pub(super) is_open: bool,
    pub(super) default_output_directory: Option<&'a str>,
    pub(super) output_directory_error: Option<&'a str>,
    pub(super) appearance: AppearanceSettings,
    pub(super) accent_preset_focuses: &'a [FocusHandle],
    pub(super) appearance_error: Option<&'a str>,
    pub(super) scroll_handle: &'a ScrollHandle,
    pub(super) theme_focuses: &'a [FocusHandle],
    pub(super) ui_scale_focuses: &'a [FocusHandle],
    pub(super) show_fps_meter: bool,
    pub(super) output_directory_focus: &'a FocusHandle,
    pub(super) fps_meter_focus: &'a FocusHandle,
    pub(super) panel_focus: &'a FocusHandle,
    pub(super) close_focus: &'a FocusHandle,
    pub(super) last_focus: &'a FocusHandle,
}

pub(super) fn app_settings_sheet(
    props: AppSettingsSheetProps<'_>,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> impl IntoElement {
    let scheme = &m3::scheme(props.appearance.color_theme, cx);
    let transition = window
        .use_keyed_transition(
            "app-settings-sheet-motion",
            cx,
            crate::material::motion::SHORT_4,
            |_window, _cx| 0.0_f32,
        )
        .with_easing(|progress| crate::material::motion::STANDARD.ease(progress));
    set_motion_target(&transition, motion_target(props.is_open), cx);
    let progress = *transition.evaluate(window, cx);
    let first_focus = props.close_focus.clone();
    let last_focus = props.last_focus.clone();
    if !props.is_open && motion_is_hidden(progress) {
        cx.defer_in(window, |root, _window, cx| {
            if root.finish_app_settings_close() {
                cx.notify();
            }
        });
    }
    div().id("app-settings-sheet").absolute().inset_0()
        .on_key_down(cx.listener(move |root, event: &gpui::KeyDownEvent, window, cx| {
            match event.keystroke.key.as_str() {
                "escape" => {
                    root.close_app_settings();
                    root.restore_focus_after_settings_close(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
                "tab" => { handle_modal_tab_navigation(event, &first_focus, &last_focus, window, cx); }
                _ => {}
            }
        }))
        .child(div().id("app-settings-backdrop").absolute().inset_0()
            .bg(color(scheme.scrim.with_alpha(0.60 * progress)))
            .occlude().on_click(cx.listener(|root, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                root.close_app_settings();
                root.restore_focus_after_settings_close(window, cx);
                cx.notify();
            })))
        .child(div().id("app-settings-panel").role(gpui::Role::Dialog).aria_label("Settings")
            .track_focus(props.panel_focus).tab_stop(false).absolute()
            .top_0().right(theme::ui_rem(settings_sheet_right_inset(progress))).bottom_0()
            .w(theme::ui_rem(384.0)).max_w(relative(0.95))
            .flex().flex_col().bg(color(scheme.surface)).text_color(color(scheme.on_surface))
            .m3_type(typography::BODY_LARGE).opacity(progress).occlude()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(div().flex().flex_none().items_center().justify_between()
                .h(theme::ui_rem(56.0)).px(theme::ui_rem(16.0))
                .child(div().m3_type(typography::TITLE_LARGE).child("Settings"))
                .child(m3::IconButton::new("app-settings-close", Symbol::Close, "Close settings")
                    .focus(props.close_focus).build(scheme, window, cx)
                    .on_click(cx.listener(|root, _, window, cx| {
                        root.close_app_settings();
                        root.restore_focus_after_settings_close(window, cx);
                        cx.notify();
                    }))))
            .child(div().id("app-settings-scroll-body").flex_1().min_h_0()
                .overflow_y_scroll().track_scroll(props.scroll_handle)
                .flex().flex_col().px(theme::ui_rem(16.0)).pb(theme::ui_rem(24.0))
                .child(app_settings_output_section(&props, scheme, window, cx))
                .child(app_settings_appearance_section(&props, scheme, window, cx))
                .child(settings_group("Diagnostics", scheme)
                    .child(div().flex().items_center().gap(theme::ui_rem(16.0))
                        .child(settings_copy("Show FPS meter", "Shows UI frame rate and frame times. Runs continuously while on.", scheme))
                        .child(m3::Switch::new("app-settings-fps-meter", "Show FPS meter", props.show_fps_meter)
                            .focus(props.fps_meter_focus).build(scheme, window, cx)
                            .on_click(cx.listener(|root, _, window, cx| {
                                root.toggle_fps_meter(&super::ToggleFpsMeter, window, cx);
                            })))))
                .child(div().pt(theme::ui_rem(24.0)).m3_type(typography::BODY_SMALL)
                    .text_color(color(scheme.on_surface_variant))
                    .child(format!("Frame v{FRAME_APP_VERSION}")))))
}

fn settings_group(label: &'static str, scheme: &ColorScheme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(16.0))
        .py(theme::ui_rem(24.0))
        .border_b_1()
        .border_color(color(scheme.outline_variant))
        .child(
            div()
                .m3_type(typography::LABEL_LARGE)
                .text_color(color(scheme.primary))
                .child(label),
        )
}

fn settings_copy(
    title: impl Into<gpui::SharedString>,
    detail: &'static str,
    scheme: &ColorScheme,
) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(4.0))
        .child(
            div()
                .m3_type(typography::BODY_LARGE)
                .overflow_hidden()
                .text_ellipsis()
                .child(title.into()),
        )
        .child(
            div()
                .m3_type(typography::BODY_MEDIUM)
                .text_color(color(scheme.on_surface_variant))
                .child(detail),
        )
}

fn settings_error(
    id: &'static str,
    message: &str,
    scheme: &ColorScheme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .role(gpui::Role::Alert)
        .aria_label(message.to_string())
        .m3_type(typography::BODY_SMALL)
        .text_color(color(scheme.error))
        .child(message.to_string())
}

fn app_settings_output_section(
    props: &AppSettingsSheetProps<'_>,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    settings_group("Output", scheme)
        .pt(theme::ui_rem(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(theme::ui_rem(16.0))
                .child(m3::m3_icon(
                    Symbol::FolderOpen,
                    false,
                    24.0,
                    color(scheme.on_surface_variant),
                ))
                .child(settings_copy(
                    props
                        .default_output_directory
                        .unwrap_or("No folder selected")
                        .to_string(),
                    "Default output folder",
                    scheme,
                ))
                .child(
                    m3::Button::new(
                        "app-settings-output-directory",
                        "Change",
                        m3::ButtonVariant::Outlined,
                    )
                    .focus(props.output_directory_focus)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(|_, _, window, cx| {
                        FrameRoot::prompt_default_output_folder(window, cx);
                    })),
                ),
        )
        .when_some(props.output_directory_error, |this, error| {
            this.child(settings_error(
                "app-settings-output-directory-error",
                error,
                scheme,
            ))
        })
}

fn app_settings_appearance_section(
    props: &AppSettingsSheetProps<'_>,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let themes = [ColorTheme::Light, ColorTheme::Dark];
    let scales = [
        ScalePreset::Percent90,
        ScalePreset::Percent100,
        ScalePreset::Percent110,
        ScalePreset::Percent125,
    ];
    let mut section = settings_group("Appearance", scheme)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(8.0))
                .child(div().m3_type(typography::LABEL_LARGE).child("Theme"))
                .child(
                    m3::SegmentedButton::new(
                        "app-settings-theme",
                        "Theme",
                        ["Light", "Dark", "System"],
                    )
                    .selected(if props.appearance.follow_system_theme {
                        Some(2)
                    } else {
                        themes
                            .iter()
                            .position(|theme| *theme == props.appearance.color_theme)
                    })
                    .focus(props.theme_focuses)
                    .on_select(cx.listener(move |root, index: &usize, window, cx| {
                        if *index == 2 {
                            root.set_system_theme(window);
                        } else {
                            root.set_color_theme(themes[*index]);
                        }
                        cx.notify();
                    }))
                    .build(scheme, window, cx),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme::ui_rem(8.0))
                .child(div().m3_type(typography::LABEL_LARGE).child("UI scale"))
                .child(
                    m3::SegmentedButton::new(
                        "app-settings-ui-scale",
                        "UI scale",
                        scales.map(ScalePreset::display),
                    )
                    .selected(
                        scales
                            .iter()
                            .position(|scale| *scale == props.appearance.ui_scale),
                    )
                    .focus(props.ui_scale_focuses)
                    .on_select(cx.listener(move |root, index: &usize, window, cx| {
                        if root.set_ui_scale(scales[*index]) {
                            window.set_rem_size(px(
                                crate::appearance::BASE_REM_PX * scales[*index].factor()
                            ));
                        }
                        cx.notify();
                    }))
                    .build(scheme, window, cx),
                ),
        )
        .child(app_settings_accent_color(props, scheme, window, cx));
    if let Some(error) = props.appearance_error {
        section = section.child(settings_error(
            "app-settings-appearance-error",
            error,
            scheme,
        ));
    }
    section
}

fn app_settings_accent_color(
    props: &AppSettingsSheetProps<'_>,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    use crate::appearance::AccentColor;
    let previews = crate::material::accent_preset_schemes(props.appearance.color_theme);
    let label = AccentColor::PRESETS
        .iter()
        .find(|(_, accent)| *accent == props.appearance.accent_color)
        .map_or("Custom", |(label, _)| *label);
    div()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(12.0))
        .child(
            div()
                .flex()
                .justify_between()
                .m3_type(typography::LABEL_LARGE)
                .child("Accent color")
                .child(label),
        )
        .child(
            div()
                .id("app-settings-accent-options")
                .role(gpui::Role::RadioGroup)
                .aria_label("Accent color")
                .flex()
                .justify_between()
                .children(AccentColor::PRESETS.iter().enumerate().map(
                    |(index, &(label, accent))| {
                        m3::ColorSwatch::new(
                            format!("app-settings-accent-{index}"),
                            label,
                            &previews[index],
                        )
                        .selected(props.appearance.accent_color == accent)
                        .focus(&props.accent_preset_focuses[index])
                        .build(scheme, window, cx)
                        .on_click(cx.listener(move |root, _, _, cx| {
                            root.set_accent_color(accent);
                            cx.notify();
                        }))
                    },
                )),
        )
}

pub(super) fn drag_drop_overlay(
    is_open: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> impl IntoElement {
    let transition = window
        .use_keyed_transition(
            "drag-drop-overlay-motion",
            cx,
            SURFACE_MOTION_DURATION,
            |_window, _cx| 0.0_f32,
        )
        .with_easing(ease_in_out);
    set_motion_target(&transition, motion_target(is_open), cx);
    let progress = *transition.evaluate(window, cx);

    if !is_open && motion_is_hidden(progress) {
        cx.defer_in(window, |root, _window, cx| {
            if root.finish_drag_drop_overlay_close() {
                cx.notify();
            }
        });
    }

    div()
        .id("drag-drop-overlay")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .bg(color(palette.canvas.with_alpha(0.60 * progress)))
        .backdrop_blur(theme::ui_rem(4.0 * progress).to_pixels(window.rem_size()))
        .opacity(progress)
        .occlude()
        .on_drop(cx.listener(|root, paths: &ExternalPaths, _window, cx| {
            cx.stop_propagation();
            root.close_drag_drop_overlay();
            FrameRoot::import_source_paths(paths.paths().to_vec(), cx);
            cx.notify();
        }))
        .child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::ui_rem(theme::RADIUS_LG))
                .border_1()
                .border_dashed()
                .border_color(color(palette.fill_subtle))
                .bg(color(palette.fill_subtle))
                .shadow(card_surface_shadows(palette))
                .child(
                    div()
                        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                        .text_color(color(palette.text_primary))
                        .child(theme::ui_text("Import source files")),
                ),
        )
}
