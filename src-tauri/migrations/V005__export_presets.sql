CREATE TABLE IF NOT EXISTS export_presets (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  format          TEXT NOT NULL,
  options_json    TEXT NOT NULL DEFAULT '{}',
  is_default      INTEGER NOT NULL DEFAULT 0,
  created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Seed built-in export presets
INSERT OR IGNORE INTO export_presets (id, name, format, options_json, is_default) VALUES
  ('preset_txt',    'Plain Text',         'txt',   '{"include_timestamps":false,"include_speakers":true}', 1),
  ('preset_srt',    'SubRip Subtitle',    'srt',   '{"include_timestamps":true,"max_chars_per_line":42}',  0),
  ('preset_vtt',    'WebVTT',             'vtt',   '{"include_timestamps":true,"max_chars_per_line":42}',  0),
  ('preset_json',   'JSON',               'json',  '{"include_timestamps":true,"include_speakers":true}',  0),
  ('preset_md',     'Markdown',           'md',    '{"include_timestamps":false,"include_speakers":true}', 0);
