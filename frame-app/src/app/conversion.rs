use super::*;

impl FrameRoot {
    pub(super) fn prepare_project_export(&mut self) -> Vec<frame_core::types::ConversionTask> {
        if self.is_processing || self.update_installation_in_progress() {
            return Vec::new();
        }
        let Some(output_directory) = self
            .default_output_directory
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
        else {
            return Vec::new();
        };
        let Some(config) = self.project_config() else {
            return Vec::new();
        };
        if config.timeline_sources.is_empty() {
            return Vec::new();
        }
        let source = config
            .timeline_sources
            .iter()
            .find(|source| source.kind != "audio")
            .or_else(|| config.timeline_sources.first())
            .unwrap();
        let mut job = FileItem::from_path("project-export", source.path.clone(), 0);
        job.name = "Current project".to_string();
        job.output_name.clone_from(&self.project.output_name);
        job.config = self.project.output.clone();
        job.status = FileStatus::Queued;
        let mut task = conversion_task_from_file(&job, &output_directory);
        task.config = config;
        if let Some(existing) = self.file_queue.file_by_id_mut("project-export") {
            *existing = job;
        } else {
            self.file_queue.add_file(job);
        }
        vec![task]
    }
    pub(super) fn start_project_export(&mut self, cx: &mut Context<Self>) {
        if self.is_processing || self.update_installation_in_progress() {
            return;
        }

        let tasks = self.prepare_project_export();
        if tasks.is_empty() {
            self.active_conversion_task_ids.clear();
            return;
        }

        self.active_conversion_task_ids = tasks.iter().map(|task| task.id.clone()).collect();
        self.is_processing = true;
        self.spawn_project_export(tasks, cx);
        cx.notify();
    }
    pub(super) fn spawn_project_export(
        &self,
        tasks: Vec<frame_core::types::ConversionTask>,
        cx: &Context<Self>,
    ) {
        let (tx, rx) = mpsc::channel();
        let controller = self.conversion_processes.clone();

        cx.background_spawn(async move {
            let result = run_conversion_batch_with_control(tasks, &controller, |event| {
                let _ = tx.send(event);
            });
            if let Err(error) = result {
                eprintln!("Conversion batch failed: {error}");
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            loop {
                let mut is_disconnected = false;
                loop {
                    match rx.try_recv() {
                        Ok(event) => {
                            if this
                                .update(cx, |root, cx| {
                                    root.apply_conversion_event(event);
                                    cx.notify();
                                })
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            is_disconnected = true;
                            break;
                        }
                    }
                }

                if is_disconnected {
                    this.update(cx, |root, cx| {
                        root.refresh_processing_state_from_queue();
                        cx.notify();
                    })
                    .ok();
                    return;
                }

                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
            }
        })
        .detach();
    }
    pub(super) fn cancel_conversion_task(&mut self, id: &str) -> bool {
        if !self
            .file_queue
            .file_by_id(id)
            .is_some_and(|file| file.status.can_be_cancelled())
        {
            return false;
        }

        match self.conversion_processes.cancel_task(id) {
            Ok(()) => self.file_queue.mark_file_cancelling(id),
            Err(error) => {
                self.log_conversion_control_error(id, "cancel", &error);
                false
            }
        }
    }
    pub(super) fn prepare_file_for_reconversion(&mut self, id: &str) -> bool {
        if self.update_installation_in_progress() {
            return false;
        }
        self.file_queue.prepare_file_for_reconversion(id)
    }
    pub(super) fn remove_file_from_queue(&mut self, id: &str) -> bool {
        if self.update_installation_in_progress() {
            return false;
        }
        let Some(status) = self.file_queue.file_by_id(id).map(|file| file.status) else {
            return false;
        };

        if status.can_be_cancelled()
            && let Err(error) = self.conversion_processes.cancel_task(id)
        {
            self.log_conversion_control_error(id, "cancel", &error);
            return false;
        }

        let removed = self.file_queue.remove_file(id).is_some();
        if removed {
            self.project.clips.retain(|clip| clip.id != id);
            self.source_metadata.remove(id);
            self.media_assets.thumbnails.remove(id);
            self.conversion_events.remove_logs(id);
            self.refresh_processing_state_from_queue();
        }

        removed
    }
    pub(super) fn log_conversion_control_error(
        &mut self,
        id: &str,
        action: &str,
        error: &frame_core::error::ConversionError,
    ) {
        self.conversion_events.apply_conversion_event(
            &mut self.file_queue,
            ConversionEvent::log(
                id.to_string(),
                format!("[ERROR] Failed to {action}: {error}"),
            ),
        );
    }
    pub(super) fn apply_conversion_event(&mut self, event: ConversionEvent) {
        self.conversion_events
            .apply_conversion_event(&mut self.file_queue, event);
        self.refresh_processing_state_from_queue();
    }

    fn refresh_processing_state_from_queue(&mut self) {
        let was_processing = self.is_processing;
        self.is_processing = !all_conversions_settled(&self.file_queue);

        if was_processing && !self.is_processing {
            self.notify_active_conversion_batch_finished();
        }
    }

    fn notify_active_conversion_batch_finished(&mut self) {
        let summary = conversion_finished_notification_for_task_ids(
            &self.file_queue,
            &self.active_conversion_task_ids,
        );
        self.active_conversion_task_ids.clear();

        if let Some(summary) = summary {
            self.notifier.notify_conversion_finished(summary);
        }
    }
}
