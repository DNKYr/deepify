//! Validation-only broker lifecycle harness. It is not installed with Deepify.
use deepify::browser::{BrowserBroker, BrowserPairing};
use std::{
    env,
    io::Write,
    thread,
    time::{Duration, Instant},
};

const SESSION_ID: &str = "phase3-lifecycle";

fn wait_until(label: &str, predicate: impl Fn() -> bool) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if predicate() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("timed out waiting for {label}"))
}

fn main() -> Result<(), String> {
    let mode = env::args()
        .nth(1)
        .ok_or("pass desktop-loss, browser-restart, startup-first, or startup-recover")?;
    if !matches!(
        mode.as_str(),
        "desktop-loss" | "browser-restart" | "startup-first" | "startup-recover"
    ) {
        return Err("pass desktop-loss, browser-restart, startup-first, or startup-recover".into());
    }
    if mode == "startup-recover" {
        let record = env::var("DEEPIFY_PAIRING_RECORD")
            .map_err(|_| "DEEPIFY_PAIRING_RECORD is required for startup-recover")?;
        let token_hash = std::fs::read_to_string(record).map_err(|error| error.to_string())?;
        let broker = BrowserBroker::start(Some(BrowserPairing {
            token_hash: token_hash.trim().into(),
            browser_kind: "firefox".into(),
            profile_label: "Lifecycle profile".into(),
            paired_at: 1,
        }))
        .map_err(|error| error.to_string())?;
        wait_until("paired browser reconnect", || broker.check_health().is_ok())?;
        broker
            .recover_cleanup(SESSION_ID)
            .map_err(|error| error.to_string())?;
        println!("PHASE3_STARTUP_RECOVERY_COMPLETE");
        std::io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    let broker = BrowserBroker::start(None).map_err(|error| error.to_string())?;
    wait_until("pending browser pairing", || {
        broker.snapshot().state == "pending_pair"
    })?;
    let pairing = broker.accept_pending().map_err(|error| error.to_string())?;
    wait_until("paired browser health", || broker.check_health().is_ok())?;
    broker
        .start_session(SESSION_ID, Vec::new(), "working", 600)
        .map_err(|error| error.to_string())?;
    if mode == "startup-first" {
        let record = env::var("DEEPIFY_PAIRING_RECORD")
            .map_err(|_| "DEEPIFY_PAIRING_RECORD is required for startup-first")?;
        std::fs::write(record, pairing.token_hash).map_err(|error| error.to_string())?;
    }
    println!("PHASE3_LIFECYCLE_READY");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;

    if mode == "desktop-loss" || mode == "startup-first" {
        thread::sleep(Duration::from_secs(3));
        return Ok(());
    }

    wait_until("browser disconnect", || broker.check_health().is_err())?;
    println!("PHASE3_BROWSER_DISCONNECTED");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    wait_until("idle reconnect", || {
        broker.snapshot().state == "healthy_idle" && broker.check_health().is_ok()
    })?;
    println!("PHASE3_BROWSER_RECONNECTED_IDLE");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    Ok(())
}
