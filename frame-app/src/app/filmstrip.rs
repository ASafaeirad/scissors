//! Cached video thumbnail strips, decoded away from the UI thread.
use super::decode_cache::DecodeKey;
use super::*;
use std::process::Command;

const FRAME_WIDTH: u32 = 128;
const FRAME_HEIGHT: u32 = 72;
const MAX_FRAMES: u32 = 32;
const MIN_SECONDS_PER_FRAME: f64 = 0.5;

/// Evenly spaced source frames, stored as the frames of one BGRA image.
#[derive(PartialEq)]
pub(super) struct Filmstrip {
    image: Arc<RenderImage>,
    frames: usize,
    seconds_per_frame: f64,
}

impl std::fmt::Debug for Filmstrip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Filmstrip")
            .field("image", &self.image.id)
            .field("frames", &self.frames)
            .field("seconds_per_frame", &self.seconds_per_frame)
            .finish()
    }
}

pub(super) fn request(
    path: &str,
    duration: f64,
    cx: &mut Context<FrameRoot>,
) -> Option<Arc<Filmstrip>> {
    super::decode_cache::request(path, None, duration, 16, decode, cx)
}

fn decode(key: &DecodeKey) -> Option<Filmstrip> {
    let duration = key.duration();
    let count =
        crate::numeric::f64_to_u32((duration / MIN_SECONDS_PER_FRAME).ceil()).clamp(1, MAX_FRAMES);
    let seconds_per_frame = duration / f64::from(count);
    // Each frame seeks on its own, so long sources cost no more than short ones.
    // The first frame doubles as a check that the source has a decodable video stream.
    let first = frame(&key.path, 0.5 * seconds_per_frame)?;
    let mut previous = first.clone();
    let frames: smallvec::SmallVec<[image::Frame; 1]> = std::iter::once(first)
        .chain((1..count).map(|index| {
            // A failed seek, usually past the last frame, repeats the frame before it.
            if let Some(frame) = frame(&key.path, (f64::from(index) + 0.5) * seconds_per_frame) {
                previous = frame;
            }
            previous.clone()
        }))
        .collect();
    Some(Filmstrip {
        frames: frames.len(),
        image: Arc::new(RenderImage::new(frames)),
        seconds_per_frame,
    })
}

/// Decode one frame at `time`, scaled to fill a fixed tile. GPUI uploads BGRA pixels.
fn frame(path: &str, time: f64) -> Option<image::Frame> {
    let output = Command::new(crate::runtime_binaries::ffmpeg_executable())
        .args([
            "-v",
            "error",
            "-nostdin",
            "-ss",
            &format!("{time:.3}"),
            "-i",
        ])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            "-vf",
            &format!(
                "scale={FRAME_WIDTH}:{FRAME_HEIGHT}:force_original_aspect_ratio=increase,\
                 crop={FRAME_WIDTH}:{FRAME_HEIGHT}"
            ),
            "-threads",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "bgra",
            "pipe:1",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    image::RgbaImage::from_raw(FRAME_WIDTH, FRAME_HEIGHT, output.stdout).map(image::Frame::new)
}

impl Filmstrip {
    fn frame_at(&self, time: f64) -> usize {
        usize::try_from(crate::numeric::f64_to_u64(
            (time / self.seconds_per_frame).floor().max(0.0),
        ))
        .unwrap_or(usize::MAX)
        .min(self.frames.saturating_sub(1))
    }
}

/// Tiles thumbnails at their natural aspect ratio. Each tile shows the source frame at its
/// center, after the source ranges concatenate like the waveform's.
/// The canvas is absolutely positioned across its parent; callers set its top and bottom.
pub(super) fn element(filmstrip: Arc<Filmstrip>, ranges: Vec<(f64, f64)>) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let total: f64 = ranges.iter().map(|(start, end)| end - start).sum();
            let width = bounds.size.width.as_f32();
            let height = bounds.size.height.as_f32();
            if total <= 0.0 || width <= 0.0 || height <= 0.0 {
                return;
            }
            let tile = height * crate::numeric::u32_to_f32(FRAME_WIDTH)
                / crate::numeric::u32_to_f32(FRAME_HEIGHT);
            let visible = bounds.intersect(&window.content_mask().bounds);
            let first_tile = crate::numeric::f64_to_u32(f64::from(
                ((visible.left() - bounds.left()).as_f32() / tile)
                    .floor()
                    .max(0.0),
            ));
            let last_tile = crate::numeric::f64_to_u32(f64::from(
                (width.min((visible.right() - bounds.left()).as_f32()) / tile).ceil(),
            ));
            // The trailing tile is cut off at the strip's edge rather than squeezed.
            window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                for index in first_tile..last_tile {
                    let x = crate::numeric::u32_to_f32(index) * tile;
                    let center = f64::from(((x + tile / 2.0).min(width)) / width) * total;
                    let frame =
                        filmstrip.frame_at(super::decode_cache::source_time(&ranges, center));
                    window
                        .paint_image(
                            Bounds::new(
                                point(bounds.left() + px(x), bounds.top()),
                                size(px(tile), px(height)),
                            ),
                            gpui::Corners::default(),
                            filmstrip.image.clone(),
                            frame,
                            false,
                        )
                        .ok();
                }
            });
        },
    )
    .absolute()
    .left_0()
    .right_0()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_at_clamps_to_the_decoded_frames() {
        let image = image::RgbaImage::new(1, 1);
        let filmstrip = Filmstrip {
            image: Arc::new(RenderImage::new(smallvec::smallvec![
                image::Frame::new(image.clone()),
                image::Frame::new(image.clone()),
                image::Frame::new(image),
            ])),
            frames: 3,
            seconds_per_frame: 2.0,
        };
        assert_eq!(filmstrip.frame_at(-1.0), 0);
        assert_eq!(filmstrip.frame_at(1.9), 0);
        assert_eq!(filmstrip.frame_at(2.0), 1);
        assert_eq!(filmstrip.frame_at(5.9), 2);
        assert_eq!(filmstrip.frame_at(60.0), 2);
    }

    #[test]
    #[ignore = "requires FFmpeg; run with --ignored"]
    fn decodes_evenly_spaced_bgra_frames_and_rejects_audio_only_sources() {
        let dir = std::env::temp_dir();
        let video = dir.join(format!("frame-filmstrip-{}.mp4", std::process::id()));
        let audio = dir.join(format!("frame-filmstrip-{}.wav", std::process::id()));
        for (input, path) in [
            // Red for the first second, then blue.
            (
                "color=c=red:s=320x180:d=2,geq=r='if(lt(T,1),255,0)':g=0:b='if(lt(T,1),0,255)'",
                &video,
            ),
            ("sine=d=1", &audio),
        ] {
            let status = Command::new(crate::runtime_binaries::ffmpeg_executable())
                .args(["-v", "error", "-nostdin", "-y", "-f", "lavfi", "-i", input])
                .arg(path)
                .status()
                .expect("generate filmstrip fixture");
            assert!(status.success());
        }
        let key = |path: &std::path::Path| DecodeKey {
            path: path.to_string_lossy().into_owned(),
            stream: None,
            duration_millis: 2000,
        };
        let filmstrip = decode(&key(&video)).expect("decode filmstrip");
        let audio_only = decode(&key(&audio));
        std::fs::remove_file(video).unwrap();
        std::fs::remove_file(audio).unwrap();

        assert_eq!(filmstrip.frames, 4);
        assert_eq!(filmstrip.image.size(0).width, gpui::DevicePixels(128));
        assert_eq!(filmstrip.image.size(0).height, gpui::DevicePixels(72));
        let first = &filmstrip.image.as_bytes(0).unwrap()[..4];
        let last = &filmstrip.image.as_bytes(3).unwrap()[..4];
        assert!(
            first[2] > 200 && first[0] < 60,
            "first frame is red: {first:?}"
        );
        assert!(
            last[0] > 200 && last[2] < 60,
            "last frame is blue: {last:?}"
        );
        assert!(audio_only.is_none());
    }
}
