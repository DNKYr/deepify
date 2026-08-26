# Phase 2 privacy and recovery boundary

Deepify stores session aggregates, intentions when supplied, whitelist rules,
local audio paths/metadata, and queue order on the device. It does not store
browsing destinations, browsing history, account identifiers, analytics, or
authentication data. Restriction attempts are counts only.

Startup recovery marks an abandoned active session interrupted and asks each
mock adapter for idempotent cleanup before another session may start. The
prototype does not claim hard-crash, suspend, logout, reboot, shutdown, or
real PipeWire recovery: those require Phase 3/4 platform validation.
