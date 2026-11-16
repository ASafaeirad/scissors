//! Settings and export choose their target independently of modal visibility.
use super::{
    ConversionConfig, FileItem, MetadataStatus, ScissorsRoot, SourceMetadata, SourceMetadataEntry,
    normalize_output_config,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum EditTarget {
    Project,
    Clip(String),
    /// Standalone sources are retained for visual fixtures and source-level tests.
    Source(String),
}

impl ScissorsRoot {
    pub(super) fn settings_target(&self) -> Option<EditTarget> {
        let id = self.file_queue.selected_file_id()?.to_string();
        Some(if self.project.enabled {
            EditTarget::Clip(id)
        } else {
            EditTarget::Source(id)
        })
    }

    pub(super) fn export_target(&self) -> Option<EditTarget> {
        if self.project.enabled {
            Some(EditTarget::Project)
        } else {
            self.settings_target()
        }
    }

    pub(super) fn config_for(&self, target: &EditTarget) -> Option<&ConversionConfig> {
        match target {
            EditTarget::Project => Some(&self.project.output),
            EditTarget::Clip(id) | EditTarget::Source(id) => {
                self.file_queue.file_by_id(id).map(|file| &file.config)
            }
        }
    }

    pub(super) fn config_for_mut(&mut self, target: &EditTarget) -> Option<&mut ConversionConfig> {
        match target {
            EditTarget::Project => Some(&mut self.project.output),
            EditTarget::Clip(id) | EditTarget::Source(id) => self
                .file_queue
                .file_by_id_mut(id)
                .map(|file| &mut file.config),
        }
    }

    pub(super) fn edit_target_locked(&self, target: &EditTarget) -> bool {
        if self.project.enabled && self.is_processing {
            return true;
        }
        match target {
            EditTarget::Project => false,
            EditTarget::Clip(id) => self.project_clip_locked(id),
            EditTarget::Source(id) => self
                .file_queue
                .file_by_id(id)
                .is_none_or(FileItem::locks_settings),
        }
    }

    pub(super) fn update_config(
        &mut self,
        target: &EditTarget,
        update: impl FnOnce(&mut ConversionConfig) -> bool,
    ) -> bool {
        if self.edit_target_locked(target) {
            return false;
        }
        let before = matches!(target, EditTarget::Clip(_)).then(|| self.timeline_snapshot());
        let changed = self.config_for_mut(target).is_some_and(update);
        if changed
            && let Some(before) = before
            && self.reconcile_track_spacing()
        {
            self.project.history.push(before);
        }
        changed
    }

    pub(super) fn metadata_entry_for(&self, target: &EditTarget) -> SourceMetadataEntry {
        match target {
            EditTarget::Project => SourceMetadataEntry {
                status: MetadataStatus::Ready,
                metadata: self.project_output_metadata(),
                error: None,
            },
            EditTarget::Clip(id) | EditTarget::Source(id) => self.source_metadata.entry_for(id),
        }
    }

    pub(super) fn metadata_for_target(&self, target: &EditTarget) -> Option<SourceMetadata> {
        self.metadata_entry_for(target).metadata
    }

    pub(super) fn normalize_config(&mut self, target: &EditTarget) -> bool {
        let metadata = self.metadata_for_target(target);
        self.update_config(target, |config| match target {
            EditTarget::Clip(_) => {
                crate::settings::normalize_clip_config(config, metadata.as_ref())
            }
            EditTarget::Project | EditTarget::Source(_) => {
                normalize_output_config(config, metadata.as_ref())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ScissorsTextInputKind, SourceKind, apply_fps, apply_trim_times, project};

    fn project_with_clip() -> ScissorsRoot {
        let mut root = project::tests::project();
        assert!(root.add_library_clip("source"));
        root
    }

    #[test]
    fn project_and_clip_use_the_same_config_and_metadata_interface() {
        let mut root = project_with_clip();
        let clip = root.settings_target().unwrap();
        for (target, fps) in [(&EditTarget::Project, "24"), (&clip, "30")] {
            assert!(root.update_config(target, |config| apply_fps(config, fps)));
            assert_eq!(root.config_for(target).unwrap().fps, fps);
            assert_eq!(
                root.metadata_entry_for(target).status,
                MetadataStatus::Ready
            );
            assert_eq!(root.metadata_for_target(target).unwrap().width, Some(320));
        }
        assert_eq!(root.config_for(&EditTarget::Project).unwrap().fps, "24");
        assert_eq!(root.config_for(&clip).unwrap().fps, "30");
        root.update_config(&clip, |config| {
            apply_trim_times(config, None, Some("00:00:05.000".to_string()))
        });
        assert_eq!(
            root.metadata_for_target(&EditTarget::Project)
                .unwrap()
                .duration
                .as_deref(),
            Some("5.000")
        );
        assert_eq!(
            root.metadata_for_target(&clip).unwrap().duration.as_deref(),
            Some("10")
        );
    }

    #[test]
    fn dialog_visibility_does_not_redirect_settings_or_preview() {
        let mut root = project_with_clip();
        let clip = root.settings_target().unwrap();
        root.update_config(&EditTarget::Project, |config| apply_fps(config, "60"));
        for open in [true, false, true] {
            if open {
                assert!(root.open_export_dialog());
            } else {
                root.close_export_dialog();
            }
            assert_eq!(root.settings_target(), Some(clip.clone()));
            assert!(root.update_selected_config(|config| apply_fps(
                config,
                if open { "24" } else { "30" }
            )));
            assert_eq!(root.config_for(&EditTarget::Project).unwrap().fps, "60");
            assert_eq!(
                root.selected_source_metadata(),
                root.metadata_for_target(&clip)
            );
        }
    }

    #[test]
    fn explicit_clip_target_survives_selection_changes_and_missing_ids_do_not_fall_back() {
        let mut root = project_with_clip();
        let first = root.settings_target().unwrap();
        root.add_library_clip("source");
        let second = root.settings_target().unwrap();
        assert_ne!(first, second);
        root.update_config(&first, |config| apply_fps(config, "24"));
        assert_ne!(root.config_for(&second).unwrap().fps, "24");
        let missing = EditTarget::Clip("missing".to_string());
        assert!(root.config_for(&missing).is_none());
        assert!(root.metadata_for_target(&missing).is_none());
        assert!(!root.update_config(&missing, |_| panic!("must not edit another target")));
    }

    #[test]
    fn export_text_fields_and_clip_text_fields_keep_their_targets() {
        let mut root = project_with_clip();
        let clip = root.settings_target().unwrap();
        for open in [false, true, false] {
            root.export_ui.open = open;
            root.write_text_input_value(ScissorsTextInputKind::AudioBitrate, "128");
            root.write_text_input_value(ScissorsTextInputKind::ExportAudioBitrate, "320");
            assert_eq!(
                root.text_input_value(ScissorsTextInputKind::AudioBitrate),
                "128"
            );
            assert_eq!(
                root.text_input_value(ScissorsTextInputKind::ExportAudioBitrate),
                "320"
            );
            assert_eq!(root.config_for(&clip).unwrap().audio_bitrate, "128");
            assert_eq!(root.project.output.audio_bitrate, "320");
        }
    }

    #[test]
    fn linked_export_dimensions_and_name_leave_the_clip_unchanged() {
        let mut root = project_with_clip();
        let clip = root.settings_target().unwrap();
        root.open_export_dialog();
        let before = root.config_for(&clip).unwrap().clone();
        let name = root.file_queue.selected_file().unwrap().output_name.clone();
        root.write_text_input_value(ScissorsTextInputKind::ExportVideoCustomWidth, "640");
        root.write_text_input_value(ScissorsTextInputKind::ExportOutputName, "render");
        assert_eq!(root.project.output.custom_width.as_deref(), Some("640"));
        assert_eq!(root.project.output.custom_height.as_deref(), Some("360"));
        assert_eq!(root.project.output_name, "render");
        assert_eq!(root.config_for(&clip).unwrap(), &before);
        assert_eq!(root.file_queue.selected_file().unwrap().output_name, name);
    }

    #[test]
    fn presets_and_their_remembered_selection_belong_to_the_target() {
        let mut root = project_with_clip();
        let clip = root.settings_target().unwrap();
        root.open_export_dialog();
        assert!(root.apply_preset_to_target(&EditTarget::Project, "web-share"));
        let output = root.project.output.clone();
        assert!(root.apply_preset_to_target(&clip, "balanced-mp4"));
        assert_eq!(root.project.output, output);
        root.close_export_dialog();
        assert!(root.apply_preset_to_target(&EditTarget::Project, "balanced-mp4"));
        assert!(root.apply_preset_to_target(&clip, "web-share"));
        assert_ne!(
            root.config_for(&clip),
            root.config_for(&EditTarget::Project)
        );
    }

    #[test]
    fn locked_clip_does_not_lock_project_output_and_processing_locks_both() {
        let mut root = project_with_clip();
        let clip = root.settings_target().unwrap();
        let track = root.project.clips[0].track;
        root.project.tracks.get_mut(track).unwrap().locked = true;
        assert!(!root.update_config(&clip, |config| apply_fps(config, "24")));
        assert!(root.update_config(&EditTarget::Project, |config| apply_fps(config, "24")));
        root.is_processing = true;
        assert!(!root.update_config(&EditTarget::Project, |config| apply_fps(config, "30")));
        assert!(!root.update_config(&clip, |config| apply_fps(config, "30")));
        assert!(
            root.write_text_input_value(ScissorsTextInputKind::ExportAudioBitrate, "320")
                .is_none()
        );
    }

    #[test]
    fn image_clip_normalization_keeps_timeline_duration_but_source_normalization_clears_it() {
        let mut root = project_with_clip();
        let clip = root.settings_target().unwrap();
        let EditTarget::Clip(id) = &clip else {
            unreachable!()
        };
        root.source_metadata.mark_ready(
            id,
            SourceMetadata {
                media_kind: Some(SourceKind::Image),
                ..Default::default()
            },
        );
        root.config_for_mut(&clip).unwrap().end_time = Some("00:00:05.000".to_string());
        root.normalize_config(&clip);
        assert_eq!(
            root.config_for(&clip).unwrap().end_time.as_deref(),
            Some("00:00:05.000")
        );
        assert!(!root.normalize_config(&clip));
        root.project.enabled = false;
        let source = root.settings_target().unwrap();
        root.normalize_config(&source);
        assert!(root.config_for(&source).unwrap().end_time.is_none());
    }
}
