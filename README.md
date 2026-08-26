# Deepify

Deepify is a local-only focus timer prototype. Phase 2 restriction adapters are
explicitly simulated; Firefox, Niri, Noctalia, and real process/window
enforcement are Phase 3/4 work.

## Development

```sh
nix develop
cargo test --workspace
cargo fmt --all --check
npm test
npm run typecheck
npm run lint
```

The Tauri-facing application boundary is in `apps/desktop`; the backend owns
session state and persistence. The current environment must have a usable Nix
registry to run `nix flake check`; absence is recorded as an unavailable
platform check, not silently substituted.
