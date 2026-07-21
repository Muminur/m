CREATE TABLE IF NOT EXISTS translation_models (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    download_url TEXT NOT NULL,
    sha256 TEXT,
    file_size_mb INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
INSERT OR IGNORE INTO translation_models (id, display_name, download_url, sha256, file_size_mb)
VALUES (
    'nllb-200-distilled-600M-int8',
    'NLLB-200 Distilled 600M (int8)',
    'https://huggingface.co/Serkan007/CTranslate2-nllb-200-int8/resolve/main',
    NULL,
    650
);
