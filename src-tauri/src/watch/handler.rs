use std::path::Path;
use std::time::SystemTime;
use tokio::time::{sleep, Duration};

/// Audio file extensions supported for watch folder auto-detection.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "m4a", "flac", "ogg", "oga"];

const STABILITY_POLL_INTERVAL: Duration = Duration::from_millis(750);
const REQUIRED_UNCHANGED_SAMPLES: usize = 4;
const MAX_STABILITY_SAMPLES: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Default)]
struct FileStabilityTracker {
    previous: Option<FileFingerprint>,
    unchanged_samples: usize,
}

impl FileStabilityTracker {
    fn observe(&mut self, fingerprint: FileFingerprint) -> bool {
        if fingerprint.len == 0 {
            self.previous = Some(fingerprint);
            self.unchanged_samples = 0;
            return false;
        }

        if self.previous == Some(fingerprint) {
            self.unchanged_samples += 1;
        } else {
            self.previous = Some(fingerprint);
            self.unchanged_samples = 0;
        }

        self.unchanged_samples >= REQUIRED_UNCHANGED_SAMPLES
    }
}

/// Check if a file path has a recognized audio extension.
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Wait until a newly-created file has stopped changing before it is handed
/// to the decoder. Finder and network copies create the destination before
/// all bytes are written; decoding directly from the Create event can read a
/// truncated file. The five-minute bound prevents abandoned writes from
/// retaining a task forever.
pub async fn wait_until_file_stable(path: &Path) -> std::io::Result<bool> {
    let mut tracker = FileStabilityTracker::default();

    for sample in 0..MAX_STABILITY_SAMPLES {
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file() {
            return Ok(false);
        }

        if tracker.observe(FileFingerprint {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }) {
            return Ok(true);
        }

        if sample + 1 < MAX_STABILITY_SAMPLES {
            sleep(STABILITY_POLL_INTERVAL).await;
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;

    #[test]
    fn test_audio_file_detection() {
        assert!(is_audio_file(&PathBuf::from("test.mp3")));
        assert!(is_audio_file(&PathBuf::from("test.WAV")));
        assert!(is_audio_file(&PathBuf::from("test.flac")));
        assert!(is_audio_file(&PathBuf::from("test.m4a")));
        assert!(is_audio_file(&PathBuf::from("/path/to/recording.oga")));
        assert!(!is_audio_file(&PathBuf::from("test.opus")));
        assert!(!is_audio_file(&PathBuf::from("test.mp4")));
        assert!(!is_audio_file(&PathBuf::from("test.txt")));
        assert!(!is_audio_file(&PathBuf::from("test.pdf")));
        assert!(!is_audio_file(&PathBuf::from("test")));
    }

    #[test]
    fn stability_requires_consecutive_unchanged_nonempty_samples() {
        let fingerprint = FileFingerprint {
            len: 42,
            modified: Some(UNIX_EPOCH),
        };
        let mut tracker = FileStabilityTracker::default();

        assert!(!tracker.observe(fingerprint));
        assert!(!tracker.observe(fingerprint));
        assert!(!tracker.observe(fingerprint));
        assert!(!tracker.observe(fingerprint));
        assert!(tracker.observe(fingerprint));
        assert!(
            STABILITY_POLL_INTERVAL.saturating_mul(REQUIRED_UNCHANGED_SAMPLES as u32)
                >= Duration::from_secs(3)
        );
    }

    #[test]
    fn stability_resets_when_size_or_timestamp_changes() {
        let mut tracker = FileStabilityTracker::default();
        let first = FileFingerprint {
            len: 42,
            modified: Some(UNIX_EPOCH),
        };
        let changed = FileFingerprint {
            len: 84,
            modified: Some(UNIX_EPOCH + std::time::Duration::from_secs(1)),
        };

        assert!(!tracker.observe(first));
        assert!(!tracker.observe(first));
        assert!(!tracker.observe(changed));
        assert!(!tracker.observe(changed));
        assert!(!tracker.observe(changed));
        assert!(!tracker.observe(changed));
        assert!(tracker.observe(changed));
    }

    #[test]
    fn empty_files_never_become_stable() {
        let empty = FileFingerprint {
            len: 0,
            modified: Some(UNIX_EPOCH),
        };
        let mut tracker = FileStabilityTracker::default();

        for _ in 0..10 {
            assert!(!tracker.observe(empty));
        }
    }
}
