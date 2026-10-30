use super::{
    AppearanceSettings, ColorTheme, Context, DecreaseUiScale, FrameRoot, IncreaseUiScale,
    PresetNotice, PresetNoticeTone, ResetUiScale, ScalePreset, Window, apply_preset,
    create_custom_preset, output_folder_dialog, pick_output_folder,
};

impl FrameRoot {
    pub(super) fn open_app_settings(&mut self) {
        self.settings_ui.is_open = true;
        self.settings_ui.is_present = true;
        self.settings_ui.output_directory_error = None;
        self.settings_ui.appearance_error = None;
    }

    pub(super) fn close_app_settings(&mut self) {
        self.settings_ui.is_open = false;
        self.settings_ui.output_directory_error = None;
        self.settings_ui.appearance_error = None;
    }

    pub(super) const fn finish_app_settings_close(&mut self) -> bool {
        if self.settings_ui.is_open || !self.settings_ui.is_present {
            return false;
        }
        self.settings_ui.is_present = false;
        true
    }

    pub(super) fn set_ui_scale(&mut self, scale: ScalePreset) -> bool {
        let mut appearance = self.appearance;
        appearance.ui_scale = scale;
        self.set_appearance(appearance)
    }

    pub(super) fn set_accent_color(&mut self, accent: crate::appearance::AccentColor) -> bool {
        let mut appearance = self.appearance;
        appearance.accent_color = accent;
        self.set_appearance(appearance)
    }

    pub(super) fn set_color_theme(&mut self, color_theme: ColorTheme) -> bool {
        let mut appearance = self.appearance;
        appearance.color_theme = color_theme;
        appearance.follow_system_theme = false;
        self.set_appearance(appearance)
    }

    pub(super) fn set_system_theme(&mut self, window: &Window) -> bool {
        let mut appearance = self.appearance;
        appearance.follow_system_theme = true;
        appearance.color_theme = system_color_theme(window);
        self.set_appearance(appearance)
    }

    pub(super) fn increase_ui_scale(
        &mut self,
        _: &IncreaseUiScale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_ui_scale_shortcut(self.appearance.ui_scale.next(), window, cx);
    }

    pub(super) fn decrease_ui_scale(
        &mut self,
        _: &DecreaseUiScale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_ui_scale_shortcut(self.appearance.ui_scale.previous(), window, cx);
    }

    pub(super) fn reset_ui_scale(
        &mut self,
        _: &ResetUiScale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_ui_scale_shortcut(ScalePreset::Percent100, window, cx);
    }

    fn apply_ui_scale_shortcut(
        &mut self,
        scale: ScalePreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.set_ui_scale(scale) {
            window.set_rem_size(gpui::px(crate::appearance::BASE_REM_PX * scale.factor()));
            cx.notify();
        }
    }

    pub(in crate::app) fn set_appearance(&mut self, appearance: AppearanceSettings) -> bool {
        if self.update_installation_in_progress() || self.appearance == appearance {
            return false;
        }

        let previous = self.appearance;
        self.appearance = appearance;
        if let Err(error) = self.persist_app_settings() {
            self.appearance = previous;
            self.settings_ui.appearance_error = Some(format!("Failed to save settings: {error}"));
            return false;
        }

        self.settings_ui.appearance_error = None;
        true
    }

    pub(super) fn prompt_default_output_folder(window: &Window, cx: &Context<Self>) {
        let dialog = output_folder_dialog(window);
        cx.spawn(async move |this, cx| {
            let Some(path) = pick_output_folder(dialog).await else {
                return;
            };

            this.update(cx, |root, cx| {
                root.apply_default_output_directory(path);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn apply_default_output_directory(&mut self, path: std::path::PathBuf) -> bool {
        match self.set_default_output_directory(path) {
            Ok(()) => {
                self.settings_ui.output_directory_error = None;
                true
            }
            Err(error) => {
                self.open_app_settings();
                self.settings_ui.output_directory_error =
                    Some(format!("Failed to save settings: {error}"));
                false
            }
        }
    }

    pub(super) fn set_default_output_directory(
        &mut self,
        path: std::path::PathBuf,
    ) -> Result<(), crate::app_persistence::AppPersistenceError> {
        if self.update_installation_in_progress() {
            return Err(crate::app_persistence::AppPersistenceError::InstallationInProgress);
        }
        let previous = self.default_output_directory.replace(path);
        if let Err(error) = self.persist_app_settings() {
            self.default_output_directory = previous;
            return Err(error);
        }

        Ok(())
    }

    pub(super) fn save_preset_from_draft(&mut self) -> bool {
        if self.update_installation_in_progress()
            || (self.project.enabled && self.is_processing)
            || (!self.editing_project_output() && self.file_queue.selected_file_locked())
        {
            return false;
        }
        let name = self.settings_ui.preset_name_draft.trim();
        if name.is_empty() {
            self.settings_ui.preset_notice = Some(PresetNotice {
                text: "Name required".to_string(),
                tone: PresetNoticeTone::Error,
            });
            return false;
        }

        let Some(config) = self.selected_config().cloned() else {
            self.settings_ui.preset_notice = Some(PresetNotice {
                text: "Preset not saved".to_string(),
                tone: PresetNoticeTone::Error,
            });
            return false;
        };

        let (id, next_sequence) = self.next_custom_preset_identity();
        self.presets
            .push(create_custom_preset(id.clone(), name, &config));
        if let Err(error) = self.persist_app_settings() {
            self.presets.pop();
            self.settings_ui.preset_notice = Some(PresetNotice {
                text: format!("Preset not saved: {error}"),
                tone: PresetNoticeTone::Error,
            });
            return false;
        }

        self.remember_export_preset(&id);
        self.settings_ui.next_custom_preset_sequence = next_sequence;
        self.settings_ui.preset_name_draft.clear();
        self.settings_ui.preset_notice = Some(PresetNotice {
            text: "Preset saved".to_string(),
            tone: PresetNoticeTone::Success,
        });
        true
    }

    pub(super) fn delete_preset(&mut self, preset_id: &str) -> bool {
        if self.update_installation_in_progress() {
            return false;
        }
        let Some(index) = self
            .presets
            .iter()
            .position(|preset| preset.id == preset_id && !preset.built_in)
        else {
            self.settings_ui.preset_notice = Some(PresetNotice {
                text: "Unable to delete".to_string(),
                tone: PresetNoticeTone::Error,
            });
            return false;
        };

        let removed = self.presets.remove(index);
        if let Err(error) = self.persist_app_settings() {
            self.presets.insert(index, removed);
            self.settings_ui.preset_notice = Some(PresetNotice {
                text: format!("Unable to delete: {error}"),
                tone: PresetNoticeTone::Error,
            });
            return false;
        }

        self.settings_ui.preset_notice = Some(PresetNotice {
            text: "Preset removed".to_string(),
            tone: PresetNoticeTone::Success,
        });
        true
    }

    pub(super) fn apply_preset_to_selected(&mut self, preset_id: &str) -> bool {
        if self.update_installation_in_progress()
            || (self.project.enabled && self.is_processing)
            || (!self.editing_project_output() && self.file_queue.selected_file_locked())
        {
            return false;
        }
        let Some(preset) = self
            .presets
            .iter()
            .find(|preset| preset.id == preset_id)
            .cloned()
        else {
            return false;
        };
        let metadata = if self.editing_project_output() {
            self.project_output_metadata()
        } else {
            self.selected_source_metadata()
        };
        if !crate::settings::preset_is_compatible(&preset, metadata.as_ref()) {
            return false;
        }
        let changed =
            self.update_selected_config(|config| apply_preset(config, &preset, metadata.as_ref()));
        let selection_changed = self.remember_export_preset(preset_id);
        if changed || selection_changed {
            self.settings_ui.preset_notice = Some(PresetNotice {
                text: format!("Applied {}", preset.name),
                tone: PresetNoticeTone::Success,
            });
        }
        changed || selection_changed
    }

    fn next_custom_preset_identity(&self) -> (String, u64) {
        let mut sequence = self.settings_ui.next_custom_preset_sequence;

        loop {
            sequence += 1;
            let id = format!("custom-preset-{sequence}");
            if !self.presets.iter().any(|preset| preset.id == id) {
                return (id, sequence);
            }
        }
    }
}

pub(super) fn system_color_theme(window: &Window) -> ColorTheme {
    match window.appearance() {
        gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => ColorTheme::Light,
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => ColorTheme::Dark,
    }
}
