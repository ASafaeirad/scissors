//! One project contains reusable media sources and independently edited timeline clips.
use super::*;
use crate::conversion_runner::core_config_from_gpui;

#[derive(Default)]
pub(super) struct Project {
    /// Normal startup always enables the project. Standalone source fixtures disable it.
    pub(super) enabled: bool,
    pub(super) library: FileQueue,
    pub(super) clips: Vec<ClipPlacement>,
    pub(super) tracks: super::project_tracks::Tracks,
    pub(super) history: super::project_history::TimelineHistory,
    pub(super) output: ConversionConfig,
    pub(super) output_name: String,
    /// The file this project was last saved to or opened from.
    pub(super) path: Option<PathBuf>,
    /// The project as last saved or opened, to detect unsaved changes.
    pub(super) saved: Option<super::project_file::ProjectFile>,
    /// The last save or open failure, shown in the project header.
    pub(super) error: Option<String>,
    /// Asking whether to discard unsaved changes before opening another project.
    pub(super) confirm_discard: bool,
}

#[derive(Clone, Debug)]
pub(super) struct ClipPlacement {
    pub(super) id: String,
    pub(super) source_id: String,
    pub(super) offset_millis: u64,
    pub(super) kind: SourceKind,
    /// Index into `Project::tracks`.
    pub(super) track: usize,
    pub(super) muted: bool,
    pub(super) speed: f64,
}

pub(super) struct ProjectRenderState {
    pub(super) library: FileQueue,
    pub(super) ready_sources: std::collections::HashSet<String>,
    pub(super) timeline: super::project_timeline::ProjectTimelineState,
}

impl FrameRoot {
    pub(super) fn project_render_state(&self, cx: &gpui::App) -> Option<ProjectRenderState> {
        if !self.project.enabled {
            return None;
        }
        Some(ProjectRenderState {
            library: self.project.library.clone(),
            ready_sources: self
                .project
                .library
                .files()
                .iter()
                .filter(|file| self.source_metadata.metadata_for(&file.id).is_some())
                .map(|file| file.id.clone())
                .collect(),
            timeline: super::project_timeline::ProjectTimelineState {
                scheme: super::m3::scheme(self.appearance.color_theme, cx),
                clips: self.project_timeline_clips(),
                tracks: self.project.tracks.clone(),
                files: self.file_queue.files().to_vec(),
                selected: self.file_queue.selected_file_id().map(str::to_string),
                duration: self.project_duration(),
                view: self.project_timeline_view(),
                current: self.preview_ui.playback.current_time(),
                enabled: !self.is_processing,
                snap: self.preview_ui.timeline_snap,
                can_undo: self.project.history.can_undo(),
                can_redo: self.project.history.can_redo(),
                playing: self.preview_ui.playback.is_playing(),
                menu: self.preview_ui.timeline_menu.clone(),
                ruler_width: self
                    .preview_ui
                    .timeline_track_bounds
                    .map(|bounds| bounds.size.width.as_f32()),
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
            SettingsTab::AudioFilters
        } else {
            SettingsTab::VideoFilters
        };
        true
    }

    pub(super) const fn editing_project_output(&self) -> bool {
        self.project.enabled && self.export_ui.open
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
        config.external_audio_tracks.clear();
        config.video_filters = frame_core::types::VideoFiltersConfig::default();
        config.audio_filters = frame_core::types::AudioFiltersConfig::default();
        config.processing_mode = "reencode".to_string();
        Some(config)
    }

    pub(super) fn add_library_clip(&mut self, source_id: &str) -> bool {
        self.insert_library_clip(source_id, None, None)
    }

    pub(super) fn add_library_clip_at(&mut self, source_id: &str, offset_millis: u64) -> bool {
        self.insert_library_clip(source_id, Some(offset_millis), None)
    }

    /// Adds a clip on `track`, or on the default track when `track` holds the other kind.
    pub(super) fn add_library_clip_on_track(
        &mut self,
        source_id: &str,
        offset_millis: u64,
        track: usize,
    ) -> bool {
        self.insert_library_clip(source_id, Some(offset_millis), Some(track))
    }

    fn insert_library_clip(
        &mut self,
        source_id: &str,
        offset_millis: Option<u64>,
        track: Option<usize>,
    ) -> bool {
        if self.is_processing {
            return false;
        }
        let Some(mut file) = self.project.library.file_by_id(source_id).cloned() else {
            return false;
        };
        let Some(metadata) = self.source_metadata.metadata_for(source_id).cloned() else {
            return false;
        };
        let kind = metadata.source_kind();
        let track_kind = super::project_tracks::TrackKind::of(kind);
        let Some(track) = track
            .filter(|&index| {
                self.project
                    .tracks
                    .get(index)
                    .is_some_and(|track| track.kind == track_kind && !track.locked)
            })
            .or_else(|| self.project.tracks.first_unlocked(track_kind))
        else {
            return false;
        };
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
        let offset_millis = offset_millis.unwrap_or_else(|| {
            if kind == SourceKind::Audio {
                crate::numeric::rounded_f64_to_u64(self.preview_ui.playback.current_time() * 1000.0)
            } else {
                self.project_timeline_clips()
                    .iter()
                    .filter(|(placement, _)| placement.track == track)
                    .map(|(_, clip)| {
                        clip.offset_millis.saturating_add(
                            frame_core::project_timeline::clip_duration_millis(clip),
                        )
                    })
                    .max()
                    .unwrap_or(0)
            }
        });
        self.record_timeline_edit();
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
            track,
            muted: false,
            speed: 1.0,
        });
        self.close_primary_track_gaps();
        self.zoom_to_first_project_clip();
        true
    }

    pub(super) fn remove_library_source(&mut self, id: &str) -> bool {
        if self.is_processing {
            return false;
        }
        // Instances keep their own path and metadata when a library entry is removed.
        let removed = self.project.library.remove_file(id).is_some();
        if removed {
            self.media_assets.thumbnails.remove(id);
        }
        removed
    }

    pub(super) fn remove_project_clip(&mut self, id: &str) -> bool {
        if self.is_processing
            || self.project_clip_locked(id)
            || self.file_queue.file_by_id(id).is_none()
        {
            return false;
        }
        self.record_timeline_edit();
        if !self.remove_file_from_queue(id) {
            return false;
        }
        self.project.clips.retain(|clip| clip.id != id);
        self.close_primary_track_gaps();
        self.file_queue
            .select_file(self.project.clips.last().map(|clip| clip.id.clone()));
        true
    }

    pub(super) fn split_project_clip_at_playhead(&mut self) -> bool {
        if self.is_processing || self.export_ui.open {
            return false;
        }
        let Some(id) = self.file_queue.selected_file_id() else {
            return false;
        };
        if self.project_clip_locked(id) {
            return false;
        }
        let Some(index) = self.project.clips.iter().position(|clip| clip.id == id) else {
            return false;
        };
        let Some((playhead, split_time)) = self.project_clip_playhead_time(index) else {
            return false;
        };
        let mut right = self.project.clips[index].clone();
        let Some(mut file) = self.file_queue.file_by_id(&right.id).cloned() else {
            return false;
        };
        let Some(metadata) = self.source_metadata.metadata_for(&right.id).cloned() else {
            return false;
        };
        let split_time = format_time(split_time);
        self.record_timeline_edit();
        let left_id = right.id.clone();
        file.id = self.next_file_id();
        file.config.start_time = Some(split_time.clone());
        right.id.clone_from(&file.id);
        right.offset_millis = crate::numeric::rounded_f64_to_u64(playhead * 1000.0);
        self.file_queue
            .file_by_id_mut(&left_id)
            .unwrap()
            .config
            .end_time = Some(split_time);
        self.source_metadata.mark_ready(right.id.clone(), metadata);
        self.file_queue.add_file(file);
        self.project.clips.insert(index + 1, right);
        self.close_primary_track_gaps();
        true
    }

    /// Cuts the clip under the playhead on the selected clip's track, removing the part
    /// before the playhead (`leading`) or after it. The rest of the clip stays in place.
    pub(super) fn cut_project_track_at_playhead(&mut self, leading: bool) -> bool {
        if self.is_processing || self.export_ui.open {
            return false;
        }
        let Some(track) = self.file_queue.selected_file_id().and_then(|id| {
            self.project
                .clips
                .iter()
                .find(|clip| clip.id == id)
                .map(|clip| clip.track)
        }) else {
            return false;
        };
        if self
            .project
            .tracks
            .get(track)
            .is_none_or(|track| track.locked)
        {
            return false;
        }
        let Some((index, (playhead, source_time))) = self
            .project
            .clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip.track == track)
            .find_map(|(index, _)| Some((index, self.project_clip_playhead_time(index)?)))
        else {
            return false;
        };
        let id = self.project.clips[index].id.clone();
        if self.file_queue.file_by_id(&id).is_none() {
            return false;
        }
        self.record_timeline_edit();
        let config = &mut self.file_queue.file_by_id_mut(&id).unwrap().config;
        if leading {
            config.start_time = Some(format_time(source_time));
            self.project.clips[index].offset_millis =
                crate::numeric::rounded_f64_to_u64(playhead * 1000.0);
        } else {
            config.end_time = Some(format_time(source_time));
        }
        self.file_queue.select_existing_file(&id);
        self.close_primary_track_gaps();
        true
    }

    /// The playhead and the source time under it, when the playhead is far enough inside
    /// the clip at `index` to leave a usable clip on each side.
    fn project_clip_playhead_time(&self, index: usize) -> Option<(f64, f64)> {
        let clip = self.project.clips.get(index)?;
        let file = self.file_queue.file_by_id(&clip.id)?;
        let metadata = self.source_metadata.metadata_for(&clip.id)?;
        let config = core_config_from_gpui(&file.config);
        let trim_start = file
            .config
            .start_time
            .as_deref()
            .map_or(0.0, parse_time_to_seconds);
        let duration = if clip.kind == SourceKind::Image {
            5.0
        } else {
            preview_duration_seconds(Some(metadata))
        };
        let playhead = self.preview_ui.playback.current_time();
        let offset = Duration::from_millis(clip.offset_millis).as_secs_f64();
        let length = frame_core::cuts::output_duration_seconds(&config)
            .unwrap_or(duration - trim_start)
            / clip.speed;
        if !playhead.is_finite() || playhead - offset < 0.05 || offset + length - playhead < 0.05 {
            return None;
        }
        let mut source_time = (playhead - offset) * clip.speed;
        for cut in frame_core::cuts::output_cut_ranges(&config) {
            if cut.start > source_time {
                break;
            }
            source_time += cut.duration();
        }
        Some((playhead, trim_start + source_time))
    }

    /// Sources in composition order: audio and lower video tracks first, so higher video
    /// tracks draw over them.
    pub(super) fn project_sources(&self) -> Vec<frame_core::types::TimelineSource> {
        let mut clips = self.project_timeline_clips();
        clips.sort_by_key(|(placement, _)| {
            (placement.kind != SourceKind::Audio).then_some(placement.track)
        });
        clips.into_iter().map(|(_, source)| source).collect()
    }

    /// Each placed clip with the source it renders, in clip order. Clips whose instance or
    /// metadata is missing are left out.
    pub(super) fn project_timeline_clips(
        &self,
    ) -> Vec<(ClipPlacement, frame_core::types::TimelineSource)> {
        self.project
            .clips
            .iter()
            .filter_map(|placement| {
                let file = self.file_queue.file_by_id(&placement.id)?;
                let metadata = self.source_metadata.metadata_for(&placement.id)?;
                let track = self.project.tracks.get(placement.track);
                let source = frame_core::types::TimelineSource {
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
                    muted: placement.muted || track.is_some_and(|track| track.muted),
                    speed: placement.speed,
                    hidden: track.is_some_and(|track| track.hidden),
                    config: Box::new(core_config_from_gpui(&file.config)),
                };
                Some((placement.clone(), source))
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
        if self.is_processing || !project_time.is_finite() || self.project_clip_locked(id) {
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
        self.close_primary_track_gaps();
        true
    }

    pub(super) fn set_project_clip_speed(&mut self, id: &str, speed: f64) -> bool {
        if self.is_processing || !speed.is_finite() || !(0.5..=20.0).contains(&speed) {
            return false;
        }
        let Some(clip) = self.project.clips.iter_mut().find(|clip| clip.id == id) else {
            return false;
        };
        if (clip.speed - speed).abs() < f64::EPSILON {
            return false;
        }
        self.record_timeline_edit();
        if let Some(clip) = self.project.clips.iter_mut().find(|clip| clip.id == id) {
            clip.speed = speed;
        }
        self.close_primary_track_gaps();
        true
    }

    /// Moves a clip without recording history. Callers record once per gesture.
    pub(super) fn move_project_clip(&mut self, id: &str, offset_millis: u64) -> bool {
        if self.is_processing || self.project_clip_locked(id) {
            return false;
        }
        let Some(clip) = self.project.clips.iter_mut().find(|clip| clip.id == id) else {
            return false;
        };
        if clip.offset_millis == offset_millis {
            return false;
        }
        clip.offset_millis = offset_millis;
        self.close_primary_track_gaps();
        true
    }
}

#[cfg(test)]
pub(in crate::app) mod tests {
    use super::*;

    pub(in crate::app) fn project() -> FrameRoot {
        let mut root = FrameRoot::new();
        root.project.enabled = true;
        // Most edit tests exercise free placement; primary snapping tests opt in.
        root.preview_ui.timeline_snap = false;
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
    fn splitting_preserves_trim_speed_settings_and_project_duration() {
        let mut root = project();
        root.add_library_clip("source");
        let id = root.project.clips[0].id.clone();
        root.move_project_clip(&id, 3_000);
        root.set_project_clip_speed(&id, 2.0);
        root.update_selected_config(|config| {
            config.start_time = Some(format_time(2.0));
            config.audio_volume = 25;
            config.cut_segments = vec![crate::settings::CutSegment {
                start_time: format_time(4.0),
                end_time: format_time(6.0),
            }];
            true
        });
        root.preview_ui.playback = preview_playback_state(PreviewMediaKind::Video, 6.0, None, None);
        let _ = root.preview_ui.playback.seek_once_to_percent(0.75);
        let duration = root.project_duration();
        assert!(root.split_project_clip_at_playhead());
        assert_eq!(root.project.clips.len(), 2);
        let sources = root.project_sources();
        assert_eq!(sources[0].config.end_time.as_deref(), Some("00:00:07.000"));
        assert_eq!(
            sources[1].config.start_time.as_deref(),
            Some("00:00:07.000")
        );
        assert_eq!(sources[1].offset_millis, 4_500);
        assert!((sources[1].config.audio_volume - 25.0).abs() < f64::EPSILON);
        assert!((sources[1].speed - 2.0).abs() < f64::EPSILON);
        assert_eq!(root.project.clips[1].source_id, "source");
        assert!((root.project_duration() - duration).abs() < 0.001);
        assert!(!root.split_project_clip_at_playhead());
    }

    #[test]
    fn splitting_ignores_boundaries_gaps_and_processing() {
        let mut root = project();
        root.add_library_clip_at("source", 2_000);
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 12.0, None, None);
        for time in [0.0, 1.0, 2.0, 12.0] {
            let _ = root.preview_ui.playback.seek_once_to_percent(time / 12.0);
            assert!(!root.split_project_clip_at_playhead());
        }
        let _ = root.preview_ui.playback.seek_once_to_percent(0.5);
        root.is_processing = true;
        assert!(!root.split_project_clip_at_playhead());
        assert_eq!(root.project.clips.len(), 1);
    }

    #[test]
    fn cutting_to_beginning_and_end_trims_the_clip_under_the_playhead_on_the_track() {
        let mut root = project();
        root.add_library_clip("source");
        root.add_library_clip("source");
        let first = root.project.clips[0].id.clone();
        let second = root.project.clips[1].id.clone();
        root.set_project_clip_speed(&second, 2.0);
        // Select the first clip: the cut still applies to whichever clip on its track is
        // under the playhead.
        root.select_project_clip(&first);
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 20.0, None, None);
        let _ = root.preview_ui.playback.seek_once_to_percent(0.6);
        assert!(root.cut_project_track_at_playhead(true));
        assert_eq!(root.file_queue.selected_file_id(), Some(second.as_str()));
        assert_eq!(root.project.clips[1].offset_millis, 12_000);
        let config = &root.file_queue.file_by_id(&second).unwrap().config;
        assert_eq!(config.start_time.as_deref(), Some("00:00:04.000"));

        let _ = root.preview_ui.playback.seek_once_to_percent(0.25);
        assert!(root.cut_project_track_at_playhead(false));
        assert_eq!(root.file_queue.selected_file_id(), Some(first.as_str()));
        assert_eq!(root.project.clips[0].offset_millis, 0);
        let config = &root.file_queue.file_by_id(&first).unwrap().config;
        assert_eq!(config.end_time.as_deref(), Some("00:00:05.000"));
        assert_eq!(config.start_time, None);
        assert!(root.undo_timeline_edit());
        assert!(root.undo_timeline_edit());
        assert!((root.project_duration() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cutting_a_track_ignores_boundaries_locked_tracks_and_processing() {
        let mut root = project();
        root.add_library_clip_at("source", 2_000);
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 12.0, None, None);
        for time in [0.0, 1.0, 2.0, 12.0] {
            let _ = root.preview_ui.playback.seek_once_to_percent(time / 12.0);
            assert!(!root.cut_project_track_at_playhead(true));
            assert!(!root.cut_project_track_at_playhead(false));
        }
        let _ = root.preview_ui.playback.seek_once_to_percent(0.5);
        root.project.tracks[0].locked = true;
        assert!(!root.cut_project_track_at_playhead(true));
        root.project.tracks[0].locked = false;
        root.is_processing = true;
        assert!(!root.cut_project_track_at_playhead(false));
        root.is_processing = false;
        assert!(root.cut_project_track_at_playhead(false));
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
        let tasks = root.prepare_project_export();
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
    fn app_starts_with_one_empty_project() {
        let root = FrameRoot::new();
        assert!(root.project.enabled);
        assert!(root.project.clips.is_empty());
        assert!(root.project.library.files().is_empty());
    }

    #[test]
    fn pending_files_cannot_be_exported_as_a_batch() {
        let mut root = FrameRoot::new();
        root.default_output_directory = Some(PathBuf::from("/tmp"));
        root.file_queue
            .add_file(FileItem::from_path("one", "/tmp/one.mp4", 0));
        root.file_queue
            .add_file(FileItem::from_path("two", "/tmp/two.mp4", 0));
        assert!(root.prepare_project_export().is_empty());
        assert!(
            root.file_queue
                .files()
                .iter()
                .all(|file| file.status == FileStatus::Idle)
        );
    }

    #[test]
    fn exporting_blocks_another_export_and_project_replacement() {
        let mut root = project();
        root.default_output_directory = Some(PathBuf::from("/tmp"));
        root.add_library_clip("source");
        let tasks = root.prepare_project_export();
        assert_eq!(tasks.len(), 1);
        root.is_processing = true;
        assert!(root.prepare_project_export().is_empty());
        assert!(
            root.replace_project(
                PathBuf::from("/tmp/other.frame"),
                super::super::project_file::ProjectFile::default(),
            )
            .is_none()
        );
        assert_eq!(root.project.clips.len(), 1);
        root.apply_conversion_event(ConversionEvent::completed(
            "project-export",
            "/tmp/result.mp4",
        ));
        assert!(!root.is_processing);
        assert_eq!(root.prepare_project_export().len(), 1);
        assert_eq!(
            root.file_queue
                .files()
                .iter()
                .filter(|file| file.id == "project-export")
                .count(),
            1
        );
    }

    #[test]
    fn fixture_has_reusable_sources_and_consecutive_video() {
        let mut root = FrameRoot::new();
        root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
        assert_eq!(root.project.library.files().len(), 2);
        assert_eq!(root.project.clips.len(), 4);
        assert_eq!(root.project.tracks.len(), 4);
        assert!(!root.project.history.can_undo());
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
