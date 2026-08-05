ALTER TABLE acceleration_stats
ADD COLUMN transcript_id TEXT REFERENCES transcripts(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_accel_stats_transcript
ON acceleration_stats(transcript_id, recorded_at DESC);
