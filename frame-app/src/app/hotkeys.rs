use super::{ActiveView, Context, FocusHandle, FrameRoot, Window};

pub(super) const PREVIEW_SEEK_FOCUS_ID: &str = "preview-timeline-track";

impl FrameRoot {
    pub(super) fn handle_preview_shortcut(
        &mut self,
        event: &gpui::KeyDownEvent,
        root_focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        // Ctrl+Z undoes, and Ctrl+Shift+Z or Ctrl+Y redoes (Cmd on macOS).
        let history_shortcut = self.project.enabled
            && modifiers.secondary()
            && !modifiers.alt
            && matches!(key, "z" | "y");
        let timeline_shortcut =
            matches!(
                key,
                "space" | "x" | "q" | "e" | "z" | "y" | "delete" | "backspace"
            ) && self.registered_control_contains_focus(PREVIEW_SEEK_FOCUS_ID, window, cx);
        if (event.is_held || modifiers.modified()) && !history_shortcut
            || window.focused(cx).is_some() && !root_focus.is_focused(window) && !timeline_shortcut
            || self.active_view != ActiveView::Workspace
            || self.settings_ui.is_present
            || self.drag_drop_ui.is_present
        {
            return;
        }

        let handled = match key {
            "z" | "y" if history_shortcut => {
                if key == "y" || modifiers.shift {
                    self.redo_timeline_edit()
                } else {
                    self.undo_timeline_edit()
                }
            }
            "delete" | "backspace" if self.project.enabled => self.remove_selected_project_clip(),
            "space" => self.toggle_preview_playback_with_context(cx),
            "s" if self.preview_timeline_enabled() => {
                self.focus_registered_control(PREVIEW_SEEK_FOCUS_ID, window, cx)
            }
            "x" if self.project.enabled => self.split_project_clip_at_playhead(),
            "x" => self.toggle_preview_cut_at_playhead(),
            "q" if self.project.enabled => self.cut_project_track_at_playhead(true),
            "e" if self.project.enabled => self.cut_project_track_at_playhead(false),
            _ => false,
        };
        if handled {
            cx.stop_propagation();
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Context, FocusHandle, FrameRoot, PREVIEW_SEEK_FOCUS_ID, Window};
    use crate::app::{Entity, FrameFocusKey};
    use gpui::{
        AppContext, InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription,
        TestAppContext, VisualTestContext, div,
    };

    struct ShortcutTestView {
        root: Entity<FrameRoot>,
        focus: FocusHandle,
        _subscription: Subscription,
    }

    impl Render for ShortcutTestView {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let focus = self.focus.clone();
            self.root.update(cx, |root, cx| {
                let seek = root.ensure_focus(
                    FrameFocusKey::Control(PREVIEW_SEEK_FOCUS_ID.to_string()),
                    true,
                    cx,
                );
                let other = root.ensure_focus(
                    FrameFocusKey::Control("other-control".to_string()),
                    true,
                    cx,
                );
                let trim =
                    root.ensure_focus(FrameFocusKey::Control("trim-handle".to_string()), true, cx);
                div()
                    .id("shortcut-test")
                    .size_full()
                    .track_focus(&focus)
                    .on_action(cx.listener(FrameRoot::open_settings_action))
                    .child(
                        div()
                            .id(PREVIEW_SEEK_FOCUS_ID)
                            .size_20()
                            .track_focus(&seek)
                            .child(div().id("trim-handle").size_4().track_focus(&trim))
                            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, _, cx| {
                                if root.adjust_preview_timeline_from_keyboard_with_context(
                                    crate::preview::TimelineDragTarget::Scrub,
                                    &event.keystroke.key,
                                    cx,
                                ) {
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                            })),
                    )
                    .child(div().id("other-control").size_20().track_focus(&other))
                    .on_key_down(cx.listener(
                        move |root, event: &gpui::KeyDownEvent, window, cx| {
                            root.handle_preview_shortcut(event, &focus, window, cx);
                        },
                    ))
            })
        }
    }

    fn preview_window(cx: &mut TestAppContext) -> (Entity<FrameRoot>, &mut VisualTestContext) {
        let (view, cx) = cx.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            let root = cx.new(|_| {
                let mut root = FrameRoot::new();
                root.apply_preview_ready_fixture();
                root.preview_ui.playback = crate::app::preview_playback_state(
                    crate::app::PreviewMediaKind::Video,
                    90.4,
                    None,
                    None,
                );
                let cuts = root.selected_config().unwrap().cut_segments.clone();
                root.preview_ui.playback.sync_cut_segments(&cuts);
                root
            });
            let subscription = cx.observe(&root, |_, _, cx| cx.notify());
            ShortcutTestView {
                root,
                focus,
                _subscription: subscription,
            }
        });
        cx.update(|window, _| window.activate_window());
        draw(cx);
        let root = view.read_with(cx, |view, _| view.root.clone());
        (root, cx)
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.update(|window, cx| window.draw(cx).clear());
        cx.run_until_parked();
    }

    fn focus(root: &Entity<FrameRoot>, id: &str, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            root.update(cx, |root, cx| {
                assert!(root.focus_registered_control(id, window, cx));
            });
        });
        draw(cx);
    }

    #[test]
    fn ctrl_comma_opens_settings_from_a_focused_control() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::app::runtime::init_app(cx, "Frame"));
        let (root, cx) = preview_window(&mut app);
        focus(&root, "other-control", cx);
        cx.simulate_keystrokes("ctrl-,");
        assert!(root.read_with(cx, |root, _| root.settings_ui.is_open));
    }

    #[test]
    fn preview_shortcuts_toggle_playback_and_cut_at_playhead() {
        let mut app = TestAppContext::single();
        let (root, cx) = preview_window(&mut app);
        cx.simulate_keystrokes("space");
        assert!(root.read_with(cx, |root, _| root.preview_ui.playback.is_playing()));
        cx.simulate_keystrokes("space");
        assert!(!root.read_with(cx, |root, _| root.preview_ui.playback.is_playing()));

        let cuts = root.read_with(cx, |root, _| {
            root.selected_config().unwrap().cut_segments.len()
        });
        cx.simulate_keystrokes("x");
        assert_eq!(
            root.read_with(cx, |root, _| root
                .selected_config()
                .unwrap()
                .cut_segments
                .len()),
            cuts + 1,
        );
    }

    #[test]
    fn seek_shortcut_focuses_timeline_and_arrow_keys_seek() {
        let mut app = TestAppContext::single();
        let (root, cx) = preview_window(&mut app);
        let before = root.read_with(cx, |root, _| root.preview_ui.playback.current_time());
        cx.simulate_keystrokes("s");
        cx.update(|window, cx| {
            root.update(cx, |root, cx| {
                let seek = root.ensure_focus(
                    FrameFocusKey::Control(PREVIEW_SEEK_FOCUS_ID.to_string()),
                    true,
                    cx,
                );
                assert!(seek.is_focused(window));
            });
        });
        draw(cx);
        cx.simulate_keystrokes("right");
        assert!(root.read_with(cx, |root, _| root.preview_ui.playback.current_time()) > before);
    }

    #[test]
    fn preview_shortcuts_ignore_focused_controls_modifiers_and_repeat() {
        let mut app = TestAppContext::single();
        let (root, cx) = preview_window(&mut app);
        let cuts = root.read_with(cx, |root, _| {
            root.selected_config().unwrap().cut_segments.clone()
        });
        for key in ["ctrl-space", "ctrl-s", "ctrl-x", "shift-space"] {
            cx.simulate_keystrokes(key);
        }
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("space").unwrap(),
            is_held: true,
            prefer_character_input: false,
        });
        focus(&root, "other-control", cx);
        for key in ["space", "s", "x"] {
            cx.simulate_keystrokes(key);
        }
        assert!(!root.read_with(cx, |root, _| root.preview_ui.playback.is_playing()));
        assert_eq!(
            root.read_with(cx, |root, _| root
                .selected_config()
                .unwrap()
                .cut_segments
                .clone()),
            cuts
        );
    }

    #[test]
    fn timeline_focus_allows_playback_and_new_segments() {
        let mut app = TestAppContext::single();
        let (root, cx) = preview_window(&mut app);
        cx.simulate_keystrokes("s");
        draw(cx);
        for id in [PREVIEW_SEEK_FOCUS_ID, "trim-handle"] {
            focus(&root, id, cx);
            cx.simulate_keystrokes("space");
            assert!(root.read_with(cx, |root, _| root.preview_ui.playback.is_playing()));
            cx.simulate_keystrokes("space");
            assert!(!root.read_with(cx, |root, _| root.preview_ui.playback.is_playing()));
            let cuts = root.read_with(cx, |root, _| {
                root.selected_config().unwrap().cut_segments.len()
            });
            cx.simulate_keystrokes("x");
            assert_eq!(
                root.read_with(cx, |root, _| root
                    .selected_config()
                    .unwrap()
                    .cut_segments
                    .len()),
                cuts + 1
            );
            // Restore this cut so the next focus case starts at an uncut playhead.
            cx.simulate_keystrokes("x");
            assert_eq!(
                root.read_with(cx, |root, _| root
                    .selected_config()
                    .unwrap()
                    .cut_segments
                    .len()),
                cuts
            );
        }
    }

    #[test]
    fn preview_shortcuts_ignore_settings_and_unavailable_media() {
        let mut app = TestAppContext::single();
        let (root, cx) = preview_window(&mut app);
        let cuts = root.read_with(cx, |root, _| {
            root.selected_config().unwrap().cut_segments.clone()
        });
        root.update(cx, |root, _| root.settings_ui.is_present = true);
        for key in ["space", "s", "x"] {
            cx.simulate_keystrokes(key);
        }
        assert!(!root.read_with(cx, |root, _| root.preview_ui.playback.is_playing()));
        assert_eq!(
            root.read_with(cx, |root, _| root
                .selected_config()
                .unwrap()
                .cut_segments
                .clone()),
            cuts
        );

        root.update(cx, |root, _| {
            root.settings_ui.is_present = false;
            root.preview_ui.playback = crate::app::preview_playback_state(
                crate::app::PreviewMediaKind::Video,
                0.0,
                None,
                None,
            );
        });
        for key in ["space", "s", "x"] {
            cx.simulate_keystrokes(key);
        }
        assert!(!root.read_with(cx, |root, _| root.preview_ui.playback.is_playing()));
        assert_eq!(
            root.read_with(cx, |root, _| root
                .selected_config()
                .unwrap()
                .cut_segments
                .clone()),
            cuts
        );
    }
}
