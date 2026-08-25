# Phase 1 — Product Definition Issues

## Purpose

This document tracks the remaining Phase 1 design and feasibility issues. Each issue should be resolved individually before implementation begins. Decisions made here should also be summarized in `DESIGN.md` and marked complete below.

## Current baseline

The following decisions are already established:

- [x] Initial platform: NixOS
- [x] Desktop environment: Niri on Wayland
- [x] Package format: Nix package
- [x] Browser support: Firefox-based browsers only
- [x] Data policy: entirely local, with no account or sign-in
- [x] MVP restriction level: Standard mode only
- [x] Visual direction: minimal and straightforward, inspired by vanilla Obsidian
- [x] The user must close existing non-whitelisted apps before a session starts
- [x] Existing non-whitelisted browser tabs must become inaccessible when a session starts

---

## P1-01 — Blocked app behavior during a session

**Status:** Resolved for MVP; technical behavior requires validation in P1-21  
**Category:** Standard-mode behavior  
**Priority:** Critical

Decide what the app does if a non-whitelisted application opens after a session has started.

### Feasibility finding

The MVP cannot guarantee that a non-whitelisted application is impossible to launch. A normal user-level application on Wayland does not control every process launch, and Niri can only report a graphical application after its window appears. Preventing process creation would require a separate system-level enforcement mechanism and would still need to address terminals, scripts, background processes, bypasses, and potential data loss.

Niri 26.04 exposes the capabilities needed for best-effort window enforcement:

- `niri msg event-stream` reports compositor events.
- `niri msg -j windows` reports window IDs, app IDs, and process IDs.
- `niri msg action close-window --id <ID>` sends a close request to a specific window without requiring it to be focused.

This means a blocked app may start and its window may appear briefly before the focus app detects it. A close request is cooperative rather than a forced process termination, so an application may refuse it or ask the user to handle unsaved work.

### MVP decision

When Niri reports a new non-whitelisted window during an active session:

1. Detect it as soon as it is reported by Niri's event stream.
2. Immediately request that Niri close the specific window by ID.
3. Show a warning identifying the blocked application and explaining that it is outside the active whitelist.
4. Recheck whether the window closed.
5. If it remains open or presents an unsaved-work prompt, keep the warning visible and require the user to close it manually before continuing their work.
6. Do not forcibly terminate the process, because doing so could destroy unsaved data.
7. Record the blocked attempt locally for restriction diagnostics and optional session summary display.

### Limitations

- Enforcement begins after a window is created; it does not prevent the underlying process from launching.
- There may be a short visible window or focus flash before closure.
- Background processes without windows are outside this window-based MVP restriction mechanism.
- The focus app is not tamper-proof because the user controls the same desktop account. Stronger system-level launch prevention belongs to a later Strict-mode investigation, not the Standard-mode MVP.
- If Niri monitoring or window closure fails, follow the restriction-system failure policy defined in P1-11.

### Validation required in P1-21

- Measure event-to-close delay.
- Confirm the relevant window-open event and its payload.
- Test apps with unsaved work and close-confirmation dialogs.
- Test whether focus can be returned to the previous allowed window to reduce access or flicker.
- Confirm behavior for multiple windows belonging to one blocked process.

---

## P1-02 — Restrictions while the timer is paused

**Status:** Resolved  
**Category:** Session lifecycle  
**Priority:** High

Decide whether pausing the timer also pauses restrictions.

### Resolution

Pausing the timer does not remove or pause either website or application restrictions. Restrictions remain fully active until the session is completed, ended early, or stopped by a restriction-system failure.

Music playback remains a separate control from the timer. The accounting of paused time in session history will be specified with the timer data model in P1-12.

---

## P1-03 — Whitelist changes during an active session

**Status:** Resolved  
**Category:** Standard-mode behavior  
**Priority:** High

Decide whether the user can add, modify, or remove whitelist entries during an active session.

### Resolution

The whitelist is read-only for the entire active session, including while the timer is paused. The user must complete or end the session before adding, modifying, or removing application or website rules.

---

## P1-04 — Ending a session early

**Status:** Resolved  
**Category:** Session lifecycle  
**Priority:** High

Standard mode permits early exit.

### Resolution

1. The user selects **End session**.
2. The app asks for confirmation, but does not ask the user to provide a reason.
3. After confirmation, there is no delay.
4. Website and application restrictions are removed immediately.
5. The session is stored with `finish_reason: "ended_early"`, displayed to the user as **Ended early**.
6. Redirected Firefox tabs immediately return to their original URLs according to P1-05.

---

## P1-05 — Restore blocked Firefox tabs after a session

**Status:** Resolved  
**Category:** Browser behavior  
**Priority:** High

Decide what happens to a browser tab after the extension blocks it.

### Resolution

The extension remembers each blocked tab's original URL. When a session ends for any reason, it immediately restores each surviving blocked tab to its original URL. For example, a tab redirected away from `youtube.com` automatically navigates back to `youtube.com`.

This behavior is the same for completed, ended-early, interrupted, and integration-failure sessions. If a tab was closed during the session, the extension does not recreate it.

---

## P1-06 — Website whitelist matching rules

**Status:** Resolved  
**Category:** Browser behavior  
**Priority:** Critical

Define exactly how Firefox URLs are compared with whitelist entries.

### Resolved MVP rules

- The extension restricts only `http://` and `https://` websites.
- All non-HTTP(S) destinations are allowed by default, including `about:`, `file:`, `ftp:`, and `moz-extension:` URLs.
- Allowing every Firefox `about:` page intentionally leaves an emergency route for disabling the extension. Whether to retain this in later versions is marked **unsure**.
- Public HTTP(S) websites use a default-deny policy: a destination must match an explicit whitelist rule.
- A rule for `google.com` automatically includes `google.com` and every subdomain, such as `docs.google.com` and `mail.google.com`. Unrelated domains remain blocked.
- HTTP and HTTPS are treated as the same destination for whitelist matching.
- URL path rules use prefix matching. For example, allowing `google.com/docs` also allows `google.com/docs/page`.
- Ports are ignored during matching. For example, a rule for `example.com` also matches `example.com:8080`.
- `localhost` and numeric loopback addresses, including `127.0.0.1` and `::1`, are allowed by default.
- Local-network HTTP(S) destinations are allowed by default. This includes private and link-local addresses reachable on the user's local network.
- Public IPv4 and IPv6 addresses require an explicit whitelist rule.

### Resolution

P1-06 is fully defined for the MVP. P1-22 should validate that Firefox and Zen Browser expose the required URL information consistently.

---

## P1-07 — Firefox profiles, containers, and private windows

**Status:** Partially resolved; containers require validation  
**Category:** Browser behavior  
**Priority:** Medium

Define the supported Firefox contexts and what happens when the extension is unavailable in one of them.

### Resolution

- The MVP integrates with one configured browser profile only.
- Firefox and Zen Browser are the two explicit MVP browser targets.
- Private windows are unsupported and may bypass website restrictions in the MVP. The UI and setup documentation must disclose this limitation clearly.
- Support for Firefox containers is not yet defined and must be checked in P1-22.
- Additional profiles are not monitored. Whether an unmonitored profile can remain open when a session starts depends on what Niri can identify and is deferred to P1-09/P1-10.

### Validation note

Firefox and Zen Browser may use separate profile and native-messaging configuration locations. P1-22 must determine whether the MVP can connect one configured profile in either browser or one profile in each browser simultaneously.

---

## P1-08 — Firefox extension disconnection or disablement

**Status:** Resolved; detection mechanics require validation in P1-22  
**Category:** Failure handling  
**Priority:** Critical

Define behavior when the extension is disconnected, disabled, or loses permissions.

### Resolution

- A session cannot start unless the configured browser extension is connected and healthy.
- The desktop app performs an explicit extension-health check at least once every minute during an active session.
- If the extension disconnects, is disabled, loses required permissions, or fails a health check, the desktop app stops the session automatically.
- The failed session is stored with `finish_reason: "extension_or_app_crash"`, displayed to the user as **Extension/app crashed**.
- Application restrictions are removed when this failure stops the session.
- The user receives a clear error explaining that focus protection stopped because the browser integration failed.
- The product never silently claims that restrictions remain active.

### Validation notes

- P1-22 should determine whether a persistent native-messaging connection can report failure immediately in addition to the one-minute heartbeat.
- If the desktop app itself crashes, it cannot write the finish reason at the moment of failure. On the next launch, recovery logic must recognize the abandoned active session, record `extension_or_app_crash`, and ensure restrictions are off. This behavior also belongs to P1-12.

---

## P1-09 — Pre-session application validation

**Status:** Mostly resolved; unknown app-ID presentation requires P1-10  
**Category:** Session entry  
**Priority:** Critical

Define the complete process for detecting and resolving existing non-whitelisted apps before starting a session.

### Resolution

1. Query every window exposed by Niri, including focused, unfocused, minimized-equivalent, hidden, floating, and scratchpad windows where Niri reports them.
2. Read each window's Wayland app ID.
3. Compare every detected window against the whitelist and implicit system allowances.
4. Show every non-whitelisted application and prevent session start until its windows are closed.
5. Let the user select a detected app and add it to the whitelist from the validation screen.
6. Let the user rerun validation.
7. Consider an application closed for focus purposes once it has no window capable of distracting the user. A background process with no user-facing window does not block session start.
8. Enable desktop **Do Not Disturb** through Noctalia when the session starts so background apps cannot distract through notifications.
9. Restore the user's previous Noctalia Do Not Disturb state whenever the session ends or fails.

How unknown or missing app IDs are identified and displayed is deferred with P1-10. Noctalia integration must be validated in P1-21.

---

## P1-10 — Application identity and implicit allowances

**Status:** Deferred to a dedicated discussion  
**Category:** Application rules  
**Priority:** Critical

This topic is too broad to define alongside the current issue batch. It requires a separate design discussion before application restriction can be implemented.

### Dedicated discussion scope

- Whether Wayland `app_id` is sufficient as the MVP identity
- Apps that expose multiple app IDs
- Terminal-hosted applications
- Progressive web apps
- Unknown or missing app IDs
- System dialogs, portals, security tools, and recovery tools that must always remain allowed
- Preventing the focus app from blocking itself

### Resolution

_No behavior is decided yet. Revisit P1-10 separately before finalizing P1-09, P1-15, P1-21, and application acceptance criteria._

---

## P1-11 — Restriction-system failure policy

**Status:** Resolved  
**Category:** Failure handling  
**Priority:** Critical

Define behavior when Niri IPC, the app monitor, Noctalia integration, or another restriction component fails.

### Resolution

- Do not begin a session unless all required restriction components report ready.
- A runtime failure immediately ends and interrupts the session, using the same fail-open cleanup approach as an extension failure.
- Remove all remaining website and application restrictions rather than leaving partially degraded protection active.
- Restore the pre-session Noctalia Do Not Disturb state.
- Restore redirected browser tabs according to P1-05.
- Store `finish_reason: "extension_or_app_crash"`, displayed as **Extension/app crashed**.
- Show a clear error identifying the failed component.
- The user must resolve the failure and start a new session; the failed session cannot be resumed.
- Never silently claim that protection remains active.

---

## P1-12 — Timer persistence and system lifecycle

**Status:** Resolved at product level; cleanup mechanics require feasibility validation  
**Category:** Timer behavior  
**Priority:** Critical

Define behavior across suspend, clock changes, crashes, restarts, logout, and shutdown.

### Resolution

- Enforce a single running desktop-app instance.
- If the computer suspends, the system clock changes, the app crashes or is force-closed, the user logs out, reboots, or shuts down during a session, that session ends and cannot resume.
- Store the session as interrupted. Use `finish_reason: "extension_or_app_crash"` for application or integration crashes. A later data-model pass may define more specific interruption codes for suspend, shutdown, or clock changes.
- Do not automatically resume restrictions or the timer after application or system restart.
- On the next launch, detect any abandoned active-session record, mark it interrupted, and ensure restrictions plus Do Not Disturb are off before allowing another session.
- Restrictions should be cleaned up before planned shutdown or app exit where possible. Because a hard crash cannot run cleanup code, P1-21 and P1-22 must validate fail-safe cleanup/recovery behavior.

---

## P1-13 — Focus and break cycle behavior

**Status:** Resolved  
**Category:** Timer behavior  
**Priority:** Medium

### Resolution

The MVP has no break concept or automatic focus/break cycles. Each session contains one manually configured working timer. When it ends, the app enters the not-working state and the user takes any break outside the session. To focus again, the user manually configures and starts a new timer.

The MVP session state model has three user-facing states:

1. **Not working:** No active session or restriction.
2. **Working:** Timer is counting down and restrictions are active.
3. **Paused:** Timer is stopped temporarily, but restrictions remain active.

Music remains independently controllable in all three states.

---

## P1-14 — First-launch and integration setup flow

**Status:** Mostly resolved; Settings re-entry remains to be confirmed  
**Category:** User flow  
**Priority:** High

### Resolution

1. Explain the desktop and browser integrations and known MVP limitations.
2. Verify that Niri IPC and Noctalia Do Not Disturb integration are available.
3. Install the Firefox extension either from Firefox Add-ons or deterministically through the user's NixOS system configuration.
4. Connect and test the single configured Firefox or Zen Browser profile.
5. Create an initial application and website whitelist.
6. Run compatibility and restriction tests.
7. Offer local music setup, but allow the user to skip files and folders and use the app without music.
8. Enter the Focus Room.

The exact deterministic extension packaging/configuration mechanism must be validated in P1-22. Whether the complete setup wizard can be rerun from Settings remains open.

---

## P1-15 — Start-session flow

**Status:** Mostly resolved; required fields remain to be specified  
**Category:** User flow  
**Priority:** Critical

### Resolution

1. Choose a timer duration.
2. Enter a session intention if desired.
3. Select a playlist if desired.
4. Verify Firefox-extension, Niri, and Noctalia integration health.
5. Scan all Niri windows.
6. If validation fails, show blocked apps and whitelist information; otherwise do not require a whitelist review.
7. Require the user to close or whitelist blocked apps, then rerun validation.
8. Apply browser restrictions and Noctalia Do Not Disturb.
9. Start the timer and application monitoring.
10. Begin music automatically if a playable queue is selected. A session without music starts normally.

### MVP limitation

There is no transactional rollback mechanism during startup. The implementation must still report partial startup failure clearly and stop the attempted session using the P1-11 cleanup policy rather than claiming that it began successfully.

Whether duration is the only required input, and whether intention is optional, should be confirmed in the timer UI/data-model discussion.

---

## P1-16 — Active-session flow and status

**Status:** Partially resolved; exact UI hierarchy awaits wireframing  
**Category:** User flow  
**Priority:** High

### Required active-session information and controls

- Remaining focus time
- Session intention
- Local audio controls
- Current restriction/integration health
- Notices for blocked actions
- Pause, resume, and early-end controls

### Resolution

- Persist active session status so that a crash or restart can identify an abandoned session and record it correctly.
- Count blocked distraction attempts during the session.
- At session end, show total deep-work time and the number of blocked attempts.
- Keep the active screen minimal; the prominence of health indicators and whether to show a live attempt history will be decided in P1-20 wireframes.

---

## P1-17 — Session completion flow

**Status:** Mostly resolved; notification behavior remains open  
**Category:** User flow  
**Priority:** High

### Resolution

1. Mark and persist the session with its finish reason.
2. Remove website and application restrictions.
3. Restore the previous Noctalia Do Not Disturb state.
4. Restore redirected tabs to their original URLs according to P1-05.
5. Fade out and stop the music.
6. Display a summary containing:
   - Total deep-work time
   - Number of blocked distraction attempts
   - Finish reason
7. Return to the idle Focus Room after the summary.

The MVP does not offer an automatic break because breaks are outside the session model. Whether to also send a desktop notification remains open.

---

## P1-18 — Whitelist management flow

**Status:** Mostly resolved; validation UX remains open  
**Category:** User flow  
**Priority:** High

### Resolution

- Users can select applications from currently detected Niri windows.
- Users can enter application IDs manually.
- The MVP has one shared application-and-website whitelist; reusable whitelist profiles are deferred.
- Users can run a whitelist test without beginning a real focus session.
- Whitelist editing is unavailable during active and paused sessions, per P1-03.

How malformed domains, paths, IP addresses, and unknown app IDs are displayed remains to be designed alongside P1-06 and P1-10.

---

## P1-19 — Local music import and library behavior

**Status:** Mostly resolved; metadata and ordering remain open  
**Category:** Local playlist  
**Priority:** Medium

### Resolution

- Users can import individual files and folders.
- MP3 is the only audio format required for the MVP.
- A playlist is a live reference to a local music folder rather than an independent copied catalog.
- When a file in an imported folder is added, moved, renamed, or deleted, the playlist content changes to reflect the folder.
- Individually imported files remain direct file references; a missing file becomes unavailable rather than being copied into app storage.
- The playback queue persists across application restarts.
- Shuffle and repeat are not included in the MVP.
- Music setup and playback are optional.

Whether title/artist metadata comes from MP3 tags or filenames, how folders are rescanned, and how queue order reacts to changed files remain open for P1-23.

---

## P1-20 — Low-fidelity wireframes

**Status:** Open  
**Category:** UX design  
**Priority:** High

Create implementation-neutral wireframes focused on information hierarchy and behavior rather than visual polish.

### Required states

- [ ] First-run setup
- [ ] Focus Room — idle
- [ ] Pre-session validation
- [ ] Pre-session blocked-app list
- [ ] Focus Room — active
- [ ] Blocked website page
- [ ] Session completion
- [ ] Sound Library
- [ ] Whitelist management
- [ ] Settings and integration health

### Resolution

_To be completed after the related behavior issues are resolved._

---

## P1-21 — Niri feasibility spike

**Status:** Open  
**Category:** Technical feasibility  
**Priority:** Critical

Build a small disposable prototype to verify that Niri supports the required application-restriction behavior.

### Verify

- [ ] Query all relevant open windows
- [ ] Read stable Wayland app IDs
- [ ] Detect newly created windows
- [ ] Detect focus changes
- [ ] Request that a window close safely
- [ ] Determine whether focus can be prevented or reversed
- [ ] Receive updates without excessive polling
- [ ] Detect and report loss of Niri IPC
- [ ] Read and change Noctalia Do Not Disturb state
- [ ] Restore the prior Do Not Disturb state after completion, failure, and startup recovery

### Deliverable

Document tested IPC commands/events, observed limitations, and the recommended MVP enforcement behavior.

---

## P1-22 — Firefox extension feasibility spike

**Status:** Open  
**Category:** Technical feasibility  
**Priority:** Critical

Build a small disposable extension and desktop communication prototype.

### Verify

- [ ] Enumerate existing tabs
- [ ] Observe or intercept navigation
- [ ] Redirect non-whitelisted tabs
- [ ] Preserve original URLs
- [ ] Restore or expose a restoration action
- [ ] Communicate with the Tauri desktop app
- [ ] Detect extension disconnection
- [ ] Work in Firefox
- [ ] Work in Zen Browser
- [ ] Install from Firefox Add-ons
- [ ] Install/configure deterministically through NixOS system configuration
- [ ] Connect exactly one configured profile
- [ ] Confirm and document the private-window bypass
- [ ] Determine container limitations

### Possible communication methods

- Firefox native messaging
- A constrained local service

### Deliverable

Document the selected communication method, required permissions, limitations, and installation approach on NixOS.

---

## P1-23 — Local audio feasibility spike

**Status:** Open  
**Category:** Technical feasibility  
**Priority:** Medium

Verify local audio behavior in the proposed Tauri stack.

### Verify

- [ ] Select individual files
- [ ] Select directories, if required by P1-19
- [ ] Play MP3 files
- [ ] Read basic MP3 track metadata and compare tag-versus-filename fallback behavior
- [ ] Retain individual-file and folder references across restarts
- [ ] Rescan live folder playlists after files are added, moved, renamed, or deleted
- [ ] Persist the playback queue and define ordering when source files change
- [ ] Handle output-device changes

### Deliverable

Document supported formats, metadata behavior, storage approach, and known platform limitations.

---

## P1-24 — MVP acceptance criteria

**Status:** Draft  
**Category:** Product definition  
**Priority:** Critical

Finalize testable acceptance criteria after the behavior and feasibility issues are resolved.

### Initial draft

- [ ] A session cannot start while a detected non-whitelisted app is open.
- [ ] A session cannot start unless the Firefox extension is connected and healthy.
- [ ] Visiting a non-whitelisted domain during a session displays the blocked experience.
- [ ] Existing non-whitelisted tabs become inaccessible when a session starts.
- [ ] Opening a non-whitelisted app during a session triggers the selected Standard-mode response.
- [ ] Restrictions are removed when a session ends.
- [ ] Restrictions remain active while the timer is paused.
- [ ] The whitelist cannot be changed during an active or paused session.
- [ ] Blocked tabs automatically return to their original URLs when a session ends.
- [ ] Any restriction-component runtime failure ends the session and removes all restrictions.
- [ ] Suspend, clock change, crash, logout, reboot, or shutdown ends and interrupts an active session rather than resuming it.
- [ ] Only one desktop-app instance can run.
- [ ] Session history remains entirely local and stores deep-work time, blocked-attempt count, and finish reason.
- [ ] Noctalia Do Not Disturb is enabled during a session and its previous state is restored afterward.
- [ ] Local MP3 playback works without internet access and is optional.
- [ ] No account or sign-in is displayed or required.
- [ ] The default UI is minimal and uses the Obsidian-inspired scheme.
- [ ] The selected MVP color scheme persists between launches.
- [ ] Restriction failures are reported clearly and are never hidden from the user.

### Resolution

_To be finalized._

---

## Suggested resolution order

Resolve issues in this order to minimize rework:

1. **P1-21:** Niri feasibility spike
2. **P1-22:** Firefox extension feasibility spike
3. **P1-01:** Blocked app behavior during a session
4. **P1-06:** Website whitelist matching rules
5. **P1-08:** Firefox extension failure behavior
6. **P1-09:** Pre-session application validation
7. **P1-10:** Application identity and implicit allowances
8. **P1-11:** Restriction-system failure policy
9. **P1-02–P1-05:** Remaining Standard-mode lifecycle decisions
10. **P1-12–P1-13:** Timer and break lifecycle
11. **P1-14–P1-19:** Complete user flows and feature details
12. **P1-23:** Local audio feasibility spike
13. **P1-20:** Low-fidelity wireframes
14. **P1-24:** Final MVP acceptance criteria
