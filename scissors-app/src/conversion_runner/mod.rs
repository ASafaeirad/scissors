//! Native GPUI conversion runner backed by the shared Scissors ffmpeg argument builder.

mod config;
mod controller;
mod output_paths;
mod process;
mod runner;
#[cfg(test)]
mod tests;

pub use config::*;
pub use controller::*;
pub use output_paths::*;
pub use runner::*;

#[cfg(test)]
use crate::file_queue::FileItem;
#[cfg(test)]
use crate::settings::ConversionConfig as GpuiConversionConfig;
#[cfg(test)]
use runner::{ProgressDuration, ffmpeg_progress_from_line, next_batch_launch_count};
#[cfg(test)]
use scissors_core::{
    events::ConversionEvent,
    types::{ConversionTask, DEFAULT_MAX_CONCURRENCY},
};
