# Phase 5 validation record

Updated: 2026-09-25. This phase is in progress; the full acceptance audit is
[MVP_ACCEPTANCE.md](MVP_ACCEPTANCE.md).

## Current checks

| Check | Result and scope |
| --- | --- |
| `nix develop -c cargo test --workspace --all-targets --features deepify-desktop/custom-protocol` | PASS: 70 tests; two hardware/display tests ignored by default |
| `nix develop -c cargo clippy --workspace --all-targets --features deepify-desktop/custom-protocol -- -D warnings` | PASS |
| Rust formatting and `git diff --check` | PASS |
| `npm test` | PASS: 18 Vitest tests and 18 Node tests across three files |
| Typecheck, lint, frontend formatting, production build | PASS |
| `nix develop -c dbus-run-session -- python3 validation/phase5-desktop-smoke.py` | PASS: current embedded-frontend desktop launches with the exact allowed app ID, a second instance exits without creating a window, and SIGTERM produces graceful exit status 0. Uses isolated app data and session D-Bus; no focus session starts |
| Actual MP3 output adapter | PASS: generated a one-second silent MP3 using the pinned FFmpeg; ran `DEEPIFY_AUDIO_FIXTURE=/tmp/deepify-phase5-silence.mp3 cargo test -p deepify-desktop --lib audio::tests::production_adapter_decodes_and_controls_real_mp3 -- --ignored --test-threads=1` in the Nix shell with the live output device |
| Clock detector | PASS: injected monotonic/boot/realtime samples distinguish suspension and forward/backward clock changes from ordinary scheduling delay; real clock sampling also tested |
| Audio folder reconciliation | PASS: symlink-cycle scan, surviving queue order and current selection, removed current file, SQLite rescan/reopen, overlapping file import |
| Browser socket security | PASS: reject symlink/insecure runtime directories without changing them; preserve live sockets and regular files; recover a verified stale socket |

The production session worker now uses monotonic focus time and the clock detector.
GLib handlers route SIGTERM/SIGINT through normal exit cleanup; logind sleep and
shutdown signals request interruption. Planned exit runs the same restriction
coordinator. Abandoned sessions are recorded as `extension_or_app_crash` on restart.

The Rodio adapter reports stream errors through an atomic flag; a backend worker
sets the visible waiting state. Retry opens the current default output. A paused
track resumes, while a changed track starts from its beginning. Watcher events are
coalesced with a bounded queue, folder scans do not follow directory symlink cycles,
and rescans preserve surviving playlist order in memory and SQLite.

## Desktop lifecycle and audio matrix (2026-09-24)

`nix develop -c dbus-run-session -- env DEEPIFY_AUDIO_VALIDATION=1 python3 validation/phase5-desktop-lifecycle.py` — **PASS**.

This controls the actual embedded-frontend Tauri desktop using WebKit WebDriver.
App data, single-instance D-Bus and the login1/notification services are disposable.
The browser is an explicitly identified protocol peer; actual Firefox/Zen tab
behavior is covered by the separate production harnesses. Niri inventory/event IPC
is real, and its command wrapper refuses every close/focus operation. Noctalia is
real, with its original runtime/display identity restored for IPC; its initial
DND value is restored after the run, including failure paths.

Verified in that run:

- Start, pause, active-session setup/pairing/whitelist guards, early end and history.
- Actual monotonic timer completion and notification delivery to the private bus.
- Injected login1 sleep and shutdown signals through the production desktop handler.
- Browser disconnect interruption, visible notice, retained incomplete cleanup and retry.
- Active SIGTERM cleanup and persisted theme after relaunch.
- SIGKILL followed by restart: abandoned-session crash reason, DND/browser cleanup,
  no automatic resume, and startup with a missing imported folder.
- Actual Rodio/ALSA playback into an isolated PipeWire server with a null sink and
  a policy-only WirePlumber instance. Healthy playback continued for more than the
  stall threshold; server loss produced the waiting state; a replacement output
  recovered through Retry and kept playing. Focus stayed active throughout.
- Removing the current MP3 while paused cleared selection and stopped the loaded
  decoder; Play with an empty queue did not resume the deleted file.

No physical device was unplugged, and the user's live PipeWire graph was untouched.
The separate earlier silent-MP3 check exercised the physical default output.
The matrix exposed and fixed an ALSA silent-stall case, history ordering when two
sessions finish within one second, and summary publication before durable history.

`nix develop -c cargo run -p deepify-desktop --example phase4_platform -- focus`
— **PASS**: actual Niri focus restored to the last allowed disposable window.
Every close/focus target was guarded; no existing user window was targeted.

## Evidence carried forward from Phase 4

- Firefox 156 and the installed Zen passed the production block/restore harness.
  The disposable harness explicitly enables remote system access to inspect the
  extension page on newer Firefox; ordinary user profiles are untouched.
- `DEEPIFY_LIFECYCLE_MODE=platform-finish node validation/phase3-firefox/production-harness.cjs`
  passed with Firefox: real broker/native host/extension, Niri fixture closure
  while paused, Noctalia enable, early end and complete restoration.
- The same command with `platform-failure` passed: induced DND loss interrupted
  the combined session and restored browser tabs and prior DND state.
- The Phase 4 Nix source snapshot `/tmp/deepify-phase4-source-q3lo08ug` built
  successfully. Its base desktop output is
  `/nix/store/0brcqjmikcjzxqa8256lxgmvld9r1kwl-deepify-0.1.0`.
  This is historical snapshot evidence, not a final build of the later Phase 5 edits.

## Open verification and delivery work

- Resolve the consent/counting decision and signed-install gate in
  [BROWSER_RELEASE_READINESS.md](BROWSER_RELEASE_READINESS.md).

Accessibility, concurrency, diagnostics, security/privacy, target system-dialog
allowances and visual review are recorded below. Named Phase 3 deliverables and
P1-24 criteria are reconciled in [PHASE3_ACCEPTANCE_AUDIT.md](PHASE3_ACCEPTANCE_AUDIT.md)
and [MVP_ACCEPTANCE.md](MVP_ACCEPTANCE.md). The current handoff is
[PHASE5_HANDOFF.md](../PHASE5_HANDOFF.md).

No physical suspend, logout, reboot, or shutdown was performed on the user's
desktop. No completed-MVP claim is made.

## Refinement and expanded acceptance (2026-09-25)

The full desktop matrix passed with `DEEPIFY_PORTAL_VALIDATION=1` and
`DEEPIFY_AUDIO_VALIDATION=1`. Opening the MP3 file chooser exposed missing GTK
GSettings schemas; the development shell now supplies the schema roots and the
Nix package uses `wrapGAppsHook3`. The actual chooser's app ID was
`com.deepify.desktop`, already covered by the implicit allowance. Cancellation
returned successfully and the full lifecycle/audio matrix then passed.

After `cargo test` with default features, rebuild the embedded frontend before
running the desktop harness:

```sh
nix develop -c npm run build
nix develop -c cargo build --workspace --bins --examples --features deepify-desktop/custom-protocol
nix develop -c dbus-run-session -- env DEEPIFY_FAILURE_VALIDATION=1 DEEPIFY_PERFORMANCE_VALIDATION=1 python3 validation/phase5-desktop-lifecycle.py
```

The failure matrix passed with the actual desktop: terminating only its own Niri
event-stream child, disabling DND during the disposable active session, and
withholding browser heartbeat replies each produced one visible interruption,
restoration, history outcome and successful repair. DND was restored after the
run. Power-state signals remain private-bus injections, not physical power tests.

A debug-binary baseline with an empty queue measured 809–1020 ms to loaded IPC;
working-state snapshot round trips had a 3.8 ms median and 5.6 ms maximum over ten
calls. The persistent desktop/WebKit/monitor tree used 2.8% of one CPU core over
five working seconds and 562.4 MiB summed RSS (shared pages may be counted more
than once). The initial idle sample included startup work and is not a steady
idle claim. Packaged and larger-queue measurements remain separate checks.

Firefox 156.0 and Zen 1.22.3b passed the current production harness, including
live countdown updates without keyboard-focus loss, paused status, container
restriction/restoration without cookies permission, redirect interception, and
closed tabs staying closed. The harness now waits for restoration acknowledgment
separately from navigation; Zen can expose additional startup tabs, so aggregate
restoration is not assumed to contain exactly one tab.

The URL audit fixed public DNS names being mistaken for local IPs, case-sensitive
path matching, trailing-slash prefix semantics and bare IPv4 rule normalization.
The shared matrix now has 38 cases consumed by Rust, production JavaScript and an
independent Node reference. Extension lifecycle operations are serialized so a
late replacement cannot overwrite restoration; failed restores retain their
in-memory originals for retry, and tab API/permission errors remove policy and
report URL-free integration errors.

`validation/check-browser-schema.py` passed using Python jsonschema's Draft
2020-12 validator: all 15 message types and 317 rejection checks. Rust separately
validates framing, byte limits, identifiers, payload keys/types and correlations.

Visual review used the actual desktop captures in `phase5-screens/`. Setup,
Obsidian/Mist idle, working, paused and failure/summary states are legible with
visible recovery information. Mist primary-button text and native-control color
schemes were corrected; command errors now flow above content without overlapping
the protection banner. Text-token contrast against each theme surface is at least
6.02:1 in Obsidian and 5.27:1 in Mist; primary buttons are 9.37:1 and 6.31:1.
Keyboard unit checks cover modal focus cycling, Escape, returned focus and active
session control availability. This is a focused accessibility review, not a
claim of a complete assistive-technology certification.

Privacy/security source review found no application HTTP client, remote assets,
account flow, analytics or updater. Tauri capabilities are limited to core,
dialog-open and notifications; CSP restricts scripts to local assets. The
extension has only the declared interception, tab, storage and native-messaging
permissions. Original destinations stay in memory, safe aggregate frames are
validated, socket/DB/backup ownership and permissions are checked, and subprocess
output and queues are bounded. Same-user desktop control remains an explicit MVP
trust boundary.

## Performance correction

The first installed-package 1000-track run exposed a repeated-scan loop: Linux
file-access notifications from reading ID3 tags were themselves scheduling the
next folder rescan. A real watcher regression failed before the fix and passed
after filtering `EventKind::Access`; file creation/removal tests still pass.
The complete Rust suite now has 70 passing tests (two display/audio tests remain
opt-in), with strict Clippy passing. The subsequent actual desktop audio/lifecycle
matrix also passed. The final installed-package measurement below includes the
correction and replaces the intermediate debug comparison.

Mozilla lint revealed a separate signing-readiness issue: the extension needs a
current data-transmission consent declaration. Local native messaging is included
in Mozilla's definition. The proposed counting/consent behavior and authoritative
sources are in [BROWSER_RELEASE_READINESS.md](BROWSER_RELEASE_READINESS.md). No
`none` declaration was added to hide local transfers. Signed installation and
that product-boundary decision remain open.

## Final installed candidate (2026-09-25)

`nix build path:/tmp/deepify-phase5-final-8pjkw_xy#default path:/tmp/deepify-phase5-final-8pjkw_xy#browser-native-host path:/tmp/deepify-phase5-final-8pjkw_xy#firefox-extension --no-link --print-out-paths`
— **PASS**, including 70 Rust tests, 18 Vitest tests, 18 Node tests, strict
Clippy, TypeScript, ESLint, formatting and production frontend compilation.
The watcher regression and null-current-track display fix are included.

`nix flake check path:/tmp/deepify-phase5-final-8pjkw_xy --all-systems` — **PASS**.
Both x86_64 and aarch64 outputs evaluated; execution/build validation was on
x86_64. The already-built local package was cached, so the command reported zero
additional check builds. No aarch64 execution is claimed.

Exact output paths, the implementation commit, source fingerprint and unsigned
XPI hash are in [phase5-release.json](phase5-release.json). All 114 runtime/build/
contract files match the implementation commit; later documentation and validation
harness edits are outside that snapshot. The five XPI assets match the current
extension sources byte for byte. Installed launcher/icon metadata and the absolute
native-host manifest target were verified. The final packaged native host and XPI
also passed the production block/restore harness in Firefox 156.0 and Zen 1.22.3b,
using temporary extension installation in disposable profiles outside the
development shell.

Outside `nix develop`, the final installed desktop passed:

```sh
dbus-run-session -- env DEEPIFY_DESKTOP_BIN=/path/to/package/bin/deepify \
  DEEPIFY_PORTAL_VALIDATION=1 DEEPIFY_AUDIO_VALIDATION=1 \
  DEEPIFY_FAILURE_VALIDATION=1 DEEPIFY_PERFORMANCE_VALIDATION=1 \
  python3 validation/phase5-desktop-lifecycle.py
dbus-run-session -- env DEEPIFY_DESKTOP_BIN=/path/to/package/bin/deepify \
  DEEPIFY_SETUP_VALIDATION_ONLY=1 python3 validation/phase5-desktop-lifecycle.py
dbus-run-session -- python3 validation/phase5-desktop-smoke.py /path/to/package/bin/deepify
```

Only the pinned FFmpeg binary directory was added to the ordinary PATH for audio
fixture generation; no development-shell GTK schema variables were supplied.
The matrix passed actual audio loss/retry, native chooser cancellation, all three
component failures, browser disconnect, completion notification, injected power
events, SIGTERM, hard-kill recovery, theme persistence and missing-folder startup.
The targeted setup check clicked Settings' rerun control and verified wizard
entry and persisted completion. Single-instance and exact app-ID checks passed.

| Final installed measurement | Result |
| --- | --- |
| Startup to loaded IPC, three launches | 710–923 ms |
| Warm idle, empty queue | 1.0% of one core; 527.4 MiB summed RSS |
| Working, empty queue | 9.6% of one core; 556.7 MiB summed RSS |
| 1000-file metadata scan and reconciliation | 110 ms |
| Working, 1000-track queue | 18.0% of one core; 565.9 MiB summed RSS |
| 1000-track snapshot round trip, ten calls | 20.0 ms median; 23.8 ms maximum |

CPU samples cover five seconds of the persistent desktop process tree. Shared
RSS pages may be counted more than once. The 1000 empty MP3 fixtures measure
metadata/queue work, not decoder throughput. The earlier installed candidate used
roughly 60% of one core in this workload before the watcher correction; these are
short target-machine observations, not a universal performance guarantee.

The signed-install harness also correctly rejected the unsigned XPI with
`ERROR_SIGNEDSTATE_REQUIRED` in Firefox. This confirms enforcement of the signing
gate and does not close it. Browser consent and signed installation remain the
open release work described above.
