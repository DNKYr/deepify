# Browser contract, version 1

The desktop is the session authority. The Firefox/Zen extension enforces its
website rules and owns original-tab restoration. The native host transports
messages between them and applies the Rust validator; it has no session policy.

Each frame is a four-byte little-endian unsigned byte length followed by UTF-8
JSON, with a maximum body of 256 KiB. Every message carries `version`, `type`, and
`message_id`. Replies correlate to a request's message ID using `request_id`.
The descriptive schema is `native-messaging.schema.json`; the executable strict
validator is `crates/browser-protocol/src/lib.rs`. It rejects unknown message
types, keys outside each message's payload, invalid enums/ranges, and malformed
rules. The transport also rejects truncated and oversized frames.

| Exchange | Purpose |
| --- | --- |
| `hello`, `pair`, `pair_result` | Identify the fixed extension and explicitly pair one random profile token |
| `heartbeat`, `heartbeat_ack` | Liveness round trip |
| `status`, `state` | Fresh health, active session and timer state; an unsolicited `state` updates a running timer |
| `start_session`, `start_result` | Send normalized whitelist rules and wait for existing-tab restriction |
| `stop_session`, `stop_result` | Stop the exact session and acknowledge restoration |
| `blocked_attempt` | Session ID only; the desktop aggregates counts |
| `integration_error`, `restore_error`, `restore_complete` | Safe error enums and aggregate restoration results |

The desktop socket is `$XDG_RUNTIME_DIR/deepify/browser-v1.sock`. It requires a
private, current-user runtime directory and checks peer credentials. The broker
bounds simultaneous peers, pending requests, frames, and unconsumed events;
handshakes and writes have timeouts. Only the paired connection receives rules.
The desktop stores the token's SHA-256 hash, never its raw value.

The extension intercepts top-level `webRequest` navigation, redirects to its own
blocked page, and rechecks `tabs.onUpdated` for history navigation. The first
original destination is retained only in extension memory; subsequent Back or
reload attempts do not overwrite it. Surviving tabs restore on acknowledged stop
or native disconnect. Closed tabs are not recreated. An extension/browser process
loss destroys that in-memory map; the app never persists destinations to disguise
that limitation. Browser restart must be idle, with no session resumption.

Explicit whitelist hosts/paths travel from the desktop to the extension.
Attempted/original destinations never travel back, appear in blocked-page query
parameters, or enter SQLite or logs. Private windows and additional profiles are
outside the approved MVP protection boundary.

`url-rule-cases.json` is exercised against the Rust matcher and the production
extension, covering public/local IPs, subdomains, paths, ports and schemes.
Production browser and desktop evidence lives in `validation/PHASE3_CHECKS.md`,
`validation/PHASE4_CHECKS.md`, and `validation/PHASE5_CHECKS.md`.
