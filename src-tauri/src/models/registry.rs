use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub file_size_mb: u64,
    pub download_url: String,
    pub sha256: Option<String>,
    pub supports_en_only: bool,
    pub supports_tdrz: bool,
    pub is_downloaded: bool,
    pub is_default: bool,
}
