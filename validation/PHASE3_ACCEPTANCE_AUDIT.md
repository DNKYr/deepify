# Phase 3 acceptance audit against current implementation

Updated 2026-09-25. `PHASE3_HANDOFF.md` and `PHASE3_CHECKS.md` retain their historic
Phase 3 scope; later corrections and current command evidence are recorded in
`PHASE5_CHECKS.md`. This audit reconciles the named P3-01 through P3-12 deliverables.

| Item | Current artifact and evidence |
| --- | --- |
| P3-01 contract/interception decision | `contracts/README.md`, complete version-1 JSON schema, Rust frame validator, 38 URL fixtures; Draft 2020-12 validation covers all 15 message types and 317 rejected mutations; current Firefox/Zen prove existing/new tabs, Back/reload, redirects, containers and restoration |
| P3-02 protocol/native host | `crates/browser-protocol`, `crates/browser-native-host`; byte/frame limits, strict payload validation, fragmented/truncated data, private socket-path tests; installed helper transports actual Firefox/Zen traffic |
| P3-03 broker/persistence/pairing | `browser.rs`, migration 002 and SQLite pairing/checkpoint repository; explicit acceptance, one profile, constant-time hash comparison, private owned socket and peer credentials, bounded peers/requests/events; wrong/second-token and active-forget tests; desktop reconnect/repair/restart matrix |
| P3-04 extension connection/health | Production `background.js`; stored random token, reconnect, mandatory capabilities, status and heartbeat; permission-loss/API-error regressions and actual desktop heartbeat timeout; no URL-bearing diagnostics |
| P3-05 restriction/blocked page | Production matcher uses parsed IP classes, case-sensitive path prefixes and shared fixtures; actual Firefox/Zen test existing/new navigation, local allowance, Back/reload/redirects, paused blocking, containers and closed tabs; blocked page polls backend-owned time without stealing keyboard focus |
| P3-06 start/stop coordinator | Browser adapter activated before DND/application enforcement; write-ahead browser checkpoint; startup/cleanup failure matrix; actual combined browser/Niri/Noctalia finish and induced DND failure |
| P3-07 counts/timer/runtime failures | Deduplicated session-scoped events, one-second timer state updates, 20-second health schedule and bounded response timeout; serialized desktop lifecycle prevents duplicate summaries/transactions; actual disconnect and timeout interruption with visible notice |
| P3-08 restoration/recovery | In-memory first-original map, serialized async lifecycle, retained failed restores and exact-session retry; normal/native-loss/container restoration; closed tabs stay closed; graceful browser quit/restart and desktop checkpoint recovery; hard browser crash memory boundary remains explicit |
| P3-09 setup/health UI | Actual desktop setup/pairing/preflight/repair; second-profile and active-setting guards; frontend tests for keyboard confirmation and visible failures; production simulation controls removed |
| P3-10 Nix artifacts | Current desktop, host/absolute manifest and five-asset deterministic unsigned XPI built; installed desktop and packaged helper/XPI tested outside the development shell; desktop entry/icon and GTK schema wrapper verified |
| P3-11 browser validation | `validation/phase3-firefox/production-harness.cjs`; Firefox 156.0 and Zen 1.22.3b; disposable profiles, real Rust native host and production extension; current lifecycle/container/redirect evidence in Phase 5; signed permanent install remains external |
| P3-12 handoff | Historical `PHASE3_HANDOFF.md`, current contract/privacy/architecture/README, this audit, Phase 4 plan/handoff and Phase 5 acceptance matrix; signed installation is explicitly unverified until an artifact/channel is available |

The tests cover the executable Rust and production extension, not the disposable
Node spike host. The other-UID check uses Linux peer credentials and owner-only
socket paths; validation did not impersonate a different real user or change
system users. No normal browser profile was modified. The original-URL memory
boundary means a hard browser/extension-process crash can leave a blocked page
whose original cannot be recovered; persisting browsing destinations is excluded
by the approved design.

For signed installation, the production harness accepts `DEEPIFY_EXTENSION_PATH`
pointing to an XPI and `DEEPIFY_SIGNED_INSTALL=1`. It then requests permanent
installation with signature enforcement enabled in the disposable profile. The
unsigned artifact must not be presented as passing this gate.
