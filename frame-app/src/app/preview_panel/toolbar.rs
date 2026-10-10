use super::super::components::{FrameTooltip, TooltipPlacement};
use super::{
    ClickEvent, Context, FlipAxis, FrameRoot, InteractiveElement, PREVIEW_TOOLBAR_OFFSET,
    ParentElement, PreviewCanvasZoomDirection, PreviewShellState, PreviewToolFocuses,
    StatefulInteractiveElement, Styled, Window, assets, color, div,
    preview_visual_controls_enabled, relative, theme,
};
use crate::app::m3;
use crate::material::{self, Symbol, shape};
use gpui::FocusHandle;

const M3_TOOL_BUTTON_SIZE: f32 = 40.0;
const PREVIEW_TOOLBAR_PADDING: f32 = 4.0;
const PREVIEW_TOOLBAR_GAP: f32 = 8.0;
const PREVIEW_TOOLBAR_VERTICAL_SEPARATOR_HEIGHT: f32 = 18.0;
const PREVIEW_TOOLBAR_VERTICAL_SEPARATOR_WIDTH: f32 = 1.0;
const PREVIEW_TOOLBAR_BUTTON_COUNT: f32 = 5.0;
const PREVIEW_TOOLBAR_GAP_COUNT: f32 = 4.0;

pub(in crate::app) const fn preview_toolbar_height() -> f32 {
    (PREVIEW_TOOLBAR_PADDING * 2.0)
        + (M3_TOOL_BUTTON_SIZE * PREVIEW_TOOLBAR_BUTTON_COUNT)
        + (PREVIEW_TOOLBAR_GAP * PREVIEW_TOOLBAR_GAP_COUNT)
}

pub(in crate::app) const fn preview_toolbar_center_margin() -> f32 {
    -(preview_toolbar_height() / 2.0)
}

#[expect(
    clippy::too_many_lines,
    reason = "The declarative toolbar layout keeps its mutually dependent controls in visual order."
)]
pub(in crate::app) fn preview_toolbar(
    state: &PreviewShellState,
    focuses: PreviewToolFocuses<'_>,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let palette = state.palette;
    let scheme = toolbar_scheme(palette);
    let transform_enabled = preview_visual_controls_enabled(state);
    let crop_enabled = transform_enabled && state.crop.has_crop_dimensions;
    let overlay_enabled = transform_enabled && state.availability.overlay_available;

    div()
        .absolute()
        .top(relative(0.5))
        .mt(theme::ui_rem(preview_toolbar_center_margin()))
        .left(theme::ui_rem(PREVIEW_TOOLBAR_OFFSET))
        .flex()
        .flex_col()
        .gap(theme::ui_rem(PREVIEW_TOOLBAR_GAP))
        .rounded(theme::ui_rem(shape::CORNER_EXTRA_LARGE))
        .bg(color(scheme.surface_container))
        .p(theme::ui_rem(PREVIEW_TOOLBAR_PADDING))
        .child(
            preview_tool_button(
                "preview-tool-rotate",
                assets::ICON_ROTATE_CW,
                "Rotate preview",
                false,
                transform_enabled,
                state.tooltip_visible_id.as_deref(),
                TooltipPlacement::Right,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                if root.rotate_selected_preview() {
                    cx.notify();
                }
            })),
        )
        .child(
            preview_tool_button(
                "preview-tool-flip-horizontal",
                assets::ICON_FLIP_HORIZONTAL,
                "Flip horizontally",
                state.crop.flip_horizontal,
                transform_enabled,
                state.tooltip_visible_id.as_deref(),
                TooltipPlacement::Right,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                if root.toggle_selected_flip(FlipAxis::Horizontal) {
                    cx.notify();
                }
            })),
        )
        .child(
            preview_tool_button(
                "preview-tool-flip-vertical",
                assets::ICON_FLIP_VERTICAL,
                "Flip vertically",
                state.crop.flip_vertical,
                transform_enabled,
                state.tooltip_visible_id.as_deref(),
                TooltipPlacement::Right,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                if root.toggle_selected_flip(FlipAxis::Vertical) {
                    cx.notify();
                }
            })),
        )
        .child(
            preview_tool_button_with_focus(
                "preview-tool-crop",
                assets::ICON_CROP,
                "Crop",
                state.crop.crop_mode || state.crop.applied_crop.is_some(),
                crop_enabled,
                focuses.crop,
                state.tooltip_visible_id.as_deref(),
                TooltipPlacement::Right,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                if root.toggle_selected_crop_mode() {
                    cx.notify();
                }
            })),
        )
        .child(
            preview_tool_button_with_focus(
                "preview-tool-overlay",
                assets::ICON_FILE_IMAGE,
                "Overlay image",
                state.overlay.overlay_mode || state.overlay.has_overlay,
                overlay_enabled,
                focuses.overlay,
                state.tooltip_visible_id.as_deref(),
                TooltipPlacement::Right,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, window, cx| {
                if root.trigger_selected_overlay(window, cx) {
                    cx.notify();
                }
            })),
        )
}

pub(in crate::app) fn preview_zoom_toolbar(
    state: &PreviewShellState,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Div {
    let palette = state.palette;
    let scheme = toolbar_scheme(palette);
    let enabled = preview_visual_controls_enabled(state);

    div()
        .absolute()
        .right(theme::ui_rem(PREVIEW_TOOLBAR_OFFSET))
        .bottom(theme::ui_rem(PREVIEW_TOOLBAR_OFFSET))
        .flex()
        .gap_2()
        .rounded(theme::ui_rem(shape::CORNER_EXTRA_LARGE))
        .bg(color(scheme.surface_container))
        .p(theme::ui_rem(4.0))
        .child(
            preview_tool_button(
                "preview-zoom-out",
                assets::ICON_MINUS,
                "Zoom out",
                false,
                enabled,
                state.tooltip_visible_id.as_deref(),
                TooltipPlacement::Above,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                if root.zoom_preview_canvas(PreviewCanvasZoomDirection::Out, cx) {
                    cx.notify();
                }
            })),
        )
        .child(
            preview_tool_button(
                "preview-zoom-in",
                assets::ICON_PLUS,
                "Zoom in",
                false,
                enabled,
                state.tooltip_visible_id.as_deref(),
                TooltipPlacement::Above,
                palette,
                window,
                cx,
            )
            .on_click(cx.listener(|root, _: &ClickEvent, _window, cx| {
                if root.zoom_preview_canvas(PreviewCanvasZoomDirection::In, cx) {
                    cx.notify();
                }
            })),
        )
}

pub(in crate::app) fn preview_toolbar_vertical_separator(
    palette: &'static theme::ThemePalette,
) -> gpui::Div {
    div()
        .flex_none()
        .h(theme::ui_rem(PREVIEW_TOOLBAR_VERTICAL_SEPARATOR_HEIGHT))
        .w(theme::ui_rem(PREVIEW_TOOLBAR_VERTICAL_SEPARATOR_WIDTH))
        .bg(color(palette.fill_selected))
}

#[expect(
    clippy::too_many_arguments,
    reason = "Toolbar buttons keep semantics, interaction state, palette, and render context explicit."
)]
pub(in crate::app) fn preview_tool_button(
    id: impl Into<String>,
    icon: &'static str,
    label: impl Into<String>,
    selected: bool,
    enabled: bool,
    tooltip_visible_id: Option<&str>,
    tooltip_placement: TooltipPlacement,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    preview_tool_button_inner(
        id,
        icon,
        label,
        selected,
        enabled,
        None,
        tooltip_visible_id,
        tooltip_placement,
        palette,
        window,
        cx,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Preview tool buttons need explicit focus handles for modal focus restoration."
)]
pub(in crate::app) fn preview_tool_button_with_focus(
    id: impl Into<String>,
    icon: &'static str,
    label: impl Into<String>,
    selected: bool,
    enabled: bool,
    focus: &FocusHandle,
    tooltip_visible_id: Option<&str>,
    tooltip_placement: TooltipPlacement,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    preview_tool_button_inner(
        id,
        icon,
        label,
        selected,
        enabled,
        Some(focus),
        tooltip_visible_id,
        tooltip_placement,
        palette,
        window,
        cx,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "The shared tool button builder preserves the existing visual contract and optionally wires a focus handle."
)]
fn preview_tool_button_inner(
    id: impl Into<String>,
    icon: &'static str,
    label: impl Into<String>,
    selected: bool,
    enabled: bool,
    focus: Option<&FocusHandle>,
    tooltip_visible_id: Option<&str>,
    tooltip_placement: TooltipPlacement,
    palette: &'static theme::ThemePalette,
    window: &mut Window,
    cx: &mut Context<FrameRoot>,
) -> gpui::Stateful<gpui::Div> {
    let button_id = id.into();
    let label = label.into();
    let symbol = match icon {
        assets::ICON_ROTATE_CW => Symbol::Rotate90DegreesCw,
        assets::ICON_FLIP_HORIZONTAL | assets::ICON_FLIP_VERTICAL => Symbol::Flip,
        assets::ICON_CROP => Symbol::Crop,
        assets::ICON_FILE_IMAGE => Symbol::AddPhotoAlternate,
        assets::ICON_MINUS => Symbol::ZoomOut,
        assets::ICON_PLUS => Symbol::ZoomIn,
        _ => Symbol::MoreVert,
    };
    let mut button = m3::IconButton::new(format!("{button_id}-visual"), symbol, label.clone())
        .toggle(selected)
        .enabled(enabled);
    if let Some(focus) = focus {
        button = button.focus(focus);
    }
    let button = div()
        .id(button_id.clone())
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .aria_disabled(!enabled)
        .child(
            button
                .build(toolbar_scheme(palette), window, cx)
                .tab_stop(false),
        );
    FrameTooltip {
        hovered: tooltip_visible_id == Some(button_id.as_str()),
        id: button_id,
        label,
        placement: tooltip_placement,
        anchor_size: M3_TOOL_BUTTON_SIZE,
    }
    .for_control(button, focus, enabled, palette, window, cx)
}

fn toolbar_scheme(palette: &theme::ThemePalette) -> &'static material::ColorScheme {
    material::scheme(if palette.is_light() {
        crate::appearance::ColorTheme::Light
    } else {
        crate::appearance::ColorTheme::Dark
    })
}
