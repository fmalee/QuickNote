CREATE TABLE IF NOT EXISTS directory_workspace_configs (
  vault_rel_dir TEXT PRIMARY KEY,
  config_json   TEXT NOT NULL,
  mtime_ms      INTEGER NOT NULL,
  parse_error   TEXT
);
