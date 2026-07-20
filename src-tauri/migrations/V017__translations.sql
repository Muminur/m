CREATE TABLE IF NOT EXISTS translations (
    id TEXT PRIMARY KEY,
    transcript_id TEXT NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
    segment_id TEXT NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
    target_lang TEXT NOT NULL,
    source_lang TEXT,
    text TEXT NOT NULL,
    engine TEXT NOT NULL DEFAULT 'nllb-600m',
    created_at TEXT NOT NULL,
    UNIQUE(segment_id, target_lang)
);
CREATE INDEX IF NOT EXISTS idx_translations_transcript
    ON translations(transcript_id, target_lang);
