# Phase 3 validation record

## Local implementation gates

| Check | Status | Evidence |
| --- | --- | --- |
| Frontend and extension unit tests | PASS | `npm test` — 2026-09-02: 10 Vitest tests and 13 Node tests, including the shared URL contract, 5 mocked production-background tests, and the first-original restoration/back-navigation cases |
| TypeScript | PASS | `npm run typecheck` — 2026-09-02 |
| Frontend lint | PASS | `npm run lint` — 2026-09-02 |
| Frontend formatting | PASS | `npm run format:check` — 2026-09-02 |
| Frontend production build | PASS | `npm run build` — 2026-09-02 |
| Full Rust workspace tests | PASS | `nix develop -c cargo test --workspace --all-targets` — 40 passed, 1 environment-dependent audio test ignored — 2026-09-02 |
| Strict Rust lint | PASS | `nix develop -c cargo clippy --workspace --all-targets -- -D warnings` — 2026-09-02 |
| Desktop compile | PASS | `nix develop -c cargo check -p deepify-desktop --bin deepify` — 2026-09-01 |
| Extension syntax/manifest boundary | PASS | `node --check` on both extension scripts; manifest asserts fixed ID, no `cookies`, and `incognito: not_allowed` — 2026-09-01 |
| Nix expression parse | PASS | `nix-instantiate --parse flake.nix` — 2026-09-01 |
| Deterministic extension artifact | PASS | `nix build .#firefox-extension --no-link` — 2026-09-02; inspected archive contains only the five production extension assets, with no fixtures, credentials, or Phase 2 simulation code |
| Disposable Firefox production harness | PASS | Firefox 154.0; `cargo build -p deepify-browser-native-host --offline` then `node validation/phase3-firefox/production-harness.cjs` — 2026-09-02: paired hello, real Rust native host, existing/new public HTTP(S) block, loopback allowance, Back/reload, paused enforcement, blocked-page keyboard focus, URL-free blocked events, normal restoration, and native-disconnect fail-open restoration passed |
| Disposable Zen production harness | PASS | Zen Beta 1.21.15b at `/etc/profiles/per-user/dnkyr/bin/zen-beta`; same command with `FIREFOX_BIN` set to that path — 2026-09-02: same scenarios passed, including blocked-page keyboard focus and discovery of the temporary `~/.mozilla/native-messaging-hosts/com.deepify.browser.json` manifest |
| Private-window bypass disclosure | PASS | Firefox and Zen disposable profiles run with `DEEPIFY_PRIVATE_WINDOW=1` — 2026-09-02: public navigation remains unblocked and produces no `blocked_attempt`, matching `incognito: not_allowed` and the visible bypass disclosure |
| Real broker lifecycle: desktop loss | PASS | Firefox and Zen disposable profiles run with `DEEPIFY_LIFECYCLE_MODE=desktop-loss` — 2026-09-02: the validation-only Rust `phase3_lifecycle` broker exits after a real paired start; native-port closure restores the blocked tab |
| Real broker lifecycle: browser restart | PASS | Firefox and Zen disposable profiles run with `DEEPIFY_LIFECYCLE_MODE=browser-restart` — 2026-09-02: browser close is detected, then profile reconnects as `healthy_idle` with no active policy resurrection |
| Real broker lifecycle: startup checkpoint | PASS | Firefox and Zen disposable profiles run with `DEEPIFY_LIFECYCLE_MODE=startup-recovery` — 2026-09-02: a restarted Rust broker loads the persisted safe token hash, receives an idle reconnect after local fail-open restoration, and clears the checkpoint |
| Nix default and native-host packages | PASS | `nix build .#default --no-link` and `nix build .#browser-native-host --no-link` — 2026-09-02; generated manifest has an absolute Nix-store helper path and only `focus@deepify.local` in `allowed_extensions` |
| Nix flake checks | PASS | `nix flake check --all-systems` — 2026-09-02; all outputs evaluated and all checks passed |

## Target-browser evidence

Firefox 154.0 is available on this machine at
`/etc/profiles/per-user/dnkyr/bin/firefox`. The harness creates its profile,
HOME, XDG runtime directory, native-host manifest, and Unix socket under `/tmp`;
it does not read or modify a normal browser profile. Its fake desktop peer
asserts that `blocked_attempt` contains only a session ID and that the original
destination does not cross native messaging. Lifecycle modes replace that peer
with the compiled validation-only Rust `phase3_lifecycle` broker and use the
production Rust native host; they are not Node native-host evidence.

Signed-XPI installation remains an external release gate. An unavailable signed
XPI is documented rather than reported as a passing result.
