//! Noctalia DND changes are write-ahead checkpointed and verified by readback.
use crate::{
    platform_command,
    services::{DoNotDisturbAdapter, RestrictionStep},
    storage::SqliteStore,
};
use std::sync::{Arc, Mutex};

pub trait NoctaliaCommands: Send {
    fn read_dnd(&mut self) -> Result<bool, String>;
    fn set_dnd(&mut self, enabled: bool) -> Result<(), String>;
}

#[derive(Default)]
pub struct NoctaliaIpc;
impl NoctaliaCommands for NoctaliaIpc {
    fn read_dnd(&mut self) -> Result<bool, String> {
        let output = platform_command::run("noctalia-shell", &["ipc", "call", "state", "all"])?;
        // Only extract this boolean. Do not retain settings, notifications or other state.
        let value: serde_json::Value =
            serde_json::from_slice(&output).map_err(|_| "Noctalia returned invalid state")?;
        value
            .get("state")
            .and_then(|s| s.get("doNotDisturb"))
            .and_then(|v| v.as_bool())
            .ok_or_else(|| {
                "Noctalia state is missing doNotDisturb; check the supported IPC version".into()
            })
    }
    fn set_dnd(&mut self, enabled: bool) -> Result<(), String> {
        platform_command::run(
            "noctalia-shell",
            &[
                "ipc",
                "call",
                "notifications",
                if enabled { "enableDND" } else { "disableDND" },
            ],
        )
        .map(|_| ())
    }
}

pub struct NoctaliaAdapter<C = NoctaliaIpc> {
    commands: C,
    database: Arc<Mutex<SqliteStore>>,
    active: bool,
    healthy: bool,
    pub detail: String,
    pub last_checked: u64,
}
impl NoctaliaAdapter {
    pub fn new(database: Arc<Mutex<SqliteStore>>) -> Self {
        Self::with_commands(NoctaliaIpc, database)
    }
}
impl<C: NoctaliaCommands> NoctaliaAdapter<C> {
    fn with_commands(commands: C, database: Arc<Mutex<SqliteStore>>) -> Self {
        Self {
            commands,
            database,
            active: false,
            healthy: false,
            detail: "Noctalia has not been checked".into(),
            last_checked: 0,
        }
    }
    pub fn cleanup_required(&self) -> Result<bool, String> {
        Ok(self
            .database
            .lock()
            .map_err(|_| "DND checkpoint lock failed")?
            .dnd_cleanup_required()
            .map_err(|_| "DND checkpoint read failed")?
            .is_some())
    }
    fn record(&mut self, result: Result<(), String>) -> Result<(), String> {
        self.healthy = result.is_ok();
        self.last_checked = crate::domain::now_seconds();
        self.detail = match &result {
            Ok(()) if self.active => "Do Not Disturb is enabled; previous state preserved".into(),
            Ok(()) => "Noctalia Do Not Disturb is available".into(),
            Err(error) => format!("Noctalia: {error}"),
        };
        result
    }
    pub fn check_health(&mut self) -> Result<(), String> {
        let result = self.commands.read_dnd().and_then(|enabled| {
            if self.active && !enabled {
                Err("Do Not Disturb was disabled during the session".into())
            } else {
                Ok(())
            }
        });
        self.record(result)
    }
    fn enable(&mut self) -> Result<(), String> {
        if self.active {
            return self.check_health();
        }
        if self.cleanup_required()? {
            return Err("DND restoration must finish before a new session".into());
        }
        let previous = self.commands.read_dnd()?;
        self.database
            .lock()
            .map_err(|_| "DND checkpoint lock failed")?
            .preserve_dnd(previous)
            .map_err(|_| "DND checkpoint could not be persisted")?;
        // The durable checkpoint precedes the first mutation, including an IPC failure
        // that may still have applied the change on the shell side.
        self.commands.set_dnd(true)?;
        if !self.commands.read_dnd()? {
            return Err("Do Not Disturb could not be enabled".into());
        }
        self.active = true;
        Ok(())
    }
    fn restore(&mut self) -> Result<(), String> {
        self.active = false;
        let previous = self
            .database
            .lock()
            .map_err(|_| "DND checkpoint lock failed")?
            .dnd_cleanup_required()
            .map_err(|_| "DND checkpoint read failed")?;
        if let Some(previous) = previous {
            self.commands.set_dnd(previous)?;
            if self.commands.read_dnd()? != previous {
                return Err("Previous Do Not Disturb state could not be restored".into());
            }
            self.database
                .lock()
                .map_err(|_| "DND checkpoint lock failed")?
                .clear_dnd_cleanup()
                .map_err(|_| "DND checkpoint could not be cleared")?;
        }
        Ok(())
    }
}
impl<C: NoctaliaCommands> RestrictionStep for NoctaliaAdapter<C> {
    fn component(&self) -> &'static str {
        "do_not_disturb"
    }
    fn preflight(&mut self) -> Result<(), String> {
        if self.cleanup_required()? {
            return self.record(Err("DND restoration remains pending".into()));
        }
        self.check_health()
    }
    fn activate(&mut self) -> Result<(), String> {
        let result = self.enable();
        self.record(result)
    }
    fn deactivate(&mut self) -> Result<(), String> {
        // No checkpoint means there is nothing to restore, not evidence of a live shell.
        if !self.cleanup_required()? {
            self.active = false;
            return Ok(());
        }
        let result = self.restore();
        self.record(result)
    }
    fn healthy(&self) -> bool {
        self.healthy
    }
}
impl<C: NoctaliaCommands> DoNotDisturbAdapter for NoctaliaAdapter<C> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct FakeNoctalia {
        enabled: bool,
        fail: bool,
        ignore_set: bool,
        changes: Vec<bool>,
    }
    impl NoctaliaCommands for FakeNoctalia {
        fn read_dnd(&mut self) -> Result<bool, String> {
            if self.fail {
                Err("shell disconnected".into())
            } else {
                Ok(self.enabled)
            }
        }
        fn set_dnd(&mut self, enabled: bool) -> Result<(), String> {
            if self.fail {
                return Err("shell disconnected".into());
            }
            self.changes.push(enabled);
            if !self.ignore_set {
                self.enabled = enabled;
            }
            Ok(())
        }
    }
    #[test]
    fn restores_both_prior_states_and_cleanup_is_idempotent() {
        for previous in [false, true] {
            let db = Arc::new(Mutex::new(SqliteStore::open_in_memory().unwrap()));
            let mut adapter = NoctaliaAdapter::with_commands(
                FakeNoctalia {
                    enabled: previous,
                    ..Default::default()
                },
                db.clone(),
            );
            adapter.preflight().unwrap();
            adapter.activate().unwrap();
            adapter.activate().unwrap();
            assert_eq!(
                db.lock().unwrap().dnd_cleanup_required().unwrap(),
                Some(previous)
            );
            assert!(adapter.commands.enabled);
            adapter.deactivate().unwrap();
            adapter.deactivate().unwrap();
            assert_eq!(adapter.commands.enabled, previous);
            assert_eq!(adapter.commands.changes, [true, previous]);
            assert!(!adapter.cleanup_required().unwrap());
        }
    }
    #[test]
    fn failure_retains_original_checkpoint_and_retry_restores_it() {
        let db = Arc::new(Mutex::new(SqliteStore::open_in_memory().unwrap()));
        let mut adapter = NoctaliaAdapter::with_commands(FakeNoctalia::default(), db.clone());
        adapter.activate().unwrap();
        adapter.commands.fail = true;
        assert!(adapter.check_health().is_err());
        assert!(adapter.deactivate().is_err());
        assert_eq!(
            db.lock().unwrap().dnd_cleanup_required().unwrap(),
            Some(false)
        );
        adapter.commands.fail = false;
        assert!(adapter.activate().is_err());
        adapter.deactivate().unwrap();
        assert!(!adapter.commands.enabled);
        assert!(!adapter.cleanup_required().unwrap());
    }
    #[test]
    fn interrupted_process_reopens_checkpoint_without_resuming_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deepify.sqlite");
        {
            let db = Arc::new(Mutex::new(SqliteStore::open(&path).unwrap()));
            let mut adapter = NoctaliaAdapter::with_commands(FakeNoctalia::default(), db);
            adapter.activate().unwrap();
        }
        let db = Arc::new(Mutex::new(SqliteStore::open(&path).unwrap()));
        let mut recovered = NoctaliaAdapter::with_commands(
            FakeNoctalia {
                enabled: true,
                ..Default::default()
            },
            db,
        );
        assert!(recovered.preflight().is_err());
        recovered.deactivate().unwrap();
        assert!(!recovered.commands.enabled);
        recovered.preflight().unwrap();
    }
    #[test]
    fn readback_detects_silent_noop_and_external_disable() {
        let db = Arc::new(Mutex::new(SqliteStore::open_in_memory().unwrap()));
        let mut adapter = NoctaliaAdapter::with_commands(
            FakeNoctalia {
                ignore_set: true,
                ..Default::default()
            },
            db,
        );
        assert!(adapter.activate().is_err());
        assert!(adapter.cleanup_required().unwrap());
        adapter.deactivate().unwrap();
        adapter.commands.ignore_set = false;
        adapter.activate().unwrap();
        adapter.commands.enabled = false;
        assert!(adapter.check_health().is_err());
    }
}
