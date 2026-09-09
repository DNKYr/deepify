CREATE TABLE IF NOT EXISTS browser_pairing (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  token_hash TEXT NOT NULL,
  browser_kind TEXT NOT NULL CHECK(browser_kind IN ('firefox', 'zen')),
  profile_label TEXT NOT NULL,
  paired_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS browser_cleanup_checkpoints (
  session_id TEXT PRIMARY KEY,
  cleanup_required INTEGER NOT NULL DEFAULT 1 CHECK(cleanup_required IN (0, 1)),
  updated_at INTEGER NOT NULL
);
