CREATE TABLE IF NOT EXISTS acceleration_stats (
    id                TEXT PRIMARY KEY,
    job_id            TEXT REFERENCES whisper_jobs(id) ON DELETE SET NULL,
    model_id          TEXT NOT NULL,
    backend           TEXT NOT NULL CHECK(backend IN ('auto','cpu','metal','core_ml')),
    audio_duration_ms INTEGER NOT NULL,
    wall_time_ms      INTEGER NOT NULL,
    realtime_factor   REAL NOT NULL,
    recorded_at       INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_accel_stats_model   ON acceleration_stats(model_id);
CREATE INDEX IF NOT EXISTS idx_accel_stats_backend ON acceleration_stats(backend);
