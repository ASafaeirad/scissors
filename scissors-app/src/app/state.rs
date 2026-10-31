use super::*;

impl ScissorsRoot {
    #[must_use]
    pub fn new() -> Self {
        Self::new_inner(None, AppSettings::default(), AppNotifier::disabled())
    }

    #[must_use]
    pub fn new_with_platform_persistence() -> Self {
        let notifier = AppNotifier::system();
        match AppPersistence::platform() {
            Ok(persistence) => Self::new_with_persistence_and_notifier(persistence, notifier),
            Err(_) => Self::new_inner(None, AppSettings::default(), notifier),
        }
    }

    pub fn load_runtime_capabilities(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let detected = cx
                .background_spawn(async {
                    (detect_available_encoders(), detect_available_filters())
                })
                .await;

            this.update(cx, |root, cx| {
                match detected.0 {
                    Ok(encoders) => root.available_encoders = encoders,
                    Err(error) => {
                        eprintln!("Failed to detect FFmpeg encoder capabilities: {error}");
                    }
                }
                match detected.1 {
                    Ok(filters) => root.available_filters = filters,
                    Err(error) => eprintln!("Failed to detect FFmpeg filter capabilities: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    #[cfg(test)]
    pub(crate) fn new_with_notifier(notifier: AppNotifier) -> Self {
        Self::new_inner(None, AppSettings::default(), notifier)
    }

    #[cfg(test)]
    pub(crate) fn new_with_persistence(persistence: AppPersistence) -> Self {
        Self::new_with_persistence_and_notifier(persistence, AppNotifier::disabled())
    }

    fn new_with_persistence_and_notifier(
        persistence: AppPersistence,
        notifier: AppNotifier,
    ) -> Self {
        let settings = persistence.load().unwrap_or_default();
        Self::new_inner(Some(persistence), settings, notifier)
    }

    fn new_inner(
        persistence: Option<AppPersistence>,
        mut persisted_settings: AppSettings,
        notifier: AppNotifier,
    ) -> Self {
        if let Some(color_theme) = crate::appearance::color_theme_from_env_value(
            std::env::var("SCISSORS_VISUAL_THEME").ok().as_deref(),
        ) {
            persisted_settings.appearance.color_theme = color_theme;
            persisted_settings.appearance.follow_system_theme = false;
        }
        let conversion_processes = ConversionProcessController::default();
        // Ignore parallelism saved by older versions. The app exports one project at a time.
        conversion_processes
            .update_max_concurrency(1)
            .expect("one export is a valid limit");
        let presets = merged_presets(persisted_settings.custom_presets);
        let settings_ui = SettingsUiState {
            next_custom_preset_sequence: next_custom_preset_sequence(&presets),
            ..SettingsUiState::default()
        };

        let mut root = Self {
            active_view: ActiveView::Workspace,
            appearance: persisted_settings.appearance,
            titlebar_drag: TitlebarDragState::default(),
            fps_meter: None,
            focus_registry: ScissorsFocusRegistry::default(),
            file_queue: FileQueue::new(),
            project: project::Project {
                enabled: true,
                ..project::Project::default()
            },
            conversion_events: ConversionEventState::new(),
            is_processing: false,
            settings_ui,
            tooltip_ui: TooltipUiState::default(),
            drag_drop_ui: DragDropUiState::default(),
            default_output_directory: persisted_settings.default_output_directory,
            text_input_ui: ScissorsTextInputUiState::default(),
            source_metadata: SourceMetadataStore::default(),
            conversion_processes,
            available_encoders: AvailableEncoders::default(),
            available_filters: AvailableFilters::default(),
            active_conversion_task_ids: Vec::new(),
            notifier,
            presets,
            preview_ui: PreviewUiState::default(),
            next_file_sequence: 0,
            persistence,
            m3_gallery: None,
            workspace_ui: workspace::WorkspaceState::default(),
            media_assets: media_assets::MediaAssetsState::default(),
            export_ui: export_dialog::ExportUiState::default(),
        };

        root.apply_visual_fixture(visual_fixture_from_env_value(
            std::env::var("SCISSORS_GPUI_VISUAL_FIXTURE")
                .ok()
                .as_deref(),
        ));
        if let Some(ui_scale) = crate::appearance::scale_preset_from_env_value(
            std::env::var("SCISSORS_GPUI_UI_SCALE").ok().as_deref(),
        ) {
            root.appearance.ui_scale = ui_scale;
        }
        root
    }
    pub(super) fn app_state(&self) -> ScissorsAppState {
        ScissorsAppState::from_file_queue(
            self.active_view,
            self.is_processing,
            self.default_output_directory.is_some(),
            &self.file_queue,
        )
    }
    pub(super) fn selected_config(&self) -> Option<&ConversionConfig> {
        if self.editing_project_output() {
            Some(&self.project.output)
        } else {
            self.file_queue.selected_file().map(|file| &file.config)
        }
    }
    pub(super) fn update_selected_config(
        &mut self,
        update: impl FnOnce(&mut ConversionConfig) -> bool,
    ) -> bool {
        if self.project.enabled && self.is_processing {
            return false;
        }
        let before = (self.project.enabled
            && self.preview_ui.timeline_snap
            && !self.editing_project_output())
        .then(|| self.timeline_snapshot());
        let changed = self.selected_config_mut().is_some_and(update);
        if changed
            && let Some(before) = before
            && self.close_primary_track_gaps()
        {
            self.project.history.push(before);
        }
        changed
    }
    pub(super) fn normalize_selected_config(&mut self, metadata: Option<&SourceMetadata>) -> bool {
        if self.editing_project_output() {
            let metadata = self.project_output_metadata();
            return self.update_selected_config(|config| {
                normalize_output_config(config, metadata.as_ref())
            });
        }
        let project_enabled = self.project.enabled;
        self.update_selected_config(|config| {
            let trim = (
                config.start_time.clone(),
                config.end_time.clone(),
                config.cut_segments.clone(),
            );
            let changed = normalize_output_config(config, metadata);
            if project_enabled {
                (config.start_time, config.end_time, config.cut_segments) = trim;
            }
            changed
        })
    }

    pub(super) fn persist_app_settings(
        &self,
    ) -> Result<(), crate::app_persistence::AppPersistenceError> {
        let Some(persistence) = &self.persistence else {
            return Ok(());
        };

        persistence.save(&AppSettings::from_runtime(
            self.appearance,
            1,
            self.default_output_directory.clone(),
            &self.presets,
        ))
    }
}

impl Default for ScissorsRoot {
    fn default() -> Self {
        Self::new()
    }
}

fn merged_presets(custom_presets: Vec<PresetDefinition>) -> Vec<PresetDefinition> {
    let mut presets = default_presets();

    for preset in custom_presets {
        if !presets.iter().any(|existing| existing.id == preset.id) {
            presets.push(preset);
        }
    }

    presets
}

fn next_custom_preset_sequence(presets: &[PresetDefinition]) -> u64 {
    presets
        .iter()
        .filter(|preset| !preset.built_in)
        .filter_map(|preset| {
            preset
                .id
                .strip_prefix("custom-preset-")
                .and_then(|suffix| suffix.parse::<u64>().ok())
        })
        .max()
        .unwrap_or(0)
}
