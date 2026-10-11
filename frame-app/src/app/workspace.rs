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
use crate::material::{self, ColorScheme, Symbol, shape, spacing, typography};
use gpui::{AppContext, prelude::FluentBuilder};

#[derive(Clone, Copy, Default)]
pub(super) struct WorkspaceState {
    pub(super) tool: usize,
    pub(super) filter: usize,
    pub(super) export_enabled: bool,
}

const TOOLS: [(Symbol, &str); 7] = [
    (Symbol::VideoLibrary, "Library"),
    (Symbol::Crop, "Crop"),
    (Symbol::VolumeUp, "Audio"),
    (Symbol::ClosedCaption, "Captions"),
    (Symbol::FileExport, "Output"),
    (Symbol::Layers, "Queue"),
    (Symbol::Info, "Logs"),
];
const FILTERS: [&str; 4] = ["All", "Video", "Audio", "Images"];

pub(super) fn workspace_view(
    file_queue: &FileQueue,
    file_list_scroll_handle: &UniformListScrollHandle,
    settings: &SettingsRenderState<'_>,
    preview_props: PreviewPanelProps<'_>,
    state: WorkspaceState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = material::scheme(if settings.palette.is_light() {
        crate::appearance::ColorTheme::Light
    } else {
        crate::appearance::ColorTheme::Dark
    });
    let project = preview_props.project.as_ref();
    let project_enabled = project.is_some();
    let rail = navigation_rail(state.tool, scheme, window, cx);
    if state.tool == 5 {
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
            project,
            scheme,
            window,
            cx,
        )
    } else if file_queue.selected_file().is_some() {
        settings_panel(settings, window, cx)
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

pub(super) fn logs_workspace_view(
    logs: gpui::Div,
    queue: &FileQueue,
    state: WorkspaceState,
    scheme: &'static ColorScheme,
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
        .child(navigation_rail(6, scheme, window, cx))
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
    scheme: &'static ColorScheme,
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
        if *index == 6 {
            root.active_view = ActiveView::Logs;
            cx.notify();
            return;
        }
        root.active_view = ActiveView::Workspace;
        root.workspace_ui.tool = *index;
        root.settings_ui.active_tab = match index {
            1 => SettingsTab::VideoFilters,
            2 => SettingsTab::Audio,
            3 => SettingsTab::Subtitles,
            4 => SettingsTab::Output,
            _ => root.settings_ui.active_tab,
        };
        if *index == 1 && !root.preview_ui.crop_mode {
            root.toggle_selected_crop_mode();
        }
        cx.notify();
    }))
    .build(scheme, window, cx)
}

fn project_header(
    queue: &FileQueue,
    export_enabled: bool,
    scheme: &'static ColorScheme,
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
    project: Option<&super::project::ProjectRenderState>,
    scheme: &'static ColorScheme,
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
            .child(
                div()
                    .flex_none()
                    .min_w_0()
                    .border_1()
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
            ),
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
    scheme: &'static ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let id = file.id.clone();
    let symbol = media_symbol(file);
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
                .child(m3::m3_icon(
                    symbol,
                    false,
                    28.0,
                    color(scheme.on_surface_variant),
                ))
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
        .relative()
        .min_w_0()
        .child(preview.on_drag(
            super::project_timeline::LibrarySourceDrag(file.id.clone()),
            |_, _, _, cx| cx.new(|_| super::project_timeline::MediaDragPreview),
        ))
        .when_some(project, |card, project| {
            card.child(
                m3::Button::new(
                    format!("library-add-{}", file.id),
                    "Add to timeline",
                    m3::ButtonVariant::Tonal,
                )
                .size(m3::ButtonSize::ExtraSmall)
                .icon(Symbol::Add)
                .enabled(project.ready_sources.contains(&file.id) && project.timeline.enabled)
                .build(scheme, window, cx)
                .on_click(cx.listener({
                    let id = file.id.clone();
                    move |root, _, _, cx| {
                        if root.add_library_clip(&id) {
                            cx.notify();
                        }
                    }
                })),
            )
            .child(
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
        .child(remove_library_button(file, scheme, window, cx))
}

fn remove_library_button(
    file: &FileItem,
    scheme: &'static ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let id = file.id.clone();
    let key_id = id.clone();
    m3::IconButton::new(
        format!("library-remove-{id}"),
        Symbol::Close,
        format!("Remove {} from library", file.name),
    )
    .variant(m3::IconButtonVariant::Tonal)
    .build(scheme, window, cx)
    .absolute()
    .top(theme::ui_rem(4.0))
    .right(theme::ui_rem(4.0))
    .on_click(cx.listener(move |root, _, _, cx| {
        if if root.project.enabled {
            root.remove_library_source(&id)
        } else {
            root.remove_file_from_queue(&id)
        } {
            cx.notify();
        }
    }))
    .on_key_down(cx.listener(move |root, event: &gpui::KeyDownEvent, _, cx| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            cx.stop_propagation();
            if if root.project.enabled {
                root.remove_library_source(&key_id)
            } else {
                root.remove_file_from_queue(&key_id)
            } {
                cx.notify();
            }
        }
    }))
}

pub(super) fn welcome_view(
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let scheme = material::scheme(if palette.is_light() {
        crate::appearance::ColorTheme::Light
    } else {
        crate::appearance::ColorTheme::Dark
    });
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
