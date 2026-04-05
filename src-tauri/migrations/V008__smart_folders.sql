CREATE TABLE IF NOT EXISTS smart_folders (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  filter_json TEXT NOT NULL DEFAULT '{}',
  created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
