//! Translation-model registry + download-on-demand commands.
//!
//! Unlike Whisper models (a single `.bin`), the NLLB CTranslate2 model is a
//! *directory* of files (`model.bin`, `config.json`, tokenizer, vocabulary…),
//! so the downloader fetches each required file into `models_dir/<model_id>/`.
//! It mirrors `ModelManager`'s HTTP + `NetworkGuard` + progress-event pattern
//! (see `models/manager.rs::run_download`), emitting `translation-model:*`
//! events instead of `model:*`.
//!
//! CPU int8 only — NEVER enable Metal/GPU here (Intel x86_64 aborts on Metal).

use std::io::Write;
use std::sync::Arc;
use tauri::{command, AppHandle, Emitter, Manager, State};

use crate::database::Database;
use crate::database::{segments, translations};
use crate::error::{AppError, ModelErrorCode, StorageErrorCode};
use crate::models::manager::ModelManager;
use crate::network::guard::NetworkGuard;
use crate::translation::manager::TranslationEngineManager;
use crate::translation::{languages, model};

/// Files that make up a functioning NLLB CTranslate2 model directory. The
/// download URL stored in the DB is the HuggingFace repo `resolve/main` base;
/// each file is fetched as `<base>/<file>`.
const MODEL_FILES: &[&str] = &[
    "config.json",
    "generation_config.json",
    "model.bin",
    "sentencepiece.bpe.model",
    "shared_vocabulary.json",
    "special_tokens_map.json",
    "tokenizer.json",
    "tokenizer_config.json",
];

// ─── Response + event payloads ──────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationModelInfo {
    pub id: String,
    pub display_name: String,
    pub file_size_mb: i64,
    pub is_downloaded: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgressEvent {
    model_id: String,
    bytes_downloaded: u64,
    total_bytes: u64,
    percentage: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadCompleteEvent {
    model_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadErrorEvent {
    model_id: String,
    error: String,
}

// ─── Commands ───────────────────────────────────────────────────────────────

#[command]
pub async fn list_translation_models(
    db: State<'_, Arc<Database>>,
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<TranslationModelInfo>, AppError> {
    let conn = db.get()?;
    let mut stmt = conn
        .prepare("SELECT id, display_name, file_size_mb FROM translation_models ORDER BY id")
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("prepare: {e}"),
        })?;
    let root = &model_manager.models_dir;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let display_name: String = row.get(1)?;
            let file_size_mb: i64 = row.get(2)?;
            Ok((id, display_name, file_size_mb))
        })
        .map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("query: {e}"),
        })?;
    let mut out = Vec::new();
    for r in rows {
        let (id, display_name, file_size_mb) = r.map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("row: {e}"),
        })?;
        let is_downloaded = model::is_downloaded(root);
        out.push(TranslationModelInfo {
            id,
            display_name,
            file_size_mb,
            is_downloaded,
        });
    }
    Ok(out)
}

#[command]
pub async fn download_translation_model(
    model_id: String,
    app_handle: AppHandle,
    db: State<'_, Arc<Database>>,
    model_manager: State<'_, Arc<ModelManager>>,
    translation_manager: State<'_, Arc<TranslationEngineManager>>,
) -> Result<(), AppError> {
    // Do not download a large model that this platform cannot execute.
    translation_manager.ensure_supported()?;

    // Fetch the download base URL + estimated size from the registry.
    let (base_url, file_size_mb) = {
        let conn = db.get()?;
        conn.query_row(
            "SELECT download_url, file_size_mb FROM translation_models WHERE id = ?1",
            rusqlite::params![model_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|_| AppError::ModelError {
            code: ModelErrorCode::NotFound,
            message: format!("Translation model '{}' not found in registry", model_id),
        })?
    };

    let models_dir = model_manager.models_dir.clone();
    let app_for_task = app_handle.clone();
    let model_id_event = model_id.clone();

    // Fire-and-forget: progress reported via events, mirroring ModelManager.
    tokio::spawn(async move {
        let result = run_translation_download(
            &models_dir,
            &base_url,
            file_size_mb.max(0) as u64,
            &app_for_task,
        )
        .await;

        match result {
            Ok(()) => {
                let _ = app_for_task.emit(
                    "translation-model:download-complete",
                    DownloadCompleteEvent {
                        model_id: model_id_event,
                    },
                );
            }
            Err(e) => {
                // Clean up the partial directory so a retry starts fresh.
                let _ = std::fs::remove_dir_all(model::model_dir(&models_dir));
                let _ = app_for_task.emit(
                    "translation-model:download-error",
                    DownloadErrorEvent {
                        model_id: model_id_event,
                        error: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(())
}

#[command]
pub async fn delete_translation_model(
    _model_id: String,
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<(), AppError> {
    let dir = model::model_dir(&model_manager.models_dir);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("delete model dir: {e}"),
        })?;
    }
    Ok(())
}

// ─── Download implementation ────────────────────────────────────────────────

/// Download every file in [`MODEL_FILES`] from `<base_url>/<file>` into
/// `models_root/<model_id>/`, emitting cumulative progress. Files are written
/// to `.partial` names and renamed on success; the dir is only considered
/// "downloaded" once `model.bin` exists (see `model::is_downloaded`), so
/// `model.bin` is fetched last.
async fn run_translation_download(
    models_root: &std::path::Path,
    base_url: &str,
    file_size_mb: u64,
    app_handle: &AppHandle,
) -> Result<(), AppError> {
    let dir = model::model_dir(models_root);
    std::fs::create_dir_all(&dir).map_err(|e| AppError::ModelError {
        code: ModelErrorCode::DownloadFailed,
        message: format!("Failed to create model directory: {}", e),
    })?;

    // Estimated total (bytes) for percentage reporting; the model row size is a
    // close upper bound since model.bin dominates.
    let total_bytes = file_size_mb.saturating_mul(1024 * 1024);
    let mut bytes_downloaded: u64 = 0;

    let guard = app_handle.state::<NetworkGuard>();
    let base = base_url.trim_end_matches('/');

    // model.bin last so is_downloaded() only flips true once everything is present.
    let mut files: Vec<&str> = MODEL_FILES
        .iter()
        .filter(|f| **f != "model.bin")
        .copied()
        .collect();
    files.push("model.bin");

    for file in files {
        let url = format!("{}/{}", base, file);
        let req = guard.client().get(&url);
        let response = guard.request(req).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::ModelError {
                code: ModelErrorCode::DownloadFailed,
                message: format!("HTTP {} downloading '{}'", status, file),
            });
        }

        let partial_path = dir.join(format!("{}.partial", file));
        let final_path = dir.join(file);

        let out_file = std::fs::File::create(&partial_path).map_err(|e| AppError::ModelError {
            code: ModelErrorCode::DownloadFailed,
            message: format!("Failed to open '{}': {}", file, e),
        })?;
        let mut writer = std::io::BufWriter::new(out_file);
        let mut response = response;

        while let Some(chunk) = response.chunk().await.map_err(|e| AppError::ModelError {
            code: ModelErrorCode::DownloadFailed,
            message: format!("Download stream error on '{}': {}", file, e),
        })? {
            writer.write_all(&chunk).map_err(|e| AppError::ModelError {
                code: ModelErrorCode::DownloadFailed,
                message: format!("Failed to write '{}': {}", file, e),
            })?;
            bytes_downloaded += chunk.len() as u64;
            let percentage = if total_bytes > 0 {
                (bytes_downloaded as f32 / total_bytes as f32).min(1.0)
            } else {
                0.0
            };
            let _ = app_handle.emit(
                "translation-model:download-progress",
                DownloadProgressEvent {
                    model_id: model::NLLB_MODEL_ID.to_string(),
                    bytes_downloaded,
                    total_bytes,
                    percentage,
                },
            );
        }

        writer.flush().map_err(|e| AppError::ModelError {
            code: ModelErrorCode::DownloadFailed,
            message: format!("Failed to flush '{}': {}", file, e),
        })?;
        drop(writer);

        std::fs::rename(&partial_path, &final_path).map_err(|e| AppError::ModelError {
            code: ModelErrorCode::DownloadFailed,
            message: format!("Failed to finalize '{}': {}", file, e),
        })?;
    }

    tracing::info!("Translation model downloaded to {:?}", dir);
    Ok(())
}

// ─── Offline translation commands ────────────────────────────────────────────

/// Translate every segment of a transcript into `target_lang` (a FLORES-200
/// code, e.g. "ben_Beng") using the offline NLLB engine, cache the results, and
/// return them. Loads the model lazily on first use.
#[command]
pub async fn translate_transcript(
    transcript_id: String,
    target_lang: String, // FLORES code e.g. "ben_Beng"
    app_handle: AppHandle,
    db: State<'_, Arc<Database>>,
    model_manager: State<'_, Arc<ModelManager>>,
    translation_manager: State<'_, Arc<TranslationEngineManager>>,
) -> Result<Vec<translations::TranslationRow>, AppError> {
    // Keep this command registered on every desktop target, but fail before
    // touching the model or database when the native NLLB runtime is absent.
    translation_manager.ensure_supported()?;

    if !languages::is_supported(&target_lang) {
        return Err(AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: format!("unsupported translation target language: {target_lang}"),
        });
    }

    let models_root = model_manager.models_dir.clone();
    if !model::is_downloaded(&models_root) {
        let _ = app_handle.emit("translation:model-missing", model::NLLB_MODEL_ID);
        return Err(AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "translation model not downloaded".into(),
        });
    }

    // Load source segments + detected source language (mapped to FLORES-200).
    let (segs, source_flores) = {
        let conn = db.get()?;
        let segs = segments::get_by_transcript(&conn, &transcript_id)?;
        let lang: Option<String> = conn
            .query_row(
                "SELECT language FROM transcripts WHERE id = ?1",
                rusqlite::params![&transcript_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("load transcript language: {e}"),
            })?;
        let source_code = lang.as_deref().unwrap_or("en");
        let src = languages::to_flores(source_code)
            .ok_or_else(|| AppError::StorageError {
                code: StorageErrorCode::DatabaseError,
                message: format!("unsupported translation source language: {source_code}"),
            })?
            .to_string();
        (segs, src)
    };

    // Skip if source == target (no-op).
    if source_flores == target_lang {
        return Ok(Vec::new());
    }

    let texts: Vec<String> = segs.iter().map(|s| s.text.clone()).collect();
    let engine_manager = Arc::clone(translation_manager.inner());
    let source_for_job = source_flores.clone();
    let target_for_job = target_lang.clone();
    let translated = tauri::async_runtime::spawn_blocking(move || {
        engine_manager.ensure_loaded(&models_root)?;
        engine_manager.translate(&texts, &source_for_job, &target_for_job)
    })
    .await
    .map_err(|e| AppError::StorageError {
        code: StorageErrorCode::DatabaseError,
        message: format!("translation worker failed: {e}"),
    })??;

    let rows: Vec<(String, String)> = segs
        .iter()
        .zip(translated.iter())
        .map(|(s, t)| (s.id.clone(), t.clone()))
        .collect();

    {
        let conn = db.get()?;
        translations::insert_batch(
            &conn,
            &transcript_id,
            &target_lang,
            Some(&source_flores),
            &rows,
        )?;
    }

    let result = {
        let conn = db.get()?;
        translations::get_by_transcript_lang(&conn, &transcript_id, &target_lang)?
    };
    let _ = app_handle.emit("translation:complete", &transcript_id);
    Ok(result)
}

/// Return cached translations for a transcript in `target_lang`, if any.
#[command]
pub async fn get_translation(
    transcript_id: String,
    target_lang: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<translations::TranslationRow>, AppError> {
    let conn = db.get()?;
    translations::get_by_transcript_lang(&conn, &transcript_id, &target_lang)
}

#[cfg(test)]
mod tests {
    use crate::database::migrations;
    use rusqlite::Connection;

    #[test]
    fn v018_seeds_nllb_row() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::run(&mut conn).unwrap();

        let (id, display_name, download_url, file_size_mb): (String, String, String, i64) = conn
            .query_row(
                "SELECT id, display_name, download_url, file_size_mb \
                 FROM translation_models WHERE id = 'nllb-200-distilled-600M-int8'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("seed row should exist");

        assert_eq!(id, "nllb-200-distilled-600M-int8");
        assert_eq!(display_name, "NLLB-200 Distilled 600M (int8)");
        assert!(
            download_url.starts_with("https://huggingface.co/"),
            "download_url should be a real HuggingFace URL, got {download_url:?}"
        );
        assert!(file_size_mb > 0, "file_size_mb should be positive");
    }

    #[test]
    fn v018_seed_is_idempotent() {
        // Running migrations is a no-op the second time; the INSERT OR IGNORE
        // must not produce a duplicate row.
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::run(&mut conn).unwrap();
        migrations::run(&mut conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM translation_models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "exactly one translation model should be seeded");
    }
}
