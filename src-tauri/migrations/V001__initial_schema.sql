CREATE TABLE IF NOT EXISTS folders (
  id        TEXT PRIMARY KEY,
  name      TEXT NOT NULL,
  parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
  color     TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS transcripts (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  duration_ms   INTEGER,
  language      TEXT,
  model_id      TEXT,
  source_type   TEXT CHECK(source_type IN ('file','mic','system','meeting','youtube')),
  source_url    TEXT,
  audio_path    TEXT,
  folder_id     TEXT REFERENCES folders(id) ON DELETE SET NULL,
  is_starred    INTEGER NOT NULL DEFAULT 0,
  is_deleted    INTEGER NOT NULL DEFAULT 0,
  deleted_at    INTEGER,
  speaker_count INTEGER DEFAULT 0,
  word_count    INTEGER DEFAULT 0,
  metadata      TEXT DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_transcripts_created ON transcripts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transcripts_folder ON transcripts(folder_id);
CREATE INDEX IF NOT EXISTS idx_transcripts_deleted ON transcripts(is_deleted, deleted_at);

CREATE TABLE IF NOT EXISTS speakers (
  id            TEXT PRIMARY KEY,
  transcript_id TEXT NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
  label         TEXT NOT NULL,
  color         TEXT
);

CREATE TABLE IF NOT EXISTS segments (
  id            TEXT PRIMARY KEY,
  transcript_id TEXT NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
  index_num     INTEGER NOT NULL,
  start_ms      INTEGER NOT NULL,
  end_ms        INTEGER NOT NULL,
  text          TEXT NOT NULL,
  speaker_id    TEXT REFERENCES speakers(id) ON DELETE SET NULL,
  confidence    REAL,
  is_deleted    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_segments_transcript ON segments(transcript_id, index_num);

CREATE TABLE IF NOT EXISTS tags (
  id   TEXT PRIMARY KEY,
  name TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS transcript_tags (
  transcript_id TEXT NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
  tag_id        TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (transcript_id, tag_id)
);

CREATE TABLE IF NOT EXISTS whisper_models (
  id               TEXT PRIMARY KEY,
  display_name     TEXT NOT NULL,
  file_path        TEXT,
  file_size_mb     INTEGER NOT NULL,
  download_url     TEXT NOT NULL,
  sha256           TEXT,
  is_downloaded    INTEGER NOT NULL DEFAULT 0,
  is_default       INTEGER NOT NULL DEFAULT 0,
  supports_tdrz    INTEGER NOT NULL DEFAULT 0,
  supports_en_only INTEGER NOT NULL DEFAULT 0,
  created_at       INTEGER NOT NULL
);
