use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, StorageErrorCode};

const RECORDINGS_DIR: &str = "recordings";

/// Delete a recording created by WhisperDesk after its database row is gone.
///
/// Only direct files under the app-owned recordings directory are eligible.
/// User-selected imports and files still referenced by a transcript or another
/// recording row are always preserved.
pub fn remove_owned_recording_if_unreferenced(
    app_data_dir: &Path,
    conn: &Connection,
    audio_path: &str,
) -> Result<bool, AppError> {
    let root = app_data_dir.join(RECORDINGS_DIR);
    let path = Path::new(audio_path);
    if path.parent() != Some(root.as_path()) || path.file_name().is_none() {
        return Ok(false);
    }

    let Some(canonical_root) = canonical_owned_recordings_directory(app_data_dir) else {
        return Ok(false);
    };
    let Some(canonical_path) = canonical_owned_recording_file(&canonical_root, path) else {
        return Ok(false);
    };

    let referenced = conn
        .query_row(
            "SELECT
               EXISTS(SELECT 1 FROM transcripts WHERE audio_path = ?1)
               OR EXISTS(SELECT 1 FROM recordings WHERE audio_path = ?1 OR system_audio_path = ?1)",
            [audio_path],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| {
            storage_error(format!("Failed to inspect recording references: {error}"))
        })?;
    if referenced {
        return Ok(false);
    }

    match std::fs::remove_file(&canonical_path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(storage_error(format!(
            "Failed to remove recording '{}': {error}",
            canonical_path.display()
        ))),
    }
}

fn canonical_owned_recordings_directory(app_data_dir: &Path) -> Option<std::path::PathBuf> {
    let canonical_app_data = std::fs::canonicalize(app_data_dir).ok()?;
    let candidate = app_data_dir.join(RECORDINGS_DIR);
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    if !metadata.file_type().is_dir() {
        return None;
    }

    let canonical_candidate = std::fs::canonicalize(candidate).ok()?;
    (canonical_candidate == canonical_app_data.join(RECORDINGS_DIR)).then_some(canonical_candidate)
}

fn canonical_owned_recording_file(
    canonical_root: &Path,
    path: &Path,
) -> Option<std::path::PathBuf> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() {
        return None;
    }

    let canonical_path = std::fs::canonicalize(path).ok()?;
    (canonical_path.parent() == Some(canonical_root)).then_some(canonical_path)
}

fn storage_error(message: String) -> AppError {
    AppError::StorageError {
        code: StorageErrorCode::IoError,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn storage_tables() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE transcripts (id TEXT PRIMARY KEY, audio_path TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE recordings (id TEXT PRIMARY KEY, audio_path TEXT, system_audio_path TEXT)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn cleanup_is_scoped_and_preserves_referenced_recordings() {
        let temp = tempfile::tempdir().unwrap();
        let recordings = temp.path().join(RECORDINGS_DIR);
        std::fs::create_dir_all(&recordings).unwrap();
        let owned = recordings.join("recording.wav");
        let external = temp.path().join("user.wav");
        std::fs::write(&owned, b"owned").unwrap();
        std::fs::write(&external, b"external").unwrap();

        let conn = storage_tables();
        let owned_string = owned.to_string_lossy().into_owned();
        conn.execute(
            "INSERT INTO transcripts (id, audio_path) VALUES ('transcript', ?1)",
            [&owned_string],
        )
        .unwrap();

        assert!(
            !remove_owned_recording_if_unreferenced(temp.path(), &conn, &owned_string).unwrap()
        );
        conn.execute("DELETE FROM transcripts", []).unwrap();
        assert!(remove_owned_recording_if_unreferenced(temp.path(), &conn, &owned_string).unwrap());
        assert!(!owned.exists());

        assert!(!remove_owned_recording_if_unreferenced(
            temp.path(),
            &conn,
            external.to_string_lossy().as_ref()
        )
        .unwrap());
        assert!(external.exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_rejects_a_linked_recordings_root() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let external_root = temp.path().join("external-recordings");
        std::fs::create_dir_all(&external_root).unwrap();
        let victim = external_root.join("victim.wav");
        std::fs::write(&victim, b"user audio").unwrap();
        symlink(&external_root, app_data.join(RECORDINGS_DIR)).unwrap();

        let conn = storage_tables();
        let linked_path = app_data.join(RECORDINGS_DIR).join("victim.wav");
        assert!(!remove_owned_recording_if_unreferenced(
            &app_data,
            &conn,
            linked_path.to_string_lossy().as_ref()
        )
        .unwrap());
        assert!(victim.exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_rejects_a_linked_recording_file() {
        let temp = tempfile::tempdir().unwrap();
        let recordings = temp.path().join(RECORDINGS_DIR);
        std::fs::create_dir_all(&recordings).unwrap();
        let victim = temp.path().join("user-audio.wav");
        std::fs::write(&victim, b"user audio").unwrap();
        let linked_audio = recordings.join("recording.wav");
        symlink(&victim, &linked_audio).unwrap();

        let conn = storage_tables();
        assert!(!remove_owned_recording_if_unreferenced(
            temp.path(),
            &conn,
            linked_audio.to_string_lossy().as_ref()
        )
        .unwrap());
        assert!(victim.exists());
        assert!(linked_audio
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
