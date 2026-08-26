# Deepify — Phase 2 Functional Prototype Implementation Plan

## Status

- **Stage:** Planning
- **Depends on:** Approved Phase 1 design and acceptance criteria
- **Architecture:** [`ARCHITECTURE.md`](ARCHITECTURE.md)
- **Goal:** Build a local desktop prototype of the approved UX and complete session lifecycle using simulated restriction integrations

Phase 2 is not an enforcement release. Firefox, Niri, and Noctalia behavior is represented through explicit mock adapters. The prototype must visibly identify simulated protection and must not claim that apps or websites are actually blocked.

---

## 1. Phase 2 Outcomes

At the end of Phase 2, the repository should contain a runnable Tauri desktop application that supports:

- The approved Obsidian-inspired shell and built-in theme tokens
- Setup wizard with simulated integration health
- Required duration and optional intention/playlist
- Full timer lifecycle: start, work, pause, resume, complete, and end early
- Failure simulation and interrupted-session summary
- Local SQLite settings and session history
- Whitelist creation, strict validation, editing, and test simulation
- Optional MP3 file/folder import and playback
- Persistent queue and deterministic folder rescans
- Session completion notification
- Single-instance behavior
- Startup recovery for an abandoned active session
- Automated tests for domain behavior, persistence, and primary screens

Real website and application enforcement remain Phase 3 and Phase 4 work.

---

## 2. Delivery Strategy

Build a thin vertical session flow first, then expand outward. Avoid implementing every screen as a disconnected static page.

```text
Scaffold
  -> backend session state + SQLite
  -> idle Focus Room
  -> mock validation
  -> working/paused timer
  -> cleanup + summary
  -> history
  -> setup/settings/whitelist
  -> local audio
  -> recovery and acceptance pass
```

Every milestone must leave the application runnable and tests passing.

---

## 3. Phase 2 Work Items

## P2-01 — Resolve bootstrap identifiers and pin the toolchain

**Priority:** Critical  
**Status:** In progress — identifiers and source scaffold recorded; immutable lock validation blocked by truncated network fetch.

### Resolved inputs

- MVP product/display name: **Deepify**
- Desktop executable name: `deepify`
- nixpkgs release baseline: 26.05

### Remaining decisions and validation

- Exact locked nixpkgs 26.05 revision
- Reverse-DNS Tauri identifier
- Browser native-host ID
- Firefox extension ID namespace
- Pinned Rust, Node, Tauri, and plugin versions compatible with the locked nixpkgs revision
- Generated TypeScript binding approach after compatibility testing

### Deliverables

- Architecture decision record for identifiers
- `flake.nix` and `flake.lock`
- Root `Cargo.toml`
- Root `package.json` and `package-lock.json`
- Toolchain and contributor commands documented

### Acceptance

- `nix develop` enters a shell containing every build dependency.
- Tool versions are reproducible from committed locks.
- No global npm package is required.

---

## P2-02 — Create the Tauri/React production scaffold

**Priority:** Critical  
**Status:** In progress — Tauri configuration, React/Vite shell, semantic themes, and single-instance configuration recorded; Tauri dependency installation remains blocked by unavailable crates.io.

### Tasks

- Create `apps/desktop` with Tauri 2, React, TypeScript, and Vite.
- Configure Rust formatting, Clippy, frontend formatting, ESLint, and TypeScript checks.
- Add React Router and the shared desktop shell.
- Add semantic CSS tokens and the default Obsidian-like dark scheme.
- Add a second simple built-in scheme to prove theme-token switching.
- Configure narrow Tauri capabilities; do not enable broad shell access.
- Add single-instance support.
- Preserve committed disposable spikes under `validation/` without converting them into production code.

### Deliverables

- Runnable empty shell with navigation placeholders
- Nix development command
- Test and quality commands
- Initial README

### Acceptance

- Desktop application launches under Niri/Wayland.
- `cargo test`, Clippy, TypeScript, lint, and frontend tests run from the Nix shell.
- A second launch focuses or reports the existing instance rather than creating another independent app.

---

## P2-03 — Implement domain types and SQLite migrations

**Priority:** Critical  
**Status:** In progress — migration SQL, SQLite adapter boundary, domain DTOs, and repository tests recorded; rusqlite integration remains blocked by unavailable crates.io.

### Tasks

- Define session states and finish reasons.
- Define typed duration, timer snapshot, integration health, and summary DTOs.
- Define whitelist, settings, music source, track, queue, and history DTOs.
- Add SQLite migration runner.
- Create repositories for sessions, settings, whitelist, music, and queue.
- Enforce one active/starting/ending session at the database level.
- Add local database backup/error behavior appropriate for an MVP.
- Generate or synchronize frontend command DTOs.

### Initial migration scope

- `sessions`
- `whitelist_entries`
- `music_sources`
- `music_tracks`
- `queue_entries`
- `settings`
- `integration_state`

### Tests

- Fresh migration
- Reopening an existing database
- Repository CRUD
- Invalid enum/value rejection
- Only one active session
- Malformed whitelist input cannot be inserted through services

### Acceptance

- No frontend component accesses storage directly.
- Migrations are repeatable and tested with a temporary database.
- An abandoned active record can be found reliably at startup.

---

## P2-04 — Implement the backend session engine

**Priority:** Critical  
**Status:** Completed for deterministic domain scope; Tauri command persistence/event wiring remains pending and platform validation is not claimed.

### Tasks

- Implement `SessionService` with an injected clock.
- Implement start, pause, resume, timer completion, and early end.
- Track focused and paused durations separately.
- Persist before and after every state transition.
- Add periodic Working-state checkpoints.
- Implement finish reasons:
  - `completed`
  - `ended_early`
  - `interrupted`
  - `extension_or_app_crash`
- Add startup recovery for abandoned sessions.
- Emit full session snapshots.
- Reject illegal and concurrent transitions.

### Tests

- Start requires a positive duration.
- Pause freezes focused-time accumulation.
- Restrictions remain logically active in Paused.
- Resume continues from the persisted focused time.
- Early end has no delay and requires no reason value.
- Timer completion records the correct actual duration.
- Runtime adapter failure interrupts and ends the session.
- Restart recovery marks an abandoned session interrupted.
- Duplicate commands are safe or return a typed conflict.

### Acceptance

- The timer remains correct when the frontend is reloaded.
- The frontend cannot create an impossible state through command ordering.
- The state machine passes deterministic fake-clock tests.

---

## P2-05 — Add mock restriction and health adapters

**Priority:** Critical  
**Status:** Completed for simulated adapter contract; Tauri command/event wiring and real integration remain Phase 3/4 work.

### Tasks

- Define `BrowserRestrictionAdapter`, `ApplicationRestrictionAdapter`, and `DoNotDisturbAdapter` traits.
- Implement mock adapters with configurable health and latency.
- Simulate clean preflight, blocked applications, runtime blocked attempts, startup failure, and runtime failure.
- Implement `RestrictionCoordinator` startup and idempotent cleanup ordering.
- Expose integration-health snapshots and last-check timestamps.
- Add an unmistakable **Simulated protection** development label.

### Failure injection tests

Inject a failure at every startup and cleanup step and assert:

- Working is never reported after partial startup.
- Best-effort cleanup is attempted.
- The session receives the correct finish reason.
- The summary reports incomplete cleanup honestly.
- A new session cannot start until required recovery succeeds.

### Acceptance

- All approved preflight and failure UI states can be reproduced without real integrations.
- No simulated state is presented as real enforcement.

---

## P2-06 — Build the vertical Focus Room flow

**Priority:** Critical  
**Status:** In progress — Focus Room React shell and backend command/event boundary recorded; full Testing Library/Vitest flow remains pending dependency installation.

### Screens

1. Focus Room — Not working
2. Pre-session validation
3. Pre-session blocked-app resolution
4. Focus Room — Working
5. Focus Room — Paused
6. End-session confirmation
7. Session summary
8. Session History

### Tasks

- Bind the approved P1-20 hierarchy to backend snapshots.
- Require duration; keep intention and playlist optional.
- Proceed automatically after successful checks.
- Show mock blocked apps only after validation fails.
- Keep navigation restricted to Focus Room during Working and Paused.
- Show compact health, blocked count, latest notice, and audio placeholder.
- Keep restrictions logically active while Paused.
- Send the completion notification.
- Show actual focused time, blocked count, and finish reason.
- Return to the idle room after summary dismissal.
- Add local daily/weekly history totals.

### Frontend tests

- Required-duration validation
- Automatic transition after successful checks
- Blocked-app resolution flow
- Pause/resume labels and keyboard order
- Whitelist/settings navigation unavailable while active
- Early-end confirmation
- Completion and failure summaries
- Accessible status and error announcements

### Acceptance

A user can complete the entire approved session flow with mock integrations, close/reopen the frontend, and see persisted history.

---

## P2-07 — Implement whitelist rules and management

**Priority:** High  
**Status:** Completed for domain and shared-contract scope; management command/UI wiring remains pending.

### Application rules

- Store Niri-style Wayland app IDs as strings.
- Keep focus-app and tentative system allowances implicit and non-removable.
- Label missing/unknown simulated IDs as **Unidentified app — allowed in MVP**.
- Keep whitelist read-only during Working and Paused.

### Website rules

Implement and test the Phase 1 rules:

- Restrict only HTTP(S).
- Domain includes all subdomains.
- Path uses prefix matching.
- Ports are ignored.
- HTTP and HTTPS are equivalent.
- Localhost, loopback, and local-network addresses are allowed.
- Public IP addresses require explicit rules.
- Non-HTTP(S) schemes are allowed.

### Validation behavior

- Reject malformed values atomically.
- Do not save or partially apply invalid input.
- Preserve invalid draft input for correction.
- Return stable field-level errors.

### Contract fixtures

Create `contracts/url-rule-cases.json` now. The same fixtures must later run in the Firefox extension.

### Acceptance

- Whitelist CRUD and test simulation work locally.
- Domain/path/IP test vectors pass in Rust and TypeScript test harnesses.
- No malformed record reaches SQLite.

---

## P2-08 — Implement setup, Settings, and integration health

**Priority:** High  
**Status:** In progress — settings persistence primitives and rerunnable setup state are present; five-step wizard and health screens remain pending.

### Tasks

- Build the five-step setup wizard.
- Use mock Niri, Noctalia, and browser checks in Phase 2.
- Disclose MVP bypasses and simulated enforcement clearly.
- Make music optional.
- Persist setup completion.
- Allow the entire wizard to be rerun from Settings.
- Add default duration, built-in color scheme, and completion-notification settings.
- Add integration detail and simulated repair actions.
- Add emergency-recovery explanation using mock cleanup.

### Acceptance

- Required setup steps cannot be skipped silently.
- Music can be skipped explicitly.
- Setup can be rerun without deleting existing local data.
- Theme and settings persist across launch.

---

## P2-09 — Integrate production local MP3 support

**Priority:** High  
**Status:** Completed for library/queue contract scope; production decoder/device adapter and ID3 parsing remain pending.

### Step 1: adapter selection spike

Compare the pinned Rust-compatible choices for:

- MP3 decode/playback
- Pause/resume/seek and volume
- Fade-out
- Output device enumeration and recovery
- ID3 metadata
- NixOS build compatibility

Select the smallest reliable implementation behind `AudioEngine`. Record the choice in `ARCHITECTURE.md`.

### Step 2: library

- Import individual MP3 files using the Tauri dialog plugin.
- Import folders as recursive live playlist sources.
- Ignore non-MP3 files.
- Read ID3 title/artist/album.
- Apply filename fallbacks.
- Persist direct references and cached metadata.
- Watch folders and perform fallback rescans.
- Mark missing direct files unavailable.

### Step 3: queue and playback

- Persist queue order.
- Retain existing paths, remove deleted folder paths, and append newly discovered paths lexically.
- Treat rename/move as remove-plus-add.
- Implement previous, play/pause, next, and volume.
- Start selected music with the session.
- Fade and stop when the session ends.
- Exclude shuffle and repeat.

### Step 4: output health

- Expose active device and device-loss state.
- Enter `waiting_for_output_device` when needed.
- Recover onto an available default/new device.
- Let focus sessions continue without music.

### Acceptance

- P1-23 behavior passes against the production adapter, not only the disposable spike.
- Audio failure never ends or weakens focus restrictions.
- Missing files and missing output devices are visible but nonfatal.

---

## P2-10 — Recovery, robustness, and privacy pass

**Priority:** Critical  
**Status:** Pending — recovery tests exist for mock backend state; real crash/suspend/platform checks remain unperformed.

### Tasks

- Enforce single-instance behavior.
- Mark abandoned active sessions interrupted at startup.
- Run idempotent mock cleanup before allowing a new session.
- Handle frontend reload without losing backend state.
- Handle SQLite and filesystem errors with typed messages.
- Avoid recording blocked browsing destinations.
- Verify local-only operation with network unavailable.
- Add local diagnostic correlation IDs without sensitive content.
- Validate reduced motion, keyboard navigation, and focus management.

### Deferred platform tests

Document—but do not falsely mark complete—the release tests requiring real integrations:

- Hard process crash during active Firefox/Niri enforcement
- Suspend, logout, reboot, and shutdown cleanup
- Real PipeWire device switch
- Signed extension/native-host installation

### Acceptance

- Recovery behavior is deterministic under mock failure injection.
- The application never claims cleanup succeeded when a mock reports failure.
- No authentication, analytics, or cloud request is present.

---

## P2-11 — Phase 2 acceptance and handoff

**Priority:** Critical  
**Status:** Pending — acceptance handoff cannot be marked complete until the blocked dependency and platform checks are resolved.

### Tasks

- Run all Rust, frontend, migration, contract, and spike regression tests.
- Review every P1-24 criterion.
- Mark criteria passed by the functional prototype.
- Mark real-integration criteria as Phase 3/4 pending rather than failed or complete.
- Capture screenshots or short recordings of every P1-20 state.
- Document known limitations and developer run commands.
- Define the Phase 3 browser-integration backlog from the production adapter contract.

### Phase 2 exit criteria

- The desktop prototype is runnable through `nix develop`.
- The complete timer flow works with simulated integrations.
- Session state, settings, whitelist, history, music sources, and queue persist locally.
- Local MP3 playback works through the selected production adapter.
- Approved screens are keyboard accessible and match the wireframe hierarchy.
- Mock integration failure produces correct cleanup and summary behavior.
- Automated tests are green.
- The UI clearly says that restriction protection is simulated.
- Phase 3 can replace the browser mock without changing the session domain model.

---

## 4. Test Commands Target

The scaffold should converge on these root commands:

```text
npm run dev              # Tauri development app
npm run test             # Frontend and contract tests
npm run typecheck
npm run lint
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
nix flake check
```

Exact scripts are established in P2-01/P2-02 and documented in the root README.

---

## 5. Dependency Order

```text
P2-01 identifiers/toolchain
  └─ P2-02 scaffold
       └─ P2-03 domain/storage
            ├─ P2-04 session engine ─┐
            ├─ P2-05 mock adapters ──┼─ P2-06 vertical Focus Room
            ├─ P2-07 whitelist ──────┤
            └─ P2-09 audio ──────────┘
                         │
               P2-08 setup/settings
                         │
               P2-10 recovery/privacy
                         │
               P2-11 acceptance/handoff
```

P2-07 and the initial P2-09 adapter spike can proceed in parallel after the storage and command boundaries exist.

---

## 6. Phase 2 Acceptance Mapping

### Expected to pass in Phase 2

- Timer input and lifecycle behavior
- Pause keeps logical restrictions active
- Whitelist read-only while active
- Strict malformed-input rejection
- Local persistence and history
- Single-instance behavior
- Setup rerun
- Notifications
- Themes
- MP3 import, metadata, queue, and playback
- Visible health/failure reporting
- Local-only/no-auth operation
- Approved screen hierarchy and accessibility basics

### Simulated in Phase 2; enforced later

- Paired Firefox health requirement
- Website blocking and tab restoration
- Pre-session Niri window blocking
- Runtime blocked-window handling
- Noctalia DND state changes
- Restriction-component disconnect cleanup

### Requires later real-platform validation

- Hard-crash cross-component cleanup
- Suspend/logout/reboot/shutdown behavior
- Real PipeWire device switching
- Signed Firefox/Zen installation
- Nix package installation and upgrade behavior

---

## 7. Risks and Controls

| Risk | Control |
| --- | --- |
| UI accidentally implies mock protection is real | Persistent development label and mock health wording |
| Timer diverges after frontend reload | Rust authority and full snapshot reload |
| Partial startup leaves inconsistent state | Persist `starting`, idempotent reverse cleanup, failure injection tests |
| SQLite schema changes break local data | Versioned migrations and migration tests |
| Audio library fails under NixOS | Adapter spike before broad UI integration |
| Folder watcher misses events | Deterministic rescan on startup/open plus watcher |
| Rust/TypeScript DTO drift | Generated bindings or checked-in schema generation |
| Browser matching differs from desktop validation | Shared URL-rule fixtures |
| Overengineering before vertical flow | One desktop backend crate; split only native host |
| Phase 1 bypasses are forgotten | Show them in setup, health, and acceptance documentation |

---

## 8. Immediate Next Action

Begin the Codex loop with P2-01. Incorporate its exact flake/toolchain validation and remaining identifier decisions before beginning P2-02. Do not scaffold Tauri until the reverse-DNS application identifier and native-host naming are chosen, because those values affect local data paths, desktop integration, browser manifests, and future upgrades.

## Implementation evidence — 2026-08-26

The following checks were run in the shared workspace:

```text
cargo fmt --all                         PASS
cargo test --workspace --offline        PASS (14 backend tests)
cargo clippy --workspace --all-targets --offline -- -D warnings  PASS
npm test                                PASS (2 contract/UI source tests)
npm run typecheck                       PASS (temporary source scan; real tsc awaits npm install)
npm run lint                            PASS (temporary source scan; real ESLint awaits npm install)
nix flake metadata ...                 BLOCKED: truncated tar archive while fetching nixpkgs
cargo fetch                             BLOCKED: static.crates.io DNS unavailable
npm install --package-lock-only        BLOCKED: registry unavailable; root lock has no resolved dependency graph
```

These results do not constitute Phase 2 completion. The Tauri shell, real
rusqlite adapter, production MP3 decoder, Testing Library suite, Nix check,
real platform enforcement, and crash/suspend/device validation remain pending.
Restriction behavior is explicitly simulated throughout the prototype.
