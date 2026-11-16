//! Undo and redo for timeline edits: adding, moving, trimming, splitting, and removing clips,
//! and changing tracks. Each step is a snapshot of the timeline, not of the library or output.
use super::project::ClipPlacement;
use super::project_tracks::Tracks;
use super::{FileItem, ScissorsRoot, SourceMetadata};

/// The most steps kept in each direction.
const HISTORY_LIMIT: usize = 100;

#[derive(Clone, PartialEq)]
pub(super) struct TimelineSnapshot {
    pub(super) clips: Vec<ClipPlacement>,
    pub(super) tracks: Tracks,
    /// Every clip instance with its own settings.
    pub(super) instances: Vec<FileItem>,
    pub(super) metadata: Vec<(String, SourceMetadata)>,
    pub(super) selected: Option<String>,
}

#[derive(Clone, Default)]
pub(super) struct TimelineHistory {
    pub(super) revision: u64,
    pub(super) undo: Vec<TimelineSnapshot>,
    pub(super) redo: Vec<TimelineSnapshot>,
}

impl TimelineHistory {
    pub(super) fn clear(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.undo.clear();
        self.redo.clear();
    }

    pub(super) fn push(&mut self, snapshot: TimelineSnapshot) {
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.revision = self.revision.wrapping_add(1);
        self.undo.push(snapshot);
        self.redo.clear();
    }

    pub(super) const fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(super) const fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl ScissorsRoot {
    pub(super) fn undo_timeline_edit(&mut self) -> bool {
        let changed = self.timeline_editor().undo_timeline_edit();
        if changed && let Some(id) = self.file_queue.selected_file_id().map(str::to_string) {
            self.select_project_clip(&id);
        }
        changed
    }
    pub(super) fn redo_timeline_edit(&mut self) -> bool {
        let changed = self.timeline_editor().redo_timeline_edit();
        if changed && let Some(id) = self.file_queue.selected_file_id().map(str::to_string) {
            self.select_project_clip(&id);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{PreviewMediaKind, format_time, preview_playback_state};

    #[test]
    fn undo_and_redo_step_through_clip_edits() {
        let mut root = crate::app::project::tests::project();
        assert!(!root.undo_timeline_edit());
        assert!(root.add_library_clip("source"));
        let id = root.project.clips[0].id.clone();
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 10.0, None, None);
        let _ = root.preview_ui.playback.seek_once_to_percent(0.4);
        assert!(root.split_project_clip_at_playhead());
        assert_eq!(root.project.clips.len(), 2);

        assert!(root.undo_timeline_edit());
        assert_eq!(root.project.clips.len(), 1);
        assert_eq!(root.file_queue.files().len(), 1);
        assert_eq!(
            root.file_queue.file_by_id(&id).unwrap().config.end_time,
            Some(format_time(10.0))
        );
        assert!(root.undo_timeline_edit());
        assert!(root.project.clips.is_empty());
        assert!(!root.undo_timeline_edit());

        assert!(root.redo_timeline_edit());
        assert!(root.redo_timeline_edit());
        assert_eq!(root.project.clips.len(), 2);
        assert!(
            root.source_metadata
                .metadata_for(&root.project.clips[1].id)
                .is_some()
        );
        assert!(!root.redo_timeline_edit());
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut root = crate::app::project::tests::project();
        assert!(root.add_library_clip("source"));
        assert!(root.undo_timeline_edit());
        assert!(root.project.history.can_redo());
        assert!(root.add_library_clip("source"));
        assert!(!root.project.history.can_redo());
    }

    #[test]
    fn history_keeps_a_bounded_number_of_steps() {
        let mut root = crate::app::project::tests::project();
        for _ in 0..HISTORY_LIMIT + 5 {
            assert!(root.add_project_track(super::super::project_tracks::TrackKind::Video));
        }
        assert_eq!(root.project.history.undo.len(), HISTORY_LIMIT);
    }
}
