# Privacy and recovery boundary

Deepify stores session aggregates, intentions when supplied, whitelist rules,
local audio paths/metadata, and queue order on the device. It does not store
browsing destinations, browsing history, account identifiers, or analytics.
Restriction attempts are counts only. There is no account or cloud service.

The Firefox/Zen integration stores a SHA-256 pairing-token hash, safe browser
kind/profile label, and cleanup checkpoints. The raw profile token remains in
the extension's `storage.local`; original tab URLs remain only in the running
extension's memory. Browser protocol frames, SQLite, diagnostics, logs, and
blocked-page query parameters never contain attempted/original destinations,
page titles, query strings, fragments, or restored URLs. Explicit whitelist
hosts and paths are local configuration and are sent to the paired extension.

Niri policy uses window IDs, application IDs and focus state. It does not inspect
terminal child processes or retain window titles. Noctalia's prior DND boolean
is checkpointed locally before changes, then cleared after verified restoration.
Subprocess output is bounded and is not included in errors or logs. SQLite stores
cleanup outcomes so an incomplete restore cannot be reported as successful.

Startup recovery records an abandoned active session as `extension_or_app_crash`
and requires cleanup resolution before another session can start. Planned exit
and login1 sleep/shutdown signals request interruption and cleanup; a clock
monitor detects suspension or wall-clock jumps independently of focus time.
No session resumes automatically. Hard browser crashes destroy the extension's
in-memory original-tab map. Deepify does not persist URLs to paper over that
limit. Exact tested lifecycle paths and outstanding physical-machine checks are
recorded in [validation/PHASE5_CHECKS.md](validation/PHASE5_CHECKS.md).
