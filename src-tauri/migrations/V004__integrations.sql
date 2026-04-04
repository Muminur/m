CREATE TABLE IF NOT EXISTS integrations (
  id          TEXT PRIMARY KEY,
  service     TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  is_enabled  INTEGER NOT NULL DEFAULT 0,
  config_json TEXT NOT NULL DEFAULT '{}',
  created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Seed built-in integration stubs
INSERT OR IGNORE INTO integrations (id, service, display_name, is_enabled, config_json) VALUES
  ('int_notion',    'notion',    'Notion',          0, '{}'),
  ('int_obsidian',  'obsidian',  'Obsidian',        0, '{}'),
  ('int_webhook',   'webhook',   'Custom Webhook',  0, '{}'),
  ('int_deepl',     'deepl',     'DeepL',           0, '{}'),
  ('int_openai',    'openai',    'OpenAI',          0, '{}'),
  ('int_anthropic', 'anthropic', 'Anthropic',       0, '{}');
