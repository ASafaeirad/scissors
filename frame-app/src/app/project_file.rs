//! Saving and opening projects.
//!
//! A project file stores the library, every timeline clip with its own settings, and the
//! project output as versioned JSON. Media are referenced by path, not copied. Metadata and
//! thumbnails are probed again on open, so a moved or deleted source shows up as an error clip.
use super::m3::M3Text;
use super::project::{ClipPlacement, Project};
use super::project_tracks::{Track, TrackKind, Tracks};
use super::*;
use crate::app_persistence::write_bytes_atomically;
use crate::material::typography;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

pub(super) const PROJECT_FILE_EXTENSION: &str = "frame";
const PROJECT_FILE_VERSION: u32 = 1;
const MAX_PROJECT_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PROJECT_ITEMS: usize = 10_000;
const UNTITLED_PROJECT: &str = "Untitled project";

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct ProjectFile {
    version: u32,
    output_name: String,
    output: ConversionConfig,
    sources: Vec<SavedSource>,
    clips: Vec<SavedClip>,
    /// Absent in files saved before tracks existed.
    tracks: Vec<SavedTrack>,
    selected_clip: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct SavedTrack {
    kind: SavedTrackKind,
    hidden: bool,
    muted: bool,
    locked: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum SavedTrackKind {
    #[default]
    Video,
    Audio,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct SavedSource {
    path: PathBuf,
    config: ConversionConfig,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct SavedClip {
    /// Index into `sources`. `None` when the library entry was removed after placing the clip.
    source: Option<usize>,
    path: PathBuf,
    kind: SavedKind,
    /// Index into `tracks`. Clips without a valid track go on the first track of their kind.
    track: Option<usize>,
    offset_millis: u64,
    muted: bool,
    speed: f64,
    output_name: String,
    config: ConversionConfig,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum SavedKind {
    #[default]
    Video,
    Audio,
    Image,
}

impl From<SourceKind> for SavedKind {
    fn from(kind: SourceKind) -> Self {
        match kind {
            SourceKind::Video => Self::Video,
            SourceKind::Audio => Self::Audio,
            SourceKind::Image => Self::Image,
        }
    }
}

impl From<SavedKind> for SourceKind {
    fn from(kind: SavedKind) -> Self {
        match kind {
            SavedKind::Video => Self::Video,
            SavedKind::Audio => Self::Audio,
            SavedKind::Image => Self::Image,
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum ProjectFileError {
    #[error("Couldn't read or write the project file: {0}")]
    Io(#[from] std::io::Error),
    #[error("This isn't a valid Frame project file.")]
    Parse(#[from] serde_json::Error),
    #[error("This project was saved by a newer version of Frame.")]
    UnsupportedVersion,
    #[error("This project file is too large to open.")]
    TooLarge,
    #[error("This project file is damaged: {0}")]
    Invalid(&'static str),
}

/// A project file turned back into runtime state, with fresh ids.
pub(super) struct RestoredProject {
    pub(super) library: FileQueue,
    /// Timeline clip instances, in clip order.
    pub(super) instances: Vec<FileItem>,
    pub(super) clips: Vec<ClipPlacement>,
    pub(super) tracks: Tracks,
    pub(super) output: ConversionConfig,
    pub(super) output_name: String,
    pub(super) selected: Option<String>,
}

impl ProjectFile {
    pub(super) fn capture(project: &Project, instances: &FileQueue) -> Self {
        let library = project.library.files();
        let clips = project
            .clips
            .iter()
            .filter_map(|clip| {
                let file = instances.file_by_id(&clip.id)?;
                Some(SavedClip {
                    source: library
                        .iter()
                        .position(|source| source.id == clip.source_id),
                    path: PathBuf::from(&file.path),
                    kind: clip.kind.into(),
                    track: Some(clip.track),
                    offset_millis: clip.offset_millis,
                    muted: clip.muted,
                    speed: clip.speed,
                    output_name: file.output_name.clone(),
                    config: file.config.clone(),
                })
            })
            .collect();
        let selected_clip = instances.selected_file_id().and_then(|selected| {
            project
                .clips
                .iter()
                .filter(|clip| instances.file_by_id(&clip.id).is_some())
                .position(|clip| clip.id == selected)
        });
        Self {
            version: PROJECT_FILE_VERSION,
            output_name: project.output_name.clone(),
            output: project.output.clone(),
            sources: library
                .iter()
                .map(|source| SavedSource {
                    path: PathBuf::from(&source.path),
                    config: source.config.clone(),
                })
                .collect(),
            clips,
            tracks: project
                .tracks
                .iter()
                .map(|track| SavedTrack {
                    kind: match track.kind {
                        TrackKind::Video => SavedTrackKind::Video,
                        TrackKind::Audio => SavedTrackKind::Audio,
                    },
                    hidden: track.hidden,
                    muted: track.muted,
                    locked: track.locked,
                })
                .collect(),
            selected_clip,
        }
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.sources.is_empty() && self.clips.is_empty()
    }

    pub(super) fn restore(self, mut next_id: impl FnMut() -> String) -> RestoredProject {
        let mut library = FileQueue::new();
        let mut source_ids = Vec::with_capacity(self.sources.len());
        for source in self.sources {
            let mut file = FileItem::from_os_path(next_id(), &source.path);
            file.config = source.config;
            source_ids.push(file.id.clone());
            library.add_file(file);
        }

        let tracks = Tracks::from_saved(
            self.tracks
                .iter()
                .map(|track| Track {
                    kind: match track.kind {
                        SavedTrackKind::Video => TrackKind::Video,
                        SavedTrackKind::Audio => TrackKind::Audio,
                    },
                    hidden: track.hidden,
                    muted: track.muted,
                    locked: track.locked,
                })
                .collect(),
        );
        let mut instances = Vec::with_capacity(self.clips.len());
        let mut clips = Vec::with_capacity(self.clips.len());
        for clip in self.clips {
            let kind = SourceKind::from(clip.kind);
            let track_kind = TrackKind::of(kind);
            let track = clip
                .track
                .filter(|&index| {
                    tracks
                        .get(index)
                        .is_some_and(|track| track.kind == track_kind)
                })
                .or_else(|| tracks.iter().position(|track| track.kind == track_kind))
                .unwrap_or_default();
            let mut file = FileItem::from_os_path(next_id(), &clip.path);
            file.output_name = clip.output_name;
            file.config = clip.config;
            clips.push(ClipPlacement {
                id: file.id.clone(),
                // A removed library entry leaves no source to point at; an empty id never matches.
                source_id: clip
                    .source
                    .and_then(|index| source_ids.get(index).cloned())
                    .unwrap_or_default(),
                offset_millis: clip.offset_millis,
                kind,
                track,
                muted: clip.muted,
                speed: clip.speed,
            });
            instances.push(file);
        }

        let selected = self
            .selected_clip
            .and_then(|index| clips.get(index))
            .or_else(|| clips.last())
            .map(|clip| clip.id.clone());
        RestoredProject {
            library,
            instances,
            clips,
            tracks,
            output: self.output,
            output_name: self.output_name,
            selected,
        }
    }

    fn validate(&self) -> Result<(), ProjectFileError> {
        if self.version == 0 {
            return Err(ProjectFileError::Invalid("missing version"));
        }
        if self.version > PROJECT_FILE_VERSION {
            return Err(ProjectFileError::UnsupportedVersion);
        }
        if self.sources.len() > MAX_PROJECT_ITEMS
            || self.clips.len() > MAX_PROJECT_ITEMS
            || self.tracks.len() > MAX_PROJECT_ITEMS
        {
            return Err(ProjectFileError::TooLarge);
        }
        let paths = self
            .sources
            .iter()
            .map(|source| &source.path)
            .chain(self.clips.iter().map(|clip| &clip.path));
        if paths.into_iter().any(|path| path.as_os_str().is_empty()) {
            return Err(ProjectFileError::Invalid("a media path is missing"));
        }
        if self
            .clips
            .iter()
            .any(|clip| !clip.speed.is_finite() || !(0.5..=20.0).contains(&clip.speed))
        {
            return Err(ProjectFileError::Invalid("a clip speed is out of range"));
        }
        Ok(())
    }

    pub(super) fn write(&self, path: &Path) -> Result<(), ProjectFileError> {
        let json = serde_json::to_vec_pretty(self)?;
        write_bytes_atomically(path, &json)?;
        Ok(())
    }

    pub(super) fn read(path: &Path) -> Result<Self, ProjectFileError> {
        if std::fs::metadata(path)?.len() > MAX_PROJECT_FILE_BYTES {
            return Err(ProjectFileError::TooLarge);
        }
        let file: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        file.validate()?;
        Ok(file)
    }
}

/// Adds the project extension unless the user already typed it.
pub(super) fn with_project_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(PROJECT_FILE_EXTENSION))
    {
        path
    } else {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".");
        name.push(PROJECT_FILE_EXTENSION);
        path.with_file_name(name)
    }
}

/// What the project header shows.
pub(super) struct ProjectHeaderProps {
    pub(super) title: SharedString,
    pub(super) error: Option<SharedString>,
}

impl FrameRoot {
    pub(super) fn project_header_props(&self) -> ProjectHeaderProps {
        ProjectHeaderProps {
            title: self
                .project
                .path
                .as_deref()
                .and_then(Path::file_stem)
                .map_or_else(
                    || UNTITLED_PROJECT.into(),
                    |stem| stem.to_string_lossy().into_owned().into(),
                ),
            error: self.project.error.clone().map(Into::into),
        }
    }

    pub(super) fn project_snapshot(&self) -> ProjectFile {
        ProjectFile::capture(&self.project, &self.file_queue)
    }

    /// True when closing the current project would lose work.
    pub(super) fn project_has_unsaved_changes(&self) -> bool {
        let current = self.project_snapshot();
        self.project
            .saved
            .as_ref()
            .map_or(!current.is_empty(), |saved| *saved != current)
    }

    /// Writes the project to `path` and makes it the current project file.
    pub(super) fn save_project_to(&mut self, path: PathBuf) -> bool {
        let snapshot = self.project_snapshot();
        match snapshot.write(&path) {
            Ok(()) => {
                self.project.path = Some(path);
                self.project.saved = Some(snapshot);
                self.project.error = None;
                true
            }
            Err(error) => {
                self.project.error = Some(error.to_string());
                false
            }
        }
    }

    /// Saves to the current project file, or asks where to save when there isn't one yet.
    pub(super) fn save_project(&mut self, save_as: bool, window: &Window, cx: &mut Context<Self>) {
        if !save_as && let Some(path) = self.project.path.clone() {
            self.save_project_to(path);
            cx.notify();
            return;
        }
        let file_name = format!(
            "{}.{PROJECT_FILE_EXTENSION}",
            self.project_header_props().title
        );
        let dialog = project_save_dialog(window, file_name);
        cx.spawn(async move |this, cx| {
            let Some(path) = pick_project_save_path(dialog).await else {
                return;
            };
            this.update(cx, |root, cx| {
                root.save_project_to(with_project_extension(path));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Opens a project file, first confirming if the current project has unsaved changes.
    pub(super) fn request_open_project(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.can_replace_project() {
            self.project.error =
                Some("Finish or cancel the export before opening another project.".to_string());
            cx.notify();
            return;
        }
        if self.project_has_unsaved_changes() {
            self.project.confirm_discard = true;
            cx.notify();
            return;
        }
        Self::prompt_open_project(window, cx);
    }

    pub(super) fn confirm_discard_and_open(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.project.confirm_discard = false;
        cx.notify();
        Self::prompt_open_project(window, cx);
    }

    fn prompt_open_project(window: &Window, cx: &Context<Self>) {
        let dialog = project_open_dialog(window);
        cx.spawn(async move |this, cx| {
            let Some(path) = pick_project_file(dialog).await else {
                return;
            };
            let read_path = path.clone();
            let file = cx
                .background_spawn(async move { ProjectFile::read(&read_path) })
                .await;
            this.update(cx, |root, cx| {
                match file {
                    Ok(file) => root.open_project_file(path, file, cx),
                    Err(error) => root.project.error = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    const fn can_replace_project(&self) -> bool {
        !self.is_processing && !self.update_installation_in_progress()
    }

    fn open_project_file(&mut self, path: PathBuf, file: ProjectFile, cx: &mut Context<Self>) {
        let Some(probe_targets) = self.replace_project(path, file) else {
            self.project.error =
                Some("Finish or cancel the export before opening another project.".to_string());
            return;
        };
        for (id, path) in probe_targets {
            self.queue_restored_source_metadata_probe(id, path, cx);
        }
    }

    /// Swaps in a loaded project. Returns the media to probe, or `None` while an export runs.
    pub(super) fn replace_project(
        &mut self,
        path: PathBuf,
        file: ProjectFile,
    ) -> Option<Vec<(String, String)>> {
        if !self.can_replace_project() {
            return None;
        }
        let restored = file.restore(|| self.next_file_id());
        let probe_targets = restored
            .library
            .files()
            .iter()
            .chain(&restored.instances)
            .map(|file| (file.id.clone(), file.path.clone()))
            .collect();

        let mut instances = FileQueue::new();
        instances.add_files(restored.instances);
        self.file_queue = instances;
        self.project.library = restored.library;
        self.project.clips = restored.clips;
        self.project.tracks = restored.tracks;
        self.project.history.clear();
        self.project.output = restored.output;
        self.project.output_name = restored.output_name;
        self.project.enabled = true;
        self.project.path = Some(path);
        self.project.error = None;
        self.project.confirm_discard = false;
        self.export_ui.open = false;
        self.export_ui.task_ids.clear();
        self.source_metadata = SourceMetadataStore::default();
        self.media_assets.thumbnails.clear();
        self.media_assets.menu = None;
        self.conversion_events = ConversionEventState::new();
        self.active_conversion_task_ids.clear();
        self.active_view = ActiveView::Workspace;
        if let Some(selected) = restored.selected {
            self.select_project_clip(&selected);
        }
        // Compare later edits against what was loaded, not against the file's exact bytes.
        self.project.saved = Some(self.project_snapshot());
        Some(probe_targets)
    }

    pub(super) fn render_discard_project_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let scheme = &crate::app::m3::scheme(self.appearance.color_theme, cx);
        let colors = m3::dialog_colors(scheme, 1.0);
        let cancel_focus = self.ensure_focus(
            FrameFocusKey::Control("discard-project-cancel".to_string()),
            true,
            cx,
        );
        let confirm_focus = self.ensure_focus(
            FrameFocusKey::Control("discard-project-confirm".to_string()),
            true,
            cx,
        );
        if !cancel_focus.contains_focused(window, cx) && !confirm_focus.contains_focused(window, cx)
        {
            cancel_focus.focus(window, cx);
        }
        let headline = "Discard unsaved changes?";
        let dialog = m3::Dialog::new("discard-project-dialog", headline)
            .build(scheme)
            .w(theme::ui_rem(360.0))
            .max_w(relative(1.0))
            .p(theme::ui_rem(24.0))
            .gap(theme::ui_rem(16.0))
            .child(
                div()
                    .m3_type(typography::HEADLINE_SMALL)
                    .text_color(color(colors.headline))
                    .child(headline),
            )
            .child(div().m3_type(typography::BODY_MEDIUM).child(
                "Opening another project closes this one. Changes you haven't saved will be lost.",
            ))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(theme::ui_rem(8.0))
                    .pt(theme::ui_rem(8.0))
                    .child(
                        m3::Button::new(
                            "discard-project-cancel",
                            "Cancel",
                            m3::ButtonVariant::Text,
                        )
                        .focus(&cancel_focus)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(
                            |root, _: &ClickEvent, _window, cx| {
                                root.project.confirm_discard = false;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        m3::Button::new(
                            "discard-project-confirm",
                            "Discard and open",
                            m3::ButtonVariant::Text,
                        )
                        .focus(&confirm_focus)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(
                            |root, _: &ClickEvent, window, cx| {
                                root.confirm_discard_and_open(window, cx);
                            },
                        )),
                    ),
            );

        m3::scrim("discard-project-scrim", 1.0, scheme)
            .on_key_down(
                cx.listener(move |root, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        root.project.confirm_discard = false;
                        cx.notify();
                        return;
                    }
                    accessibility::handle_modal_tab_navigation(
                        event,
                        &cancel_focus,
                        &confirm_focus,
                        window,
                        cx,
                    );
                }),
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                cx.stop_propagation();
                root.project.confirm_discard = false;
                cx.notify();
            }))
            .child(dialog)
    }

    pub(super) fn save_project_action(
        &mut self,
        _: &SaveProject,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_project(false, window, cx);
    }

    pub(super) fn save_project_as_action(
        &mut self,
        _: &SaveProjectAs,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_project(true, window, cx);
    }

    pub(super) fn open_project_action(
        &mut self,
        _: &OpenProject,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_open_project(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_project_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "frame-project-test-{}-{}.frame",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// Two clips of one source, edited independently, plus a library-only source.
    fn edited_project() -> FrameRoot {
        let mut root = super::super::project::tests::project();
        root.project
            .library
            .add_file(FileItem::from_path("unused", "/tmp/unused.wav", 1));
        root.add_library_clip("source");
        root.add_library_clip("source");
        let second = root.project.clips[1].id.clone();
        root.move_project_clip(&second, 12_000);
        root.set_project_clip_speed(&second, 2.0);
        root.project.clips[1].muted = true;
        root.file_queue
            .file_by_id_mut(&second)
            .unwrap()
            .config
            .audio_volume = 40;
        root.project.output.container = "mkv".to_string();
        root.project.output_name = "Holiday".to_string();
        root.file_queue.select_existing_file(&second);
        root
    }

    #[test]
    fn saved_project_reopens_with_the_same_timeline_and_settings() {
        let original = edited_project();
        let path = temp_project_path();
        let mut saver = edited_project();
        assert!(saver.save_project_to(path.clone()));
        assert!(!saver.project_has_unsaved_changes());

        let mut root = FrameRoot::new();
        let probes = root
            .replace_project(path.clone(), ProjectFile::read(&path).unwrap())
            .unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(
            probes.len(),
            4,
            "two sources and two clips are probed again"
        );
        assert_eq!(root.project_snapshot(), original.project_snapshot());
        assert_eq!(root.project.library.files().len(), 2);
        let clip = &root.project.clips[1];
        assert_eq!(clip.offset_millis, 12_000);
        assert!(clip.muted);
        assert!((clip.speed - 2.0).abs() < f64::EPSILON);
        assert_eq!(clip.source_id, root.project.library.files()[0].id);
        let file = root.file_queue.file_by_id(&clip.id).unwrap();
        assert_eq!(file.config.audio_volume, 40);
        assert_eq!(root.file_queue.selected_file_id(), Some(clip.id.as_str()));
        assert_eq!(root.project.output.container, "mkv");
        assert_eq!(root.project.output_name, "Holiday");
        assert_eq!(
            root.project_header_props().title.as_ref(),
            path.file_stem().unwrap().to_str().unwrap()
        );
        assert!(!root.project_has_unsaved_changes());
    }

    #[test]
    fn edits_after_saving_are_unsaved_changes() {
        let mut root = edited_project();
        assert!(root.project_has_unsaved_changes(), "never saved");
        let path = temp_project_path();
        assert!(root.save_project_to(path.clone()));
        std::fs::remove_file(&path).ok();
        let id = root.project.clips[0].id.clone();
        root.move_project_clip(&id, 3_000);
        assert!(root.project_has_unsaved_changes());
    }

    #[test]
    fn an_empty_new_project_has_nothing_to_lose() {
        assert!(!FrameRoot::new().project_has_unsaved_changes());
    }

    #[test]
    fn clips_keep_their_settings_after_their_library_source_is_removed() {
        let mut root = edited_project();
        root.remove_library_source("source");
        let restored = root.project_snapshot().restore({
            let mut next = 0;
            move || {
                next += 1;
                format!("restored-{next}")
            }
        });
        assert_eq!(restored.library.files().len(), 1);
        assert_eq!(restored.clips.len(), 2);
        assert!(restored.clips.iter().all(|clip| clip.source_id.is_empty()));
    }

    #[test]
    fn tracks_and_clip_tracks_survive_a_round_trip() {
        let mut root = edited_project();
        assert!(root.add_project_track(TrackKind::Video));
        let second = root.project.clips[1].id.clone();
        assert!(root.move_project_clip_to_track(&second, 2));
        root.project.tracks[2].hidden = true;
        root.project.tracks[1].locked = true;
        let restored = root.project_snapshot().restore({
            let mut next = 0;
            move || {
                next += 1;
                format!("restored-{next}")
            }
        });
        assert_eq!(restored.tracks, root.project.tracks);
        assert_eq!(restored.clips[0].track, 0);
        assert_eq!(restored.clips[1].track, 2);
    }

    #[test]
    fn files_without_tracks_put_clips_on_the_first_track_of_their_kind() {
        let file: ProjectFile = serde_json::from_str(
            r#"{"version": 1, "clips": [
                {"path": "/a.mp4", "kind": "video", "speed": 1},
                {"path": "/b.wav", "kind": "audio", "speed": 1}
            ]}"#,
        )
        .unwrap();
        let restored = file.restore({
            let mut next = 0;
            move || {
                next += 1;
                format!("restored-{next}")
            }
        });
        assert_eq!(restored.tracks, Tracks::default());
        assert_eq!(restored.clips[0].track, 0);
        assert_eq!(restored.clips[1].track, 1);
    }

    #[test]
    fn opening_is_refused_while_exporting() {
        let mut root = FrameRoot::new();
        root.is_processing = true;
        assert!(
            root.replace_project(PathBuf::from("/tmp/x.frame"), ProjectFile::default())
                .is_none()
        );
    }

    #[test]
    fn unreadable_files_are_rejected() {
        let path = temp_project_path();
        let check = |contents: &str| {
            std::fs::write(&path, contents).unwrap();
            ProjectFile::read(&path)
        };
        assert!(matches!(check("not json"), Err(ProjectFileError::Parse(_))));
        assert!(matches!(check("{}"), Err(ProjectFileError::Invalid(_))));
        assert!(matches!(
            check(r#"{"version": 99}"#),
            Err(ProjectFileError::UnsupportedVersion)
        ));
        assert!(matches!(
            check(r#"{"version": 1, "clips": [{"path": "/a.mp4", "speed": 0}]}"#),
            Err(ProjectFileError::Invalid(_))
        ));
        assert!(matches!(
            check(r#"{"version": 1, "sources": [{"path": ""}]}"#),
            Err(ProjectFileError::Invalid(_))
        ));
        assert!(check(r#"{"version": 1}"#).unwrap().is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn save_dialog_paths_get_the_project_extension() {
        assert_eq!(
            with_project_extension(PathBuf::from("/x/My cut")),
            PathBuf::from("/x/My cut.frame")
        );
        assert_eq!(
            with_project_extension(PathBuf::from("/x/a.FRAME")),
            PathBuf::from("/x/a.FRAME")
        );
        assert_eq!(
            with_project_extension(PathBuf::from("/x/a.v2")),
            PathBuf::from("/x/a.v2.frame")
        );
    }
}
