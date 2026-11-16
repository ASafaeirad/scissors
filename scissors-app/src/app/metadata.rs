use super::*;

impl ScissorsRoot {
    pub(super) fn selected_source_metadata_entry(&self) -> SourceMetadataEntry {
        self.settings_target()
            .map_or_else(SourceMetadataEntry::default, |target| {
                self.metadata_entry_for(&target)
            })
    }

    pub(super) fn selected_source_metadata(&self) -> Option<SourceMetadata> {
        self.metadata_for_target(self.settings_target().as_ref()?)
    }

    pub(super) fn queue_source_metadata_probe(
        &mut self,
        file_id: String,
        file_path: String,
        cx: &mut Context<Self>,
    ) {
        self.queue_source_metadata_probe_inner(file_id, file_path, true, cx);
    }

    pub(super) fn queue_restored_source_metadata_probe(
        &mut self,
        file_id: String,
        file_path: String,
        cx: &mut Context<Self>,
    ) {
        self.queue_source_metadata_probe_inner(file_id, file_path, false, cx);
    }

    fn queue_source_metadata_probe_inner(
        &mut self,
        file_id: String,
        file_path: String,
        normalize_selected_config: bool,
        cx: &mut Context<Self>,
    ) {
        self.source_metadata.mark_loading(file_id.clone());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let metadata = probe_source_metadata(&file_path);
                    let thumbnail = metadata
                        .as_ref()
                        .ok()
                        .filter(|metadata| metadata.media_kind == Some(SourceKind::Video))
                        .and_then(|_| super::media_assets::video_thumbnail(&file_path));
                    (metadata, thumbnail)
                })
                .await;

            this.update(cx, |root, cx| {
                let (result, thumbnail) = result;
                if let Some(thumbnail) = thumbnail
                    && (root.project.library.file_by_id(&file_id).is_some()
                        || root.file_queue.file_by_id(&file_id).is_some())
                {
                    root.media_assets
                        .thumbnails
                        .insert(file_id.clone(), thumbnail);
                }
                match result {
                    Ok(metadata) => {
                        if normalize_selected_config
                            && let Some(file) = root
                                .project
                                .library
                                .file_by_id_mut(&file_id)
                                .or_else(|| root.file_queue.file_by_id_mut(&file_id))
                        {
                            initialize_output_config(&mut file.config, Some(&metadata));
                        }
                        root.source_metadata.mark_ready(file_id.clone(), metadata);
                        if root.file_queue.selected_file_id() == Some(file_id.as_str()) {
                            let selected_metadata = root.selected_source_metadata();
                            root.resolve_selected_settings_tab(selected_metadata.as_ref());
                        }
                    }
                    Err(error) => {
                        root.source_metadata
                            .mark_error(file_id.clone(), error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
