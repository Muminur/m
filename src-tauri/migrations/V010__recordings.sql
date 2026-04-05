-- M5: Recordings table for audio capture sessions
CREATE TABLE IF NOT EXISTS recordings (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL CHECK(source IN ('mic', 'system', 'both')),
    device_id TEXT,
    device_name TEXT,
    audio_path TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    sample_rate INTEGER NOT NULL DEFAULT 16000,
    channels INTEGER NOT NULL DEFAULT 1,
    transcript_id TEXT REFERENCES transcripts(id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_recordings_created_at ON recordings(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_recordings_transcript_id ON recordings(transcript_id);

-- Watch folder config persistence (supplements settings.json watch_folders)
CREATE TABLE IF NOT EXISTS watch_folder_events (
    id TEXT PRIMARY KEY,
    folder_path TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('detected', 'queued', 'transcribed', 'failed')),
    transcript_id TEXT REFERENCES transcripts(id) ON DELETE SET NULL,
    error_message TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    processed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_watch_folder_events_status ON watch_folder_events(status);
