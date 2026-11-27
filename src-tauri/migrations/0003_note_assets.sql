CREATE TABLE IF NOT EXISTS note_assets (
  source_rel_path TEXT NOT NULL,
  asset_rel_path  TEXT NOT NULL,
  PRIMARY KEY (source_rel_path, asset_rel_path)
);

CREATE INDEX IF NOT EXISTS idx_note_assets_asset ON note_assets(asset_rel_path);
