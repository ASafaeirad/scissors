#![expect(
    clippy::float_cmp,
    reason = "App state tests compare exact deterministic layout and timeline values."
)]

use super::input::{
    should_capture_text_input_drag, should_handle_text_input, text_input_scroll_x_for_cursor,
};
use super::preview_actions::{
    lerp_preview_canvas_value, lerp_preview_canvas_value_for_elapsed, preview_canvas_initial_zoom,
    preview_canvas_layout_metrics, preview_canvas_pan_limits, preview_canvas_transform_settled,
    preview_canvas_transform_visual_delta, preview_crop_keyboard_delta,
    preview_overlay_keyboard_delta, preview_runtime_dimensions,
};
use super::preview_panel::{
    CropSourceDimensions, crop_size_label, format_clip_duration, format_timeline_ruler_time,
    format_timeline_time, preview_crop_visual_rect, preview_presented_frame, preview_shell_state,
    preview_timeline_info, preview_timeline_tracks, preview_trim_enabled,
    preview_visual_controls_visible, timeline_fraction_from_percent,
    timeline_keyboard_time_for_key, timeline_ruler_ticks, timeline_slider_percent_from_bounds,
};
use super::primitives::frame_highlight_px;
use super::*;
use crate::app_persistence::{AppPersistence, AppSettings};
use crate::notifications::{AppNotifier, ConversionNotificationSummary};
use crate::preview_engine::{
    PreviewCrop as EnginePreviewCrop, PreviewFrame, render_image_from_frame,
};
use crate::{TITLEBAR_MACOS_NATIVE_TRAFFIC_LIGHT_X, TITLEBAR_MACOS_NATIVE_TRAFFIC_LIGHT_Y};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static TEST_SETTINGS_PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

mod scissors_root_imports {
    use super::*;

    #[test]
    fn allocate_file_imports_assigns_incrementing_ids() {
        let mut root = ScissorsRoot::new();

        let imports = root.allocate_file_imports(vec![
            PathBuf::from("/tmp/one.mp4"),
            PathBuf::from("/tmp/two.mp4"),
        ]);

        assert_eq!(imports[0].0, "file-1");
        assert_eq!(imports[1].0, "file-2");
    }

    #[test]
    fn drag_drop_overlay_stays_present_until_close_motion_finishes() {
        let mut root = ScissorsRoot::new();
        root.drag_drop_ui.is_open = true;
        root.drag_drop_ui.is_present = true;

        assert!(root.close_drag_drop_overlay());

        assert!(!root.drag_drop_ui.is_open);
        assert!(root.drag_drop_ui.is_present);
        assert!(root.finish_drag_drop_overlay_close());
        assert!(!root.drag_drop_ui.is_present);
        assert!(!root.finish_drag_drop_overlay_close());
    }

    #[test]
    fn drag_drop_overlay_open_is_stable_without_pointer_motion() {
        let mut root = ScissorsRoot::new();

        assert!(root.open_drag_drop_overlay());
        assert!(!root.open_drag_drop_overlay());
        assert!(root.drag_drop_ui.is_open);
        assert!(root.drag_drop_ui.is_present);
    }

    #[test]
    fn allocate_file_imports_continues_after_previous_batch() {
        let mut root = ScissorsRoot::new();
        root.allocate_file_imports(vec![PathBuf::from("/tmp/one.mp4")]);

        let imports = root.allocate_file_imports(vec![PathBuf::from("/tmp/two.mp4")]);

        assert_eq!(imports[0].0, "file-2");
    }

    #[test]
    fn allocate_file_imports_returns_empty_for_empty_drop() {
        let mut root = ScissorsRoot::new();

        let imports = root.allocate_file_imports(Vec::new());

        assert!(imports.is_empty());
    }

    #[test]
    fn allocate_file_imports_skips_unsupported_source_extensions() {
        let mut root = ScissorsRoot::new();

        let imports = root.allocate_file_imports(vec![
            PathBuf::from("/tmp/one.mp4"),
            PathBuf::from("/tmp/readme.txt"),
            PathBuf::from("/tmp/two.PNG"),
        ]);

        assert_eq!(
            imports,
            [
                ("file-1".to_string(), PathBuf::from("/tmp/one.mp4")),
                ("file-2".to_string(), PathBuf::from("/tmp/two.PNG")),
            ]
        );
    }

    #[test]
    fn allocate_file_imports_does_not_advance_ids_for_unsupported_sources() {
        let mut root = ScissorsRoot::new();
        root.allocate_file_imports(vec![PathBuf::from("/tmp/readme.txt")]);

        let imports = root.allocate_file_imports(vec![PathBuf::from("/tmp/clip.mov")]);

        assert_eq!(imports[0].0, "file-1");
    }
}

mod scissors_root_conversion {
    use super::*;
    fn root_with_output_directory() -> ScissorsRoot {
        let mut root = ScissorsRoot::new();
        root.default_output_directory = Some(PathBuf::from("/tmp/scissors-output"));
        root
    }

    #[test]
    fn prepare_project_export_requires_default_output_directory() {
        let mut root = super::super::project::tests::project();
        root.add_library_clip("source");
        let before = root.file_queue.files().to_vec();
        assert!(root.prepare_project_export().is_empty());
        assert_eq!(root.file_queue.files(), before);
    }

    #[test]
    fn apply_conversion_event_updates_processing_state_from_queue() {
        let mut root = root_with_output_directory();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue.queue_selected_pending_conversions();
        root.is_processing = true;

        root.apply_conversion_event(ConversionEvent::completed("first", "/tmp/one.mp4"));

        assert!(!root.is_processing);
        assert_eq!(
            root.file_queue.file_by_id("first").map(|file| file.status),
            Some(FileStatus::Completed)
        );
    }

    #[test]
    fn apply_conversion_event_notifies_when_active_batch_settles() {
        let notifications = std::sync::Arc::new(Mutex::new(Vec::new()));
        let received_notifications = notifications.clone();
        let mut root = ScissorsRoot::new_with_notifier(
            AppNotifier::from_conversion_finished_handler(move |summary| {
                received_notifications
                    .lock()
                    .expect("notifications should be writable")
                    .push(summary);
            }),
        );
        root.default_output_directory = Some(PathBuf::from("/tmp/scissors-output"));
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .add_file(FileItem::from_path("second", "/tmp/two.mp4", 1));
        root.file_queue.queue_selected_pending_conversions();
        root.active_conversion_task_ids = vec!["first".to_string(), "second".to_string()];
        root.is_processing = true;

        root.apply_conversion_event(ConversionEvent::completed("first", "/tmp/one.mp4"));
        root.apply_conversion_event(ConversionEvent::error("second", "ffmpeg failed"));

        assert_eq!(
            notifications
                .lock()
                .expect("notifications should be readable")
                .as_slice(),
            [ConversionNotificationSummary {
                completed_count: 1,
                error_count: 1,
            }]
        );
    }

    #[test]
    fn apply_conversion_event_does_not_notify_when_active_batch_has_no_results() {
        let notifications = std::sync::Arc::new(Mutex::new(Vec::new()));
        let received_notifications = notifications.clone();
        let mut root = ScissorsRoot::new_with_notifier(
            AppNotifier::from_conversion_finished_handler(move |summary| {
                received_notifications
                    .lock()
                    .expect("notifications should be writable")
                    .push(summary);
            }),
        );
        root.default_output_directory = Some(PathBuf::from("/tmp/scissors-output"));
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue.queue_selected_pending_conversions();
        root.active_conversion_task_ids = vec!["first".to_string()];
        root.is_processing = true;

        root.apply_conversion_event(ConversionEvent::cancelled("first"));

        assert!(
            notifications
                .lock()
                .expect("notifications should be readable")
                .is_empty()
        );
    }

    #[test]
    fn remove_file_from_queue_cancels_and_removes_paused_file() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .update_status("first", FileStatus::Paused, 30);
        root.conversion_events
            .apply_conversion_event(&mut root.file_queue, ConversionEvent::log("first", "line"));

        assert!(root.remove_file_from_queue("first"));

        assert!(root.file_queue.file_by_id("first").is_none());
        assert!(root.conversion_events.logs_for("first").is_empty());
    }

    #[test]
    fn cancel_conversion_task_keeps_source_until_runner_confirms_cancellation() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .update_status("first", FileStatus::Queued, 0);

        assert!(root.cancel_conversion_task("first"));
        assert_eq!(
            root.file_queue.file_by_id("first").map(|file| file.status),
            Some(FileStatus::Cancelling)
        );

        root.apply_conversion_event(ConversionEvent::cancelled("first"));

        assert_eq!(
            root.file_queue.file_by_id("first").map(|file| file.status),
            Some(FileStatus::Idle)
        );
    }

    #[test]
    fn prepare_file_for_reconversion_keeps_source_settings_and_enables_start() {
        let mut root = ScissorsRoot::new();
        let mut file = FileItem::from_path("first", "/tmp/one.mp4", 1);
        file.config.container = "mkv".to_string();
        root.file_queue.add_file(file);
        root.file_queue
            .update_status("first", FileStatus::Completed, 100);

        assert!(root.prepare_file_for_reconversion("first"));

        let file = root
            .file_queue
            .file_by_id("first")
            .expect("source should remain in queue");
        assert_eq!(file.status, FileStatus::Idle);
        assert_eq!(file.config.container, "mkv");
        root.default_output_directory = Some(PathBuf::from("/tmp/scissors-output"));
        assert!(root.app_state().can_start_conversion());
    }

    #[test]
    fn exports_are_limited_to_one_project() {
        let root = ScissorsRoot::new();

        assert_eq!(
            root.conversion_processes
                .current_max_concurrency()
                .expect("max concurrency should be readable"),
            1
        );
    }

    #[test]
    fn legacy_parallelism_is_ignored_while_custom_presets_are_restored() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        persistence
            .save(&AppSettings {
                max_concurrency: 6,
                custom_presets: vec![PresetDefinition::custom(
                    "custom-preset-7".to_string(),
                    "Review MP4".to_string(),
                    ConversionConfig::default(),
                )],
                ..AppSettings::default()
            })
            .expect("settings should be saved");

        let root = ScissorsRoot::new_with_persistence(persistence);

        assert_eq!(
            root.conversion_processes
                .current_max_concurrency()
                .expect("max concurrency should be readable"),
            1
        );
        assert!(
            root.presets
                .iter()
                .any(|preset| preset.name == "Review MP4")
        );
        assert_eq!(root.settings_ui.next_custom_preset_sequence, 7);
    }

    #[test]
    fn new_with_persistence_hydrates_appearance_before_rendering() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        persistence
            .save(&AppSettings {
                appearance: AppearanceSettings {
                    ui_scale: ScalePreset::Percent175,
                    color_theme: crate::appearance::ColorTheme::Light,
                    follow_system_theme: false,
                    accent_color: crate::appearance::AccentColor::default(),
                },
                ..AppSettings::default()
            })
            .expect("settings should be saved");

        let root = ScissorsRoot::new_with_persistence(persistence);

        assert_eq!(
            root.appearance,
            AppearanceSettings {
                ui_scale: ScalePreset::Percent175,
                color_theme: crate::appearance::ColorTheme::Light,
                follow_system_theme: false,
                accent_color: crate::appearance::AccentColor::default(),
            }
        );
    }

    #[test]
    fn default_output_directory_persists_for_future_sessions() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        let mut root = ScissorsRoot::new_with_persistence(persistence.clone());

        root.set_default_output_directory(PathBuf::from("/tmp/scissors-output"))
            .expect("output directory should persist");

        assert_eq!(
            persistence
                .load()
                .expect("settings should be readable")
                .default_output_directory,
            Some(PathBuf::from("/tmp/scissors-output"))
        );
        assert_eq!(
            ScissorsRoot::new_with_persistence(persistence).default_output_directory,
            Some(PathBuf::from("/tmp/scissors-output"))
        );
    }

    #[test]
    fn output_directory_save_failure_opens_settings_with_recovery_message() {
        let blocker = test_settings_path();
        std::fs::create_dir_all(blocker.parent().expect("test path should have a parent"))
            .expect("test settings parent should be created");
        std::fs::write(&blocker, b"not a directory").expect("blocker file should be created");
        let persistence = AppPersistence::from_settings_path(blocker.join("settings.json"));
        let mut root = ScissorsRoot::new_with_persistence(persistence);

        assert!(!root.apply_default_output_directory(PathBuf::from("/tmp/scissors-output")));

        assert!(root.settings_ui.is_open);
        assert!(root.default_output_directory.is_none());
        assert!(
            root.settings_ui
                .output_directory_error
                .as_deref()
                .is_some_and(|error| error.starts_with("Failed to save settings:"))
        );
    }

    #[test]
    fn app_settings_close_keeps_sheet_present_until_motion_finishes() {
        let mut root = ScissorsRoot::new();

        root.open_app_settings();
        root.close_app_settings();

        assert!(!root.settings_ui.is_open);
        assert!(root.settings_ui.is_present);

        assert!(root.finish_app_settings_close());
        assert!(!root.settings_ui.is_present);
    }

    #[test]
    fn app_settings_sheet_motion_keeps_final_edge_inset() {
        assert_eq!(settings_sheet_right_inset(1.0), 0.0);
        assert_eq!(settings_sheet_right_inset(0.0), -24.0);
    }

    #[test]
    fn ui_scale_selection_persists() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        let mut root = ScissorsRoot::new_with_persistence(persistence.clone());

        assert!(root.set_ui_scale(ScalePreset::Percent125));

        assert_eq!(
            persistence
                .load()
                .expect("appearance settings should be readable")
                .appearance,
            AppearanceSettings {
                ui_scale: ScalePreset::Percent125,
                color_theme: crate::appearance::ColorTheme::Dark,
                follow_system_theme: false,
                accent_color: crate::appearance::AccentColor::default(),
            }
        );
    }

    #[test]
    fn accent_changes_persist_and_preserve_theme_and_scale() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        let mut root = ScissorsRoot::new_with_persistence(persistence.clone());
        root.appearance.color_theme = ColorTheme::Light;
        root.appearance.ui_scale = ScalePreset::Percent150;
        root.open_app_settings();
        root.set_accent_color(crate::appearance::AccentColor::PRESETS[2].1);
        let expected = crate::appearance::AccentColor::from_hex("#006A60").unwrap();
        assert_eq!(root.appearance.accent_color, expected);
        assert_eq!(root.appearance.color_theme, ColorTheme::Light);
        assert_eq!(root.appearance.ui_scale, ScalePreset::Percent150);
        assert_eq!(persistence.load().unwrap().appearance, root.appearance);
        root.close_app_settings();
        root.open_app_settings();
        assert_eq!(root.appearance.accent_color, expected);
        assert!(root.set_accent_color(crate::appearance::AccentColor::default()));
        assert_eq!(
            persistence.load().unwrap().appearance.accent_color.hex(),
            "#0061A4"
        );
    }

    #[test]
    fn accent_save_failure_keeps_the_previous_palette() {
        let blocker = test_settings_path();
        std::fs::create_dir_all(blocker.parent().unwrap()).unwrap();
        std::fs::write(&blocker, b"not a directory").unwrap();
        let mut root = ScissorsRoot::new_with_persistence(AppPersistence::from_settings_path(
            blocker.join("settings.json"),
        ));
        root.set_accent_color(crate::appearance::AccentColor::PRESETS[2].1);
        assert_eq!(
            root.appearance.accent_color,
            crate::appearance::AccentColor::default()
        );
        assert!(root.settings_ui.appearance_error.is_some());
    }

    #[test]
    fn color_theme_selection_persists_without_changing_ui_scale() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        let mut root = ScissorsRoot::new_with_persistence(persistence.clone());
        root.appearance.ui_scale = ScalePreset::Percent150;

        assert!(root.set_color_theme(crate::appearance::ColorTheme::Light));

        assert_eq!(root.appearance.ui_scale, ScalePreset::Percent150);
        assert_eq!(
            persistence
                .load()
                .expect("appearance settings should be readable")
                .appearance,
            AppearanceSettings {
                ui_scale: ScalePreset::Percent150,
                color_theme: crate::appearance::ColorTheme::Light,
                follow_system_theme: false,
                accent_color: crate::appearance::AccentColor::default(),
            }
        );
    }

    #[test]
    fn ui_scale_selection_preserves_color_theme() {
        let mut root = ScissorsRoot::new();
        root.appearance.color_theme = crate::appearance::ColorTheme::Light;

        assert!(root.set_ui_scale(ScalePreset::Percent125));

        assert_eq!(
            root.appearance.color_theme,
            crate::appearance::ColorTheme::Light
        );
    }

    #[test]
    fn selecting_active_theme_is_a_no_op_without_attempting_to_save() {
        let blocker = test_settings_path();
        std::fs::create_dir_all(blocker.parent().expect("test path should have a parent"))
            .expect("test settings parent should be created");
        std::fs::write(&blocker, b"not a directory").expect("blocker file should be created");
        let persistence = AppPersistence::from_settings_path(blocker.join("settings.json"));
        let mut root = ScissorsRoot::new_with_persistence(persistence);

        assert!(!root.set_color_theme(crate::appearance::ColorTheme::Dark));
        assert!(root.settings_ui.appearance_error.is_none());
    }

    #[test]
    fn appearance_save_failure_rolls_runtime_state_back() {
        let blocker = test_settings_path();
        std::fs::create_dir_all(blocker.parent().expect("test path should have a parent"))
            .expect("test settings parent should be created");
        std::fs::write(&blocker, b"not a directory").expect("blocker file should be created");
        let persistence = AppPersistence::from_settings_path(blocker.join("settings.json"));
        let mut root = ScissorsRoot::new_with_persistence(persistence);

        assert!(!root.set_ui_scale(ScalePreset::Percent150));

        assert_eq!(root.appearance, AppearanceSettings::default());
        assert!(
            root.settings_ui
                .appearance_error
                .as_deref()
                .is_some_and(|error| error.starts_with("Failed to save settings:"))
        );
    }

    #[test]
    fn theme_save_failure_rolls_runtime_state_back() {
        let blocker = test_settings_path();
        std::fs::create_dir_all(blocker.parent().expect("test path should have a parent"))
            .expect("test settings parent should be created");
        std::fs::write(&blocker, b"not a directory").expect("blocker file should be created");
        let persistence = AppPersistence::from_settings_path(blocker.join("settings.json"));
        let mut root = ScissorsRoot::new_with_persistence(persistence);

        assert!(!root.set_color_theme(crate::appearance::ColorTheme::Light));

        assert_eq!(root.appearance, AppearanceSettings::default());
        assert!(root.settings_ui.appearance_error.is_some());
    }

    #[test]
    fn successful_appearance_save_clears_a_previous_error() {
        let blocker = test_settings_path();
        std::fs::create_dir_all(blocker.parent().expect("test path should have a parent"))
            .expect("test settings parent should be created");
        std::fs::write(&blocker, b"not a directory").expect("blocker file should be created");
        let mut root = ScissorsRoot::new_with_persistence(AppPersistence::from_settings_path(
            blocker.join("settings.json"),
        ));
        assert!(!root.set_color_theme(crate::appearance::ColorTheme::Light));

        root.persistence = Some(AppPersistence::from_settings_path(test_settings_path()));
        assert!(root.set_color_theme(crate::appearance::ColorTheme::Light));

        assert!(root.settings_ui.appearance_error.is_none());
    }

    #[test]
    fn selecting_manual_theme_disables_system_following() {
        let mut root = ScissorsRoot::new();
        root.appearance.follow_system_theme = true;
        assert!(root.set_color_theme(root.appearance.color_theme));
        assert!(!root.appearance.follow_system_theme);
    }

    #[test]
    fn ui_scale_step_uses_the_next_supported_preset() {
        let mut root = ScissorsRoot::new();
        assert!(root.set_ui_scale(ScalePreset::Percent175));

        assert!(root.set_ui_scale(root.appearance.ui_scale.next()));

        assert_eq!(root.appearance.ui_scale, ScalePreset::Percent200);
    }

    #[test]
    fn ui_scale_steps_at_bounds_are_no_ops() {
        let mut root = ScissorsRoot::new();
        root.appearance.ui_scale = ScalePreset::Percent200;
        assert!(!root.set_ui_scale(root.appearance.ui_scale.next()));

        root.appearance.ui_scale = ScalePreset::Percent80;
        assert!(!root.set_ui_scale(root.appearance.ui_scale.previous()));
    }

    #[test]
    fn export_name_input_appends_text_at_selection() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        let len = root
            .file_queue
            .selected_file()
            .map_or(0, |file| file.output_name.len());
        root.text_input_runtime_mut(ScissorsTextInputKind::ExportName)
            .selected_range = len..len;

        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::ExportName,
            None,
            "x",
            None,
            false,
        ));

        assert_eq!(
            root.file_queue
                .selected_file()
                .map(|file| file.output_name.as_str()),
            Some("one_convertedx")
        );
    }

    #[test]
    fn export_name_input_delete_can_leave_field_empty() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue.update_selected_output_name("a");
        root.text_input_runtime_mut(ScissorsTextInputKind::ExportName)
            .selected_range = 0..1;

        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::ExportName,
            None,
            "",
            None,
            false,
        ));

        assert_eq!(
            root.file_queue
                .selected_file()
                .map(|file| file.output_name.as_str()),
            Some("")
        );
    }

    fn root_with_timecode_preview(duration: f64) -> ScissorsRoot {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.preview_ui.playback_file_id = Some("first".to_string());
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, duration, None, None);
        root
    }

    #[test]
    fn preview_timecode_input_types_digits_across_fixed_separators() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 0..12;

        for digit in "001500000".chars() {
            let mut encoded = [0; 4];
            assert!(root.replace_text_input_range(
                ScissorsTextInputKind::PreviewStartTime,
                None,
                digit.encode_utf8(&mut encoded),
                None,
                false,
            ));
        }

        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewStartTime),
            "00:15:00.000"
        );
        assert_eq!(
            root.text_input_runtime(ScissorsTextInputKind::PreviewStartTime)
                .selected_range,
            12..12
        );
    }

    #[test]
    fn preview_timecode_input_does_not_commit_while_typing() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 0..12;

        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewStartTime,
            None,
            "001500000",
            None,
            false,
        ));

        assert_eq!(
            root.file_queue
                .selected_file()
                .and_then(|file| file.config.start_time.as_deref()),
            None
        );
        assert_eq!(root.preview_ui.playback.start_value(), 0.0);
    }

    #[test]
    fn preview_timecode_input_ignores_manually_typed_separator() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 3..3;

        assert!(!root.replace_text_input_range(
            ScissorsTextInputKind::PreviewStartTime,
            None,
            ":",
            None,
            false,
        ));

        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewStartTime),
            "00:00:00.000"
        );
        assert_eq!(
            root.text_input_runtime(ScissorsTextInputKind::PreviewStartTime)
                .selected_range,
            3..3
        );
    }

    #[test]
    fn preview_timecode_input_accepts_formatted_paste_without_editing_separators() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewEndTime)
            .selected_range = 0..12;

        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewEndTime,
            None,
            "01:15:00.000",
            None,
            false,
        ));

        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewEndTime),
            "01:15:00.000"
        );
    }

    #[test]
    fn preview_timecode_input_commits_draft_on_request() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 0..12;
        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewStartTime,
            None,
            "001500000",
            None,
            false,
        ));

        assert!(root.commit_preview_timecode_input(ScissorsTextInputKind::PreviewStartTime, None));

        assert_eq!(
            root.file_queue
                .selected_file()
                .and_then(|file| file.config.start_time.as_deref()),
            Some("00:15:00.000")
        );
        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewStartTime),
            "00:15:00.000"
        );
    }

    #[test]
    fn preview_timecode_input_clamps_end_to_duration_only_when_committed() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewEndTime)
            .selected_range = 0..12;
        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewEndTime,
            None,
            "013500000",
            None,
            false,
        ));
        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewEndTime),
            "01:35:00.000"
        );

        assert!(root.commit_preview_timecode_input(ScissorsTextInputKind::PreviewEndTime, None));

        assert_eq!(
            root.file_queue
                .selected_file()
                .and_then(|file| file.config.end_time.as_deref()),
            None
        );
        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewEndTime),
            "01:30:00.000"
        );
    }

    #[test]
    fn preview_timecode_input_clamps_start_below_end_when_committed() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 0..12;
        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewStartTime,
            None,
            "020000000",
            None,
            false,
        ));

        assert!(root.commit_preview_timecode_input(ScissorsTextInputKind::PreviewStartTime, None));

        assert_eq!(
            root.file_queue
                .selected_file()
                .and_then(|file| file.config.start_time.as_deref()),
            Some("01:29:59.999")
        );
    }

    #[test]
    fn preview_timecode_input_normalizes_overflowing_segments_when_committed() {
        let mut root = root_with_timecode_preview(10_000.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 0..12;
        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewStartTime,
            None,
            "007500000",
            None,
            false,
        ));

        assert!(root.commit_preview_timecode_input(ScissorsTextInputKind::PreviewStartTime, None));

        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewStartTime),
            "01:15:00.000"
        );
    }

    #[test]
    fn preview_timecode_input_commits_to_draft_file_after_selection_changes() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.file_queue
            .add_file(FileItem::from_path("second", "/tmp/two.mp4", 1));
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 0..12;
        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewStartTime,
            None,
            "001500000",
            None,
            false,
        ));
        assert!(root.file_queue.select_existing_file("second"));

        assert!(root.commit_preview_timecode_input(ScissorsTextInputKind::PreviewStartTime, None));

        assert_eq!(
            root.file_queue
                .file_by_id("first")
                .and_then(|file| file.config.start_time.as_deref()),
            Some("00:15:00.000")
        );
        assert_eq!(
            root.file_queue
                .file_by_id("second")
                .and_then(|file| file.config.start_time.as_deref()),
            None
        );
    }

    #[test]
    fn preview_timecode_input_escape_discards_uncommitted_draft() {
        let mut root = root_with_timecode_preview(5_400.0);
        root.text_input_runtime_mut(ScissorsTextInputKind::PreviewStartTime)
            .selected_range = 0..12;
        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PreviewStartTime,
            None,
            "001500000",
            None,
            false,
        ));

        assert!(root.cancel_preview_timecode_input(ScissorsTextInputKind::PreviewStartTime));

        assert_eq!(
            root.text_input_value(ScissorsTextInputKind::PreviewStartTime),
            "00:00:00.000"
        );
        assert_eq!(
            root.file_queue
                .selected_file()
                .and_then(|file| file.config.start_time.as_deref()),
            None
        );
    }

    #[test]
    fn metadata_title_input_inserts_free_text_at_selection() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .selected_file_mut()
            .unwrap()
            .config
            .metadata
            .title = Some("Render".to_string());
        root.text_input_runtime_mut(ScissorsTextInputKind::MetadataTitle)
            .selected_range = 6..6;

        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::MetadataTitle,
            None,
            " Title",
            None,
            false,
        ));

        assert_eq!(
            root.file_queue
                .selected_file()
                .and_then(|file| file.config.metadata.title.as_deref()),
            Some("Render Title")
        );
    }

    #[test]
    fn preset_name_input_inserts_free_text_at_selection() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.settings_ui.preset_name_draft = "Review".to_string();
        root.text_input_runtime_mut(ScissorsTextInputKind::PresetName)
            .selected_range = 6..6;

        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::PresetName,
            None,
            " MP4",
            None,
            false,
        ));

        assert_eq!(root.settings_ui.preset_name_draft, "Review MP4");
    }

    #[test]
    fn save_preset_from_draft_adds_custom_preset() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.settings_ui.preset_name_draft = "Review MP4".to_string();

        assert!(root.save_preset_from_draft());

        assert!(
            root.presets
                .iter()
                .any(|preset| preset.name == "Review MP4")
        );
        assert!(root.settings_ui.preset_name_draft.is_empty());
    }

    #[test]
    fn save_preset_from_draft_persists_custom_preset_with_unique_id() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        persistence
            .save(&AppSettings {
                max_concurrency: DEFAULT_MAX_CONCURRENCY,
                custom_presets: vec![PresetDefinition::custom(
                    "custom-preset-3".to_string(),
                    "Existing".to_string(),
                    ConversionConfig::default(),
                )],
                ..AppSettings::default()
            })
            .expect("settings should be saved");
        let mut root = ScissorsRoot::new_with_persistence(persistence.clone());
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.settings_ui.preset_name_draft = "Review MP4".to_string();

        assert!(root.save_preset_from_draft());

        let settings = persistence.load().expect("settings should be readable");
        assert!(
            settings
                .custom_presets
                .iter()
                .any(|preset| preset.id == "custom-preset-4" && preset.name == "Review MP4")
        );
    }

    #[test]
    fn delete_preset_persists_removed_custom_preset() {
        let persistence = AppPersistence::from_settings_path(test_settings_path());
        persistence
            .save(&AppSettings {
                max_concurrency: DEFAULT_MAX_CONCURRENCY,
                custom_presets: vec![PresetDefinition::custom(
                    "custom-preset-1".to_string(),
                    "Review MP4".to_string(),
                    ConversionConfig::default(),
                )],
                ..AppSettings::default()
            })
            .expect("settings should be saved");
        let mut root = ScissorsRoot::new_with_persistence(persistence.clone());

        assert!(root.delete_preset("custom-preset-1"));

        assert!(
            persistence
                .load()
                .expect("settings should be readable")
                .custom_presets
                .is_empty()
        );
    }

    #[test]
    fn audio_bitrate_input_inserts_digits_at_selection() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .selected_file_mut()
            .unwrap()
            .config
            .audio_bitrate = "12".to_string();
        root.text_input_runtime_mut(ScissorsTextInputKind::AudioBitrate)
            .selected_range = 1..1;

        assert!(root.replace_text_input_range(
            ScissorsTextInputKind::AudioBitrate,
            None,
            "9",
            None,
            false,
        ));

        assert_eq!(
            root.file_queue
                .selected_file()
                .map(|file| file.config.audio_bitrate.as_str()),
            Some("192")
        );
        assert_eq!(
            root.text_input_runtime(ScissorsTextInputKind::AudioBitrate)
                .selected_range,
            2..2
        );
    }

    #[test]
    fn audio_bitrate_input_rejects_non_digits() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .selected_file_mut()
            .unwrap()
            .config
            .audio_bitrate = "128".to_string();
        root.text_input_runtime_mut(ScissorsTextInputKind::AudioBitrate)
            .selected_range = 3..3;

        assert!(!root.replace_text_input_range(
            ScissorsTextInputKind::AudioBitrate,
            None,
            "k",
            None,
            false,
        ));

        assert_eq!(
            root.file_queue
                .selected_file()
                .map(|file| file.config.audio_bitrate.as_str()),
            Some("128")
        );
    }

    #[test]
    fn text_input_handler_is_scoped_to_the_active_focused_field() {
        assert!(!should_handle_text_input(false, false, false));
        assert!(!should_handle_text_input(false, true, false));
        assert!(!should_handle_text_input(false, false, true));
        assert!(should_handle_text_input(false, true, true));
        assert!(!should_handle_text_input(true, true, true));
    }

    #[test]
    fn text_input_outside_mouse_up_captures_only_while_selecting() {
        assert!(!should_capture_text_input_drag(false));
        assert!(should_capture_text_input_drag(true));
    }

    #[test]
    fn text_input_scroll_reveals_cursor_past_right_edge() {
        let scroll_x =
            text_input_scroll_x_for_cursor(px(0.0), px(180.0), px(240.0), px(120.0), px(1.0));

        assert!(scroll_x > px(0.0));
    }

    #[test]
    fn text_input_scroll_reveals_cursor_past_left_edge() {
        let scroll_x =
            text_input_scroll_x_for_cursor(px(80.0), px(40.0), px(240.0), px(120.0), px(1.0));

        assert_eq!(scroll_x, px(40.0));
    }

    #[test]
    fn text_input_scroll_stays_zero_when_content_fits() {
        let scroll_x =
            text_input_scroll_x_for_cursor(px(0.0), px(60.0), px(90.0), px(120.0), px(1.0));

        assert_eq!(scroll_x, px(0.0));
    }
}

mod scissors_root_config {
    use super::*;

    fn preview_test_bounds(width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)))
    }

    fn root_with_preview_canvas_media(width: u32, height: u32) -> ScissorsRoot {
        let mut root = ScissorsRoot::new();
        let row_bytes = width.checked_mul(4).expect("row bytes");
        let data_len = usize::try_from(row_bytes.checked_mul(height).expect("data length"))
            .expect("data length usize");
        let frame = PreviewFrame::bgra(width, height, row_bytes, 0, vec![0; data_len])
            .expect("preview frame");
        root.preview_ui.render_image = Some(render_image_from_frame(&frame).expect("render image"));
        root.preview_ui.render_presentation = PreviewRenderPresentation::default();
        root.preview_ui.canvas_bounds = Some(preview_test_bounds(1000.0, 500.0));
        root.preview_ui.canvas.auto_fit_pending = false;
        root
    }

    fn seed_ready_video(root: &mut ScissorsRoot, id: &str, path: &str) {
        root.project.enabled = false;
        root.file_queue.add_file(FileItem::from_path(id, path, 1));
        root.source_metadata.mark_ready(
            id.to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                duration: Some("90.0".to_string()),
                ..SourceMetadata::default()
            },
        );
    }

    fn test_video_preview_session(file_id: &str) -> std::sync::Arc<PreviewSession> {
        std::sync::Arc::new(PreviewSession::new_for_test(PreviewSessionConfig {
            file_id: file_id.to_string(),
            path: PathBuf::from("/tmp/one.mp4"),
            source_kind: EnginePreviewSourceKind::Video,
            source_width: Some(1920),
            source_height: Some(1080),
            selected_audio_track: None,
            duration_seconds: 90.0,
            max_width: DEFAULT_PREVIEW_MAX_WIDTH,
            max_height: DEFAULT_PREVIEW_MAX_HEIGHT,
            fps: DEFAULT_PREVIEW_FPS,
            conversion_config: crate::conversion_runner::core_config_from_gpui(
                &ConversionConfig::default(),
            ),
        }))
    }

    #[test]
    fn trim_preview_seek_state_overwrites_pending_without_starting_worker() {
        let mut state = TrimPreviewSeekState::default();

        state.queue(10.0);
        state.queue(20.0);

        assert_eq!(state.pending_seconds, Some(20.0));
        assert!(!state.worker_active);
    }

    #[test]
    fn trim_preview_seek_state_skips_duplicate_pending_within_epsilon() {
        let mut state = TrimPreviewSeekState {
            last_sent_seconds: Some(10.0),
            ..Default::default()
        };
        state.queue(10.0 + (TRIM_PREVIEW_SEEK_EPSILON_SECONDS / 2.0));

        assert_eq!(state.take_next(), None);
        assert_eq!(state.pending_seconds, None);
    }

    #[test]
    fn trim_preview_seek_state_finishes_with_restore_and_clears_after_completion() {
        let mut state = TrimPreviewSeekState::default();
        state.begin_drag(30.0);
        state.queue(60.0);

        assert!(state.finish());
        assert_eq!(
            state.take_next(),
            Some(TrimPreviewSeekRequest {
                seconds: 30.0,
                pause_first: false,
            })
        );
        assert!(state.complete_restore(30.0));

        assert_eq!(
            state,
            TrimPreviewSeekState {
                generation: 1,
                ..Default::default()
            }
        );
    }

    #[test]
    fn trim_preview_seek_state_marks_only_next_request_for_pause() {
        let mut state = TrimPreviewSeekState::default();
        state.pause_before_next_seek();
        state.queue(10.0);

        assert_eq!(
            state.take_next(),
            Some(TrimPreviewSeekRequest {
                seconds: 10.0,
                pause_first: true,
            })
        );

        state.queue(20.0);
        assert_eq!(
            state.take_next(),
            Some(TrimPreviewSeekRequest {
                seconds: 20.0,
                pause_first: false,
            })
        );
    }

    #[test]
    fn preview_media_snapshot_sync_is_blocked_while_trim_preview_is_active() {
        let mut root = ScissorsRoot::new();
        root.preview_ui.trim_preview_seek.begin_drag(30.0);

        assert!(root.preview_media_snapshot_sync_blocked());
    }

    #[test]
    fn update_selected_config_mutates_only_selected_file() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .add_file(FileItem::from_path("second", "/tmp/two.mp4", 1));
        root.file_queue.select_existing_file("second");

        root.update_selected_config(|config| {
            config.container = "webm".to_string();
            true
        });

        assert_eq!(
            root.file_queue
                .file_by_id("first")
                .map(|file| file.config.container.as_str()),
            Some("mp4")
        );
        assert_eq!(
            root.file_queue
                .file_by_id("second")
                .map(|file| file.config.container.as_str()),
            Some("webm")
        );
    }

    #[test]
    fn normalize_selected_config_clears_trim_for_selected_image_only() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.file_queue
            .add_file(FileItem::from_path("image", "/tmp/two.png", 1));

        for id in ["video", "image"] {
            root.file_queue.select_existing_file(id);
            root.update_selected_config(|config| {
                config.start_time = Some("00:00:05.000".to_string());
                config.end_time = Some("00:00:30.000".to_string());
                true
            });
        }
        root.file_queue.select_existing_file("image");

        root.normalize_selected_config(Some(&SourceMetadata {
            media_kind: Some(SourceKind::Image),
            ..SourceMetadata::default()
        }));

        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.start_time.as_deref()),
            Some("00:00:05.000")
        );
        assert_eq!(
            root.file_queue
                .file_by_id("image")
                .and_then(|file| file.config.start_time.as_deref()),
            None
        );
    }

    #[test]
    fn apply_preview_timeline_drag_updates_playback_start_without_committing_config() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                duration: Some("90.0".to_string()),
                ..SourceMetadata::default()
            },
        );
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.session = Some(test_video_preview_session("video"));

        let changed = root.apply_preview_timeline_drag(TimelineDragTarget::Start, 0.25);

        assert!(changed);
        assert!((root.preview_ui.playback.start_value() - 22.5).abs() < 0.000_001);
        assert!((root.preview_ui.playback.current_time() - 0.0).abs() < 0.000_001);
        assert_eq!(root.preview_ui.trim_preview_seek.restore_seconds, Some(0.0));
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(22.5)
        );
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.start_time.as_deref()),
            None
        );
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.end_time.as_deref()),
            None
        );

        assert!(root.end_preview_timeline_drag());
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.start_time.as_deref()),
            Some("00:00:22.500")
        );
    }

    #[test]
    fn apply_preview_timeline_handle_drag_pauses_active_playback() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                duration: Some("90.0".to_string()),
                ..SourceMetadata::default()
            },
        );
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.session = Some(test_video_preview_session("video"));
        root.preview_ui.playback.sync_media(MediaSnapshot {
            current_time: 30.0,
            duration: 90.0,
            paused: false,
        });

        assert!(root.apply_preview_timeline_drag(TimelineDragTarget::End, 0.75));

        assert!(!root.preview_ui.playback.is_playing());
        assert!((root.preview_ui.playback.end_value() - 67.5).abs() < 0.000_001);
        assert!((root.preview_ui.playback.current_time() - 30.0).abs() < 0.000_001);
        assert_eq!(
            root.preview_ui.trim_preview_seek.restore_seconds,
            Some(30.0)
        );
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(67.5)
        );
        assert!(root.preview_ui.trim_preview_seek.pause_before_next_seek);
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.end_time.as_deref()),
            None
        );

        assert!(root.end_preview_timeline_drag());
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(30.0)
        );
        assert!(root.preview_ui.trim_preview_seek.finish_after_restore);
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.end_time.as_deref()),
            Some("00:01:07.500")
        );
    }

    #[test]
    fn apply_preview_timeline_scrub_queues_preview_seek_for_paused_video() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.session = Some(test_video_preview_session("video"));

        assert!(root.apply_preview_timeline_drag(TimelineDragTarget::Scrub, 0.5));

        assert!(!root.preview_ui.playback.is_playing());
        assert_eq!(
            root.preview_ui.playback.dragging(),
            Some(TimelineDragTarget::Scrub)
        );
        assert!((root.preview_ui.playback.current_time() - 45.0).abs() < 0.000_001);
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(45.0)
        );
        assert!(!root.preview_ui.trim_preview_seek.pause_before_next_seek);
        assert_eq!(root.preview_ui.trim_preview_seek.restore_seconds, None);
    }

    #[test]
    fn apply_preview_timeline_scrub_pauses_and_marks_first_preview_seek_when_playing() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.session = Some(test_video_preview_session("video"));
        root.preview_ui.playback.sync_media(MediaSnapshot {
            current_time: 30.0,
            duration: 90.0,
            paused: false,
        });

        assert!(root.apply_preview_timeline_drag(TimelineDragTarget::Scrub, 0.5));

        assert!(!root.preview_ui.playback.is_playing());
        assert!((root.preview_ui.playback.current_time() - 45.0).abs() < 0.000_001);
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(45.0)
        );
        assert!(root.preview_ui.trim_preview_seek.pause_before_next_seek);
        assert_eq!(root.preview_ui.trim_preview_seek.restore_seconds, None);

        assert!(root.end_preview_timeline_drag());
        assert!(root.preview_ui.playback.is_playing());
        assert!((root.preview_ui.playback.current_time() - 45.0).abs() < 0.000_001);
        assert_eq!(root.preview_ui.trim_preview_seek.restore_seconds, None);
        assert!(!root.preview_ui.trim_preview_seek.finish_after_restore);
    }

    #[test]
    fn commit_preview_timeline_seek_cancels_pending_trim_restore() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.session = Some(test_video_preview_session("video"));
        root.preview_ui.playback.sync_media(MediaSnapshot {
            current_time: 30.0,
            duration: 90.0,
            paused: false,
        });
        root.set_preview_timeline_track_bounds(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(100.0), px(30.0)),
        });

        assert!(root.apply_preview_timeline_drag(TimelineDragTarget::End, 0.75));
        assert!(root.end_preview_timeline_drag());
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(30.0)
        );
        assert!(root.preview_ui.trim_preview_seek.finish_after_restore);

        assert!(root.commit_preview_timeline_seek_at_position(point(px(50.0), px(0.0))));

        assert!((root.preview_ui.playback.current_time() - 45.0).abs() < 0.000_001);
        assert_eq!(root.preview_ui.trim_preview_seek.pending_seconds, None);
        assert_eq!(root.preview_ui.trim_preview_seek.restore_seconds, None);
        assert!(!root.preview_ui.trim_preview_seek.finish_after_restore);
    }

    #[test]
    fn apply_preview_timeline_scrub_cancels_pending_trim_restore() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.session = Some(test_video_preview_session("video"));
        root.preview_ui.playback.sync_media(MediaSnapshot {
            current_time: 30.0,
            duration: 90.0,
            paused: false,
        });

        assert!(root.apply_preview_timeline_drag(TimelineDragTarget::End, 0.75));
        assert!(root.end_preview_timeline_drag());
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(30.0)
        );
        assert!(root.preview_ui.trim_preview_seek.finish_after_restore);

        assert!(root.apply_preview_timeline_drag(TimelineDragTarget::Scrub, 0.5));

        assert!((root.preview_ui.playback.current_time() - 45.0).abs() < 0.000_001);
        assert_eq!(
            root.preview_ui.trim_preview_seek.pending_seconds,
            Some(45.0)
        );
        assert_eq!(root.preview_ui.trim_preview_seek.restore_seconds, None);
        assert!(!root.preview_ui.trim_preview_seek.finish_after_restore);
    }

    #[test]
    fn apply_preview_timeline_drag_preserves_gap_when_end_moves_before_start() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                duration: Some("90.0".to_string()),
                ..SourceMetadata::default()
            },
        );
        root.update_selected_config(|config| {
            config.start_time = Some("00:00:20.000".to_string());
            true
        });
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, Some("00:00:20.000"), None);

        let changed = root.apply_preview_timeline_drag(TimelineDragTarget::End, 0.10);

        assert!(changed);
        assert!((root.preview_ui.playback.end_value() - 21.0).abs() < 0.000_001);
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.end_time.as_deref()),
            None
        );

        assert!(root.end_preview_timeline_drag());
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.end_time.as_deref()),
            Some("00:00:21.000")
        );
    }

    #[test]
    fn apply_preview_timeline_drag_ignores_image_sources() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("image", "/tmp/one.png", 1));
        root.source_metadata.mark_ready(
            "image".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Image),
                duration: Some("90.0".to_string()),
                ..SourceMetadata::default()
            },
        );

        let changed = root.apply_preview_timeline_drag(TimelineDragTarget::Start, 0.25);

        assert!(!changed);
        assert_eq!(
            root.file_queue
                .file_by_id("image")
                .and_then(|file| file.config.start_time.as_deref()),
            None
        );
    }

    #[test]
    fn commit_preview_timeline_seek_at_position_updates_local_playhead() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                duration: Some("90.0".to_string()),
                ..SourceMetadata::default()
            },
        );
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.set_preview_timeline_track_bounds(Bounds {
            origin: point(px(10.0), px(0.0)),
            size: size(px(100.0), px(30.0)),
        });

        let changed = root.commit_preview_timeline_seek_at_position(point(px(60.0), px(0.0)));

        assert!(changed);
        assert_eq!(root.preview_ui.playback.current_time(), 45.0);
    }

    #[test]
    fn toggle_selected_crop_mode_initializes_default_video_draft() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );

        let changed = root.toggle_selected_crop_mode();

        assert!(changed);
        assert!(root.preview_ui.crop_mode);
        assert_eq!(root.preview_ui.draft_crop, Some(default_crop_rect()));
        assert_eq!(root.preview_ui.crop_aspect, "free");
    }

    #[test]
    fn select_preview_crop_aspect_keeps_side_rotation_preview_square() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );

        assert!(root.rotate_selected_preview());
        assert!(root.toggle_selected_crop_mode());
        assert!(root.select_preview_crop_aspect("1:1"));

        let config = &root.file_queue.file_by_id("video").unwrap().config;
        let metadata_entry = root.source_metadata.entry_for("video");
        let crop_state = root.preview_crop_render_state(metadata_entry.metadata.as_ref(), config);
        let visual_rect = preview_crop_visual_rect(&crop_state);
        let visual_width = visual_rect.width * 1080.0;
        let visual_height = visual_rect.height * 1920.0;

        assert!((visual_width - visual_height).abs() < 0.000_001);
        assert!(root.apply_selected_crop());
        let crop = root
            .file_queue
            .file_by_id("video")
            .and_then(|file| file.config.crop.as_ref())
            .expect("crop settings");
        assert_eq!(crop.width, crop.height);
    }

    #[test]
    fn four_by_three_crop_of_full_hd_reports_output_pixel_size() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );

        assert!(root.toggle_selected_crop_mode());
        assert!(root.reset_preview_crop_selection());
        assert!(root.select_preview_crop_aspect("4:3"));

        let config = &root.file_queue.file_by_id("video").unwrap().config;
        let metadata_entry = root.source_metadata.entry_for("video");
        let crop_state = root.preview_crop_render_state(metadata_entry.metadata.as_ref(), config);
        assert_eq!(
            crop_state.draft_size,
            Some(CropSourceDimensions {
                width: 1440,
                height: 1080,
            })
        );
        assert_eq!(
            crop_state.draft_size.map(crop_size_label),
            Some("1440 × 1080".to_string())
        );
    }

    #[test]
    fn crop_render_state_has_no_draft_size_outside_crop_mode() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );

        let config = &root.file_queue.file_by_id("video").unwrap().config;
        let metadata_entry = root.source_metadata.entry_for("video");
        let crop_state = root.preview_crop_render_state(metadata_entry.metadata.as_ref(), config);
        assert_eq!(crop_state.draft_size, None);
    }

    #[test]
    fn apply_selected_crop_writes_selected_file_crop_pixels() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("first", "/tmp/one.mp4", 1));
        root.file_queue
            .add_file(FileItem::from_path("second", "/tmp/two.mp4", 1));
        root.file_queue.select_existing_file("second");
        root.source_metadata.mark_ready(
            "second".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );
        root.preview_ui.crop_mode = true;
        root.preview_ui.draft_crop = Some(CropRect {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        });
        root.preview_ui.crop_aspect = "16:9".to_string();

        let changed = root.apply_selected_crop();

        assert!(changed);
        assert_eq!(
            root.file_queue
                .file_by_id("first")
                .and_then(|file| file.config.crop.as_ref()),
            None
        );
        assert_eq!(
            root.file_queue
                .file_by_id("second")
                .and_then(|file| file.config.crop.as_ref()),
            Some(&CropSettings {
                enabled: true,
                x: 480,
                y: 270,
                width: 960,
                height: 540,
                source_width: Some(1920),
                source_height: Some(1080),
                aspect_ratio: Some("16:9".to_string()),
            })
        );
    }

    #[test]
    fn apply_selected_full_crop_clears_existing_crop() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );
        root.update_selected_config(|config| {
            config.crop = Some(CropSettings {
                enabled: true,
                x: 100,
                y: 100,
                width: 1000,
                height: 600,
                source_width: Some(1920),
                source_height: Some(1080),
                aspect_ratio: None,
            });
            true
        });
        root.preview_ui.crop_mode = true;
        root.preview_ui.draft_crop = Some(full_crop_rect());

        let changed = root.apply_selected_crop();

        assert!(changed);
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.crop.as_ref()),
            None
        );
    }

    #[test]
    fn rotate_and_flip_preview_update_selected_config() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );

        assert!(root.rotate_selected_preview());
        assert!(root.toggle_selected_flip(FlipAxis::Horizontal));

        let config = &root.file_queue.file_by_id("video").unwrap().config;
        assert_eq!(config.rotation, "90");
        assert!(config.flip_horizontal);
        assert!(!config.flip_vertical);
    }

    #[test]
    fn preview_runtime_request_is_silent_without_explicit_audio_selection() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                audio_tracks: vec![crate::settings::AudioTrack {
                    index: 1,
                    codec: "aac".to_string(),
                    ..crate::settings::AudioTrack::default()
                }],
                ..SourceMetadata::default()
            },
        );
        let metadata_entry = root.source_metadata.entry_for("video");

        let request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("preview request");

        assert_eq!(request.config.selected_audio_track, None);
    }

    #[test]
    fn preview_runtime_request_uses_first_valid_explicit_audio_selection() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                audio_tracks: vec![crate::settings::AudioTrack {
                    index: 1,
                    codec: "aac".to_string(),
                    ..crate::settings::AudioTrack::default()
                }],
                ..SourceMetadata::default()
            },
        );
        root.update_selected_config(|config| {
            config.selected_audio_tracks = vec![99, 1];
            true
        });
        let metadata_entry = root.source_metadata.entry_for("video");

        let request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("preview request");

        assert_eq!(request.config.selected_audio_track, Some(1));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "This scenario verifies preview runtime hashing across visual, audio, trim, and transform config."
    )]
    fn preview_runtime_key_tracks_visual_config_but_ignores_encoder_quality() {
        let mut root = ScissorsRoot::new();
        root.project.enabled = false;
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                duration: Some("12.5".to_string()),
                audio_codec: Some("aac".to_string()),
                audio_tracks: vec![crate::settings::AudioTrack {
                    index: 1,
                    codec: "aac".to_string(),
                    channels: Some("2".to_string()),
                    language: None,
                    label: None,
                    bitrate_kbps: None,
                    sample_rate: Some("48000".to_string()),
                }],
                ..SourceMetadata::default()
            },
        );
        let metadata_entry = root.source_metadata.entry_for("video");
        let initial_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("initial request");

        root.update_selected_config(|config| {
            config.crf = 18;
            true
        });
        let quality_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("quality request");

        assert_eq!(quality_request.key, initial_request.key);

        root.update_selected_config(|config| {
            config.start_time = Some("00:00:02.000".to_string());
            config.end_time = Some("00:00:08.000".to_string());
            true
        });
        let trim_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("trim request");

        assert_eq!(trim_request.key, initial_request.key);
        assert_eq!(
            trim_request.config.conversion_config.start_time.as_deref(),
            Some("00:00:02.000")
        );
        assert_eq!(
            trim_request.config.conversion_config.end_time.as_deref(),
            Some("00:00:08.000")
        );

        root.update_selected_config(|config| {
            config.video_filters.temperature.value = 3200;
            true
        });
        let disabled_video_filter_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("disabled video filter request");

        assert_eq!(disabled_video_filter_request.key, initial_request.key);

        root.update_selected_config(|config| {
            config.video_filters.color.brightness.enabled = true;
            config.video_filters.color.brightness.value = 20;
            true
        });
        let video_filter_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("video filter request");

        assert_ne!(
            video_filter_request.key.visual_hash,
            initial_request.key.visual_hash
        );
        assert_eq!(
            video_filter_request.key.audio_hash,
            initial_request.key.audio_hash
        );

        root.update_selected_config(|config| {
            config.video_filters.color.brightness.enabled = false;
            config.video_filters.color.brightness.value = 80;
            true
        });
        let disabled_video_value_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("disabled video filter value request");

        assert_eq!(
            disabled_video_value_request.key.visual_hash,
            disabled_video_filter_request.key.visual_hash
        );

        root.update_selected_config(|config| {
            config.audio_filters.bass.value = 12;
            true
        });
        let disabled_audio_filter_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("disabled audio filter request");

        assert_eq!(
            disabled_audio_filter_request.key.audio_hash,
            initial_request.key.audio_hash
        );

        root.update_selected_config(|config| {
            config.audio_filters.limiter.enabled = true;
            config.audio_filters.limiter.value = -3;
            true
        });
        let audio_filter_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("audio filter request");

        assert_eq!(
            audio_filter_request.key.visual_hash,
            disabled_audio_filter_request.key.visual_hash
        );
        assert_ne!(
            audio_filter_request.key.audio_hash,
            initial_request.key.audio_hash
        );

        root.update_selected_config(|config| {
            config.video_filters = crate::settings::VideoFiltersConfig::default();
            config.audio_filters = crate::settings::AudioFiltersConfig::default();
            true
        });

        root.update_selected_config(|config| {
            config.audio_volume = 80;
            true
        });
        let audio_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("audio request");

        assert_ne!(audio_request.key, initial_request.key);

        assert!(root.rotate_selected_preview());
        assert!(root.toggle_selected_flip(FlipAxis::Horizontal));
        root.preview_ui.crop_mode = true;
        root.preview_ui.draft_crop = Some(CropRect {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        });
        assert!(root.apply_selected_crop());
        let transformed_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("transformed request");

        assert_ne!(transformed_request.key, initial_request.key);
        assert_eq!(
            transformed_request.presentation,
            PreviewRenderPresentation::default()
        );
        assert_eq!(transformed_request.config.conversion_config.rotation, "90");
        assert!(transformed_request.config.conversion_config.flip_horizontal);
        assert!(
            transformed_request
                .config
                .conversion_config
                .crop
                .as_ref()
                .is_some_and(|crop| crop.enabled)
        );
    }

    #[test]
    fn preview_runtime_key_reconfigures_only_same_source_identity() {
        let current = PreviewRuntimeKey {
            file_id: "video".to_string(),
            path: "/tmp/one.mp4".to_string(),
            source_kind: EnginePreviewSourceKind::Video,
            source_width: Some(1920),
            source_height: Some(1080),
            duration_millis: 12_500,
            preview_dimensions: PreviewRuntimeDimensions {
                max_width: 1280,
                max_height: 720,
            },
            visual_hash: 1,
            audio_hash: 1,
        };
        let mut next = current.clone();
        next.visual_hash = 2;
        next.audio_hash = 3;

        assert!(current.can_reconfigure_to(&next));

        next.path = "/tmp/two.mp4".to_string();
        assert!(!current.can_reconfigure_to(&next));
    }

    #[test]
    fn preview_runtime_key_changes_when_adaptive_dimensions_change() {
        let current = PreviewRuntimeKey {
            file_id: "video".to_string(),
            path: "/tmp/one.mp4".to_string(),
            source_kind: EnginePreviewSourceKind::Video,
            source_width: Some(1920),
            source_height: Some(1080),
            duration_millis: 12_500,
            preview_dimensions: PreviewRuntimeDimensions {
                max_width: 960,
                max_height: 540,
            },
            visual_hash: 1,
            audio_hash: 1,
        };
        let mut next = current.clone();
        next.preview_dimensions = PreviewRuntimeDimensions {
            max_width: 1280,
            max_height: 720,
        };

        assert_ne!(current, next);
        assert!(current.can_reconfigure_to(&next));
    }

    #[test]
    fn selected_preview_runtime_request_keeps_dimensions_while_playing() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.canvas_bounds = Some(preview_test_bounds(900.0, 500.0));
        root.preview_ui.canvas.target_zoom = 1.0;
        root.preview_ui.playback_file_id = Some("video".to_string());
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.playback.handle_play();
        let metadata_entry = root.source_metadata.entry_for("video");
        let initial_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("initial request");

        root.preview_ui.canvas_bounds = Some(preview_test_bounds(1200.0, 700.0));
        root.preview_ui.canvas.current_zoom = 2.0;
        root.preview_ui.canvas.target_zoom = 2.0;
        let resized_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("resized request");

        assert_eq!(
            resized_request.key.preview_dimensions,
            initial_request.key.preview_dimensions
        );
        assert_eq!(
            resized_request.config.max_width,
            initial_request.config.max_width
        );
        assert_eq!(
            resized_request.config.max_height,
            initial_request.config.max_height
        );
    }

    #[test]
    fn selected_preview_runtime_request_refreshes_dimensions_while_paused() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.canvas_bounds = Some(preview_test_bounds(900.0, 500.0));
        root.preview_ui.canvas.target_zoom = 1.0;
        root.preview_ui.playback_file_id = Some("video".to_string());
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        let metadata_entry = root.source_metadata.entry_for("video");
        let initial_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("initial request");

        root.preview_ui.canvas_bounds = Some(preview_test_bounds(1200.0, 700.0));
        root.preview_ui.canvas.current_zoom = 2.0;
        root.preview_ui.canvas.target_zoom = 2.0;
        let resized_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("resized request");

        assert_ne!(
            resized_request.key.preview_dimensions,
            initial_request.key.preview_dimensions
        );
        assert_eq!(
            resized_request.key.preview_dimensions,
            PreviewRuntimeDimensions {
                max_width: 1280,
                max_height: 720
            }
        );
    }

    #[test]
    fn selected_preview_runtime_request_refreshes_dimensions_after_pause() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.canvas_bounds = Some(preview_test_bounds(900.0, 500.0));
        root.preview_ui.canvas.target_zoom = 1.0;
        root.preview_ui.playback_file_id = Some("video".to_string());
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.playback.handle_play();
        let metadata_entry = root.source_metadata.entry_for("video");
        let initial_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("initial request");

        root.preview_ui.canvas_bounds = Some(preview_test_bounds(1200.0, 700.0));
        root.preview_ui.canvas.current_zoom = 2.0;
        root.preview_ui.canvas.target_zoom = 2.0;
        let playing_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("playing request");
        root.preview_ui.playback.handle_pause();
        let paused_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("paused request");

        assert_eq!(
            playing_request.key.preview_dimensions,
            initial_request.key.preview_dimensions
        );
        assert_eq!(
            paused_request.key.preview_dimensions,
            PreviewRuntimeDimensions {
                max_width: 1280,
                max_height: 720
            }
        );
    }

    #[test]
    fn selected_preview_runtime_request_defers_paused_zoom_until_canvas_settles() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.canvas_bounds = Some(preview_test_bounds(900.0, 500.0));
        root.preview_ui.canvas.current_zoom = 1.0;
        root.preview_ui.canvas.target_zoom = 1.0;
        root.preview_ui.playback_file_id = Some("video".to_string());
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        let metadata_entry = root.source_metadata.entry_for("video");
        let initial_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("initial request");

        root.preview_ui.canvas.target_zoom = 2.0;
        let animating_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("animating request");
        root.preview_ui.canvas.current_zoom = 2.0;
        let settled_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("settled request");

        assert_eq!(
            animating_request.key.preview_dimensions,
            initial_request.key.preview_dimensions
        );
        assert_eq!(
            settled_request.key.preview_dimensions,
            PreviewRuntimeDimensions {
                max_width: 1280,
                max_height: 720
            }
        );
    }

    #[test]
    fn selected_preview_runtime_request_debounces_paused_resize_dimensions() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "video", "/tmp/one.mp4");
        root.preview_ui.canvas_bounds = Some(preview_test_bounds(900.0, 500.0));
        root.preview_ui.canvas.current_zoom = 1.0;
        root.preview_ui.canvas.target_zoom = 1.0;
        root.preview_ui.playback_file_id = Some("video".to_string());
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        let metadata_entry = root.source_metadata.entry_for("video");
        let initial_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("initial request");

        root.preview_ui.canvas_bounds = Some(preview_test_bounds(1200.0, 700.0));
        root.preview_ui.preview_dimensions_debounce_until =
            Some(std::time::Instant::now() + PREVIEW_DIMENSION_DEBOUNCE_INTERVAL);
        let debounced_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("debounced request");
        root.preview_ui.preview_dimensions_debounce_until = Some(
            std::time::Instant::now()
                .checked_sub(PREVIEW_DIMENSION_DEBOUNCE_INTERVAL)
                .expect("deadline should move into the past"),
        );
        let settled_request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("settled request");

        assert_eq!(
            debounced_request.key.preview_dimensions,
            initial_request.key.preview_dimensions
        );
        assert_eq!(
            settled_request.key.preview_dimensions,
            PreviewRuntimeDimensions {
                max_width: 1280,
                max_height: 720
            }
        );
    }

    #[test]
    fn filter_preview_debounce_active_expires_deadline() {
        let mut root = ScissorsRoot::new();

        root.preview_ui.filter_preview_debounce_until =
            Some(std::time::Instant::now() + PREVIEW_FILTER_DEBOUNCE_INTERVAL);
        assert!(root.filter_preview_debounce_active());

        root.preview_ui.filter_preview_debounce_until = Some(
            std::time::Instant::now()
                .checked_sub(PREVIEW_FILTER_DEBOUNCE_INTERVAL)
                .expect("deadline should move into the past"),
        );
        assert!(!root.filter_preview_debounce_active());
        assert!(root.preview_ui.filter_preview_debounce_until.is_none());
    }

    #[test]
    fn selected_preview_runtime_request_ignores_playing_latch_for_different_file() {
        let mut root = ScissorsRoot::new();
        seed_ready_video(&mut root, "first", "/tmp/one.mp4");
        seed_ready_video(&mut root, "second", "/tmp/two.mp4");
        root.file_queue.select_existing_file("second");
        root.preview_ui.canvas_bounds = Some(preview_test_bounds(900.0, 500.0));
        root.preview_ui.canvas.target_zoom = 1.0;
        root.preview_ui.playback_file_id = Some("first".to_string());
        root.preview_ui.playback =
            preview_playback_state(PreviewMediaKind::Video, 90.0, None, None);
        root.preview_ui.playback.handle_play();
        root.preview_ui.active_preview_dimensions = Some(PreviewRuntimeDimensions {
            max_width: 640,
            max_height: 360,
        });
        let metadata_entry = root.source_metadata.entry_for("second");

        let request = root
            .selected_preview_runtime_request(&metadata_entry)
            .expect("request");

        assert_eq!(
            request.key.preview_dimensions,
            PreviewRuntimeDimensions {
                max_width: 1088,
                max_height: 576
            }
        );
    }

    #[test]
    fn preview_runtime_dimensions_fall_back_without_canvas_bounds() {
        let dimensions = preview_runtime_dimensions(None, 1.0);

        assert_eq!(
            dimensions,
            PreviewRuntimeDimensions {
                max_width: 1280,
                max_height: 720
            }
        );
    }

    #[test]
    fn preview_runtime_dimensions_clamp_small_canvas_to_minimum() {
        let dimensions = preview_runtime_dimensions(
            Some(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(200.0), px(120.0)),
            )),
            1.0,
        );

        assert_eq!(
            dimensions,
            PreviewRuntimeDimensions {
                max_width: 640,
                max_height: 360
            }
        );
    }

    #[test]
    fn preview_runtime_dimensions_quantize_typical_canvas_below_720p() {
        let dimensions = preview_runtime_dimensions(
            Some(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(900.0), px(500.0)),
            )),
            1.0,
        );

        assert_eq!(
            dimensions,
            PreviewRuntimeDimensions {
                max_width: 1088,
                max_height: 576
            }
        );
    }

    #[test]
    fn preview_runtime_dimensions_ignore_resize_within_same_quantum() {
        let first = preview_runtime_dimensions(
            Some(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(900.0), px(500.0)),
            )),
            1.0,
        );
        let second = preview_runtime_dimensions(
            Some(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(910.0), px(500.0)),
            )),
            1.0,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn preview_runtime_dimensions_zoom_up_without_exceeding_720p_cap() {
        let dimensions = preview_runtime_dimensions(
            Some(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(900.0), px(500.0)),
            )),
            2.0,
        );

        assert_eq!(
            dimensions,
            PreviewRuntimeDimensions {
                max_width: 1280,
                max_height: 720
            }
        );
    }

    #[test]
    fn preview_presentation_maps_rotation_and_crop_without_rewriting_frame() {
        let frame = PreviewFrame::bgra(
            4,
            2,
            16,
            0,
            vec![
                0, 0, 0, 255, 1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255, 5, 0, 0, 255,
                6, 0, 0, 255, 7, 0, 0, 255,
            ],
        )
        .expect("frame");
        let render_image = render_image_from_frame(&frame).expect("render image");

        let presented = preview_presented_frame(
            &render_image,
            PreviewRenderPresentation {
                transform: PreviewTransform {
                    rotation_degrees: 90,
                    flip_horizontal: false,
                    flip_vertical: false,
                },
                crop: Some(EnginePreviewCrop {
                    x: 0,
                    y: 1,
                    width: 2,
                    height: 2,
                }),
                crop_source_width: Some(2),
                crop_source_height: Some(4),
            },
        )
        .expect("presented frame");

        assert_eq!(render_image.size(0).width.0, 4);
        assert_eq!(render_image.size(0).height.0, 2);
        assert_eq!(presented.full_width, 2);
        assert_eq!(presented.full_height, 4);
        assert_eq!(presented.visible_x, 0);
        assert_eq!(presented.visible_y, 1);
        assert_eq!(presented.visible_width, 2);
        assert_eq!(presented.visible_height, 2);
    }

    #[test]
    fn apply_preview_crop_drag_updates_draft_without_persisting_config() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );
        root.preview_ui.crop_mode = true;
        root.preview_ui.draft_crop = Some(CropRect {
            x: 0.10,
            y: 0.10,
            width: 0.50,
            height: 0.50,
        });

        assert!(
            !root.apply_preview_crop_drag(DragHandle::Move, PreviewPoint { x: 0.50, y: 0.50 },)
        );
        assert!(root.apply_preview_crop_drag(DragHandle::Move, PreviewPoint { x: 0.60, y: 0.55 },));

        let draft = root.preview_ui.draft_crop.unwrap();
        assert!((draft.x - 0.20).abs() < 0.000_001);
        assert!((draft.y - 0.15).abs() < 0.000_001);
        assert_eq!(draft.width, 0.50);
        assert_eq!(draft.height, 0.50);
        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.crop.as_ref()),
            None
        );
    }

    #[test]
    fn apply_preview_crop_drag_moves_visual_rect_after_side_rotation() {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );

        assert!(root.rotate_selected_preview());
        assert!(root.toggle_selected_crop_mode());
        assert!(root.select_preview_crop_aspect("1:1"));

        let config = &root.file_queue.file_by_id("video").unwrap().config;
        let metadata_entry = root.source_metadata.entry_for("video");
        let crop_state = root.preview_crop_render_state(metadata_entry.metadata.as_ref(), config);
        let before = preview_crop_visual_rect(&crop_state);

        assert!(!root.apply_preview_crop_drag(DragHandle::Move, PreviewPoint { x: 0.50, y: 0.50 }));
        assert!(root.apply_preview_crop_drag(DragHandle::Move, PreviewPoint { x: 0.60, y: 0.50 }));

        let config = &root.file_queue.file_by_id("video").unwrap().config;
        let metadata_entry = root.source_metadata.entry_for("video");
        let crop_state = root.preview_crop_render_state(metadata_entry.metadata.as_ref(), config);
        let after = preview_crop_visual_rect(&crop_state);

        assert!((after.x - before.x - 0.10).abs() < 0.000_001);
        assert!((after.y - before.y).abs() < 0.000_001);
    }

    #[test]
    fn sync_preview_canvas_for_selection_resets_canvas_when_file_changes() {
        let mut root = ScissorsRoot::new();
        root.sync_preview_canvas_for_selection(Some("first"));
        root.preview_ui.canvas.current_zoom = 2.0;
        root.preview_ui.canvas.target_zoom = 3.0;
        root.preview_ui.canvas.current_pan_x = 0.5;
        root.preview_ui.canvas.target_pan_y = -0.5;
        root.preview_ui.canvas.auto_fit_pending = false;

        root.sync_preview_canvas_for_selection(Some("first"));

        assert_eq!(root.preview_ui.canvas.current_zoom, 2.0);

        root.sync_preview_canvas_for_selection(Some("second"));

        assert_eq!(root.preview_ui.canvas, PreviewCanvasState::default());
    }

    #[test]
    fn lerp_preview_canvas_value_moves_toward_target_without_overshooting() {
        let next = lerp_preview_canvas_value(1.0, 2.0);

        assert!(next > 1.0 && next < 2.0);
    }

    #[test]
    fn lerp_preview_canvas_value_uses_elapsed_time() {
        let normal = lerp_preview_canvas_value_for_elapsed(0.0, 100.0, Duration::from_millis(16));
        let delayed = lerp_preview_canvas_value_for_elapsed(0.0, 100.0, Duration::from_millis(64));

        assert!((normal - 20.0).abs() < 0.000_001);
        assert!(delayed > normal);
        assert!(delayed < 100.0);
    }

    #[test]
    fn preview_canvas_transform_waits_for_all_axes_to_settle() {
        assert!(!preview_canvas_transform_settled(
            1.0, 1.000_01, 10.0, 10.005, -10.0, -10.005,
        ));
        assert!(preview_canvas_transform_settled(
            1.0,
            1.000_000_5,
            10.0,
            10.005,
            -10.0,
            -10.005,
        ));
    }

    #[test]
    fn preview_canvas_transform_visual_delta_measures_rendered_bounds() {
        let delta = preview_canvas_transform_visual_delta(
            1000.0, 500.0, 16.0, 9.0, 1.0, 1.000_4, 0.0, 0.20, 0.0, -0.20,
        )
        .expect("visual delta");

        assert!(delta < PREVIEW_CANVAS_VISUAL_SETTLE_EPSILON);
    }

    #[test]
    fn tick_preview_canvas_animation_keeps_pan_moving_until_zoom_settles() {
        let mut root = ScissorsRoot::new();
        root.preview_ui.canvas.current_zoom = 1.0;
        root.preview_ui.canvas.target_zoom = 1.000_01;
        root.preview_ui.canvas.current_pan_x = 10.0;
        root.preview_ui.canvas.target_pan_x = 10.005;
        root.preview_ui.canvas.current_pan_y = -10.0;
        root.preview_ui.canvas.target_pan_y = -10.005;

        assert!(root.tick_preview_canvas_animation());
        assert!(root.preview_ui.canvas.current_zoom > 1.0);
        assert!(root.preview_ui.canvas.current_pan_x > 10.0);
        assert!(root.preview_ui.canvas.current_pan_y < -10.0);
    }

    #[test]
    fn tick_preview_canvas_animation_settles_subpixel_visual_motion() {
        let mut root = root_with_preview_canvas_media(16, 9);
        root.preview_ui.canvas.current_zoom = 1.0;
        root.preview_ui.canvas.target_zoom = 1.000_4;
        root.preview_ui.canvas.current_pan_x = 0.0;
        root.preview_ui.canvas.target_pan_x = 0.20;
        root.preview_ui.canvas.current_pan_y = 0.0;
        root.preview_ui.canvas.target_pan_y = -0.20;

        assert!(root.tick_preview_canvas_animation());
        assert_eq!(
            root.preview_ui.canvas.current_zoom,
            root.preview_ui.canvas.target_zoom
        );
        assert_eq!(
            root.preview_ui.canvas.current_pan_x,
            root.preview_ui.canvas.target_pan_x
        );
        assert_eq!(
            root.preview_ui.canvas.current_pan_y,
            root.preview_ui.canvas.target_pan_y
        );
    }

    #[test]
    fn tick_preview_canvas_animation_keeps_visible_motion_lerping() {
        let mut root = root_with_preview_canvas_media(16, 9);
        root.preview_ui.canvas.current_zoom = 1.0;
        root.preview_ui.canvas.target_zoom = 1.01;
        root.preview_ui.canvas.current_pan_x = 0.0;
        root.preview_ui.canvas.target_pan_x = 2.0;

        assert!(root.tick_preview_canvas_animation());
        assert_ne!(
            root.preview_ui.canvas.current_zoom,
            root.preview_ui.canvas.target_zoom
        );
    }

    #[test]
    fn tick_preview_canvas_animation_settles_exactly_inside_visual_epsilon() {
        let mut root = ScissorsRoot::new();
        root.preview_ui.canvas.current_zoom = 1.0;
        root.preview_ui.canvas.target_zoom = 1.000_000_5;
        root.preview_ui.canvas.current_pan_x = 10.0;
        root.preview_ui.canvas.target_pan_x = 10.005;
        root.preview_ui.canvas.current_pan_y = -10.0;
        root.preview_ui.canvas.target_pan_y = -10.005;

        assert!(root.tick_preview_canvas_animation());
        assert_eq!(
            root.preview_ui.canvas.current_zoom,
            root.preview_ui.canvas.target_zoom
        );
        assert_eq!(
            root.preview_ui.canvas.current_pan_x,
            root.preview_ui.canvas.target_pan_x
        );
        assert_eq!(
            root.preview_ui.canvas.current_pan_y,
            root.preview_ui.canvas.target_pan_y
        );
    }

    #[test]
    fn apply_preview_overlay_drag_updates_draft_until_done_persists_position() {
        let mut root = root_with_overlay();

        assert!(root.apply_preview_overlay_drag(
            OverlayDragHandle::Move,
            OverlayDragPoint {
                x: 0.50,
                y: 0.50,
                width: Some(0.20),
                height: Some(0.20),
            },
        ));
        assert!(root.apply_preview_overlay_drag(
            OverlayDragHandle::Move,
            OverlayDragPoint {
                x: 0.60,
                y: 0.55,
                width: Some(0.20),
                height: Some(0.20),
            },
        ));

        let committed_overlay = root
            .file_queue
            .file_by_id("video")
            .and_then(|file| file.config.overlay.as_ref())
            .unwrap();
        assert!((committed_overlay.x - 0.50).abs() < 0.000_001);
        assert!((committed_overlay.y - 0.50).abs() < 0.000_001);

        let draft = root.preview_ui.overlay.overlay().unwrap();
        assert!((draft.x - 0.60).abs() < 0.000_001);
        assert!((draft.y - 0.55).abs() < 0.000_001);

        assert!(root.set_selected_overlay_mode(false));
        let overlay = root
            .file_queue
            .file_by_id("video")
            .and_then(|file| file.config.overlay.as_ref())
            .unwrap();
        assert!((overlay.x - 0.60).abs() < 0.000_001);
        assert!((overlay.y - 0.55).abs() < 0.000_001);
        assert_eq!(overlay.anchor, "custom");
    }

    #[test]
    fn commit_preview_overlay_opacity_at_position_updates_draft_until_done() {
        let mut root = root_with_overlay();
        root.set_preview_overlay_opacity_slider_bounds(Bounds {
            origin: point(px(10.0), px(0.0)),
            size: size(px(100.0), px(30.0)),
        });

        assert!(root.commit_preview_overlay_opacity_at_position(point(px(60.0), px(0.0))));

        let committed_overlay = root
            .file_queue
            .file_by_id("video")
            .and_then(|file| file.config.overlay.as_ref())
            .unwrap();
        assert!((committed_overlay.opacity - 1.0).abs() < 0.000_001);

        let draft = root.preview_ui.overlay.overlay().unwrap();
        assert!((draft.opacity - 0.50).abs() < 0.000_001);

        assert!(root.set_selected_overlay_mode(false));
        let overlay = root
            .file_queue
            .file_by_id("video")
            .and_then(|file| file.config.overlay.as_ref())
            .unwrap();
        assert!((overlay.opacity - 0.50).abs() < 0.000_001);
    }

    #[test]
    fn remove_selected_overlay_clears_selected_file_overlay_config() {
        let mut root = root_with_overlay();

        assert!(root.remove_selected_overlay());

        assert_eq!(
            root.file_queue
                .file_by_id("video")
                .and_then(|file| file.config.overlay.as_ref()),
            None
        );
        assert!(root.preview_ui.overlay.overlay().is_none());
    }

    fn root_with_overlay() -> ScissorsRoot {
        let mut root = ScissorsRoot::new();
        root.file_queue
            .add_file(FileItem::from_path("video", "/tmp/one.mp4", 1));
        root.source_metadata.mark_ready(
            "video".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                width: Some(1920),
                height: Some(1080),
                ..SourceMetadata::default()
            },
        );
        let overlay = PreviewOverlay {
            enabled: true,
            path: "/tmp/logo.png".to_string(),
            x: 0.50,
            y: 0.50,
            width: 0.20,
            opacity: 1.0,
            anchor: "custom".to_string(),
        };
        root.preview_ui.overlay.sync_initial_overlay(Some(&overlay));
        root.preview_ui.overlay.set_overlay_mode(true, false);
        root.update_selected_config(|config| {
            config.overlay = Some(OverlaySettings {
                enabled: overlay.enabled,
                path: overlay.path.clone(),
                x: overlay.x,
                y: overlay.y,
                width: overlay.width,
                opacity: overlay.opacity,
                anchor: overlay.anchor.clone(),
            });
            true
        });
        root
    }
}

mod scissors_window_options {
    use super::*;

    #[test]
    fn keeps_transparent_titlebar_with_native_controls_in_frame_slot() {
        let options = scissors_window_options(Bounds::default());
        let titlebar = options
            .titlebar
            .as_ref()
            .expect("Scissors keeps a transparent native titlebar host for macOS controls");

        assert!(titlebar.appears_transparent);
        assert_eq!(
            titlebar.traffic_light_position,
            Some(point(
                px(TITLEBAR_MACOS_NATIVE_TRAFFIC_LIGHT_X),
                px(TITLEBAR_MACOS_NATIVE_TRAFFIC_LIGHT_Y),
            ))
        );
    }

    #[test]
    fn preserves_original_minimum_window_size() {
        let options = scissors_window_options(Bounds::default());

        assert_eq!(
            options.window_min_size,
            Some(size(px(WINDOW_MIN_WIDTH), px(WINDOW_MIN_HEIGHT)))
        );
    }

    #[test]
    fn sets_the_scissors_application_id() {
        let options = scissors_window_options(Bounds::default());

        assert_eq!(options.app_id.as_deref(), Some(SCISSORS_APP_ID));
    }

    #[test]
    fn sets_the_native_window_title() {
        let options = scissors_window_options(Bounds::default());
        let titlebar = options
            .titlebar
            .as_ref()
            .expect("Scissors should configure native titlebar metadata");

        assert_eq!(titlebar.title.as_deref(), Some("Scissors"));
    }
}

mod visual_fixtures {
    use super::*;

    #[test]
    fn app_settings_fixture_opens_runtime_settings_sheet() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::AppSettings));

        assert!(root.settings_ui.is_open);
        assert_eq!(
            root.conversion_processes.current_max_concurrency().unwrap(),
            1
        );
    }

    #[test]
    fn ui_scale_fixture_opens_settings_at_maximum_scale() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::AppSettingsUiOpen));

        assert!(root.settings_ui.is_open);
        assert_eq!(root.appearance.ui_scale, ScalePreset::Percent200);
    }

    #[test]
    fn theme_fixture_opens_settings() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::AppSettingsThemeOpen));

        assert!(root.settings_ui.is_open);
    }

    #[test]
    fn preview_ready_fixture_seeds_selected_video_metadata() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::PreviewReady));

        assert_eq!(root.active_view, ActiveView::Workspace);
        assert_eq!(
            root.file_queue
                .selected_file()
                .map(|file| file.name.as_str()),
            Some("source_render.mov")
        );
        assert_eq!(
            root.selected_source_metadata()
                .map(|metadata| metadata.source_kind()),
            Some(SourceKind::Video)
        );
    }

    #[test]
    fn multitrack_fixture_keeps_audio_offsets_and_mute_state() {
        let mut root = ScissorsRoot::new();
        root.apply_visual_fixture(Some(VisualFixture::PreviewMultitrack));
        let tracks = &root
            .file_queue
            .selected_file()
            .unwrap()
            .config
            .external_audio_tracks;
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].offset_millis, 0);
        assert_eq!(tracks[1].offset_millis, 25_000);
        assert!(tracks[1].muted);
        let core = crate::conversion_runner::core_config_from_gpui(
            &root.file_queue.selected_file().unwrap().config,
        );
        assert_eq!(&core.external_audio_tracks, tracks);
    }

    #[test]
    fn preview_crop_fixture_enters_crop_mode() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::PreviewCrop));

        assert!(root.preview_ui.crop_mode);
        assert!(root.preview_ui.draft_crop.is_some());
        assert_eq!(root.preview_ui.crop_aspect, "1:1");
    }

    #[test]
    fn workspace_empty_fixture_clears_workspace_state() {
        let mut root = ScissorsRoot::new();
        root.apply_visual_fixture(Some(VisualFixture::PreviewReady));

        root.apply_visual_fixture(Some(VisualFixture::WorkspaceEmpty));

        assert_eq!(root.active_view, ActiveView::Workspace);
        assert!(root.file_queue.files().is_empty());
        assert_eq!(
            root.selected_source_metadata_entry().status,
            MetadataStatus::Idle
        );
    }

    #[test]
    fn workspace_audio_fixture_seeds_selected_audio_source() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::WorkspaceAudio));

        assert_eq!(root.active_view, ActiveView::Workspace);
        assert_eq!(
            root.file_queue
                .selected_file()
                .map(|file| file.name.as_str()),
            Some("source_mix.wav")
        );
        assert_eq!(
            root.selected_source_metadata()
                .map(|metadata| metadata.source_kind()),
            Some(SourceKind::Audio)
        );
    }

    #[test]
    fn workspace_image_fixture_seeds_selected_image_source() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::WorkspaceImage));

        assert_eq!(root.active_view, ActiveView::Workspace);
        assert_eq!(
            root.selected_source_metadata()
                .map(|metadata| metadata.source_kind()),
            Some(SourceKind::Image)
        );
    }

    #[test]
    fn settings_audio_filters_fixture_opens_audio_tool_with_enabled_filters() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::SettingsAudioFilters));

        let selected = root
            .file_queue
            .selected_file()
            .expect("audio filters fixture should select a file");
        assert_eq!(root.workspace_ui.tool, 2);
        assert!(selected.config.audio_normalize);
        assert_eq!(selected.config.audio_volume, 132);
        assert!(selected.config.audio_filters.compressor_enabled);
        assert!(selected.config.audio_filters.limiter.enabled);
        assert!(selected.config.audio_filters.high_pass.enabled);
        assert!(selected.config.audio_filters.low_pass.enabled);
        assert!(selected.config.audio_filters.stereo_width.enabled);
    }

    #[test]
    fn settings_metadata_fixture_opens_metadata_tab_with_source_tags() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::SettingsMetadata));

        assert_eq!(root.settings_ui.active_tab, SettingsTab::Metadata);
        assert_eq!(
            root.selected_source_metadata()
                .and_then(|metadata| metadata.tags)
                .and_then(|tags| tags.title),
            Some("Original Scene 24A".to_string())
        );
    }

    #[test]
    fn export_dialog_fixture_opens_the_export_dialog() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::ExportDialog));

        assert!(root.export_ui.open);
        assert_eq!(
            root.file_queue
                .selected_file()
                .and_then(|file| file.config.custom_width.as_deref()),
            Some("1280")
        );
    }

    #[test]
    fn settings_video_filters_fixture_opens_video_tool_with_enabled_filters() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::SettingsVideoFilters));

        let selected = root
            .file_queue
            .selected_file()
            .expect("video filters fixture should select a file");
        assert_eq!(root.workspace_ui.tool, 1);
        assert!(selected.config.video_filters.color.brightness.enabled);
        assert!(selected.config.video_filters.color.contrast.enabled);
        assert!(selected.config.video_filters.temperature.enabled);
        assert!(selected.config.video_filters.sharpen.enabled);
        assert!(selected.config.video_filters.denoise_enabled);
        assert_eq!(
            selected.config.video_filters.deinterlace,
            crate::settings::DeinterlaceMode::Auto
        );
    }

    #[test]
    fn settings_images_fixture_opens_images_tab_for_image_source() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::SettingsImages));

        assert_eq!(root.settings_ui.active_tab, SettingsTab::Images);
        assert_eq!(
            root.selected_source_metadata()
                .map(|metadata| metadata.source_kind()),
            Some(SourceKind::Image)
        );
    }

    #[test]
    fn settings_presets_fixture_opens_presets_tab_with_custom_draft() {
        let mut root = ScissorsRoot::new();

        root.apply_visual_fixture(Some(VisualFixture::SettingsPresets));

        assert!(root.export_ui.open);
        assert_eq!(root.settings_ui.preset_name_draft, "Client Review MP4");
        assert!(
            root.presets
                .iter()
                .any(|preset| preset.id == "custom-review")
        );
    }
}

mod surface_highlights {
    use super::*;

    #[test]
    fn frame_highlight_width_matches_platform_renderer() {
        let expected = if cfg!(target_os = "macos") { 0.5 } else { 1.0 };

        assert_eq!(frame_highlight_px(), expected);
    }
}

mod preview_shell {
    use super::preview_panel::PreviewShellStateInput;
    use super::*;

    fn empty_filters() -> &'static AvailableFilters {
        static FILTERS: AvailableFilters = AvailableFilters {
            eq: false,
            hue: false,
            colortemperature: false,
            unsharp: false,
            gblur: false,
            hqdn3d: false,
            deband: false,
            vignette: false,
            bwdif: false,
            highpass: false,
            lowpass: false,
            afftdn: false,
            deesser: false,
            bass: false,
            treble: false,
            acompressor: false,
            loudnorm: false,
            volume: false,
            stereotools: false,
            alimiter: false,
        };
        &FILTERS
    }

    fn settings_state<'a>(
        config: &'a ConversionConfig,
        metadata: Option<&'a SourceMetadata>,
        status: MetadataStatus,
    ) -> SettingsRenderState<'a> {
        SettingsRenderState {
            palette: theme::palette(crate::appearance::ColorTheme::Dark),
            active_tab: SettingsTab::VideoFilters,
            tooltip_visible_id: None,
            config,
            metadata,
            metadata_status: status,
            metadata_error: None,
            settings_disabled: false,
            video_width_focus: None,
            video_height_focus: None,
            metadata_focuses: SettingsMetadataInputFocuses {
                title: None,
                artist: None,
                album: None,
                genre: None,
                date: None,
                comment: None,
                service_name: None,
                service_provider: None,
            },
            available_filters: empty_filters(),
        }
    }

    fn crop_state() -> PreviewCropRenderState {
        PreviewCropRenderState {
            crop_mode: false,
            draft_crop: None,
            applied_crop: None,
            crop_aspect: "free".to_string(),
            has_crop_dimensions: false,
            draft_size: None,
            status: None,
            can_detect_bars: false,
            rotation: "0".to_string(),
            flip_horizontal: false,
            flip_vertical: false,
        }
    }

    #[test]
    fn ready_video_metadata_populates_timeline_bounds() {
        let config = ConversionConfig::default();
        let metadata = SourceMetadata {
            media_kind: Some(SourceKind::Video),
            duration: Some("90.4".to_string()),
            ..SourceMetadata::default()
        };
        let file = FileItem::from_path("video", "/tmp/render.mov", 1024);
        let settings = settings_state(&config, Some(&metadata), MetadataStatus::Ready);

        let state = preview_shell_state(PreviewShellStateInput {
            selected_file: Some(&file),
            settings: &settings,
            crop: crop_state(),
            overlay: PreviewOverlayRenderState::empty(),
            canvas: PreviewCanvasRenderState::default(),
            playback: preview_playback_state(PreviewMediaKind::Video, 90.4, None, None),
            presentation: PreviewRenderPresentation::default(),
            render_image: None,
            runtime_error: None,
        });

        assert_eq!(state.availability.media_kind, PreviewMediaKind::Video);
        assert!(preview_trim_enabled(&state));
        assert_eq!(state.playback.start_value(), 0.0);
        assert_eq!(state.playback.end_value(), 90.4);
        assert_eq!(state.playback.output_duration(), 90.4);
    }

    #[test]
    fn ready_video_metadata_uses_configured_trim_bounds() {
        let config = ConversionConfig {
            start_time: Some("00:00:05.000".to_string()),
            end_time: Some("00:00:30.250".to_string()),
            ..ConversionConfig::default()
        };
        let metadata = SourceMetadata {
            media_kind: Some(SourceKind::Video),
            duration: Some("90.4".to_string()),
            ..SourceMetadata::default()
        };
        let file = FileItem::from_path("video", "/tmp/render.mov", 1024);
        let settings = settings_state(&config, Some(&metadata), MetadataStatus::Ready);

        let state = preview_shell_state(PreviewShellStateInput {
            selected_file: Some(&file),
            settings: &settings,
            crop: crop_state(),
            overlay: PreviewOverlayRenderState::empty(),
            canvas: PreviewCanvasRenderState::default(),
            playback: preview_playback_state(
                PreviewMediaKind::Video,
                90.4,
                config.start_time.as_deref(),
                config.end_time.as_deref(),
            ),
            presentation: PreviewRenderPresentation::default(),
            render_image: None,
            runtime_error: None,
        });

        assert_eq!(state.playback.start_value(), 5.0);
        assert_eq!(state.playback.end_value(), 30.25);
        assert_eq!(state.playback.output_duration(), 25.25);
    }

    #[test]
    fn image_metadata_disables_timeline_trimming() {
        let config = ConversionConfig::default();
        let metadata = SourceMetadata {
            media_kind: Some(SourceKind::Image),
            duration: Some("10.0".to_string()),
            ..SourceMetadata::default()
        };
        let file = FileItem::from_path("image", "/tmp/still.png", 1024);
        let settings = settings_state(&config, Some(&metadata), MetadataStatus::Ready);

        let state = preview_shell_state(PreviewShellStateInput {
            selected_file: Some(&file),
            settings: &settings,
            crop: crop_state(),
            overlay: PreviewOverlayRenderState::empty(),
            canvas: PreviewCanvasRenderState::default(),
            playback: PreviewPlaybackState::new(false),
            presentation: PreviewRenderPresentation::default(),
            render_image: None,
            runtime_error: None,
        });

        assert_eq!(state.availability.media_kind, PreviewMediaKind::Image);
        assert!(state.availability.trim_disabled);
    }

    #[test]
    fn audio_metadata_hides_visual_controls() {
        let config = ConversionConfig::default();
        let metadata = SourceMetadata {
            media_kind: Some(SourceKind::Audio),
            duration: Some("00:00:12.500".to_string()),
            ..SourceMetadata::default()
        };
        let settings = settings_state(&config, Some(&metadata), MetadataStatus::Ready);

        let state = preview_shell_state(PreviewShellStateInput {
            selected_file: None,
            settings: &settings,
            crop: crop_state(),
            overlay: PreviewOverlayRenderState::empty(),
            canvas: PreviewCanvasRenderState::default(),
            playback: PreviewPlaybackState::new(false),
            presentation: PreviewRenderPresentation::default(),
            render_image: None,
            runtime_error: None,
        });

        assert_eq!(state.availability.media_kind, PreviewMediaKind::Audio);
        assert!(state.availability.hide_visual_controls);
        assert!(!preview_visual_controls_visible(&state));
        assert_eq!(preview_duration_seconds(Some(&metadata)), 12.5);
    }

    #[test]
    fn timeline_has_one_track_whose_clips_follow_the_removed_segments() {
        let config = ConversionConfig::default();
        let metadata = SourceMetadata {
            media_kind: Some(SourceKind::Audio),
            duration: Some("00:00:40.000".to_string()),
            ..SourceMetadata::default()
        };
        let settings = settings_state(&config, Some(&metadata), MetadataStatus::Ready);
        let mut playback = PreviewPlaybackState::new(false);
        playback.sync_media(MediaSnapshot {
            current_time: 25.0,
            duration: 40.0,
            paused: true,
        });
        playback.sync_initial_values(None, None);
        playback.sync_cut_segments(&[crate::settings::CutSegment {
            start_time: "00:00:10.000".to_string(),
            end_time: "00:00:20.000".to_string(),
        }]);

        let state = preview_shell_state(PreviewShellStateInput {
            selected_file: None,
            settings: &settings,
            crop: crop_state(),
            overlay: PreviewOverlayRenderState::empty(),
            canvas: PreviewCanvasRenderState::default(),
            playback,
            presentation: PreviewRenderPresentation::default(),
            render_image: None,
            runtime_error: None,
        });
        let tracks = preview_timeline_tracks(&state);

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].label, "Audio");
        assert_eq!(tracks[0].clips.len(), 2);
        assert_eq!(
            preview_timeline_info(&state.playback, &tracks),
            "Clip 2 of 2 · 00:20.0 – 00:40.0"
        );
    }

    #[test]
    fn timeline_info_explains_editing_when_the_playhead_is_outside_clips() {
        let mut playback = PreviewPlaybackState::new(false);
        let tracks = |playback: &PreviewPlaybackState| {
            vec![super::preview_panel::PreviewTimelineTrack {
                label: "Video",
                icon: crate::material::Symbol::VideoFile,
                clips: playback.clips(),
            }]
        };

        assert_eq!(
            preview_timeline_info(&playback, &tracks(&playback)),
            "Select a video or audio source to edit its timeline."
        );

        playback.sync_media(MediaSnapshot {
            current_time: 15.0,
            duration: 40.0,
            paused: true,
        });
        playback.sync_initial_values(None, None);
        playback.sync_cut_segments(&[crate::settings::CutSegment {
            start_time: "00:00:10.000".to_string(),
            end_time: "00:00:20.000".to_string(),
        }]);

        assert_eq!(
            preview_timeline_info(&playback, &tracks(&playback)),
            "Drag clip edges to trim. Press X to remove a segment at the playhead."
        );
    }

    #[test]
    fn loading_metadata_keeps_preview_unknown() {
        let config = ConversionConfig::default();
        let metadata = SourceMetadata {
            media_kind: Some(SourceKind::Video),
            duration: Some("90.0".to_string()),
            ..SourceMetadata::default()
        };
        let settings = settings_state(&config, Some(&metadata), MetadataStatus::Loading);

        let state = preview_shell_state(PreviewShellStateInput {
            selected_file: None,
            settings: &settings,
            crop: crop_state(),
            overlay: PreviewOverlayRenderState::empty(),
            canvas: PreviewCanvasRenderState::default(),
            playback: PreviewPlaybackState::new(false),
            presentation: PreviewRenderPresentation::default(),
            render_image: None,
            runtime_error: None,
        });

        assert_eq!(state.availability.media_kind, PreviewMediaKind::Unknown);
        assert!(state.availability.trim_disabled);
    }

    #[test]
    fn timeline_ruler_ticks_use_a_readable_step_with_at_most_eight_ticks() {
        assert_eq!(
            timeline_ruler_ticks(40.0),
            vec![0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0]
        );
        assert_eq!(
            timeline_ruler_ticks(3.0),
            vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5]
        );
        assert_eq!(timeline_ruler_ticks(596.0).len(), 5);
        assert_eq!(timeline_ruler_ticks(90.4).last(), Some(&75.0));
        assert!(timeline_ruler_ticks(0.0).is_empty());
        assert!(timeline_ruler_ticks(f64::NAN).is_empty());
    }

    #[test]
    fn timeline_times_drop_the_hour_for_short_sources() {
        assert_eq!(format_timeline_ruler_time(65.0, 120.0), "01:05");
        assert_eq!(format_timeline_ruler_time(1.5, 3.0), "00:01.5");
        assert_eq!(format_timeline_ruler_time(3_725.0, 7_200.0), "1:02:05");
        assert_eq!(format_timeline_time(5.34, 120.0), "00:05.3");
        assert_eq!(format_timeline_time(59.96, 120.0), "01:00.0");
        assert_eq!(format_timeline_time(3_725.0, 7_200.0), "1:02:05.0");
    }

    #[test]
    fn clip_durations_are_compact() {
        assert_eq!(format_clip_duration(4.24), "4.2 s");
        assert_eq!(format_clip_duration(725.0), "12:05");
        assert_eq!(format_clip_duration(3_725.0), "1:02:05");
    }

    #[test]
    fn timeline_fraction_from_percent_clamps_to_track_range() {
        assert_eq!(timeline_fraction_from_percent(-25.0), 0.0);
        assert_eq!(timeline_fraction_from_percent(50.0), 0.5);
        assert_eq!(timeline_fraction_from_percent(125.0), 1.0);
    }

    #[test]
    fn timeline_slider_percent_from_bounds_clamps_pointer_to_track() {
        let bounds = Bounds {
            origin: point(px(10.0), px(0.0)),
            size: size(px(100.0), px(30.0)),
        };

        assert_eq!(
            timeline_slider_percent_from_bounds(point(px(60.0), px(0.0)), bounds),
            0.5
        );
        assert_eq!(
            timeline_slider_percent_from_bounds(point(px(-10.0), px(0.0)), bounds),
            0.0
        );
        assert_eq!(
            timeline_slider_percent_from_bounds(point(px(140.0), px(0.0)), bounds),
            1.0
        );
    }

    #[test]
    fn preview_canvas_layout_metrics_preserve_media_aspect_when_zooming() {
        let metrics = preview_canvas_layout_metrics(1000.0, 500.0, 1920.0, 1080.0, 1.18, 0.0, 0.0)
            .expect("metrics");

        assert!(((metrics.width / metrics.height) - (16.0 / 9.0)).abs() < 0.000_001);
        assert!(metrics.width > 1000.0);
        assert!((metrics.top - -45.0).abs() < 0.000_001);
    }

    #[test]
    fn preview_canvas_pan_limits_allow_original_overscroll_window() {
        let (max_x, max_y) =
            preview_canvas_pan_limits(1000.0, 500.0, 1920.0, 1080.0, 0.25).expect("limits");

        assert_eq!(max_x, 1000.0);
        assert_eq!(max_y, 500.0);
    }

    #[test]
    fn preview_canvas_initial_zoom_starts_inset_from_object_contain() {
        let zoom =
            preview_canvas_initial_zoom(1000.0, 500.0, 1920.0, 1080.0).expect("initial zoom");

        assert!((zoom - 0.9).abs() < 0.000_001);

        let metrics = preview_canvas_layout_metrics(1000.0, 500.0, 1920.0, 1080.0, zoom, 0.0, 0.0)
            .expect("metrics");
        assert!(metrics.width < 1000.0);
        assert!(metrics.height < 500.0);
        assert!(metrics.left > 0.0);
        assert!(metrics.top > 0.0);
    }
}

mod visual_contract {
    use super::*;

    #[test]
    fn audio_slider_helpers_map_values_to_original_range() {
        assert_eq!(settings_panel::range_fraction(100, 0, 200), 0.5);
        assert_eq!(settings_panel::range_value_from_fraction(0.5, 0, 200), 100);
        assert_eq!(
            settings_panel::range_value_for_key(100, 0, 200, "right"),
            Some(101)
        );
        assert_eq!(
            settings_panel::range_value_for_key(100, 0, 200, "pageup"),
            Some(80)
        );
        assert_eq!(
            settings_panel::range_value_for_key(100, 0, 200, "home"),
            Some(0)
        );
    }

    #[test]
    fn timeline_keyboard_helper_maps_standard_slider_keys() {
        assert_eq!(
            timeline_keyboard_time_for_key(30.0, 60.0, "right"),
            Some(30.6)
        );
        assert_eq!(
            timeline_keyboard_time_for_key(30.0, 60.0, "pageup"),
            Some(24.0)
        );
        assert_eq!(
            timeline_keyboard_time_for_key(30.0, 60.0, "end"),
            Some(60.0)
        );
        assert_eq!(timeline_keyboard_time_for_key(30.0, 0.0, "right"), None);
    }

    #[test]
    fn preview_keyboard_delta_helpers_ignore_irrelevant_keys() {
        assert_eq!(
            preview_crop_keyboard_delta(DragHandle::North, "up", false),
            Some(PreviewPoint { x: 0.0, y: -0.01 })
        );
        assert_eq!(
            preview_crop_keyboard_delta(DragHandle::North, "left", false),
            None
        );
        assert_eq!(
            preview_overlay_keyboard_delta("down", false),
            Some(PreviewPoint { x: 0.0, y: 0.01 })
        );
        assert_eq!(
            preview_crop_keyboard_delta(DragHandle::North, "up", true),
            Some(PreviewPoint { x: 0.0, y: -0.05 })
        );
        assert_eq!(
            preview_overlay_keyboard_delta("down", true),
            Some(PreviewPoint { x: 0.0, y: 0.05 })
        );
    }

    #[test]
    fn preview_crop_handles_use_screen_space_cursors() {
        assert_eq!(
            preview_panel::crop_handle_screen_cursor(DragHandle::NorthEast),
            "nesw-resize"
        );
        assert_eq!(
            preview_panel::crop_handle_screen_cursor(DragHandle::NorthWest),
            "nwse-resize"
        );
    }
}

fn test_settings_path() -> PathBuf {
    let sequence = TEST_SETTINGS_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis();

    std::env::temp_dir()
        .join("scissors-root-persistence-tests")
        .join(format!("{}-{millis}-{sequence}", std::process::id()))
        .join("settings.json")
}
