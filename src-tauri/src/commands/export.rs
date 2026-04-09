use crate::database::{segments, transcripts, Database};
use crate::error::{AppError, ExportErrorCode};
use crate::export;
use std::sync::Arc;
use tauri::{command, State};

#[command]
pub async fn export_transcript(
    transcript_id: String,
    format: String,
    options: Option<serde_json::Value>,
    db: State<'_, Arc<Database>>,
) -> Result<String, AppError> {
    export_transcript_inner(&transcript_id, &format, options.as_ref(), &db)
}

#[command]
pub async fn export_to_file(
    transcript_id: String,
    format: String,
    file_path: String,
    options: Option<serde_json::Value>,
    db: State<'_, Arc<Database>>,
) -> Result<(), AppError> {
    if format == "whisper" {
        let conn = db.get()?;
        let transcript =
            transcripts::get_by_id(&conn, &transcript_id)?.ok_or_else(|| AppError::ExportError {
                code: ExportErrorCode::FormatError,
                message: format!("Transcript '{}' not found", transcript_id),
            })?;
        let segs = segments::get_by_transcript(&conn, &transcript_id)?;
        let audio_path = transcript.audio_path.as_deref().map(std::path::Path::new);
        return export::whisper_archive::export_archive(
            std::path::Path::new(&file_path),
            &transcript,
            &segs,
            audio_path,
        );
    }

    // Binary formats: write raw bytes
    if format == "pdf" || format == "docx" {
        let conn = db.get()?;
        let transcript =
            transcripts::get_by_id(&conn, &transcript_id)?.ok_or_else(|| AppError::ExportError {
                code: ExportErrorCode::FormatError,
                message: format!("Transcript '{}' not found", transcript_id),
            })?;
        let segs = segments::get_by_transcript(&conn, &transcript_id)?;
        let title = &transcript.title;
        let duration_ms = transcript.duration_ms.unwrap_or(0);
        let language = transcript.language.as_deref().unwrap_or("en");

        let bytes = match format.as_str() {
            "pdf" => export::pdf::export_pdf(title, &segs, duration_ms, language)?,
            "docx" => export::docx::export_docx(title, &segs, duration_ms, language)?,
            _ => unreachable!(),
        };
        std::fs::write(&file_path, &bytes).map_err(|e| AppError::ExportError {
            code: ExportErrorCode::IoError,
            message: format!("Failed to write file: {}", e),
        })?;
        return Ok(());
    }

    let content = export_transcript_inner(&transcript_id, &format, options.as_ref(), &db)?;
    std::fs::write(&file_path, &content).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("Failed to write file: {}", e),
    })
}

fn export_transcript_inner(
    transcript_id: &str,
    format: &str,
    options: Option<&serde_json::Value>,
    db: &Arc<Database>,
) -> Result<String, AppError> {
    let conn = db.get()?;
    let transcript =
        transcripts::get_by_id(&conn, transcript_id)?.ok_or_else(|| AppError::ExportError {
            code: ExportErrorCode::FormatError,
            message: format!("Transcript '{}' not found", transcript_id),
        })?;
    let segs = segments::get_by_transcript(&conn, transcript_id)?;
    let include_timestamps = options
        .and_then(|o| o.get("includeTimestamps"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_speakers = options
        .and_then(|o| o.get("includeSpeakers"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let title = &transcript.title;
    let duration_ms = transcript.duration_ms.unwrap_or(0);
    let language = transcript.language.as_deref().unwrap_or("en");

    match format {
        "txt" => Ok(export::text::render(
            &transcript,
            &segs,
            include_timestamps,
            include_speakers,
        )),
        "srt" => Ok(export::srt::render(&segs)),
        "vtt" => Ok(export::vtt::render(&segs)),
        "html" => export::html::export_html(title, &segs, duration_ms, language),
        "csv" => export::csv::export_csv(&segs),
        "json" => export::json::export_json(title, &segs, duration_ms, language),
        "markdown" | "md" => export::markdown::export_markdown(title, &segs, duration_ms, language),
        "pdf" => {
            let bytes = export::pdf::export_pdf(title, &segs, duration_ms, language)?;
            use base64::Engine;
            Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
        }
        "docx" => {
            let bytes = export::docx::export_docx(title, &segs, duration_ms, language)?;
            use base64::Engine;
            Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
        }
        _ => Err(AppError::ExportError {
            code: ExportErrorCode::FormatError,
            message: format!("Unknown format: {}", format),
        }),
    }
}

/// Render a transcript using a user-supplied Handlebars template string.
#[command]
pub async fn render_custom_template(
    template: String,
    transcript_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<String, AppError> {
    let conn = db.get()?;
    let transcript =
        transcripts::get_by_id(&conn, &transcript_id)?.ok_or_else(|| AppError::ExportError {
            code: ExportErrorCode::FormatError,
            message: format!("Transcript '{}' not found", transcript_id),
        })?;
    let segs = segments::get_by_transcript(&conn, &transcript_id)?;
    let title = &transcript.title;
    let duration_ms = transcript.duration_ms.unwrap_or(0);
    let language = transcript.language.as_deref().unwrap_or("en");

    export::template::render_export_template(&template, title, &segs, duration_ms, language)
}

#[command]
pub async fn copy_transcript_text(
    transcript_id: String,
    segment_ids: Option<Vec<String>>,
    db: State<'_, Arc<Database>>,
) -> Result<String, AppError> {
    let conn = db.get()?;
    let segs = segments::get_by_transcript(&conn, &transcript_id)?;
    let filtered: Vec<&segments::SegmentRow> = match &segment_ids {
        Some(ids) => segs.iter().filter(|s| ids.contains(&s.id)).collect(),
        None => segs.iter().collect(),
    };
    let text = filtered
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text)
}
