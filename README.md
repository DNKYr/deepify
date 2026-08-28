# Deepify

Deepify is a local-only Tauri focus-room prototype for NixOS. Phase 2 implements
the complete desktop session lifecycle, SQLite persistence, whitelist and setup
flows, local MP3 playback, recovery behavior, notifications, and deterministic
tests. Firefox, Niri, and Noctalia restriction adapters remain unmistakably
simulated; real enforcement is Phase 3/4 work.

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
nix flake check
```

No global npm package is required. `flake.lock`, `Cargo.lock`, and
`package-lock.json` pin the build inputs. Build the Nix package with
`nix build`; it installs the `deepify` desktop executable plus the
`com.deepify.browser` native-host helper and manifest.
Use `nix build .#browser-native-host` when only the helper and Firefox native
messaging manifest are needed.

The Rust backend in `apps/desktop/src-tauri` is authoritative for session state,
audio, and storage. React is a presentation layer over typed Tauri commands.
Application data is stored locally under Tauri's `com.deepify.desktop` data
directory; the product has no account, analytics, or cloud service.

See `PHASE2_HANDOFF.md` and `validation/PHASE2_CHECKS.md` for the verified scope,
known platform limits, and Phase 3/4 backlog.
