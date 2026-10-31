use super::accessibility::{APP_ROOT_FOCUS_ID, handle_tab_navigation};
use super::files::FileDropLifecycleProbe;
use super::preview_panel::{
    PreviewEditToolbarFocus, PreviewEditToolbarFocuses, PreviewToolFocuses, PreviewViewportFocuses,
};
use super::*;

impl Render for FrameRoot {
    #[expect(
        clippy::too_many_lines,
        reason = "The root GPUI render function assembles the full application shell from a single state snapshot."
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(
            crate::appearance::BASE_REM_PX * self.appearance.ui_scale.factor()
        ));
        if !self.settings_ui.appearance_observer_registered {
            cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                .detach();
            self.settings_ui.appearance_observer_registered = true;
        }
        if self.appearance.follow_system_theme {
            self.appearance.color_theme = super::settings_actions::system_color_theme(window);
        }
        m3::update_color_schemes(self.appearance.accent_color, cx);
        let palette = theme::palette(self.appearance.color_theme);
        self.begin_accessibility_frame();
        let app_root_focus = self.ensure_focus(
            FrameFocusKey::Control(APP_ROOT_FOCUS_ID.to_string()),
            false,
            cx,
        );
        if window.focused(cx).is_none() {
            app_root_focus.focus(window, cx);
        }

        self.reconcile_text_input_focus(window, cx);

        let state = self.app_state();
        let source_metadata_entry = self.selected_source_metadata_entry();
        let source_metadata = source_metadata_entry.metadata.clone();
        self.normalize_selected_config(source_metadata.as_ref());
        self.resolve_selected_settings_tab(source_metadata.as_ref());
        self.conversion_events
            .ensure_selected_log_file(&self.file_queue);
        self.update_log_scroll_target();
        let selected_file_id = self.file_queue.selected_file_id().map(str::to_string);
        let selected_config_snapshot = self.selected_config().cloned().unwrap_or_default();

        let preview_runtime_request = self.selected_preview_runtime_request(&source_metadata_entry);
        self.sync_preview_crop_for_selection(
            selected_file_id.as_deref(),
            &selected_config_snapshot,
        );
        self.sync_preview_overlay_for_selection(
            selected_file_id.as_deref(),
            &selected_config_snapshot,
            cx,
        );
        self.sync_preview_canvas_for_selection(selected_file_id.as_deref());
        self.sync_preview_runtime_for_selection(preview_runtime_request, cx);
        self.sync_preview_playback_for_selection(
            selected_file_id.as_deref(),
            source_metadata.as_ref(),
            &selected_config_snapshot,
            cx,
        );
        self.sync_preview_canvas_auto_fit();
        let preview_crop =
            self.preview_crop_render_state(source_metadata.as_ref(), &selected_config_snapshot);
        let preview_overlay = self.preview_overlay_render_state();
        let preview_canvas = self.preview_canvas_render_state();
        let preview_playback = self.preview_playback_state();
        let preview_presentation = self.preview_ui.render_presentation;
        let preview_render_image = self.preview_render_image();
        let preview_runtime_error = self.preview_runtime_error();
        let preview_availability = preview_control_availability(PreviewControlInput {
            metadata_status: if source_metadata.is_some() {
                PreviewMetadataStatus::Ready
            } else {
                PreviewMetadataStatus::Idle
            },
            source_media_kind: source_metadata.as_ref().map(preview_source_media_kind),
            controls_disabled: self.file_queue.selected_file_locked(),
            processing_mode: selected_config_snapshot.processing_mode,
            container: Some(selected_config_snapshot.container.as_str()),
        });
        let preview_visual_controls_enabled = preview_availability.media_kind
            != PreviewMediaKind::Unknown
            && !preview_availability.hide_visual_controls
            && !self.file_queue.selected_file_locked();
        let crop_tool_enabled = preview_visual_controls_enabled && preview_crop.has_crop_dimensions;
        let overlay_tool_enabled =
            preview_visual_controls_enabled && preview_availability.overlay_available;
        let preview_viewport_pan_enabled = preview_render_image.is_some()
            && preview_visual_controls_enabled
            && !preview_crop.crop_mode
            && !preview_overlay.overlay_mode;
        let preview_viewport_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-viewport".to_string()),
            preview_viewport_pan_enabled,
            cx,
        );
        let preview_crop_tool_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-tool-crop".to_string()),
            crop_tool_enabled,
            cx,
        );
        let preview_overlay_tool_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-tool-overlay".to_string()),
            overlay_tool_enabled,
            cx,
        );
        let crop_toolbar_active = preview_crop.crop_mode && preview_crop.draft_crop.is_some();
        let crop_toolbar_panel_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-crop-toolbar".to_string()),
            false,
            cx,
        );
        let crop_toolbar_first_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-crop-action-free".to_string()),
            crop_toolbar_active,
            cx,
        );
        let crop_toolbar_last_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-crop-action-apply".to_string()),
            crop_toolbar_active,
            cx,
        );
        let overlay_toolbar_active =
            preview_overlay.overlay_mode && preview_overlay.overlay.is_some();
        let overlay_toolbar_panel_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-overlay-toolbar".to_string()),
            false,
            cx,
        );
        let overlay_toolbar_first_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-overlay-replace".to_string()),
            overlay_toolbar_active,
            cx,
        );
        let overlay_toolbar_last_focus = self.ensure_focus(
            FrameFocusKey::Control("preview-overlay-done".to_string()),
            overlay_toolbar_active,
            cx,
        );
        if !crop_toolbar_active
            && !overlay_toolbar_active
            && (crop_toolbar_first_focus.is_focused(window)
                || crop_toolbar_last_focus.is_focused(window))
        {
            if crop_tool_enabled {
                preview_crop_tool_focus.focus(window, cx);
            } else {
                app_root_focus.focus(window, cx);
            }
        }
        if !overlay_toolbar_active
            && !crop_toolbar_active
            && (overlay_toolbar_first_focus.is_focused(window)
                || overlay_toolbar_last_focus.is_focused(window))
        {
            if overlay_tool_enabled {
                preview_overlay_tool_focus.focus(window, cx);
            } else {
                app_root_focus.focus(window, cx);
            }
        }
        if !preview_viewport_pan_enabled && preview_viewport_focus.is_focused(window) {
            app_root_focus.focus(window, cx);
        }
        if crop_toolbar_active && !crop_toolbar_panel_focus.contains_focused(window, cx) {
            crop_toolbar_first_focus.focus(window, cx);
        }
        if overlay_toolbar_active && !overlay_toolbar_panel_focus.contains_focused(window, cx) {
            overlay_toolbar_first_focus.focus(window, cx);
        }
        let content = div().flex_1().min_h_0();
        let active_content_view = if state.file_count == 0 && !self.project.enabled {
            None
        } else {
            Some(state.active_view)
        };
        let content = match active_content_view {
            None => content.child(welcome_view(palette, window, cx)),
            Some(ActiveView::Workspace) => {
                let preview_seek_focus = self.ensure_focus(
                    FrameFocusKey::Control(hotkeys::PREVIEW_SEEK_FOCUS_ID.to_string()),
                    self.preview_timeline_enabled(),
                    cx,
                );
                let video_width_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::VideoCustomWidth, cx);
                let video_height_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::VideoCustomHeight, cx);
                let metadata_title_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataTitle, cx);
                let metadata_artist_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataArtist, cx);
                let metadata_album_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataAlbum, cx);
                let metadata_genre_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataGenre, cx);
                let metadata_date_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataDate, cx);
                let metadata_comment_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataComment, cx);
                let metadata_service_name_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataServiceName, cx);
                let metadata_service_provider_focus =
                    self.ensure_text_input_focus(FrameTextInputKind::MetadataServiceProvider, cx);
                let settings = SettingsRenderState {
                    palette,
                    active_tab: self.settings_ui.active_tab,
                    tooltip_visible_id: self.tooltip_ui.visible_id.as_deref(),
                    config: &selected_config_snapshot,
                    metadata: source_metadata.as_ref(),
                    metadata_status: source_metadata_entry.status,
                    metadata_error: source_metadata_entry.error.as_deref(),
                    settings_disabled: self.file_queue.selected_file_locked(),
                    video_width_focus: Some(&video_width_focus),
                    video_height_focus: Some(&video_height_focus),
                    metadata_focuses: SettingsMetadataInputFocuses {
                        title: Some(&metadata_title_focus),
                        artist: Some(&metadata_artist_focus),
                        album: Some(&metadata_album_focus),
                        genre: Some(&metadata_genre_focus),
                        date: Some(&metadata_date_focus),
                        comment: Some(&metadata_comment_focus),
                        service_name: Some(&metadata_service_name_focus),
                        service_provider: Some(&metadata_service_provider_focus),
                    },
                    available_filters: &self.available_filters,
                };
                content.child(workspace_view(
                    super::media_assets::MediaAssetsProps {
                        state: &self.media_assets,
                        delete_enabled: !self.project.enabled || !self.is_processing,
                    },
                    &self.file_queue,
                    &settings,
                    PreviewPanelProps {
                        project: self.project_render_state(cx),
                        canvas: preview_canvas,
                        crop: preview_crop,
                        overlay: preview_overlay,
                        viewport_focuses: PreviewViewportFocuses {
                            viewport: &preview_viewport_focus,
                            tools: PreviewToolFocuses {
                                crop: &preview_crop_tool_focus,
                                overlay: &preview_overlay_tool_focus,
                            },
                            edit_toolbars: PreviewEditToolbarFocuses {
                                crop: PreviewEditToolbarFocus {
                                    panel: &crop_toolbar_panel_focus,
                                    first: &crop_toolbar_first_focus,
                                    last: &crop_toolbar_last_focus,
                                },
                                overlay: PreviewEditToolbarFocus {
                                    panel: &overlay_toolbar_panel_focus,
                                    first: &overlay_toolbar_first_focus,
                                    last: &overlay_toolbar_last_focus,
                                },
                            },
                        },
                        seek_focus: &preview_seek_focus,
                        playback: preview_playback,
                        presentation: preview_presentation,
                        render_image: preview_render_image,
                        runtime_error: preview_runtime_error,
                    },
                    workspace::WorkspaceState {
                        export_enabled: if self.project.enabled {
                            !self.project.clips.is_empty()
                        } else {
                            self.file_queue.selected_file().is_some()
                        },
                        ..self.workspace_ui
                    },
                    &self.project_header_props(),
                    window,
                    cx,
                ))
            }
            Some(ActiveView::Logs) => {
                let logs = logs_view(
                    &self.file_queue,
                    &self.conversion_events,
                    &self.logs_scroll_handle,
                    self.logs_follow_tail,
                    self.copied_log_file_id.as_deref(),
                    palette,
                    window,
                    cx,
                );
                content.child(workspace::logs_workspace_view(
                    logs,
                    &self.file_queue,
                    workspace::WorkspaceState {
                        export_enabled: if self.project.enabled {
                            !self.project.clips.is_empty()
                        } else {
                            self.file_queue.selected_file().is_some()
                        },
                        ..self.workspace_ui
                    },
                    &self.project_header_props(),
                    &crate::app::m3::scheme(self.appearance.color_theme, cx),
                    window,
                    cx,
                ))
            }
        };
        let content = self.m3_gallery.clone().map_or(content, |gallery| {
            div()
                .flex_1()
                .p(theme::ui_rem(CONTENT_PADDING))
                .child(self.render_m3_gallery(&gallery, window, cx))
        });

        let shortcut_root_focus = app_root_focus.clone();
        let mut root = div()
            .id(APP_ROOT_FOCUS_ID)
            .size_full()
            .track_focus(&app_root_focus)
            .tab_stop(false)
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .group(ROOT_DROP_GROUP)
            .bg(color(palette.canvas))
            .text_color(color(palette.text_primary))
            .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
            .font_family(assets::FRAME_FONT_FAMILY)
            .font_weight(theme::TEXT_WEIGHT_REGULAR)
            .font_features(assets::frame_font_features())
            .on_action(cx.listener(Self::increase_ui_scale))
            .on_action(cx.listener(Self::decrease_ui_scale))
            .on_action(cx.listener(Self::reset_ui_scale))
            .on_action(cx.listener(Self::toggle_fps_meter))
            .on_action(cx.listener(Self::save_project_action))
            .on_action(cx.listener(Self::save_project_as_action))
            .on_action(cx.listener(Self::open_project_action))
            .on_action(cx.listener(Self::open_settings_action))
            .on_key_down(
                cx.listener(move |root, event: &gpui::KeyDownEvent, window, cx| {
                    handle_tab_navigation(event, window, cx);
                    root.handle_preview_shortcut(event, &shortcut_root_focus, window, cx);
                }),
            )
            .on_drop(cx.listener(|root, paths: &ExternalPaths, _window, cx| {
                cx.stop_propagation();
                root.close_drag_drop_overlay();
                Self::import_source_paths(paths.paths().to_vec(), cx);
                cx.notify();
            }))
            .on_drag_move(cx.listener(
                |root, _event: &DragMoveEvent<ExternalPaths>, _window, cx| {
                    if root.open_drag_drop_overlay() {
                        cx.notify();
                    }
                },
            ))
            .child(content)
            .child(FileDropLifecycleProbe { owner: cx.entity() });

        if self.settings_ui.is_present {
            let theme_option_focuses = ["light", "dark", "system"].map(|name| {
                self.ensure_focus(
                    FrameFocusKey::Control(format!("app-settings-theme-{name}")),
                    true,
                    cx,
                )
            });
            let ui_scale_option_focuses = [90, 100, 110, 125].map(|percent| {
                self.ensure_focus(
                    FrameFocusKey::Control(format!("app-settings-ui-scale-{percent}")),
                    true,
                    cx,
                )
            });
            let accent_preset_focuses = crate::appearance::AccentColor::PRESETS
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    self.ensure_focus(
                        FrameFocusKey::Control(format!("app-settings-accent-{index}")),
                        true,
                        cx,
                    )
                })
                .collect::<Vec<_>>();
            let output_directory_focus = self.ensure_focus(
                FrameFocusKey::Control("app-settings-output-directory".to_string()),
                true,
                cx,
            );
            let fps_meter_focus = self.ensure_focus(
                FrameFocusKey::Control("app-settings-fps-meter".to_string()),
                true,
                cx,
            );
            let panel_focus = self.ensure_focus(
                FrameFocusKey::Control("app-settings-panel".to_string()),
                false,
                cx,
            );
            let close_focus = self.ensure_focus(
                FrameFocusKey::Control("app-settings-close".to_string()),
                true,
                cx,
            );
            if self.settings_ui.is_open && !panel_focus.contains_focused(window, cx) {
                close_focus.focus(window, cx);
            }
            root = root.child(app_settings_sheet(
                AppSettingsSheetProps {
                    is_open: self.settings_ui.is_open,
                    default_output_directory: self
                        .default_output_directory
                        .as_deref()
                        .and_then(std::path::Path::to_str),
                    output_directory_error: self.settings_ui.output_directory_error.as_deref(),
                    appearance: self.appearance,
                    accent_preset_focuses: &accent_preset_focuses,
                    appearance_error: self.settings_ui.appearance_error.as_deref(),
                    scroll_handle: &self.settings_ui.app_settings_scroll_handle,
                    theme_focuses: &theme_option_focuses,
                    ui_scale_focuses: &ui_scale_option_focuses,
                    show_fps_meter: self.fps_meter.is_some(),
                    output_directory_focus: &output_directory_focus,
                    fps_meter_focus: &fps_meter_focus,
                    panel_focus: &panel_focus,
                    close_focus: &close_focus,
                    last_focus: &fps_meter_focus,
                },
                window,
                cx,
            ));
        }

        if self.export_ui.present {
            root = root.child(self.render_export_dialog(window, cx));
        }

        if self.project.confirm_discard {
            root = root.child(self.render_discard_project_dialog(window, cx));
        }

        if self.drag_drop_ui.is_present {
            root = root.child(drag_drop_overlay(
                self.drag_drop_ui.is_open,
                palette,
                window,
                cx,
            ));
        }

        if let Some(meter) = self.fps_meter.as_mut() {
            if window.is_window_active() {
                meter.record_frame(Instant::now());
                window.request_animation_frame();
            } else {
                meter.reset();
            }
            root = root.child(meter.overlay(palette));
        }

        self.finish_accessibility_frame(window, cx, Some(&app_root_focus));

        linux_window_frame(root, window, palette)
    }
}

#[cfg(target_os = "linux")]
fn linux_window_frame(
    root: gpui::Stateful<gpui::Div>,
    window: &Window,
    palette: &'static theme::ThemePalette,
) -> impl IntoElement {
    let should_draw_frame = matches!(
        window.window_decorations(),
        gpui::Decorations::Client { tiling }
            if !(tiling.top || tiling.right || tiling.bottom || tiling.left)
    );

    if !should_draw_frame {
        return div().size_full().child(root);
    }

    div().size_full().p(px(LINUX_WINDOW_FRAME_INSET)).child(
        root.rounded(px(theme::RADIUS_LG))
            .border_1()
            .border_color(color(palette.fill_selected))
            .shadow(linux_window_frame_shadows(palette)),
    )
}

#[cfg(not(target_os = "linux"))]
fn linux_window_frame(
    root: gpui::Stateful<gpui::Div>,
    _window: &Window,
    _palette: &'static theme::ThemePalette,
) -> impl IntoElement {
    root
}

#[cfg(target_os = "linux")]
fn linux_window_frame_shadows(palette: &'static theme::ThemePalette) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: color(palette.shadow.with_alpha(0.28)).into(),
            offset: point(px(0.0), px(8.0)),
            blur_radius: px(8.0),
            spread_radius: px(-7.0),
            inset: false,
        },
        BoxShadow {
            color: color(palette.shadow.with_alpha(0.18)).into(),
            offset: point(px(0.0), px(3.0)),
            blur_radius: px(5.0),
            spread_radius: px(-3.0),
            inset: false,
        },
        BoxShadow {
            color: color(palette.fill_selected).into(),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: true,
        },
    ]
}
