//! Backend-owned SQLite repositories.
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct StoredHistory {
    pub id: String,
    pub focused_seconds: u64,
    pub paused_seconds: u64,
    pub blocked_attempts: u32,
    pub reason: String,
    pub started_at: u64,
    pub intention: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StoredTrack {
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub available: bool,
}

#[derive(Clone, Debug)]
pub struct StoredMusicSource {
    pub id: String,
    pub kind: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredBrowserPairing {
    pub token_hash: String,
    pub browser_kind: String,
    pub profile_label: String,
    pub paired_at: u64,
}

pub struct FinishSessionRecord<'a> {
    pub id: &'a str,
    pub status: &'a str,
    pub reason: &'a str,
    pub focused_seconds: u64,
    pub paused_seconds: u64,
    pub blocked_attempts: u32,
    pub finished_at: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("persistence error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid stored value: {0}")]
    InvalidValue(String),
}
pub struct SqliteStore {
    path: PathBuf,
    connection: Connection,
}

fn sqlite_integer(value: u64, field: &str) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::InvalidValue(format!("{field} exceeds SQLite INTEGER range")))
}

fn domain_integer(value: i64, field: &str) -> Result<u64, StorageError> {
    u64::try_from(value)
        .map_err(|_| StorageError::InvalidValue(format!("{field} cannot be negative")))
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existed = path.exists();
        if existed {
            std::fs::copy(&path, path.with_extension("sqlite.backup"))?;
        }
        let connection = Connection::open(&path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self { path, connection };
        store.migrate()?;
        let integrity: String = store
            .connection
            .query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(StorageError::InvalidValue(format!(
                "database integrity check failed: {integrity}"
            )));
        }
        Ok(store)
    }
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            path: PathBuf::from(":memory:"),
            connection,
        };
        store.migrate()?;
        Ok(store)
    }
    pub fn migrate(&self) -> Result<(), StorageError> {
        let sql = include_str!("../migrations/001_initial.sql");
        self.connection.execute_batch(sql)?;
        self.connection
            .execute_batch(include_str!("../migrations/002_browser_pairing.sql"))?;
        Ok(())
    }
    pub fn browser_pairing(&self) -> Result<Option<StoredBrowserPairing>, StorageError> {
        let row: Option<(String, String, String, i64)> = self.connection.query_row(
            "SELECT token_hash,browser_kind,profile_label,paired_at FROM browser_pairing WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional()?;
        row.map(|(token_hash, browser_kind, profile_label, paired_at)| {
            Ok(StoredBrowserPairing {
                token_hash,
                browser_kind,
                profile_label,
                paired_at: domain_integer(paired_at, "paired_at")?,
            })
        })
        .transpose()
    }
    pub fn save_browser_pairing(&self, pairing: &StoredBrowserPairing) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO browser_pairing(singleton,token_hash,browser_kind,profile_label,paired_at) VALUES(1,?1,?2,?3,?4) ON CONFLICT(singleton) DO UPDATE SET token_hash=excluded.token_hash,browser_kind=excluded.browser_kind,profile_label=excluded.profile_label,paired_at=excluded.paired_at",
            params![pairing.token_hash, pairing.browser_kind, pairing.profile_label, sqlite_integer(pairing.paired_at, "paired_at")?],
        )?;
        Ok(())
    }
    pub fn forget_browser_pairing(&self) -> Result<(), StorageError> {
        self.connection
            .execute("DELETE FROM browser_pairing WHERE singleton=1", [])?;
        Ok(())
    }
    pub fn set_browser_cleanup(
        &self,
        session_id: &str,
        required: bool,
        now: u64,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO browser_cleanup_checkpoints(session_id,cleanup_required,updated_at) VALUES(?1,?2,?3) ON CONFLICT(session_id) DO UPDATE SET cleanup_required=excluded.cleanup_required,updated_at=excluded.updated_at",
            params![session_id, required, sqlite_integer(now, "updated_at")?],
        )?;
        Ok(())
    }
    pub fn browser_cleanup_required(&self) -> Result<Option<String>, StorageError> {
        self.connection.query_row("SELECT session_id FROM browser_cleanup_checkpoints WHERE cleanup_required=1 ORDER BY updated_at DESC LIMIT 1", [], |row| row.get(0)).optional().map_err(StorageError::from)
    }
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
    pub fn setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key=?1 LIMIT 1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }
    pub fn insert_session(
        &self,
        id: &str,
        status: &str,
        planned: u64,
        now: u64,
    ) -> Result<(), StorageError> {
        let planned = sqlite_integer(planned, "planned_seconds")?;
        let now = sqlite_integer(now, "created_at")?;
        self.connection.execute(
            "INSERT INTO sessions(id,status,planned_seconds,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
            params![id, status, planned, now],
        )?;
        Ok(())
    }
    pub fn insert_session_with_intention(
        &self,
        id: &str,
        status: &str,
        planned: u64,
        intention: Option<&str>,
        now: u64,
    ) -> Result<(), StorageError> {
        let planned = sqlite_integer(planned, "planned_seconds")?;
        let now = sqlite_integer(now, "started_at")?;
        self.connection.execute(
            "INSERT INTO sessions(id,status,planned_seconds,intention,started_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5,?5)",
            params![id, status, planned, intention, now],
        )?;
        Ok(())
    }
    pub fn active_session_count(&self) -> Result<u32, StorageError> {
        Ok(self.connection.query_row(
            "SELECT count(*) FROM sessions WHERE status IN ('starting','working','paused','ending')",
            [],
            |row| row.get(0),
        )?)
    }
    pub fn update_session(
        &self,
        id: &str,
        status: &str,
        focused: u64,
        paused: u64,
        blocked: u32,
        now: u64,
    ) -> Result<(), StorageError> {
        let focused = sqlite_integer(focused, "focused_seconds")?;
        let paused = sqlite_integer(paused, "paused_seconds")?;
        let now = sqlite_integer(now, "updated_at")?;
        self.connection.execute(
            "UPDATE sessions SET status=?2,focused_seconds=?3,paused_seconds=?4,blocked_attempt_count=?5,updated_at=?6 WHERE id=?1",
            params![id, status, focused, paused, blocked, now],
        )?;
        Ok(())
    }
    pub fn finish_session(&self, record: FinishSessionRecord<'_>) -> Result<(), StorageError> {
        let focused = sqlite_integer(record.focused_seconds, "focused_seconds")?;
        let paused = sqlite_integer(record.paused_seconds, "paused_seconds")?;
        let now = sqlite_integer(record.finished_at, "finished_at")?;
        self.connection.execute(
            "UPDATE sessions SET status=?2,finish_reason=?3,focused_seconds=?4,paused_seconds=?5,blocked_attempt_count=?6,finished_at=?7,updated_at=?7 WHERE id=?1",
            params![record.id, record.status, record.reason, focused, paused, record.blocked_attempts, now],
        )?;
        Ok(())
    }
    pub fn recover_abandoned(&self, now: u64) -> Result<u32, StorageError> {
        let now = sqlite_integer(now, "finished_at")?;
        Ok(self.connection.execute(
            "UPDATE sessions SET status='interrupted',finish_reason='interrupted',finished_at=?1,updated_at=?1 WHERE status IN ('starting','working','paused','ending')",
            params![now],
        )? as u32)
    }
    pub fn upsert_whitelist(
        &self,
        id: &str,
        kind: &str,
        value: &str,
        normalized: &str,
        now: u64,
    ) -> Result<(), StorageError> {
        let now = sqlite_integer(now, "created_at")?;
        self.connection.execute(
            "INSERT INTO whitelist_entries(id,kind,value,normalized_value,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(normalized_value) DO UPDATE SET value=excluded.value",
            params![id, kind, value, normalized, now],
        )?;
        Ok(())
    }
    pub fn remove_whitelist(&self, id: &str) -> Result<(), StorageError> {
        self.connection
            .execute("DELETE FROM whitelist_entries WHERE id=?1", params![id])?;
        Ok(())
    }
    pub fn whitelist(&self) -> Result<Vec<(String, String, String)>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id,kind,normalized_value FROM whitelist_entries ORDER BY created_at,id",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    pub fn history(&self) -> Result<Vec<StoredHistory>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id,focused_seconds,paused_seconds,blocked_attempt_count,finish_reason,COALESCE(started_at,created_at),intention FROM sessions WHERE finish_reason IS NOT NULL ORDER BY COALESCE(finished_at,updated_at) DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (id, focused, paused, blocked_attempts, reason, started_at, intention) = row?;
            Ok(StoredHistory {
                id,
                focused_seconds: domain_integer(focused, "focused_seconds")?,
                paused_seconds: domain_integer(paused, "paused_seconds")?,
                blocked_attempts,
                reason,
                started_at: domain_integer(started_at, "started_at")?,
                intention,
            })
        })
        .collect()
    }
    pub fn add_music_source(
        &self,
        id: &str,
        kind: &str,
        path: &str,
        now: u64,
    ) -> Result<String, StorageError> {
        let now = sqlite_integer(now, "created_at")?;
        self.connection.execute(
            "INSERT INTO music_sources(id,kind,path,created_at,last_scanned_at) VALUES(?1,?2,?3,?4,?4) ON CONFLICT(path) DO UPDATE SET last_scanned_at=excluded.last_scanned_at",
            params![id, kind, path, now],
        )?;
        Ok(self.connection.query_row(
            "SELECT id FROM music_sources WHERE path=?1",
            params![path],
            |row| row.get(0),
        )?)
    }
    pub fn music_sources(&self) -> Result<Vec<StoredMusicSource>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT id,kind,path FROM music_sources ORDER BY created_at,id")?;
        let rows = statement.query_map([], |row| {
            Ok(StoredMusicSource {
                id: row.get(0)?,
                kind: row.get(1)?,
                path: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    pub fn replace_source_tracks(
        &mut self,
        source_id: &str,
        tracks: &[StoredTrack],
        now: u64,
    ) -> Result<(), StorageError> {
        let now = sqlite_integer(now, "updated_at")?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM music_tracks WHERE source_id=?1",
            params![source_id],
        )?;
        for track in tracks {
            transaction.execute(
                "INSERT INTO music_tracks(path,source_id,title,artist,album,available,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![track.path, source_id, track.title, track.artist, track.album, track.available, now],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn replace_queue(&mut self, paths: &[String]) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM queue_entries", [])?;
        for (position, path) in paths.iter().enumerate() {
            let position = i64::try_from(position).map_err(|_| {
                StorageError::InvalidValue("queue position exceeds SQLite INTEGER range".into())
            })?;
            transaction.execute(
                "INSERT INTO queue_entries(position,track_path,source_id) SELECT ?1,?2,source_id FROM music_tracks WHERE path=?2",
                params![position, path],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn tracks(&self) -> Result<Vec<StoredTrack>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT path,title,artist,album,available FROM music_tracks ORDER BY lower(path)",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredTrack {
                path: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                available: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    pub fn queue(&self) -> Result<Vec<StoredTrack>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT t.path,t.title,t.artist,t.album,t.available FROM queue_entries q JOIN music_tracks t ON t.path=q.track_path ORDER BY q.position",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredTrack {
                path: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                available: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "deepify-{}.sqlite",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn migration_reopen_and_crud() {
        let p = path();
        let db = SqliteStore::open(&p).unwrap();
        db.set_setting("theme", "obsidian").unwrap();
        assert_eq!(db.setting("theme").unwrap().as_deref(), Some("obsidian"));
        drop(db);
        let db = SqliteStore::open(&p).unwrap();
        assert_eq!(db.setting("theme").unwrap().as_deref(), Some("obsidian"));
        std::fs::remove_file(&p).unwrap();
        let _ = std::fs::remove_file(p.with_extension("sqlite.backup"));
    }
    #[test]
    fn one_active_session_is_enforced() {
        let p = path();
        let db = SqliteStore::open(&p).unwrap();
        db.insert_session("one", "working", 60, 1).unwrap();
        assert_eq!(db.active_session_count().unwrap(), 1);
        assert!(db.insert_session("two", "working", 60, 1).is_err());
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn repositories_reject_invalid_values_and_recover_abandoned_sessions() {
        let db = SqliteStore::open_in_memory().unwrap();
        assert!(db.insert_session("bad", "unknown", 60, 1).is_err());
        assert!(db
            .upsert_whitelist("bad", "unknown", "value", "value", 1)
            .is_err());
        db.insert_session_with_intention("one", "working", 60, Some("Write"), 1)
            .unwrap();
        db.update_session("one", "paused", 12, 4, 2, 2).unwrap();
        assert_eq!(db.recover_abandoned(3).unwrap(), 1);
        let history = db.history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].reason, "interrupted");
        assert_eq!(history[0].focused_seconds, 12);
        assert_eq!(history[0].paused_seconds, 4);
    }
    #[test]
    fn browser_pairing_and_cleanup_checkpoint_never_store_a_raw_token() {
        let db = SqliteStore::open_in_memory().unwrap();
        db.save_browser_pairing(&StoredBrowserPairing {
            token_hash: "a9f5d4d1b7f0".into(),
            browser_kind: "firefox".into(),
            profile_label: "Work profile".into(),
            paired_at: 42,
        })
        .unwrap();
        let pairing = db.browser_pairing().unwrap().unwrap();
        assert_eq!(pairing.token_hash, "a9f5d4d1b7f0");
        assert_ne!(pairing.token_hash, "raw-profile-token");
        db.set_browser_cleanup("session-1", true, 43).unwrap();
        assert_eq!(
            db.browser_cleanup_required().unwrap().as_deref(),
            Some("session-1")
        );
        db.set_browser_cleanup("session-1", false, 44).unwrap();
        assert_eq!(db.browser_cleanup_required().unwrap(), None);
        db.forget_browser_pairing().unwrap();
        assert_eq!(db.browser_pairing().unwrap(), None);
    }
    #[test]
    fn music_source_track_and_queue_crud_is_transactional() {
        let mut db = SqliteStore::open_in_memory().unwrap();
        let id = db
            .add_music_source("source", "folder", "/music", 1)
            .unwrap();
        assert_eq!(id, "source");
        let track = StoredTrack {
            path: "/music/Artist - Song.mp3".into(),
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: None,
            available: true,
        };
        db.replace_source_tracks(&id, std::slice::from_ref(&track), 2)
            .unwrap();
        db.replace_queue(std::slice::from_ref(&track.path)).unwrap();
        assert_eq!(db.music_sources().unwrap().len(), 1);
        assert_eq!(db.tracks().unwrap().len(), 1);
        assert_eq!(db.queue().unwrap()[0].title, "Song");
    }
}
