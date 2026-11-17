//! Timeline editing owns linked instance/placement/metadata mutations, spacing and history.
//!
//! GPUI supplies project times and gesture tokens; it never coordinates snapshots.
use super::project::ClipPlacement;
use super::project_history::{TimelineHistory, TimelineSnapshot};
use super::project_tracks::{Track, TrackKind, TrackToggle, Tracks};
use super::*;
use crate::conversion_runner::core_config_from_gpui;
use scissors_core::types::TimelineSource;
use std::{cell::Cell, rc::Rc};

/// A gesture commits its first successful update immediately.
///
/// Dropping/cancelling a drag keeps those updates, as the original pointer paths did. Interleaving
/// another edit or undo ends its history group.
#[derive(Clone, Default)]
pub(super) struct EditGesture(Rc<Cell<Option<u64>>>);

#[derive(Clone, Copy)]
pub(super) enum TimelineEdit<'a> {
    Insert {
        source: &'a str,
        offset: Option<u64>,
        track: Option<usize>,
    },
    Remove(&'a str),
    Split,
    Cut {
        leading: bool,
    },
    Move {
        id: &'a str,
        offset: u64,
    },
    Trim {
        id: &'a str,
        leading: bool,
        time: f64,
        initial_start: f64,
        initial_offset: u64,
    },
    Speed {
        id: &'a str,
        speed: f64,
    },
    MoveToTrack {
        id: &'a str,
        track: usize,
    },
    AddTrack(TrackKind),
    RemoveTrack(usize),
    ToggleTrack(usize, TrackToggle),
    Reconcile,
}

pub(super) struct TimelineEditor<'a> {
    clips: &'a mut Vec<ClipPlacement>,
    tracks: &'a mut Tracks,
    history: &'a mut TimelineHistory,
    file_queue: &'a mut FileQueue,
    source_metadata: &'a mut SourceMetadataStore,
    sequence: &'a mut u64,
    library: &'a FileQueue,
    is_processing: bool,
    snap: bool,
    playhead: f64,
    export_open: bool,
}

impl ScissorsRoot {
    pub(super) const fn timeline_editor(&mut self) -> TimelineEditor<'_> {
        TimelineEditor {
            clips: &mut self.project.clips,
            tracks: &mut self.project.tracks,
            history: &mut self.project.history,
            file_queue: &mut self.file_queue,
            source_metadata: &mut self.source_metadata,
            sequence: &mut self.next_file_sequence,
            library: &self.project.library,
            is_processing: self.is_processing,
            snap: self.preview_ui.timeline_snap,
            playhead: self.preview_ui.playback.current_time(),
            export_open: self.export_ui.open,
        }
    }
}

impl TimelineEditor<'_> {
    pub(super) fn apply(&mut self, edit: TimelineEdit<'_>) -> bool {
        self.apply_grouped(edit, None)
    }

    pub(super) fn gesture(&mut self, gesture: &EditGesture, edit: TimelineEdit<'_>) -> bool {
        self.apply_grouped(edit, Some(gesture))
    }

    fn apply_grouped(&mut self, edit: TimelineEdit<'_>, gesture: Option<&EditGesture>) -> bool {
        if self.is_processing {
            return false;
        }
        let before = self.snapshot();
        let locking = matches!(edit, TimelineEdit::ToggleTrack(_, TrackToggle::Locked));
        let accepted = match edit {
            TimelineEdit::Insert {
                source,
                offset,
                track,
            } => self.insert_library_clip(source, offset, track),
            TimelineEdit::Remove(id) => self.remove_project_clip(id),
            TimelineEdit::Split => self.split_project_clip_at_playhead(),
            TimelineEdit::Cut { leading } => self.cut_project_track_at_playhead(leading),
            TimelineEdit::Move { id, offset } => self.move_project_clip(id, offset),
            TimelineEdit::Trim {
                id,
                leading,
                time,
                initial_start,
                initial_offset,
            } => self.trim_project_clip(id, leading, time, initial_start, initial_offset),
            TimelineEdit::Speed { id, speed } => self.set_project_clip_speed(id, speed),
            TimelineEdit::MoveToTrack { id, track } => self.move_project_clip_to_track(id, track),
            TimelineEdit::AddTrack(kind) => self.add_project_track(kind),
            TimelineEdit::RemoveTrack(index) => self.remove_project_track(index),
            TimelineEdit::ToggleTrack(index, toggle) => self.toggle_project_track(index, toggle),
            TimelineEdit::Reconcile => true,
        };
        if !accepted {
            return false;
        }
        if !locking {
            self.reconcile();
        }
        if self.snapshot() == before {
            return false;
        }
        if !locking {
            if gesture.is_none_or(|gesture| gesture.0.get() != Some(self.history.revision)) {
                self.history.push(before);
            }
            if let Some(gesture) = gesture {
                gesture.0.set(Some(self.history.revision));
            }
        }
        true
    }

    fn next_file_id(&mut self) -> String {
        *self.sequence += 1;
        format!("file-{}", self.sequence)
    }

    fn remove_instance(&mut self, id: &str) -> bool {
        if self.file_queue.remove_file(id).is_none() {
            return false;
        }
        self.clips.retain(|clip| clip.id != id);
        self.source_metadata.remove(id);
        true
    }

    /// Reconcile only tracks whose complete edited durations are known.
    fn reconcile(&mut self) -> bool {
        let sources = self.project_timeline_clips();
        let mut changed = false;
        for track in 0..self.tracks.len() {
            if self.tracks[track].locked {
                continue;
            }
            let mut clips: Vec<_> = sources
                .iter()
                .filter(|(clip, _)| clip.track == track)
                .cloned()
                .collect();
            if clips.len() != self.clips.iter().filter(|clip| clip.track == track).count() {
                continue;
            }
            clips.sort_by_key(|(clip, _)| clip.offset_millis);
            changed |= self.place_track_clips(track, clips, None);
        }
        changed
    }

    /// Preserves free-placement gaps and pushes colliding clips after their predecessor.
    fn place_track_clips(
        &mut self,
        track: usize,
        clips: Vec<(ClipPlacement, TimelineSource)>,
        moved: Option<(&str, u64)>,
    ) -> bool {
        let close_gaps = self.snap && track == self.tracks.primary();
        let mut end = 0_u64;
        let mut changed = false;
        for (clip, source) in clips {
            let requested = moved
                .filter(|(id, _)| *id == clip.id)
                .map_or(clip.offset_millis, |(_, offset)| offset);
            let offset = if close_gaps { end } else { requested.max(end) };
            let placement = self
                .clips
                .iter_mut()
                .find(|item| item.id == clip.id)
                .unwrap();
            changed |= placement.offset_millis != offset;
            placement.offset_millis = offset;
            end = offset.saturating_add(scissors_core::project_timeline::clip_duration_millis(
                &source,
            ));
        }
        changed
    }

    /// Inserts a moved clip between its neighbors without overlapping them.
    fn place_moved_project_clip(&mut self, id: &str, offset_millis: u64) -> bool {
        let Some(track) = self
            .clips
            .iter()
            .find(|clip| clip.id == id)
            .map(|clip| clip.track)
        else {
            return false;
        };
        let mut clips: Vec<_> = self
            .project_timeline_clips()
            .into_iter()
            .filter(|(clip, _)| clip.track == track)
            .collect();
        // Do not rearrange a track until every clip has a known duration.
        if clips.len() != self.clips.iter().filter(|clip| clip.track == track).count() {
            return false;
        }
        clips.sort_by_key(|(clip, _)| clip.offset_millis);
        let Some(index) = clips.iter().position(|(clip, _)| clip.id == id) else {
            return false;
        };
        let moved = clips.remove(index);
        let insertion = clips
            .iter()
            .position(|(clip, source)| {
                let midpoint = clip.offset_millis.saturating_add(
                    scissors_core::project_timeline::clip_duration_millis(source) / 2,
                );
                midpoint > offset_millis
            })
            .unwrap_or(clips.len());
        clips.insert(insertion, moved);
        self.place_track_clips(track, clips, Some((id, offset_millis)))
    }

    /// Settings alone historically do not enter history. Only changes that move
    /// placements record the configuration and all neighbor moves together.
    pub(super) fn settings(
        &mut self,
        id: &str,
        update: impl FnOnce(&mut ConversionConfig) -> bool,
    ) -> bool {
        if self.is_processing || self.project_clip_locked(id) {
            return false;
        }
        let before = self.snapshot();
        let Some(file) = self.file_queue.file_by_id_mut(id) else {
            return false;
        };
        let original = file.config.clone();
        if !update(&mut file.config) {
            file.config = original;
            return false;
        }
        if file.config == original {
            return false;
        }
        if self.reconcile() {
            self.history.push(before);
        } else {
            self.history.revision = self.history.revision.wrapping_add(1);
        }
        true
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
        let Some(mut file) = self.library.file_by_id(source_id).cloned() else {
            return false;
        };
        let Some(metadata) = self.source_metadata.metadata_for(source_id).cloned() else {
            return false;
        };
        let kind = metadata.source_kind();
        let track_kind = super::project_tracks::TrackKind::of(kind);
        let Some(track) = track
            .filter(|&index| {
                self.tracks
                    .get(index)
                    .is_some_and(|track| track.kind == track_kind && !track.locked)
            })
            .or_else(|| self.tracks.first_unlocked(track_kind))
        else {
            return false;
        };
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
                crate::numeric::rounded_f64_to_u64(self.playhead * 1000.0)
            } else {
                self.project_timeline_clips()
                    .iter()
                    .filter(|(placement, _)| placement.track == track)
                    .map(|(_, clip)| {
                        clip.offset_millis.saturating_add(
                            scissors_core::project_timeline::clip_duration_millis(clip),
                        )
                    })
                    .max()
                    .unwrap_or(0)
            }
        });
        file.id = self.next_file_id();
        file.output_name = "Untitled project".to_string();
        initialize_output_config(&mut file.config, Some(&metadata));
        file.config.end_time = Some(format_time(duration));
        let id = file.id.clone();
        self.source_metadata.mark_ready(id.clone(), metadata);
        self.file_queue.add_file(file);
        self.file_queue.select_existing_file(&id);
        self.clips.push(ClipPlacement {
            id,
            source_id: source_id.to_string(),
            offset_millis,
            kind,
            track,
            muted: false,
            speed: 1.0,
        });
        true
    }

    fn remove_project_clip(&mut self, id: &str) -> bool {
        if self.is_processing
            || self.project_clip_locked(id)
            || self.file_queue.file_by_id(id).is_none()
        {
            return false;
        }
        if !self.remove_instance(id) {
            return false;
        }
        self.clips.retain(|clip| clip.id != id);
        self.file_queue
            .select_file(self.clips.last().map(|clip| clip.id.clone()));
        true
    }

    fn split_project_clip_at_playhead(&mut self) -> bool {
        if self.is_processing || self.export_open {
            return false;
        }
        let Some(id) = self.file_queue.selected_file_id() else {
            return false;
        };
        if self.project_clip_locked(id) {
            return false;
        }
        let Some(index) = self.clips.iter().position(|clip| clip.id == id) else {
            return false;
        };
        let Some((playhead, split_time)) = self.project_clip_playhead_time(index) else {
            return false;
        };
        let mut right = self.clips[index].clone();
        let Some(mut file) = self.file_queue.file_by_id(&right.id).cloned() else {
            return false;
        };
        let Some(metadata) = self.source_metadata.metadata_for(&right.id).cloned() else {
            return false;
        };
        let split_time = format_time(split_time);
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
        self.clips.insert(index + 1, right);
        true
    }

    fn cut_project_track_at_playhead(&mut self, leading: bool) -> bool {
        if self.is_processing || self.export_open {
            return false;
        }
        let Some(track) = self.file_queue.selected_file_id().and_then(|id| {
            self.clips
                .iter()
                .find(|clip| clip.id == id)
                .map(|clip| clip.track)
        }) else {
            return false;
        };
        if self.tracks.get(track).is_none_or(|track| track.locked) {
            return false;
        }
        let Some((index, (playhead, source_time))) = self
            .clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip.track == track)
            .find_map(|(index, _)| Some((index, self.project_clip_playhead_time(index)?)))
        else {
            return false;
        };
        let id = self.clips[index].id.clone();
        if self.file_queue.file_by_id(&id).is_none() {
            return false;
        }
        let config = &mut self.file_queue.file_by_id_mut(&id).unwrap().config;
        if leading {
            config.start_time = Some(format_time(source_time));
            self.clips[index].offset_millis = crate::numeric::rounded_f64_to_u64(playhead * 1000.0);
        } else {
            config.end_time = Some(format_time(source_time));
        }
        self.file_queue.select_existing_file(&id);
        true
    }

    fn project_clip_playhead_time(&self, index: usize) -> Option<(f64, f64)> {
        let clip = self.clips.get(index)?;
        let source = self
            .project_timeline_clips()
            .into_iter()
            .find(|(placement, _)| placement.id == clip.id)?
            .1;
        let playhead = self.playhead;
        let offset = Duration::from_millis(clip.offset_millis).as_secs_f64();
        let length = scissors_core::clip_time::duration_seconds(&source);
        if !playhead.is_finite() || playhead - offset < 0.05 || offset + length - playhead < 0.05 {
            return None;
        }
        Some((
            playhead,
            scissors_core::clip_time::source_time(&source, playhead),
        ))
    }

    fn project_timeline_clips(&self) -> Vec<(ClipPlacement, scissors_core::types::TimelineSource)> {
        sources(
            self.clips,
            self.tracks,
            self.file_queue,
            self.source_metadata,
        )
    }

    fn trim_project_clip(
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
        let Some(index) = self.clips.iter().position(|clip| clip.id == id) else {
            return false;
        };
        let placement = &self.clips[index];
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
            let start = format_time(next);
            let next_offset = crate::numeric::rounded_f64_to_u64(
                (offset + (next - initial_start) / speed).max(0.0) * 1000.0,
            );
            if (parse_time_to_seconds(&start) - trim_start).abs() < f64::EPSILON
                && self.clips[index].offset_millis == next_offset
            {
                return false;
            }
            file.config.start_time = Some(start);
            self.clips[index].offset_millis = next_offset;
        } else {
            let next = (project_time - offset)
                .mul_add(speed, trim_start)
                .clamp(trim_start + 0.05, duration.max(trim_start + 0.05));
            let end = format_time(next);
            if (parse_time_to_seconds(&end) - trim_end).abs() < f64::EPSILON {
                return false;
            }
            file.config.end_time = Some(end);
        }
        true
    }

    fn set_project_clip_speed(&mut self, id: &str, speed: f64) -> bool {
        if self.is_processing
            || self.project_clip_locked(id)
            || !speed.is_finite()
            || !(0.5..=20.0).contains(&speed)
        {
            return false;
        }
        let Some(clip) = self.clips.iter_mut().find(|clip| clip.id == id) else {
            return false;
        };
        if (clip.speed - speed).abs() < f64::EPSILON {
            return false;
        }
        if let Some(clip) = self.clips.iter_mut().find(|clip| clip.id == id) {
            clip.speed = speed;
        }
        true
    }

    fn move_project_clip(&mut self, id: &str, offset_millis: u64) -> bool {
        if self.is_processing || self.project_clip_locked(id) {
            return false;
        }
        self.place_moved_project_clip(id, offset_millis)
    }

    fn add_project_track(&mut self, kind: TrackKind) -> bool {
        if self.is_processing {
            return false;
        }
        self.tracks.push(Track::new(kind));
        true
    }

    fn remove_project_track(&mut self, index: usize) -> bool {
        let Some(track) = self.tracks.get(index).copied() else {
            return false;
        };
        if self.is_processing || track.locked || self.tracks.count(track.kind) < 2 {
            return false;
        }
        let ids: Vec<_> = self
            .clips
            .iter()
            .filter(|clip| clip.track == index)
            .map(|clip| clip.id.clone())
            .collect();
        for id in ids {
            self.remove_instance(&id);
        }
        self.tracks.remove(index);
        for clip in self.clips.iter_mut() {
            if clip.track > index {
                clip.track -= 1;
            }
        }
        if self.file_queue.selected_file().is_none() {
            self.file_queue
                .select_file(self.clips.last().map(|clip| clip.id.clone()));
        }
        true
    }

    fn toggle_project_track(&mut self, index: usize, toggle: TrackToggle) -> bool {
        if self.is_processing || index >= self.tracks.len() {
            return false;
        }
        let track = &mut self.tracks[index];
        let flag = match toggle {
            TrackToggle::Hidden => &mut track.hidden,
            TrackToggle::Muted => &mut track.muted,
            TrackToggle::Locked => &mut track.locked,
        };
        *flag = !*flag;
        true
    }

    fn project_clip_locked(&self, id: &str) -> bool {
        clip_locked(self.clips, self.tracks, id)
    }

    fn move_project_clip_to_track(&mut self, id: &str, track: usize) -> bool {
        if self.is_processing || self.project_clip_locked(id) {
            return false;
        }
        let Some(target) = self.tracks.get(track).copied() else {
            return false;
        };
        let Some(clip) = self.clips.iter_mut().find(|clip| clip.id == id) else {
            return false;
        };
        if clip.track == track || target.locked || target.kind != TrackKind::of(clip.kind) {
            return false;
        }
        clip.track = track;
        true
    }

    fn snapshot(&self) -> TimelineSnapshot {
        TimelineSnapshot {
            clips: self.clips.clone(),
            tracks: self.tracks.clone(),
            instances: self.file_queue.files().to_vec(),
            metadata: self
                .clips
                .iter()
                .filter_map(|clip| {
                    Some((
                        clip.id.clone(),
                        self.source_metadata.metadata_for(&clip.id)?.clone(),
                    ))
                })
                .collect(),
            selected: self.file_queue.selected_file_id().map(str::to_string),
        }
    }

    pub(super) fn undo_timeline_edit(&mut self) -> bool {
        if self.is_processing {
            return false;
        }
        let Some(snapshot) = self.history.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.history.redo.push(current);
        self.history.revision = self.history.revision.wrapping_add(1);
        self.restore_snapshot(snapshot);
        true
    }

    pub(super) fn redo_timeline_edit(&mut self) -> bool {
        if self.is_processing {
            return false;
        }
        let Some(snapshot) = self.history.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.history.undo.push(current);
        self.history.revision = self.history.revision.wrapping_add(1);
        self.restore_snapshot(snapshot);
        true
    }

    fn restore_snapshot(&mut self, snapshot: TimelineSnapshot) {
        let mut instances = FileQueue::new();
        instances.add_files(snapshot.instances);
        instances.select_file(snapshot.selected);
        *self.file_queue = instances;
        for clip in self.clips.iter() {
            self.source_metadata.remove(&clip.id);
        }
        for (id, metadata) in snapshot.metadata {
            self.source_metadata.mark_ready(id, metadata);
        }
        *self.clips = snapshot.clips;
        *self.tracks = snapshot.tracks;
    }
}

pub(super) fn clip_locked(clips: &[ClipPlacement], tracks: &Tracks, id: &str) -> bool {
    clips
        .iter()
        .find(|clip| clip.id == id)
        .and_then(|clip| tracks.get(clip.track))
        .is_some_and(|track| track.locked)
}

pub(super) fn sources(
    clips: &[ClipPlacement],
    tracks: &Tracks,
    files: &FileQueue,
    metadata: &SourceMetadataStore,
) -> Vec<(ClipPlacement, scissors_core::types::TimelineSource)> {
    clips
        .iter()
        .filter_map(|placement| {
            let file = files.file_by_id(&placement.id)?;
            let metadata = metadata.metadata_for(&placement.id)?;
            let track = tracks.get(placement.track);
            let source = scissors_core::types::TimelineSource {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn selected_settings(
        root: &mut ScissorsRoot,
        update: impl FnOnce(&mut ConversionConfig) -> bool,
    ) -> bool {
        let id = root.file_queue.selected_file_id().unwrap().to_string();
        root.timeline_editor().settings(&id, update)
    }

    fn insert(root: &mut ScissorsRoot, offset: u64) -> String {
        assert!(root.timeline_editor().apply(TimelineEdit::Insert {
            source: "source",
            offset: Some(offset),
            track: None
        }));
        root.project.clips.last().unwrap().id.clone()
    }

    #[test]
    fn discrete_edits_restore_all_linked_state_and_ignore_rejections() {
        let mut root = super::super::project::tests::project();
        let first = insert(&mut root, 3_000);
        let second = insert(&mut root, 30_000);
        assert!(selected_settings(&mut root, |config| {
            config.audio_volume = 37;
            true
        }));
        let before = root.timeline_editor().snapshot();
        assert!(root.timeline_editor().apply(TimelineEdit::Speed {
            id: &first,
            speed: 0.5
        }));
        let after = root.timeline_editor().snapshot();
        assert!(root.timeline_editor().undo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == before);
        assert!(!root.timeline_editor().apply(TimelineEdit::Speed {
            id: &first,
            speed: 1.0
        }));
        assert!(
            !root
                .timeline_editor()
                .apply(TimelineEdit::Remove("missing"))
        );
        assert!(root.project.history.can_redo());
        assert!(root.timeline_editor().redo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == after);
        assert!(root.timeline_editor().apply(TimelineEdit::Remove(&second)));
        assert!(root.source_metadata.metadata_for(&second).is_none());
        assert!(root.timeline_editor().undo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == after);
        assert!(root.remove_library_source("source"));
        assert!(root.file_queue.file_by_id(&first).is_some());
    }

    #[test]
    fn gesture_groups_move_trim_cross_track_and_neighbor_placement() {
        let mut root = super::super::project::tests::project();
        let first = insert(&mut root, 0);
        let second = insert(&mut root, 10_000);
        assert!(
            root.timeline_editor()
                .apply(TimelineEdit::AddTrack(TrackKind::Video))
        );
        root.project.history.clear();
        let before = root.timeline_editor().snapshot();
        let gesture = EditGesture::default();
        for offset in [2_000, 4_000, 6_000] {
            assert!(
                root.timeline_editor()
                    .gesture(&gesture, TimelineEdit::Move { id: &first, offset })
            );
        }
        assert_eq!(root.project.clips[1].offset_millis, 16_000);
        assert!(root.timeline_editor().gesture(
            &gesture,
            TimelineEdit::MoveToTrack {
                id: &first,
                track: 2
            }
        ));
        assert!(root.timeline_editor().gesture(
            &gesture,
            TimelineEdit::Trim {
                id: &first,
                leading: true,
                time: 7.0,
                initial_start: 0.0,
                initial_offset: 6_000
            }
        ));
        assert!(root.timeline_editor().gesture(
            &gesture,
            TimelineEdit::Trim {
                id: &first,
                leading: false,
                time: 10.0,
                initial_start: 1.0,
                initial_offset: 7_000
            }
        ));
        let after = root.timeline_editor().snapshot();
        assert!(root.timeline_editor().undo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == before);
        assert!(!root.project.history.can_undo());
        assert!(root.timeline_editor().redo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == after);
        // Reusing a gesture after undo/redo starts a fresh group.
        assert!(root.timeline_editor().gesture(
            &gesture,
            TimelineEdit::Move {
                id: &second,
                offset: 40_000
            }
        ));
        assert!(root.timeline_editor().undo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == after);
    }

    #[test]
    fn rejected_and_noop_gestures_preserve_redo_and_locks() {
        let mut root = super::super::project::tests::project();
        let id = insert(&mut root, 0);
        assert!(
            root.timeline_editor()
                .apply(TimelineEdit::AddTrack(TrackKind::Video))
        );
        assert!(root.timeline_editor().undo_timeline_edit());
        let gesture = EditGesture::default();
        assert!(
            !root
                .timeline_editor()
                .gesture(&gesture, TimelineEdit::Move { id: &id, offset: 0 })
        );
        assert!(!root.timeline_editor().gesture(
            &gesture,
            TimelineEdit::Trim {
                id: &id,
                leading: false,
                time: 10.0,
                initial_start: 0.0,
                initial_offset: 0
            }
        ));
        assert!(
            !root
                .timeline_editor()
                .gesture(&gesture, TimelineEdit::MoveToTrack { id: &id, track: 1 })
        );
        root.project.tracks[0].locked = true;
        assert!(!root.timeline_editor().gesture(
            &gesture,
            TimelineEdit::Move {
                id: &id,
                offset: 2_000
            }
        ));
        assert!(!root.timeline_editor().apply(TimelineEdit::Speed {
            id: &id,
            speed: 2.0
        }));
        assert!(!selected_settings(&mut root, |config| {
            config.start_time = Some(format_time(1.0));
            true
        }));
        assert!(root.project.history.can_redo());
        root.project.tracks[0].locked = false;
        let before = root.timeline_editor().snapshot();
        assert!(!selected_settings(&mut root, |config| {
            config.end_time = Some(format_time(3.0));
            false
        }));
        assert!(root.timeline_editor().snapshot() == before);
        assert!(root.project.history.can_redo());
    }

    #[test]
    fn settings_record_only_placement_changes_and_undo_configuration_with_neighbors() {
        let mut root = super::super::project::tests::project();
        let first = insert(&mut root, 0);
        insert(&mut root, 10_000);
        root.file_queue.select_existing_file(&first);
        root.project.history.clear();
        assert!(selected_settings(&mut root, |config| {
            config.audio_volume = 37;
            true
        }));
        assert!(!root.project.history.can_undo());
        // Free placement keeps gaps, but expanding a clip must push its neighbor.
        assert!(root.timeline_editor().apply(TimelineEdit::Speed {
            id: &first,
            speed: 0.5
        }));
        root.project.history.clear();
        root.preview_ui.timeline_snap = true;
        let before = root.timeline_editor().snapshot();
        assert!(selected_settings(&mut root, |config| {
            config.end_time = Some(format_time(6.0));
            true
        }));
        assert_eq!(root.project.clips[1].offset_millis, 12_000);
        let after = root.timeline_editor().snapshot();
        assert!(root.timeline_editor().undo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == before);
        assert!(!selected_settings(&mut root, |_| true));
        root.normalize_selected_config();
        assert!(root.project.history.can_redo());
        assert!(root.timeline_editor().redo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == after);
    }

    #[test]
    fn split_source_instants_agree_with_renderer_including_millisecond_serialization() {
        for speed in [0.5, 1.0, 2.0, 20.0] {
            for elapsed in [0.8746, 0.875, 2.875, 3.3334] {
                let mut root = super::super::project::tests::project();
                let id = insert(&mut root, 3_125);
                selected_settings(&mut root, |config| {
                    config.start_time = Some("00:00:01.125".to_string());
                    config.end_time = Some("00:00:09.875".to_string());
                    config.cut_segments = [("2", "3"), ("5", "6.25")]
                        .into_iter()
                        .map(|(start, end)| crate::settings::CutSegment {
                            start_time: start.to_string(),
                            end_time: end.to_string(),
                        })
                        .collect();
                    true
                });
                root.timeline_editor()
                    .apply(TimelineEdit::Speed { id: &id, speed });
                let time = 3.125 + elapsed / speed;
                root.preview_ui.playback =
                    preview_playback_state(PreviewMediaKind::Video, 30.0, None, None);
                let _ = root.preview_ui.playback.seek_once_to_percent(time / 30.0);
                let actual_time = root.preview_ui.playback.current_time();
                let source = root.project_sources().remove(0);
                let expected = scissors_core::clip_time::source_time(&source, actual_time);
                let mut args = Vec::new();
                scissors_core::project_timeline::add_graph(
                    &mut args,
                    &[source],
                    &scissors_core::project_timeline::RenderWindow {
                        width: 64,
                        height: 36,
                        fps: 30,
                        start: actual_time,
                        end: actual_time + 0.01,
                        video: true,
                        audio: true,
                        realtime: false,
                    },
                );
                assert_eq!(&args[..2], &["-ss".to_string(), format!("{expected:.6}")]);
                if elapsed / speed < 0.05 {
                    // Rendering has no editing minimum; these fast seeks remain ineligible.
                    assert!(!root.timeline_editor().apply(TimelineEdit::Split));
                    continue;
                }
                assert!(
                    root.timeline_editor().apply(TimelineEdit::Split),
                    "speed={speed} elapsed={elapsed}"
                );
                let left = &root.file_queue.file_by_id(&id).unwrap().config;
                let right = &root
                    .file_queue
                    .file_by_id(&root.project.clips[1].id)
                    .unwrap()
                    .config;
                assert_eq!(
                    left.end_time.as_deref(),
                    Some(format_time(expected).as_str())
                );
                assert_eq!(left.end_time, right.start_time);
                assert!(
                    (parse_time_to_seconds(right.start_time.as_deref().unwrap()) - expected).abs()
                        <= 0.000_501
                );
            }
        }
    }

    #[test]
    fn cuts_and_removed_segments_restore_configuration_and_spacing_together() {
        for leading in [false, true] {
            let mut root = super::super::project::tests::project();
            let first = insert(&mut root, 0);
            insert(&mut root, 10_000);
            root.file_queue.select_existing_file(&first);
            root.preview_ui.timeline_snap = true;
            root.preview_ui.playback =
                preview_playback_state(PreviewMediaKind::Video, 20.0, None, None);
            let _ = root.preview_ui.playback.seek_once_to_percent(0.2);
            let before = root.timeline_editor().snapshot();
            assert!(root.timeline_editor().apply(TimelineEdit::Cut { leading }));
            assert_eq!(
                root.project.clips[1].offset_millis,
                if leading { 6_000 } else { 4_000 }
            );
            let after = root.timeline_editor().snapshot();
            assert!(root.timeline_editor().undo_timeline_edit());
            assert!(root.timeline_editor().snapshot() == before);
            assert!(root.timeline_editor().redo_timeline_edit());
            assert!(root.timeline_editor().snapshot() == after);
        }
        let mut root = super::super::project::tests::project();
        let first = insert(&mut root, 0);
        insert(&mut root, 10_000);
        root.file_queue.select_existing_file(&first);
        root.timeline_editor().apply(TimelineEdit::Speed {
            id: &first,
            speed: 2.0,
        });
        root.preview_ui.timeline_snap = true;
        let before = root.timeline_editor().snapshot();
        assert!(selected_settings(&mut root, |config| {
            config.cut_segments = vec![crate::settings::CutSegment {
                start_time: "2".to_string(),
                end_time: "4".to_string(),
            }];
            true
        }));
        assert_eq!(root.project.clips[1].offset_millis, 4_000);
        assert!(root.timeline_editor().undo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == before);
        assert!(root.project.history.can_redo());
        let output_before = root.file_queue.selected_file().unwrap().config.clone();
        root.export_ui.open = true;
        root.update_config(&EditTarget::Project, |config| {
            config.audio_volume = 22;
            true
        });
        assert_eq!(
            root.file_queue.selected_file().unwrap().config,
            output_before
        );
        assert!(root.project.history.can_redo());
        root.export_ui.open = false;
        assert!(root.timeline_editor().redo_timeline_edit());
        let gesture = EditGesture::default();
        assert!(root.timeline_editor().gesture(
            &gesture,
            TimelineEdit::Trim {
                id: &first,
                leading: false,
                time: 4.5,
                initial_start: 0.0,
                initial_offset: 0
            }
        ));
        assert!(root.timeline_editor().undo_timeline_edit());
        assert_eq!(root.project.clips[1].offset_millis, 4_000);
    }

    #[test]
    fn removing_track_restores_membership_properties_metadata_and_selection() {
        let mut root = super::super::project::tests::project();
        let id = insert(&mut root, 0);
        assert!(
            root.timeline_editor()
                .apply(TimelineEdit::AddTrack(TrackKind::Video))
        );
        assert!(
            root.timeline_editor()
                .apply(TimelineEdit::MoveToTrack { id: &id, track: 2 })
        );
        let before = root.timeline_editor().snapshot();
        assert!(root.timeline_editor().apply(TimelineEdit::RemoveTrack(2)));
        assert!(root.project.clips.is_empty());
        assert!(root.timeline_editor().undo_timeline_edit());
        assert!(root.timeline_editor().snapshot() == before);
        assert!(root.timeline_editor().redo_timeline_edit());
        assert!(root.project.clips.is_empty());
    }
}
