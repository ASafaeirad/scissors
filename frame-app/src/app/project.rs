//! Project media are reusable sources; queue items are independent clip instances.
use super::*;
use crate::conversion_runner::core_config_from_gpui;

#[derive(Default)]
pub(super) struct Project {
    pub(super) enabled: bool,
    pub(super) library: FileQueue,
    pub(super) clips: Vec<ClipPlacement>,
    pub(super) output: ConversionConfig,
    pub(super) output_name: String,
}

#[derive(Clone, Debug)]
pub(super) struct ClipPlacement {
    pub(super) id: String,
    pub(super) source_id: String,
    pub(super) offset_millis: u64,
    pub(super) kind: SourceKind,
    pub(super) muted: bool,
    pub(super) speed: f64,
}

pub(super) struct ProjectRenderState {
    pub(super) library: FileQueue,
    pub(super) exports: FileQueue,
    pub(super) ready_sources: std::collections::HashSet<String>,
    pub(super) timeline: super::project_timeline::ProjectTimelineState,
}

impl FrameRoot {
    pub(super) fn project_render_state(&self) -> Option<ProjectRenderState> {
        if !self.project.enabled {
            return None;
        }
        let mut exports = FileQueue::new();
        for id in &self.export_ui.task_ids {
            if let Some(file) = self.file_queue.file_by_id(id) {
                exports.add_file(file.clone());
            }
        }
        Some(ProjectRenderState {
            library: self.project.library.clone(),
            exports,
            ready_sources: self
                .project
                .library
                .files()
                .iter()
                .filter(|file| self.source_metadata.metadata_for(&file.id).is_some())
                .map(|file| file.id.clone())
                .collect(),
            timeline: super::project_timeline::ProjectTimelineState {
                scheme: crate::material::scheme(self.appearance.color_theme),
                clips: self.project.clips.clone(),
                sources: self.project_sources(),
                files: self.file_queue.files().to_vec(),
                selected: self.file_queue.selected_file_id().map(str::to_string),
                duration: self.project_duration(),
                current: self.preview_ui.playback.current_time(),
                enabled: !self.is_processing,
            },
        })
    }

    pub(super) fn select_project_clip(&mut self, id: &str) -> bool {
        if !self.select_workspace_file(id) {
            return false;
        }
        let audio = self
            .project
            .clips
            .iter()
            .any(|clip| clip.id == id && clip.kind == SourceKind::Audio);
        self.workspace_ui.tool = if audio { 2 } else { 1 };
        self.settings_ui.active_tab = if audio {
            SettingsTab::Audio
        } else {
            SettingsTab::VideoFilters
        };
        true
    }

    pub(super) const fn editing_project_output(&self) -> bool {
        self.project.enabled && (self.export_ui.open || self.workspace_ui.tool == 4)
    }

    pub(super) fn project_output_metadata(&self) -> Option<SourceMetadata> {
        let placement = self
            .project
            .clips
            .iter()
            .find(|clip| clip.kind != SourceKind::Audio)
            .or_else(|| self.project.clips.first())?;
        let mut metadata = self.source_metadata.metadata_for(&placement.id)?.clone();
        metadata.duration = Some(format!("{:.3}", self.project_duration()));
        if placement.kind == SourceKind::Image {
            metadata.media_kind = Some(SourceKind::Video);
        }
        Some(metadata)
    }

    pub(super) fn selected_config_mut(&mut self) -> Option<&mut ConversionConfig> {
        if self.editing_project_output() {
            Some(&mut self.project.output)
        } else {
            self.file_queue
                .selected_file_mut()
                .map(|file| &mut file.config)
        }
    }

    pub(super) fn select_library_source(&mut self, id: &str) -> bool {
        if self.project.enabled {
            self.project.library.select_existing_file(id)
        } else {
            self.select_workspace_file(id)
        }
    }

    pub(super) fn project_config(&self) -> Option<frame_core::types::ConversionConfig> {
        self.file_queue.selected_file()?;
        let mut config = core_config_from_gpui(&self.project.output);
        config.timeline_sources = self.project_sources();
        config.start_time = None;
        config.end_time = Some(format!("{:.3}", self.project_duration()));
        config.cut_segments.clear();
        config.crop = None;
        config.overlay = None;
        config.rotation = "0".to_string();
        config.flip_horizontal = false;
        config.flip_vertical = false;
        config.selected_audio_tracks.clear();
        config.selected_subtitle_tracks.clear();
        config.external_audio_tracks.clear();
        config.external_subtitle_tracks.clear();
        config.video_filters = frame_core::types::VideoFiltersConfig::default();
        config.audio_filters = frame_core::types::AudioFiltersConfig::default();
        config.processing_mode = "reencode".to_string();
        Some(config)
    }

    pub(super) fn add_library_clip(&mut self, source_id: &str) -> bool {
        if self.is_processing || self.update_installation_in_progress() {
            return false;
        }
        let Some(mut file) = self.project.library.file_by_id(source_id).cloned() else {
            return false;
        };
        let Some(metadata) = self.source_metadata.metadata_for(source_id).cloned() else {
            return false;
        };
        let kind = metadata.source_kind();
        if self.project.clips.is_empty()
            || (kind != SourceKind::Audio
                && self
                    .project
                    .clips
                    .iter()
                    .all(|clip| clip.kind == SourceKind::Audio))
        {
            let mut output_metadata = metadata.clone();
            // Still images become finite visual clips in the project.
            if kind == SourceKind::Image {
                output_metadata.media_kind = Some(SourceKind::Video);
            }
            initialize_output_config(&mut self.project.output, Some(&output_metadata));
            if self.project.output_name.is_empty() {
                self.project.output_name = "Untitled project".to_string();
            }
        }
        let duration = if kind == SourceKind::Image {
            5.0
        } else {
            preview_duration_seconds(Some(&metadata))
        };
        if duration <= 0.0 {
            return false;
        }
        let offset_millis = if kind == SourceKind::Audio {
            crate::numeric::rounded_f64_to_u64(self.preview_ui.playback.current_time() * 1000.0)
        } else {
            self.project_sources()
                .iter()
                .filter(|clip| clip.kind != "audio")
                .map(|clip| {
                    clip.offset_millis
                        .saturating_add(frame_core::project_timeline::clip_duration_millis(clip))
                })
                .max()
                .unwrap_or(0)
        };
        file.id = self.next_file_id();
        file.output_name = "Untitled project".to_string();
        initialize_output_config(&mut file.config, Some(&metadata));
        file.config.end_time = Some(format_time(duration));
        let id = file.id.clone();
        self.source_metadata.mark_ready(id.clone(), metadata);
        self.file_queue.add_file(file);
        self.file_queue.select_existing_file(&id);
        self.project.clips.push(ClipPlacement {
            id,
            source_id: source_id.to_string(),
            offset_millis,
            kind,
            muted: false,
            speed: 1.0,
        });
        true
    }

    pub(super) fn remove_library_source(&mut self, id: &str) -> bool {
        if self.is_processing || self.update_installation_in_progress() {
            return false;
        }
        // Instances keep their own path and metadata when a library entry is removed.
        self.project.library.remove_file(id).is_some()
    }

    pub(super) fn remove_project_clip(&mut self, id: &str) -> bool {
        if self.is_processing || self.update_installation_in_progress() {
            return false;
        }
        if !self.remove_file_from_queue(id) {
            return false;
        }
        self.project.clips.retain(|clip| clip.id != id);
        self.file_queue
            .select_file(self.project.clips.last().map(|clip| clip.id.clone()));
        true
    }

    pub(super) fn project_sources(&self) -> Vec<frame_core::types::TimelineSource> {
        self.project
            .clips
            .iter()
            .filter_map(|placement| {
                let file = self.file_queue.file_by_id(&placement.id)?;
                let metadata = self.source_metadata.metadata_for(&placement.id)?;
                Some(frame_core::types::TimelineSource {
                    path: file.path.clone(),
                    kind: match placement.kind {
                        SourceKind::Video => "video",
                        SourceKind::Audio => "audio",
                        SourceKind::Image => "image",
                    }
                    .to_string(),
                    offset_millis: placement.offset_millis,
                    duration_millis: crate::numeric::rounded_f64_to_u64(
                        if placement.kind == SourceKind::Image {
                            5.0
                        } else {
                            preview_duration_seconds(Some(metadata))
                        } * 1000.0,
                    ),
                    muted: placement.muted,
                    speed: placement.speed,
                    config: Box::new(core_config_from_gpui(&file.config)),
                })
            })
            .collect()
    }

    pub(super) fn project_duration(&self) -> f64 {
        frame_core::project_timeline::duration_seconds(&self.project_sources())
    }

    pub(super) fn trim_project_clip(
        &mut self,
        id: &str,
        leading: bool,
        project_time: f64,
        initial_start: f64,
        initial_offset: u64,
    ) -> bool {
        if self.is_processing || self.update_installation_in_progress() || !project_time.is_finite()
        {
            return false;
        }
        let Some(index) = self.project.clips.iter().position(|clip| clip.id == id) else {
            return false;
        };
        let placement = &self.project.clips[index];
        let Some(metadata) = self.source_metadata.metadata_for(id) else {
            return false;
        };
        let duration = if placement.kind == SourceKind::Image {
            5.0
        } else {
            preview_duration_seconds(Some(metadata))
        };
        let offset = std::time::Duration::from_millis(initial_offset).as_secs_f64();
        let speed = placement.speed;
        let Some(file) = self.file_queue.file_by_id_mut(id) else {
            return false;
        };
        let trim_start = file
            .config
            .start_time
            .as_deref()
            .map_or(0.0, parse_time_to_seconds);
        let trim_end = file
            .config
            .end_time
            .as_deref()
            .map_or(duration, parse_time_to_seconds);
        if leading {
            let lower = offset.mul_add(-speed, initial_start).max(0.0);
            let next = (project_time - offset)
                .mul_add(speed, initial_start)
                .clamp(lower, (trim_end - 0.05).max(lower));
            file.config.start_time = Some(format_time(next));
            self.project.clips[index].offset_millis = crate::numeric::rounded_f64_to_u64(
                (offset + (next - initial_start) / speed).max(0.0) * 1000.0,
            );
        } else {
            let next = (project_time - offset)
                .mul_add(speed, trim_start)
                .clamp(trim_start + 0.05, duration.max(trim_start + 0.05));
            file.config.end_time = Some(format_time(next));
        }
        true
    }

    pub(super) fn set_project_clip_speed(&mut self, id: &str, speed: f64) -> bool {
        if self.is_processing
            || self.update_installation_in_progress()
            || !speed.is_finite()
            || !(0.5..=20.0).contains(&speed)
        {
            return false;
        }
        let Some(clip) = self.project.clips.iter_mut().find(|clip| clip.id == id) else {
            return false;
        };
        if (clip.speed - speed).abs() < f64::EPSILON {
            return false;
        }
        clip.speed = speed;
        true
    }

    pub(super) fn move_project_clip(&mut self, id: &str, offset_millis: u64) -> bool {
        if self.is_processing || self.update_installation_in_progress() {
            return false;
        }
        let Some(clip) = self.project.clips.iter_mut().find(|clip| clip.id == id) else {
            return false;
        };
        if clip.offset_millis == offset_millis {
            return false;
        }
        clip.offset_millis = offset_millis;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> FrameRoot {
        let mut root = FrameRoot::new();
        root.project.enabled = true;
        root.project
            .library
            .add_file(FileItem::from_path("source", "/tmp/example.mp4", 1));
        root.source_metadata.mark_ready(
            "source",
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                duration: Some("10".to_string()),
                width: Some(320),
                height: Some(180),
                audio_tracks: vec![crate::settings::AudioTrack {
                    index: 1,
                    codec: "aac".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        root
    }

    #[test]
    fn library_selection_does_not_switch_or_create_timeline_instances() {
        let mut root = project();
        assert!(root.select_library_source("source"));
        assert!(root.file_queue.files().is_empty());
        assert!(root.add_library_clip("source"));
        let selected = root.file_queue.selected_file_id().unwrap().to_string();
        assert!(root.select_library_source("source"));
        assert_eq!(root.file_queue.selected_file_id(), Some(selected.as_str()));
    }

    #[test]
    fn repeated_sources_append_and_own_independent_properties() {
        let mut root = project();
        assert!(root.add_library_clip("source"));
        assert!(root.add_library_clip("source"));
        let first = root.project.clips[0].id.clone();
        let second = root.project.clips[1].id.clone();
        assert_ne!(first, second);
        assert_eq!(root.project.clips[1].offset_millis, 10_000);
        root.update_selected_config(|config| {
            config.audio_volume = 25;
            true
        });
        assert_eq!(
            root.file_queue
                .file_by_id(&first)
                .unwrap()
                .config
                .audio_volume,
            100
        );
        assert_eq!(
            root.file_queue
                .file_by_id(&second)
                .unwrap()
                .config
                .audio_volume,
            25
        );
        assert_eq!(
            root.project
                .library
                .file_by_id("source")
                .unwrap()
                .config
                .audio_volume,
            100
        );
        assert!(root.move_project_clip(&second, 15_000));
        assert!((root.project_duration() - 25.0).abs() < f64::EPSILON);
        assert_eq!(root.project.clips[0].offset_millis, 0);
    }

    #[test]
    fn speed_is_independent_and_changes_project_preview_and_export() {
        let mut root = project();
        root.add_library_clip("source");
        root.add_library_clip("source");
        let first = root.project.clips[0].id.clone();
        let second = root.project.clips[1].id.clone();
        assert!(root.set_project_clip_speed(&second, 0.5));
        assert!((root.project_sources()[0].speed - 1.0).abs() < f64::EPSILON);
        assert!((root.project_sources()[1].speed - 0.5).abs() < f64::EPSILON);
        assert!((root.project_duration() - 30.0).abs() < f64::EPSILON);
        assert!(
            (root.project_config().unwrap().timeline_sources[1].speed - 0.5).abs() < f64::EPSILON
        );
        assert!(root.set_project_clip_speed(&second, 20.0));
        assert!((root.project_duration() - 10.5).abs() < f64::EPSILON);
        assert!(!root.set_project_clip_speed(&first, 20.1));
        assert!(!root.set_project_clip_speed(&first, f64::NAN));
        root.is_processing = true;
        assert!(!root.set_project_clip_speed(&first, 2.0));
    }

    #[test]
    fn trimming_a_fast_track_uses_source_time_and_keeps_its_trailing_edge() {
        let mut root = project();
        root.add_library_clip("source");
        let id = root.project.clips[0].id.clone();
        root.set_project_clip_speed(&id, 2.0);
        assert!(root.trim_project_clip(&id, true, 1.0, 0.0, 0));
        assert_eq!(root.project.clips[0].offset_millis, 1_000);
        assert_eq!(
            root.file_queue
                .file_by_id(&id)
                .unwrap()
                .config
                .start_time
                .as_deref(),
            Some("00:00:02.000")
        );
        assert!((root.project_duration() - 5.0).abs() < f64::EPSILON);
        assert!(root.trim_project_clip(&id, false, 4.0, 2.0, 1_000));
        assert!((root.project_duration() - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn removing_library_source_preserves_instances() {
        let mut root = project();
        root.add_library_clip("source");
        assert!(root.remove_library_source("source"));
        assert_eq!(root.project_sources().len(), 1);
        assert!(root.remove_project_clip(&root.project.clips[0].id.clone()));
        assert!(root.project_sources().is_empty());
    }

    #[test]
    fn project_export_is_one_job_and_keeps_clip_settings() {
        let mut root = project();
        root.default_output_directory = Some(PathBuf::from("/tmp"));
        root.add_library_clip("source");
        root.add_library_clip("source");
        let before = root.file_queue.files().to_vec();
        root.export_ui.open = true;
        root.update_selected_config(|config| {
            config.container = "mkv".to_string();
            true
        });
        assert_eq!(&root.file_queue.files()[..2], &before);
        let tasks = root.queue_selected_conversion_tasks();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].config.timeline_sources.len(), 2);
        assert_eq!(tasks[0].config.container, "mkv");
        assert_eq!(&root.file_queue.files()[..2], &before);
        assert_eq!(
            root.file_queue.selected_file_id(),
            Some(before[1].id.as_str())
        );
    }

    #[test]
    fn fixture_has_reusable_sources_and_consecutive_video() {
        let mut root = FrameRoot::new();
        root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
        assert_eq!(root.project.library.files().len(), 2);
        assert_eq!(root.project.clips.len(), 3);
        assert!(root.project.clips[1].offset_millis > 0);
        assert_eq!(
            root.project.clips[0].source_id,
            root.project.clips[1].source_id
        );
    }
    #[test]
    fn project_workspace_renders_while_root_is_being_updated() {
        let mut app = gpui::TestAppContext::single();
        let (_, cx) = app.add_window_view(|_, _| {
            let mut root = project();
            root.active_view = ActiveView::Workspace;
            root
        });
        cx.update(|window, cx| window.draw(cx).clear());
    }
    #[test]
    fn leading_trim_moves_only_its_clip_and_keeps_the_trailing_edge() {
        let mut root = project();
        root.add_library_clip("source");
        root.add_library_clip("source");
        let id = root.project.clips[1].id.clone();
        assert!(root.trim_project_clip(&id, true, 12.0, 0.0, 10_000));
        assert_eq!(root.project.clips[1].offset_millis, 12_000);
        assert_eq!(
            root.file_queue
                .file_by_id(&id)
                .unwrap()
                .config
                .start_time
                .as_deref(),
            Some("00:00:02.000")
        );
        assert!((root.project_duration() - 20.0).abs() < f64::EPSILON);
        assert_eq!(root.project.clips[0].offset_millis, 0);
    }
}
