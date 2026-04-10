use crate::database::{segments::SegmentRow, transcripts::TranscriptRow};
use crate::error::{AppError, ExportErrorCode};
use std::io::{Read, Write};
use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ArchiveManifest {
    pub version: u32,
    pub created_at: i64,
    pub app_version: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ArchiveTranscript {
    pub transcript: TranscriptRow,
    pub segments: Vec<SegmentRow>,
}

pub fn export_archive(
    path: &Path,
    transcript: &TranscriptRow,
    segments: &[SegmentRow],
    audio_path: Option<&Path>,
) -> Result<(), AppError> {
    let file = std::fs::File::create(path).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("Failed to create archive: {}", e),
    })?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Write manifest
    let manifest = ArchiveManifest {
        version: 1,
        created_at: chrono::Utc::now().timestamp(),
        app_version: "1.0.0".into(),
    };
    zip.start_file("manifest.json", options)
        .map_err(|e| AppError::ExportError {
            code: ExportErrorCode::IoError,
            message: format!("{}", e),
        })?;
    zip.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(|e| AppError::ExportError {
                code: ExportErrorCode::FormatError,
                message: format!("Failed to serialize manifest: {}", e),
            })?
            .as_bytes(),
    )
    .map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("{}", e),
    })?;

    // Write transcript data
    let archive_data = ArchiveTranscript {
        transcript: transcript.clone(),
        segments: segments.to_vec(),
    };
    zip.start_file("transcript.json", options)
        .map_err(|e| AppError::ExportError {
            code: ExportErrorCode::IoError,
            message: format!("{}", e),
        })?;
    zip.write_all(
        serde_json::to_string_pretty(&archive_data)
            .map_err(|e| AppError::ExportError {
                code: ExportErrorCode::FormatError,
                message: format!("Failed to serialize transcript: {}", e),
            })?
            .as_bytes(),
    )
    .map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("{}", e),
    })?;

    // Optionally include audio file
    if let Some(audio) = audio_path {
        if audio.exists() {
            let filename = audio
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("audio.wav");
            zip.start_file(format!("audio/{}", filename), options)
                .map_err(|e| AppError::ExportError {
                    code: ExportErrorCode::IoError,
                    message: format!("{}", e),
                })?;
            let mut audio_file = std::fs::File::open(audio).map_err(|e| AppError::ExportError {
                code: ExportErrorCode::IoError,
                message: format!("{}", e),
            })?;
            let mut buf = Vec::new();
            audio_file
                .read_to_end(&mut buf)
                .map_err(|e| AppError::ExportError {
                    code: ExportErrorCode::IoError,
                    message: format!("{}", e),
                })?;
            zip.write_all(&buf).map_err(|e| AppError::ExportError {
                code: ExportErrorCode::IoError,
                message: format!("{}", e),
            })?;
        }
    }

    zip.finish().map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("{}", e),
    })?;
    Ok(())
}

pub fn import_archive(path: &Path) -> Result<ArchiveTranscript, AppError> {
    let file = std::fs::File::open(path).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::IoError,
        message: format!("Failed to open archive: {}", e),
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| AppError::ExportError {
        code: ExportErrorCode::FormatError,
        message: format!("Invalid archive: {}", e),
    })?;

    let mut transcript_file =
        archive
            .by_name("transcript.json")
            .map_err(|e| AppError::ExportError {
                code: ExportErrorCode::FormatError,
                message: format!("Missing transcript.json: {}", e),
            })?;
    let mut contents = String::new();
    transcript_file
        .read_to_string(&mut contents)
        .map_err(|e| AppError::ExportError {
            code: ExportErrorCode::IoError,
            message: format!("Failed to read transcript.json: {}", e),
        })?;
    let data: ArchiveTranscript =
        serde_json::from_str(&contents).map_err(|e| AppError::ExportError {
            code: ExportErrorCode::FormatError,
            message: format!("Invalid transcript data: {}", e),
        })?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn sample_transcript() -> TranscriptRow {
        TranscriptRow {
            id: "t1".into(),
            title: "Test Archive".into(),
            created_at: 1000,
            updated_at: 1000,
            duration_ms: Some(60000),
            language: Some("en".into()),
            model_id: None,
            source_type: None,
            source_url: None,
            audio_path: None,
            folder_id: None,
            is_starred: false,
            is_deleted: false,
            deleted_at: None,
            speaker_count: 0,
            word_count: 5,
            metadata: "{}".into(),
        }
    }

    fn sample_segments() -> Vec<SegmentRow> {
        vec![SegmentRow {
            id: "s1".into(),
            transcript_id: "t1".into(),
            index_num: 0,
            start_ms: 0,
            end_ms: 5000,
            text: "Hello world".into(),
            speaker_id: None,
            confidence: Some(0.9),
            is_deleted: false,
        }]
    }

    #[test]
    fn test_export_import_roundtrip() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("whisper");
        export_archive(&path, &sample_transcript(), &sample_segments(), None).unwrap();
        let imported = import_archive(&path).unwrap();
        assert_eq!(imported.transcript.title, "Test Archive");
        assert_eq!(imported.segments.len(), 1);
        assert_eq!(imported.segments[0].text, "Hello world");
        std::fs::remove_file(&path).ok();
    }
}
