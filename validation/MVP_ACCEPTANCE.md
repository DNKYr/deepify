# MVP acceptance audit — in progress

Baseline: the 29 approved P1-24 criteria in PHASE1_ISSUES.md. Phase 4 and Phase 5
validation records identify exact commands. “Implemented” below is not a claim
of complete release verification.

| # | Requirement | Current evidence / remaining verification |
| --- | --- | --- |
| 1 | Identified blocked apps prevent start | Real Niri refusal/preflight harness and exact-ID unit tests pass |
| 2 | Unidentified app IDs are visibly labeled and allowed | PASS: exact-ID policy plus frontend regression shows unidentified count even without blocked windows |
| 3 | Classify terminals by app ID, ignore hosted processes | Niri DTO/policy consume no process contents; exact-ID tests pass |
| 4 | Implicit Deepify/system allowances | PASS: exact-ID policy; live desktop and native file chooser both use `com.deepify.desktop`; shell/portal/authentication allowances are explicit |
| 5 | Paired healthy browser required | PASS: explicit pairing, fresh health, second-profile rejection, and actual desktop setup/pairing guards |
| 6 | Block new unapproved domains | Current Firefox and Zen production harnesses pass |
| 7 | Block existing unapproved tabs | Current Firefox and Zen production harnesses pass |
| 8 | Standard response to new blocked apps | Real two-window close/refusal harness passes |
| 9 | Remove restrictions at session end | PASS: combined browser/Niri/Noctalia cleanup plus real desktop completion, early end, exit and restart matrix |
| 10 | Keep restrictions while paused | Real combined paused session plus browser paused-navigation tests pass |
| 11 | Whitelist read-only while active/paused | PASS: lifecycle transitions serialized; actual desktop rejects whitelist mutation while paused |
| 12 | Reject malformed whitelist input without saving | Rust normalization and frontend validation tests pass |
| 13 | Restore blocked tabs automatically | Current browser normal/disconnect tests and combined cleanup pass |
| 14 | Any integration failure ends/cleans up session | PASS: coordinator matrix plus actual desktop Niri loss, DND loss, browser disconnect and heartbeat timeout; visible cleanup/retry |
| 15 | Suspend/clock/crash/logout/reboot/shutdown interrupt, never resume | PASS for actual desktop SIGTERM/SIGKILL restart, injected logind sleep/shutdown and clock unit matrix; no physical power-state changes performed |
| 16 | Single desktop instance | PASS: current isolated desktop smoke verifies a second instance exits without another window |
| 17 | Local history stores focus time, counts, reason | PASS: SQLite plus actual desktop persistence; same-second ordering and summary/history publication races fixed |
| 18 | Enable and restore prior Noctalia DND | Live false/true prior-state and combined-session tests pass |
| 19 | Duration required; intention/music optional | Frontend and domain tests pass |
| 20 | Completion notification | PASS: actual desktop timer completion delivered a notification to the private D-Bus fixture |
| 21 | Rerun setup from Settings | PASS: rerun-setup backend guard exercised while paused; keyboard confirmation regressions pass |
| 22 | Optional offline MP3 playback | PASS: silent real MP3 decoder/output test; optional-music frontend behavior also tested |
| 23 | ID3 and filename fallbacks | PASS: ID3 title/artist/album precedence and blank-tag filename fallbacks tested |
| 24 | Recursive live folder rescan and deterministic queue | PASS: recursive watcher, symlink-cycle, selection/order, SQLite reopen, library refresh, missing-folder startup and paused-file deletion |
| 25 | Visible audio-device loss and recovery | PASS: actual desktop + Rodio/ALSA + private PipeWire output loss/replacement/retry; physical default output also tested separately |
| 26 | No account or sign-in | PASS: source/capability/CSP review; no account flow, application HTTP client, analytics or remote assets |
| 27 | Minimal default Obsidian scheme | PASS: actual desktop state captures; Obsidian default, Mist contrast fix, keyboard modal/control regressions |
| 28 | Scheme persists across launches | PASS: actual desktop saved Mist theme, then verified it across graceful and hard-kill relaunch |
| 29 | Clear, visible restriction failures | PASS: actual component-failure UI, persistent cleanup outcomes and repair; command errors no longer overlap the banner |

## Other delivery gates

- Detailed Phase 3 deliverables are mapped in `PHASE3_ACCEPTANCE_AUDIT.md`;
  historical evidence is preserved.
- Phase 4 target acceptance and handoff are complete; see `PHASE4_HANDOFF.md`.
- The Nix release candidate built, flake evaluation/checks passed, and installed
  desktop/helper/XPI checks passed. The final watcher-loop correction is being
  rebuilt and remeasured; see `PHASE5_HANDOFF.md` for the remaining release work.
- Signed XPI installation needs a signed artifact/signing channel. Development
  installation is proven separately; external signing is still unverified.
- Mozilla consent declaration and optional-counting behavior require the release
  decision described in `BROWSER_RELEASE_READINESS.md`.

No full-MVP completion claim is made while any of these items remains open.
