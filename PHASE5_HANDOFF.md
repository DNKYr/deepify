# Phase 5 handoff — release gates still open

Deepify's production desktop integration, lifecycle recovery, audio behavior,
browser restriction and visible failure handling are implemented. The full
acceptance map is [validation/MVP_ACCEPTANCE.md](validation/MVP_ACCEPTANCE.md),
with exact commands and measured scope in
[validation/PHASE5_CHECKS.md](validation/PHASE5_CHECKS.md).

The current implementation has 70 passing Rust tests and two opt-in device/display
tests; 18 frontend and 18 Node tests pass. Strict Clippy, TypeScript, ESLint,
formatting and the production frontend build pass. Actual Firefox/Zen tests cover
restriction, redirects, containers, pause, restoration, desktop loss, orderly
browser restart, checkpoint recovery and combined Niri/Noctalia sessions. The
actual desktop matrix verifies completion notification, early end, SIGTERM,
hard-kill recovery, injected logind events, component failure/repair, and real
Rodio/PipeWire output loss and retry.

The current Nix release candidate includes desktop launcher/icon metadata, the
native-host manifest and GTK schema wrappers. The first installed package passed
single-instance, file chooser and lifecycle checks outside the development shell.
A 1000-track performance run then exposed a watcher feedback loop; the regression
was reproduced and fixed by ignoring metadata-read access events. The final
package rebuild and measurement must include that correction.

## Remaining release decisions

- Resolve native-message consent and optional browser counting as described in
  [BROWSER_RELEASE_READINESS.md](validation/BROWSER_RELEASE_READINESS.md).
  Mozilla's linter reports the missing data-consent declaration; it is not hidden
  with an inaccurate “no data” declaration.
- Supply a signed XPI path or an authorized signing channel, then exercise
  permanent signed installation with `DEEPIFY_SIGNED_INSTALL=1` in disposable
  Firefox and Zen profiles. No external submission or publication has occurred.

Until these gates are resolved, this is a verified development release candidate,
not a completed signed release. Existing display fixes and unrelated user data
are preserved. No physical suspend/reboot/logout or hardware unplug was performed;
power and device-loss scenarios use isolated injection/output infrastructure.

## Installation and recovery

Run inside the Niri user session with the verified Noctalia CLI available. Build
`.#default` for the desktop and native host, and `.#firefox-extension` for the
unsigned development XPI. The package installs `com.deepify.desktop.desktop`, its
icon and an absolute-path native-host manifest. Use the manifest through NixOS's
Firefox native-messaging-host option or a user-managed manifest link as documented
in README. Development extension installation is temporary until signing is done.

Deepify saves local SQLite data under the `com.deepify.desktop` application data
directory. Pairing credentials are hashed before desktop persistence; original
browsing destinations remain only in extension memory. Reopen after interruption
and run integration checks to retry any unresolved cleanup before starting a new
session. A hard extension/browser process loss can destroy the original-tab map;
no destination persistence was added to conceal that boundary.
