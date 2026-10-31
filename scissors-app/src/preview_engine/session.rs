use std::sync::{
    Mutex, MutexGuard, TryLockError,
    atomic::{AtomicU64, Ordering},
};

use super::metrics::PreviewRuntimeMetricsStore;
use super::{
    LatestFrameSnapshot, LatestFrameStore, PreviewCommand, PreviewDimensions, PreviewEngineError,
    PreviewPlaybackSnapshot, PreviewSessionConfig, PreviewSessionSnapshot, PreviewSessionStatus,
    PreviewSourceKind, RunningPreviewProcess, start_ffmpeg_preview_process,
};

pub struct PreviewSession {
    config: Mutex<PreviewSessionConfig>,
    dimensions: Mutex<PreviewDimensions>,
    duration_seconds: Mutex<f64>,
    frame_store: LatestFrameStore,
    metrics: PreviewRuntimeMetricsStore,
    pipeline: Mutex<Option<RunningPreviewProcess>>,
    playing: Mutex<bool>,
    status: Mutex<PreviewSessionStatus>,
    /// Playback served by `snapshot` while a command holds the pipeline.
    last_playback: Mutex<PreviewPlaybackSnapshot>,
    /// The newest queued seek, reported until it lands.
    pending_seek: Mutex<Option<(u64, f64)>>,
    seek_tickets: AtomicU64,
}

/// A command with its place in the seek order; see [`PreviewSession::queue`].
#[derive(Clone, Copy, Debug)]
pub struct QueuedPreviewCommand {
    command: PreviewCommand,
    ticket: u64,
}

impl PreviewSession {
    /// Starts a preview session for an image, video, or audio source.
    ///
    /// # Errors
    ///
    /// Returns an error when the config is invalid or the `FFmpeg` preview
    /// process cannot render the initial frame.
    pub fn start(config: PreviewSessionConfig) -> Result<Self, PreviewEngineError> {
        config.validate()?;
        let frame_store = LatestFrameStore::new();
        let metrics = PreviewRuntimeMetricsStore::new();

        match config.source_kind {
            PreviewSourceKind::Image | PreviewSourceKind::Video | PreviewSourceKind::Audio => {
                let (pipeline, dimensions, duration_seconds) =
                    start_ffmpeg_preview_process(&config, frame_store.clone(), metrics.clone())?;
                Ok(Self {
                    config: Mutex::new(config),
                    dimensions: Mutex::new(dimensions),
                    duration_seconds: Mutex::new(duration_seconds),
                    frame_store,
                    metrics,
                    pipeline: Mutex::new(Some(pipeline)),
                    playing: Mutex::new(false),
                    status: Mutex::new(PreviewSessionStatus::Ready),
                    last_playback: Mutex::new(PreviewPlaybackSnapshot {
                        position_seconds: 0.0,
                        duration_seconds,
                        playing: false,
                    }),
                    pending_seek: Mutex::new(None),
                    seek_tickets: AtomicU64::new(0),
                })
            }
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn new_for_test(config: PreviewSessionConfig) -> Self {
        let frame_store = LatestFrameStore::new();
        let metrics = PreviewRuntimeMetricsStore::new();
        Self {
            dimensions: Mutex::new(config.target_dimensions()),
            duration_seconds: Mutex::new(config.duration_seconds),
            last_playback: Mutex::new(PreviewPlaybackSnapshot {
                position_seconds: 0.0,
                duration_seconds: config.duration_seconds,
                playing: false,
            }),
            pending_seek: Mutex::new(None),
            seek_tickets: AtomicU64::new(0),
            config: Mutex::new(config),
            frame_store,
            metrics,
            pipeline: Mutex::new(None),
            playing: Mutex::new(false),
            status: Mutex::new(PreviewSessionStatus::Ready),
        }
    }

    /// Rebuilds the preview pipeline in-place while keeping the existing frame
    /// store alive until the replacement process has produced its first frame.
    ///
    /// # Errors
    ///
    /// Returns an error when the replacement config is invalid or the
    /// replacement `FFmpeg` process cannot be started.
    #[expect(
        clippy::significant_drop_tightening,
        reason = "The pipeline mutex guard must live across the in-place FFmpeg reconfiguration."
    )]
    pub fn reconfigure(&self, config: PreviewSessionConfig) -> Result<(), PreviewEngineError> {
        config.validate()?;
        let dimensions = {
            let mut pipeline_guard = lock(&self.pipeline);
            let playing = *lock(&self.playing);
            let Some(pipeline) = pipeline_guard.as_mut() else {
                let (pipeline, dimensions, _) = start_ffmpeg_preview_process(
                    &config,
                    self.frame_store.clone(),
                    self.metrics.clone(),
                )?;
                *pipeline_guard = Some(pipeline);
                self.store_reconfigured_state(config, dimensions);
                return Ok(());
            };
            let seconds = pipeline.position();
            pipeline.reconfigure(config.clone(), seconds, playing, true)?
        };

        self.store_reconfigured_state(config, dimensions);
        Ok(())
    }

    #[must_use]
    pub fn latest_frame(&self) -> Option<LatestFrameSnapshot> {
        self.frame_store.latest()
    }

    #[must_use]
    pub fn frame_store(&self) -> LatestFrameStore {
        self.frame_store.clone()
    }

    pub fn mark_frame_presented(&self, generation: u64) {
        self.frame_store.mark_presented(generation);
        self.metrics.record_frame_presented();
    }

    /// Sends a playback command to the running preview pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying pipeline command fails.
    pub fn command(&self, command: PreviewCommand) -> Result<(), PreviewEngineError> {
        self.run(self.queue(command))
    }

    /// Orders `command` before it runs on another thread.
    ///
    /// Call this where commands are issued: a seek reports its target from
    /// [`Self::snapshot`] at once, and is skipped if a later seek is queued
    /// before it runs.
    pub fn queue(&self, command: PreviewCommand) -> QueuedPreviewCommand {
        let ticket = match command {
            PreviewCommand::SeekFast(seconds) | PreviewCommand::SeekPrecise(seconds) => {
                let ticket = self.seek_tickets.fetch_add(1, Ordering::SeqCst) + 1;
                *lock(&self.pending_seek) = Some((ticket, seconds));
                ticket
            }
            PreviewCommand::Play | PreviewCommand::Pause => 0,
        };
        QueuedPreviewCommand { command, ticket }
    }

    /// Runs a command from [`Self::queue`], blocking until the pipeline applies it.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying pipeline command fails.
    pub fn run(&self, queued: QueuedPreviewCommand) -> Result<(), PreviewEngineError> {
        let QueuedPreviewCommand { command, ticket } = queued;
        let superseded = || ticket != 0 && self.seek_tickets.load(Ordering::SeqCst) != ticket;
        if superseded() {
            return Ok(());
        }
        let result = self.run_on_pipeline(command, &superseded);
        if ticket != 0 {
            let mut pending = lock(&self.pending_seek);
            if pending.is_some_and(|(pending, _)| pending == ticket) {
                *pending = None;
            }
        }
        result
    }

    #[expect(
        clippy::significant_drop_tightening,
        reason = "The pipeline mutex guard must live across the selected FFmpeg process command."
    )]
    fn run_on_pipeline(
        &self,
        command: PreviewCommand,
        superseded: &dyn Fn() -> bool,
    ) -> Result<(), PreviewEngineError> {
        let next_playing = {
            let pipeline = lock(&self.pipeline);
            let Some(pipeline) = pipeline.as_ref() else {
                return Ok(());
            };
            // A later seek may have been queued while this one waited.
            if superseded() {
                return Ok(());
            }

            match command {
                PreviewCommand::Play => {
                    pipeline.resume()?;
                    Some(true)
                }
                PreviewCommand::Pause => {
                    pipeline.pause()?;
                    Some(false)
                }
                PreviewCommand::SeekFast(seconds) => {
                    let was_playing = *lock(&self.playing);
                    pipeline.seek(seconds, !was_playing, false)?;
                    None
                }
                PreviewCommand::SeekPrecise(seconds) => {
                    let was_playing = *lock(&self.playing);
                    pipeline.seek(seconds, !was_playing, true)?;
                    None
                }
            }
        };

        if let Some(playing) = next_playing {
            *lock(&self.playing) = playing;
        }

        Ok(())
    }

    /// Current session state. Never waits for an in-flight command: while one
    /// holds the pipeline, the last known playback is reported instead.
    #[must_use]
    pub fn snapshot(&self) -> PreviewSessionSnapshot {
        let read = |pipeline: &Option<RunningPreviewProcess>| {
            pipeline
                .as_ref()
                .map(|pipeline| (pipeline.duration(), pipeline.ended(), pipeline.position()))
        };
        let pipeline_snapshot = match self.pipeline.try_lock() {
            Ok(pipeline) => Some(read(&pipeline)),
            Err(TryLockError::Poisoned(pipeline)) => Some(read(&pipeline.into_inner())),
            Err(TryLockError::WouldBlock) => None,
        };
        let mut playback = pipeline_snapshot.map_or_else(
            || *lock(&self.last_playback),
            |pipeline_snapshot| {
                let duration = pipeline_snapshot.map_or_else(
                    || *lock(&self.duration_seconds),
                    |(duration, _, _)| self.update_duration(duration),
                );
                let ended = pipeline_snapshot.is_some_and(|(_, ended, _)| ended);
                let position =
                    pipeline_snapshot.map_or(
                        0.0,
                        |(_, ended, position)| {
                            if ended { duration } else { position }
                        },
                    );
                let playback = PreviewPlaybackSnapshot {
                    position_seconds: position,
                    duration_seconds: duration,
                    playing: pipeline_snapshot.is_some_and(|_| !ended && *lock(&self.playing)),
                };
                *lock(&self.last_playback) = playback;
                playback
            },
        );
        let pending_seek = *lock(&self.pending_seek);
        if let Some((_, seconds)) = pending_seek {
            playback.position_seconds = seconds;
        }

        let config = lock(&self.config);
        PreviewSessionSnapshot {
            file_id: config.file_id.clone(),
            source_kind: config.source_kind,
            dimensions: *lock(&self.dimensions),
            status: lock(&self.status).clone(),
            playback,
            frame_generation: self.frame_store.generation(),
            frame_stats: self.frame_store.stats(),
            runtime_metrics: self.metrics.snapshot(),
        }
    }

    /// Stops the preview pipeline and releases its `FFmpeg` processes.
    ///
    /// # Errors
    ///
    /// Returns an error when the preview pipeline cannot stop a child process.
    pub fn stop(&self) -> Result<(), PreviewEngineError> {
        let pipeline = { lock(&self.pipeline).take() };
        let result = pipeline.map_or(Ok(()), |mut pipeline| pipeline.stop());
        *lock(&self.playing) = false;
        result
    }

    fn update_duration(&self, duration: f64) -> f64 {
        if duration.is_finite() && duration > 0.0 {
            *lock(&self.duration_seconds) = duration;
            duration
        } else {
            *lock(&self.duration_seconds)
        }
    }

    fn store_reconfigured_state(
        &self,
        config: PreviewSessionConfig,
        dimensions: PreviewDimensions,
    ) {
        let duration_seconds = config.duration_seconds;
        *lock(&self.config) = config;
        *lock(&self.dimensions) = dimensions;
        *lock(&self.duration_seconds) = duration_seconds;
    }
}

impl Drop for PreviewSession {
    fn drop(&mut self) {
        let pipeline = { lock(&self.pipeline).take() };
        if let Some(mut pipeline) = pipeline {
            let _ = pipeline.stop();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
