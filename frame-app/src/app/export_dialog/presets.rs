use super::*;

#[derive(Clone)]
pub(super) struct PresetSelection {
    pub(super) id: String,
    config: ConversionConfig,
}

fn reusable_config(config: &ConversionConfig) -> ConversionConfig {
    create_custom_preset(String::new(), "", config).config
}

impl FrameRoot {
    fn export_preset_target(&self) -> Option<String> {
        if self.editing_project_output() {
            Some("project".to_string())
        } else {
            self.file_queue
                .selected_file()
                .map(|file| format!("file:{}", file.id))
        }
    }

    pub(in crate::app) fn remember_export_preset(&mut self, id: &str) -> bool {
        let Some(target) = self.export_preset_target() else {
            return false;
        };
        let Some(config) = self.selected_config() else {
            return false;
        };
        let config = reusable_config(config);
        let changed = self
            .export_ui
            .preset_selections
            .get(&target)
            .is_none_or(|selection| selection.id != id || selection.config != config);
        self.export_ui.preset_selections.insert(
            target,
            PresetSelection {
                id: id.to_string(),
                config,
            },
        );
        changed
    }

    pub(super) fn export_preset_selection(&self, view: &ExportView) -> Option<PresetSelection> {
        if let Some(selection) = self
            .export_preset_target()
            .and_then(|target| self.export_ui.preset_selections.get(&target))
            .filter(|selection| self.presets.iter().any(|preset| preset.id == selection.id))
        {
            return Some(selection.clone());
        }
        self.presets.iter().find_map(|preset| {
            if !crate::settings::preset_is_compatible(preset, view.metadata()) {
                return None;
            }
            let mut expected = view.config.clone();
            apply_preset(&mut expected, preset, view.metadata());
            (reusable_config(&expected) == reusable_config(&view.config)).then(|| PresetSelection {
                id: preset.id.clone(),
                config: reusable_config(&expected),
            })
        })
    }

    pub(super) fn initialize_export_preset(&mut self, view: &ExportView) {
        if self
            .export_preset_target()
            .is_some_and(|target| self.export_ui.preset_selections.contains_key(&target))
        {
            return;
        }
        if let Some(selection) = self.export_preset_selection(view) {
            self.remember_export_preset(&selection.id);
        }
    }

    pub(super) fn export_preset_edited(&self, view: &ExportView) -> bool {
        self.export_preset_selection(view)
            .is_some_and(|selection| selection.config != reusable_config(&view.config))
    }

    pub(super) fn revert_export_preset(&mut self) -> bool {
        let scheme = crate::material::scheme(self.appearance.color_theme);
        let view = self.export_view(scheme);
        let Some(selection) = self.export_preset_selection(&view) else {
            return false;
        };
        self.apply_preset_to_selected(&selection.id)
    }

    pub(super) fn export_presets_panel(
        &self,
        view: &ExportView,
        focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let scheme = &view.scheme;
        let selection = self.export_preset_selection(view);
        let edited = selection
            .as_ref()
            .is_some_and(|selection| selection.config != reusable_config(&view.config));
        let applied = selection
            .as_ref()
            .and_then(|selection| self.presets.iter().find(|preset| preset.id == selection.id));
        let save_enabled = !view.disabled && !self.settings_ui.preset_name_draft.trim().is_empty();
        let list = preset_list(&self.presets, view, applied, edited, window, cx);
        div()
            .w(theme::ui_rem(240.0))
            .flex_none()
            .flex()
            .flex_col()
            .p(theme::ui_rem(16.0))
            .gap(theme::ui_rem(8.0))
            .border_r_1()
            .border_color(color(scheme.outline_variant))
            .bg(color(scheme.surface_container_low))
            .text_color(color(scheme.on_surface))
            .child(div().m3_type(typography::TITLE_SMALL).child("Presets"))
            .child(
                div()
                    .m3_type(typography::BODY_SMALL)
                    .text_color(color(scheme.on_surface_variant))
                    .child(applied.map_or_else(
                        || "No preset selected".to_string(),
                        |preset| format!("Applied: {}", preset.name),
                    )),
            )
            .child(export_text_field(
                ExportTextField {
                    id: "export-preset-name",
                    input_id: "export-preset-name-input",
                    label: "Save current as…",
                    value: self.settings_ui.preset_name_draft.clone(),
                    kind: FrameTextInputKind::PresetName,
                    suffix: None,
                    supporting: None,
                    error: None,
                    container: scheme.surface_container_low,
                },
                focus,
                view.disabled,
                scheme,
                window,
                cx,
            ))
            .child(
                div().flex().justify_end().child(
                    m3::Button::new("export-save-preset", "Save", m3::ButtonVariant::Tonal)
                        .icon(Symbol::Add)
                        .enabled(save_enabled)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                            if save_enabled {
                                root.save_preset_from_draft();
                                cx.notify();
                            }
                        })),
                ),
            )
            .when_some(self.settings_ui.preset_notice.as_ref(), |panel, notice| {
                panel.child(
                    div()
                        .id("export-preset-notice")
                        .role(if notice.tone == PresetNoticeTone::Error {
                            gpui::Role::Alert
                        } else {
                            gpui::Role::Status
                        })
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(if notice.tone == PresetNoticeTone::Error {
                            scheme.error
                        } else {
                            scheme.on_surface_variant
                        }))
                        .child(notice.text.clone()),
                )
            })
            .child(list)
    }
}

fn preset_group(preset: &PresetDefinition) -> &'static str {
    if !preset.built_in {
        return "Custom";
    }
    if is_audio_only_container(&preset.config.container) {
        return "Audio";
    }
    if is_gif_container(&preset.config.container) {
        return "GIF";
    }
    if matches!(preset.config.container.as_str(), "m2t" | "m2ts" | "ts") {
        return "Broadcast";
    }
    if matches!(
        preset.id.as_str(),
        "social-tiktok" | "yt-1080p" | "yt-4k" | "x-landscape" | "x-portrait" | "discord"
    ) {
        return "Social";
    }
    "Video"
}

fn preset_summary(config: &ConversionConfig) -> String {
    let size = if config.resolution == "original" {
        "Source size".to_string()
    } else {
        config.resolution.clone()
    };
    format!(
        "{} · {} · {}",
        config.container.to_uppercase(),
        preset_codec_label(config),
        size
    )
}

fn preset_codec_label(config: &ConversionConfig) -> &str {
    if is_audio_only_container(&config.container) {
        return &config.audio_codec;
    }
    match config.video_codec.as_str() {
        "libx264" => "H.264",
        "libx265" => "H.265",
        "libsvtav1" => "AV1",
        "vp9" => "VP9",
        "mpeg2video" => "MPEG-2",
        "prores" => "ProRes",
        _ => &config.video_codec,
    }
}

fn preset_list(
    presets: &[PresetDefinition],
    view: &ExportView,
    applied: Option<&PresetDefinition>,
    edited: bool,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let scheme = &view.scheme;
    let mut list = div()
        .id("export-preset-list")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(4.0))
        .role(gpui::Role::RadioGroup)
        .aria_label("Export presets");
    for group in ["Custom", "Video", "Broadcast", "GIF", "Audio", "Social"] {
        let presets: Vec<_> = presets
            .iter()
            .filter(|preset| preset_group(preset) == group)
            .filter(|preset| crate::settings::preset_is_compatible(preset, view.metadata()))
            .collect();
        if presets.is_empty() {
            continue;
        }
        list = list.child(m3::list_subheader(group, scheme));
        for preset in presets {
            let selected = applied.is_some_and(|applied| applied.id == preset.id);
            list = list.child(preset_row(preset, selected, edited, view, window, cx));
        }
    }

    list
}

fn preset_row(
    preset: &PresetDefinition,
    selected: bool,
    edited: bool,
    view: &ExportView,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = &view.scheme;
    let id = preset.id.clone();
    let delete_id = id.clone();
    let row = m3::SelectCard::new(format!("export-preset-{id}"), preset.name.clone(), selected)
        .single_line()
        .supporting(preset_summary(&preset.config))
        .enabled(!view.disabled)
        .build(scheme, window, cx)
        .relative()
        .border_0()
        .p(theme::ui_rem(12.0))
        .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
            if root.apply_preset_to_selected(&id) {
                cx.notify();
            }
        }))
        .when(selected, |row| {
            row.pr(theme::ui_rem(if edited { 64.0 } else { 32.0 }))
                .child(
                    div()
                        .absolute()
                        .right(theme::ui_rem(8.0))
                        .top(theme::ui_rem(8.0))
                        .rounded(theme::ui_rem(shape::CORNER_SMALL))
                        .px(theme::ui_rem(6.0))
                        .py(theme::ui_rem(2.0))
                        .m3_type(typography::LABEL_SMALL)
                        .bg(color(if edited {
                            scheme.tertiary_container
                        } else {
                            scheme.secondary_container
                        }))
                        .text_color(color(if edited {
                            scheme.on_tertiary_container
                        } else {
                            scheme.on_secondary_container
                        }))
                        .when(edited, |badge| badge.child("Edited"))
                        .when(!edited, |badge| {
                            badge.child(m3::m3_icon(
                                Symbol::Check,
                                false,
                                18.0,
                                color(scheme.on_secondary_container),
                            ))
                        }),
                )
        });
    div()
        .flex()
        .items_center()
        .child(row.flex_1().min_w_0())
        .when(!preset.built_in, |row| {
            row.child(
                m3::IconButton::new(
                    format!("export-delete-preset-{delete_id}"),
                    Symbol::Delete,
                    "Delete preset",
                )
                .enabled(!view.disabled)
                .build(scheme, window, cx)
                .on_click(cx.listener(move |root, _: &ClickEvent, _window, cx| {
                    cx.stop_propagation();
                    if root.delete_preset(&delete_id) {
                        cx.notify();
                    }
                })),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> FrameRoot {
        let mut root = FrameRoot::new();
        root.apply_preview_ready_fixture();
        root.open_export_dialog();
        root.apply_preset_to_selected("balanced-mp4");
        root
    }

    fn edited(root: &FrameRoot) -> bool {
        root.export_preset_edited(
            &root.export_view(crate::material::scheme(root.appearance.color_theme)),
        )
    }

    #[test]
    fn edits_keep_selection_and_reopening_keeps_edited_state() {
        let mut root = root();
        assert!(!edited(&root));
        root.update_selected_config(|config| apply_fps(config, "24"));
        assert!(edited(&root));
        root.close_export_dialog();
        root.open_export_dialog();
        assert!(edited(&root));
        assert!(root.revert_export_preset());
        assert!(!edited(&root));
        root.update_selected_config(|config| apply_audio_bitrate(config, "320"));
        assert!(edited(&root));
        root.revert_export_preset();
        root.update_selected_config(|config| apply_pixel_format(config, "yuv420p"));
        assert!(edited(&root));
    }

    #[test]
    fn restoring_settings_clears_edited_without_reselecting() {
        let mut root = root();
        let original = root.selected_config().unwrap().clone();
        root.update_selected_config(|config| apply_fps(config, "24"));
        assert!(edited(&root));
        root.update_selected_config(|config| {
            *config = original;
            true
        });
        assert!(!edited(&root));
    }

    #[test]
    fn saving_selects_current_options_and_presets_survive_reuse() {
        let mut root = root();
        root.update_selected_config(|config| apply_fps(config, "24"));
        root.settings_ui.preset_name_draft = "Review".to_string();
        assert!(root.save_preset_from_draft());
        let id = root.presets.last().unwrap().id.clone();
        assert!(!edited(&root));
        root.update_selected_config(|config| apply_audio_bitrate(config, "320"));
        assert!(edited(&root));
        assert!(root.revert_export_preset());
        assert_eq!(root.selected_config().unwrap().fps, "24");
        assert_eq!(
            root.export_preset_selection(
                &root.export_view(crate::material::scheme(root.appearance.color_theme))
            )
            .unwrap()
            .id,
            id
        );
    }

    #[test]
    fn source_tracks_do_not_mark_preset_edited_and_revert_preserves_them() {
        let mut root = root();
        root.selected_config_mut().unwrap().selected_audio_tracks = vec![7];
        assert!(!edited(&root));
        root.update_selected_config(|config| apply_fps(config, "24"));
        root.revert_export_preset();
        assert_eq!(
            root.selected_config().unwrap().selected_audio_tracks,
            vec![7]
        );
    }

    #[test]
    fn selections_belong_to_each_file() {
        let mut root = root();
        let first = root.file_queue.selected_file().unwrap().id.clone();
        root.update_selected_config(|config| apply_fps(config, "24"));
        root.file_queue
            .add_file(FileItem::from_path("second", "/tmp/second.mp4", 1));
        root.file_queue.select_existing_file("second");
        root.apply_preset_to_selected("web-share");
        assert!(!edited(&root));
        root.file_queue.select_existing_file(&first);
        assert!(edited(&root));
    }

    #[test]
    fn timeline_preset_edits_output_and_leaves_clip_settings() {
        let mut root = FrameRoot::new();
        root.apply_project_timeline_fixture();
        let original = root.file_queue.selected_file().unwrap().config.clone();
        root.open_export_dialog();
        assert!(root.apply_preset_to_selected("web-share"));
        assert_eq!(root.project.output.container, "webm");
        assert_eq!(root.file_queue.selected_file().unwrap().config, original);
        assert!(!edited(&root));
        root.update_selected_config(|config| apply_fps(config, "24"));
        assert!(edited(&root));
        root.revert_export_preset();
        assert!(!edited(&root));
    }
}
