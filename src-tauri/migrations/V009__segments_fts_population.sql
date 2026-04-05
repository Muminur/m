INSERT OR IGNORE INTO transcripts_fts(rowid, text)
  SELECT rowid, text FROM segments WHERE is_deleted = 0;
