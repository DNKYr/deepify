# Phase 2 handoff status

The shared workspace contains the domain/session prototype, mock restriction
adapter, local SQLite schema boundary, URL contract fixtures, setup primitives,
and deterministic MP3 queue rules. Restriction integrations are intentionally
simulated.

## Verified

- Fake-clock start/pause/resume/complete/early-end/recovery behavior.
- Mock activation and cleanup failures never claim healthy cleanup.
- Migration creation/reopen, settings CRUD, and one-active-session constraint
  using the local SQLite CLI adapter test harness.
- Recursive MP3 filtering, filename metadata fallback, deterministic queue
  ordering, and output-device waiting state.
- Four Node contract/UI tests, including all shared URL fixture vectors.

## Open gates

The prototype is not a Phase 2 release. Tauri and rusqlite dependency
installation, real TypeScript/ESLint/Vitest checks, Nix shell/flake checks,
production decoder/device playback, backend Tauri command registration,
completion notifications, full history/settings routes, and all real Firefox,
Niri, Noctalia, crash/suspend/PipeWire validation remain open. These are not
silently marked passed.

The next implementation step is to restore network access (or provide a valid
cached nixpkgs/crates/npm source), replace the CLI SQLite fallback with
rusqlite, and run the full acceptance suite in a Nix development shell.
