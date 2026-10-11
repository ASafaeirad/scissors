//! The `m3-gallery` visual fixture: a static layout modeled on the "Video Editor Timeline" design
//! that renders every M3 component in context. Use it to review the components while migrating.
//! It is not a working editor.

use super::{
    Button, ButtonVariant, Chip, ChipKind, ColorScheme, Dialog, IconButton, IconButtonVariant,
    LinearProgress, M3Text, Menu, MenuItem, NavigationItem, NavigationRail, RadioItem,
    SegmentedButton, SelectCard, Slider, Switch, Symbol, TextField, ToolbarColor, ToolbarVariant,
    color, div, m3_icon, m3_surface, theme, toolbar,
};
use crate::app::{
    Context, FrameRoot, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled,
    Window,
};
use crate::material::{shape, typography};
use gpui::{prelude::FluentBuilder, relative};

const TOOLS: [(Symbol, &str, &str); 5] = [
    (
        Symbol::VideoLibrary,
        "Library",
        "Drag media onto the timeline, or press + to add it at the playhead.",
    ),
    (
        Symbol::Crop,
        "Crop",
        "Change the frame shape and orientation.",
    ),
    (
        Symbol::VolumeUp,
        "Audio",
        "Adjust the volume of the selected clip.",
    ),
    (
        Symbol::ClosedCaption,
        "Captions",
        "Add subtitles or burn them into the video.",
    ),
    (
        Symbol::FileExport,
        "Output",
        "Pick a format and quality for export.",
    ),
];
const FILTERS: [&str; 4] = ["All", "Video", "Audio", "Images"];
const ASPECTS: [&str; 4] = ["Original", "16:9", "9:16", "1:1"];
const FORMATS: [&str; 3] = ["MP4", "WebM", "GIF"];
const ENCODERS: [(&str, &str); 3] = [
    ("H.264", "Plays everywhere"),
    ("H.265 / HEVC", "Smaller files"),
    ("AV1", "Smallest, slow"),
];
const MEDIA: [(Symbol, &str, &str); 6] = [
    (Symbol::Movie, "recording_2026-09-25_16.30.58.mp4", "0:19"),
    (Symbol::Movie, "screen_demo_v2.mov", "0:24"),
    (Symbol::MusicNote, "lofi_background.mp3", "2:10"),
    (Symbol::MusicNote, "voiceover_take3.wav", "0:22"),
    (Symbol::GifBox, "logo_spin.gif", "GIF"),
    (Symbol::Image, "title_card.png", "Image"),
];

#[derive(Clone, Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag mirrors one independent toggle in the gallery layout."
)]
pub(in crate::app) struct M3GalleryState {
    tool: usize,
    filter: usize,
    aspect: usize,
    format: usize,
    encoder: usize,
    muted: bool,
    fade: bool,
    burn_captions: bool,
    playing: bool,
    hidden_tracks: [bool; 3],
}

impl M3GalleryState {
    pub(in crate::app) fn new() -> Self {
        Self {
            fade: true,
            ..Self::default()
        }
    }
}

impl FrameRoot {
    pub(in crate::app) fn render_m3_gallery(
        &self,
        state: &M3GalleryState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        super::update_color_schemes(self.appearance.accent_color, cx);
        let scheme = &crate::app::m3::scheme(self.appearance.color_theme, cx);
        let rail = NavigationRail::new(
            "m3-rail",
            TOOLS
                .iter()
                .map(|(icon, label, _)| NavigationItem::new(*icon, *label))
                .collect(),
        )
        .selected(state.tool)
        .menu(|_, _, _| {})
        .on_select(cx.listener(|root, index: &usize, _, cx| {
            root.update_m3_gallery(cx, |gallery| gallery.tool = *index);
        }))
        .build(scheme, window, cx);

        div()
            .size_full()
            .flex()
            .rounded(theme::ui_rem(shape::CORNER_EXTRA_LARGE))
            .overflow_hidden()
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
                    .child(top_bar(scheme, window, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .gap(theme::ui_rem(12.0))
                            .pr(theme::ui_rem(16.0))
                            .child(Self::tool_panel(state, scheme, window, cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(theme::ui_rem(12.0))
                                    .child(Self::preview_area(state, scheme, window, cx))
                                    .child(Self::timeline(state, scheme, window, cx)),
                            ),
                    ),
            )
    }

    fn update_m3_gallery(
        &mut self,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut M3GalleryState),
    ) {
        if let Some(gallery) = self.m3_gallery.as_mut() {
            update(gallery);
            cx.notify();
        }
    }

    fn tool_panel(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let (_, title, hint) = TOOLS[state.tool];
        let header = div()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(4.0))
            .child(div().m3_type(typography::TITLE_LARGE).child(title))
            .child(
                div()
                    .m3_type(typography::BODY_MEDIUM)
                    .text_color(color(scheme.on_surface_variant))
                    .child(hint),
            );
        let body = match state.tool {
            0 => Self::library_panel(state, scheme, window, cx),
            1 => Self::crop_panel(state, scheme, window, cx),
            2 => Self::audio_panel(state, scheme, window, cx),
            3 => Self::captions_panel(state, scheme, window, cx),
            _ => Self::output_panel(state, scheme, window, cx),
        };

        m3_surface(
            scheme.surface_container_low,
            shape::CORNER_EXTRA_LARGE - 4.0,
        )
        .w(theme::ui_rem(320.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(16.0))
        .p(theme::ui_rem(20.0))
        .child(header)
        .child(
            div()
                .id("m3-tool-panel-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .child(body.flex_none()),
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "The gallery renders media cards and library controls together for visual review."
    )]
    fn library_panel(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let chips: Vec<_> = FILTERS
            .iter()
            .enumerate()
            .map(|(index, label)| {
                Chip::new(format!("m3-filter-{index}"), *label, ChipKind::Filter)
                    .selected(state.filter == index)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(move |root, _, _, cx| {
                        root.update_m3_gallery(cx, |gallery| gallery.filter = index);
                    }))
            })
            .collect();
        let media: Vec<_> = MEDIA
            .iter()
            .enumerate()
            .filter(|(_, (icon, _, _))| match state.filter {
                1 => *icon == Symbol::Movie,
                2 => *icon == Symbol::MusicNote,
                3 => matches!(icon, Symbol::GifBox | Symbol::Image),
                _ => true,
            })
            .map(|(index, (icon, name, badge))| {
                div()
                    .group(format!("m3-media-{index}"))
                    .flex()
                    .flex_col()
                    .gap(theme::ui_rem(6.0))
                    .min_w_0()
                    .child(
                        m3_surface(scheme.surface_container_highest, shape::CORNER_MEDIUM)
                            .relative()
                            .h(theme::ui_rem(84.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(m3_icon(
                                *icon,
                                false,
                                28.0,
                                color(scheme.on_surface_variant),
                            ))
                            .child(
                                m3_surface(
                                    scheme.surface_container_lowest,
                                    shape::CORNER_EXTRA_SMALL,
                                )
                                .absolute()
                                .left(theme::ui_rem(6.0))
                                .bottom(theme::ui_rem(6.0))
                                .px(theme::ui_rem(6.0))
                                .py(theme::ui_rem(2.0))
                                .m3_type(typography::LABEL_SMALL)
                                .child(*badge),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(theme::ui_rem(2.0))
                                    .right(theme::ui_rem(2.0))
                                    .child(
                                        IconButton::new(
                                            format!("m3-media-actions-{index}"),
                                            Symbol::MoreVert,
                                            "Media actions",
                                        )
                                        .variant(IconButtonVariant::Tonal)
                                        .size(super::IconButtonSize::ExtraSmall)
                                        .build(scheme, window, cx)
                                        .opacity(0.0)
                                        .group_hover(format!("m3-media-{index}"), |style| {
                                            style.opacity(1.0)
                                        })
                                        .focus_visible(|style| style.opacity(1.0)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .m3_type(typography::LABEL_MEDIUM)
                            .truncate()
                            .child(*name),
                    )
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(16.0))
            .child(
                div().flex().child(
                    Button::new("m3-import", "Import media", ButtonVariant::Tonal)
                        .icon(Symbol::Upload)
                        .build(scheme, window, cx),
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
                    .grid()
                    .grid_cols(2)
                    .gap(theme::ui_rem(12.0))
                    .children(media),
            )
            .child(
                Menu::new(
                    "m3-media-menu",
                    vec![
                        MenuItem::new("Add to timeline", Symbol::Add),
                        MenuItem::new("Delete", Symbol::Delete),
                    ],
                )
                .build(scheme, window, cx),
            )
    }

    fn crop_panel(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let chips: Vec<_> = ASPECTS
            .iter()
            .enumerate()
            .map(|(index, label)| {
                Chip::new(format!("m3-aspect-{index}"), *label, ChipKind::Filter)
                    .selected(state.aspect == index)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(move |root, _, _, cx| {
                        root.update_m3_gallery(cx, |gallery| gallery.aspect = index);
                    }))
            })
            .collect();
        let rotate = [
            (Symbol::Rotate90DegreesCcw, "Rotate left"),
            (Symbol::Rotate90DegreesCw, "Rotate right"),
            (Symbol::Flip, "Flip"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (icon, label))| {
            IconButton::new(format!("m3-rotate-{index}"), icon, label)
                .variant(IconButtonVariant::Tonal)
                .build(scheme, window, cx)
        })
        .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(12.0))
            .child(div().m3_type(typography::TITLE_SMALL).child("Aspect ratio"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::ui_rem(8.0))
                    .children(chips),
            )
            .child(
                div()
                    .pt(theme::ui_rem(8.0))
                    .m3_type(typography::TITLE_SMALL)
                    .child("Orientation"),
            )
            .child(div().flex().gap(theme::ui_rem(8.0)).children(rotate))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::ui_rem(8.0))
                    .child(
                        Chip::new("m3-assist", "Auto-straighten", ChipKind::Assist)
                            .icon(Symbol::Crop)
                            .build(scheme, window, cx),
                    )
                    .child(
                        Chip::new("m3-suggest", "Fit to frame", ChipKind::Suggestion)
                            .elevated(true)
                            .build(scheme, window, cx),
                    ),
            )
    }

    fn audio_panel(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let volume = if state.muted { 0.0 } else { 100.0 };
        div()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(8.0))
            .child(label_value_row(
                "Volume",
                if state.muted { "Muted" } else { "100%" },
                scheme,
            ))
            .child(
                Slider::new("m3-volume", "Volume", volume, 0.0, 200.0)
                    .value_text(format!("{volume:.0}%"))
                    .enabled(!state.muted)
                    .build(scheme),
            )
            .child(switch_row(
                "Mute clip",
                Switch::new("m3-mute", "Mute clip", state.muted)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(|root, _, _, cx| {
                        root.update_m3_gallery(cx, |gallery| gallery.muted = !gallery.muted);
                    })),
            ))
            .child(switch_row(
                "Fade in & out",
                Switch::new("m3-fade", "Fade in and out", state.fade)
                    .icons(true)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(|root, _, _, cx| {
                        root.update_m3_gallery(cx, |gallery| gallery.fade = !gallery.fade);
                    })),
            ))
    }

    fn captions_panel(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(8.0))
            .child(switch_row(
                "Burn in captions",
                Switch::new("m3-burn", "Burn in captions", state.burn_captions)
                    .build(scheme, window, cx)
                    .on_click(cx.listener(|root, _, _, cx| {
                        root.update_m3_gallery(cx, |gallery| {
                            gallery.burn_captions = !gallery.burn_captions;
                        });
                    })),
            ))
            .child(
                div().flex().child(
                    Button::new("m3-subtitle", "Add subtitle file", ButtonVariant::Outlined)
                        .icon(Symbol::UploadFile)
                        .build(scheme, window, cx),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme::ui_rem(8.0))
                    .child(
                        Button::new("m3-elevated", "Elevated", ButtonVariant::Elevated)
                            .build(scheme, window, cx),
                    )
                    .child(
                        Button::new("m3-text", "Text", ButtonVariant::Text)
                            .build(scheme, window, cx),
                    )
                    .child(
                        Button::new("m3-disabled", "Disabled", ButtonVariant::Filled)
                            .enabled(false)
                            .build(scheme, window, cx),
                    ),
            )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "The output panel shows every export-related component for visual review."
    )]
    fn output_panel(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(theme::ui_rem(12.0))
            .child(
                SegmentedButton::new("m3-format", "Format", FORMATS)
                    .selected(Some(state.format))
                    .on_select(cx.listener(|root, index: &usize, _, cx| {
                        root.update_m3_gallery(cx, |gallery| gallery.format = *index);
                    }))
                    .build(scheme, window, cx),
            )
            .child(label_value_row("Quality", "Balanced", scheme))
            .child(
                div().pt(theme::ui_rem(56.0)).child(
                    Slider::new("m3-quality", "Quality", 70.0, 0.0, 100.0)
                        .show_value_label(true)
                        .build(scheme),
                ),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(theme::ui_rem(8.0))
                    .child(
                        SelectCard::new("m3-card-mp4", "MP4", state.format == 0)
                            .icon(Symbol::Movie)
                            .supporting("Share anywhere")
                            .build(scheme, window, cx)
                            .on_click(cx.listener(|root, _, _, cx| {
                                root.update_m3_gallery(cx, |gallery| gallery.format = 0);
                            })),
                    )
                    .child(
                        SelectCard::new("m3-card-gif", "GIF", state.format == 2)
                            .icon(Symbol::GifBox)
                            .supporting("Short loops")
                            .build(scheme, window, cx)
                            .on_click(cx.listener(|root, _, _, cx| {
                                root.update_m3_gallery(cx, |gallery| gallery.format = 2);
                            })),
                    ),
            )
            .children(ENCODERS.iter().enumerate().map(|(index, (label, hint))| {
                RadioItem::new(
                    format!("m3-encoder-{index}"),
                    *label,
                    state.encoder == index,
                )
                .supporting(*hint)
                .enabled(index < 2)
                .build(scheme, window, cx)
                .on_click(cx.listener(move |root, _, _, cx| {
                    root.update_m3_gallery(cx, |gallery| gallery.encoder = index);
                }))
            }))
            .child(
                div().pt(theme::ui_rem(8.0)).child(
                    TextField::new("m3-file-name", "File name")
                        .input(div().child("render_final"))
                        .populated(true)
                        .suffix(".mp4")
                        .supporting_text("Saved to Movies")
                        .container(scheme.surface_container_low)
                        .build(scheme, window, cx),
                ),
            )
            .child(
                TextField::new("m3-file-name-empty", "Width")
                    .error(Some("Use even numbers"))
                    .container(scheme.surface_container_low)
                    .build(scheme, window, cx),
            )
            .child(LinearProgress::new("m3-progress", "Export progress", 0.42).build(scheme))
            .child(
                Dialog::new("m3-dialog", "Discard export?")
                    .build(scheme)
                    .p(theme::ui_rem(24.0))
                    .gap(theme::ui_rem(16.0))
                    .child(
                        div()
                            .m3_type(typography::HEADLINE_SMALL)
                            .text_color(color(scheme.on_surface))
                            .child("Discard export?"),
                    )
                    .child("The file exported so far will be deleted.")
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(theme::ui_rem(8.0))
                            .child(
                                Button::new("m3-dialog-keep", "Keep", ButtonVariant::Text)
                                    .build(scheme, window, cx),
                            )
                            .child(
                                Button::new("m3-dialog-discard", "Discard", ButtonVariant::Text)
                                    .build(scheme, window, cx),
                            ),
                    ),
            )
    }

    fn preview_area(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let controls = toolbar(ToolbarVariant::Floating, ToolbarColor::Standard, scheme)
            .child(
                IconButton::new("m3-back", Symbol::Replay5, "Back 5 seconds")
                    .build(scheme, window, cx),
            )
            .child(
                IconButton::new(
                    "m3-play",
                    if state.playing {
                        Symbol::Pause
                    } else {
                        Symbol::PlayArrow
                    },
                    if state.playing { "Pause" } else { "Play" },
                )
                .variant(IconButtonVariant::Filled)
                .build(scheme, window, cx)
                .on_click(cx.listener(|root, _, _, cx| {
                    root.update_m3_gallery(cx, |gallery| gallery.playing = !gallery.playing);
                })),
            )
            .child(
                IconButton::new("m3-forward", Symbol::Forward5, "Forward 5 seconds")
                    .build(scheme, window, cx),
            )
            .child(
                div()
                    .px(theme::ui_rem(12.0))
                    .m3_type(typography::TITLE_SMALL)
                    .whitespace_nowrap()
                    .child("00:06.4 / 00:40.0"),
            )
            .child(
                Button::new("m3-split", "Split", ButtonVariant::Tonal)
                    .icon(Symbol::ContentCut)
                    .build(scheme, window, cx),
            )
            .child(
                IconButton::new("m3-delete", Symbol::Delete, "Delete clip")
                    .enabled(false)
                    .build(scheme, window, cx),
            )
            .child(
                IconButton::new("m3-frame", Symbol::AddPhotoAlternate, "Export frame")
                    .build(scheme, window, cx),
            );

        m3_surface(
            scheme.surface_container_lowest,
            shape::CORNER_EXTRA_LARGE - 4.0,
        )
        .relative()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .pt(theme::ui_rem(20.0))
        .px(theme::ui_rem(24.0))
        .pb(theme::ui_rem(84.0))
        .child(
            m3_surface(scheme.surface_container_high, shape::CORNER_MEDIUM)
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(theme::ui_rem(8.0))
                .text_color(color(scheme.on_surface_variant))
                .child(m3_icon(
                    Symbol::Movie,
                    false,
                    40.0,
                    color(scheme.on_surface_variant),
                ))
                .child(
                    div()
                        .m3_type(typography::LABEL_LARGE)
                        .child("recording_2026-09-25_16.30.58.mp4"),
                ),
        )
        .child(
            div()
                .absolute()
                .bottom(theme::ui_rem(12.0))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(controls),
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "The gallery keeps the ruler, track toggles, lanes, and playhead in layout order."
    )]
    fn timeline(
        state: &M3GalleryState,
        scheme: &ColorScheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let tracks = [
            (
                "Video",
                Symbol::Movie,
                false,
                [(0.0, 0.3), (0.3, 0.3), (0.6, 0.4)],
                (scheme.primary_container, scheme.on_primary_container),
            ),
            (
                "Overlay",
                Symbol::Layers,
                false,
                [(0.0, 0.075), (0.45, 0.1), (0.0, 0.0)],
                (scheme.tertiary_container, scheme.on_tertiary_container),
            ),
            (
                "Music",
                Symbol::MusicNote,
                true,
                [(0.0, 0.6), (0.0, 0.0), (0.0, 0.0)],
                (scheme.secondary_container, scheme.on_secondary_container),
            ),
        ];
        let rows: Vec<_> = tracks
            .into_iter()
            .enumerate()
            .map(|(index, (name, icon, audio, clips, (fill, on_fill)))| {
                let hidden = state.hidden_tracks[index];
                let toggle_icon = match (audio, hidden) {
                    (true, false) => Symbol::VolumeUp,
                    (true, true) => Symbol::VolumeOff,
                    (false, false) => Symbol::Visibility,
                    (false, true) => Symbol::VisibilityOff,
                };
                let lane = track_lane(scheme, (fill, on_fill), icon, name, clips, hidden);
                div()
                    .flex()
                    .items_center()
                    .gap(theme::ui_rem(12.0))
                    .h(theme::ui_rem(52.0))
                    .child(
                        div()
                            .w(theme::ui_rem(136.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(theme::ui_rem(8.0))
                            .pl(theme::ui_rem(8.0))
                            .child(m3_icon(icon, false, 20.0, color(scheme.on_surface_variant)))
                            .child(div().flex_1().m3_type(typography::LABEL_LARGE).child(name))
                            .child(
                                IconButton::new(
                                    format!("m3-track-{index}"),
                                    toggle_icon,
                                    "Toggle track",
                                )
                                .toggle(!hidden)
                                .build(scheme, window, cx)
                                .on_click(cx.listener(
                                    move |root, _, _, cx| {
                                        root.update_m3_gallery(cx, |gallery| {
                                            gallery.hidden_tracks[index] =
                                                !gallery.hidden_tracks[index];
                                        });
                                    },
                                )),
                            ),
                    )
                    .child(lane)
            })
            .collect();

        m3_surface(
            scheme.surface_container_low,
            shape::CORNER_EXTRA_LARGE - 4.0,
        )
        .rounded_b_none()
        .flex()
        .flex_col()
        .gap(theme::ui_rem(6.0))
        .pt(theme::ui_rem(8.0))
        .px(theme::ui_rem(16.0))
        .pb(theme::ui_rem(16.0))
        .child(timeline_header(scheme, window, cx))
        .child(
            div()
                .relative()
                .child(
                    div()
                        .ml(theme::ui_rem(148.0))
                        .h(theme::ui_rem(28.0))
                        .relative()
                        .children((0_u8..8).map(|index| {
                            div()
                                .absolute()
                                .left(relative(f32::from(index) / 8.0))
                                .top_0()
                                .bottom_0()
                                .border_l_1()
                                .border_color(color(scheme.outline_variant))
                                .pl(theme::ui_rem(4.0))
                                .m3_type(typography::BODY_SMALL)
                                .text_color(color(scheme.on_surface_variant))
                                .child(format!("00:{:02}", index * 5))
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(theme::ui_rem(6.0))
                        .children(rows),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(theme::ui_rem(148.0))
                        .right_0()
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(relative(0.16))
                                .w(theme::ui_rem(1.0))
                                .bg(color(scheme.on_surface))
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left(theme::ui_rem(-5.5))
                                        .size(theme::ui_rem(12.0))
                                        .rounded_full()
                                        .bg(color(scheme.on_surface)),
                                ),
                        ),
                ),
        )
    }
}

fn top_bar(scheme: &ColorScheme, window: &mut Window, cx: &mut Context<FrameRoot>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .h(theme::ui_rem(64.0))
        .pl(theme::ui_rem(8.0))
        .pr(theme::ui_rem(16.0))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .m3_type(typography::TITLE_MEDIUM)
                        .child("Untitled project"),
                )
                .child(
                    div()
                        .m3_type(typography::BODY_SMALL)
                        .text_color(color(scheme.on_surface_variant))
                        .child("3 tracks · 6 media · 00:40.0"),
                ),
        )
        .child(div().flex_1())
        .child(IconButton::new("m3-undo", Symbol::Undo, "Undo").build(scheme, window, cx))
        .child(IconButton::new("m3-redo", Symbol::Redo, "Redo").build(scheme, window, cx))
        .child(
            Button::new("m3-export", "Export", ButtonVariant::Filled)
                .icon(Symbol::FileExport)
                .build(scheme, window, cx),
        )
}

fn timeline_header(
    scheme: &ColorScheme,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(theme::ui_rem(12.0))
        .min_h(theme::ui_rem(40.0))
        .child(div().m3_type(typography::TITLE_SMALL).child("Timeline"))
        .child(
            div()
                .m3_type(typography::BODY_SMALL)
                .text_color(color(scheme.on_surface_variant))
                .child("recording_2026-09-25_16.30.58.mp4 · 00:00.0 – 00:12.0"),
        )
        .child(div().flex_1())
        .child(
            IconButton::new("m3-zoom-out", Symbol::ZoomOut, "Zoom out").build(scheme, window, cx),
        )
        .child(IconButton::new("m3-zoom-in", Symbol::ZoomIn, "Zoom in").build(scheme, window, cx))
}

/// A timeline lane with clip blocks. Clips are `(left, width)` fractions of the lane.
fn track_lane(
    scheme: &ColorScheme,
    (fill, on_fill): (crate::theme::RgbaToken, crate::theme::RgbaToken),
    icon: Symbol,
    name: &'static str,
    clips: [(f32, f32); 3],
    hidden: bool,
) -> gpui::Div {
    let clips = clips
        .into_iter()
        .enumerate()
        .filter(|(_, (_, width))| *width > 0.0)
        .map(|(index, (left, width))| {
            let selected = name == "Video" && index == 0;
            let label = match (name, index) {
                ("Video", 0) => "recording_2026-09-25_16.30.58.mp4",
                ("Video", _) => "screen_demo_v2.mov",
                ("Overlay", 0) => "title_card.png",
                ("Overlay", _) => "logo_spin.gif",
                _ => "lofi_background.mp3",
            };
            let clip_icon = match (name, index) {
                ("Overlay", 0) => Symbol::Image,
                ("Overlay", _) => Symbol::GifBox,
                _ => icon,
            };
            m3_surface(fill, shape::CORNER_SMALL)
                .absolute()
                .top(theme::ui_rem(4.0))
                .bottom(theme::ui_rem(4.0))
                .left(relative(left))
                .w(relative(width))
                .flex()
                .items_center()
                .gap(theme::ui_rem(6.0))
                .px(theme::ui_rem(16.0))
                .overflow_hidden()
                .text_color(color(on_fill))
                .when(selected, |clip| {
                    clip.border_2().border_color(color(scheme.primary))
                })
                .child(
                    div()
                        .absolute()
                        .left(theme::ui_rem(4.0))
                        .w(theme::ui_rem(2.0))
                        .h(theme::ui_rem(16.0))
                        .bg(color(on_fill)),
                )
                .child(
                    div()
                        .absolute()
                        .right(theme::ui_rem(4.0))
                        .w(theme::ui_rem(2.0))
                        .h(theme::ui_rem(16.0))
                        .bg(color(on_fill)),
                )
                .when(name == "Music", |clip| {
                    clip.child(
                        div()
                            .absolute()
                            .inset_0()
                            .px(theme::ui_rem(24.0))
                            .flex()
                            .items_center()
                            .gap(theme::ui_rem(2.0))
                            .children((0_u16..40).map(|bar| {
                                div()
                                    .flex_1()
                                    .h(theme::ui_rem(8.0 + f32::from((bar * 7) % 23)))
                                    .bg(color(on_fill.with_alpha(0.28)))
                            })),
                    )
                })
                .child(m3_icon(clip_icon, false, 16.0, color(on_fill)))
                .child(
                    div()
                        .m3_type(typography::LABEL_MEDIUM)
                        .truncate()
                        .child(label),
                )
                .child(
                    div()
                        .flex_none()
                        .m3_type(typography::BODY_SMALL)
                        .child(format!("{:.1} s", width * 40.0)),
                )
        });
    m3_surface(scheme.surface_container, shape::CORNER_SMALL)
        .relative()
        .flex_1()
        .h_full()
        .when(hidden, |this| this.opacity(0.4))
        .children(clips)
}

fn label_value_row(label: &'static str, value: &'static str, scheme: &ColorScheme) -> gpui::Div {
    div()
        .flex()
        .justify_between()
        .m3_type(typography::BODY_MEDIUM)
        .child(
            div()
                .text_color(color(scheme.on_surface_variant))
                .child(label),
        )
        .child(value)
}

fn switch_row(label: &'static str, switch: gpui::Stateful<gpui::Div>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .min_h(theme::ui_rem(48.0))
        .m3_type(typography::BODY_LARGE)
        .child(label)
        .child(switch)
}
