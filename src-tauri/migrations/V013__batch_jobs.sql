CREATE TABLE batch_jobs (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'Pending',
    concurrency INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE batch_job_items (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES batch_jobs(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    transcript_id TEXT,
    status TEXT NOT NULL DEFAULT 'Queued',
    error TEXT,
    progress REAL NOT NULL DEFAULT 0.0,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_batch_job_items_job ON batch_job_items(job_id, sort_order);
