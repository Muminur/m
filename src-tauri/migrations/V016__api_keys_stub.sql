CREATE TABLE IF NOT EXISTS api_keys (
    service TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
-- Note: actual key values are stored in system Keychain, never in this table.
-- This table only tracks which services have been configured.
