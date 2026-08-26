# Phase 2 check record

Date: 2026-08-26. Environment: shared workspace, Rust 1.96, Node 24.19.

Passed locally:

- `cargo fmt --all` (before Tauri dependency restoration)
- `cargo test --workspace --offline` — 15 tests passed before the Tauri crate was restored in the default manifest
- `cargo clippy --workspace --all-targets --offline -- -D warnings` — passed in the same dependency-free profile
- `npm test` — 4 tests passed (Focus Room copy/theme, setup, and shared URL fixtures)

Not run / blocked:

- `cargo test --workspace` with Tauri: crates.io could not resolve `static.crates.io`.
- `npm run typecheck`, `npm run lint`, `npm run build`, and `tauri dev`: npm dependencies are locked but not installed because the registry was unavailable.
- `nix flake check` / `nix develop`: nixpkgs 26.05 fetch returned a truncated tar archive; the committed lock therefore requires repair before it can be evidence of reproducibility.
- Firefox, Zen, Niri, Noctalia, PipeWire, suspend, crash, and signed native-host validation: not exercised in Phase 2; restriction integrations remain simulated.
