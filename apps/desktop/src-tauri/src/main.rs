#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deepify::{
    audio::{watch_folder, AudioEngine, PlaybackHealth, RodioPlayer, Track},
    browser::{BrowserBroker, BrowserPairing},
    domain::{
        implicit_app_allowed, normalize_app_id, normalize_website, validate_duration,
        website_allowed, FinishReason, SessionState, WhitelistEntry,
    },
    services::{
        MockFailure, ProductionRestrictionCoordinator, RestrictionStep, SessionService, SystemClock,
    },
    storage::{FinishSessionRecord, SqliteStore, StoredBrowserPairing, StoredHistory, StoredTrack},
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

struct AppState {
    diagnostic_id: String,
    sessions: Mutex<SessionService<SystemClock, ProductionRestrictionCoordinator>>,
    browser: BrowserBroker,
    database: Mutex<SqliteStore>,
    audio: Mutex<AudioEngine>,
    player: Mutex<Option<RodioPlayer>>,
    blocked_apps: Mutex<Vec<String>>,
    watchers: Mutex<Vec<notify::RecommendedWatcher>>,
    summary: Mutex<Option<SessionSummaryDto>>,
    last_browser_health: Mutex<Instant>,
    pending_browser_cleanup: Mutex<Option<String>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionSnapshotDto {
    state: &'static str,
    remaining_seconds: u64,
    focused_seconds: u64,
    paused_seconds: u64,
    blocked_attempts: u32,
    restrictions_active: bool,
    intention: Option<String>,
    latest_notice: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionSummaryDto {
    id: String,
    focused_seconds: u64,
    paused_seconds: u64,
    blocked_attempts: u32,
    reason: &'static str,
    cleanup_complete: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDto {
    default_duration_seconds: u64,
    theme: String,
    notifications: bool,
    setup_complete: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthDto {
    component: &'static str,
    status: &'static str,
    last_checked: u64,
    detail: String,
}
#[derive(Clone, Serialize)]
struct WhitelistDto {
    id: String,
    kind: String,
    value: String,
}
#[derive(Clone, Serialize)]
struct TrackDto {
    path: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    available: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackDto {
    current_index: Option<usize>,
    playing: bool,
    volume: u8,
    health: &'static str,
    output: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryDto {
    id: String,
    focused_seconds: u64,
    paused_seconds: u64,
    blocked_attempts: u32,
    reason: String,
    cleanup_complete: bool,
    started_at: u64,
    intention: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshotDto {
    diagnostic_id: String,
    session: SessionSnapshotDto,
    settings: SettingsDto,
    health: Vec<HealthDto>,
    whitelist: Vec<WhitelistDto>,
    tracks: Vec<TrackDto>,
    queue: Vec<TrackDto>,
    history: Vec<HistoryDto>,
    playback: PlaybackDto,
    blocked_apps: Vec<String>,
    summary: Option<SessionSummaryDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WhitelistTestDto {
    allowed: bool,
    detail: &'static str,
}

fn state_name(state: SessionState) -> &'static str {
    match state {
        SessionState::Starting => "starting",
        SessionState::Working => "working",
        SessionState::Paused => "paused",
        SessionState::Ending => "ending",
        SessionState::Finished | SessionState::Interrupted => "not_working",
    }
}
fn finish_name(reason: FinishReason) -> &'static str {
    reason.as_str()
}
fn stored_track(track: &Track) -> StoredTrack {
    StoredTrack {
        path: track.path.to_string_lossy().into_owned(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        available: track.available,
    }
}
fn track_dto(track: StoredTrack) -> TrackDto {
    TrackDto {
        path: track.path,
        title: track.title,
        artist: track.artist,
        album: track.album,
        available: track.available,
    }
}
fn history_dto(item: StoredHistory) -> HistoryDto {
    HistoryDto {
        id: item.id,
        focused_seconds: item.focused_seconds,
        paused_seconds: item.paused_seconds,
        blocked_attempts: item.blocked_attempts,
        reason: item.reason,
        cleanup_complete: true,
        started_at: item.started_at,
        intention: item.intention,
    }
}

fn snapshot(state: &AppState) -> Result<AppSnapshotDto, String> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    let session = if let Some(current) = sessions.current.as_ref() {
        let timer = sessions.snapshot();
        SessionSnapshotDto {
            state: state_name(timer.state),
            remaining_seconds: timer.remaining.as_secs(),
            focused_seconds: timer.focused.as_secs(),
            paused_seconds: timer.paused.as_secs(),
            blocked_attempts: timer.blocked_attempts,
            restrictions_active: timer.restrictions_active,
            intention: current.intention.clone(),
            latest_notice: None,
        }
    } else {
        SessionSnapshotDto {
            state: "not_working",
            remaining_seconds: 0,
            focused_seconds: 0,
            paused_seconds: 0,
            blocked_attempts: 0,
            restrictions_active: false,
            intention: None,
            latest_notice: None,
        }
    };
    let integration_health = [
        sessions.restriction.browser.healthy(),
        sessions.restriction.applications.healthy(),
        sessions.restriction.do_not_disturb.healthy(),
    ];
    drop(sessions);
    let browser_status = state.browser.snapshot();
    let database = state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    let theme = database
        .setting("theme")
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| "obsidian".into());
    let default_duration_seconds = database
        .setting("default_duration_seconds")
        .map_err(|error| error.to_string())?
        .and_then(|value| value.parse().ok())
        .unwrap_or(1500);
    let notifications = database
        .setting("notifications")
        .map_err(|error| error.to_string())?
        .as_deref()
        != Some("false");
    let setup_complete = database
        .setting("setup_complete")
        .map_err(|error| error.to_string())?
        .as_deref()
        == Some("true");
    let whitelist = database
        .whitelist()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(id, kind, value)| WhitelistDto { id, kind, value })
        .collect();
    let tracks = database
        .tracks()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(track_dto)
        .collect();
    let queue = database
        .queue()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(track_dto)
        .collect();
    let history = database
        .history()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(history_dto)
        .collect();
    drop(database);
    let (current_index, playing, volume, health) = {
        let audio = state
            .audio
            .lock()
            .map_err(|_| "audio lock poisoned".to_string())?;
        (
            audio.current,
            audio.playing,
            audio.volume,
            match audio.health {
                PlaybackHealth::Ready => "ready",
                PlaybackHealth::WaitingForOutputDevice => "waiting_for_output_device",
                PlaybackHealth::MissingFile => "missing_file",
            },
        )
    };
    let output = state
        .player
        .lock()
        .map_err(|_| "player lock poisoned".to_string())?
        .as_ref()
        .map(RodioPlayer::output_description);
    let playback = PlaybackDto {
        current_index,
        playing,
        volume,
        health,
        output,
    };
    let summary = state
        .summary
        .lock()
        .map_err(|_| "summary lock poisoned".to_string())?
        .clone();
    let blocked_apps = state
        .blocked_apps
        .lock()
        .map_err(|_| "blocked-app lock poisoned".to_string())?
        .clone();
    let now = deepify::domain::now_seconds();
    Ok(AppSnapshotDto {
        diagnostic_id: state.diagnostic_id.clone(),
        session,
        settings: SettingsDto {
            default_duration_seconds,
            theme,
            notifications,
            setup_complete,
        },
        health: vec![
            HealthDto {
                component: "Firefox/Zen profile",
                status: if integration_health[0] {
                    "healthy"
                } else {
                    "unhealthy"
                },
                last_checked: now,
                detail: match (&browser_status.browser_kind, &browser_status.profile_label) {
                    (Some(kind), Some(label)) => {
                        format!("{} · {} ({kind})", browser_status.detail, label)
                    }
                    _ => browser_status.detail.to_string(),
                },
            },
            HealthDto {
                component: "Niri application monitor",
                status: if integration_health[1] {
                    "healthy"
                } else {
                    "unhealthy"
                },
                last_checked: now,
                detail: if integration_health[1] {
                    "Simulated inventory".into()
                } else {
                    "Simulated preflight failure".into()
                },
            },
            HealthDto {
                component: "Noctalia DND",
                status: if integration_health[2] {
                    "healthy"
                } else {
                    "unhealthy"
                },
                last_checked: now,
                detail: if integration_health[2] {
                    "Simulated preservation".into()
                } else {
                    "Simulated preflight failure".into()
                },
            },
        ],
        whitelist,
        tracks,
        queue,
        history,
        playback,
        blocked_apps,
        summary,
    })
}

fn play_current_audio(state: &AppState) {
    let (path, volume) = {
        let Ok(mut audio) = state.audio.lock() else {
            return;
        };
        if audio.current.is_none() && !audio.queue.is_empty() {
            audio.current = Some(0);
        }
        let Some(track) = audio.current.and_then(|index| audio.queue.get(index)) else {
            audio.playing = false;
            return;
        };
        if !track.path.exists() {
            audio.health = PlaybackHealth::MissingFile;
            audio.playing = false;
            return;
        }
        (track.path.clone(), audio.volume)
    };
    let Ok(mut player) = state.player.lock() else {
        return;
    };
    if player.is_none() {
        *player = RodioPlayer::open_default().ok();
    }
    let Some(output) = player.as_ref() else {
        if let Ok(mut audio) = state.audio.lock() {
            audio.set_output_available(false);
        }
        return;
    };
    output.set_volume(volume);
    match output.play_file(&path) {
        Ok(()) => {
            if let Ok(mut audio) = state.audio.lock() {
                audio.set_output_available(true);
                audio.play();
            }
        }
        Err(_) => {
            if let Ok(mut audio) = state.audio.lock() {
                audio.health = PlaybackHealth::MissingFile;
                audio.pause();
            }
        }
    }
}

fn stop_audio(state: &AppState, fade: bool) {
    if let Ok(player) = state.player.lock() {
        if let Some(output) = player.as_ref() {
            if fade {
                output.fade_and_stop(Duration::from_millis(300));
            } else {
                output.stop();
            }
        }
    }
    if let Ok(mut audio) = state.audio.lock() {
        audio.pause();
    }
}
fn emit_snapshot(app: &AppHandle, state: &AppState) -> Result<AppSnapshotDto, String> {
    let value = snapshot(state)?;
    app.emit("app://snapshot", value.clone())
        .map_err(|error| error.to_string())?;
    Ok(value)
}
fn persist_current(state: &AppState) -> Result<(), String> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    if let Some(current) = sessions.current.as_ref() {
        state
            .database
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?
            .update_session(
                &current.id,
                state_name(current.state),
                current.focused.as_secs(),
                current.paused.as_secs(),
                current.blocked_attempts,
                deepify::domain::now_seconds(),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
fn persist_summary(state: &AppState, summary: &SessionSummaryDto) -> Result<(), String> {
    let database = state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    database
        .finish_session(FinishSessionRecord {
            id: &summary.id,
            status: if matches!(summary.reason, "interrupted" | "extension_or_app_crash") {
                "interrupted"
            } else {
                "finished"
            },
            reason: summary.reason,
            focused_seconds: summary.focused_seconds,
            paused_seconds: summary.paused_seconds,
            blocked_attempts: summary.blocked_attempts,
            finished_at: deepify::domain::now_seconds(),
        })
        .map_err(|error| error.to_string())?;
    if summary.cleanup_complete {
        database
            .set_browser_cleanup(&summary.id, false, deepify::domain::now_seconds())
            .map_err(|error| error.to_string())?;
        *state
            .pending_browser_cleanup
            .lock()
            .map_err(|_| "browser cleanup lock poisoned".to_string())? = None;
    } else {
        *state
            .pending_browser_cleanup
            .lock()
            .map_err(|_| "browser cleanup lock poisoned".to_string())? = Some(summary.id.clone());
    }
    Ok(())
}
fn resolve_browser_cleanup(state: &AppState) -> Result<(), String> {
    let session_id = state
        .pending_browser_cleanup
        .lock()
        .map_err(|_| "browser cleanup lock poisoned".to_string())?
        .clone();
    let Some(session_id) = session_id else {
        return Ok(());
    };
    state
        .browser
        .recover_cleanup(&session_id)
        .map_err(|error| format!("browser cleanup remains unresolved: {error}"))?;
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .set_browser_cleanup(&session_id, false, deepify::domain::now_seconds())
        .map_err(|error| error.to_string())?;
    *state
        .pending_browser_cleanup
        .lock()
        .map_err(|_| "browser cleanup lock poisoned".to_string())? = None;
    Ok(())
}
fn interrupt_for_browser_failure(app: &AppHandle, state: &AppState) -> Result<bool, String> {
    let mut sessions = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    if sessions.current.is_none() {
        return Ok(false);
    }
    let summary = sessions
        .runtime_failure("browser")
        .map_err(|error| error.to_string())?;
    drop(sessions);
    let summary = summary_dto(summary);
    stop_audio(state, true);
    persist_summary(state, &summary)?;
    *state
        .summary
        .lock()
        .map_err(|_| "summary lock poisoned".to_string())? = Some(summary);
    emit_snapshot(app, state)?;
    Ok(true)
}
fn summary_dto(value: deepify::domain::SessionSummary) -> SessionSummaryDto {
    SessionSummaryDto {
        id: value.id,
        focused_seconds: value.focused.as_secs(),
        paused_seconds: value.paused.as_secs(),
        blocked_attempts: value.blocked_attempts,
        reason: finish_name(value.reason),
        cleanup_complete: value.cleanup_complete,
    }
}

#[tauri::command]
fn app_snapshot(state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    snapshot(&state)
}
#[tauri::command]
fn session_start(
    app: AppHandle,
    state: State<'_, AppState>,
    seconds: u64,
    intention: Option<String>,
    track_path: Option<String>,
) -> Result<AppSnapshotDto, String> {
    validate_duration(seconds).map_err(|error| error.to_string())?;
    if !state
        .blocked_apps
        .lock()
        .map_err(|_| "blocked-app lock poisoned".to_string())?
        .is_empty()
    {
        return Err("preflight_blocked: close the simulated blocked applications".into());
    }
    resolve_browser_cleanup(&state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let started_at = deepify::domain::now_seconds();
    let website_rules = state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .whitelist()
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter_map(|(_, kind, normalized)| {
            (kind == "website")
                .then(|| normalize_website(&normalized).ok())
                .flatten()
        })
        .collect::<Vec<_>>();
    {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        if sessions.current.is_some() || sessions.recovery_required {
            return Err("Conflict: an active session or cleanup recovery already exists".into());
        }
    }
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .insert_session_with_intention(&id, "starting", seconds, intention.as_deref(), started_at)
        .map_err(|error| error.to_string())?;
    let current = {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        sessions
            .restriction
            .browser
            .configure(id.clone(), website_rules, seconds);
        state
            .database
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?
            .set_browser_cleanup(&id, true, started_at)
            .map_err(|error| error.to_string())?;
        if let Err(error) = sessions.start(seconds, intention.clone()) {
            let cleanup_required = sessions.recovery_required;
            drop(sessions);
            state
                .database
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?
                .finish_session(FinishSessionRecord {
                    id: &id,
                    status: "interrupted",
                    reason: "interrupted",
                    focused_seconds: 0,
                    paused_seconds: 0,
                    blocked_attempts: 0,
                    finished_at: started_at,
                })
                .map_err(|storage_error| storage_error.to_string())?;
            state
                .database
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?
                .set_browser_cleanup(&id, cleanup_required, started_at)
                .map_err(|storage_error| storage_error.to_string())?;
            return Err(error.to_string());
        }
        if let Some(current) = sessions.current.as_mut() {
            current.id.clone_from(&id);
        }
        sessions
            .current
            .as_ref()
            .cloned()
            .ok_or("session did not start")?
    };
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .update_session(
            &current.id,
            "working",
            current.focused.as_secs(),
            current.paused.as_secs(),
            current.blocked_attempts,
            deepify::domain::now_seconds(),
        )
        .map_err(|error| error.to_string())?;
    let should_play = track_path.is_some();
    if let Some(path) = track_path {
        if let Ok(mut audio) = state.audio.lock() {
            audio.current = audio
                .queue
                .iter()
                .position(|track| track.path.to_string_lossy() == path);
        }
    }
    if should_play {
        play_current_audio(&state);
    }
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(250));
        let managed = handle.state::<AppState>();
        for session_id in managed.browser.take_blocked_events() {
            if let Ok(mut sessions) = managed.sessions.lock() {
                if sessions
                    .current
                    .as_ref()
                    .is_some_and(|current| current.id == session_id)
                {
                    sessions.blocked_attempt();
                }
            }
        }
        let integration_failed = !managed.browser.take_integration_errors().is_empty();
        let health_due = managed
            .last_browser_health
            .lock()
            .map(|mut checked| {
                if checked.elapsed() >= Duration::from_secs(20) {
                    *checked = Instant::now();
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
        let health_failed = health_due
            && managed
                .browser
                .heartbeat()
                .and_then(|_| managed.browser.check_health())
                .is_err();
        if (integration_failed || health_failed)
            && interrupt_for_browser_failure(&handle, &managed).unwrap_or(false)
        {
            break;
        }
        let result = {
            let mut sessions = match managed.sessions.lock() {
                Ok(value) => value,
                Err(_) => break,
            };
            if sessions.current.is_none() {
                break;
            }
            sessions.tick()
        };
        match result {
            Ok(Some(done)) => {
                let summary = summary_dto(done);
                stop_audio(&managed, true);
                let _ = persist_summary(&managed, &summary);
                if let Ok(mut slot) = managed.summary.lock() {
                    *slot = Some(summary);
                }
                if snapshot(&managed)
                    .ok()
                    .is_some_and(|value| value.settings.notifications)
                {
                    let _ = handle
                        .notification()
                        .builder()
                        .title("Deepify")
                        .body("Your focus session ended.")
                        .show();
                }
                let _ = emit_snapshot(&handle, &managed);
                break;
            }
            Ok(None) => {
                let _ = persist_current(&managed);
                let _ = emit_snapshot(&handle, &managed);
            }
            Err(_) => break,
        }
    });
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn session_pause(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    let timer_snapshot = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .pause()
        .map_err(|error| error.to_string())?;
    let id = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .current
        .as_ref()
        .map(|current| current.id.clone())
        .ok_or("missing active session")?;
    if state
        .browser
        .sync_timer_state(&id, "paused", timer_snapshot.remaining.as_secs())
        .is_err()
    {
        interrupt_for_browser_failure(&app, &state)?;
        return snapshot(&state);
    }
    persist_current(&state)?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn session_resume(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    let timer_snapshot = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .resume()
        .map_err(|error| error.to_string())?;
    let id = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .current
        .as_ref()
        .map(|current| current.id.clone())
        .ok_or("missing active session")?;
    if state
        .browser
        .sync_timer_state(&id, "working", timer_snapshot.remaining.as_secs())
        .is_err()
    {
        interrupt_for_browser_failure(&app, &state)?;
        return snapshot(&state);
    }
    persist_current(&state)?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn session_end(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    let value = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .finish(FinishReason::EndedEarly)
        .map_err(|error| error.to_string())?;
    let summary = summary_dto(value);
    stop_audio(&state, true);
    persist_summary(&state, &summary)?;
    *state
        .summary
        .lock()
        .map_err(|_| "summary lock poisoned".to_string())? = Some(summary);
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn dismiss_summary(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    *state
        .summary
        .lock()
        .map_err(|_| "summary lock poisoned".to_string())? = None;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn save_setting(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<AppSnapshotDto, String> {
    match key.as_str() {
        "theme" if matches!(value.as_str(), "obsidian" | "mist") => {}
        "notifications" if matches!(value.as_str(), "true" | "false") => {}
        "default_duration_seconds" if value.parse::<u64>().is_ok_and(|number| number > 0) => {}
        _ => return Err("validation_error: unsupported setting".into()),
    }
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .set_setting(&key, &value)
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn complete_setup(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    if state.browser.snapshot().state != "healthy_idle" {
        return Err("browser pairing is required before setup can be completed".into());
    }
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .set_setting("setup_complete", "true")
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn browser_accept_pairing(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    if state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .current
        .is_some()
    {
        return Err("conflict: browser pairing cannot change during a session".into());
    }
    let pairing = state
        .browser
        .accept_pending()
        .map_err(|error| error.to_string())?;
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .save_browser_pairing(&StoredBrowserPairing {
            token_hash: pairing.token_hash,
            browser_kind: pairing.browser_kind,
            profile_label: pairing.profile_label,
            paired_at: pairing.paired_at,
        })
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn browser_forget_pairing(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    if state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .current
        .is_some()
    {
        return Err("conflict: browser pairing cannot change during a session".into());
    }
    state
        .browser
        .forget_pairing()
        .map_err(|error| error.to_string())?;
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .forget_browser_pairing()
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn browser_retry_health(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    resolve_browser_cleanup(&state)?;
    state
        .browser
        .check_health()
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn rerun_setup(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .set_setting("setup_complete", "false")
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn repair_integrations(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    if sessions.current.is_some() {
        return Err("conflict: integration repair is unavailable during a session".into());
    }
    drop(sessions);
    resolve_browser_cleanup(&state)?;
    let mut sessions = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    sessions.restriction.applications.failure = MockFailure::None;
    sessions.restriction.do_not_disturb.failure = MockFailure::None;
    if sessions.recovery_required {
        sessions
            .retry_cleanup()
            .map_err(|error| error.to_string())?;
    }
    drop(sessions);
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn simulate_unhealthy_integration(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    let mut sessions = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    if sessions.current.is_some() {
        return Err("conflict: health simulation is unavailable during a session".into());
    }
    sessions.restriction.applications.failure = MockFailure::Preflight;
    drop(sessions);
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn add_whitelist(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    value: String,
) -> Result<AppSnapshotDto, String> {
    if state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .current
        .is_some()
    {
        return Err("conflict: whitelist is read-only during a session".into());
    }
    let normalized = match kind.as_str() {
        "application" => normalize_app_id(&value).map_err(|error| error.to_string())?,
        "website" => {
            let (host, path) = normalize_website(&value).map_err(|error| error.to_string())?;
            format!("{host}{path}")
        }
        _ => return Err("validation_error: invalid whitelist kind".into()),
    };
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .upsert_whitelist(
            &uuid::Uuid::new_v4().to_string(),
            &kind,
            &value,
            &normalized,
            deepify::domain::now_seconds(),
        )
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn test_whitelist(
    state: State<'_, AppState>,
    kind: String,
    value: String,
) -> Result<WhitelistTestDto, String> {
    let stored = state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .whitelist()
        .map_err(|error| error.to_string())?;
    let rules = stored
        .iter()
        .filter_map(|(_, stored_kind, normalized)| match stored_kind.as_str() {
            "website" => normalize_website(normalized)
                .ok()
                .map(|(host, path)| WhitelistEntry::Website { host, path }),
            "application" => Some(WhitelistEntry::Application(normalized.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let allowed = match kind.as_str() {
        "website" => {
            let probe = if value.contains("://") {
                value
            } else {
                format!("https://{value}")
            };
            website_allowed(&probe, &rules)
        }
        "application" => {
            let candidate = normalize_app_id(&value).map_err(|error| error.to_string())?;
            rules
                .iter()
                .any(|rule| matches!(rule, WhitelistEntry::Application(id) if id == &candidate))
                || implicit_app_allowed(Some(&candidate), "com.deepify.desktop")
        }
        _ => return Err("validation_error: invalid whitelist kind".into()),
    };
    Ok(WhitelistTestDto {
        allowed,
        detail: if allowed {
            "Allowed by the current local configuration"
        } else {
            "Would be blocked by the current local configuration"
        },
    })
}
#[tauri::command]
fn remove_whitelist(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<AppSnapshotDto, String> {
    if state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .current
        .is_some()
    {
        return Err("conflict: whitelist is read-only during a session".into());
    }
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .remove_whitelist(&id)
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &state)
}

fn save_import(
    state: &AppState,
    source_kind: &str,
    source_path: &str,
    tracks: Vec<Track>,
) -> Result<(), String> {
    let proposed_id = uuid::Uuid::new_v4().to_string();
    let now = deepify::domain::now_seconds();
    let stored: Vec<_> = tracks.iter().map(stored_track).collect();
    {
        let mut database = state
            .database
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let id = database
            .add_music_source(&proposed_id, source_kind, source_path, now)
            .map_err(|error| error.to_string())?;
        database
            .replace_source_tracks(&id, &stored, now)
            .map_err(|error| error.to_string())?;
    }
    let mut audio = state
        .audio
        .lock()
        .map_err(|_| "audio lock poisoned".to_string())?;
    for track in tracks {
        if !audio.queue.iter().any(|item| item.path == track.path) {
            audio.queue.push(track);
        }
    }
    audio
        .queue
        .sort_by(|left, right| left.path.cmp(&right.path));
    let paths = audio
        .queue
        .iter()
        .map(|item| item.path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    state
        .database
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?
        .replace_queue(&paths)
        .map_err(|error| error.to_string())
}

fn install_folder_watcher(app: AppHandle, path: PathBuf) -> Result<(), String> {
    let (watcher, receiver) = watch_folder(&path).map_err(|error| error.to_string())?;
    app.state::<AppState>()
        .watchers
        .lock()
        .map_err(|_| "watcher lock poisoned".to_string())?
        .push(watcher);
    std::thread::spawn(move || {
        while receiver.recv().is_ok() {
            std::thread::sleep(Duration::from_millis(150));
            while receiver.try_recv().is_ok() {}
            let state = app.state::<AppState>();
            let tracks = AudioEngine::scan_folder(&path);
            let _ = save_import(&state, "folder", &path.to_string_lossy(), tracks);
            let _ = emit_snapshot(&app, &state);
        }
    });
    Ok(())
}
#[tauri::command]
async fn import_music_files(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    if let Some(files) = app
        .dialog()
        .file()
        .add_filter("MP3", &["mp3"])
        .blocking_pick_files()
    {
        for file in files {
            let path = file.into_path().map_err(|error| error.to_string())?;
            if let Some(track) = AudioEngine::import_file(path.clone()) {
                save_import(&state, "file", &path.to_string_lossy(), vec![track])?;
            }
        }
    }
    emit_snapshot(&app, &state)
}
#[tauri::command]
async fn import_music_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    if let Some(folder) = app.dialog().file().blocking_pick_folder() {
        let path = folder.into_path().map_err(|error| error.to_string())?;
        save_import(
            &state,
            "folder",
            &path.to_string_lossy(),
            AudioEngine::scan_folder(&path),
        )?;
        install_folder_watcher(app.clone(), path)?;
    }
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn simulate_blocked_attempt(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    let mut sessions = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    if sessions.current.is_none() {
        return Err("conflict: no active session".into());
    }
    sessions.blocked_attempt();
    drop(sessions);
    persist_current(&state)?;
    emit_snapshot(&app, &state)
}
#[tauri::command]
fn simulate_runtime_failure(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    let value = state
        .sessions
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?
        .runtime_failure("simulated integration")
        .map_err(|error| error.to_string())?;
    let summary = summary_dto(value);
    stop_audio(&state, true);
    persist_summary(&state, &summary)?;
    *state
        .summary
        .lock()
        .map_err(|_| "summary lock poisoned".to_string())? = Some(summary);
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn simulate_blocked_app(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    *state
        .blocked_apps
        .lock()
        .map_err(|_| "blocked-app lock poisoned".to_string())? =
        vec!["com.example.Chat".to_string()];
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn resolve_blocked_apps(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    state
        .blocked_apps
        .lock()
        .map_err(|_| "blocked-app lock poisoned".to_string())?
        .clear();
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn audio_toggle(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    let playing = state
        .audio
        .lock()
        .map_err(|_| "audio lock poisoned".to_string())?
        .playing;
    if playing {
        if let Some(output) = state
            .player
            .lock()
            .map_err(|_| "player lock poisoned".to_string())?
            .as_ref()
        {
            output.pause();
        }
        state
            .audio
            .lock()
            .map_err(|_| "audio lock poisoned".to_string())?
            .pause();
    } else {
        play_current_audio(&state);
    }
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn audio_retry_output(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshotDto, String> {
    *state
        .player
        .lock()
        .map_err(|_| "player lock poisoned".to_string())? = None;
    play_current_audio(&state);
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn audio_previous(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    let was_playing = {
        let mut audio = state
            .audio
            .lock()
            .map_err(|_| "audio lock poisoned".to_string())?;
        let playing = audio.playing;
        audio.previous();
        playing
    };
    if was_playing {
        play_current_audio(&state);
    }
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn audio_next(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshotDto, String> {
    let was_playing = {
        let mut audio = state
            .audio
            .lock()
            .map_err(|_| "audio lock poisoned".to_string())?;
        let playing = audio.playing;
        audio.next();
        playing
    };
    if was_playing {
        play_current_audio(&state);
    }
    emit_snapshot(&app, &state)
}

#[tauri::command]
fn audio_set_volume(
    app: AppHandle,
    state: State<'_, AppState>,
    volume: u8,
) -> Result<AppSnapshotDto, String> {
    let volume = volume.min(100);
    state
        .audio
        .lock()
        .map_err(|_| "audio lock poisoned".to_string())?
        .set_volume(volume);
    if let Some(output) = state
        .player
        .lock()
        .map_err(|_| "player lock poisoned".to_string())?
        .as_ref()
    {
        output.set_volume(volume);
    }
    emit_snapshot(&app, &state)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let mut database = SqliteStore::open(directory.join("deepify.sqlite"))?;
            let browser_pairing = database.browser_pairing()?.map(|pairing| BrowserPairing {
                token_hash: pairing.token_hash,
                browser_kind: pairing.browser_kind,
                profile_label: pairing.profile_label,
                paired_at: pairing.paired_at,
            });
            let browser = BrowserBroker::start(browser_pairing)?;
            let pending_browser_cleanup = database.browser_cleanup_required()?;
            let recovered = database.recover_abandoned(deepify::domain::now_seconds())?;
            let recovered_summary = if recovered > 0 {
                database
                    .history()?
                    .first()
                    .cloned()
                    .map(|item| SessionSummaryDto {
                        id: item.id,
                        focused_seconds: item.focused_seconds,
                        paused_seconds: item.paused_seconds,
                        blocked_attempts: item.blocked_attempts,
                        reason: "interrupted",
                        cleanup_complete: pending_browser_cleanup.is_none(),
                    })
            } else {
                None
            };
            let sources = database.music_sources()?;
            let watched_folders = sources
                .iter()
                .filter(|source| source.kind == "folder")
                .map(|source| PathBuf::from(&source.path))
                .collect::<Vec<_>>();
            for source in sources {
                let path = PathBuf::from(&source.path);
                let tracks = if source.kind == "folder" {
                    AudioEngine::scan_folder(&path)
                } else {
                    AudioEngine::import_file(path).into_iter().collect()
                };
                let stored = tracks.iter().map(stored_track).collect::<Vec<_>>();
                database.replace_source_tracks(
                    &source.id,
                    &stored,
                    deepify::domain::now_seconds(),
                )?;
            }
            let queue = database
                .queue()?
                .into_iter()
                .map(|item| Track {
                    path: PathBuf::from(item.path),
                    title: item.title,
                    artist: item.artist,
                    album: item.album,
                    available: item.available,
                })
                .collect();
            app.manage(AppState {
                diagnostic_id: uuid::Uuid::new_v4().simple().to_string(),
                sessions: Mutex::new(SessionService::new(
                    SystemClock,
                    ProductionRestrictionCoordinator::new(browser.clone()),
                )),
                browser,
                database: Mutex::new(database),
                audio: Mutex::new(AudioEngine {
                    queue,
                    ..Default::default()
                }),
                player: Mutex::new(None),
                blocked_apps: Mutex::new(Vec::new()),
                watchers: Mutex::new(Vec::new()),
                summary: Mutex::new(recovered_summary),
                last_browser_health: Mutex::new(Instant::now()),
                pending_browser_cleanup: Mutex::new(pending_browser_cleanup),
            });
            for folder in watched_folders {
                install_folder_watcher(app.handle().clone(), folder)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_snapshot,
            session_start,
            session_pause,
            session_resume,
            session_end,
            dismiss_summary,
            save_setting,
            complete_setup,
            browser_accept_pairing,
            browser_forget_pairing,
            browser_retry_health,
            rerun_setup,
            repair_integrations,
            simulate_unhealthy_integration,
            add_whitelist,
            test_whitelist,
            remove_whitelist,
            import_music_files,
            import_music_folder,
            audio_toggle,
            audio_retry_output,
            audio_previous,
            audio_next,
            audio_set_volume,
            simulate_blocked_app,
            resolve_blocked_apps,
            simulate_blocked_attempt,
            simulate_runtime_failure
        ])
        .run(tauri::generate_context!())
        .expect("error while running Deepify");
}
