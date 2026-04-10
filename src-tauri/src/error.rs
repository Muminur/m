use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum AppError {
    #[error("Transcription error: {message}")]
    TranscriptionError {
        code: TranscriptionErrorCode,
        message: String,
    },

    #[error("Audio error: {message}")]
    AudioError {
        code: AudioErrorCode,
        message: String,
    },

    #[error("Model error: {message}")]
    ModelError {
        code: ModelErrorCode,
        message: String,
    },

    #[error("Export error: {message}")]
    ExportError {
        code: ExportErrorCode,
        message: String,
    },

    #[error("Integration error: {message}")]
    IntegrationError {
        code: IntegrationErrorCode,
        message: String,
    },

    #[error("License error: {message}")]
    LicenseError {
        code: LicenseErrorCode,
        message: String,
    },

    #[error("Storage error: {message}")]
    StorageError {
        code: StorageErrorCode,
        message: String,
    },

    #[error("Network error: {message}")]
    NetworkError {
        code: NetworkErrorCode,
        message: String,
    },

    #[error("Dictation error: {message}")]
    DictationError {
        code: DictationErrorCode,
        message: String,
    },

    #[error("Import error: {message}")]
    ImportError {
        code: ImportErrorCode,
        message: String,
    },

    #[error("Diarization error: {message}")]
    DiarizationError {
        code: DiarizationErrorCode,
        message: String,
    },

    #[error("Batch error: {message}")]
    BatchError {
        code: BatchErrorCode,
        message: String,
    },

    #[error("AI error: {message}")]
    AiError { code: AiErrorCode, message: String },

    #[error("Cloud transcription error: {message}")]
    CloudTranscriptionError {
        code: CloudTranscriptionErrorCode,
        message: String,
    },
}

#[derive(Debug, Serialize, Clone)]
pub enum TranscriptionErrorCode {
    ModelNotLoaded,
    InvalidAudioFormat,
    Cancelled,
    InferenceFailure,
    VadError,
    BackendUnavailable,
}

#[derive(Debug, Serialize, Clone)]
pub enum AudioErrorCode {
    DeviceNotFound,
    PermissionDenied,
    CaptureFailure,
    DecodeFailure,
    UnsupportedFormat,
    InvalidAudioFormat,
}

#[derive(Debug, Serialize, Clone)]
pub enum ModelErrorCode {
    DownloadFailed,
    ChecksumMismatch,
    CorruptedFile,
    NotFound,
    InsufficientDisk,
}

#[derive(Debug, Serialize, Clone)]
pub enum ExportErrorCode {
    FormatError,
    IoError,
    TemplateError,
    PermissionDenied,
}

#[derive(Debug, Serialize, Clone)]
pub enum IntegrationErrorCode {
    AuthenticationFailed,
    ApiError,
    ConfigurationMissing,
    RateLimited,
}

#[derive(Debug, Serialize, Clone)]
pub enum LicenseErrorCode {
    InvalidKey,
    Expired,
    MachineLimit,
    ActivationFailed,
    SignatureInvalid,
}

#[derive(Debug, Serialize, Clone)]
pub enum StorageErrorCode {
    DatabaseError,
    MigrationFailed,
    DiskFull,
    IoError,
}

#[derive(Debug, Serialize, Clone)]
pub enum DictationErrorCode {
    InvalidState,
    AccessibilityDenied,
    InsertionFailed,
    CorrectionFailed,
}

#[derive(Debug, Serialize, Clone)]
pub enum ImportErrorCode {
    YtDlpNotFound,
    DownloadFailed,
    InvalidUrl,
    UnsupportedPlatform,
}

#[derive(Debug, Serialize, Clone)]
pub enum NetworkErrorCode {
    PolicyBlocked,
    ConnectionFailed,
    Timeout,
    TlsError,
    HttpError { status: u16 },
}

#[derive(Debug, Serialize, Clone)]
pub enum DiarizationErrorCode {
    ProviderNotFound,
    ApiError,
    NoSpeakersDetected,
    InvalidTranscript,
    ValidationError,
}

#[derive(Debug, Serialize, Clone)]
pub enum BatchErrorCode {
    JobNotFound,
    InvalidState,
    ConcurrencyLimit,
    ExportFailed,
}

#[derive(Debug, Serialize, Clone)]
pub enum AiErrorCode {
    ProviderNotFound,
    ModelNotFound,
    ApiError,
    RateLimited,
    TokenLimitExceeded,
    InvalidApiKey,
}

#[derive(Debug, Serialize, Clone)]
pub enum CloudTranscriptionErrorCode {
    ProviderNotFound,
    UploadFailed,
    TranscriptionFailed,
    InvalidApiKey,
    FileTooLarge,
}

// Implement From for common error types
impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: e.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::StorageError {
            code: StorageErrorCode::IoError,
            message: format!("JSON error: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_error_serializes_to_typed_json() {
        let err = AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "connection failed".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["kind"], "StorageError");
        assert_eq!(json["detail"]["message"], "connection failed");
    }

    #[test]
    fn test_network_error_serializes_to_typed_json() {
        let err = AppError::NetworkError {
            code: NetworkErrorCode::PolicyBlocked,
            message: "offline mode".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["kind"], "NetworkError");
    }

    #[test]
    fn test_transcription_error_serializes() {
        let err = AppError::TranscriptionError {
            code: TranscriptionErrorCode::ModelNotLoaded,
            message: "no model".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["kind"], "TranscriptionError");
        assert_eq!(json["detail"]["code"], "ModelNotLoaded");
    }

    #[test]
    fn test_all_error_kinds_serialize() {
        let errors: Vec<AppError> = vec![
            AppError::TranscriptionError {
                code: TranscriptionErrorCode::Cancelled,
                message: "test".into(),
            },
            AppError::AudioError {
                code: AudioErrorCode::DeviceNotFound,
                message: "test".into(),
            },
            AppError::ModelError {
                code: ModelErrorCode::NotFound,
                message: "test".into(),
            },
            AppError::ExportError {
                code: ExportErrorCode::FormatError,
                message: "test".into(),
            },
            AppError::IntegrationError {
                code: IntegrationErrorCode::ApiError,
                message: "test".into(),
            },
            AppError::LicenseError {
                code: LicenseErrorCode::InvalidKey,
                message: "test".into(),
            },
            AppError::StorageError {
                code: StorageErrorCode::DiskFull,
                message: "test".into(),
            },
            AppError::NetworkError {
                code: NetworkErrorCode::Timeout,
                message: "test".into(),
            },
            AppError::DictationError {
                code: DictationErrorCode::InvalidState,
                message: "test".into(),
            },
        ];
        for err in errors {
            let result = serde_json::to_value(&err);
            assert!(result.is_ok(), "Failed to serialize: {:?}", err);
            let json = result.unwrap();
            assert!(json["kind"].is_string());
        }
    }
}
