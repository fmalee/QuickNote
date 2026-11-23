CREATE TABLE IF NOT EXISTS library_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  key              TEXT PRIMARY KEY,
  id               TEXT,
  rel_path         TEXT NOT NULL UNIQUE,
  title            TEXT NOT NULL,
  doc_type         TEXT NOT NULL,
  project          TEXT,
  mtime_ms         INTEGER NOT NULL,
  size_bytes       INTEGER NOT NULL,
  body_hash        TEXT NOT NULL,
  frontmatter_json TEXT NOT NULL,
  excerpt          TEXT NOT NULL,
  meta_error       TEXT,
  pinned           INTEGER NOT NULL DEFAULT 0,
  archived         INTEGER NOT NULL DEFAULT 0,
  is_inbox         INTEGER NOT NULL DEFAULT 0,
  updated_at       INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_id ON notes(id);
CREATE INDEX IF NOT EXISTS idx_notes_project ON notes(project);

CREATE TABLE IF NOT EXISTS note_tags (
  note_key TEXT NOT NULL,
  tag      TEXT NOT NULL,
  PRIMARY KEY (note_key, tag),
  FOREIGN KEY (note_key) REFERENCES notes(key) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag);

CREATE TABLE IF NOT EXISTS note_links (
  source_rel_path TEXT NOT NULL,
  target_rel_path TEXT NOT NULL,
  PRIMARY KEY (source_rel_path, target_rel_path)
);
CREATE INDEX IF NOT EXISTS idx_note_links_target ON note_links(target_rel_path);

CREATE TABLE IF NOT EXISTS commands (
  id            TEXT PRIMARY KEY,
  note_rel_path TEXT NOT NULL,
  section       TEXT NOT NULL,
  title         TEXT NOT NULL,
  description   TEXT NOT NULL,
  code          TEXT NOT NULL,
  language      TEXT,
  keywords      TEXT NOT NULL,
  is_danger     INTEGER NOT NULL DEFAULT 0,
  line          INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
  note_key UNINDEXED,
  title_tokens,
  tag_tokens,
  body_tokens,
  tokenize = 'unicode61'
);
