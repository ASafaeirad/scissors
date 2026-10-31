use super::{super::accessibility::apply_accessible_text_input, text::clamp_text_range, *};

pub(super) struct ScissorsTextInputElement {
    owner: Entity<ScissorsRoot>,
    kind: ScissorsTextInputKind,
    placeholder: SharedString,
    disabled: bool,
    focus_handle: FocusHandle,
    colors: ScissorsTextInputColors,
}

/// Paint colors for the editable text, so Scissors and M3 fields can share one input element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct ScissorsTextInputColors {
    pub(in crate::app) text: theme::RgbaToken,
    pub(in crate::app) placeholder: theme::RgbaToken,
    pub(in crate::app) caret: theme::RgbaToken,
    pub(in crate::app) selection: theme::RgbaToken,
}

impl ScissorsTextInputColors {
    pub(in crate::app) const fn from_palette(palette: &theme::ThemePalette) -> Self {
        Self {
            text: palette.text_primary,
            placeholder: palette.text_muted,
            caret: palette.text_primary,
            selection: palette.accent.with_alpha(0.30),
        }
    }
}

pub(super) struct ScissorsTextInputPrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
    metrics: ScissorsTextInputMetrics,
}

pub(in crate::app) const fn should_handle_text_input(
    disabled: bool,
    focused: bool,
    active: bool,
) -> bool {
    !disabled && focused && active
}

pub(in crate::app) const fn should_capture_text_input_drag(is_selecting: bool) -> bool {
    is_selecting
}

impl IntoElement for ScissorsTextInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ScissorsTextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = ScissorsTextInputPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let metrics = scissors_text_input_metrics(self.owner.read(cx));
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = px(metrics.control_height).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let root = self.owner.read(cx);
        let content = root.text_input_value(self.kind);
        let runtime = root.text_input_runtime(self.kind);
        let selected_range = clamp_text_range(&content, &runtime.selected_range);
        let cursor_offset = if runtime.selection_reversed {
            selected_range.start
        } else {
            selected_range.end
        };
        let is_placeholder = content.is_empty();
        let display_text: SharedString = if is_placeholder {
            theme::ui_text(self.placeholder.as_ref()).into()
        } else {
            content.into()
        };
        let mut style = window.text_style();
        style.color = if is_placeholder || self.disabled {
            color(self.colors.placeholder).into()
        } else {
            color(self.colors.text).into()
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &[run], None);
        let metrics = scissors_text_input_metrics(root);
        let text_top = bounds.top() + (bounds.size.height - px(metrics.caret_height)) / 2.0;
        let focused = self.focus_handle.is_focused(window);
        let should_reveal_cursor =
            focused && root.text_input_ui.active == Some(self.kind) && !is_placeholder;
        let cursor_x = line.x_for_index(cursor_offset);
        let scroll_x = if is_placeholder {
            Pixels::ZERO
        } else if should_reveal_cursor {
            text_input_scroll_x_for_cursor(
                runtime.scroll_x,
                cursor_x,
                line.width(),
                bounds.size.width,
                px(metrics.caret_width),
            )
        } else {
            clamp_text_input_scroll_x(runtime.scroll_x, line.width(), bounds.size.width)
        };
        let show_cursor = focused
            && root.text_input_ui.active == Some(self.kind)
            && root.text_input_ui.cursor_visible
            && window.is_window_active()
            && selected_range.is_empty();

        let cursor = show_cursor.then(|| {
            fill(
                Bounds::new(
                    point(bounds.left() + cursor_x - scroll_x, text_top),
                    size(px(metrics.caret_width), px(metrics.caret_height)),
                ),
                color(self.colors.caret),
            )
        });
        let selection = (!selected_range.is_empty()).then(|| {
            fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selected_range.start) - scroll_x,
                        text_top,
                    ),
                    point(
                        bounds.left() + line.x_for_index(selected_range.end) - scroll_x,
                        text_top + px(metrics.caret_height),
                    ),
                ),
                color(self.colors.selection),
            )
        });

        ScissorsTextInputPrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_x,
            metrics,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focused = self.focus_handle.is_focused(window);
        let kind = self.kind;
        let active = self.owner.read(cx).text_input_ui.active == Some(kind);

        if should_handle_text_input(self.disabled, focused, active) {
            window.handle_input(
                &self.focus_handle,
                ElementInputHandler::new(bounds, self.owner.clone()),
                cx,
            );
        }

        self.owner.update(cx, |root, cx| {
            if focused && root.text_input_ui.active != Some(kind) {
                root.activate_text_input(kind, cx);
            }
        });

        let line = prepaint.line.take().expect("input line should be shaped");
        let text_top =
            bounds.top() + (bounds.size.height - px(prepaint.metrics.caret_height)) / 2.0;
        let scroll_x = prepaint.scroll_x;
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(selection);
            }

            line.paint(
                point(bounds.left() - scroll_x, text_top),
                px(prepaint.metrics.caret_height),
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            )
            .ok();

            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });

        self.owner.update(cx, |root, _cx| {
            let runtime = root.text_input_runtime_mut(kind);
            runtime.last_layout = Some(line);
            runtime.last_bounds = Some(bounds);
            runtime.scroll_x = scroll_x;
        });
    }
}

#[derive(Clone, Copy)]
pub(in crate::app) struct ScissorsTextInputSpec<'a> {
    pub(in crate::app) id: &'static str,
    pub(in crate::app) value: &'a str,
    pub(in crate::app) placeholder: &'a str,
    pub(in crate::app) disabled: bool,
    pub(in crate::app) focus: Option<&'a FocusHandle>,
    pub(in crate::app) kind: ScissorsTextInputKind,
}

pub(in crate::app) fn scissors_text_input(
    spec: ScissorsTextInputSpec<'_>,
    palette: &'static theme::ThemePalette,
    _window: &Window,
    cx: &Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    let ScissorsTextInputSpec {
        id,
        value,
        placeholder,
        disabled,
        focus,
        kind,
    } = spec;
    let is_placeholder = value.is_empty();
    let label = if is_placeholder {
        theme::ui_text(placeholder)
    } else {
        value.to_string()
    };
    let label_color = if disabled || is_placeholder {
        palette.text_muted
    } else {
        palette.text_primary
    };

    let field = div()
        .id(id)
        .min_h(theme::ui_rem(SETTINGS_CONTROL_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .min_w_0()
        .rounded(theme::ui_rem(theme::RADIUS_SM))
        .bg(color(palette.canvas))
        .px(theme::ui_rem(10.0))
        .text_size(theme::ui_rem(theme::TEXT_UI_BASE_SIZE))
        .text_color(color(label_color))
        .opacity(if disabled { 0.5 } else { 1.0 })
        .shadow(input_highlight_shadows(palette));
    let field = text_input_editable(
        field,
        spec,
        ScissorsTextInputColors::from_palette(palette),
        cx,
    );
    let field = if focus.is_some() {
        field
    } else {
        field.child(div().w_full().min_w_0().truncate().child(label))
    };

    apply_accessible_text_input(field, kind.accessibility_label(), !disabled, value, palette)
}

/// The bare editable text for an M3 text field (`m3::TextField::input`): no box, padding or
/// focus ring, just the caret, selection and input handlers. Type styles come from the parent.
pub(in crate::app) fn m3_text_input(
    spec: ScissorsTextInputSpec<'_>,
    colors: ScissorsTextInputColors,
    cx: &Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    let field = div()
        .id(spec.id)
        .size_full()
        .min_w_0()
        .flex()
        .items_center();
    let field = text_input_editable(field, spec, colors, cx);
    let field = field
        .role(gpui::Role::TextInput)
        .aria_label(spec.kind.accessibility_label())
        .aria_value(spec.value.to_string());
    if spec.disabled {
        field.tab_stop(false).aria_disabled(true)
    } else {
        field
    }
}

/// Adds the key context, editing actions, pointer handlers and the editable element to `field`.
/// Without a focus handle the field gets handlers but no editable text.
fn text_input_editable(
    field: gpui::Stateful<gpui::Div>,
    spec: ScissorsTextInputSpec<'_>,
    colors: ScissorsTextInputColors,
    cx: &Context<ScissorsRoot>,
) -> gpui::Stateful<gpui::Div> {
    let ScissorsTextInputSpec {
        placeholder,
        disabled,
        focus,
        kind,
        ..
    } = spec;
    let field = field
        .key_context(if kind.is_preview_timecode() {
            SCISSORS_TIMECODE_TEXT_INPUT_CONTEXT
        } else {
            SCISSORS_TEXT_INPUT_CONTEXT
        })
        .when(!disabled, gpui::Styled::cursor_text)
        .when(disabled, gpui::Styled::cursor_not_allowed)
        .when(!disabled, |this| {
            this.on_action(cx.listener(ScissorsRoot::text_input_backspace))
                .on_action(cx.listener(ScissorsRoot::text_input_delete))
                .on_action(cx.listener(ScissorsRoot::text_input_left))
                .on_action(cx.listener(ScissorsRoot::text_input_right))
                .on_action(cx.listener(ScissorsRoot::text_input_select_left))
                .on_action(cx.listener(ScissorsRoot::text_input_select_right))
                .on_action(cx.listener(ScissorsRoot::text_input_home))
                .on_action(cx.listener(ScissorsRoot::text_input_end))
                .on_action(cx.listener(ScissorsRoot::text_input_select_all))
                .on_action(cx.listener(ScissorsRoot::text_input_copy))
                .on_action(cx.listener(ScissorsRoot::text_input_cut))
                .on_action(cx.listener(ScissorsRoot::text_input_paste))
                .when(kind.is_preview_timecode(), |this| {
                    this.on_action(cx.listener(ScissorsRoot::text_input_commit))
                        .on_action(cx.listener(ScissorsRoot::text_input_cancel))
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |root, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        root.text_input_mouse_down(kind, event, window, cx);
                    }),
                )
                .on_mouse_move(
                    cx.listener(move |root, event: &MouseMoveEvent, window, cx| {
                        if should_capture_text_input_drag(
                            root.text_input_runtime(kind).is_selecting,
                        ) {
                            cx.stop_propagation();
                            root.text_input_mouse_move(kind, event, window, cx);
                        }
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |root, event: &MouseUpEvent, window, cx| {
                        if should_capture_text_input_drag(
                            root.text_input_runtime(kind).is_selecting,
                        ) {
                            cx.stop_propagation();
                        }
                        root.text_input_mouse_up(kind, event, window, cx);
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(move |root, event: &MouseUpEvent, window, cx| {
                        if should_capture_text_input_drag(
                            root.text_input_runtime(kind).is_selecting,
                        ) {
                            cx.stop_propagation();
                            root.text_input_mouse_up(kind, event, window, cx);
                        }
                    }),
                )
        });

    let Some(focus) = focus else {
        return field;
    };
    let focus = focus.clone().tab_stop(!disabled);
    field.track_focus(&focus).child(ScissorsTextInputElement {
        owner: cx.entity(),
        kind,
        placeholder: SharedString::from(placeholder),
        disabled,
        focus_handle: focus,
        colors,
    })
}

fn scissors_text_input_metrics(root: &ScissorsRoot) -> ScissorsTextInputMetrics {
    ScissorsTextInputMetrics::from_ui_scale(root.appearance.ui_scale.factor())
}
