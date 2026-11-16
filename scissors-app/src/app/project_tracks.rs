//! Timeline tracks hold clips of one kind. Video tracks stack upward: V2 draws over V1.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TrackKind {
    Video,
    Audio,
}

impl TrackKind {
    /// Images are visual clips, so they share video tracks.
    pub(super) fn of(kind: SourceKind) -> Self {
        if kind == SourceKind::Audio {
            Self::Audio
        } else {
            Self::Video
        }
    }

    const fn letter(self) -> &'static str {
        match self {
            Self::Video => "V",
            Self::Audio => "A",
        }
    }

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Track {
    pub(super) kind: TrackKind,
    /// Hidden video tracks leave the picture; their audio still plays.
    pub(super) hidden: bool,
    pub(super) muted: bool,
    /// Locked tracks keep their clips from being moved, trimmed, split, or removed.
    pub(super) locked: bool,
}

impl Track {
    pub(super) const fn new(kind: TrackKind) -> Self {
        Self {
            kind,
            hidden: false,
            muted: false,
            locked: false,
        }
    }
}

/// The project's tracks, indexed by `ClipPlacement::track`. Starts with one video and one
/// audio track, and never drops below one of each.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Tracks(Vec<Track>);

impl Default for Tracks {
    fn default() -> Self {
        Self(vec![
            Track::new(TrackKind::Video),
            Track::new(TrackKind::Audio),
        ])
    }
}

impl std::ops::Deref for Tracks {
    type Target = Vec<Track>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Tracks {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Tracks {
    /// Restores saved tracks, adding a missing video or audio track.
    pub(super) fn from_saved(tracks: Vec<Track>) -> Self {
        let mut tracks = Self(tracks);
        for kind in [TrackKind::Video, TrackKind::Audio] {
            if !tracks.iter().any(|track| track.kind == kind) {
                tracks.push(Track::new(kind));
            }
        }
        tracks
    }

    /// Video 1 is always the primary track, including after track removal or loading.
    pub(super) fn primary(&self) -> usize {
        self.iter()
            .position(|track| track.kind == TrackKind::Video)
            .unwrap()
    }

    /// The 1-based number of a track among tracks of its kind.
    pub(super) fn number(&self, index: usize) -> usize {
        let kind = self[index].kind;
        self[..=index]
            .iter()
            .filter(|track| track.kind == kind)
            .count()
    }

    /// The short badge label, like `V1`.
    pub(super) fn label(&self, index: usize) -> String {
        format!("{}{}", self[index].kind.letter(), self.number(index))
    }

    /// The spoken name, like `Video 1`.
    pub(super) fn name(&self, index: usize) -> String {
        format!("{} {}", self[index].kind.name(), self.number(index))
    }

    /// Top-to-bottom display order: video tracks from the highest down to V1, then audio
    /// tracks from A1 down.
    pub(super) fn display_order(&self) -> Vec<usize> {
        let of = |kind| {
            self.iter()
                .enumerate()
                .filter(move |(_, track)| track.kind == kind)
                .map(|(index, _)| index)
        };
        of(TrackKind::Video)
            .rev()
            .chain(of(TrackKind::Audio))
            .collect()
    }

    /// The lowest unlocked track of `kind`, where new clips land by default.
    pub(super) fn first_unlocked(&self, kind: TrackKind) -> Option<usize> {
        self.iter()
            .position(|track| track.kind == kind && !track.locked)
    }

    pub(super) fn count(&self, kind: TrackKind) -> usize {
        self.iter().filter(|track| track.kind == kind).count()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TrackToggle {
    Hidden,
    Muted,
    Locked,
}

impl ScissorsRoot {
    pub(super) fn add_project_track(&mut self, kind: TrackKind) -> bool {
        self.timeline_editor()
            .apply(super::timeline_edit::TimelineEdit::AddTrack(kind))
    }

    /// Removes a track and its clips. The last track of a kind stays.
    pub(super) fn remove_project_track(&mut self, index: usize) -> bool {
        // UI caches and conversion logs are outside timeline history.
        let ids: Vec<_> = self
            .project
            .clips
            .iter()
            .filter(|clip| clip.track == index)
            .map(|clip| clip.id.clone())
            .collect();
        let changed = self
            .timeline_editor()
            .apply(super::timeline_edit::TimelineEdit::RemoveTrack(index));
        if changed {
            for id in ids {
                self.media_assets.thumbnails.remove(&id);
                self.conversion_events.remove_logs(&id);
            }
        }
        changed
    }

    pub(super) fn toggle_project_track(&mut self, index: usize, toggle: TrackToggle) -> bool {
        self.timeline_editor()
            .apply(super::timeline_edit::TimelineEdit::ToggleTrack(
                index, toggle,
            ))
    }

    pub(super) fn project_clip_locked(&self, id: &str) -> bool {
        super::timeline_edit::clip_locked(&self.project.clips, &self.project.tracks, id)
    }

    /// Moves a clip to another unlocked track of its kind.
    pub(super) fn move_project_clip_to_track(&mut self, id: &str, track: usize) -> bool {
        self.timeline_editor()
            .apply(super::timeline_edit::TimelineEdit::MoveToTrack { id, track })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(kinds: &[TrackKind]) -> Tracks {
        Tracks(kinds.iter().copied().map(Track::new).collect())
    }

    #[test]
    fn labels_number_tracks_within_their_kind() {
        let tracks = tracks(&[
            TrackKind::Video,
            TrackKind::Audio,
            TrackKind::Video,
            TrackKind::Audio,
        ]);
        let labels: Vec<_> = (0..4).map(|index| tracks.label(index)).collect();
        assert_eq!(labels, ["V1", "A1", "V2", "A2"]);
        assert_eq!(tracks.name(2), "Video 2");
    }

    #[test]
    fn primary_is_video_one_even_when_audio_precedes_it() {
        let mut tracks = tracks(&[TrackKind::Audio, TrackKind::Video, TrackKind::Video]);
        assert_eq!(tracks.primary(), 1);
        assert_eq!(tracks.label(tracks.primary()), "V1");
        tracks.remove(1);
        assert_eq!(tracks.primary(), 1);
        assert_eq!(tracks.label(tracks.primary()), "V1");
    }

    #[test]
    fn video_tracks_stack_upward_above_audio() {
        let tracks = tracks(&[
            TrackKind::Video,
            TrackKind::Audio,
            TrackKind::Video,
            TrackKind::Audio,
        ]);
        assert_eq!(tracks.display_order(), [2, 0, 1, 3]);
    }

    #[test]
    fn restored_tracks_keep_one_of_each_kind() {
        assert_eq!(Tracks::from_saved(Vec::new()), Tracks::default());
        let restored = Tracks::from_saved(vec![Track::new(TrackKind::Audio)]);
        assert_eq!(restored.count(TrackKind::Video), 1);
        assert_eq!(restored.count(TrackKind::Audio), 1);
    }

    #[test]
    fn new_clips_skip_locked_tracks() {
        let mut tracks = tracks(&[TrackKind::Video, TrackKind::Video]);
        tracks[0].locked = true;
        assert_eq!(tracks.first_unlocked(TrackKind::Video), Some(1));
        tracks[1].locked = true;
        assert_eq!(tracks.first_unlocked(TrackKind::Video), None);
    }

    #[test]
    fn removing_a_track_removes_its_clips_and_shifts_later_tracks() {
        let mut root = crate::app::project::tests::project();
        assert!(root.add_project_track(TrackKind::Video));
        assert!(root.add_project_track(TrackKind::Video));
        assert!(root.add_library_clip("source"));
        let on_v2 = root.project.clips[0].id.clone();
        assert!(root.move_project_clip_to_track(&on_v2, 2));
        assert!(root.add_library_clip("source"));
        let on_v3 = root.project.clips[1].id.clone();
        assert!(root.move_project_clip_to_track(&on_v3, 3));

        assert!(root.remove_project_track(2));
        assert_eq!(root.project.tracks.len(), 3);
        assert!(root.file_queue.file_by_id(&on_v2).is_none());
        assert_eq!(root.project.clips.len(), 1);
        assert_eq!(root.project.clips[0].track, 2);
        assert_eq!(root.file_queue.selected_file_id(), Some(on_v3.as_str()));

        // The audio track is the last of its kind.
        assert!(!root.remove_project_track(1));
    }

    #[test]
    fn clips_only_move_to_unlocked_tracks_of_their_kind() {
        let mut root = crate::app::project::tests::project();
        assert!(root.add_library_clip("source"));
        let id = root.project.clips[0].id.clone();
        assert!(!root.move_project_clip_to_track(&id, 1), "audio track");
        assert!(root.add_project_track(TrackKind::Video));
        root.project.tracks[2].locked = true;
        assert!(!root.move_project_clip_to_track(&id, 2), "locked target");
        root.project.tracks[2].locked = false;
        root.project.tracks[0].locked = true;
        assert!(!root.move_project_clip_to_track(&id, 2), "locked source");
        assert!(!root.move_project_clip(&id, 5_000));
        assert!(!root.remove_project_clip(&id));
        root.project.tracks[0].locked = false;
        assert!(root.move_project_clip_to_track(&id, 2));
    }

    #[test]
    fn hidden_and_muted_tracks_reach_the_composition() {
        let mut root = crate::app::project::tests::project();
        assert!(root.add_library_clip("source"));
        assert!(root.toggle_project_track(0, TrackToggle::Hidden));
        assert!(root.toggle_project_track(0, TrackToggle::Muted));
        let source = &root.project_sources()[0];
        assert!(source.hidden);
        assert!(source.muted);
    }

    #[test]
    fn higher_video_tracks_composite_over_lower_ones() {
        let mut root = crate::app::project::tests::project();
        assert!(root.add_project_track(TrackKind::Video));
        assert!(root.add_library_clip("source"));
        assert!(root.add_library_clip("source"));
        let top = root.project.clips[0].id.clone();
        assert!(root.move_project_clip_to_track(&top, 2));
        let sources = root.project_sources();
        // The V2 clip comes last, so it overlays V1.
        assert_eq!(sources[0].offset_millis, 10_000);
        assert_eq!(sources[1].offset_millis, 0);
    }
}
