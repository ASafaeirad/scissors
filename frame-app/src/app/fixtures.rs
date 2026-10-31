use super::*;

use crate::settings::{CutSegment, DeinterlaceMode, FilterStrength};
impl FrameRoot {
    pub(super) fn apply_visual_fixture(&mut self, fixture: Option<VisualFixture>) {
        match fixture {
            Some(VisualFixture::AppSettings) => self.open_app_settings(),
            Some(VisualFixture::AppSettingsThemeOpen) => {
                self.open_app_settings();
            }
            Some(VisualFixture::AppSettingsUiOpen) => {
                self.appearance = AppearanceSettings {
                    ui_scale: ScalePreset::Percent200,
                    ..self.appearance
                };
                self.open_app_settings();
            }
            Some(VisualFixture::LogsActive) => self.apply_logs_active_fixture(),
            Some(VisualFixture::M3Gallery) => self.m3_gallery = Some(m3::M3GalleryState::new()),
            Some(VisualFixture::PreviewCrop) => self.apply_preview_crop_fixture(),
            Some(VisualFixture::PreviewReady) => self.apply_preview_ready_fixture(),
            Some(VisualFixture::ProjectTimeline) => self.apply_project_timeline_fixture(),
            Some(VisualFixture::PreviewMultitrack) => {
                self.apply_preview_ready_fixture();
                if let Some(file) = self.file_queue.selected_file_mut() {
                    file.config.external_audio_tracks = vec![
                        frame_core::types::ExternalAudioTrack {
                            path: "/tmp/lofi_background.mp3".to_string(),
                            duration_millis: 90_400,
                            ..Default::default()
                        },
                        frame_core::types::ExternalAudioTrack {
                            path: "/tmp/voice_over.wav".to_string(),
                            duration_millis: 30_000,
                            offset_millis: 25_000,
                            muted: true,
                        },
                    ];
                }
            }
            Some(VisualFixture::SettingsAudioFilters) => {
                self.apply_settings_audio_filters_fixture();
            }
            Some(VisualFixture::SettingsImages) => self.apply_settings_images_fixture(),
            Some(VisualFixture::SettingsMetadata) => self.apply_settings_metadata_fixture(),
            Some(VisualFixture::SettingsPresets) => self.apply_settings_presets_fixture(),
            Some(VisualFixture::SettingsVideoFilters) => {
                self.apply_settings_video_filters_fixture();
            }
            Some(VisualFixture::ExportDialog) => self.apply_export_dialog_fixture(),
            Some(VisualFixture::WorkspaceAudio) => self.apply_workspace_audio_fixture(),
            Some(VisualFixture::WorkspaceEmpty) => self.apply_workspace_empty_fixture(),
            Some(VisualFixture::WorkspaceImage) => self.apply_workspace_image_fixture(),
            None => {}
        }
        self.workspace_ui.tool = match fixture {
            Some(VisualFixture::SettingsAudioFilters) => 2,
            Some(
                VisualFixture::ProjectTimeline
                | VisualFixture::PreviewCrop
                | VisualFixture::SettingsVideoFilters
                | VisualFixture::SettingsImages
                | VisualFixture::SettingsMetadata
                | VisualFixture::SettingsPresets,
            ) => 1,
            _ => 0,
        };
    }

    pub(super) fn apply_project_timeline_fixture(&mut self) {
        self.apply_preview_ready_fixture();
        let mut video = self.file_queue.selected_file().unwrap().clone();
        let mut metadata = self.selected_source_metadata().unwrap();
        metadata.duration = Some("4.0".to_string());
        metadata.width = Some(640);
        metadata.height = Some(360);
        metadata.resolution = Some("640x360".to_string());
        video.path = "/tmp/frame_project_fixture/video.mp4".to_string();
        video.name = "Morning walk.mp4".to_string();
        video.original_format = "mp4".to_string();
        video.id = "library-video".to_string();
        video.config.start_time = None;
        video.config.end_time = None;
        video.config.cut_segments.clear();
        // Stream 1 is the fixture video's audio, selected as metadata loading would.
        video.config.selected_audio_tracks = vec![1];
        self.file_queue = FileQueue::new();
        self.project.enabled = true;
        self.project.library.add_file(video);
        self.source_metadata.mark_ready("library-video", metadata);
        self.add_library_clip("library-video");
        self.add_library_clip("library-video");
        let audio = FileItem::from_path(
            "library-audio",
            "/tmp/frame_project_fixture/music.wav",
            4_000_000,
        );
        self.project.library.add_file(audio);
        self.source_metadata.mark_ready(
            "library-audio",
            SourceMetadata {
                media_kind: Some(SourceKind::Audio),
                duration: Some("4".to_string()),
                audio_tracks: vec![crate::settings::AudioTrack {
                    index: 0,
                    codec: "mp3".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        self.add_library_clip("library-audio");
        // V2 over V1 and a second audio track, as in a typical edit.
        self.add_project_track(super::project_tracks::TrackKind::Video);
        self.add_project_track(super::project_tracks::TrackKind::Audio);
        self.add_library_clip_on_track("library-audio", 1_000, 3);
        let id = self.project.clips[1].id.clone();
        self.set_project_clip_speed(&id, 2.0);
        self.move_project_clip_to_track(&id, 2);
        self.select_project_clip(&id);
        // The setup is not something to undo.
        self.project.history.clear();
        // Show the fitted layout, not the first-clip zoom that the setup above triggered.
        self.preview_ui.project_timeline_view = None;
    }

    pub(super) fn apply_workspace_empty_fixture(&mut self) {
        self.active_view = ActiveView::Workspace;
        self.file_queue = FileQueue::new();
        self.source_metadata = SourceMetadataStore::default();
        self.settings_ui.active_tab = SettingsTab::VideoFilters;
    }
    pub(super) fn apply_workspace_audio_fixture(&mut self) {
        self.seed_audio_source_fixture();
        self.settings_ui.active_tab = SettingsTab::AudioFilters;
    }
    pub(super) fn apply_workspace_image_fixture(&mut self) {
        self.seed_image_source_fixture();
        self.settings_ui.active_tab = SettingsTab::VideoFilters;
    }
    pub(super) fn apply_logs_active_fixture(&mut self) {
        self.active_view = ActiveView::Logs;
        self.file_queue.add_file(FileItem::from_path(
            "fixture-video",
            "/tmp/source_render.mov",
            1_572_864_000,
        ));
        self.file_queue
            .update_status("fixture-video", FileStatus::Converting, 64);

        for line in [
            "ffmpeg version 7.1.1 Copyright (c) 2000-2025 the FFmpeg developers",
            "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'source_render.mov':",
            "Stream #0:0: Video: prores (HQ), yuv422p10le, 3840x2160, 24 fps",
            "Stream mapping:",
            "frame=  148 fps= 27 q=-0.0 size=   65536kB time=00:00:06.16 bitrate=87145.2kbits/s speed=1.12x",
            "frame=  296 fps= 28 q=-0.0 size=  131072kB time=00:00:12.33 bitrate=87042.7kbits/s speed=1.14x",
            "frame=  444 fps= 29 q=-0.0 size=  196608kB time=00:00:18.50 bitrate=87054.9kbits/s speed=1.16x",
        ] {
            self.conversion_events.apply_conversion_event(
                &mut self.file_queue,
                ConversionEvent::log("fixture-video", line),
            );
        }
    }
    pub(super) fn apply_preview_ready_fixture(&mut self) {
        // Standalone preview fixtures exercise source controls without a project timeline.
        self.project.enabled = false;
        self.active_view = ActiveView::Workspace;
        let mut file =
            FileItem::from_path("fixture-preview", "/tmp/source_render.mov", 1_572_864_000);
        file.config.cut_segments = vec![
            CutSegment {
                start_time: "00:00:12.000".to_string(),
                end_time: "00:00:21.500".to_string(),
            },
            CutSegment {
                start_time: "00:00:55.000".to_string(),
                end_time: "00:01:02.000".to_string(),
            },
        ];
        self.file_queue.add_file(file);
        self.source_metadata.mark_ready(
            "fixture-preview".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                duration: Some("90.400000".to_string()),
                bitrate: Some("12000000".to_string()),
                video_codec: Some("prores".to_string()),
                audio_codec: Some("aac".to_string()),
                resolution: Some("3840x2160".to_string()),
                frame_rate: Some(24.0),
                width: Some(3840),
                height: Some(2160),
                video_bitrate_kbps: Some(12_000.0),
                ..SourceMetadata::default()
            },
        );
    }
    pub(super) fn apply_preview_crop_fixture(&mut self) {
        self.apply_preview_ready_fixture();
        self.preview_ui.crop_file_id = Some("fixture-preview".to_string());
        self.preview_ui.crop_mode = true;
        self.preview_ui.draft_crop = Some(CropRect {
            x: 0.18,
            y: 0.16,
            width: 0.64,
            height: 0.64,
        });
        self.preview_ui.crop_aspect = "1:1".to_string();
    }
    pub(super) fn apply_export_dialog_fixture(&mut self) {
        self.apply_preview_ready_fixture();
        if let Some(file) = self.file_queue.selected_file_mut() {
            file.config.resolution = "custom".to_string();
            file.config.custom_width = Some("1280".to_string());
            file.config.custom_height = Some("720".to_string());
            file.config.video_bitrate_mode = "crf".to_string();
            file.config.crf = 18;
            file.config.selected_audio_tracks = vec![1];
        }
        if let (Some(id), Some(mut metadata)) = (
            self.file_queue.selected_file().map(|file| file.id.clone()),
            self.selected_source_metadata(),
        ) {
            metadata.audio_tracks = vec![crate::settings::AudioTrack {
                index: 1,
                codec: "aac".to_string(),
                channels: Some("2".to_string()),
                ..Default::default()
            }];
            self.source_metadata.mark_ready(id, metadata);
        }
        self.open_export_dialog();
        self.export_ui.advanced = true;
    }
    pub(super) fn apply_settings_video_filters_fixture(&mut self) {
        self.apply_project_timeline_fixture();
        self.settings_ui.active_tab = SettingsTab::VideoFilters;
        if let Some(file) = self.file_queue.selected_file_mut() {
            file.config.video_filters.color.brightness.enabled = true;
            file.config.video_filters.color.brightness.value = 18;
            file.config.video_filters.color.contrast.enabled = true;
            file.config.video_filters.color.contrast.value = 112;
            file.config.video_filters.temperature.enabled = true;
            file.config.video_filters.temperature.value = 5200;
            file.config.video_filters.sharpen.enabled = true;
            file.config.video_filters.sharpen.value = 35;
            file.config.video_filters.denoise_enabled = true;
            file.config.video_filters.denoise_strength = FilterStrength::Medium;
            file.config.video_filters.deband.enabled = true;
            file.config.video_filters.deband.value = 28;
            file.config.video_filters.vignette.enabled = true;
            file.config.video_filters.vignette.value = 18;
            file.config.video_filters.deinterlace = DeinterlaceMode::Auto;
        }
    }
    pub(super) fn apply_settings_audio_filters_fixture(&mut self) {
        self.seed_audio_source_fixture();
        self.settings_ui.active_tab = SettingsTab::AudioFilters;
        if let Some(file) = self.file_queue.selected_file_mut() {
            file.config.audio_volume = 132;
            file.config.audio_normalize = true;
            file.config.audio_filters.compressor_enabled = true;
            file.config.audio_filters.compressor_strength = FilterStrength::Medium;
            file.config.audio_filters.limiter.enabled = true;
            file.config.audio_filters.limiter.value = -3;
            file.config.audio_filters.bass.enabled = true;
            file.config.audio_filters.bass.value = 4;
            file.config.audio_filters.treble.enabled = true;
            file.config.audio_filters.treble.value = 3;
            file.config.audio_filters.high_pass.enabled = true;
            file.config.audio_filters.high_pass.value = 80;
            file.config.audio_filters.low_pass.enabled = true;
            file.config.audio_filters.low_pass.value = 16_000;
            file.config.audio_filters.noise_reduction.enabled = true;
            file.config.audio_filters.noise_reduction.value = 12;
            file.config.audio_filters.de_esser.enabled = true;
            file.config.audio_filters.de_esser.value = 35;
            file.config.audio_filters.stereo_width.enabled = true;
            file.config.audio_filters.stereo_width.value = 118;
        }
    }
    pub(super) fn apply_settings_images_fixture(&mut self) {
        self.seed_image_source_fixture();
        if let Some(file) = self.file_queue.selected_file_mut() {
            file.config.container = "png".to_string();
            file.config.resolution = "custom".to_string();
            file.config.custom_width = Some("2048".to_string());
            file.config.custom_height = Some("1080".to_string());
        }
        self.settings_ui.active_tab = SettingsTab::Images;
    }
    pub(super) fn apply_settings_metadata_fixture(&mut self) {
        self.apply_preview_ready_fixture();
        self.settings_ui.active_tab = SettingsTab::Metadata;
        self.source_metadata.mark_ready(
            "fixture-preview".to_string(),
            SourceMetadata {
                media_kind: Some(SourceKind::Video),
                duration: Some("90.400000".to_string()),
                bitrate: Some("12000000".to_string()),
                video_codec: Some("prores".to_string()),
                audio_codec: Some("aac".to_string()),
                resolution: Some("3840x2160".to_string()),
                frame_rate: Some(24.0),
                width: Some(3840),
                height: Some(2160),
                video_bitrate_kbps: Some(12_000.0),
                tags: Some(SourceTags {
                    title: Some("Original Scene 24A".to_string()),
                    artist: Some("Frame Camera".to_string()),
                    album: Some("Dailies".to_string()),
                    genre: Some("Editorial".to_string()),
                    date: Some("2026".to_string()),
                    comment: Some("Camera roll A014".to_string()),
                }),
                ..SourceMetadata::default()
            },
        );
        if let Some(file) = self.file_queue.selected_file_mut() {
            file.config.metadata.title = Some("Render Scene 24A".to_string());
            file.config.metadata.comment = Some("Color pass".to_string());
        }
    }
    pub(super) fn apply_settings_presets_fixture(&mut self) {
        self.apply_preview_ready_fixture();
        self.open_export_dialog();
        self.settings_ui.preset_name_draft = "Client Review MP4".to_string();
        self.presets.push(PresetDefinition::custom(
            "custom-review".to_string(),
            "Client Review MP4".to_string(),
            ConversionConfig {
                video_bitrate_mode: "bitrate".to_string(),
                video_bitrate: "7500".to_string(),
                audio_bitrate: "192".to_string(),
                audio_channels: "stereo".to_string(),
                resolution: "1080p".to_string(),
                preset: "fast".to_string(),
                ..ConversionConfig::default()
            },
        ));
        self.apply_preset_to_selected("balanced-mp4");
        self.update_selected_config(|config| apply_fps(config, "30"));
    }

    fn seed_audio_source_fixture(&mut self) {
        self.active_view = ActiveView::Workspace;
        self.file_queue.add_file(FileItem::from_path(
            "fixture-audio",
            "/tmp/source_mix.wav",
            96_468_480,
        ));
        self.source_metadata.mark_ready(
            "fixture-audio",
            SourceMetadata {
                media_kind: Some(SourceKind::Audio),
                duration: Some("184.250000".to_string()),
                bitrate: Some("1536000".to_string()),
                audio_codec: Some("pcm_s16le".to_string()),
                audio_tracks: vec![
                    crate::settings::AudioTrack {
                        index: 0,
                        codec: "pcm_s16le".to_string(),
                        channels: Some("2".to_string()),
                        language: Some("eng".to_string()),
                        label: Some("Main mix".to_string()),
                        bitrate_kbps: Some(1536.0),
                        sample_rate: Some("48000".to_string()),
                    },
                    crate::settings::AudioTrack {
                        index: 1,
                        codec: "aac".to_string(),
                        channels: Some("2".to_string()),
                        language: Some("eng".to_string()),
                        label: Some("Reference".to_string()),
                        bitrate_kbps: Some(192.0),
                        sample_rate: Some("48000".to_string()),
                    },
                ],
                tags: Some(SourceTags {
                    title: Some("Source Mix".to_string()),
                    artist: Some("Frame Audio".to_string()),
                    album: None,
                    genre: None,
                    date: Some("2026".to_string()),
                    comment: Some("Stereo master".to_string()),
                }),
                ..SourceMetadata::default()
            },
        );
        if let Some(file) = self.file_queue.selected_file_mut() {
            file.config.container = "mp3".to_string();
            file.config.audio_codec = "mp3".to_string();
        }
    }

    fn seed_image_source_fixture(&mut self) {
        self.active_view = ActiveView::Workspace;
        self.file_queue.add_file(FileItem::from_path(
            "fixture-image",
            "/tmp/source_frame.png",
            8_388_608,
        ));
        self.source_metadata.mark_ready(
            "fixture-image",
            SourceMetadata {
                media_kind: Some(SourceKind::Image),
                video_codec: Some("png".to_string()),
                resolution: Some("4096x2160".to_string()),
                width: Some(4096),
                height: Some(2160),
                pixel_format: Some("rgba".to_string()),
                color_space: Some("bt709".to_string()),
                color_range: Some("pc".to_string()),
                ..SourceMetadata::default()
            },
        );
    }
}
