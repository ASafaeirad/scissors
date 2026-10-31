use super::{ScissorsRoot, ToggleFpsMeter, primitives::color};
use crate::theme;
use gpui::{Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px};
use std::time::{Duration, Instant};

const SAMPLE_DURATION: Duration = Duration::from_millis(500);

/// Measures intervals between root redraws, including UI work and vsync waits.
/// Sampling deliberately keeps the UI redrawing while the meter is enabled.
#[derive(Default)]
pub(super) struct FpsMeter {
    previous_frame: Option<Instant>,
    elapsed: Duration,
    frames: u32,
    slowest_frame: Duration,
    reading: Option<FrameReading>,
}

#[derive(Clone, Copy)]
struct FrameReading {
    fps: f64,
    average_ms: f64,
    slowest_ms: f64,
}

impl FpsMeter {
    pub(super) fn record_frame(&mut self, now: Instant) {
        let Some(previous) = self.previous_frame.replace(now) else {
            return;
        };
        let interval = now.duration_since(previous);
        self.elapsed += interval;
        self.frames += 1;
        self.slowest_frame = self.slowest_frame.max(interval);

        if self.elapsed >= SAMPLE_DURATION {
            let seconds = self.elapsed.as_secs_f64();
            self.reading = Some(FrameReading {
                fps: f64::from(self.frames) / seconds,
                average_ms: seconds * 1_000.0 / f64::from(self.frames),
                slowest_ms: self.slowest_frame.as_secs_f64() * 1_000.0,
            });
            self.elapsed = Duration::ZERO;
            self.frames = 0;
            self.slowest_frame = Duration::ZERO;
        }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn overlay(&self, palette: &'static theme::ThemePalette) -> impl IntoElement {
        let text = self.reading.map_or_else(
            || "FPS sampling...".to_string(),
            |reading| {
                format!(
                    "{:.0} FPS  |  {:.1} ms avg  |  {:.1} ms max",
                    reading.fps, reading.average_ms, reading.slowest_ms
                )
            },
        );
        div()
            .id("fps-meter")
            .absolute()
            .bottom(px(8.0))
            .left(px(8.0))
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(theme::RADIUS_SM))
            .border_1()
            .border_color(color(palette.fill_selected))
            .bg(color(palette.canvas))
            .text_color(color(palette.text_primary))
            .text_size(px(12.0))
            .flex()
            .flex_col()
            .child(text)
            .child(
                div()
                    .text_color(color(palette.text_muted))
                    .child("UI redraws · F12 to hide"),
            )
    }
}

impl ScissorsRoot {
    pub(super) fn toggle_fps_meter(
        &mut self,
        _: &ToggleFpsMeter,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fps_meter = if self.fps_meter.is_some() {
            None
        } else {
            Some(FpsMeter::default())
        };
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_frame_rate_and_reports_a_stall() {
        let mut meter = FpsMeter::default();
        let start = Instant::now();
        meter.record_frame(start);
        for frame in 1..=25 {
            meter.record_frame(start + Duration::from_millis(frame * 16));
        }
        assert!(meter.reading.is_none());
        meter.record_frame(start + Duration::from_millis(500));
        let reading = meter.reading.expect("half-second sample should be ready");
        assert!((reading.fps - 52.0).abs() < 0.001);
        assert!((reading.average_ms - 500.0 / 26.0).abs() < 0.001);
        assert!((reading.slowest_ms - 100.0).abs() < 0.001);
    }

    #[test]
    fn reset_excludes_time_spent_inactive() {
        let mut meter = FpsMeter::default();
        let start = Instant::now();
        meter.record_frame(start);
        meter.record_frame(start + Duration::from_millis(250));
        meter.reset();
        let resumed = start + Duration::from_secs(30);
        meter.record_frame(resumed);
        meter.record_frame(resumed + Duration::from_millis(250));
        meter.record_frame(resumed + Duration::from_millis(500));
        let reading = meter.reading.expect("new sample should be ready");
        assert!((reading.fps - 4.0).abs() < 0.001);
        assert!((reading.slowest_ms - 250.0).abs() < 0.001);
    }
}
