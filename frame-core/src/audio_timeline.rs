//! Audio clips mixed on the source timeline, shared by playback and export.

use crate::{filters::build_audio_filters, types::ConversionConfig};

#[must_use]
pub fn has_external_audio(config: &ConversionConfig) -> bool {
    config
        .external_audio_tracks
        .iter()
        .any(|track| !track.muted)
}

/// Add inputs without seeking them. The graph places and seeks each clip.
pub fn add_inputs(args: &mut Vec<String>, config: &ConversionConfig) {
    for track in &config.external_audio_tracks {
        args.extend(["-i".to_string(), track.path.clone()]);
    }
}

/// Build a finite mix, measured in source seconds before removed segments.
#[must_use]
pub fn mix_graph(
    config: &ConversionConfig,
    first_input: usize,
    source_tracks: &[u32],
    start: f64,
    duration: f64,
) -> String {
    let mut chains = Vec::new();
    let mut labels = Vec::new();
    for index in source_tracks {
        let label = format!("frame_audio_{}", labels.len());
        chains.push(format!(
            "[0:{index}]asetpts=PTS-STARTPTS,apad,atrim=duration={duration:.6}[{label}]"
        ));
        labels.push(format!("[{label}]"));
    }
    for (index, track) in config.external_audio_tracks.iter().enumerate() {
        if track.muted {
            continue;
        }
        let label = format!("frame_audio_{}", labels.len());
        let clip_duration = std::time::Duration::from_millis(track.duration_millis).as_secs_f64();
        chains.push(format!(
            "[{}:a:0]atrim=duration={clip_duration:.6},asetpts=PTS-STARTPTS,adelay={}:all=1,apad,atrim=start={start:.6}:duration={duration:.6},asetpts=PTS-STARTPTS[{label}]",
            first_input + index, track.offset_millis,
        ));
        // The delay includes the clip's offset on the untrimmed source timeline.
        labels.push(format!("[{label}]"));
    }
    let mut effects = build_audio_filters(config);
    effects.insert(
        0,
        format!("amix=inputs={}:duration=longest:normalize=0", labels.len()),
    );
    chains.push(format!(
        "{}{}[frame_audio_out]",
        labels.join(""),
        effects.join(",")
    ));
    chains.join(";")
}

/// Merge into an existing video/overlay graph rather than add a second graph.
pub fn add_graph(args: &mut Vec<String>, graph: String) {
    if let Some(index) = args.iter().position(|arg| arg == "-filter_complex") {
        args[index + 1].push(';');
        args[index + 1].push_str(&graph);
    } else {
        args.extend(["-filter_complex".to_string(), graph]);
    }
    args.extend(["-map".to_string(), "[frame_audio_out]".to_string()]);
}
