//! One policy for project playhead selection. Time updates cross this seam;
//!
//! callers retain transport and pointer geometry. Snapshot updates only follow
//! selection after time changes, preserving explicit selection on paused frames.
use super::project_tracks::TrackKind;
use super::{Duration, PreviewPlaybackState, ScissorsRoot};

#[derive(Clone, Copy)]
pub(super) enum PlayheadChange {
    Seek(Option<usize>),
    Scrub,
    Snapshot,
    Handle,
}

impl ScissorsRoot {
    pub(super) fn update_project_playhead<T>(
        &mut self,
        trigger: PlayheadChange,
        update: impl FnOnce(&mut PreviewPlaybackState) -> T,
    ) -> (T, bool) {
        let previous = self.preview_ui.playback.current_time();
        let result = update(&mut self.preview_ui.playback);
        let follow = match trigger {
            PlayheadChange::Seek(_) | PlayheadChange::Scrub => true,
            PlayheadChange::Snapshot => {
                (self.preview_ui.playback.current_time() - previous).abs() > f64::EPSILON
            }
            PlayheadChange::Handle => false,
        };
        let lane = if let PlayheadChange::Seek(track) = trigger {
            track
        } else {
            None
        };
        let selected = follow && self.select_project_clip_at_playhead(lane);
        (result, selected)
    }

    /// Prefers a clip in the clicked lane, then the highest video clip at the playhead.
    /// Keeps the current selection when no eligible clip covers the playhead.
    fn select_project_clip_at_playhead(&mut self, track: Option<usize>) -> bool {
        if !self.project.enabled
            || self.preview_ui.crop_mode
            || self.preview_ui.overlay.overlay_mode()
        {
            return false;
        }
        let time = self.preview_ui.playback.current_time();
        let selected = self
            .project_timeline_clips()
            .into_iter()
            .filter(|(placement, source)| {
                let start = Duration::from_millis(source.offset_millis).as_secs_f64();
                let length = Duration::from_millis(
                    scissors_core::project_timeline::clip_duration_millis(source),
                )
                .as_secs_f64();
                let eligible = track == Some(placement.track)
                    || self.project.tracks[placement.track].kind == TrackKind::Video;
                eligible && time >= start && time < start + length
            })
            .max_by_key(|(placement, _)| (track == Some(placement.track), placement.track))
            .map(|(placement, _)| placement.id);
        selected.is_some_and(|id| {
            self.file_queue.selected_file_id() != Some(id.as_str()) && self.select_project_clip(&id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{PreviewMediaKind, SettingsTab, preview_playback_state};

    #[test]
    fn every_playhead_trigger_obeys_lane_gaps_snapshot_and_mode_policy() {
        let mut root = super::super::project::tests::project();
        assert!(root.add_library_clip_at("source", 0));
        let low = root.project.clips[0].id.clone();
        assert!(root.add_project_track(TrackKind::Video));
        assert!(root.add_library_clip_on_track("source", 0, 2));
        let high = root.project.clips[1].id.clone();
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 20.0, None, None);
        for trigger in [
            PlayheadChange::Seek(None),
            PlayheadChange::Scrub,
            PlayheadChange::Snapshot,
        ] {
            root.select_project_clip(&low);
            root.update_project_playhead(PlayheadChange::Handle, |playback| {
                playback.seek_once_to_percent(0.0)
            });
            assert!(
                root.update_project_playhead(trigger, |playback| playback
                    .seek_once_to_percent(0.25))
                    .1
            );
            assert_eq!(root.file_queue.selected_file_id(), Some(high.as_str()));
            assert_eq!(root.workspace_ui.tool, 1);
            assert_eq!(root.settings_ui.active_tab, SettingsTab::VideoFilters);
        }
        assert!(
            root.update_project_playhead(PlayheadChange::Seek(Some(0)), |playback| playback
                .seek_once_to_percent(0.25))
                .1
        );
        assert_eq!(root.file_queue.selected_file_id(), Some(low.as_str()));
        assert!(
            !root
                .update_project_playhead(PlayheadChange::Snapshot, |playback| playback
                    .seek_once_to_percent(0.25))
                .1
        );
        for percent in [0.75, 1.0] {
            assert!(
                !root
                    .update_project_playhead(PlayheadChange::Seek(None), |playback| playback
                        .seek_once_to_percent(percent))
                    .1
            );
            assert_eq!(root.file_queue.selected_file_id(), Some(low.as_str()));
        }
        root.preview_ui.crop_mode = true;
        assert!(
            !root
                .update_project_playhead(PlayheadChange::Scrub, |playback| playback
                    .seek_once_to_percent(0.25))
                .1
        );
        root.preview_ui.crop_mode = false;
        root.preview_ui
            .overlay
            .set_overlay_from_path("/tmp/logo.png", false);
        assert!(
            !root
                .update_project_playhead(PlayheadChange::Seek(None), |playback| playback
                    .seek_once_to_percent(0.3))
                .1
        );
        assert_eq!(root.file_queue.selected_file_id(), Some(low.as_str()));
    }
}
