use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{AppError, ImportErrorCode, StorageErrorCode};

pub const ABANDONED_IMPORT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const YOUTUBE_IMPORT_DIR: &str = "imports/youtube";
const YOUTUBE_STAGING_DIR: &str = "imports/.staging/youtube";

pub fn youtube_import_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(YOUTUBE_IMPORT_DIR)
}

pub fn youtube_staging_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(YOUTUBE_STAGING_DIR)
}

/// Create and validate the two roots used by YouTube imports.
///
/// Each component below app data is created separately and checked before the
/// next component is touched. This prevents an existing link or junction from
/// redirecting downloads and their later cleanup outside app-owned storage.
pub fn prepare_youtube_storage(app_data_dir: &Path) -> Result<(), AppError> {
    ensure_owned_directory(app_data_dir, YOUTUBE_IMPORT_DIR)?;
    ensure_owned_directory(app_data_dir, YOUTUBE_STAGING_DIR)?;
    Ok(())
}

/// Copy a completed download into its durable, app-owned job directory.
///
/// The copy is first written to a partial file and then renamed so callers
/// never receive a path to a partially copied WAV. The staging source remains
/// in place until the caller has received a successful result.
pub fn promote_audio(
    source: &Path,
    staging_job_dir: &Path,
    durable_job_dir: &Path,
) -> Result<PathBuf, AppError> {
    if !source.is_file() {
        return Err(import_error(format!(
            "Downloaded audio file does not exist: {}",
            source.display()
        )));
    }

    let canonical_source = std::fs::canonicalize(source).map_err(|error| {
        import_error(format!(
            "Cannot resolve downloaded audio '{}': {error}",
            source.display()
        ))
    })?;
    let canonical_staging = std::fs::canonicalize(staging_job_dir).map_err(|error| {
        import_error(format!(
            "Cannot resolve YouTube staging directory '{}': {error}",
            staging_job_dir.display()
        ))
    })?;
    if canonical_source.parent() != Some(canonical_staging.as_path()) {
        return Err(import_error(format!(
            "Downloaded audio resolved outside its isolated staging directory: {}",
            source.display()
        )));
    }

    std::fs::create_dir_all(durable_job_dir).map_err(|error| {
        import_error(format!(
            "Cannot create durable YouTube import directory '{}': {error}",
            durable_job_dir.display()
        ))
    })?;

    let file_name = canonical_source
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| import_error("Downloaded audio file has no filename".into()))?;
    let destination = durable_job_dir.join(file_name);
    let partial = durable_job_dir.join(".audio.partial");

    if let Err(error) = std::fs::copy(&canonical_source, &partial) {
        let _ = std::fs::remove_file(&partial);
        return Err(import_error(format!(
            "Cannot store imported audio in '{}': {error}",
            durable_job_dir.display()
        )));
    }
    if let Err(error) = std::fs::rename(&partial, &destination) {
        let _ = std::fs::remove_file(&partial);
        return Err(import_error(format!(
            "Cannot finalize imported audio '{}': {error}",
            destination.display()
        )));
    }

    Ok(destination)
}

/// Remove old, interrupted per-job staging directories.
///
/// Only UUID-named direct children of a real, app-owned staging root are
/// eligible. Linked components below `app_data_dir` are rejected so cleanup
/// cannot be redirected into user-controlled storage.
pub fn cleanup_stale_staging(app_data_dir: &Path, max_age: Duration) -> usize {
    let Some(root) = canonical_owned_directory(app_data_dir, YOUTUBE_STAGING_DIR) else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_uuid_job_directory(&root, &path) || !entry_is_directory(&entry) {
            continue;
        }
        if !is_at_least_age(&entry, max_age) {
            continue;
        }
        let Some(canonical_job) = canonical_owned_job_directory(&root, &path) else {
            continue;
        };
        match std::fs::remove_dir_all(&canonical_job) {
            Ok(()) => removed += 1,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    ?canonical_job,
                    "failed to clean stale YouTube staging job"
                )
            }
        }
    }
    removed
}

/// Remove durable YouTube job directories that were never attached to a
/// transcript and have exceeded the grace period.
pub fn cleanup_abandoned_imports(
    app_data_dir: &Path,
    conn: &Connection,
    max_age: Duration,
) -> Result<usize, AppError> {
    let lexical_root = youtube_import_root(app_data_dir);
    let Some(root) = canonical_owned_directory(app_data_dir, YOUTUBE_IMPORT_DIR) else {
        return Ok(0);
    };
    let referenced_paths = referenced_audio_paths(conn)?;
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Ok(0);
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let job_dir = entry.path();
        if !is_uuid_job_directory(&root, &job_dir) || !entry_is_directory(&entry) {
            continue;
        }
        if !is_at_least_age(&entry, max_age)
            || referenced_paths
                .iter()
                .any(|path| referenced_path_belongs_to_job(path, &lexical_root, &root, &job_dir))
        {
            continue;
        }
        let Some(canonical_job) = canonical_owned_job_directory(&root, &job_dir) else {
            continue;
        };
        match std::fs::remove_dir_all(&canonical_job) {
            Ok(()) => removed += 1,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    ?canonical_job,
                    "failed to clean abandoned YouTube import"
                )
            }
        }
    }

    Ok(removed)
}

/// Remove one completed/failed staging job without following linked roots.
pub fn remove_owned_staging_job(app_data_dir: &Path, job_dir: &Path) -> Result<bool, AppError> {
    remove_owned_job_directory(app_data_dir, YOUTUBE_STAGING_DIR, job_dir)
}

/// Remove one incomplete durable import without following linked roots.
pub fn remove_owned_import_job(app_data_dir: &Path, job_dir: &Path) -> Result<bool, AppError> {
    remove_owned_job_directory(app_data_dir, YOUTUBE_IMPORT_DIR, job_dir)
}

/// Delete an app-owned YouTube audio file after its final transcript reference
/// has gone away. User-selected files and recording files are never eligible.
pub fn remove_owned_import_if_unreferenced(
    app_data_dir: &Path,
    conn: &Connection,
    audio_path: &str,
) -> Result<bool, AppError> {
    let root = youtube_import_root(app_data_dir);
    let path = Path::new(audio_path);
    if !is_owned_import_path(&root, path) || audio_path_is_referenced(conn, audio_path)? {
        return Ok(false);
    }

    let Some(canonical_root) = canonical_owned_directory(app_data_dir, YOUTUBE_IMPORT_DIR) else {
        return Ok(false);
    };
    let Some(canonical_path) = canonical_owned_import_file(&canonical_root, path) else {
        return Ok(false);
    };

    let removed = match std::fs::remove_file(&canonical_path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(storage_error(format!(
                "Failed to remove imported audio '{}': {error}",
                canonical_path.display()
            )))
        }
    };

    // Every import has its own job directory. Remove it only when empty; an
    // unexpected sibling is left untouched for later scoped cleanup.
    if let Some(job_dir) = canonical_path.parent() {
        match std::fs::remove_dir(job_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(
                    ?error,
                    ?job_dir,
                    "failed to remove empty YouTube import job"
                )
            }
        }
    }

    Ok(removed)
}

fn referenced_audio_paths(conn: &Connection) -> Result<Vec<PathBuf>, AppError> {
    let mut stmt = conn
        .prepare("SELECT audio_path FROM transcripts WHERE audio_path IS NOT NULL")
        .map_err(|error| storage_error(format!("Failed to inspect imported audio: {error}")))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| storage_error(format!("Failed to inspect imported audio: {error}")))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map(|paths| paths.into_iter().map(PathBuf::from).collect())
        .map_err(|error| storage_error(format!("Failed to inspect imported audio: {error}")))
}

fn referenced_path_belongs_to_job(
    path: &Path,
    lexical_root: &Path,
    canonical_root: &Path,
    canonical_job: &Path,
) -> bool {
    if path.starts_with(canonical_job)
        || std::fs::canonicalize(path)
            .ok()
            .is_some_and(|canonical| canonical.starts_with(canonical_job))
    {
        return true;
    }

    path.strip_prefix(lexical_root)
        .ok()
        .is_some_and(|relative| canonical_root.join(relative).starts_with(canonical_job))
}

fn audio_path_is_referenced(conn: &Connection, audio_path: &str) -> Result<bool, AppError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM transcripts WHERE audio_path = ?1)",
        [audio_path],
        |row| row.get::<_, bool>(0),
    )
    .map_err(|error| storage_error(format!("Failed to inspect imported audio: {error}")))
}

fn is_owned_import_path(root: &Path, path: &Path) -> bool {
    let Some(job_dir) = path.parent() else {
        return false;
    };
    path.file_name().is_some() && is_uuid_job_directory(root, job_dir)
}

fn canonical_owned_directory(app_data_dir: &Path, relative: &str) -> Option<PathBuf> {
    let canonical_app_data = std::fs::canonicalize(app_data_dir).ok()?;
    let candidate = app_data_dir.join(relative);
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    if !metadata.file_type().is_dir() {
        return None;
    }

    let canonical_candidate = std::fs::canonicalize(candidate).ok()?;
    (canonical_candidate == canonical_app_data.join(relative)).then_some(canonical_candidate)
}

fn ensure_owned_directory(app_data_dir: &Path, relative: &str) -> Result<PathBuf, AppError> {
    let canonical_app_data = std::fs::canonicalize(app_data_dir).map_err(|error| {
        storage_error(format!(
            "Cannot resolve app data directory '{}': {error}",
            app_data_dir.display()
        ))
    })?;
    let mut candidate = app_data_dir.to_path_buf();
    let mut expected = canonical_app_data;

    for component in Path::new(relative).components() {
        let std::path::Component::Normal(name) = component else {
            return Err(storage_error("Invalid app-owned import directory".into()));
        };
        candidate.push(name);
        expected.push(name);

        match std::fs::create_dir(&candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(storage_error(format!(
                    "Cannot create app-owned import directory '{}': {error}",
                    candidate.display()
                )))
            }
        }

        let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
            storage_error(format!(
                "Cannot inspect app-owned import directory '{}': {error}",
                candidate.display()
            ))
        })?;
        let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|error| {
            storage_error(format!(
                "Cannot resolve app-owned import directory '{}': {error}",
                candidate.display()
            ))
        })?;
        if !metadata.file_type().is_dir() || canonical_candidate != expected {
            return Err(storage_error(format!(
                "App-owned import directory is linked outside app data: {}",
                candidate.display()
            )));
        }
    }

    Ok(expected)
}

fn remove_owned_job_directory(
    app_data_dir: &Path,
    relative: &str,
    job_dir: &Path,
) -> Result<bool, AppError> {
    let Some(canonical_root) = canonical_owned_directory(app_data_dir, relative) else {
        return Ok(false);
    };
    let Some(canonical_job) = canonical_owned_job_directory(&canonical_root, job_dir) else {
        return Ok(false);
    };

    match std::fs::remove_dir_all(&canonical_job) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(storage_error(format!(
            "Failed to remove app-owned import job '{}': {error}",
            canonical_job.display()
        ))),
    }
}

fn canonical_owned_job_directory(canonical_root: &Path, job_dir: &Path) -> Option<PathBuf> {
    let metadata = std::fs::symlink_metadata(job_dir).ok()?;
    if !metadata.file_type().is_dir() {
        return None;
    }

    let canonical_job = std::fs::canonicalize(job_dir).ok()?;
    is_uuid_job_directory(canonical_root, &canonical_job).then_some(canonical_job)
}

fn canonical_owned_import_file(canonical_root: &Path, path: &Path) -> Option<PathBuf> {
    let job_dir = path.parent()?;
    let canonical_job = canonical_owned_job_directory(canonical_root, job_dir)?;

    let file_metadata = std::fs::symlink_metadata(path).ok()?;
    if !file_metadata.file_type().is_file() {
        return None;
    }

    let canonical_path = std::fs::canonicalize(path).ok()?;
    (canonical_path.parent() == Some(canonical_job.as_path())).then_some(canonical_path)
}

fn is_uuid_job_directory(root: &Path, path: &Path) -> bool {
    path.parent() == Some(root)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| uuid::Uuid::parse_str(name).is_ok())
}

fn entry_is_directory(entry: &std::fs::DirEntry) -> bool {
    entry
        .file_type()
        .map(|file_type| file_type.is_dir())
        .unwrap_or(false)
}

fn is_at_least_age(entry: &std::fs::DirEntry, max_age: Duration) -> bool {
    entry
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= max_age)
}

fn import_error(message: String) -> AppError {
    AppError::ImportError {
        code: ImportErrorCode::DownloadFailed,
        message,
    }
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

    fn transcripts_table() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE transcripts (id TEXT PRIMARY KEY, audio_path TEXT)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn promotes_audio_atomically_into_a_unique_job() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("download.wav");
        std::fs::write(&source, b"audio bytes").unwrap();
        let job_dir = temp.path().join(uuid::Uuid::new_v4().to_string());

        let promoted = promote_audio(&source, temp.path(), &job_dir).unwrap();

        assert_eq!(promoted, job_dir.join("download.wav"));
        assert_eq!(std::fs::read(promoted).unwrap(), b"audio bytes");
        assert!(
            source.exists(),
            "staging remains until the caller cleans it"
        );
        assert!(!job_dir.join(".audio.partial").exists());
    }

    #[test]
    fn promotion_rejects_audio_outside_the_staging_job() {
        let temp = tempfile::tempdir().unwrap();
        let staging = temp.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let outside = temp.path().join("outside.wav");
        std::fs::write(&outside, b"not this job").unwrap();
        let job_dir = temp.path().join(uuid::Uuid::new_v4().to_string());

        assert!(promote_audio(&outside, &staging, &job_dir).is_err());
        assert!(!job_dir.exists());
    }

    #[test]
    fn abandoned_cleanup_keeps_referenced_imports() {
        let temp = tempfile::tempdir().unwrap();
        let root = youtube_import_root(temp.path());
        let referenced_job = root.join(uuid::Uuid::new_v4().to_string());
        let abandoned_job = root.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&referenced_job).unwrap();
        std::fs::create_dir_all(&abandoned_job).unwrap();
        let referenced_audio = referenced_job.join("kept.wav");
        std::fs::write(&referenced_audio, b"kept").unwrap();
        std::fs::write(abandoned_job.join("old.wav"), b"old").unwrap();
        let conn = transcripts_table();
        conn.execute(
            "INSERT INTO transcripts (id, audio_path) VALUES ('kept', ?1)",
            [referenced_audio.to_string_lossy().as_ref()],
        )
        .unwrap();

        let count = cleanup_abandoned_imports(temp.path(), &conn, Duration::ZERO).unwrap();

        assert_eq!(count, 1);
        assert!(referenced_job.exists());
        assert!(!abandoned_job.exists());
    }

    #[test]
    fn permanent_cleanup_is_scoped_and_waits_for_final_reference() {
        let temp = tempfile::tempdir().unwrap();
        let root = youtube_import_root(temp.path());
        let job = root.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&job).unwrap();
        let owned_audio = job.join("audio.wav");
        std::fs::write(&owned_audio, b"owned").unwrap();
        let external_audio = temp.path().join("user-file.wav");
        std::fs::write(&external_audio, b"user").unwrap();
        let conn = transcripts_table();
        let owned = owned_audio.to_string_lossy().into_owned();
        conn.execute(
            "INSERT INTO transcripts (id, audio_path) VALUES ('one', ?1), ('two', ?1)",
            [&owned],
        )
        .unwrap();

        assert!(!remove_owned_import_if_unreferenced(temp.path(), &conn, &owned).unwrap());
        conn.execute("DELETE FROM transcripts WHERE id = 'one'", [])
            .unwrap();
        assert!(!remove_owned_import_if_unreferenced(temp.path(), &conn, &owned).unwrap());
        conn.execute("DELETE FROM transcripts WHERE id = 'two'", [])
            .unwrap();
        assert!(remove_owned_import_if_unreferenced(temp.path(), &conn, &owned).unwrap());
        assert!(!owned_audio.exists());
        assert!(!job.exists());

        let external = external_audio.to_string_lossy();
        assert!(!remove_owned_import_if_unreferenced(temp.path(), &conn, &external).unwrap());
        assert!(external_audio.exists());
    }

    #[test]
    fn staging_cleanup_only_removes_uuid_job_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = youtube_staging_root(temp.path());
        let job = root.join(uuid::Uuid::new_v4().to_string());
        let unrelated = root.join("keep-me");
        std::fs::create_dir_all(&job).unwrap();
        std::fs::create_dir_all(&unrelated).unwrap();

        assert_eq!(cleanup_stale_staging(temp.path(), Duration::ZERO), 1);
        assert!(!job.exists());
        assert!(unrelated.exists());
    }

    #[test]
    fn direct_uuid_job_cleanup_remains_supported() {
        let temp = tempfile::tempdir().unwrap();
        prepare_youtube_storage(temp.path()).unwrap();
        let job = youtube_staging_root(temp.path()).join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&job).unwrap();
        std::fs::write(job.join("partial.wav"), b"partial").unwrap();

        assert!(remove_owned_staging_job(temp.path(), &job).unwrap());
        assert!(!job.exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_rejects_a_linked_import_root() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(app_data.join("imports")).unwrap();

        let external_root = temp.path().join("external-imports");
        let external_job = external_root.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&external_job).unwrap();
        let victim = external_job.join("victim.wav");
        std::fs::write(&victim, b"user audio").unwrap();
        symlink(&external_root, youtube_import_root(&app_data)).unwrap();

        let conn = transcripts_table();
        assert!(prepare_youtube_storage(&app_data).is_err());
        assert_eq!(
            cleanup_abandoned_imports(&app_data, &conn, Duration::ZERO).unwrap(),
            0
        );
        let linked_job = youtube_import_root(&app_data).join(external_job.file_name().unwrap());
        let linked_path = linked_job.join("victim.wav");
        assert!(!remove_owned_import_job(&app_data, &linked_job).unwrap());
        assert!(!remove_owned_import_if_unreferenced(
            &app_data,
            &conn,
            linked_path.to_string_lossy().as_ref()
        )
        .unwrap());
        assert!(victim.exists());
    }

    #[cfg(unix)]
    #[test]
    fn permanent_cleanup_rejects_a_linked_job_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = youtube_import_root(temp.path());
        std::fs::create_dir_all(&root).unwrap();

        let external_job = temp.path().join("external-job");
        std::fs::create_dir_all(&external_job).unwrap();
        let victim = external_job.join("victim.wav");
        std::fs::write(&victim, b"user audio").unwrap();
        let linked_job = root.join(uuid::Uuid::new_v4().to_string());
        symlink(&external_job, &linked_job).unwrap();

        let conn = transcripts_table();
        let linked_path = linked_job.join("victim.wav");
        assert!(!remove_owned_import_job(temp.path(), &linked_job).unwrap());
        assert!(!remove_owned_import_if_unreferenced(
            temp.path(),
            &conn,
            linked_path.to_string_lossy().as_ref()
        )
        .unwrap());
        assert!(victim.exists());
        assert!(linked_job
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn permanent_cleanup_rejects_a_linked_audio_file() {
        let temp = tempfile::tempdir().unwrap();
        let job = youtube_import_root(temp.path()).join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&job).unwrap();

        let victim = temp.path().join("user-audio.wav");
        std::fs::write(&victim, b"user audio").unwrap();
        let linked_audio = job.join("audio.wav");
        symlink(&victim, &linked_audio).unwrap();

        let conn = transcripts_table();
        conn.execute(
            "INSERT INTO transcripts (id, audio_path) VALUES ('linked', ?1)",
            [linked_audio.to_string_lossy().as_ref()],
        )
        .unwrap();
        assert_eq!(
            cleanup_abandoned_imports(temp.path(), &conn, Duration::ZERO).unwrap(),
            0
        );
        conn.execute("DELETE FROM transcripts", []).unwrap();
        assert!(!remove_owned_import_if_unreferenced(
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
        assert_eq!(
            cleanup_abandoned_imports(temp.path(), &conn, Duration::ZERO).unwrap(),
            1
        );
        assert!(victim.exists());
    }
}
