# Phase 2 handoff

**Status:** Complete — functional prototype with explicitly simulated restriction integrations.

Phase 2 now provides a runnable Tauri/React desktop application whose Rust
backend owns the session state machine and local SQLite data. It includes the
five-step setup wizard, Focus Room lifecycle, failure summaries, history,
whitelist management, settings and themes, local MP3 import/playback, folder
watching and deterministic rescans, completion notifications, startup recovery,
and single-instance behavior. Whitelist probes are evaluated locally without
being saved; history reports daily and weekly totals; local diagnostic IDs
correlate user-visible failures without recording sensitive content.

## Verified deliverable

- Immutable Nix, Cargo, and npm locks plus permanent identifiers:
  `com.deepify.desktop`, `com.deepify.browser`, and `focus@deepify.local`.
- Real Tauri 2, React, TypeScript, Vite, Vitest, ESLint, Prettier, rusqlite,
  Rodio/Symphonia, ID3, and Notify implementations—no placeholder toolchain.
- Backend-authoritative start, work, pause, resume, completion, early end,
  interruption, cleanup recovery, snapshot events, and persistence.
- Parameterized SQLite migrations and repositories for every Phase 2 table,
  including the database constraint allowing only one active session.
- Separate browser, application, and Do Not Disturb adapter traits with mock
  health/latency configuration, failure injection, observable repair, and honest
  partial-cleanup reporting.
- All approved desktop states plus the packaged extension blocked-page preview;
  the UI persistently labels restriction protection as simulated.
- Production MP3 decode/control through the live system output, ID3 and filename
  metadata fallback, recursive sources, persistent queue order, and nonfatal
  missing-file/device states.
- A user-triggered output retry drops the stale sink and opens the current
  default/new output device without interrupting the focus session.
- Hermetic Nix package/check build and native Niri/Wayland launch. The window is
  fixed at 960×720 to avoid a known GTK3/WebKitGTK resize defect under current
  Wayland tiling compositors.

Exact commands and results are recorded in `validation/PHASE2_CHECKS.md`.

## Deliberately not claimed

Phase 2 does not enforce restrictions. Firefox/Zen interception and restoration,
Niri window enforcement, Noctalia state changes, pairing/signing, hard-crash
cross-component cleanup, suspend/logout/reboot/shutdown behavior, and real
PipeWire device switching remain Phase 3/4 or release-platform validation.
Private windows, other browser profiles, unidentified applications, and
terminal-hosted commands remain disclosed MVP bypasses.

## Phase 3 browser backlog

Replace `BrowserRestrictionAdapter` with the packaged native-host/extension
transport without changing the session domain model. Enforce the pairing token,
run the shared URL fixtures in the extension, maintain heartbeat health, retain
original URLs without persisting browsing history, restore every surviving tab
on all stop/failure paths, and validate signed installation in Firefox and Zen.
