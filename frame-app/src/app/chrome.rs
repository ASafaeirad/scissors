use super::accessibility::{apply_accessible_button_with_focus, handle_modal_tab_navigation};
use super::components::{frame_text_button, frame_vertical_scrollbar};
use super::m3::{self, M3Text};
use super::primitives::{
    ButtonVariant, action_button, animated_button_colors, apply_button_motion, button_colors,
    card_surface_shadows, color, icon_svg, panel_bottom_separator,
};
use super::{
    AppearanceSettings, ClickEvent, ColorTheme, Context, ExternalPaths, FILE_LIST_ACTION_ICON_SIZE,
    FRAME_APP_VERSION, FluentBuilder, FocusHandle, FrameRoot, InteractiveElement, IntoElement,
    MouseButton, PANEL_HEADER_HEIGHT, ParentElement, SETTINGS_CONTROL_HEIGHT,
    SURFACE_MOTION_DURATION, ScalePreset, ScrollHandle, StatefulInteractiveElement, Styled,
    UPDATE_INSTALL_WAIT_MESSAGE, UpdateInfo, UpdateStatus, Window, WindowControlArea, assets, div,
    ease_in_out, format_total_size, motion_is_hidden, motion_target, px, relative,
    set_motion_target, settings_sheet_right_inset, theme,
};
use crate::material::{ColorScheme, Symbol, shape, typography};
use gpui::{HighlightStyle, StyledText};

const MAX_RELEASE_NOTES_CHARS: usize = 8_000;
const UPDATE_RELEASE_NOTES_MAX_HEIGHT: f32 = 360.0;
const UPDATE_RELEASE_NOTES_MIN_HEIGHT: f32 = 180.0;
const UPDATE_RELEASE_NOTES_PADDING_Y: f32 = 24.0;
const UPDATE_RELEASE_NOTES_LINE_HEIGHT: f32 = 16.0;
const UPDATE_RELEASE_NOTES_LINE_PADDING_BOTTOM: f32 = 4.0;
const UPDATE_RELEASE_NOTES_BLANK_LINE_HEIGHT: f32 = 8.0;

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
#[expect(
    clippy::struct_excessive_bools,
    reason = "Settings visibility, switches and installation readiness are independent states."
)]
pub(super) struct AppSettingsSheetProps<'a> {
    pub(super) is_open: bool,
    pub(super) current_max_concurrency: usize,
    pub(super) error: Option<&'a str>,
    pub(super) default_output_directory: Option<&'a str>,
    pub(super) output_directory_error: Option<&'a str>,
    pub(super) appearance: AppearanceSettings,
    pub(super) accent_preset_focuses: &'a [FocusHandle],
    pub(super) appearance_error: Option<&'a str>,
    pub(super) scroll_handle: &'a ScrollHandle,
    pub(super) theme_focuses: &'a [FocusHandle],
    pub(super) ui_scale_focuses: &'a [FocusHandle],
    pub(super) decrease_focus: &'a FocusHandle,
    pub(super) increase_focus: &'a FocusHandle,
    pub(super) auto_update_check: bool,
    pub(super) show_fps_meter: bool,
    pub(super) update_status: &'a UpdateStatus,
    pub(super) update_install_ready: bool,
    pub(super) output_directory_focus: &'a FocusHandle,
    pub(super) auto_update_focus: &'a FocusHandle,
    pub(super) fps_meter_focus: &'a FocusHandle,
    pub(super) check_now_focus: &'a FocusHandle,
    pub(super) download_focus: &'a FocusHandle,
    pub(super) skip_focus: &'a FocusHandle,
    pub(super) install_focus: &'a FocusHandle,
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
                .child(app_settings_updates_section(
                    props.auto_update_check, props.update_status, props.update_install_ready,
                    AppSettingsUpdateFocuses { auto_update: props.auto_update_focus,
                        check_now: props.check_now_focus, download: props.download_focus,
                        skip: props.skip_focus, install: props.install_focus },
                    scheme, window, cx,
                ))
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

#[expect(
    clippy::too_many_lines,
    reason = "Output rows keep their labels and controls together."
)]
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
        .child(
            div()
                .flex()
                .items_center()
                .gap(theme::ui_rem(16.0))
                .child(m3::m3_icon(
                    Symbol::Layers,
                    false,
                    24.0,
                    color(scheme.on_surface_variant),
                ))
                .child(settings_copy(
                    "Max concurrency",
                    "Conversions that run at the same time",
                    scheme,
                ))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .rounded(theme::ui_rem(shape::CORNER_FULL))
                        .bg(color(scheme.surface_container_highest))
                        .text_color(color(scheme.on_surface))
                        .child(
                            m3::IconButton::new(
                                "app-settings-concurrency-decrease",
                                Symbol::Remove,
                                "Decrease max concurrency",
                            )
                            .enabled(props.current_max_concurrency > 1)
                            .focus(props.decrease_focus)
                            .build(scheme, window, cx)
                            .on_click(cx.listener(|root, _, _, cx| {
                                root.step_max_concurrency(false);
                                cx.notify();
                            })),
                        )
                        .child(
                            div()
                                .min_w(theme::ui_rem(24.0))
                                .text_center()
                                .m3_type(typography::LABEL_LARGE)
                                .child(props.current_max_concurrency.to_string()),
                        )
                        .child(
                            m3::IconButton::new(
                                "app-settings-concurrency-increase",
                                Symbol::Add,
                                "Increase max concurrency",
                            )
                            .enabled(props.current_max_concurrency < usize::MAX)
                            .focus(props.increase_focus)
                            .build(scheme, window, cx)
                            .on_click(cx.listener(|root, _, _, cx| {
                                root.step_max_concurrency(true);
                                cx.notify();
                            })),
                        ),
                ),
        )
        .when_some(props.error, |this, error| {
            this.child(settings_error(
                "app-settings-max-concurrency-error",
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

#[derive(Clone, Copy)]
struct AppSettingsUpdateFocuses<'a> {
    auto_update: &'a FocusHandle,
    check_now: &'a FocusHandle,
    download: &'a FocusHandle,
    skip: &'a FocusHandle,
    install: &'a FocusHandle,
}

#[expect(
    clippy::too_many_lines,
    reason = "The updates section handles each update state and its available actions."
)]
fn app_settings_updates_section(
    auto_update_check: bool,
    update_status: &UpdateStatus,
    update_install_ready: bool,
    focuses: AppSettingsUpdateFocuses<'_>,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let mut section = settings_group("Updates", scheme)
        .child(
            div()
                .flex()
                .items_center()
                .gap(theme::ui_rem(16.0))
                .child(settings_copy(
                    "Check automatically",
                    "Check for signed releases in the background.",
                    scheme,
                ))
                .child(
                    m3::Switch::new(
                        "app-settings-auto-update-check",
                        "Check automatically",
                        auto_update_check,
                    )
                    .focus(focuses.auto_update)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(|root, _, _, cx| {
                        if root.toggle_auto_update_check(cx) {
                            cx.notify();
                        }
                    })),
                ),
        )
        .child(
            m3::Button::new(
                "app-settings-update-check-now",
                "Check now",
                m3::ButtonVariant::Outlined,
            )
            .enabled(!update_status.is_busy())
            .focus(focuses.check_now)
            .build(scheme, window, cx)
            .on_click(cx.listener(|root, _, _, cx| {
                root.check_for_updates(true, cx);
                cx.notify();
            })),
        )
        .child(
            div()
                .id("app-settings-update-status")
                .role(gpui::Role::Status)
                .m3_type(typography::BODY_MEDIUM)
                .text_color(color(scheme.on_surface_variant))
                .child(update_status_text(update_status, update_install_ready)),
        );
    if let UpdateStatus::Downloading {
        progress_percent,
        received_bytes,
        total_bytes,
        ..
    } = update_status
    {
        section = section
            .child(
                m3::LinearProgress::new(
                    "app-settings-update-progress",
                    "Update download progress",
                    progress_percent.map_or(0.0, |percent| f32::from(percent) / 100.0),
                )
                .build(scheme),
            )
            .child(
                div()
                    .m3_type(typography::BODY_SMALL)
                    .child(total_bytes.map_or_else(
                        || format!("{} downloaded", format_total_size(*received_bytes)),
                        |total| {
                            format!(
                                "{} of {}",
                                format_total_size(*received_bytes),
                                format_total_size(total)
                            )
                        },
                    )),
            );
    }
    match update_status {
        UpdateStatus::Available(_) => {
            section = section.child(
                div()
                    .flex()
                    .gap(theme::ui_rem(8.0))
                    .child(
                        m3::Button::new(
                            "app-settings-update-download",
                            "Download",
                            m3::ButtonVariant::Filled,
                        )
                        .focus(focuses.download)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, _, cx| {
                            root.download_available_update(cx);
                            cx.notify();
                        })),
                    )
                    .child(
                        m3::Button::new(
                            "app-settings-update-skip",
                            "Skip",
                            m3::ButtonVariant::Outlined,
                        )
                        .focus(focuses.skip)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|root, _, _, cx| {
                            root.skip_available_update(cx);
                            cx.notify();
                        })),
                    ),
            );
        }
        UpdateStatus::ReadyToInstall(_) => {
            section = section.child(
                m3::Button::new(
                    "app-settings-update-install",
                    "Install and restart",
                    m3::ButtonVariant::Filled,
                )
                .enabled(update_install_ready)
                .focus(focuses.install)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    root.install_downloaded_update(cx);
                    cx.notify();
                })),
            );
        }
        _ => {}
    }
    section
}

fn update_status_text(status: &UpdateStatus, update_install_ready: bool) -> String {
    match status {
        UpdateStatus::Idle => "No update check is running.".to_string(),
        UpdateStatus::Checking => "Checking for updates...".to_string(),
        UpdateStatus::UpToDate => "Frame is up to date.".to_string(),
        UpdateStatus::Available(info) => {
            format!("Frame {} is available.", info.version)
        }
        UpdateStatus::Downloading {
            version,
            progress_percent,
            ..
        } => progress_percent.map_or_else(
            || format!("Downloading Frame {version}..."),
            |percent| format!("Downloading Frame {version}: {percent}%"),
        ),
        UpdateStatus::ReadyToInstall(_) if !update_install_ready => {
            UPDATE_INSTALL_WAIT_MESSAGE.to_string()
        }
        UpdateStatus::ReadyToInstall(package) => {
            format!("Frame {} is ready to install.", package.version)
        }
        UpdateStatus::Installing => "Installing update and restarting...".to_string(),
        UpdateStatus::Disabled(explanation) => explanation.clone(),
        UpdateStatus::Error(error) => error.clone(),
    }
}

fn update_release_notes_text(info: Option<&UpdateInfo>) -> Option<String> {
    let notes = info?.release_notes_markdown.as_deref()?;
    let notes = notes.trim();
    if notes.is_empty() {
        return None;
    }

    let mut text = notes
        .chars()
        .take(MAX_RELEASE_NOTES_CHARS + 1)
        .collect::<String>();
    if text.chars().count() > MAX_RELEASE_NOTES_CHARS {
        text = text.chars().take(MAX_RELEASE_NOTES_CHARS).collect();
        text.push_str("...");
    }
    Some(text)
}

fn update_release_notes_block(
    notes: &str,
    scroll_handle: &ScrollHandle,
    palette: &'static theme::ThemePalette,
) -> gpui::Stateful<gpui::Div> {
    let lines = normalized_release_note_lines(notes);
    let content_height = update_release_notes_content_height(&lines);
    let mut content = div()
        .id("update-dialog-release-notes-content")
        .min_h(theme::ui_rem(UPDATE_RELEASE_NOTES_MIN_HEIGHT))
        .max_h(theme::ui_rem(UPDATE_RELEASE_NOTES_MAX_HEIGHT))
        .overflow_y_scroll()
        .track_scroll(scroll_handle)
        .p_3()
        .pr_5();

    for line in lines {
        content = content.child(update_release_note_line(&line, palette));
    }

    div()
        .id("update-dialog-release-notes")
        .relative()
        .min_h(theme::ui_rem(UPDATE_RELEASE_NOTES_MIN_HEIGHT))
        .max_h(theme::ui_rem(UPDATE_RELEASE_NOTES_MAX_HEIGHT))
        .overflow_hidden()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .bg(color(palette.fill_subtle))
        .child(content)
        .child(frame_vertical_scrollbar(
            "update-dialog-release-notes-scrollbar",
            scroll_handle.clone(),
            content_height,
            palette,
        ))
}

fn update_release_notes_content_height(lines: &[String]) -> f32 {
    UPDATE_RELEASE_NOTES_PADDING_Y
        + lines.iter().fold(0.0, |height, line| {
            if line.trim().is_empty() {
                height + UPDATE_RELEASE_NOTES_BLANK_LINE_HEIGHT
            } else {
                height + UPDATE_RELEASE_NOTES_LINE_HEIGHT + UPDATE_RELEASE_NOTES_LINE_PADDING_BOTTOM
            }
        })
}

fn normalized_release_note_lines(notes: &str) -> Vec<String> {
    let mut lines = notes
        .lines()
        .map(str::trim_end)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        vec!["No release notes were published for this version.".to_string()]
    } else {
        lines
    }
}

fn update_release_note_line(line: &str, palette: &'static theme::ThemePalette) -> gpui::Div {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return div().h(theme::ui_rem(8.0));
    }

    let heading = trimmed.trim_start_matches('#').trim();
    let bullet = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("• "));
    let (text, left_padding, text_color, font_weight) =
        if !heading.is_empty() && trimmed.starts_with('#') {
            (
                heading.to_string(),
                0.0,
                palette.text_primary,
                theme::TEXT_WEIGHT_MEDIUM,
            )
        } else if let Some(bullet) = bullet {
            (
                format!("• {bullet}"),
                8.0,
                palette.text_muted,
                theme::TEXT_WEIGHT_REGULAR,
            )
        } else {
            (
                trimmed.to_string(),
                0.0,
                palette.text_muted,
                theme::TEXT_WEIGHT_REGULAR,
            )
        };
    let (text, highlights) = parse_update_release_note_emphasis(&text, palette);

    let mut line = div()
        .pl(theme::ui_rem(left_padding))
        .pb(theme::ui_rem(4.0))
        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
        .line_height(theme::ui_rem(16.0))
        .text_color(color(text_color))
        .font_weight(font_weight);

    if highlights.is_empty() {
        line = line.child(text);
    } else {
        line = line.child(StyledText::new(text).with_highlights(highlights));
    }

    line
}

fn parse_update_release_note_emphasis(
    input: &str,
    palette: &'static theme::ThemePalette,
) -> (String, Vec<(std::ops::Range<usize>, HighlightStyle)>) {
    let mut text = String::with_capacity(input.len());
    let mut highlights = Vec::new();
    let mut rest = input;
    let highlight_style = HighlightStyle {
        color: Some(color(palette.text_primary).into()),
        font_weight: Some(theme::TEXT_WEIGHT_MEDIUM),
        ..HighlightStyle::default()
    };

    loop {
        let Some(start) = rest.find("**") else {
            text.push_str(rest);
            break;
        };
        text.push_str(&rest[..start]);

        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("**") else {
            text.push_str(&rest[start..]);
            break;
        };

        let highlight_start = text.len();
        text.push_str(&after_start[..end]);
        let highlight_end = text.len();
        if highlight_start < highlight_end {
            highlights.push((highlight_start..highlight_end, highlight_style));
        }
        rest = &after_start[end + 2..];
    }

    (text, highlights)
}

fn update_progress_bar(
    progress_percent: Option<u8>,
    palette: &'static theme::ThemePalette,
) -> gpui::Stateful<gpui::Div> {
    let fraction = progress_percent.map_or(0.0, |percent| f32::from(percent) / 100.0);
    let numeric_percent = progress_percent.map_or(0.0, f64::from);
    let value_text = progress_percent.map_or_else(
        || "Download progress unknown".to_string(),
        |percent| format!("{percent}%"),
    );

    div()
        .id("app-settings-update-progress")
        .role(gpui::Role::ProgressIndicator)
        .aria_label("Update download progress")
        .aria_numeric_value(numeric_percent)
        .aria_min_numeric_value(0.0)
        .aria_max_numeric_value(100.0)
        .aria_value(value_text)
        .h(theme::ui_rem(6.0))
        .w_full()
        .overflow_hidden()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .bg(color(palette.fill_subtle))
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0.0, 1.0)))
                .rounded(theme::ui_rem(theme::RADIUS_SM))
                .bg(color(palette.accent)),
        )
}

fn update_download_detail(
    received_bytes: u64,
    total_bytes: Option<u64>,
    progress_percent: Option<u8>,
    palette: &'static theme::ThemePalette,
) -> gpui::Div {
    let detail = match (total_bytes, progress_percent) {
        (Some(total_bytes), Some(percent)) => format!(
            "{} of {} ({percent}%)",
            format_total_size(received_bytes),
            format_total_size(total_bytes)
        ),
        (Some(total_bytes), None) => format!(
            "{} of {}",
            format_total_size(received_bytes),
            format_total_size(total_bytes)
        ),
        (None, _) => format!("{} downloaded", format_total_size(received_bytes)),
    };

    div()
        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
        .text_color(color(palette.text_muted))
        .font_features(assets::frame_tabular_number_font_features())
        .child(detail)
}

#[derive(Clone, Copy)]
pub(super) struct UpdateDialogView<'a> {
    pub(super) status: &'a UpdateStatus,
    pub(super) info: Option<&'a UpdateInfo>,
    pub(super) install_ready: bool,
    pub(super) release_notes_scroll_handle: &'a ScrollHandle,
    pub(super) panel_focus: &'a FocusHandle,
    pub(super) close_focus: &'a FocusHandle,
    pub(super) palette: &'static theme::ThemePalette,
}

pub(super) fn update_dialog(
    is_open: bool,
    view: UpdateDialogView<'_>,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> impl IntoElement {
    let palette = view.palette;
    let transition = window
        .use_keyed_transition(
            "update-dialog-motion",
            cx,
            SURFACE_MOTION_DURATION,
            |_window, _cx| 0.0_f32,
        )
        .with_easing(ease_in_out);
    set_motion_target(&transition, motion_target(is_open), cx);
    let progress = *transition.evaluate(window, cx);
    let panel_offset = (1.0 - progress.clamp(0.0, 1.0)) * 12.0;

    if !is_open && motion_is_hidden(progress) {
        cx.defer_in(window, |root, _window, cx| {
            if root.finish_update_dialog_close() {
                cx.notify();
            }
        });
    }

    div()
        .id("update-dialog")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .bg(color(palette.canvas.with_alpha(0.64 * progress)))
        .backdrop_blur(theme::ui_rem(4.0 * progress).to_pixels(window.rem_size()))
        .opacity(progress)
        .occlude()
        .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
            match event.keystroke.key.as_str() {
                "escape" if !root.update_ui.status.is_busy() => {
                    root.close_update_dialog();
                    root.restore_focus_after_update_dialog_close(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
                "tab" => {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                }
                _ => {}
            }
        }))
        .on_click(cx.listener(|root, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            if !root.update_ui.status.is_busy() {
                root.close_update_dialog();
                root.restore_focus_after_update_dialog_close(window, cx);
                cx.notify();
            }
        }))
        .child(update_dialog_panel(panel_offset, view, window, cx))
}

fn update_dialog_panel(
    panel_offset: f32,
    view: UpdateDialogView<'_>,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let palette = view.palette;
    let mut panel = div()
        .id("update-dialog-panel")
        .role(gpui::Role::AlertDialog)
        .aria_label(update_dialog_title(view.status))
        .track_focus(view.panel_focus)
        .tab_stop(false)
        .mt(theme::ui_rem(panel_offset))
        .w_full()
        .max_w(theme::ui_rem(640.0))
        .max_h(relative(0.86))
        .overflow_hidden()
        .rounded(theme::ui_rem(theme::RADIUS_LG))
        .bg(color(palette.surface))
        .shadow(card_surface_shadows(palette))
        .occlude()
        .on_click(cx.listener(|_, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
        }))
        .child(update_dialog_header(
            view.status,
            view.close_focus,
            palette,
            window,
            cx,
        ))
        .child(update_dialog_body(
            view.status,
            view.info,
            view.install_ready,
            view.release_notes_scroll_handle,
            palette,
        ))
        .child(update_dialog_footer(
            view.status,
            view.install_ready,
            palette,
            window,
            cx,
        ));

    if matches!(view.status, UpdateStatus::Downloading { .. }) {
        panel = panel.child(update_dialog_download_state(view.status, palette));
    }

    panel
}

fn update_dialog_header(
    status: &UpdateStatus,
    close_focus: &FocusHandle,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let mut title_stack = div().flex().flex_col().gap_1();
    if let Some(kicker) = update_dialog_kicker(status) {
        title_stack = title_stack.child(
            div()
                .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                .font_weight(theme::TEXT_WEIGHT_MEDIUM)
                .text_color(color(palette.text_muted))
                .child(theme::ui_text(kicker)),
        );
    }
    title_stack = title_stack.child(
        div()
            .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
            .font_weight(theme::TEXT_WEIGHT_MEDIUM)
            .text_color(color(palette.text_primary))
            .child(theme::ui_text_owned(update_dialog_title(status))),
    );

    div()
        .relative()
        .h(theme::ui_rem(PANEL_HEADER_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_4()
        .child(title_stack)
        .child(
            app_settings_close_button(
                "update-dialog-close",
                "Close update dialog",
                !status.is_busy(),
                close_focus,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                if !root.update_ui.status.is_busy() {
                    root.close_update_dialog();
                    root.restore_focus_after_update_dialog_close(window, cx);
                    cx.notify();
                }
            })),
        )
        .child(panel_bottom_separator(palette))
}

fn update_dialog_body(
    status: &UpdateStatus,
    info: Option<&UpdateInfo>,
    install_ready: bool,
    release_notes_scroll_handle: &ScrollHandle,
    palette: &'static theme::ThemePalette,
) -> gpui::Div {
    let notes = update_release_notes_text(info);
    let mut body = div().flex().flex_col().gap_3().p_4();

    if let Some(summary) = update_dialog_summary(status, notes.is_some(), install_ready) {
        body = body.child(
            div()
                .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                .line_height(theme::ui_rem(16.0))
                .text_color(color(palette.text_muted))
                .child(theme::ui_text_owned(summary)),
        );
    }

    if let Some(notes) = notes.as_deref() {
        body = body.child(update_release_notes_block(
            notes,
            release_notes_scroll_handle,
            palette,
        ));
    }

    if let UpdateStatus::Error(error) = status {
        body = body.child(
            div()
                .id("update-dialog-error-alert")
                .role(gpui::Role::Alert)
                .aria_label(error.clone())
                .rounded(theme::ui_rem(theme::RADIUS_SM))
                .bg(color(palette.danger.with_alpha(0.08)))
                .p_3()
                .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
                .line_height(theme::ui_rem(16.0))
                .text_color(color(palette.danger))
                .child(error.clone()),
        );
    }

    body
}

fn update_dialog_download_state(
    status: &UpdateStatus,
    palette: &'static theme::ThemePalette,
) -> gpui::Div {
    let UpdateStatus::Downloading {
        progress_percent,
        received_bytes,
        total_bytes,
        ..
    } = status
    else {
        return div();
    };

    div()
        .px_4()
        .pb_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(update_progress_bar(*progress_percent, palette))
        .child(update_download_detail(
            *received_bytes,
            *total_bytes,
            *progress_percent,
            palette,
        ))
}

fn update_dialog_footer(
    status: &UpdateStatus,
    install_ready: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    div()
        .relative()
        .w_full()
        .flex()
        .items_center()
        .justify_end()
        .gap_3()
        .pb_4()
        .px_4()
        .child(
            frame_text_button(
                "update-dialog-later",
                "Later",
                ButtonVariant::Ghost,
                false,
                !status.is_busy(),
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                if !root.update_ui.status.is_busy() {
                    root.close_update_dialog();
                    root.restore_focus_after_update_dialog_close(window, cx);
                    cx.notify();
                }
            })),
        )
        .child(update_dialog_primary_action(
            status,
            install_ready,
            palette,
            window,
            cx,
        ))
}

fn update_dialog_primary_action(
    status: &UpdateStatus,
    install_ready: bool,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    match status {
        UpdateStatus::Available(_) => action_button(
            "update-dialog-download",
            assets::ICON_DOWNLOAD_02,
            Some("Download"),
            "Download",
            ButtonVariant::Default,
            true,
            palette,
            window,
            cx,
        )
        .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            root.download_available_update(cx);
            cx.notify();
        })),
        UpdateStatus::ReadyToInstall(_) => frame_text_button(
            "update-dialog-install",
            "Install and restart",
            ButtonVariant::Default,
            false,
            install_ready,
            palette,
            window,
            cx,
        )
        .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            root.install_downloaded_update(cx);
            cx.notify();
        })),
        UpdateStatus::Error(_) => frame_text_button(
            "update-dialog-dismiss",
            "Dismiss",
            ButtonVariant::Secondary,
            false,
            true,
            palette,
            window,
            cx,
        )
        .on_click(cx.listener(|root, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            root.dismiss_update_status();
            root.restore_focus_after_update_dialog_close(window, cx);
            cx.notify();
        })),
        UpdateStatus::Downloading { .. } | UpdateStatus::Installing => frame_text_button(
            "update-dialog-busy",
            "Working",
            ButtonVariant::Secondary,
            false,
            false,
            palette,
            window,
            cx,
        ),
        UpdateStatus::Idle
        | UpdateStatus::Checking
        | UpdateStatus::UpToDate
        | UpdateStatus::Disabled(_) => frame_text_button(
            "update-dialog-close",
            "Close",
            ButtonVariant::Secondary,
            false,
            true,
            palette,
            window,
            cx,
        )
        .on_click(cx.listener(|root, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            root.close_update_dialog();
            root.restore_focus_after_update_dialog_close(window, cx);
            cx.notify();
        })),
    }
}

const fn update_dialog_kicker(status: &UpdateStatus) -> Option<&'static str> {
    match status {
        UpdateStatus::Available(_) => None,
        UpdateStatus::Downloading { .. } => Some("Downloading update"),
        UpdateStatus::ReadyToInstall(_) => Some("Ready to install"),
        UpdateStatus::Installing => Some("Installing update"),
        UpdateStatus::Error(_) => Some("Update error"),
        UpdateStatus::Checking => Some("Checking for updates"),
        UpdateStatus::UpToDate => Some("No update available"),
        UpdateStatus::Disabled(_) => Some("Updates disabled"),
        UpdateStatus::Idle => Some("Updates"),
    }
}

fn update_dialog_title(status: &UpdateStatus) -> String {
    match status {
        UpdateStatus::Available(info) => format!("Frame {} is available", info.version),
        UpdateStatus::Downloading { version, .. } => format!("Downloading Frame {version}"),
        UpdateStatus::ReadyToInstall(package) => {
            format!("Frame {} is ready to install", package.version)
        }
        UpdateStatus::Installing => "Installing update and restarting".to_string(),
        UpdateStatus::Error(_) => "Frame could not complete the update".to_string(),
        UpdateStatus::Checking => "Checking for updates".to_string(),
        UpdateStatus::UpToDate => "Frame is up to date".to_string(),
        UpdateStatus::Disabled(explanation) => explanation.clone(),
        UpdateStatus::Idle => "Frame updates".to_string(),
    }
}

fn update_dialog_summary(
    status: &UpdateStatus,
    has_notes: bool,
    install_ready: bool,
) -> Option<String> {
    match status {
        UpdateStatus::Available(_) if has_notes => None,
        UpdateStatus::Available(_) => Some(
            "A signed update is available, but this release did not include notes.".to_string()
        ),
        UpdateStatus::Downloading { .. } => Some(
            "Keep Frame open while the update package is downloaded and verified.".to_string()
        ),
        UpdateStatus::ReadyToInstall(_) if !install_ready => {
            Some(UPDATE_INSTALL_WAIT_MESSAGE.to_string())
        }
        UpdateStatus::ReadyToInstall(_) => Some(
            "The update was downloaded and verified. Frame will restart to finish installation."
                .to_string()
        ),
        UpdateStatus::Installing => Some(
            "Frame is handing installation to the bundled update helper.".to_string()
        ),
        UpdateStatus::Error(_) => Some(
            "The updater stopped before installation completed. You can dismiss this and try again."
                .to_string()
        ),
        UpdateStatus::Checking => Some(
            "Frame is checking the latest signed release manifest.".to_string()
        ),
        UpdateStatus::UpToDate => Some("No newer signed release is available.".to_string()),
        UpdateStatus::Disabled(explanation) => Some(explanation.clone()),
        UpdateStatus::Idle => Some("No update check is running.".to_string()),
    }
}

pub(super) fn app_settings_close_button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    focus: &FocusHandle,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let colors = button_colors(ButtonVariant::Ghost, false, enabled, palette);
    let animated = animated_button_colors(id, colors, window, cx);
    let background = animated.background;
    let foreground = animated.foreground;
    let motion = animated.motion;

    let button = div()
        .id(id)
        .group(id)
        .w(theme::ui_rem(SETTINGS_CONTROL_HEIGHT))
        .h(theme::ui_rem(SETTINGS_CONTROL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .bg(background)
        .text_color(foreground)
        .when(enabled, |this| this.hover(gpui::Styled::cursor_pointer))
        .when(enabled, |this| {
            this.active(move |style| style.bg(color(colors.active_background)))
        })
        .child(icon_svg(
            assets::ICON_CLOSE,
            FILE_LIST_ACTION_ICON_SIZE,
            foreground,
        ));
    let button = apply_button_motion(button, motion, enabled);

    apply_accessible_button_with_focus(button, label, enabled, focus, palette)
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

#[cfg(test)]
mod tests {
    use super::*;
    use frame_updater::{PlatformAssetKey, UpdateAssetKind, UpdateChannel, UpdatePackage};
    use semver::Version;
    use std::path::PathBuf;

    #[test]
    fn release_note_emphasis_strips_markers_and_highlights_range() {
        let palette = theme::palette(crate::appearance::ColorTheme::Dark);
        let (text, highlights) = parse_update_release_note_emphasis(
            "• **Native GPUI Application:** Rebuilt Frame",
            palette,
        );

        assert_eq!(text, "• Native GPUI Application: Rebuilt Frame");
        assert_eq!(highlights.len(), 1);
        assert_eq!(&text[highlights[0].0.clone()], "Native GPUI Application:");
        assert_eq!(
            highlights[0].1.color,
            Some(color(palette.text_primary).into())
        );
        assert_eq!(highlights[0].1.font_weight, Some(theme::TEXT_WEIGHT_MEDIUM));
    }

    #[test]
    fn release_note_emphasis_keeps_unclosed_markers_literal() {
        let (text, highlights) = parse_update_release_note_emphasis(
            "• **Native GPUI Application: Rebuilt Frame",
            theme::palette(crate::appearance::ColorTheme::Dark),
        );

        assert_eq!(text, "• **Native GPUI Application: Rebuilt Frame");
        assert!(highlights.is_empty());
    }

    #[test]
    fn ready_update_explains_why_install_is_blocked() {
        let status = UpdateStatus::ReadyToInstall(Box::new(test_update_package()));

        assert_eq!(
            update_status_text(&status, false),
            UPDATE_INSTALL_WAIT_MESSAGE
        );
        assert_eq!(
            update_dialog_summary(&status, false, false).as_deref(),
            Some(UPDATE_INSTALL_WAIT_MESSAGE)
        );
    }

    fn test_update_package() -> UpdatePackage {
        UpdatePackage {
            version: Version::new(0, 32, 0),
            channel: UpdateChannel::Stable,
            asset_key: PlatformAssetKey::MacosAarch64,
            kind: UpdateAssetKind::MacosAppZip,
            file_name: "Frame.zip".to_string(),
            path: PathBuf::from("/tmp/Frame.zip"),
            size_bytes: 1,
            sha256: "00".repeat(32),
            installer_args: Vec::new(),
        }
    }
}
