# Phase 2 acceptance record

Date: 2026-08-28. Target: NixOS, Niri, Wayland. Restriction adapters are mocks.

## Automated gates

| Gate | Result |
| --- | --- |
| `npm test` | PASS — 8 Vitest UI tests plus 7 Node contract/reviewer tests |
| `npm run typecheck` | PASS — real `tsc --noEmit` |
| `npm run lint` | PASS — real ESLint |
| `npm run build` | PASS — Vite production bundle with embedded Tauri assets |
| `npm run format:check` | PASS — Prettier |
| `cargo test -p deepify-desktop --features custom-protocol --locked --offline` | PASS — 22 tests; the live-output test is ignored by default |
| `cargo clippy -p deepify-desktop --features custom-protocol --all-targets --locked --offline -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| production Rodio test with `DEEPIFY_AUDIO_FIXTURE=/tmp/deepify-rodio-fixture.mp3` and `--ignored` | PASS — decoded and controlled a real MP3 through the live PipeWire output |
| `node validation/p1-22-firefox-spike/test/static-test.js` | PASS |
| `npm --prefix validation/p1-23-local-audio-spike test` | PASS — 5 checks |
| `nix flake check --no-build --all-systems` | PASS — x86_64 and aarch64 outputs evaluate |
| `nix build .#checks.x86_64-linux.default --no-link` | PASS — full hermetic package/check derivation |
| `npm audit --audit-level=moderate` | PASS — 0 vulnerabilities before the final source-only patches; no dependency changed afterward |

Rust storage coverage uses temporary rusqlite databases and exercises migration
reopen, repository CRUD, invalid values, the one-active-session constraint,
abandoned-session recovery, queue/music persistence, and atomic whitelist
validation. Fake-clock tests cover start, pause, resume, completion, early end,
runtime failure, cleanup failure, and recovery.

## Runtime and visual checks

- The Nix-packaged executable launched under Niri/Wayland with data isolated in
  the `com.deepify.desktop` Tauri directory and loaded embedded assets without a
  development server.
- A second launch exited without creating a second process/window; the existing
  instance remained.
- The current WebKitGTK/Wry resize path produces incorrect webview geometry when
  Niri immediately tiles and resizes a new GTK3 window. Deepify therefore ships
  a fixed 960×720 window, which maps as a 972×732 decorated floating surface on
  this target. The upstream sizing defect is tracked in
  [Wry issue 1727](https://github.com/tauri-apps/wry/issues/1727).
- `validation/phase2-screens/01` through `14` cover every approved Phase 2 UI
  state and the extension blocked-page preview using production components.
- Focus-visible styles, semantic landmarks/labels/status/alert/dialog roles,
  active-session navigation removal, and `prefers-reduced-motion` handling were
  inspected; Testing Library drives the primary controls by accessible names.

The workstation auto-locked during the last post-workaround screenshot attempt,
so no screenshot of the fixed Nix window is claimed. This does not substitute a
synthetic image for runtime evidence; the mapped geometry and process/window
checks are recorded separately from the 14 component-state images.

## P1-24 mapping for the Phase 2 prototype

“PASS (mock)” means the approved behavior is reproduced deterministically through
the explicit Phase 2 adapter, not that enforcement exists.

| P1-24 criterion | Phase 2 result | Next validation |
| --- | --- | --- |
| Block session start for an identified non-whitelisted app | PASS (mock) | Real Niri inventory in Phase 4 |
| Label and allow missing/unknown app IDs | PASS | Real Niri inventory in Phase 4 |
| Ignore terminal-hosted processes; classify the terminal window | Documented only | Phase 4 |
| Implicitly allow Deepify/system components | PASS (mock/UI); final system list pending | Phase 4 inventory |
| Require a healthy paired Firefox extension | PASS (mock) | Phase 3 |
| Show blocked experience for a non-whitelisted domain | Preview + shared contract PASS | Phase 3 interception |
| Make existing non-whitelisted tabs inaccessible | PASS (mock) | Phase 3 |
| Respond to a newly opened blocked app | PASS (mock) | Phase 4 |
| Remove restrictions when a session ends | PASS (mock cleanup/failure injection) | Phase 3/4 integrations |
| Keep restrictions active while paused | PASS |
| Keep whitelist read-only while active/paused | PASS |
| Reject malformed whitelist input atomically with a clear error | PASS |
| Restore surviving blocked tabs on end | Contract/spike PASS | Phase 3 production adapter |
| Runtime restriction failure ends the session and cleans up | PASS (mock) | Phase 3/4 disconnects |
| Interrupt rather than resume after lifecycle discontinuity | Startup recovery PASS; other events deferred | Suspend/logout/reboot/shutdown/hard crash |
| Run one desktop instance | PASS |
| Keep local history with focus time, attempts, and reason | PASS |
| Enable/restore Noctalia Do Not Disturb | PASS (mock) | Phase 4 |
| Require duration; keep intention/music optional | PASS |
| Send completion notification | PASS |
| Rerun setup from Settings | PASS |
| Play optional local MP3 without internet | PASS |
| Prefer ID3, then documented filename fallbacks | PASS |
| Recursively rescan folders and reconcile the queue deterministically | PASS |
| Show device loss and recover playback | State/adapter PASS; real switch deferred | Live PipeWire switch |
| Require no account/sign-in | PASS |
| Use the minimal Obsidian-inspired default UI | PASS |
| Persist the selected color scheme | PASS |
| Report restriction failures clearly | PASS (mock) |

## Explicitly deferred

- Production Firefox/Zen pairing, signing, interception, heartbeat, and tab restoration
- Production Niri monitoring/closure/focus restoration and system-component inventory
- Production Noctalia prior-state preservation
- Hard-crash cross-component cleanup and suspend/logout/reboot/shutdown behavior
- Real PipeWire device loss/switch/recovery
- Installed-package upgrade behavior

These are not Phase 2 failures or silent passes; they are the Phase 3/4 and
release-platform gates identified by the approved plan.
