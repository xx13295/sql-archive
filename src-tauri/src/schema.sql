PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_meta (
  version INTEGER NOT NULL
);

INSERT INTO schema_meta(version)
SELECT 1
WHERE NOT EXISTS (SELECT 1 FROM schema_meta);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS script_marks (
  root_id TEXT NOT NULL,
  relative_path TEXT NOT NULL,
  file_sha256 TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'passed', 'returned')),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (root_id, relative_path)
);

CREATE TABLE IF NOT EXISTS archive_runs (
  id TEXT PRIMARY KEY,
  root_id TEXT NOT NULL,
  root_path TEXT NOT NULL,
  product TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('running', 'committed', 'failed', 'rolled_back', 'needs_attention')),
  backup_date TEXT NOT NULL,
  summary_json TEXT,
  error TEXT,
  started_at TEXT NOT NULL,
  finished_at TEXT
);
CREATE INDEX IF NOT EXISTS archive_runs_root_started
ON archive_runs(root_id, started_at DESC);

CREATE TABLE IF NOT EXISTS archive_run_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id TEXT NOT NULL REFERENCES archive_runs(id) ON DELETE CASCADE,
  group_key TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  source_path TEXT NOT NULL,
  source_file_name TEXT NOT NULL,
  source_sha256 TEXT NOT NULL,
  source_db TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  backup_path TEXT,
  archive_paths_json TEXT NOT NULL,
  state TEXT NOT NULL,
  error TEXT
);
CREATE INDEX IF NOT EXISTS archive_run_items_run
ON archive_run_items(run_id, sort_order);

CREATE TABLE IF NOT EXISTS archive_run_steps (
  run_id TEXT NOT NULL REFERENCES archive_runs(id) ON DELETE CASCADE,
  step TEXT NOT NULL,
  state TEXT NOT NULL,
  detail_json TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, step)
);

CREATE TABLE IF NOT EXISTS validation_cache (
  root_id TEXT NOT NULL,
  file_sha256 TEXT NOT NULL,
  source_db TEXT NOT NULL,
  engine_fingerprint TEXT NOT NULL,
  check_rule_sha256 TEXT NOT NULL,
  sharding_sha256 TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ok', 'warning', 'error')),
  errors_json TEXT NOT NULL,
  warnings_json TEXT NOT NULL,
  tenant_code INTEGER NOT NULL,
  checked_at TEXT NOT NULL,
  PRIMARY KEY (root_id, file_sha256, source_db, engine_fingerprint, check_rule_sha256, sharding_sha256)
);