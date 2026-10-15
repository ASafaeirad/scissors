use super::media_assets::MediaAssetsProps;
use super::{
    ActiveView, ClickEvent, Context, FileItem, FileQueue, FrameRoot, InteractiveElement,
    ParentElement, PreviewPanelProps, SettingsRenderState, SettingsTab, StatefulInteractiveElement,
    Styled, UniformListScrollHandle, Window, color, div, theme,
};
use super::{
    file_list_panel::file_list_panel,
    m3::{self, M3Text},
    preview_panel::preview_panel,
    settings_panel::settings_panel,
};
use crate::material::{ColorScheme, Symbol, shape, spacing, typography};
use gpui::{AppContext, StyledImage, prelude::FluentBuilder};

#[cfg(test)]
mod drag_drop_tests {
    use super::*;
    use crate::app::{Entity, FocusHandle, IntoElement, Render, VisualFixture};
    use gpui::{Modifiers, MouseButton, TestAppContext, point, px};

    struct MediaTimelineTestView {
        root: Entity<FrameRoot>,
        focus: FocusHandle,
    }

    impl Render for MediaTimelineTestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.root.update(cx, |root, cx| {
                let project = root.project_render_state(cx).unwrap();
                let file = project.library.files().first().unwrap();
                div()
                    .flex()
                    .flex_col()
                    .child(
                        media_card(
                            file,
                            false,
                            Some(&project),
                            &project.timeline.scheme,
                            MediaAssetsProps {
                                state: &root.media_assets,
                                delete_enabled: !root.is_processing,
                            },
                            window,
                            cx,
                        )
                        .debug_selector(|| "drag-source".to_string())
                        .w(theme::ui_rem(180.0))
                        .ml(theme::ui_rem(220.0)),
                    )
                    .child(super::super::project_timeline::project_timeline(
                        &project.timeline,
                        &self.focus,
                        window,
                        cx,
                    ))
            })
        }
    }

    #[test]
    fn media_menu_adds_deletes_and_restores_keyboard_focus() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| {
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                root
            });
            MediaTimelineTestView {
                root,
                focus: cx.focus_handle(),
            }
        });
        let root = view.read_with(cx, |view, _| view.root.clone());
        cx.update(|window, cx| window.draw(cx).clear());
        let trigger_bounds = cx.debug_bounds("library-menu-library-video").unwrap();
        let trigger = trigger_bounds.center();
        cx.simulate_click(trigger, Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear());
        root.read_with(cx, |root, _| assert!(root.media_assets.menu.is_some()));
        let menu_bounds = cx.debug_bounds("library-context-library-video").unwrap();
        let gap = cx.update(|window, _| theme::ui_rem(4.0).to_pixels(window.rem_size()));
        assert!((menu_bounds.right() - trigger_bounds.right()).abs() < px(1.0));
        assert!((menu_bounds.top() - trigger_bounds.bottom() - gap).abs() < px(1.0));
        let count = root.read_with(cx, |root, _| root.project.clips.len());
        let add = cx
            .debug_bounds("library-context-library-video-0")
            .unwrap()
            .center();
        cx.simulate_click(add, Modifiers::default());
        root.read_with(cx, |root, _| {
            assert_eq!(root.project.clips.len(), count + 1);
            assert!(root.media_assets.menu.is_none());
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.simulate_click(trigger, Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear());
        let focus = root.read_with(cx, |root, _| {
            root.media_assets.menu.as_ref().unwrap().trigger.clone()
        });
        cx.simulate_keystrokes("escape");
        root.read_with(cx, |root, _| assert!(root.media_assets.menu.is_none()));
        cx.update(|window, _| assert!(focus.is_focused(window)));
        cx.update(|window, cx| window.draw(cx).clear());
        cx.simulate_keystrokes("enter");
        cx.update(|window, cx| window.draw(cx).clear());
        cx.simulate_keystrokes("down");
        cx.simulate_keystrokes("enter");
        root.read_with(cx, |root, _| {
            assert!(root.project.library.file_by_id("library-video").is_none());
            assert_eq!(root.project.clips.len(), count + 1);
            assert!(root.media_assets.menu.is_none());
        });
    }

    struct LibraryTestView {
        root: Entity<FrameRoot>,
    }

    impl Render for LibraryTestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.root.update(cx, |root, cx| {
                library_panel(
                    &root.project.library,
                    root.workspace_ui.filter,
                    MediaAssetsProps {
                        state: &root.media_assets,
                        delete_enabled: true,
                    },
                    None,
                    crate::material::scheme(crate::appearance::ColorTheme::Dark),
                    window,
                    cx,
                )
            })
        }
    }

    #[test]
    fn drop_zone_only_appears_when_the_library_is_empty() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|_, cx| LibraryTestView {
            root: cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                root
            }),
        });
        let root = view.read_with(cx, |view, _| view.root.clone());
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("library-drop-zone").is_none());
        root.update(cx, |root, cx| {
            root.workspace_ui.filter = 3;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("library-drop-zone").is_none());
        root.update(cx, |root, cx| {
            root.project.library = FileQueue::new();
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("library-drop-zone").is_some());
    }

    #[test]
    fn media_card_drop_places_a_new_clip_at_visible_time() {
        for (empty, processing) in [(false, false), (true, false), (false, true)] {
            let mut app = TestAppContext::single();
            let (view, cx) = app.add_window_view(|_, cx| {
                let root = cx.new(|_| {
                    let mut root = FrameRoot::new();
                    root.apply_visual_fixture(Some(VisualFixture::ProjectTimeline));
                    root.is_processing = processing;
                    if empty {
                        root.project.clips.clear();
                        root.file_queue = FileQueue::new();
                    }
                    root
                });
                MediaTimelineTestView {
                    root,
                    focus: cx.focus_handle(),
                }
            });
            let root = view.read_with(cx, |view, _| view.root.clone());
            cx.update(|window, cx| window.draw(cx).clear());
            let source = cx.debug_bounds("drag-source").unwrap();
            let bounds =
                root.read_with(cx, |root, _| root.preview_ui.timeline_track_bounds.unwrap());
            let initial_count = root.read_with(cx, |root, _| root.project.clips.len());
            // Zoom and pan before dropping; fitting the project would give a different time.
            cx.simulate_event(gpui::PinchEvent {
                position: bounds.center(),
                delta: 1.0,
                ..Default::default()
            });
            cx.simulate_event(gpui::ScrollWheelEvent {
                position: bounds.center(),
                delta: gpui::ScrollDelta::Pixels(point(-bounds.size.width * 0.5, px(0.0))),
                ..Default::default()
            });
            cx.update(|window, cx| window.draw(cx).clear());
            let original_view = root.read_with(cx, |root, _| root.project_timeline_view());
            let start = point(source.center().x, source.top() + px(30.0));
            let end = point(bounds.left() + bounds.size.width * 0.75, bounds.center().y);
            let expected = root.read_with(cx, |root, _| root.project_duration().max(1.0) * 625.0);
            cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
            cx.simulate_mouse_move(
                point(start.x + px(8.0), start.y),
                MouseButton::Left,
                Modifiers::default(),
            );
            cx.update(|window, cx| window.draw(cx).clear());
            cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
            cx.update(|window, cx| window.draw(cx).clear());
            cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
            root.read_with(cx, |root, _| {
                assert_eq!(
                    root.project.clips.len(),
                    initial_count + usize::from(!processing)
                );
                if processing {
                    return;
                }
                let clip = root.project.clips.last().unwrap();
                assert_eq!(clip.source_id, "library-video");
                assert!(
                    clip.offset_millis
                        .abs_diff(crate::numeric::rounded_f64_to_u64(expected))
                        <= 1
                );
                assert_eq!(root.project.library.files().len(), 2);
                assert_eq!(root.file_queue.selected_file_id(), Some(clip.id.as_str()));
                assert_eq!(root.project_timeline_view(), original_view);
            });
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct WorkspaceState {
    pub(super) tool: usize,
    pub(super) filter: usize,
    pub(super) export_enabled: bool,
}

const TOOLS: [(Symbol, &str); 5] = [
    (Symbol::VideoLibrary, "Library"),
    (Symbol::Movie, "Video"),
    (Symbol::VolumeUp, "Audio"),
    (Symbol::Layers, "Queue"),
    (Symbol::Info, "Logs"),
];
/// Index of the Video tool in [`TOOLS`].
const VIDEO_TOOL: usize = 1;
const AUDIO_TOOL: usize = 2;
const FILTERS: [&str; 4] = ["All", "Video", "Audio", "Images"];

#[expect(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "The workspace composes navigation, track properties, preview, and project controls from explicit render inputs."
)]
pub(super) fn workspace_view(
    assets: MediaAssetsProps<'_>,
    file_queue: &FileQueue,
    file_list_scroll_handle: &UniformListScrollHandle,
    settings: &SettingsRenderState<'_>,
    preview_props: PreviewPanelProps<'_>,
    state: WorkspaceState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = &crate::app::m3::scheme(
        if settings.palette.is_light() {
            crate::appearance::ColorTheme::Light
        } else {
            crate::appearance::ColorTheme::Dark
        },
        cx,
    );
    let project = preview_props.project.as_ref();
    let project_enabled = project.is_some();
    let rail = navigation_rail(state.tool, scheme, window, cx);
    if state.tool == 3 {
        let queue = project.map_or(file_queue, |project| &project.exports);
        return div()
            .size_full()
            .flex()
            .min_h_0()
            .bg(color(scheme.surface))
            .child(rail)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(project_header(
                        file_queue,
                        state.export_enabled,
                        scheme,
                        window,
                        cx,
                    ))
                    .child(
                        file_list_panel(queue, file_list_scroll_handle, settings.palette, cx)
                            .flex_1(),
                    ),
            );
    }
    let side = if state.tool == 0 {
        library_panel(
            if project_enabled {
                &project.unwrap().library
            } else {
                file_queue
            },
            state.filter,
            assets,
            project,
            scheme,
            window,
            cx,
        )
    } else if file_queue.selected_file().is_some() {
        let speed = project
            .map(|project| &project.timeline)
            .and_then(|timeline| {
                let index = timeline
                    .clips
                    .iter()
                    .position(|clip| file_queue.selected_file_id() == Some(clip.id.as_str()))?;
                Some((timeline, index))
            })
            .map(|(timeline, index)| {
                super::track_speed::track_speed_control(
                    &timeline.clips[index],
                    // Sources skip clips without metadata, so they only line up when none are
                    // missing.
                    (timeline.sources.len() == timeline.clips.len())
                        .then(|| &timeline.sources[index]),
                    !settings.settings_disabled,
                    scheme,
                    window,
                    cx,
                )
            });
        let video_stream = settings
            .metadata
            .filter(|_| state.tool == VIDEO_TOOL)
            .and_then(crate::settings::source_video_stream_rows)
            .map(|rows| {
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .mb(theme::ui_rem(spacing::SPACE_3))
                    .child(m3::list_subheader("Video stream", scheme))
                    .children(
                        rows.into_iter()
                            .map(|row| m3::ListItem::new(row.label).value(row.value).build(scheme)),
                    )
            });
        let audio_metadata = settings
            .metadata
            .filter(|_| state.tool == AUDIO_TOOL)
            .map(|metadata| audio_source_information(metadata, scheme));
        div().size_full().min_w_0().child(
            m3::m3_surface(scheme.surface_container_low, shape::CORNER_EXTRA_LARGE)
                .id("workspace-track-properties")
                .size_full()
                .overflow_hidden()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .p(theme::ui_rem(spacing::SPACE_3))
                .children(speed)
                .children(audio_metadata)
                .children(video_stream)
                .child(settings_panel(settings, window, cx).flex_none()),
        )
    } else {
        m3::m3_surface(scheme.surface_container_low, shape::CORNER_EXTRA_LARGE)
            .p(theme::ui_rem(24.0))
            .child("Select a timeline track to edit its properties.")
    };
    div()
        .size_full()
        .flex()
        .min_h_0()
        .bg(color(scheme.surface))
        .text_color(color(scheme.on_surface))
        .m3_type(typography::BODY_MEDIUM)
        .child(rail)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(project_header(
                    file_queue,
                    state.export_enabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .gap(theme::ui_rem(spacing::SPACE_3))
                        .pr(theme::ui_rem(16.0))
                        .child(
                            div()
                                .w(theme::ui_rem(if state.tool == 0 { 320.0 } else { 380.0 }))
                                .flex_none()
                                .min_h_0()
                                .h_full()
                                .child(side.size_full()),
                        )
                        .child(
                            preview_panel(file_queue, settings, preview_props, window, cx)
                                .flex_1()
                                .min_w_0(),
                        ),
                ),
        )
}

/// Source file and audio stream details below the selected clip's speed control.
fn audio_source_information(
    metadata: &crate::settings::SourceMetadata,
    scheme: &ColorScheme,
) -> gpui::Div {
    use crate::settings::SourceInfoSection;

    let mut content = div()
        .flex_none()
        .flex()
        .flex_col()
        .mb(theme::ui_rem(spacing::SPACE_3));
    for section in crate::settings::source_info_sections(metadata) {
        match section {
            SourceInfoSection::Rows { title, rows } if title == "File information" => {
                content =
                    content
                        .child(m3::list_subheader(title, scheme))
                        .children(rows.into_iter().map(|row| {
                            m3::ListItem::new(row.label).value(row.value).build(scheme)
                        }));
            }
            SourceInfoSection::Tracks { title, tracks } if title == "Audio stream" => {
                content = content.child(m3::list_subheader(title, scheme));
                let multiple_tracks = tracks.len() > 1;
                for track in tracks {
                    if multiple_tracks {
                        content = content.child(m3::list_subheader(track.label, scheme));
                    }
                    content =
                        content.children(track.rows.into_iter().map(|row| {
                            m3::ListItem::new(row.label).value(row.value).build(scheme)
                        }));
                }
            }
            _ => {}
        }
    }
    content
}

pub(super) fn logs_workspace_view(
    logs: gpui::Div,
    queue: &FileQueue,
    state: WorkspaceState,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .min_h_0()
        .bg(color(scheme.surface))
        .text_color(color(scheme.on_surface))
        .m3_type(typography::BODY_MEDIUM)
        .child(navigation_rail(4, scheme, window, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .flex()
                .flex_col()
                .child(project_header(
                    queue,
                    state.export_enabled,
                    scheme,
                    window,
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .px(theme::ui_rem(spacing::SPACE_4))
                        .child(logs),
                ),
        )
}

fn navigation_rail(
    selected: usize,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    m3::NavigationRail::new(
        "workspace-rail",
        TOOLS
            .iter()
            .map(|(icon, label)| m3::NavigationItem::new(*icon, *label))
            .collect(),
    )
    .selected(selected)
    .menu(cx.listener(|root, _, _, cx| {
        root.open_app_settings();
        cx.notify();
    }))
    .on_select(cx.listener(|root, index: &usize, _, cx| {
        if *index == 4 {
            root.active_view = ActiveView::Logs;
            cx.notify();
            return;
        }
        root.active_view = ActiveView::Workspace;
        root.workspace_ui.tool = *index;
        root.settings_ui.active_tab = match index {
            1 => SettingsTab::VideoFilters,
            2 => SettingsTab::Audio,
            _ => root.settings_ui.active_tab,
        };
        cx.notify();
    }))
    .build(scheme, window, cx)
}

fn project_header(
    queue: &FileQueue,
    export_enabled: bool,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    div()
        .h(theme::ui_rem(64.0))
        .flex_none()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .pr(theme::ui_rem(16.0))
        .pl(theme::ui_rem(8.0))
        .child(
            super::chrome::titlebar_drag_surface(cx)
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .m3_type(typography::TITLE_MEDIUM)
                        .child("Untitled project"),
                )
                .child(
                    div()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .child(format!("{} media", queue.files().len())),
                ),
        )
        .child(
            m3::Button::new("workspace-export", "Export", m3::ButtonVariant::Filled)
                .icon(Symbol::FileExport)
                .enabled(export_enabled)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    if root.open_export_dialog() {
                        cx.notify();
                    }
                })),
        )
}

fn media_symbol(file: &FileItem) -> Symbol {
    match file.original_format.to_ascii_lowercase().as_str() {
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "opus" | "aiff" => Symbol::MusicNote,
        "gif" => Symbol::GifBox,
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tiff" | "avif" | "heic" => Symbol::Image,
        _ => Symbol::Movie,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "The library keeps its import controls, filtered media cards, and drop target in layout order."
)]
fn library_panel(
    queue: &FileQueue,
    filter: usize,
    assets: MediaAssetsProps<'_>,
    project: Option<&super::project::ProjectRenderState>,
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let chips: Vec<_> = FILTERS
        .iter()
        .enumerate()
        .map(|(index, label)| {
            m3::Chip::new(
                format!("library-filter-{index}"),
                *label,
                m3::ChipKind::Filter,
            )
            .selected(filter == index)
            .build(scheme, window, cx)
            .on_click(cx.listener(move |root, _, _, cx| {
                root.workspace_ui.filter = index;
                cx.notify();
            }))
        })
        .collect();
    let cards: Vec<_> = queue
        .files()
        .iter()
        .filter(|file| match filter {
            1 => media_symbol(file) == Symbol::Movie,
            2 => media_symbol(file) == Symbol::MusicNote,
            3 => matches!(media_symbol(file), Symbol::Image | Symbol::GifBox),
            _ => true,
        })
        .map(|file| {
            media_card(
                file,
                queue.selected_file_id() == Some(file.id.as_str()),
                project,
                scheme,
                assets,
                window,
                cx,
            )
        })
        .collect();
    div().size_full().min_w_0().child(
        m3::m3_surface(scheme.surface_container_low, shape::CORNER_EXTRA_LARGE)
            .id("workspace-library")
            .w(theme::ui_rem(320.0))
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(16.0))
            .p(theme::ui_rem(20.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme::ui_rem(4.0))
                    .child(div().m3_type(typography::TITLE_LARGE).child("Library"))
                    .min_w_0()
                    .flex_none()
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .text_color(color(scheme.on_surface_variant))
                            .child("Import sources, then add them to the timeline."),
                    ),
            )
            .child(
                div().flex().child(
                    m3::Button::new("library-import", "Import media", m3::ButtonVariant::Tonal)
                        .icon(Symbol::Upload)
                        .build(scheme, window, cx)
                        .on_click(cx.listener(|_, _, window, cx| {
                            FrameRoot::prompt_add_source(window, cx);
                        })),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::ui_rem(8.0))
                    .children(chips),
            )
            .child(
                div()
                    .flex_none()
                    .min_w_0()
                    .content_start()
                    .items_start()
                    .grid()
                    .grid_cols(2)
                    .gap(theme::ui_rem(12.0))
                    .children(cards),
            )
            .when(queue.files().is_empty(), |panel| {
                panel.child(
                    div()
                        .flex_none()
                        .min_w_0()
                        .border_1()
                        .debug_selector(|| "library-drop-zone".to_string())
                        .border_dashed()
                        .border_color(color(scheme.outline))
                        .rounded(theme::ui_rem(shape::CORNER_LARGE))
                        .p(theme::ui_rem(16.0))
                        .flex()
                        .items_center()
                        .gap(theme::ui_rem(12.0))
                        .text_color(color(scheme.on_surface_variant))
                        .child(m3::m3_icon(
                            Symbol::Upload,
                            false,
                            24.0,
                            color(scheme.on_surface_variant),
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child("Drop video, audio, images or GIFs here"),
                        ),
                )
            }),
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "A library card combines accessible source selection, drag, add, and removal actions."
)]
fn media_card(
    file: &FileItem,
    selected: bool,
    project: Option<&super::project::ProjectRenderState>,
    scheme: &ColorScheme,
    assets: MediaAssetsProps<'_>,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let id = file.id.clone();
    let symbol = media_symbol(file);
    let thumbnail = assets.state.thumbnails.get(&id).cloned();
    let preview = div()
        .id(format!("library-media-{id}"))
        .flex()
        .flex_col()
        .flex_none()
        .min_w_0()
        .gap(theme::ui_rem(6.0))
        .role(gpui::Role::Button)
        .aria_label(format!("Select {} source", file.name))
        .aria_selected(selected)
        .focusable()
        .tab_stop(true)
        .cursor_pointer()
        .focus_visible(move |style| {
            style
                .ring_width(theme::ui_rem(3.0))
                .ring_color(color(scheme.secondary))
        })
        .on_click(cx.listener(move |root, _: &ClickEvent, _, cx| {
            if root.select_library_source(&id) {
                cx.notify();
            }
        }))
        .on_key_down(cx.listener({
            let id = file.id.clone();
            move |root, event: &gpui::KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    if root.select_library_source(&id) {
                        cx.notify();
                    }
                }
            }
        }))
        .child(
            m3::m3_surface(scheme.surface_container_highest, shape::CORNER_MEDIUM)
                .h(theme::ui_rem(84.0))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |card| {
                    card.border_2().border_color(color(scheme.primary))
                })
                .overflow_hidden()
                .when_some(thumbnail.clone(), |card, thumbnail| {
                    card.child(
                        gpui::img(thumbnail)
                            .size_full()
                            .object_fit(gpui::ObjectFit::Cover),
                    )
                })
                .when(thumbnail.is_none(), |card| {
                    card.child(m3::m3_icon(
                        symbol,
                        false,
                        28.0,
                        color(scheme.on_surface_variant),
                    ))
                })
                .child(
                    m3::m3_surface(scheme.surface_container_lowest, shape::CORNER_EXTRA_SMALL)
                        .absolute()
                        .left(theme::ui_rem(6.0))
                        .bottom(theme::ui_rem(6.0))
                        .px(theme::ui_rem(6.0))
                        .py(theme::ui_rem(2.0))
                        .text_color(color(scheme.on_surface))
                        .m3_type(typography::LABEL_SMALL)
                        .child(file.original_format.to_uppercase()),
                ),
        )
        .child(
            div()
                .m3_type(typography::LABEL_MEDIUM)
                .truncate()
                .child(file.name.clone()),
        );
    div()
        .id(format!("library-card-{}", file.id))
        .group(format!("library-card-{}", file.id))
        .relative()
        .min_w_0()
        .child(preview.when(
            project.is_some_and(|project| {
                project.ready_sources.contains(&file.id) && project.timeline.enabled
            }),
            |preview| {
                let name = file.name.clone();
                let scheme = *scheme;
                preview.cursor_grab().on_drag(
                    super::project_timeline::LibrarySourceDrag(file.id.clone()),
                    move |_, _, _, cx| {
                        cx.new(|_| super::project_timeline::LibraryDragPreview {
                            name: name.clone(),
                            scheme,
                        })
                    },
                )
            },
        ))
        .when_some(project, |card, project| {
            card.child(
                div()
                    .m3_type(typography::LABEL_SMALL)
                    .text_color(color(scheme.on_surface_variant))
                    .child(format!(
                        "{} timeline uses",
                        project
                            .timeline
                            .clips
                            .iter()
                            .filter(|clip| clip.source_id == file.id)
                            .count()
                    )),
            )
        })
        .child(library_asset_actions(
            file, project, scheme, assets, window, cx,
        ))
}

#[expect(
    clippy::too_many_lines,
    reason = "The asset action trigger and its anchored menu share focus and dismissal handlers."
)]
fn library_asset_actions(
    file: &FileItem,
    project: Option<&super::project::ProjectRenderState>,
    scheme: &ColorScheme,
    assets: MediaAssetsProps<'_>,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let trigger = window
        .use_keyed_state(format!("library-menu-focus-{}", file.id), cx, |_, cx| {
            cx.focus_handle()
        })
        .read(cx)
        .clone();
    let menu = assets
        .state
        .menu
        .clone()
        .filter(|menu| menu.source_id == file.id);
    let can_add = project.is_some_and(|project| {
        project.ready_sources.contains(&file.id) && project.timeline.enabled
    });
    let can_delete = assets.delete_enabled;
    let id = file.id.clone();
    let click_trigger = trigger.clone();
    let key_trigger = trigger.clone();
    let key_id = id.clone();
    let button = m3::IconButton::new(
        format!("library-menu-{id}"),
        Symbol::MoreVert,
        format!("Actions for {}", file.name),
    )
    .variant(m3::IconButtonVariant::Tonal)
    .size(m3::IconButtonSize::ExtraSmall)
    .focus(&trigger)
    .build(scheme, window, cx)
    .debug_selector({
        let id = file.id.clone();
        move || format!("library-menu-{id}")
    })
    .aria_expanded(menu.is_some())
    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
    .on_click(cx.listener(move |root, _, window, cx| {
        cx.stop_propagation();
        root.toggle_asset_menu(&id, click_trigger.clone(), can_add, window, cx);
    }))
    .on_key_down(
        cx.listener(move |root, event: &gpui::KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space" | "down") {
                cx.stop_propagation();
                root.toggle_asset_menu(&key_id, key_trigger.clone(), can_add, window, cx);
            }
        }),
    );
    div()
        .id(format!("library-actions-{}", file.id))
        .size(theme::ui_rem(32.0))
        .absolute()
        .top(theme::ui_rem(4.0))
        .right(theme::ui_rem(4.0))
        .child(
            div()
                .id(format!("library-actions-trigger-{}", file.id))
                .opacity(if menu.is_some() { 1.0 } else { 0.0 })
                .group_hover(format!("library-card-{}", file.id), |style| {
                    style.opacity(1.0)
                })
                .contains_focus(|style| style.opacity(1.0))
                .child(button),
        )
        .when_some(menu, |actions, menu| {
            let id = file.id.clone();
            let action_menu = m3::Menu::new(
                format!("library-context-{}", file.id),
                vec![
                    m3::MenuItem::new("Add to timeline", Symbol::Add)
                        .enabled(can_add)
                        .focus(&menu.focuses[0]),
                    m3::MenuItem::new("Delete", Symbol::Delete)
                        .enabled(can_delete)
                        .focus(&menu.focuses[1]),
                ],
            )
            .on_select(cx.listener(move |root, index: &usize, window, cx| {
                root.dismiss_asset_menu(true, window, cx);
                if *index == 0 {
                    root.add_library_clip(&id);
                } else if root.project.enabled {
                    root.remove_library_source(&id);
                } else {
                    root.remove_file_from_queue(&id);
                }
                cx.notify();
            }))
            .build(scheme, window, cx)
            .debug_selector({
                let id = file.id.clone();
                move || format!("library-context-{id}")
            })
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|root, _, window, cx| {
                root.dismiss_asset_menu(false, window, cx);
            }))
            .on_key_down(cx.listener(
                |root, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "escape" | "tab") {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                        }
                        root.dismiss_asset_menu(true, window, cx);
                    }
                },
            ));
            // A zero-size anchor keeps the menu out of the trigger's layout.
            // Its origin is the button's bottom-right corner plus a 4px gap.
            actions.child(
                div()
                    .absolute()
                    .size_0()
                    .right_0()
                    .top(theme::ui_rem(36.0))
                    .child(
                        gpui::deferred(
                            gpui::anchored()
                                .anchor(gpui::Anchor::TopRight)
                                .snap_to_window()
                                .child(action_menu),
                        )
                        .with_priority(20),
                    ),
            )
        })
}

impl FrameRoot {
    fn toggle_asset_menu(
        &mut self,
        id: &str,
        trigger: gpui::FocusHandle,
        can_add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .media_assets
            .menu
            .as_ref()
            .is_some_and(|menu| menu.source_id == id)
        {
            self.dismiss_asset_menu(true, window, cx);
            return;
        }
        let focuses = [cx.focus_handle(), cx.focus_handle()];
        focuses[usize::from(!can_add)].focus(window, cx);
        self.media_assets.menu = Some(super::media_assets::AssetMenu {
            source_id: id.to_string(),
            focuses,
            trigger,
        });
        cx.notify();
    }

    fn dismiss_asset_menu(
        &mut self,
        restore_focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.media_assets.menu.take() {
            if restore_focus {
                menu.trigger.focus(window, cx);
            }
            cx.notify();
        }
    }
}

pub(super) fn welcome_view(
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = &crate::app::m3::scheme(
        if palette.is_light() {
            crate::appearance::ColorTheme::Light
        } else {
            crate::appearance::ColorTheme::Dark
        },
        cx,
    );
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(color(scheme.surface))
        .text_color(color(scheme.on_surface))
        .child(
            div()
                .max_w(theme::ui_rem(420.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(theme::ui_rem(20.0))
                .text_center()
                .child(m3::m3_icon(
                    Symbol::VideoLibrary,
                    false,
                    56.0,
                    color(scheme.primary),
                ))
                .child(
                    div()
                        .m3_type(typography::HEADLINE_SMALL)
                        .child("Your media starts here"),
                )
                .child(
                    div()
                        .m3_type(typography::BODY_MEDIUM)
                        .text_color(color(scheme.on_surface_variant))
                        .child("Import video, audio or images to start a project."),
                )
                .child(
                    div()
                        .flex()
                        .gap(theme::ui_rem(8.0))
                        .child(
                            m3::Button::new(
                                "welcome-open-file",
                                "Import media",
                                m3::ButtonVariant::Filled,
                            )
                            .icon(Symbol::Upload)
                            .build(scheme, window, cx)
                            .on_click(cx.listener(
                                |_, _, window, cx| FrameRoot::prompt_add_source(window, cx),
                            )),
                        )
                        .child(
                            m3::Button::new(
                                "welcome-open-folder",
                                "Import folder",
                                m3::ButtonVariant::Outlined,
                            )
                            .icon(Symbol::UploadFile)
                            .build(scheme, window, cx)
                            .on_click(cx.listener(
                                |_, _, window, cx| FrameRoot::prompt_add_source_folder(window, cx),
                            )),
                        ),
                ),
        )
}
