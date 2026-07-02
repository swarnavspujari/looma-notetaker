//! ffmpeg-sidecar `ScreenRecorder` (Windows gdigrab). Full screen, a single
//! window by title, or a fixed region. Stopped gracefully by sending `q` on
//! stdin so ffmpeg finalizes the MP4 moov atom.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use crate::{CaptureTarget, Result, ScreenError, ScreenRecorder, ScreenSession};

pub struct FfmpegScreenRecorder {
    pub exe: PathBuf,
    pub framerate: u32,
}

impl FfmpegScreenRecorder {
    pub fn new(exe: PathBuf) -> Self {
        Self { exe, framerate: 10 }
    }
}

/// Build the gdigrab input arguments for a capture target.
pub fn gdigrab_args(target: &CaptureTarget, framerate: u32) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-f".into(),
        "gdigrab".into(),
        "-framerate".into(),
        framerate.to_string(),
    ];
    match target {
        CaptureTarget::FullScreen => {
            args.extend(["-i".into(), "desktop".into()]);
        }
        CaptureTarget::Window { title } => {
            args.extend(["-i".into(), format!("title={title}")]);
        }
        CaptureTarget::Region {
            x,
            y,
            width,
            height,
        } => {
            // gdigrab requires even dimensions for yuv420p; round down
            let w = width & !1;
            let h = height & !1;
            args.extend([
                "-offset_x".into(),
                x.to_string(),
                "-offset_y".into(),
                y.to_string(),
                "-video_size".into(),
                format!("{w}x{h}"),
                "-i".into(),
                "desktop".into(),
            ]);
        }
    }
    args
}

impl ScreenRecorder for FfmpegScreenRecorder {
    fn is_available(&self) -> bool {
        self.exe.exists()
    }

    fn start(&self, target: CaptureTarget, out_path: &Path) -> Result<Box<dyn ScreenSession>> {
        if !self.exe.exists() {
            return Err(ScreenError::Unavailable(format!(
                "ffmpeg not found at {}",
                self.exe.display()
            )));
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut cmd = Command::new(&self.exe);
        cmd.args(gdigrab_args(&target, self.framerate))
            .args([
                "-c:v",
                "libx264",
                // ultrafast + a 1080p cap keep encoding realtime even for
                // high-DPI screens on laptop CPUs; x264 falling behind would
                // silently compress the recording's timeline.
                "-preset",
                "ultrafast",
                "-crf",
                "28",
                "-pix_fmt",
                "yuv420p",
                // cap width at 1920 and force even dimensions for yuv420p
                "-vf",
                "scale='trunc(min(1920,iw)/2)*2':-2",
                "-y",
            ])
            .arg(out_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let child = cmd
            .spawn()
            .map_err(|e| ScreenError::Capture(format!("failed to launch ffmpeg: {e}")))?;

        Ok(Box::new(FfmpegSession {
            child,
            out_path: out_path.to_path_buf(),
            started: Instant::now(),
        }))
    }
}

struct FfmpegSession {
    child: Child,
    out_path: PathBuf,
    started: Instant,
}

impl ScreenSession for FfmpegSession {
    fn stop(mut self: Box<Self>) -> Result<PathBuf> {
        // graceful: 'q' lets ffmpeg finalize the container
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q");
            let _ = stdin.flush();
        }
        drop(self.child.stdin.take());

        // give it a few seconds, then force-kill as a last resort
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() && !self.out_path.exists() {
                        let mut stderr = String::new();
                        if let Some(mut e) = self.child.stderr.take() {
                            use std::io::Read;
                            let _ = e.read_to_string(&mut stderr);
                        }
                        return Err(ScreenError::Capture(format!(
                            "ffmpeg exited with {status}: {}",
                            stderr
                                .chars()
                                .rev()
                                .take(400)
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect::<String>()
                        )));
                    }
                    break;
                }
                Ok(None) if Instant::now() > deadline => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
                Err(e) => return Err(ScreenError::Capture(e.to_string())),
            }
        }

        if !self.out_path.exists() {
            return Err(ScreenError::Capture(
                "ffmpeg produced no output file".into(),
            ));
        }
        Ok(self.out_path)
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdigrab_args_for_each_target() {
        let full = gdigrab_args(&CaptureTarget::FullScreen, 15);
        assert!(full.windows(2).any(|w| w == ["-i", "desktop"]));

        let win = gdigrab_args(
            &CaptureTarget::Window {
                title: "Budget – Zoom".into(),
            },
            15,
        );
        assert!(win.iter().any(|a| a == "title=Budget – Zoom"));

        let region = gdigrab_args(
            &CaptureTarget::Region {
                x: 10,
                y: 20,
                width: 801, // odd → rounded down
                height: 600,
            },
            30,
        );
        assert!(region.windows(2).any(|w| w == ["-video_size", "800x600"]));
        assert!(region.windows(2).any(|w| w == ["-offset_x", "10"]));
        assert!(region.contains(&"30".to_string()));
    }
}
