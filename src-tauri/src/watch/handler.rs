use std::path::Path;

/// Audio file extensions supported for watch folder auto-detection.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "opus", "m4a", "aac", "wma", "webm", "mp4", "mov",
];

/// Check if a file path has a recognized audio extension.
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_audio_file_detection() {
        assert!(is_audio_file(&PathBuf::from("test.mp3")));
        assert!(is_audio_file(&PathBuf::from("test.WAV")));
        assert!(is_audio_file(&PathBuf::from("test.flac")));
        assert!(is_audio_file(&PathBuf::from("test.m4a")));
        assert!(is_audio_file(&PathBuf::from("/path/to/recording.opus")));
        assert!(!is_audio_file(&PathBuf::from("test.txt")));
        assert!(!is_audio_file(&PathBuf::from("test.pdf")));
        assert!(!is_audio_file(&PathBuf::from("test")));
    }
}
