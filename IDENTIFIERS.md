# P2-01 — Deepify bootstrap identifiers

Status: accepted 2026-08-26.

These identifiers are deliberately stable and product-owned. They are not
derived from a temporary spike or the user's filesystem.

| Boundary | Identifier | Reason |
| --- | --- | --- |
| Tauri reverse-DNS application ID | `com.deepify.desktop` | Stable vendor/product namespace used by Tauri and application data paths. |
| Executable | `deepify` | Lowercase product executable. |
| Firefox native-host ID | `com.deepify.browser` | Native messaging host name shared by Firefox and Zen manifests. |
| Firefox extension ID namespace | `focus@deepify.local` | Fixed development/release namespace; a signed AMO ID can replace this at packaging time without changing protocol messages. |

The Phase 2 browser integration is simulated. These names do not claim that a
real extension or native host has been installed. The native host contract
rejects unpaired profiles and remains a Phase 3 integration boundary.

## Toolchain pin

The project targets nixpkgs 26.05. The flake pins the release branch input to
the immutable revision recorded in `flake.lock`; when the Nix registry is not
available, `nix flake check` is reported as unavailable rather than treated as
proof of a build.

Rust and Node versions are supplied by the development shell, while Cargo and
npm lockfiles pin project dependencies. No global npm package is required.
