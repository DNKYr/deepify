CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY, status TEXT NOT NULL CHECK(status IN ('starting','working','paused','ending','finished','interrupted')),
  planned_seconds INTEGER NOT NULL CHECK(planned_seconds > 0), focused_seconds INTEGER NOT NULL DEFAULT 0,
  paused_seconds INTEGER NOT NULL DEFAULT 0, intention TEXT, playlist_source_id TEXT,
  started_at INTEGER, finished_at INTEGER, finish_reason TEXT, interruption_cause TEXT,
  blocked_attempt_count INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS one_active_session ON sessions((1))
  WHERE status IN ('starting','working','paused','ending');
CREATE TABLE IF NOT EXISTS whitelist_entries (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('application','website')), value TEXT NOT NULL, normalized_value TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS music_sources (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('file','folder')), path TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL, last_scanned_at INTEGER);
CREATE TABLE IF NOT EXISTS music_tracks (path TEXT PRIMARY KEY, source_id TEXT NOT NULL REFERENCES music_sources(id) ON DELETE CASCADE, title TEXT NOT NULL, artist TEXT, album TEXT, available INTEGER NOT NULL DEFAULT 1, updated_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS queue_entries (position INTEGER PRIMARY KEY, track_path TEXT NOT NULL REFERENCES music_tracks(path) ON DELETE CASCADE, source_id TEXT REFERENCES music_sources(id) ON DELETE CASCADE);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS integration_state (name TEXT PRIMARY KEY, previous_state TEXT, cleanup_required INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL);
