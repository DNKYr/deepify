CREATE TABLE IF NOT EXISTS dnd_cleanup_checkpoint (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  previous_enabled INTEGER NOT NULL CHECK(previous_enabled IN (0,1))
);
CREATE TABLE IF NOT EXISTS session_cleanup_outcomes (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id),
  complete INTEGER NOT NULL CHECK(complete IN (0,1))
);
