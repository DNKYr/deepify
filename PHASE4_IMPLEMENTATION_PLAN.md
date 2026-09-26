# Deepify — Phase 4 application restriction

Status: implementation and target acceptance complete. Phase 3's browser path is implemented; its
signed-XPI release gate remains open. Existing display fixes are preserved.

The scope comes from DESIGN.md Phase 4 and P1-01, P1-09–12, P1-24 in
PHASE1_ISSUES.md. Phase 5 and external signing are still required for release.

| Item | Required outcome | Status |
| --- | --- | --- |
| P4-01 | Bounded Niri/Noctalia command transport and exact app-ID policy | Implemented and unit-tested |
| P4-02 | Full window preflight, one event stream, deduplicated targeted cooperative close, focus recovery, unresolved-window warning | Implemented; validation below |
| P4-03 | Preserve DND before enabling; durable checkpoint; verified restoration and retry on every stop/recovery path | Implemented; validation below |
| P4-04 | Production coordinator, component failure interruption, real setup/settings/preflight UI; remove production simulation controls | Implemented; validation below |
| P4-05 | Disposable-window and DND runtime validation, failure/recovery tests, Nix packaging, quality gates, handoff | Complete; see PHASE4_HANDOFF.md and Phase 5 evidence |

Validation must exercise actual behavior, including close refusal, duplicate
events, multiple windows per app, stream loss, a failed DND restore, and restart
recovery. Live enforcement tests must target disposable windows only; existing
user windows must not be closed by validation. Window titles are neither needed
for policy nor retained in diagnostics.

## Target evidence (2026-09-23)

Read-only inspection confirmed Niri 26.04 and Noctalia Shell revision `fe6fa12`.
The latter exposes `ipc call state all` with `state.doNotDisturb` and
`notifications enableDND/disableDND`. Its installed QML is authoritative for this
target; newer Noctalia v5 documentation describes a different CLI.

Observed app IDs include `Alacritty`, `md.obsidian.Obsidian`,
`com.anthropic.Claude`, `firefox`, and `zen-beta`. Ordinary applications still
require explicit whitelist entries. Browser app IDs are implicitly allowed so
that the approved profile-based website policy remains usable. The policy also
allows Deepify, exact shell/portal/authentication IDs, and unidentified windows;
it does not match arbitrary system-looking prefixes or inspect terminal children.

## Completion beyond Phase 4

Phase 5 must audit every P1-24 criterion, implement/verify system lifecycle and
exit/crash cleanup, validate audio-device recovery, review keyboard accessibility,
privacy/security and performance, and verify installable artifacts. Signed browser
installation needs an externally signed XPI and cannot be marked passed by an
unsigned development build.

Detailed current evidence and outstanding checks: [validation/PHASE4_CHECKS.md](validation/PHASE4_CHECKS.md).
