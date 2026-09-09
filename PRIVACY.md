# Phase 3 privacy and recovery boundary

Deepify stores session aggregates, intentions when supplied, whitelist rules,
local audio paths/metadata, and queue order on the device. It does not store
browsing destinations, browsing history, account identifiers, analytics, or
authentication data. Restriction attempts are counts only.

The Firefox/Zen integration stores a SHA-256 pairing-token hash, safe browser
kind/profile label, and cleanup checkpoints. The raw profile token remains in
the extension's `storage.local`; original tab URLs remain only in the running
extension's memory. Browser protocol frames, SQLite, diagnostics, logs, and
blocked-page query parameters never contain destinations, titles, hosts, query
strings, fragments, or restored URLs.

Startup recovery marks an abandoned active session interrupted and requires
browser cleanup resolution before another session can start. Hard browser
crashes, suspend, logout, reboot, and shutdown lifecycle behavior remain
target-machine validation gates; Deepify does not persist URLs to paper over
those limits.
