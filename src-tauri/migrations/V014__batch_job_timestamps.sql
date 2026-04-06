ALTER TABLE batch_jobs ADD COLUMN started_at INTEGER;
ALTER TABLE batch_jobs ADD COLUMN completed_at INTEGER;
ALTER TABLE batch_job_items ADD COLUMN processing_ms INTEGER;
