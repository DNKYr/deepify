# Tauri build dependencies

The desktop build pins `tauri = 2.8.4` and
`tauri-plugin-single-instance = 2.3.6`. `cargo test --workspace` cannot resolve
these crates in the current environment because crates.io DNS is unavailable;
that failed check is recorded in the implementation plan. A networked Nix
shell must run the full Tauri build before distribution.
