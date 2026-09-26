# Deepify

Deepify is a local-only Tauri focus application for NixOS, Niri, and Wayland.
Phase 5 refinement is in progress: the desktop uses the real Firefox/Zen browser adapter,
Niri window monitoring, and Noctalia Do Not Disturb. A paired healthy browser
and healthy desktop integrations are required before a session starts.

Niri sends one cooperative close request per blocked window; an application can
refuse or ask you to save work. Deepify then shows a manual-close warning and
keeps the session controls available. It never force-kills blocked applications.
DND's previous state is checkpointed before enabling it and restored on cleanup.

Current evidence and remaining acceptance work are in
[validation/PHASE5_CHECKS.md](validation/PHASE5_CHECKS.md) and the
[MVP acceptance audit](validation/MVP_ACCEPTANCE.md). The release handoff is
[PHASE5_HANDOFF.md](PHASE5_HANDOFF.md). Browser consent/counting decisions and
permanent signed installation remain open; see
[browser release readiness](validation/BROWSER_RELEASE_READINESS.md).

## Develop and verify

```sh
nix develop
npm ci
npm run dev

npm test
npm run typecheck
npm run lint
npm run build
npm run format:check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
nix flake check --all-systems
nix build .#default
nix build .#browser-native-host
nix build .#firefox-extension
```

No global npm package is required. `flake.lock`, `Cargo.lock`, and
`package-lock.json` pin the build inputs. Build the Nix package with
`nix build`; it installs the `deepify` desktop executable plus the
`com.deepify.browser` native-host helper and manifest. The separate
`firefox-extension` package produces a deterministic unsigned development XPI.
Use `nix build .#browser-native-host` when only the helper and Firefox native
messaging manifest are needed.

The Rust backend in `apps/desktop/src-tauri` is authoritative for session state,
audio, and storage. React is a presentation layer over typed Tauri commands.
Application data is stored locally under Tauri's `com.deepify.desktop` data
directory; the product has no account, analytics, or cloud service.

On Linux, Deepify supplies GDK's default 96 DPI when the process's screen
resolution is unspecified or invalid, before creating the webview. This prevents
negative viewport dimensions on affected WebKitGTK runtimes while preserving
valid DPI settings and monitor scaling. See
[the display validation record](validation/DISPLAY_CHECKS.md).

To install the native host, link its generated
`lib/mozilla/native-messaging-hosts/com.deepify.browser.json` into Firefox's
native-messaging-host directory (for Home Manager, commonly
`~/.mozilla/native-messaging-hosts/`). Do not modify an existing profile
automatically. The extension is temporary-development-install only until it is
signed; private windows and additional/unpaired profiles are disclosed bypasses.
Firefox 156.0 and Zen 1.22.3b were verified with the same native-host manifest
layout in disposable profiles.

See [PHASE3_HANDOFF.md](PHASE3_HANDOFF.md) for historical browser verification
and its external signing gate. `PHASE2_HANDOFF.md` and `validation/PHASE2_CHECKS.md` remain the historic
prototype evidence.

## Desktop integrations

Run Deepify inside the same user session as Niri and Noctalia Shell. The `niri`
and `noctalia-shell` commands must be on PATH and their IPC sockets accessible.
The verified Noctalia target is the legacy shell CLI exposing
`ipc call state all` and `ipc call notifications enableDND/disableDND`.
Settings and setup show installation, permission, timeout, and protocol errors.

Deepify, Firefox/Zen, and exact shell/authentication/portal IDs are implicitly
allowed. Other application IDs need explicit rules; unidentified windows and
terminal-hosted commands remain disclosed MVP bypasses. The window policy never
uses window titles or terminal contents. If cleanup is interrupted, reopen
Deepify and use **Rerun integration health checks** before starting again.
