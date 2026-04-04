-- Job tracking table for transcription queue
CREATE TABLE IF NOT EXISTS whisper_jobs (
  id            TEXT PRIMARY KEY,
  transcript_id TEXT REFERENCES transcripts(id) ON DELETE CASCADE,
  model_id      TEXT NOT NULL,
  status        TEXT NOT NULL CHECK(status IN ('queued','running','completed','failed','cancelled')),
  error_message TEXT,
  created_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  started_at    INTEGER,
  completed_at  INTEGER
);

CREATE INDEX IF NOT EXISTS idx_whisper_jobs_status ON whisper_jobs(status);
CREATE INDEX IF NOT EXISTS idx_whisper_jobs_transcript ON whisper_jobs(transcript_id);

-- Seed official Whisper models from ggerganov/whisper.cpp
-- SHA256 hashes sourced from https://huggingface.co/ggerganov/whisper.cpp
INSERT OR IGNORE INTO whisper_models (id, display_name, file_size_mb, download_url, sha256, supports_en_only, supports_tdrz, is_downloaded, is_default, created_at) VALUES
  ('tiny',       'Tiny',           75,   'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin',        'bd577a113a864445d4c299885e0cb97d4ba92b5f', 0, 0, 0, 0, strftime('%s','now')),
  ('tiny.en',    'Tiny (English)', 75,   'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin',     'c78c86eb1a8faa21b369bcd33207cc90d64ae9df', 1, 0, 0, 1, strftime('%s','now')),
  ('base',       'Base',           142,  'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin',        '465707469ff3a37a2b9b8d8f89f2f99de7128a9d', 0, 0, 0, 0, strftime('%s','now')),
  ('base.en',    'Base (English)', 142,  'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin',     '137c40403d78fd54d454da0f9bd998f78703390c', 1, 0, 0, 0, strftime('%s','now')),
  ('small',      'Small',          466,  'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin',       '55356645c2b361a969dfd0ef2c5a50d530afd8d5', 0, 0, 0, 0, strftime('%s','now')),
  ('small.en',   'Small (English)',466,  'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin',    'db8a495a91d927739e50b3fc1cc4c6b8f6c2d022', 1, 0, 0, 0, strftime('%s','now')),
  ('medium',     'Medium',         1500, 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin',      'fd9727b6e1217c2f614f9b698455c4ffd82463b4', 0, 0, 0, 0, strftime('%s','now')),
  ('large-v2',   'Large v2',       2900, 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v2.bin',    '0f4c8e34f21cf1a914c59d8b3ce882345ad349d6', 0, 0, 0, 0, strftime('%s','now')),
  ('large-v3',   'Large v3',       2900, 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin',    'ad82bf6a9043ceed055076d0fd39f5f186ff8062', 0, 0, 0, 0, strftime('%s','now'));
