//! Small SQLite adapter used by the prototype. SQLite remains backend-owned;
//! the CLI invocation is a portability fallback for this dependency-free build
//! environment and can be replaced by rusqlite without changing callers.
use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    Sqlite(String),
}
impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
pub struct SqliteStore {
    path: PathBuf,
}
impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let store = Self {
            path: path.as_ref().to_path_buf(),
        };
        store.migrate()?;
        Ok(store)
    }
    fn run(&self, sql: &str) -> Result<String, StorageError> {
        let out = Command::new("sqlite3")
            .arg("-batch")
            .arg(&self.path)
            .arg(sql)
            .output()?;
        if !out.status.success() {
            return Err(StorageError::Sqlite(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
    pub fn migrate(&self) -> Result<(), StorageError> {
        let sql = include_str!("../migrations/001_initial.sql");
        self.run(sql).map(|_| ())
    }
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let key = key.replace(char::from(39), "''");
        let value = value.replace(char::from(39), "''");
        self.run(&format!("INSERT INTO settings(key,value) VALUES('{key}','{value}') ON CONFLICT(key) DO UPDATE SET value=excluded.value;" )).map(|_| ())
    }
    pub fn setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let key = key.replace(char::from(39), "''");
        let value = self.run(&format!(
            "SELECT value FROM settings WHERE key='{key}' LIMIT 1;"
        ))?;
        let value = value.trim().to_string();
        Ok((!value.is_empty()).then_some(value))
    }
    pub fn insert_session(
        &self,
        id: &str,
        status: &str,
        planned: u64,
        now: u64,
    ) -> Result<(), StorageError> {
        let id = id.replace(char::from(39), "''");
        let status = status.replace(char::from(39), "''");
        self.run(&format!("INSERT INTO sessions(id,status,planned_seconds,created_at,updated_at) VALUES('{id}','{status}',{planned},{now},{now});")).map(|_| ())
    }
    pub fn active_session_count(&self) -> Result<u32, StorageError> {
        Ok(self.run("SELECT count(*) FROM sessions WHERE status IN ('starting','working','paused','ending');")?.trim().parse().unwrap_or(0))
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
        std::fs::remove_file(p).unwrap();
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
}
