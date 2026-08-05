//! NLLB model metadata + on-disk path resolution.
use std::path::{Path, PathBuf};

pub const NLLB_MODEL_ID: &str = "nllb-200-distilled-600M-int8";

pub fn model_dir(models_root: &Path) -> PathBuf {
    models_root.join(NLLB_MODEL_ID)
}

pub fn is_downloaded(models_root: &Path) -> bool {
    model_dir(models_root).join("model.bin").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn model_dir_appends_id() {
        let root = PathBuf::from("/tmp/models");
        assert_eq!(
            model_dir(&root),
            PathBuf::from("/tmp/models/nllb-200-distilled-600M-int8")
        );
    }

    #[test]
    fn not_downloaded_when_missing() {
        assert!(!is_downloaded(&PathBuf::from(
            "/tmp/definitely-not-here-xyz"
        )));
    }
}
