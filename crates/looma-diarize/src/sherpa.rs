//! sherpa-onnx sidecar diarization: pyannote segmentation + speaker
//! embedding + clustering, all on CPU, all local — on every tier (§6.3).

use std::path::{Path, PathBuf};

use looma_core::SpeakerTurn;

use crate::{DiarizationEngine, DiarizeError, DiarizeOptions, Result};

pub struct SherpaDiarizeEngine {
    /// Path to sherpa-onnx-offline-speaker-diarization(.exe).
    pub exe: PathBuf,
    /// pyannote segmentation model (model.onnx).
    pub segmentation_model: PathBuf,
    /// Speaker embedding model (CAM++ ONNX).
    pub embedding_model: PathBuf,
    pub threads: usize,
}

#[async_trait::async_trait]
impl DiarizationEngine for SherpaDiarizeEngine {
    fn id(&self) -> &'static str {
        "sherpa-onnx"
    }

    async fn diarize(&self, wav_path: &Path, opts: &DiarizeOptions) -> Result<Vec<SpeakerTurn>> {
        for (what, p) in [
            ("segmentation model", &self.segmentation_model),
            ("embedding model", &self.embedding_model),
        ] {
            if !p.exists() {
                return Err(DiarizeError::ModelMissing(format!(
                    "{what}: {}",
                    p.display()
                )));
            }
        }
        if !wav_path.exists() {
            return Err(DiarizeError::BadAudio(wav_path.display().to_string()));
        }

        let mut cmd = tokio::process::Command::new(&self.exe);
        cmd.arg(format!(
            "--segmentation.pyannote-model={}",
            self.segmentation_model.display()
        ))
        .arg(format!(
            "--embedding.model={}",
            self.embedding_model.display()
        ))
        .arg(format!(
            "--segmentation.num-threads={}",
            self.threads.max(1)
        ))
        .arg(format!("--embedding.num-threads={}", self.threads.max(1)));
        if let Some(n) = opts.num_speakers {
            cmd.arg(format!("--clustering.num-clusters={n}"));
        }
        cmd.arg(wav_path);
        #[cfg(windows)]
        {
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }

        let output = cmd
            .output()
            .await
            .map_err(|e| DiarizeError::Engine(format!("failed to launch sherpa-onnx: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DiarizeError::Engine(format!(
                "sherpa-onnx exited with {}: {}",
                output.status,
                stderr.chars().take(500).collect::<String>()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_diarization_output(&stdout, &opts.speaker_key_prefix))
    }
}

/// Parse lines shaped `0.318 -- 6.865 speaker_00` (sherpa prints config and
/// progress around them; everything non-matching is ignored).
pub fn parse_diarization_output(output: &str, key_prefix: &str) -> Vec<SpeakerTurn> {
    let mut turns = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        let Some((times, speaker)) = line.rsplit_once(' ') else {
            continue;
        };
        let Some(num) = speaker.strip_prefix("speaker_") else {
            continue;
        };
        let Ok(idx) = num.parse::<u32>() else {
            continue;
        };
        let Some((start, end)) = times.trim().split_once("--") else {
            continue;
        };
        let (Ok(start_s), Ok(end_s)) = (start.trim().parse::<f64>(), end.trim().parse::<f64>())
        else {
            continue;
        };
        turns.push(SpeakerTurn {
            speaker_key: format!("{key_prefix}_{idx}"),
            start_ms: (start_s * 1000.0) as u64,
            end_ms: (end_s * 1000.0) as u64,
        });
    }
    turns.sort_by_key(|t| t.start_ms);
    turns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_turn_lines_ignoring_noise() {
        let out = "\
progress 100.00%
Duration : 27.540 s
OfflineSpeakerDiarizationConfig(...)
Started
0.031 -- 1.347 speaker_00
5.465 -- 6.342 speaker_01
2.174 -- 4.655 speaker_00
";
        let turns = parse_diarization_output(out, "spk");
        assert_eq!(turns.len(), 3);
        // sorted by start
        assert_eq!(turns[0].speaker_key, "spk_0");
        assert_eq!(turns[0].start_ms, 31);
        assert_eq!(turns[1].start_ms, 2174);
        assert_eq!(turns[2].speaker_key, "spk_1");
        assert_eq!(turns[2].end_ms, 6342);
    }

    #[test]
    fn empty_output_gives_no_turns() {
        assert!(parse_diarization_output("no matches here", "spk").is_empty());
    }
}
