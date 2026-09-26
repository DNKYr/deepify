# Phase 4 validation record

Date: 2026-09-23. Target: NixOS, Niri 26.04, Noctalia Shell `fe6fa12`.

## Verified

| Check | Evidence |
| --- | --- |
| Exact app identity, complete window inventory, missing identities and browser/system allowances | Rust `niri::tests`; no process/terminal-child inspection or window-title persistence |
| Deduplication, multiple windows, refused closes, targeted closure failure | Rust window-policy tests |
| Invalid event/EOF detection and event-stream child cleanup | Rust process-backed event-stream tests |
| Bounded command output, timeout and redacted errors | Rust `platform_command::tests` |
| Durable prior DND state, failed restore retry, silent IPC failure, external disable, reopen recovery | Rust `noctalia::tests`, including file-backed SQLite reopen |
| Incomplete cleanup stays visible in history | Rust storage test requires both browser and DND checkpoints cleared before reporting recovery complete |
| Live Niri existing-window preflight and two-window cooperative closure | `nix develop -c cargo run -p deepify-desktop --example phase4_platform -- niri` — PASS |
| Live close refusal and persistent blocked inventory | Same harness, two GTK windows with delete-event refusal — PASS; one close request per window; next preflight refused |
| Live Noctalia enable/readback and restoration from both prior states | `nix develop -c cargo run -p deepify-desktop --example phase4_platform -- dnd` — PASS; initial user setting restored |
| Full Rust workspace | `nix develop -c cargo test --workspace --all-targets` — 52 passed; audio/display hardware tests ignored by default |
| Strict Rust lint | `nix develop -c cargo clippy --workspace --all-targets -- -D warnings` — PASS |
| Frontend and browser-contract regression | `npm test` — 13 Vitest tests and all three Node test files pass |
| Frontend static checks and build | Typecheck, lint, formatting and production Vite build — PASS |

The Niri harness wraps the real adapter's command port with a guard that refuses
to close any window not owned by its fixture app ID. It never inspects or changes
the contents of user windows. It creates two real GTK windows per scenario and
uses the production event-stream reader. Focus targeting is covered by the unit
tests; the harness deliberately does not move focus to existing user windows.

GTK's Wayland app ID came from the program name on this runtime. Both Deepify
and the fixture now set that name before GTK initialization. The fixture's live
close/refusal tests confirm exact identity matching works.

Noctalia's installed legacy CLI is the verified target. Its source exposes
`state.all` and `notifications.enableDND/disableDND`. Newer Noctalia v5 uses a
different CLI; an unsupported installation reports an integration error rather
than silently claiming DND protection.

## Acceptance completion and Phase 5 boundary

- Phase 4's source snapshot build passed; final Phase 5 artifacts have a separate gate.
- Combined Firefox/browser-broker/Niri/Noctalia early-end and induced DND-failure
  scenarios passed. Actual desktop exit/crash, monitor-loss and DND-loss recovery
  also passed in Phase 5; see [PHASE5_CHECKS.md](PHASE5_CHECKS.md).
- Guarded live focus restoration to a disposable allowed window passed on 2026-09-24.
- The target file chooser's actual app ID is `com.deepify.desktop`, verified during
  the complete desktop lifecycle/audio matrix on 2026-09-25.
- Handoff: [PHASE4_HANDOFF.md](../PHASE4_HANDOFF.md).

Phase 5 final packaged installation and external signed-XPI checks remain release
work. This record does not claim a finished release.
