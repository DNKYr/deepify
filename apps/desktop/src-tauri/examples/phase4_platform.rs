//! Target-desktop validation. Only windows owned by this disposable fixture
//! may receive a close/focus request, even if other apps open during the test.
use deepify::{
    browser::BrowserBroker,
    domain::FinishReason,
    niri::{NiriAdapter, NiriCommands, NiriIpc, Window},
    noctalia::{NoctaliaAdapter, NoctaliaCommands, NoctaliaIpc},
    services::{
        FakeClock, ProductionBrowserRestrictionStep, RestrictionCoordinator, RestrictionStep,
        SessionService,
    },
    storage::SqliteStore,
};
use std::{
    io::Write,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn wait_for(label: &str, mut check: impl FnMut() -> Result<bool, String>) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        if check()? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    Err(format!("timed out waiting for {label}"))
}

struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct RestoreDnd(bool);
impl Drop for RestoreDnd {
    fn drop(&mut self) {
        let _ = NoctaliaIpc.set_dnd(self.0);
    }
}
struct GuardedIpc {
    app_id: String,
    closed: Arc<Mutex<Vec<u64>>>,
    focus_target: Option<u64>,
    focused: Arc<Mutex<Vec<u64>>>,
}
impl NiriCommands for GuardedIpc {
    fn windows(&mut self) -> Result<Vec<Window>, String> {
        NiriIpc.windows()
    }
    fn close(&mut self, id: u64) -> Result<(), String> {
        if !self
            .windows()?
            .iter()
            .any(|w| w.id == id && w.app_id.as_deref() == Some(self.app_id.as_str()))
        {
            return Err("Validation refused to close a non-fixture window".into());
        }
        self.closed.lock().unwrap().push(id);
        NiriIpc.close(id)
    }
    fn focus(&mut self, id: u64) -> Result<(), String> {
        if let Some(target) = self.focus_target {
            if id != target {
                return Err("Validation refused to focus a non-fixture window".into());
            }
            NiriIpc.focus(id)?;
            self.focused.lock().unwrap().push(id);
        }
        Ok(())
    }
}

fn fixture(app_id: &str, refuse: bool) {
    use gtk::prelude::*;
    gtk::glib::set_prgname(Some(app_id));
    deepify::display::initialize().unwrap();
    let app = gtk::Application::new(Some(app_id), gtk::gio::ApplicationFlags::NON_UNIQUE);
    app.connect_activate(move |app| {
        for _ in 0..2 {
            let window = gtk::ApplicationWindow::new(app);
            window.set_title("Deepify disposable validation window");
            window.set_default_size(280, 100);
            window.add(&gtk::Label::new(Some(if refuse {
                "Validation: this window refuses cooperative closure."
            } else {
                "Validation: this window permits cooperative closure."
            })));
            if refuse {
                window.connect_delete_event(|_, _| gtk::glib::Propagation::Stop);
            }
            window.show_all();
        }
    });
    app.run_with_args(&["deepify-platform-fixture"]);
}

fn niri_validation() -> Result<(), String> {
    for refuse in [false, true] {
        let app_id = format!(
            "com.deepify.validation.p{}.{}",
            std::process::id(),
            if refuse { "refuse" } else { "close" }
        );
        let closed = Arc::new(Mutex::new(Vec::new()));
        let mut adapter = NiriAdapter::with_commands(GuardedIpc {
            app_id: app_id.clone(),
            closed: closed.clone(),
            focus_target: None,
            focused: Arc::new(Mutex::new(Vec::new())),
        });
        let original = NiriIpc.windows()?;
        adapter
            .policy
            .configure(original.iter().filter_map(|w| w.app_id.clone()).collect());
        adapter.preflight()?;
        adapter.activate()?;
        adapter.activate()?; // A second activation must not spawn another stream.
        let mut fixture = Fixture(
            Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .args([
                    "--fixture",
                    &app_id,
                    if refuse { "refuse" } else { "close" },
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?,
        );
        let started = Instant::now();
        let mut blocked = 0;
        while started.elapsed() < Duration::from_secs(8) {
            blocked += adapter.poll()?;
            if closed.lock().unwrap().len() >= 2 {
                // Consume duplicate/change/close events too, not just the first request.
                for _ in 0..10 {
                    std::thread::sleep(Duration::from_millis(30));
                    blocked += adapter.poll()?;
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        adapter.refresh()?;
        assert_eq!(
            closed.lock().unwrap().len(),
            2,
            "one close per fixture window"
        );
        assert_eq!(blocked, 2, "one attempt per fixture window");
        assert_eq!(!adapter.policy.blocked_apps().is_empty(), refuse);
        adapter.deactivate()?;
        adapter.deactivate()?;
        // A still-open refused fixture must block a subsequent preflight.
        assert_eq!(adapter.preflight().is_err(), refuse);
        let _ = fixture.0.kill();
        let _ = fixture.0.wait();
        println!(
            "PASS: Niri {} (2 windows; deduplicated; real event stream)",
            if refuse {
                "cooperative refusal and preflight"
            } else {
                "targeted close"
            }
        );
    }
    Ok(())
}

fn niri_focus_validation() -> Result<(), String> {
    let allowed_app = format!("com.deepify.validation.p{}.allowed", std::process::id());
    let blocked_app = format!("com.deepify.validation.p{}.blocked", std::process::id());
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let _allowed = Fixture(
        Command::new(&executable)
            .args(["--fixture", &allowed_app, "close"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?,
    );
    let mut target = None;
    wait_for("focused allowed fixture", || {
        target = NiriIpc
            .windows()?
            .iter()
            .find(|w| w.app_id.as_deref() == Some(&allowed_app) && w.is_focused)
            .map(|w| w.id);
        Ok(target.is_some())
    })?;
    let focused = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = NiriAdapter::with_commands(GuardedIpc {
        app_id: blocked_app.clone(),
        closed: Arc::new(Mutex::new(Vec::new())),
        focus_target: target,
        focused: focused.clone(),
    });
    adapter.policy.configure(
        NiriIpc
            .windows()?
            .iter()
            .filter_map(|w| w.app_id.clone())
            .collect(),
    );
    adapter.activate()?;
    let _blocked = Fixture(
        Command::new(&executable)
            .args(["--fixture", &blocked_app, "close"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?,
    );
    wait_for("focus restored to allowed fixture", || {
        adapter.poll()?;
        Ok(focused.lock().unwrap().contains(&target.unwrap())
            && NiriIpc
                .windows()?
                .iter()
                .any(|w| Some(w.id) == target && w.is_focused))
    })?;
    adapter.deactivate()?;
    println!("PASS: Niri focus restored to the last allowed disposable window");
    Ok(())
}

fn dnd_validation() -> Result<(), String> {
    let original = NoctaliaIpc.read_dnd()?;
    let _restore_on_error = RestoreDnd(original);
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let result: Result<(), String> = (|| {
        for prior in [false, true] {
            NoctaliaIpc.set_dnd(prior)?;
            let database = Arc::new(Mutex::new(
                SqliteStore::open(directory.path().join("dnd.sqlite"))
                    .map_err(|e| e.to_string())?,
            ));
            let mut adapter = NoctaliaAdapter::new(database.clone());
            adapter.preflight()?;
            adapter.activate()?;
            assert!(NoctaliaIpc.read_dnd()?);
            assert_eq!(
                database.lock().unwrap().dnd_cleanup_required().unwrap(),
                Some(prior)
            );
            // Simulate process loss: a new adapter must recover the persisted prior state.
            drop(adapter);
            let mut recovered = NoctaliaAdapter::new(database);
            assert!(recovered.preflight().is_err());
            recovered.deactivate()?;
            assert_eq!(NoctaliaIpc.read_dnd()?, prior);
            assert!(!recovered.cleanup_required()?);
            println!("PASS: real Noctalia enable and restart recovery with prior DND={prior}");
        }
        Ok(())
    })();
    // Always attempt to restore the actual user's initial setting.
    let restore = NoctaliaIpc.set_dnd(original);
    result?;
    restore?;
    if NoctaliaIpc.read_dnd()? != original {
        return Err("Validation did not restore initial DND".into());
    }
    Ok(())
}

fn session_validation() -> Result<(), String> {
    let original = NoctaliaIpc.read_dnd()?;
    let _restore_on_error = RestoreDnd(original);
    let runtime = std::env::var("DEEPIFY_BROWSER_RUNTIME")
        .map_err(|_| "DEEPIFY_BROWSER_RUNTIME is required")?;
    let runtime = std::path::Path::new(&runtime);
    let database = Arc::new(Mutex::new(
        SqliteStore::open(runtime.join("platform.sqlite")).map_err(|e| e.to_string())?,
    ));
    let broker = BrowserBroker::start_in(runtime, None).map_err(|e| e.to_string())?;
    wait_for("pending profile", || {
        Ok(broker.snapshot().state == "pending_pair")
    })?;
    broker.accept_pending().map_err(|e| e.to_string())?;
    wait_for("healthy profile", || Ok(broker.check_health().is_ok()))?;

    let app_id = format!("com.deepify.validation.p{}.session", std::process::id());
    let closed = Arc::new(Mutex::new(Vec::new()));
    let mut applications = NiriAdapter::with_commands(GuardedIpc {
        app_id: app_id.clone(),
        closed: closed.clone(),
        focus_target: None,
        focused: Arc::new(Mutex::new(Vec::new())),
    });
    applications.policy.configure(
        NiriIpc
            .windows()?
            .into_iter()
            .filter_map(|w| w.app_id)
            .collect(),
    );
    let mut browser = ProductionBrowserRestrictionStep::new(broker.clone());
    browser.configure("phase4-integration".into(), vec![], 600);
    let restriction = RestrictionCoordinator {
        browser,
        applications,
        do_not_disturb: NoctaliaAdapter::new(database.clone()),
    };
    let clock = FakeClock::new(1);
    let mut service = SessionService::new(clock.clone(), restriction);
    service.start(600, None).map_err(|e| e.to_string())?;
    service.current.as_mut().unwrap().id = "phase4-integration".into();
    clock.advance(2);
    service.pause().map_err(|e| e.to_string())?;
    broker
        .sync_timer_state("phase4-integration", "paused", 598)
        .map_err(|e| e.to_string())?;
    assert!(NoctaliaIpc.read_dnd()?);
    let _fixture = Fixture(
        Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .args(["--fixture", &app_id, "close"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?,
    );
    wait_for("blocked windows while paused", || {
        let count = service.restriction.applications.poll()?;
        for _ in 0..count {
            service.blocked_attempt();
        }
        Ok(closed.lock().unwrap().len() == 2)
    })?;
    assert_eq!(service.current.as_ref().unwrap().blocked_attempts, 2);
    println!("PHASE4_PLATFORM_READY");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut command = String::new();
    std::io::stdin()
        .read_line(&mut command)
        .map_err(|e| e.to_string())?;
    clock.advance(4);
    let summary = match command.trim() {
        "finish" => service.finish(FinishReason::EndedEarly),
        "failure" => {
            // Induce an actual DND health failure and route it through the same
            // runtime_failure coordinator path used by the desktop worker.
            NoctaliaIpc.set_dnd(false)?;
            assert!(service.restriction.do_not_disturb.check_health().is_err());
            service.runtime_failure("Noctalia Do Not Disturb")
        }
        _ => return Err("expected finish or failure".into()),
    }
    .map_err(|e| e.to_string())?;
    assert!(summary.cleanup_complete);
    assert_eq!(summary.focused.as_secs(), 2);
    assert_eq!(summary.paused.as_secs(), 4);
    assert_eq!(NoctaliaIpc.read_dnd()?, original);
    assert!(!service.restriction.do_not_disturb.cleanup_required()?);
    assert_eq!(broker.snapshot().state, "healthy_idle");
    println!("PHASE4_PLATFORM_CLEANUP_COMPLETE");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("--fixture") {
        fixture(&args[2], args[3] == "refuse");
        return Ok(());
    }
    match args.get(1).map(String::as_str) {
        Some("niri") => niri_validation(),
        Some("focus") => niri_focus_validation(),
        Some("dnd") => dnd_validation(),
        Some("session") => session_validation(),
        _ => Err("Usage: phase4_platform niri|focus|dnd|session".into()),
    }
}
