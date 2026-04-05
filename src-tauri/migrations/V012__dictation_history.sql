CREATE TABLE dictation_history (
    id TEXT PRIMARY KEY,
    text TEXT NOT NULL,
    app_target TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_dictation_history_created ON dictation_history(created_at DESC);
