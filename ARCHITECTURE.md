# Deepify — Technical Architecture

## Status

- **Stage:** Phase 3 browser-enforcement implementation in progress
- **Target:** NixOS, Niri, and Wayland
- **Application model:** Local-only Tauri desktop application with a Firefox extension
- **Related documents:** [`DESIGN.md`](DESIGN.md), [`PHASE1_ISSUES.md`](PHASE1_ISSUES.md), and [`PHASE2_IMPLEMENTATION_PLAN.md`](PHASE2_IMPLEMENTATION_PLAN.md)

This document defines the intended production architecture. Phase 3 connects the Firefox/Zen adapter; Phase 4 will connect Niri and Noctalia enforcement.

---

## 1. Architecture Goals

1. Keep the timer and session lifecycle authoritative outside the WebView.
2. Keep all user data local and usable without an account or network connection.
3. Isolate Firefox, Niri, Noctalia, audio, notifications, and persistence behind replaceable adapters.
4. Make restriction failure explicit; never report protection as active when an integration is unhealthy.
5. Preserve deterministic cleanup behavior across completion, early exit, integration failure, and restart recovery.
6. Keep the UI minimal and implementation-independent from platform commands.
7. Make product rules testable without launching Firefox, Niri, or a real audio device.
8. Package and develop reproducibly on NixOS.

---

## 2. Technology Decisions

### Desktop and interface

- **Desktop shell:** Tauri 2
- **Frontend:** React, TypeScript, and Vite
- **Backend:** Rust
- **Package manager:** npm with a committed lockfile
- **Styling:** Plain CSS/CSS Modules with semantic CSS custom properties
- **Routing:** React Router for top-level screens
- **Generated command types:** Rust DTOs exported to TypeScript through a checked-in generation step; use `specta`/`tauri-specta` if compatible with the pinned Tauri version

No large component library or utility-CSS framework is planned for the MVP. The Obsidian-inspired UI should be implemented with small local components and semantic theme tokens.

### Persistence

- **Structured data:** SQLite owned exclusively by the Rust backend
- **SQLite access:** `rusqlite` with explicit versioned migrations
- **Preferences:** Typed settings stored in SQLite; small bootstrap values may use a local config file only when needed before the database opens
- **Audio files:** Never copied into application storage; persist direct file and folder paths
- **Database location:** Tauri application-data directory
- **Runtime socket and locks:** `$XDG_RUNTIME_DIR`

The frontend never opens SQLite directly. It uses typed Tauri commands.

### Audio

- **Architecture:** Rust `AudioEngine` adapter
- **Preferred implementation direction:** Rust-native decoding/playback, with MP3 decoding and metadata libraries selected by the Phase 2 audio integration task
- **Metadata:** ID3 first; `Artist - Title.mp3`, then filename-stem fallback
- **Folder updates:** `notify` filesystem watcher plus deterministic rescans; rescan on startup and when reopening the library as a fallback
- **Device behavior:** Explicit `waiting_for_output_device` health state and recovery onto an available device

The P1-23 FFmpeg/VLC spike proves target-machine feasibility, not the final playback library.

#### Phase 2 audio adapter decision

Phase 2 uses `rodio` with its Symphonia MP3 decoder for playback, `id3` for embedded metadata, and `notify` for live folder updates. The implementation remains behind the `AudioEngine` boundary so output-device handling and future decoder changes do not leak into the session domain or React UI. Direct imported paths remain authoritative; Deepify does not copy media into application storage. Startup and library-open rescans provide deterministic recovery when watcher events are missed.

### Linux and browser integrations

- **Niri:** One persistent `niri msg -j event-stream` child process plus targeted `niri msg` requests
- **Noctalia:** `noctalia-shell ipc` adapter that reads, changes, and restores Do Not Disturb state
- **Firefox/Zen:** WebExtension using Firefox native messaging
- **Native messaging host:** Separate small Rust binary launched by the browser
- **Desktop/helper transport:** User-only Unix-domain socket under `$XDG_RUNTIME_DIR`
- **Notifications:** Tauri notification plugin or a narrow Rust notification adapter
- **Single instance:** Tauri single-instance plugin or equivalent Rust lock

The dedicated native-host binary is necessary because Firefox launches native messaging hosts as child processes. It forwards framed messages to the already-running desktop backend rather than embedding the full desktop application.

### Phase 3 browser contract

Firefox and Zen communicate only through version-one, length-prefixed JSON
frames. Every request has a message ID and every response correlates to its
request. The helper and desktop socket reject oversized (over 256 KiB), invalid,
wrong-version, and URL-bearing frames. The socket is
`$XDG_RUNTIME_DIR/deepify/browser-v1.sock`, with a `0700` directory, `0600`
socket, and same-UID peer check.

The extension generates a profile-local random token. The desktop stores only
its SHA-256 hash, browser kind, profile label, and pair time; explicit desktop
acceptance is required before the profile receives a policy. The extension owns
its in-memory original-tab map. No original or attempted destination is sent to
the helper, socket, database, logs, diagnostics, or blocked-page URL.

Top-level public HTTP(S) pages are default-denied while a paired session is
active. Non-HTTP(S), loopback, private, and link-local destinations remain
allowed. Existing tabs are scanned before start acknowledgement, and the
extension locally restores surviving blocked tabs if the native port closes.

---

## 3. Repository Layout

The production code should use this structure:

```text
.
├── apps/
│   └── desktop/
│       ├── src/                    # React/TypeScript UI
│       ├── src-tauri/
│       │   ├── src/
│       │   │   ├── domain/         # Pure session and whitelist rules
│       │   │   ├── services/       # Application use cases/coordinators
│       │   │   ├── adapters/       # SQLite, mock, audio, Niri, Noctalia
│       │   │   ├── commands/       # Narrow Tauri command surface
│       │   │   ├── events/         # Backend-to-frontend event mapping
│       │   │   └── main.rs
│       │   ├── migrations/
│       │   └── tauri.conf.json
│       └── package.json
├── crates/
│   └── browser-native-host/        # Firefox native messaging stdio helper
├── extensions/
│   └── firefox/                    # Production WebExtension
├── contracts/
│   ├── url-rule-cases.json         # Shared whitelist matching fixtures
│   ├── native-messaging.schema.json
│   └── generated/                  # Generated TypeScript command DTOs
├── tests/
│   └── fixtures/
├── validation/
│   ├── P1-21_NIRI_VALIDATION.md
│   ├── p1-22-firefox-spike/
│   └── p1-23-local-audio-spike/
├── ARCHITECTURE.md
├── DESIGN.md
├── PHASE1_ISSUES.md
├── PHASE2_IMPLEMENTATION_PLAN.md
├── Cargo.toml                      # Rust workspace
├── package.json                    # npm workspace
├── flake.nix
└── flake.lock
```

The existing disposable spikes and validation evidence remain under `validation/`. They are committed evidence and test references, not production modules.

For the MVP, the desktop backend remains one Rust crate organized by modules. Only the native messaging host is a separate crate. Additional Rust crates should be introduced only when a boundary needs independent compilation or reuse.

---

## 4. Runtime Components

```text
┌───────────────────────────────────────────────────────────────────────┐
│ Tauri desktop process                                                 │
│                                                                       │
│  React UI ──typed commands/events──> Rust application services        │
│                                          │                            │
│                    ┌─────────────────────┼──────────────────────┐     │
│                    │                     │                      │     │
│              Session engine       SQLite repositories      AudioEngine│
│                    │                     │                      │     │
│             RestrictionCoordinator      local DB          output device│
│              │          │                                         │  │
│          Browser     Application/DND                                 │
│          adapter      adapters                                      │
└──────────────┼──────────────┼─────────────────────────────────────────┘
               │              ├── niri event stream / actions
               │              └── Noctalia IPC
               │
        user-only Unix socket
               │
┌──────────────▼─────────────┐       native messaging       ┌───────────┐
│ Rust native-host helper    │<────────────────────────────>│ Firefox / │
│ length-prefixed JSON stdio │                              │ Zen ext.  │
└────────────────────────────┘                              └───────────┘
```

### Ownership rules

- The Rust backend owns session state, elapsed time, persistence, cleanup, and integration health.
- React owns transient presentation state such as open dialogs, selected rows, and unsaved form input.
- SQLite is accessed only by Rust repositories.
- The extension owns active tab interception and original URL restoration.
- The desktop owns whether a focus session is valid and active.
- The native-host helper transports messages only; it does not own session policy.

---

## 5. Backend Layering

### Domain

Pure types and rules with no Tauri, SQLite, subprocess, filesystem, or clock dependencies:

- `Session`
- `SessionState`
- `FinishReason`
- `TimerSnapshot`
- `WhitelistEntry`
- URL-rule normalization and validation
- `PlaylistSource`
- `QueueEntry`
- `IntegrationHealth`
- `BlockedAttemptSummary`

Domain logic accepts injected clocks and IDs in tests.

### Application services

Coordinate domain behavior and ports:

- `SessionService`
- `PreflightService`
- `RestrictionCoordinator`
- `WhitelistService`
- `MusicLibraryService`
- `PlaybackService`
- `HistoryService`
- `SetupService`
- `RecoveryService`

### Ports/adapters

Define Rust traits for external behavior:

```text
SessionRepository
SettingsRepository
WhitelistRepository
MusicRepository
Clock
RestrictionAdapter
BrowserRestrictionAdapter
ApplicationRestrictionAdapter
DoNotDisturbAdapter
AudioEngine
NotificationAdapter
FileWatcher
```

Phase 2 supplies mock restriction adapters. Phase 3 and Phase 4 replace them without changing session-domain rules.

### Tauri commands

Commands are thin request/response boundaries. They validate DTO shape, call one service, and return typed errors. They do not contain policy or SQL.

---

## 6. Session State and Cleanup

### Persisted states

```text
not_working (absence of active session)
starting
working
paused
ending
finished
interrupted
```

The user-facing states remain **Not working**, **Working**, and **Paused**. `starting` and `ending` are internal transaction/coordinator states.

### Primary transition flow

```text
Not working
  └─ start request
       └─ starting
            ├─ preflight/start failure ─> cleanup ─> interrupted/not working
            └─ success ─> working
                           ├─ pause ─> paused ─> resume ─> working
                           ├─ timer complete ─> ending ─> finished
                           ├─ end early ─────> ending ─> finished
                           └─ integration failure ─> ending ─> interrupted
```

### Start sequence

1. Validate required duration and optional inputs.
2. Verify that no other session is active.
3. Persist a `starting` session record.
4. Run preflight checks.
5. Start browser restriction adapter.
6. Preserve and enable Do Not Disturb.
7. Start application monitoring.
8. Mark the session `working` and persist the start checkpoint.
9. Emit a complete session snapshot to the UI.
10. Start selected audio if available.

There is no promise of an atomic platform transaction. If a step fails, the coordinator performs best-effort reverse cleanup and records the failure. The UI never reports Working until all required startup steps succeed.

### End sequence

1. Persist `ending` and stop accepting pause/resume commands.
2. Stop application monitoring.
3. Stop browser restrictions and request original-tab restoration.
4. Restore the previous Do Not Disturb state.
5. Fade and stop audio.
6. Persist elapsed focus time, blocked count, finish reason, and final state.
7. Send the desktop notification.
8. Emit the summary snapshot.

Cleanup operations should be idempotent so recovery can safely retry them.

### Timer authority

- Rust tracks elapsed working time with a monotonic clock.
- Paused time never increments focused time.
- React renders a countdown from backend snapshots but cannot finalize a session itself.
- Persist checkpoints on every transition and periodically while Working.
- An abandoned active record on launch is marked interrupted before a new session can start.
- Suspend, wall-clock discontinuity, process crash, logout, reboot, or shutdown ends rather than resumes the session, according to P1-12.

---

## 7. Restriction Coordination

### Phase 2 mock contract

The mock adapter supports:

- Healthy/unhealthy preflight states
- A configurable blocked-app result
- Simulated blocked attempts
- Simulated runtime failure
- Idempotent activate/deactivate
- Integration-health snapshots

The UI must visibly label this as simulated protection in development builds so it never misrepresents enforcement.

### Phase 3 browser adapter

- Pair one configured profile token.
- Maintain a persistent native-messaging connection.
- Keep a one-minute desktop health check in addition to disconnect events.
- Send session rules and active state.
- Receive blocked-attempt and health events.
- Restore surviving blocked tabs on all stop/failure paths.
- Keep private windows and unmonitored profiles documented as MVP bypasses.

### Phase 4 application and DND adapters

- Read every Niri-reported window before session start.
- Use Wayland `app_id` identity rules from P1-10.
- Keep one deduplicated event-stream connection.
- Request one targeted cooperative close for blocked windows.
- Restore focus to the last allowed window on a best-effort basis.
- Require manual closure after a refused close or unsaved-work prompt.
- Read and preserve Noctalia DND before enabling it.
- Treat Niri/event-stream/Noctalia failure as session-ending.

---

## 8. Browser Native Messaging Contract

### Topology

```text
Extension <-> native-host stdio <-> Unix socket <-> desktop backend
```

### Security requirements

- Fixed extension ID in the native-host manifest.
- Profile pairing token generated by the extension and explicitly accepted by the desktop.
- Socket directory and socket accessible only to the current user.
- Reject unknown tokens, malformed frames, oversized messages, and protocol-version mismatches.
- Never accept arbitrary command execution or file paths through browser messages.
- Use bounded message sizes and timeouts.

### Message families

```text
hello / pair / pair_result
heartbeat / heartbeat_ack
start_session / start_result
stop_session / stop_result
status / state
blocked_attempt
integration_error
restore_complete / restore_error
```

All messages include a protocol version and request/correlation ID when a response is expected. JSON schemas and fixtures live under `contracts/`.

---

## 9. Persistence Model

### Tables

#### `sessions`

- `id`
- `status`
- `planned_seconds`
- `focused_seconds`
- `paused_seconds`
- `intention` nullable
- `playlist_source_id` nullable
- `started_at` nullable
- `finished_at` nullable
- `finish_reason` nullable
- `interruption_cause` nullable
- `blocked_attempt_count`
- `created_at`
- `updated_at`

Enforce at most one `starting`, `working`, `paused`, or `ending` row with a partial unique index or a singleton runtime table.

#### `whitelist_entries`

- `id`
- `kind` (`application`, `website`)
- `value`
- `normalized_value`
- `created_at`

Malformed entries are rejected before insertion. Implicit system/focus-app allowances are code-owned and are not removable database rows.

#### `music_sources`

- `id`
- `kind` (`file`, `folder`)
- `path`
- `created_at`
- `last_scanned_at`

#### `music_tracks`

A cache of discovered metadata keyed by source and normalized absolute path. Folder rescans may replace this cache; source paths remain authoritative.

#### `queue_entries`

- `position`
- `track_path`
- `source_id`

Queue reconciliation retains existing paths, removes missing folder paths, and appends new paths deterministically.

#### `settings`

Typed keys for:

- Default duration
- Built-in color scheme
- Session-completion notifications
- Paired browser/profile label and token reference
- Setup completion
- Audio output preference

#### `integration_state`

Stores non-secret recovery checkpoints such as previous DND state and whether cleanup may be required. Runtime health itself is live adapter state.

### Privacy

Store aggregate blocked-attempt counts, not browsing destinations or a detailed browsing history. Keep the latest active warning in memory unless diagnostics explicitly require persistence later.

---

## 10. Frontend Architecture

### Screens

- Setup wizard
- Focus Room: Not working
- Preflight validation
- Blocked-app resolution
- Focus Room: Working/Paused
- Session summary
- Sound Library
- Whitelist
- Settings/integration health
- Session History

The blocked website page is extension UI, not a Tauri route.

### State split

**Backend-owned:**

- Session and timer snapshot
- Integration health
- Whitelist records
- Library and queue
- Playback health/state
- History and settings

**Frontend-owned:**

- Current route
- Open dialog
- Draft form values and validation display
- Selected rows
- Temporary search/filter text

### Events

- `session://changed`
- `session://blocked-attempt`
- `integration://changed`
- `audio://changed`
- `library://changed`
- `recovery://required`

On startup or after an event gap, the frontend requests a full `AppSnapshot` rather than attempting to reconstruct backend state from events.

### Styling and accessibility

- Semantic tokens: background, surface, elevated surface, border, text, muted text, accent, danger, warning, success, focus ring.
- Default built-in scheme: Obsidian-like dark neutral.
- Theme selection changes tokens only.
- Full keyboard navigation and visible focus rings.
- Status and validation are represented by text and icons, never color alone.
- Respect reduced-motion preferences; avoid decorative animation.
- Use native buttons, labels, dialogs, and landmarks where possible.

---

## 11. Error Model

Every backend error maps to a stable typed code:

```text
validation_error
conflict
integration_unavailable
integration_disconnected
permission_denied
persistence_error
file_missing
unsupported_audio
output_device_unavailable
cleanup_incomplete
internal_error
```

A command error includes:

- Stable code
- Safe user-facing message
- Optional field name
- Optional recovery action
- Correlation ID for local diagnostics

Do not expose arbitrary subprocess stderr, browser URLs, secrets, or filesystem contents directly in UI errors.

---

## 12. Testing Architecture

### Rust

- Domain state-machine unit tests with fake clocks
- URL/whitelist validation table tests
- Repository integration tests against temporary SQLite databases
- Coordinator tests with mock adapters and injected failures at every startup/cleanup step
- Native-host framing and socket authentication tests
- Audio queue reconciliation and metadata tests

### Frontend

- Vitest and Testing Library
- Screen-state tests based on P1-20
- Keyboard navigation and validation tests
- Theme token and persistence tests
- Typed command adapter mocks

### Contracts

- Shared URL-rule fixture suite run by Rust and the Firefox extension
- Native-message JSON schema tests
- Generated DTO consistency check
- Migration tests from every released schema version

### Platform/manual integration

- Niri event and cooperative-close scenarios
- Noctalia prior-state restoration
- Firefox and Zen profile pairing
- Private-window bypass disclosure
- Real PipeWire device switching
- Suspend, restart, logout, and hard-crash recovery
- Nix package installation on the target machine

P1-24 is the release-level acceptance baseline. Phase 2 passes only the criteria that do not require real restriction adapters and clearly records simulated results.

---

## 13. Nix and Build Architecture

The root flake should provide:

- `devShells.default`: Rust, Node/npm, Tauri CLI, pkg-config, WebKitGTK/GTK dependencies, SQLite tooling, and test utilities
- `packages.default`: Desktop package when packaging is introduced
- `packages.browser-native-host`: Native-host binary and manifest package
- `checks`: Rust tests, frontend tests, formatting, linting, and contract tests

Commit:

- `flake.lock`
- `Cargo.lock`
- `package-lock.json`

Development and tests must not depend on globally installed npm packages.

---

## 14. Architectural Decisions and Follow-ups

### Accepted

1. Product display name: **Deepify**.
2. Desktop executable name: `deepify` (normalized lowercase executable form of the product name).
3. nixpkgs release baseline: 26.05, locked at `f4f698677b11021a8f84f452e23ae9ef2427bec3`.
4. Tauri 2 + React/TypeScript + Rust.
5. Rust backend is authoritative for sessions and persistence.
6. SQLite is the structured local data store.
7. External systems use adapter traits; Phase 2 uses mocks.
8. Browser communication uses a separate Rust native-host helper and user-only Unix socket.
9. Styling uses semantic CSS tokens and local components.
10. Store aggregate blocked-attempt counts, not browsing history.
11. Production audio is behind a Rust adapter.

### Resolved during Phase 2

1. Identifiers: Tauri `com.deepify.desktop`, native host
   `com.deepify.browser`, Firefox extension `focus@deepify.local`.
2. The exact nixpkgs revision and Cargo/npm dependency graphs are committed in
   `flake.lock`, `Cargo.lock`, and `package-lock.json`.
3. Production audio uses Rodio with Symphonia MP3 decoding, `id3` metadata, and
   `notify` folder watching behind `AudioEngine`.
4. Phase 2 uses a checked-in typed TypeScript command boundary; backend and
   reviewer tests guard behavior without adding a binding generator.

### Later implementation follow-ups

1. Build the exact implicit system-component app-ID list from the target NixOS/Niri inventory.
2. Validate hard-crash and cross-component recovery.
3. Validate real PipeWire device loss/switch/recovery.
4. Publish the signed Firefox XPI and verify Zen's manifest path.
5. Replace all mock restriction health labels before an enforcing build is distributed.
