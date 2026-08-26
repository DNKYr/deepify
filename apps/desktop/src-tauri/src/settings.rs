use crate::storage::{SqliteStore, StorageError};

pub const DEFAULT_DURATION_SECONDS: u64 = 25 * 60;
pub const DEFAULT_THEME: &str = "obsidian";

pub struct SettingsService<'a> {
    db: &'a SqliteStore,
}
impl<'a> SettingsService<'a> {
    pub fn new(db: &'a SqliteStore) -> Self {
        Self { db }
    }
    pub fn theme(&self) -> Result<String, StorageError> {
        Ok(self
            .db
            .setting("theme")?
            .unwrap_or_else(|| DEFAULT_THEME.into()))
    }
    pub fn set_theme(&self, theme: &str) -> Result<(), StorageError> {
        if matches!(theme, "obsidian" | "mist") {
            self.db.set_setting("theme", theme)
        } else {
            Err(StorageError::Sqlite("unsupported theme".into()))
        }
    }
    pub fn setup_complete(&self) -> Result<bool, StorageError> {
        Ok(self.db.setting("setup_complete")?.as_deref() == Some("true"))
    }
    pub fn complete_setup(&self) -> Result<(), StorageError> {
        self.db.set_setting("setup_complete", "true")
    }
}
