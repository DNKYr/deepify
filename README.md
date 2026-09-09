# Deepify

Deepify is a local-only Tauri focus-room prototype for NixOS. Phase 3 adds real
website restriction for one explicitly paired, healthy Firefox/Zen profile;
the browser extension is required before a focus session can start. Application
restriction (Niri) and Do Not Disturb (Noctalia) remain explicitly simulated
until Phase 4.

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

To install the native host, link its generated
`lib/mozilla/native-messaging-hosts/com.deepify.browser.json` into Firefox's
native-messaging-host directory (for Home Manager, commonly
`~/.mozilla/native-messaging-hosts/`). Do not modify an existing profile
automatically. The extension is temporary-development-install only until it is
signed; private windows and additional/unpaired profiles are disclosed bypasses.
Zen Beta 1.21.15b was verified with the same native-host manifest path in a
disposable profile.

See [PHASE3_HANDOFF.md](PHASE3_HANDOFF.md) for current verification and release
gates. `PHASE2_HANDOFF.md` and `validation/PHASE2_CHECKS.md` remain the historic
prototype evidence; Niri and Noctalia enforcement follows in Phase 4.
