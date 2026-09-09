# Deepify — Phase 3 Website Restriction Implementation Plan

## Status

- **Stage:** Planned — ready for sequential implementation
- **Depends on:** Accepted Phase 2 desktop prototype and approved Phase 1 browser behavior
- **Architecture:** [`ARCHITECTURE.md`](ARCHITECTURE.md)
- **Phase 2 handoff:** [`PHASE2_HANDOFF.md`](PHASE2_HANDOFF.md)
- **Goal:** Replace the simulated browser adapter with real website restriction for one paired Firefox or Zen Browser profile

Phase 3 is the browser-enforcement phase. The desktop remains the session authority,
the extension owns tab interception/restoration, and the native host remains a
transport-only helper. Real Niri application restriction and Noctalia Do Not Disturb
remain Phase 4 work.

A Phase 3 build must describe protection precisely: website restriction may be real
when a paired profile is healthy, while application restriction and Do Not Disturb
remain simulated. It must never imply that Phase 4 protections are active.

---

## 1. Phase 3 Outcomes

At the end of Phase 3, Deepify should provide:

- A production Firefox WebExtension with the permanent ID `focus@deepify.local`
- One explicitly paired Firefox or Zen Browser profile
- A production `com.deepify.browser` native-messaging host
- A user-only Unix-socket connection from native hosts to the running desktop
- Versioned, bounded, validated browser messages with request correlation
- A persistent browser connection and explicit health checks at least once per minute
- Backend preflight that prevents session start unless the paired extension is healthy
- Synchronization of active session state and normalized website rules
- Default-deny restriction of public HTTP(S) destinations
- Automatic allowance of non-HTTP(S), loopback, private-network, and link-local destinations
- Immediate restriction of existing non-whitelisted tabs when a session starts
- Restriction of new non-whitelisted top-level navigation while Working or Paused
- The approved calm blocked-page experience without an in-session restore action
- Aggregate blocked-attempt events without sending or storing browsing destinations
- Restoration of every surviving blocked tab on normal completion, early end, failure,
  and native disconnect where the extension process remains able to run cleanup
- Clear interruption of the desktop session when the browser integration fails
- Production integration tests in Firefox and Zen Browser
- Reproducible native-host and unsigned development-extension artifacts

---

## 2. Scope Boundaries

### Included

- Firefox and Zen Browser only
- One configured profile
- Ordinary and container tabs in that profile under one shared policy
- Standard restriction mode only
- HTTP(S) top-level website navigation
- Browser setup, pairing, health, restriction, restoration, and diagnostics
- Nix packaging and installation instructions for the native host
- An unsigned development XPI/source artifact and signed-installation readiness

### Excluded

- Chromium-family browsers
- Multiple monitored profiles
- Per-container rules
- Private-window enforcement
- Restriction of non-HTTP(S) schemes
- Niri application enforcement
- Real Noctalia state changes
- Gentle or Strict modes
- Accounts, cloud synchronization, analytics, or remote services
- Automatic modification of an existing Firefox or Zen profile
- AMO publication credentials or release-channel approval

Private windows and additional profiles remain disclosed bypasses. The extension must
use `incognito: "not_allowed"` and must not request the `cookies` permission.

Signed AMO publication is an external release gate. Phase 3 must produce a linted,
reproducible extension artifact and validate temporary/development installation. A
signed-install test is recorded when a signed artifact and signing channel are available,
but lack of AMO credentials does not justify embedding an unsafe unsigned-install path.

---

## 3. Resolved Technical Direction

### Topology

```text
Firefox/Zen extension
  <-> Firefox native-messaging frames on stdin/stdout
  <-> deepify-browser-native-host
  <-> framed JSON over $XDG_RUNTIME_DIR/deepify/browser-v1.sock
  <-> desktop BrowserConnectionService
  <-> production BrowserRestrictionAdapter
  <-> existing RestrictionCoordinator and SessionService
```

### Authority and ownership

- The desktop decides whether a session may start and whether it remains valid.
- The extension decides which top-level browser navigations are allowed and owns the
  in-memory original-tab map.
- The native host forwards validated frames only. It owns no pairing, whitelist,
  session, or restoration policy.
- The desktop stores the paired profile identity and a token hash, never the raw token.
- The desktop stores aggregate blocked counts only, never attempted URLs.

### Socket security

- Use `$XDG_RUNTIME_DIR/deepify/browser-v1.sock`; do not fall back to a shared `/tmp` path.
- Create the Deepify runtime directory with mode `0700` and the socket with mode `0600`.
- Accept only same-UID peers and reject connections when ownership or permissions are wrong.
- Remove a stale socket only after proving it is owned by the current user and no live
  listener is present.
- Bound every frame to 256 KiB and apply connection/request timeouts.

### Pairing

- On first run, the extension generates 32 cryptographically random bytes and stores the
  token in that profile's `storage.local`.
- `hello` includes the fixed extension ID, protocol version, token, browser kind,
  capabilities, and a safe profile label/fingerprint. It does not include tab URLs.
- The desktop exposes an unpaired connection as **Pairing required**.
- The user explicitly accepts the pending profile from setup or Settings.
- The desktop persists SHA-256 of the token plus the browser/profile label; the raw token
  remains in memory only for the current connection.
- Only one token may be paired. Replacing or forgetting the pair is unavailable during an
  active session and requires confirmation while idle.
- Unknown or mismatched tokens cannot receive session rules or activate restrictions.

### Health

- Keep the native connection persistent while the browser profile is running.
- Send a heartbeat every 20 seconds and require a valid acknowledgement within 60 seconds.
- Preflight performs a fresh status round trip; a merely open socket is not sufficient.
- Disconnect, permission/capability loss, protocol mismatch, or heartbeat timeout marks the
  browser unhealthy and interrupts an active session exactly once.
- Reconnection uses bounded exponential backoff and never resumes an interrupted session.

### URL privacy

- Original URLs stay inside the extension's active-tab restoration state.
- Do not send attempted/original URLs through native messaging or the Unix socket.
- Do not write them to SQLite, `storage.local`, logs, diagnostics, or blocked-page query
  parameters.
- The blocked page may display a sanitized host and path supplied in memory by the
  background script; omit credentials, query strings, and fragments.
- P3-01 must validate graceful close/restart restoration without introducing destination
  persistence. If the browser APIs cannot satisfy both recovery and this privacy rule, stop
  for a product decision instead of silently persisting browsing history.

### Mixed Phase 3 status

- The packaged desktop uses the real browser adapter; there is no silent production fallback
  to a mock browser adapter.
- Mock browser adapters remain available to Rust/unit tests only.
- Niri and Noctalia remain explicit mock adapters until Phase 4.
- UI copy distinguishes **Website protection active** from **Application protection
  simulated** and **Do Not Disturb simulated**.

---

## 4. Delivery Strategy

Build and prove the security boundaries before connecting them to session lifecycle:

```text
contracts and browser-behavior gate
  -> framed Rust protocol and native-host proxy
  -> desktop socket broker and pairing persistence
  -> extension connection and pairing
  -> URL enforcement and blocked page
  -> production BrowserRestrictionAdapter
  -> health/events/failure handling
  -> restoration and recovery
  -> setup/settings UX
  -> Nix/XPI packaging
  -> Firefox/Zen validation
  -> acceptance handoff
```

Every milestone must leave existing Phase 2 behavior and automated gates passing.

---

## 5. Phase 3 Work Items

## P3-01 — Freeze the production browser contract and interception strategy

**Priority:** Critical  
**Status:** Planned

### Tasks

- Replace the minimal native-message schema with complete version-1 request, response,
  event, length, and `additionalProperties` rules.
- Define exact payloads for:
  - `hello`, `pair`, and `pair_result`
  - `heartbeat` and `heartbeat_ack`
  - `start_session` and `start_result`
  - `stop_session` and `stop_result`
  - `status` and `state`
  - `blocked_attempt`
  - `integration_error`
  - `restore_complete` and `restore_error`
- Require protocol version and message ID on every message; require a request ID on every
  response.
- Define safe enums for browser kind, health, timer state, error code, and capability names.
- Expand URL fixtures to cover domains, subdomains, path prefixes, ports, public IPs,
  private/link-local IPv4 and IPv6, malformed destinations, and non-HTTP(S) schemes.
- Run the same URL fixtures against Rust and production extension code.
- Use a disposable browser probe to choose and document the top-level interception mechanism
  and least-privilege permission set. Validate existing tabs, new navigation, Back, reload,
  redirects, closed tabs, containers, and graceful browser close/restart.
- Confirm that original-tab restoration can meet the privacy boundary without `storage.local`
  URL persistence or URL-bearing blocked-page parameters.
- Record the selected behavior in `ARCHITECTURE.md` and the contract README.

### Acceptance

- Contract tests reject unknown types, extra fields, missing IDs, invalid versions, and
  oversized representative payloads.
- Rust and JavaScript produce identical results for every shared URL-rule fixture.
- The selected interception mechanism works in disposable Firefox and Zen profiles.
- Browser close/restart behavior and the original-URL retention boundary are explicit.
- No implementation milestone depends on an unresolved browser-policy choice.

---

## P3-02 — Implement the bounded Rust protocol and production native host

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-01

### Tasks

- Introduce a small shared Rust browser-protocol crate if needed by both the desktop and
  native host; this independent reuse justifies the additional crate.
- Implement strict little-endian native-message frame reading and writing.
- Handle partial reads/writes, EOF, invalid UTF-8/JSON, protocol mismatches, and oversized
  frames without panics or unbounded allocation.
- Replace the Phase 2 heartbeat stub with a full-duplex native-stdio/Unix-socket proxy.
- Use the fixed XDG runtime socket path and refuse insecure/missing runtime directories.
- Ensure either transport side closing shuts down the other side and exits the helper.
- Keep stdout protocol-only; safe diagnostics may use stderr but must omit tokens, rules,
  URLs, and raw payloads.
- Add in-memory and real Unix-socket transport tests, including fragmented frames and
  simultaneous bidirectional traffic.

### Acceptance

- The helper forwards messages in both directions without interpreting session policy.
- A desktop socket close causes helper exit and observable extension disconnect.
- Malformed, oversized, and wrong-version frames are rejected deterministically.
- Tests prove that sensitive message bodies are not logged.
- `cargo test`, strict Clippy, and formatting pass in the Nix shell.

---

## P3-03 — Add the desktop browser broker and pairing persistence

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-02

### Tasks

- Add a versioned SQLite migration for one paired browser/profile record and browser cleanup
  checkpoints. Do not overload arbitrary settings strings with raw pairing data.
- Implement the user-only Unix listener, same-UID peer verification, connection registry,
  request correlation, response timeout handling, and clean shutdown.
- Model disconnected, pending-pair, connected-unhealthy, healthy-idle, active, and
  cleanup-required states.
- Hash tokens before persistence and use constant-time hash comparison.
- Permit unpaired `hello` and pairing traffic only; reject all session commands for unknown
  tokens.
- Add Tauri commands to accept a pending profile, forget/replace a profile while idle, retry
  connection checks, and retrieve a safe browser integration snapshot.
- Persist only safe browser kind/label, token hash, pair time, and cleanup checkpoint.
- Make listener startup and stale-socket cleanup idempotent.

### Tests

- Fresh and upgrade migration
- Pair, reconnect, reject wrong token, replace, and forget
- Second-profile rejection
- Pairing commands rejected during Working and Paused
- Same-UID acceptance and wrong-permission/peer rejection
- Request timeout and late/duplicate response handling
- Restart with a persisted pair but no connected browser

### Acceptance

- A profile cannot become healthy without explicit desktop acceptance.
- Raw pairing tokens never appear in SQLite, snapshots, errors, or logs.
- Only one paired profile can receive session data.
- Broker restart does not falsely report a stale connection as healthy.

---

## P3-04 — Implement extension connection, pairing, and health

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-03

### Tasks

- Replace the Phase 2 extension placeholder with a production background implementation.
- Add `incognito: "not_allowed"` and audit every requested permission.
- Generate and retain the profile token in `storage.local`; retain no browsing destinations
  there.
- Connect persistently to `com.deepify.browser` and send validated `hello` metadata.
- Implement pending/accepted/rejected pairing states.
- Implement heartbeat acknowledgement, status response, capability reporting, and bounded
  reconnect backoff.
- Treat native disconnect as immediate integration failure; if restriction is active, begin
  local fail-open tab restoration before reconnecting.
- Expose safe extension status for setup diagnostics without exposing the token.
- Add JavaScript unit tests with mocked WebExtension APIs and native ports.

### Acceptance

- Firefox and Zen can connect through the Rust helper to the desktop broker.
- A second/unpaired profile remains visibly unpaired and receives no rules.
- Disconnect and reconnect do not reactivate an ended session.
- The extension requests no cookies, history, downloads, broad filesystem, or unrelated
  permissions.

---

## P3-05 — Implement URL enforcement and the blocked-page experience

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-04

### Tasks

- Implement the production URL matcher from the shared fixtures.
- Restrict only top-level HTTP(S) destinations.
- Allow all non-HTTP(S) schemes and approved local addresses by default.
- Apply domain/subdomain, path-prefix, scheme-equivalence, and port-ignoring rules exactly as
  approved in P1-06.
- On `start_session`, install policy before scanning every existing tab and acknowledge start
  only after the scan/restriction pass completes.
- Intercept new top-level navigation while Working and Paused.
- Keep an in-memory map from tab ID to original URL; remove entries for closed tabs.
- Prevent browser Back, reload, or repeated navigation from exposing a blocked destination
  while restriction remains active.
- Replace the Phase 2 blocked-page preview copy with production state.
- Display only sanitized destination host/path and current session status/remaining time.
- Keep **Open a new tab** and remove every manual restore/bypass action.
- Emit one aggregate `blocked_attempt` event per actual restriction action, with no URL.

### Tests

- Shared URL fixtures in Rust and extension JavaScript
- Existing allowed/blocked tabs
- New navigation, redirects, reload, and Back
- Closed blocked tab is not recreated
- Container tab treated as an ordinary tab without cookies permission
- Blocked-page accessibility and keyboard behavior
- No attempted URL in native messages, query parameters, logs, or desktop state

### Acceptance

- Public non-whitelisted HTTP(S) pages show the blocked experience.
- Allowed, local, and non-HTTP(S) destinations remain accessible.
- Existing non-whitelisted tabs become inaccessible before session start is acknowledged.
- Paused sessions continue enforcing the same policy.

---

## P3-06 — Wire the production browser adapter into session start and stop

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-05

### Tasks

- Implement a production `BrowserRestrictionAdapter` backed by the browser broker.
- Adjust the browser adapter boundary to accept the current session ID and normalized website
  rules without moving policy into React or the native host.
- Load website whitelist entries from SQLite immediately before start preflight.
- Require a fresh paired-profile health/status response during preflight.
- Make activation wait for `start_result`; do not expose Working until existing-tab
  restriction succeeds.
- Make deactivation send `stop_session`, wait for restoration completion, and remain
  idempotent for reverse cleanup and recovery.
- Keep Phase 4 application and DND adapters as explicit mocks.
- Set browser cleanup-required before activation and clear it only after confirmed stop or a
  confirmed inactive status.
- Preserve existing completion, early-end, pause/resume, and summary behavior.

### Tests

- Healthy start with normalized rules
- Unpaired, disconnected, stale-heartbeat, and permission-failure preflight
- Start timeout and partial-start reverse cleanup
- Pause/resume does not deactivate the browser adapter
- Completion and early end request restoration exactly once
- Repeated cleanup is safe

### Acceptance

- A session cannot start unless the paired extension is connected and healthy.
- The desktop reaches Working only after the extension has restricted existing tabs.
- Every ordinary session-end path requests and confirms browser restoration.
- Production builds cannot silently substitute simulated browser health.

---

## P3-07 — Connect blocked attempts, timer state, and runtime failures

**Priority:** High  
**Status:** Planned  
**Depends on:** P3-06

### Tasks

- Route `blocked_attempt` events to the active backend session and persistent aggregate count.
- Deduplicate event/message IDs so reconnect or retry cannot double-count an attempt.
- Ignore events for stale/unknown session IDs and record only safe diagnostics.
- Synchronize Working/Paused state and remaining time to the extension on transitions and
  periodic health traffic.
- Make blocked pages query the background script for current state rather than trusting URL
  parameters.
- Run 20-second heartbeat checks and enforce the 60-second health deadline.
- Convert disconnect, heartbeat timeout, permission loss, and extension integration errors
  into one `extension_or_app_crash` interruption.
- Prevent races between user end, timer completion, and browser failure from creating two
  summaries or cleanup transactions.
- Emit updated snapshots and clear user-facing errors naming the browser component.

### Acceptance

- Blocked counts appear in the active Focus Room, summary, and local history.
- No destination or per-tab browsing record is persisted.
- Remaining time stops changing on pause while restriction remains active.
- A browser runtime failure interrupts the session once and never leaves the UI claiming
  protection is active.

---

## P3-08 — Complete tab restoration and cross-process recovery

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-07

### Tasks

- Restore all surviving blocked tabs for completion, early end, integration failure, failed
  startup, and planned desktop exit.
- Restore locally when the extension's native port disconnects during an active restriction.
- Never recreate a tab that the user closed while blocked.
- Report restoration counts and safe errors; do not include restored URLs.
- Make stop, disconnect restoration, and startup recovery idempotent.
- On desktop startup, inspect abandoned session/browser cleanup checkpoints. Before allowing a
  new session, wait for the paired extension when necessary, request status, and force inactive
  restoration if the extension still reports active.
- Validate desktop process termination while Firefox/Zen remains open: socket closure must
  disconnect the native port and trigger extension restoration.
- Validate graceful browser close/restart while the desktop remains open: the session must
  interrupt and must not resume after reconnection.
- Record hard browser-process crash behavior separately if browser shutdown prevents extension
  cleanup; do not add persistent URL storage without an approved privacy decision.

### Acceptance

- Normal and failure paths restore surviving tabs and clear active browser policy.
- Desktop restart cannot begin a new session while browser cleanup is unresolved.
- Reconnection never resurrects an interrupted session.
- Cleanup failures are visible and retryable rather than reported as complete.

---

## P3-09 — Replace simulated browser setup and health UI

**Priority:** High  
**Status:** Planned  
**Depends on:** P3-08

### Tasks

- Replace the setup wizard's simulated browser step with installation, connection, pending-pair,
  paired, unhealthy, and ready states.
- Require a paired healthy profile before setup completion and session start.
- Show Firefox or Zen, a safe profile label/fingerprint, connection status, last successful
  health check, and recovery action.
- Add pair, retry, replace, and forget controls with confirmation and active-session guards.
- Update Settings health controls so the browser is real while Niri and Noctalia remain
  unmistakably simulated.
- Update Focus Room wording to distinguish real website protection from simulated application
  and DND protection.
- Preserve disclosures for private windows, other profiles, and containers.
- Update the blocked page for production wording and remove **Preview only**.
- Add keyboard, focus-management, and screen-state tests for all pairing/health states.

### Acceptance

- UI never describes an unpaired or unhealthy extension as protected.
- The user can complete pairing and diagnose a disconnected browser without reading logs.
- Pairing identity cannot be changed while Working or Paused.
- Browser bypass disclosures remain visible in setup and Settings.

---

## P3-10 — Package the extension and native host for NixOS

**Priority:** High  
**Status:** Planned  
**Depends on:** P3-09

### Tasks

- Add a reproducible `packages.firefox-extension` development artifact/source bundle.
- Keep the fixed extension ID in development and release manifests.
- Package the production helper and generated absolute-path native manifest as
  `packages.browser-native-host`.
- Verify the manifest exposes only `focus@deepify.local` in `allowed_extensions`.
- Document NixOS/Home Manager native-host configuration for Firefox without modifying user
  profiles automatically.
- Verify and document the native-host lookup path for the target Zen Browser package.
- Add extension lint, manifest validation, deterministic archive, and artifact-content checks.
- Audit final extension permissions and remove unused permissions.
- Document temporary development installation separately from signed release installation.
- Prepare the source/XPI inputs needed for AMO signing without committing credentials.

### Acceptance

- Nix builds the desktop, native host, manifest, and development extension artifact offline
  from locked inputs.
- Firefox discovers the native host through the documented Nix configuration.
- Zen discovers the host through a verified, documented target-package path.
- The extension archive contains no test fixtures, secrets, absolute local paths, or Phase 2
  simulation code.

---

## P3-11 — Run production Firefox and Zen integration validation

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-10

### Automated scenarios

Use disposable browser profiles and homes. Do not modify the developer's normal profiles.

- Native framing and broker connection
- First-profile pairing and paired reconnect
- Wrong token, second profile, wrong extension ID, wrong protocol, malformed message, and
  oversized frame rejection
- Healthy preflight and session start
- Existing allowed and blocked tabs
- New allowed and blocked navigation
- Domain/subdomain/path/port/local-network/non-HTTP fixture matrix
- Browser Back and reload remain restricted
- Pause keeps restriction active
- Completion and early-end restoration
- Closed blocked tabs are not recreated
- Blocked-attempt count reaches the desktop without URLs
- Native-host disconnect restores tabs and interrupts the session
- Heartbeat timeout interrupts the session
- Desktop process termination triggers extension fail-open restoration
- Graceful browser close/restart interrupts and does not resume the session
- Startup cleanup checkpoint and idempotent recovery
- Container tabs work under the profile-wide policy without cookies permission

### Manual/target-platform scenarios

- Firefox private-window bypass remains disclosed and unmonitored
- Zen private-window bypass is manually confirmed
- Temporary extension install in Firefox and Zen
- Nix-provided native host discovery in Firefox
- Target Zen package native-host discovery
- Blocked-page visual and keyboard review
- Signed XPI installation when a signed artifact is available

### Evidence

Record exact browser versions, package paths, commands, result logs with sensitive fields
redacted, and PASS/FAIL/SKIPPED status in `validation/PHASE3_CHECKS.md`.

### Acceptance

- Core start/block/pause/restore/disconnect scenarios pass in Firefox and Zen.
- No test uses the disposable Node spike host as production evidence.
- Any unavailable external signing step is marked as an external release gate, not silently
  reported as passed.
- No normal browser profile or history is modified by the harness.

---

## P3-12 — Complete Phase 3 acceptance and handoff

**Priority:** Critical  
**Status:** Planned  
**Depends on:** P3-11

### Tasks

- Run all frontend, Node contract, Rust, extension, browser-integration, and Nix gates.
- Map real evidence to the browser-related P1-24 criteria.
- Update `DESIGN.md`, `ARCHITECTURE.md`, `PRIVACY.md`, and README wording from Phase 2 browser
  simulation to the exact Phase 3 boundary.
- Add `PHASE3_HANDOFF.md` with verified behavior, limitations, installation instructions,
  evidence links, and Phase 4 dependencies.
- Preserve Phase 2 evidence; do not rewrite it as though browser enforcement existed then.
- List remaining release gates: AMO signing, hard lifecycle validation where still unresolved,
  and final target-package compatibility.
- Define the Phase 4 Niri/Noctalia adapter backlog without implementing it opportunistically.

### Browser-related P1-24 criteria to verify

- Paired Firefox extension is required and healthy before start.
- Visiting a non-whitelisted website displays the blocked experience.
- Existing non-whitelisted tabs become inaccessible before Working.
- Browser restrictions remain active while Paused.
- Browser restrictions are removed on every session-end path.
- Surviving blocked tabs return to their original URLs.
- Browser runtime failure interrupts the session and triggers cleanup.
- Restriction failures are clear and never hidden.
- Whitelist remains read-only during Working/Paused.
- Browsing destinations remain absent from Deepify persistence and diagnostics.

Application restriction, real Noctalia behavior, and complete system lifecycle criteria remain
Phase 4/5 pending and must not be marked complete here.

### Acceptance

- Every completed Phase 3 claim has reproducible evidence.
- All required local gates are green from the locked Nix environment.
- The production UI no longer labels browser restriction as simulated.
- Niri and Noctalia are still clearly identified as simulated.
- Phase 4 can replace its two mocks without changing the browser protocol or session rules.

---

## 6. Protocol Invariants

The final version-1 schema may refine field names in P3-01, but it must preserve these
invariants:

- Every message has `version`, `type`, and a unique message ID.
- Every response identifies its request.
- Session-affecting messages identify the session.
- Unknown fields and unknown enum values are rejected.
- Start policy contains normalized host/path rules, not raw whitelist form input.
- `blocked_attempt` contains no URL, title, query, hostname, or tab content.
- Restore results contain counts and safe error codes only.
- Tokens appear only in pairing/authentication messages and are redacted from diagnostics.
- A stale session ID cannot activate, mutate, or count against the current session.
- Start/stop requests are idempotent by request/session ID.
- No browser message can request subprocess execution, filesystem access, or arbitrary desktop
  commands.

---

## 7. Quality Gates

### Required on every milestone

Run the checks affected by that milestone and keep the working tree free of unrelated changes:

```bash
npm test
npm run typecheck
npm run lint
npm run format:check
npm run build
nix develop -c cargo fmt --all -- --check
nix develop -c cargo test --workspace --all-targets
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
```

Feature flags required by the existing Tauri package must remain included where applicable.

### Required for protocol/extension milestones

- Native-message schema tests
- Shared Rust/JavaScript URL fixture tests
- Extension unit tests with mocked browser APIs
- Extension lint and deterministic archive checks
- Native-host framing, socket, timeout, and malformed-input tests

### Required before P3-12 completion

```bash
nix flake check --all-systems
nix build .#default
nix build .#browser-native-host
nix build .#firefox-extension
```

Also run the production Firefox and Zen harness commands documented by P3-11. Hardware/browser
checks that cannot run in generic CI must have exact target-machine evidence.

---

## 8. Milestone Execution Rules

Implementation should process P3-01 through P3-12 sequentially.

For each milestone:

1. Read this plan, the referenced Phase 1 decisions, and current implementation.
2. Confirm the previous milestone is committed and the working tree is clean.
3. Implement only the current milestone and necessary enabling refactors.
4. Add behavior-focused tests; do not satisfy acceptance with source-string assertions alone.
5. Run the milestone's relevant gates.
6. Append exact evidence to `validation/PHASE3_CHECKS.md`.
7. Update only that milestone's status and completion evidence.
8. Run `git diff --check` and verify no sensitive browser data entered fixtures/logs.
9. Commit the milestone before starting the next one.

Suggested commit prefixes:

```text
feat(p3-01): ...
feat(p3-02): ...
test(p3-11): ...
docs(p3-12): ...
```

Do not run concurrent coding agents against the same working tree.

### Stop and escalate when

- Meeting restart recovery appears to require persisting attempted/original URLs.
- A material protocol or authority change conflicts with `ARCHITECTURE.md`.
- Firefox and Zen require incompatible behavior that cannot be isolated safely.
- The target Zen package/native-host path cannot be identified.
- A required permission materially broadens the approved privacy boundary.
- An unrecoverable test failure remains after a focused investigation.
- AMO credentials, signing approval, or another external decision is required.

Unavailable external signing is documented and deferred; it must not lead to disabling Firefox
signature protections or modifying a user's release profile.

---

## 9. Risks and Mitigations

| Risk | Mitigation |
| --- | --- |
| Desktop reports health from a stale socket | Fresh status preflight plus 20-second heartbeat and 60-second deadline |
| Another profile connects through the same host manifest | Explicit random token pairing and one persisted token hash |
| Browser URLs leak into desktop storage/logs | URL-free protocol events, redaction tests, in-extension restoration map |
| Browser and Rust matching diverge | One committed fixture suite executed by both implementations |
| Existing tabs remain briefly accessible | Start acknowledgement waits for complete existing-tab restriction pass |
| Disconnect races with normal completion | Idempotent request/session IDs and one backend finish transaction |
| Extension disconnect leaves blocked tabs stranded | Local extension restoration before reconnect and startup cleanup checkpoint |
| Native host becomes a local command channel | Fixed message schema; no shell, path, or arbitrary command fields |
| Malicious frames exhaust memory | 256 KiB limit, strict schema, bounded queues, and timeouts |
| Multiple native-host child processes appear | Desktop connection registry accepts only the paired profile as active |
| Browser Back bypasses restriction | Dedicated Back/reload/redirect integration scenarios |
| Zen packaging differs from Firefox | Explicit target Zen manifest-path milestone and runtime evidence |
| Signed installation is unavailable during development | Reproducible unsigned artifact plus separate external AMO release gate |
| Mixed Phase 3/4 status misleads users | Per-component health and explicit website-real/application-simulated copy |

---

## 10. Phase 3 Definition of Done

Phase 3 is complete when:

1. The production desktop uses the real browser adapter.
2. One explicitly paired Firefox or Zen profile is required for session start.
3. Existing and new non-whitelisted public HTTP(S) tabs are restricted.
4. Approved local, non-HTTP(S), and whitelisted destinations remain available.
5. Restriction remains active while the timer is paused.
6. Surviving blocked tabs restore on verified stop and failure paths.
7. Browser disconnect/health failure interrupts the session without a false protection claim.
8. Blocked attempts are persisted as aggregate counts only.
9. Pairing tokens and browsing destinations do not appear in diagnostics or desktop storage.
10. Firefox and Zen production integration scenarios pass with disposable profiles.
11. Nix builds the desktop, native host, manifest, and extension development artifact.
12. UI and documentation accurately distinguish real website protection from simulated Phase 4
    integrations.
13. All local quality gates pass and evidence is committed.
14. Remaining external signing and lifecycle gates are explicitly documented rather than
    misreported as complete.
