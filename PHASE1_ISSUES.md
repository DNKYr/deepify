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

**Status:** Resolved and validated by P1-21
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

### P1-21 validation results

- Niri emitted `WindowOpenedOrChanged` with the window ID, app ID, PID, workspace, and state.
- The tested app-launch-to-window-event time was approximately 252 ms; this includes application startup and compositor scheduling rather than only monitor latency.
- A targeted cooperative close completed in approximately 42 ms in the disposable-window test.
- An unsaved-work confirmation kept the window open, confirming the need for the persistent manual-close warning.
- Niri cannot prevent initial focus, but the app can restore focus to the last allowed window by ID on a best-effort basis.
- Multiple windows from one process have separate window IDs and can be closed independently.
- Repeated `WindowOpenedOrChanged` events must be deduplicated by window ID and relevant changed fields.

Full evidence is recorded in [`P1-21_NIRI_VALIDATION.md`](P1-21_NIRI_VALIDATION.md).

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

P1-06 is fully defined for the MVP. P1-22 validated the required URL information and
tab-update behavior in Firefox and Zen Browser.

---

## P1-07 — Firefox profiles, containers, and private windows

**Status:** Resolved with documented limitations
**Category:** Browser behavior  
**Priority:** Medium

Define the supported Firefox contexts and what happens when the extension is unavailable in one of them.

### Resolution

- The MVP integrates with one configured browser profile only.
- Firefox and Zen Browser are the two explicit MVP browser targets.
- Private windows are unsupported and may bypass website restrictions in the MVP. The UI and setup documentation must disclose this limitation clearly.
- Container tabs are part of the one configured profile. The MVP observes their
  `cookieStoreId` but does not request the `cookies` permission or implement per-container
  whitelist policy.
- Additional profiles are not monitored and remain unblocked in the MVP. They are a disclosed bypass under P1-10.

### Validation note

Firefox and Zen Browser may use separate profile and native-messaging configuration locations.
P1-22 validated the core connection in one configured profile per browser; target-package
installation still needs to verify Zen's final manifest location.

---

## P1-08 — Firefox extension disconnection or disablement

**Status:** Resolved and validated by P1-22
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

- P1-22 confirmed that a persistent native-messaging disconnect is observable immediately
  through the extension port, in addition to the one-minute heartbeat requirement.
- If the desktop app itself crashes, it cannot write the finish reason at the moment of failure. On the next launch, recovery logic must recognize the abandoned active session, record `extension_or_app_crash`, and ensure restrictions are off. This behavior also belongs to P1-12.

---

## P1-09 — Pre-session application validation

**Status:** Resolved for MVP with documented identity limitations
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

Windows with a missing or unknown app ID are labeled **Unidentified app — allowed in MVP** and do not block session entry. This is a disclosed enforcement limitation from P1-10. P1-21 validated Noctalia state reading, enabling, disabling, and restoration.

---

## P1-10 — Application identity and implicit allowances

**Status:** Resolved for MVP with documented bypasses; system-component policy is tentative
**Category:** Application rules  
**Priority:** Critical

### MVP resolution

- Use Niri's Wayland `app_id` as the application whitelist identity. Each distinct app ID is treated as a distinct identity.
- A window with a missing or unknown app ID is unblockable and therefore allowed. Display it as **Unidentified app — allowed in MVP** so the limitation is visible.
- Do not inspect processes running inside a terminal. Terminal-hosted commands are ignored and treated as safe. The terminal window itself is still classified using the terminal emulator's Wayland app ID.
- Keep the focus app permanently present in the implicit whitelist. The user cannot remove this entry.
- Allow system components implicitly in the MVP. The exact built-in system-component list is tentative and should be assembled from the target NixOS/Niri environment during implementation.
- Allow unmonitored Firefox and Zen Browser profiles. Because they may share the browser's Wayland app ID but lack the paired extension connection, they are a documented website-restriction bypass in the MVP.
- Progressive web apps and apps exposing multiple IDs receive no special grouping in the MVP; each Niri-reported app ID follows the ordinary rule.

### Known limitations

- Missing/unknown app IDs, terminal-hosted commands, private browser windows, and unmonitored browser profiles can bypass restrictions.
- The policy for identifying which known app IDs qualify as system components is not final. Revisit it after collecting the actual NixOS/Niri window inventory.
- These trade-offs are accepted for Standard mode and must be disclosed in setup or integration-health documentation. Stronger identity and process enforcement is deferred.

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
- Restrictions should be cleaned up before planned shutdown or app exit where possible. P1-21
  validated Noctalia cleanup for planned failure and startup-recovery simulations, and P1-22
  validated browser-tab restoration after native stop and disconnect. Because a hard crash
  cannot run cleanup code, complete cross-component recovery still requires implementation-level
  testing.

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

**Status:** Resolved
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

P1-22 defined deterministic native-host packaging through
`programs.firefox.nativeMessagingHosts.packages`. The extension itself remains a separate
signed XPI/AMO installable. The complete setup wizard can be rerun from Settings.

---

## P1-15 — Start-session flow

**Status:** Resolved
**Category:** User flow  
**Priority:** Critical

### Resolution

1. Choose a timer duration; this is the only required session input.
2. Enter an optional session intention.
3. Select an optional playlist.
4. Verify Firefox-extension, Niri, and Noctalia integration health.
5. Scan all Niri windows.
6. If validation fails, show blocked apps and whitelist information; otherwise do not require a whitelist review.
7. Require the user to close or whitelist blocked apps, then rerun validation.
8. Apply browser restrictions and Noctalia Do Not Disturb.
9. Start the timer and application monitoring.
10. Begin music automatically if a playable queue is selected. A session without music starts normally.

### MVP limitation

There is no transactional rollback mechanism during startup. The implementation must still report partial startup failure clearly and stop the attempted session using the P1-11 cleanup policy rather than claiming that it began successfully.

Duration is required. Intention and playlist are optional.

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

**Status:** Resolved
**Category:** User flow  
**Priority:** High

### Resolution

1. Mark and persist the session with its finish reason.
2. Remove website and application restrictions.
3. Restore the previous Noctalia Do Not Disturb state.
4. Restore redirected tabs to their original URLs according to P1-05.
5. Fade out and stop the music.
6. Send a desktop notification that the session ended.
7. Display a summary containing:
   - Total deep-work time
   - Number of blocked distraction attempts
   - Finish reason
8. Return to the idle Focus Room after the summary.

The MVP does not offer an automatic break because breaks are outside the session model.

---

## P1-18 — Whitelist management flow

**Status:** Resolved
**Category:** User flow  
**Priority:** High

### Resolution

- Users can select applications from currently detected Niri windows.
- Users can enter application IDs manually.
- The MVP has one shared application-and-website whitelist; reusable whitelist profiles are deferred.
- Users can run a whitelist test without beginning a real focus session.
- Whitelist editing is unavailable during active and paused sessions, per P1-03.
- Strictly reject malformed website domains, paths, IP addresses, and manually entered app IDs. Invalid input is not saved, does not become a whitelist member, and has no effect on active rules.
- Show a clear validation error and retain the entered value so the user can correct or cancel it. Do not silently accept a partial value.
- Unknown or missing app IDs detected from existing windows are shown as **Unidentified app — allowed in MVP** per P1-10, but cannot be added as malformed whitelist entries.

---

## P1-19 — Local music import and library behavior

**Status:** Resolved for MVP
**Category:** Local playlist  
**Priority:** Medium

### Resolution

- Users can import individual files and folders.
- MP3 is the only audio format required for the MVP.
- A playlist is a live reference to a local music folder rather than an independent copied catalog.
- When a file in an imported folder is added, moved, renamed, or deleted, the playlist content changes to reflect the folder. The MVP rescans recursively and sorts tracks by normalized relative path.
- Individually imported files remain direct file references; a missing file becomes unavailable rather than being copied into app storage.
- The playback queue persists across application restarts.
- Shuffle and repeat are not included in the MVP.
- Music setup and playback are optional.

ID3 title, artist, and album tags take precedence. When tags are absent, `Artist - Title.mp3` supplies artist and title; otherwise the filename stem is the title.
The persisted queue retains entries whose paths still exist, removes deleted paths, and appends newly discovered paths in lexical order. A move or rename is a new path and is treated as remove-plus-add rather than guessed identity matching.
Full evidence and the implementation handoff are recorded in [`p1-23-local-audio-spike/README.md`](p1-23-local-audio-spike/README.md).

---

## P1-20 — Low-fidelity wireframes

**Status:** Draft complete; pending product review
**Category:** UX design  
**Priority:** High

Create implementation-neutral wireframes focused on information hierarchy and behavior rather than visual polish.

### Required states

- [x] First-run setup
- [x] Focus Room — idle
- [x] Pre-session validation
- [x] Pre-session blocked-app list
- [x] Focus Room — active
- [x] Blocked website page
- [x] Session completion
- [x] Sound Library
- [x] Whitelist management
- [x] Settings and integration health

### Resolution

The following implementation-neutral wireframes define the information hierarchy and
primary behavior for the MVP. They are deliberately textual: spacing, typography,
icons, and component technology remain implementation decisions.

#### Shared shell and interaction rules

The desktop app uses one quiet shell for all non-browser screens:

```text
+--------------------------------------------------------------------------------+
| Deep Work Focus                                      [status] [Settings]        |
+----------------------+---------------------------------------------------------+
| Focus Room           |                                                         |
| Sound Library        |                     page content                        |
| Whitelist            |                                                         |
| Session History      |                                                         |
+----------------------+---------------------------------------------------------+
| [integration summary]                                      [Help / Recovery]   |
+--------------------------------------------------------------------------------+
```

- The left navigation is available only when no session is active. During Working
  and Paused states, the Focus Room is the only primary destination; Settings and
  whitelist editing are unavailable.
- The top status is concise and redundant with the page's main state: **Ready**,
  **Working**, **Paused**, or **Attention needed**. It is not the only place where
  a failure is shown.
- Every page has one visually dominant primary action and a low-emphasis way to
  go back or cancel.
- Destructive or session-ending actions require confirmation. Closing an existing
  blocked application is never represented as a force-quit action.
- Focus indicators, labels, and validation messages must not depend on color alone.
  All controls need keyboard focus, visible names, and an accessible status update.

#### 1. First-run setup

The setup wizard is a linear flow with a visible step indicator. The user may skip
music, but cannot finish until required desktop and browser integrations are healthy.

```text
+--------------------------------------------------------------------------------+
| Set up Deep Work Focus                                      Step 1 of 5          |
| [Integrations] -- [Browser] -- [Whitelist] -- [Music] -- [Ready]               |
+--------------------------------------------------------------------------------+
|                                                                                |
| Protect your focus on this computer                                            |
|                                                                                |
| Deep Work Focus uses Niri, Noctalia, and one Firefox-based browser profile.    |
| Restrictions are local and work without an internet connection.                |
|                                                                                |
| [✓] Niri / Wayland                 Connected                                  |
| [✓] Noctalia notifications          Available                                  |
| [!] Browser extension               Needs connection                           |
|                                                                                |
| Known MVP limits: private windows, other browser profiles, and some            |
| unidentified or terminal-hosted applications may bypass restrictions.          |
|                                                                                |
| [Run integration check]                                      [Continue →]      |
+--------------------------------------------------------------------------------+
```

Wizard states:

- **Integrations:** Check Niri IPC and Noctalia Do Not Disturb support. A failed
  check explains the repair action and keeps **Continue** disabled.
- **Browser:** Choose Firefox or Zen Browser, install/enable the extension, pair
  exactly one profile, and run a connection test. Show the connected profile as
  a human-readable label without exposing the pairing token.
- **Whitelist:** Add at least any required application IDs or website rules. The
  same validation rules as Whitelist Management apply; invalid values remain in
  their fields and are not saved.
- **Music:** Import MP3 files or live folders, or choose **Skip music**. Skipping
  is explicit but reversible from Sound Library.
- **Ready:** Summarize integration health and limitations, then offer **Open Focus
  Room**. The wizard can be rerun later from Settings.

#### 2. Focus Room — idle / Not working

This is the default landing screen and the destination after a session summary.

```text
+--------------------------------------------------------------------------------+
| Focus Room                                                [Ready]              |
+--------------------------------------------------------------------------------+
|                                                                                |
|                         Not working                                            |
|                                                                                |
|                         00:00                                                  |
|                                                                                |
|  Duration *       [ 45 min v ]       Intention     [What will you focus on? ]  |
|  Playlist         [ No music      v ]                                            |
|                                                                                |
|                         [ Start focus session ]                                |
|                                                                                |
|  Focus Shield     Ready   Apps: 8 allowed   Websites: 5 allowed                |
|  Browser profile  Firefox — Connected       DND: Off (restored after session)  |
|                                                                                |
|  Recent focus     Today 0h 00m · 0 sessions                  [View history]     |
+--------------------------------------------------------------------------------+
```

- Duration is the only required input. The start action is disabled until it is a
  valid positive duration and required integrations are ready.
- Intention, playlist, and music are optional. A missing or unavailable track is
  shown as unavailable and does not make a music-free session impossible.
- Selecting **Start focus session** opens Pre-session validation. If validation
  succeeds, the app applies restrictions and starts without an extra review page.
- If an integration is unhealthy, replace the start action with **Fix integrations**
  and link to the relevant Settings health detail.

#### 3. Pre-session validation

Validation is a short, blocking gate between the idle room and an active session.

```text
+--------------------------------------------------------------------------------+
| Check before starting                                      [Cancel]             |
+--------------------------------------------------------------------------------+
| Focus plan: 45 min · "Write project brief" · No music                         |
|                                                                                |
| [✓] Firefox extension       Connected and healthy                              |
| [✓] Niri                    Connected · 8 windows found                        |
| [✓] Noctalia DND            Ready to preserve and restore current state        |
| [!] Open application check  2 windows need attention                          |
|                                                                                |
| [Review blocked applications]                             [Run checks again]   |
+--------------------------------------------------------------------------------+
```

- Show progress while each check runs; do not imply that protection is active yet.
- A successful check changes the primary action to **Start session**. The app then
  enables browser restrictions, records the prior DND state, enables DND, starts
  application monitoring, and starts the timer.
- Any startup failure shows the failed component and the cleanup result. The app
  returns to Not working and never presents the session as started.
- The check can find windows on any Niri-reported workspace, including hidden or
  scratchpad windows where they are reported.

#### 4. Pre-session blocked-app list

This screen is shown only when validation finds identified, non-whitelisted windows.

```text
+--------------------------------------------------------------------------------+
| Close these applications before starting                    2 windows         |
| The Focus Shield cannot start while these windows are open.                    |
+--------------------------------------------------------------------------------+
| Application                    Windows       Identity / reason                 |
| [icon] Steam                    1             app_id: steam                     |
| [icon] Chat client              1             app_id: example.chat              |
|                                                                                |
| [Select row]  [Add to whitelist]  [Open whitelist]                              |
|                                                                                |
| Unknown identity windows are allowed in the MVP and are labeled                 |
| "Unidentified app — allowed in MVP". Terminal-hosted commands are not listed.  |
|                                                                                |
| [Refresh window list]                                      [Back to checks]     |
+--------------------------------------------------------------------------------+
```

- List each detected window or a grouped app identity with a window count. The
  detail view must make it possible to distinguish multiple windows of one app.
- **Add to whitelist** is available for a valid detected app ID. It does not close
  the window; the user must refresh and confirm the resulting state.
- **Refresh window list** is the only automatic recheck. Session start stays
  disabled until every identified non-whitelisted window is gone or whitelisted.
- A close affordance may request a cooperative Niri close, but the UI must explain
  that unsaved-work prompts can leave the window open and require manual closure.
- The list is not shown for a clean validation result. Missing or unknown app IDs
  appear as an informational limitation, not a blocking error.

#### 5. Focus Room — active / Working and Paused

The active room keeps the timer and the current intention dominant. Restriction
health is always visible, but detailed diagnostics remain secondary.

```text
+--------------------------------------------------------------------------------+
| Focus Room                                  [Working] [Shield active]            |
+--------------------------------------------------------------------------------+
|                                                                                |
|                         37:42                                                  |
|                  Writing the project brief                                     |
|                                                                                |
|                    [ Pause ]        [ End session ]                            |
|                                                                                |
|  Sound                                                                       |
|  [previous]  Deep Focus.mp3                         [pause] [next] [volume]    |
|  Queue: 3 tracks · shuffle/repeat unavailable in MVP                           |
|                                                                                |
|  Focus Shield                                                                  |
|  [✓] Websites restricted     [✓] Applications monitored     [✓] DND enabled    |
|  Blocked attempts: 2                                                           |
|                                                                                |
|  [!] A blocked application was closed. View details                            |
+--------------------------------------------------------------------------------+
```

- In Working, the primary control is **Pause**. In Paused, the timer area changes
  to **Paused · 37:42 remaining** and the control becomes **Resume**.
- Pausing stops the timer only. Keep the shield status explicitly **Restrictions
  remain active while paused** near the Resume control.
- **End session** opens a confirmation dialog with no required reason and no delay.
  Confirmation immediately begins the normal cleanup flow.
- A blocked app notice identifies the app and whether Niri closed it or manual
  closure is still required. Keep the notice persistent while the window remains.
- A component failure replaces the normal health row with an urgent, specific
  failure message and states that the session has ended and restrictions are being
  removed. Do not offer Resume.

#### 6. Blocked website page

This is rendered by the browser extension in place of a blocked HTTP(S) destination.

```text
+--------------------------------------------------------------------------------+
| Deep Work Focus — Website blocked                                             |
+--------------------------------------------------------------------------------+
|                                                                                |
|                              Focus Shield                                      |
|                                                                                |
|                 This website is outside your active whitelist.                 |
|                 The page will be restored when your session ends.              |
|                                                                                |
|                 Destination: example.com / path                                |
|                 Session remaining: 37:42                                       |
|                                                                                |
|                 [Return to previous page]                                      |
|                                                                                |
|                 Restrictions are active. No restore option is available        |
|                 during this session.                                           |
+--------------------------------------------------------------------------------+
```

- Retain the original URL in extension state, not as a user-editable control.
- The page must not provide a restore or bypass action while the session is active.
  A normal browser back action should not reveal the blocked destination again.
- **Return to previous page** navigates to a safe page such as the Focus Room or
  browser new-tab page; it does not restore the blocked URL.
- Use a clear domain/path summary without exposing query parameters unnecessarily.
- When the session ends, the extension restores each surviving redirected tab to
  its original URL. The blocked page itself does not need to offer that action.

#### 7. Session completion

Cleanup happens before the summary is shown. The summary is informational and has
one clear way back to the idle room.

```text
+--------------------------------------------------------------------------------+
| Session complete                                                               |
+--------------------------------------------------------------------------------+
|                                                                                |
|                         Deep work complete                                     |
|                                                                                |
|                         45:00                                                  |
|                         Deep-work time                                         |
|                                                                                |
|  Blocked attempts       2                                                      |
|  Finish reason          Completed                                              |
|  Websites               Restored                                               |
|  Applications            Restrictions removed                                  |
|  Notifications          Previous Do Not Disturb state restored                 |
|                                                                                |
|                         [Back to Focus Room]                                   |
+--------------------------------------------------------------------------------+
```

- Finish reasons are human-readable: **Completed**, **Ended early**, **Interrupted**,
  or **Extension/app crashed** as appropriate.
- For an early end, show the actual deep-work time rather than the configured
  duration. Do not ask for a reason.
- If cleanup itself reports a problem, keep the summary visible and add a clear
  recovery link; never claim that all restrictions were removed without confirmation.
- Send the desktop notification when the session ends, before or alongside this
  summary. The summary remains available even if notifications are disabled.

#### 8. Sound Library

The library separates source references, discovered tracks, and the persistent queue.

```text
+--------------------------------------------------------------------------------+
| Sound Library                                             [Focus Room]          |
+--------------------------------------------------------------------------------+
| [Import MP3 files] [Import folder]                         Search [         ]   |
|                                                                                |
| Sources                                                                        |
| [folder] Ambient / Music                         Live folder · 24 tracks       |
| [file]   Single track.mp3                       Direct file · Available         |
|                                                                                |
| Tracks in Ambient / Music                                                        |
| [ ] Artist — Track title.mp3                    04:12          [Add to queue]   |
| [ ] Another track.mp3                           03:48          [Add to queue]   |
| [!] Missing track.mp3                           Unavailable                    |
|                                                                                |
| Playback queue (saved locally)                                                 |
| 1. Artist — Track title.mp3                         [remove]                    |
| 2. Another track.mp3                                [remove]                    |
|                                                                                |
| MP3 only · folders rescan recursively · shuffle/repeat not in MVP              |
+--------------------------------------------------------------------------------+
```

- Import accepts individual MP3 files or folders. Non-MP3 files are ignored and
  need not appear as errors.
- Show ID3 title, artist, and album metadata when available; otherwise use the
  documented filename fallback.
- A folder source is live: refresh/rescan reflects additions and removals. Missing
  direct-file references remain visible as unavailable rather than being copied.
- Queue reconciliation is deterministic: retain existing paths, remove deleted
  paths, and append newly discovered paths in normalized lexical order. A move or
  rename is treated as remove-plus-add.
- During a session, Sound Library is read-only. Compact playback controls remain
  available in the active Focus Room, including volume and play/pause.

#### 9. Whitelist management

Use one shared whitelist screen with separate application and website sections.

```text
+--------------------------------------------------------------------------------+
| Whitelist                                                  [Test configuration] |
| Changes apply to the next session.                                             |
+--------------------------------------------------------------------------------+
| Applications                                                                  |
| Allowed app IDs                                                                |
| [Alacritty]                                                   [Remove]          |
| [dev.zed.Zed]                                                 [Remove]          |
|                                                                                |
| [Choose detected window]  [Add app ID manually]                                |
|                                                                                |
| Websites                                                                       |
| Allowed domains / paths                                                        |
| [docs.google.com/docs]                                         [Remove]         |
| [localhost]                                                    [Remove]         |
| Add website [ example.com/path                         ] [Add]                  |
|                                                                                |
| [!] Enter a valid domain, IP address, or path. Nothing was saved.               |
|                                                                                |
| Implicitly allowed: focus app · MVP system components · local addresses        |
| MVP limitations: private windows and unmonitored browser profiles may bypass.  |
|                                                                                |
| [Cancel changes]                                             [Save whitelist]  |
+--------------------------------------------------------------------------------+
```

- Whitelist edits are disabled in Working and Paused states. Replace save controls
  with a read-only explanation and a link back to the active Focus Room.
- Detected app selection should show the Niri `app_id` before adding it. Missing or
  unknown IDs are informational and cannot be added as whitelist entries.
- Website validation is atomic: malformed domains, paths, IP addresses, and app IDs
  remain in the input, show an actionable error, and have no effect on saved rules.
- Explain matching beside the website list: subdomains are included, paths use
  prefix matching, ports are ignored, HTTP and HTTPS are equivalent, and local
  addresses are allowed by default.
- **Test configuration** runs the pre-session checks without starting a timed
  session or changing restrictions. Its result uses the same validation and blocked
  app list states.

#### 10. Settings and integration health

Settings is organized around recovery and configuration rather than frequent use.

```text
+--------------------------------------------------------------------------------+
| Settings                                                                       |
+--------------------------------------------------------------------------------+
| Integration health                                                             |
| [✓] Niri / Wayland                 Connected                 [Details]          |
| [✓] Noctalia DND                   Available                 [Details]          |
| [✓] Firefox extension              Paired · Firefox            [Reconnect]      |
| [!] Audio output                   Waiting for device          [Details]       |
|                                                                                |
| Focus defaults                                                                 |
| Default duration        [45 min v]                                             |
| Color scheme            [Obsidian dark v]                                      |
| Notifications            [✓] Session completion                               |
|                                                                                |
| Recovery and privacy                                                            |
| [Run setup wizard again]   [Run complete integration test]                      |
| [Open emergency recovery]                                                        |
| Local-only data · no account or sign-in                                        |
|                                                                                |
| MVP limitations: one browser profile; private windows and some app identities  |
| may bypass restrictions.                                                        |
+--------------------------------------------------------------------------------+
```

- Each **Details** view reports the current state, last successful check, and a
  concrete repair action. A stale heartbeat or lost IPC is an error, not a warning
  that can be ignored before starting.
- Audio-device loss is a visible waiting/recovery state; it does not affect focus
  restrictions. The user can continue without music or select another available
  device when offered.
- **Open emergency recovery** explains how to restore browser tabs, stop a failed
  session, and return DND to its previous state. It must remain accessible from
  the tools needed to recover the app safely.
- Color-scheme selection is limited to built-in MVP schemes and persists locally.

#### Wireframe decisions and deferred polish

- The active session shows a compact blocked-attempt count and the latest notice,
  not a scrolling activity feed; detailed history belongs in Session History.
- Health is persistent but compact in normal operation and expands only when a
  component needs attention.
- Mobile layouts, full theme customization, automatic breaks, shuffle/repeat,
  reusable whitelist profiles, and strict/gentle restriction-mode UI are deferred.
- The next design pass should validate labels, keyboard order, error announcements,
  and the exact visual density with a small interactive prototype.

---

## P1-21 — Niri feasibility spike

**Status:** Resolved
**Category:** Technical feasibility  
**Priority:** Critical

Build a small disposable prototype to verify that Niri supports the required application-restriction behavior.

### Verify

- [x] Query all relevant open windows
- [x] Read stable Wayland app IDs
- [x] Detect newly created windows
- [x] Detect focus changes
- [x] Request that a window close safely
- [x] Determine whether focus can be prevented or reversed
- [x] Receive updates without excessive polling
- [x] Detect and report loss of Niri IPC
- [x] Read and change Noctalia Do Not Disturb state
- [x] Restore the prior Do Not Disturb state after completion, failure, and startup recovery

### Deliverable

Documented in [`P1-21_NIRI_VALIDATION.md`](P1-21_NIRI_VALIDATION.md), including tested IPC commands/events, measured close behavior, observed limitations, and the recommended MVP enforcement behavior. Niri provides focus reversal rather than focus prevention, and close requests remain cooperative when an application presents an unsaved-work confirmation.

### Accepted implementation constraints

- Maintain one persistent Niri event-stream connection rather than polling.
- Deduplicate repeated window-change events.
- Store the last allowed window ID for best-effort focus restoration.
- Issue one targeted cooperative close request, then re-query the window inventory.
- If the window remains, require manual closure rather than repeatedly closing or terminating its process.
- Treat event-stream termination or IPC errors as restriction-component failures under P1-11.
- Read and preserve Noctalia's prior Do Not Disturb state before enabling it.

---

## P1-22 — Firefox extension feasibility spike

**Status:** Resolved for MVP feasibility; signed add-on publication remains a release step
**Category:** Technical feasibility  
**Priority:** Critical

Build a small disposable extension and desktop communication prototype. The spike is
located in [`p1-22-firefox-spike/`](p1-22-firefox-spike/).

### Verification checklist

- [x] Enumerate existing tabs, including URL, `incognito`, and `cookieStoreId`.
- [x] Observe navigation through `tabs.onUpdated`.
- [x] Redirect non-whitelisted HTTP(S) tabs to a blocked page.
- [x] Preserve original URLs and restore surviving tabs when the session stops.
- [x] Exercise a diagnostic restore action from the spike's blocked page. This control is test-only and must not ship in the MVP because restoring a blocked URL during an active session would bypass the restriction. Production restoration occurs only when the session ends.
- [x] Exercise the native-messaging contract that a Tauri desktop app or helper must implement.
- [x] Detect native-host disconnection and restore blocked tabs.
- [x] Work in Firefox 154.0.
- [x] Work in Zen Beta 1.21.15b for the core restriction/restore flow.
- [ ] Install from Firefox Add-ons: not performed because this disposable extension is not
      published or signed; temporary installation was validated.
- [x] Define deterministic NixOS native-host packaging with
      `programs.firefox.nativeMessagingHosts.packages`.
- [x] Define one configured profile pairing: the extension persists a profile-specific
      pairing token and sends it in `hello`; production desktop code must accept only the
      paired token. The browser/native-host manifest alone cannot distinguish profiles.
- [x] Confirm and document the private-window bypass. Firefox runtime validation passed;
      Zen's Marionette implementation does not support the private-window automation
      command used by the probe, so Zen private-window behavior needs a manual follow-up.
- [x] Determine container limitations: tabs expose `cookieStoreId`, but creating a tab in a
      container requires the `cookies` permission. The spike deliberately does not request
      that permission, so its runtime probe fails with `No permission for cookieStoreId`.
      Containers therefore share the one profile-wide connection and have no separate
      whitelist policy in the MVP.

### Possible communication methods

- Firefox native messaging
- A constrained local service

### Deliverable

Native messaging is selected. The spike requests `tabs`, `webNavigation`, `nativeMessaging`,
`storage`, and `contextualIdentities`; it intentionally does not request `cookies`. Before
release, the production extension must review and remove any permission that its final
implementation does not use. The native host uses Firefox's length-prefixed JSON stdio
protocol. A Tauri desktop app can implement that host contract directly or delegate it to a
helper; the Node host in this spike is only a protocol/runtime probe.

NixOS packages the host and manifest with the Firefox native-messaging-host package option,
while the extension remains a separate signed XPI/AMO installation. The disposable harness
uses temporary Marionette installation and does not modify the user's browser profiles.

### Validation results — 2026-08-26

The Marionette harness ran two scenarios in each browser:

| Browser | Start/stop | Native disconnect | Private-window probe |
| --- | --- | --- | --- |
| Firefox 154.0 | PASS | PASS | PASS: private URL remained unrestricted |
| Zen Beta 1.21.15b | PASS | PASS | Not automated: Zen reports `openWindow() not supported` |

In both browsers, an HTTP(S) tab was redirected to `blocked.html` with its original URL
encoded in the query string, then restored on stop. The disconnect scenario restored the tab
after the host exited. The `hello` message included the current tab inventory and profile
pairing token; heartbeat acknowledgements and native-host framing were also observed.

The container probe confirmed the permission boundary described above. This is a deliberate
least-privilege result, not a failed MVP requirement: existing container tabs can be observed,
but the MVP does not provide container-specific policy.

### Accepted implementation constraints and release follow-ups

- Use a persistent Firefox native-messaging connection and retain the one-minute heartbeat as a second health check.
- Enforce the profile pairing token in production desktop code; the native-host manifest cannot identify a browser profile by itself.
- On native disconnect, restore blocked tabs immediately and signal the desktop side to apply P1-08/P1-11 session cleanup.
- Do not include the spike's manual **Restore** control in the production blocked page while a session is active.
- Treat containers as ordinary tabs in the configured profile; do not add per-container whitelist behavior or request `cookies` for the MVP.
- Keep private windows outside extension access and disclose them as an intentional MVP bypass.
- Manually confirm the private-window bypass in the target Zen package because Zen's Marionette implementation could not automate that check.
- Publish a signed XPI through Firefox Add-ons for release; temporary unsigned installation is development-only.
- Package the native host through `programs.firefox.nativeMessagingHosts.packages` and verify the final native-host manifest location for the target Zen package.
- Replace or port the disposable Node host with the production Tauri/native helper implementation.

Full spike evidence and reproduction instructions are in [`p1-22-firefox-spike/README.md`](p1-22-firefox-spike/README.md).

---

## P1-23 — Local audio feasibility spike

**Status:** Resolved for MVP feasibility; hardware device switching remains an implementation test
**Category:** Technical feasibility  
**Priority:** Medium

Verify local audio behavior in the proposed Tauri stack.

### Verify

- [x] Select individual files
- [x] Select directories, if required by P1-19
- [x] Play MP3 files
- [x] Read basic MP3 track metadata and compare tag-versus-filename fallback behavior
- [x] Retain individual-file and folder references across restarts
- [x] Rescan live folder playlists after files are added, moved, renamed, or deleted
- [x] Persist the playback queue and define ordering when source files change
- [x] Handle output-device changes through an explicit device-loss/recovery state contract; real hardware switching remains an implementation test

### Validation results — 2026-08-26

The disposable harness in [`p1-23-local-audio-spike/`](p1-23-local-audio-spike/) passed
the file-selection, MP3 playback/decode, metadata, persistence, rescan, and queue tests.
The output-device loss/recovery state contract also passed. A real output-device switch
was not observable in this headless run because no PipeWire output device was available;
that remains an implementation/release test.

### Deliverable

The MVP supports MP3 only. It stores direct file and live folder references plus the
playback queue in local JSON/SQLite-equivalent application storage; audio is not copied
into app storage. ID3 metadata is preferred with the documented filename fallback.
Folder playlists are recursive and sorted by relative path. Missing individual files
remain unavailable references. The production Tauri player must expose output-device
loss as a health state and reselect the default/new device when one becomes available.
Native playback was validated with the target machine's FFmpeg/VLC stack; Tauri WebView
or Rust audio-crate integration still requires Phase 2 implementation testing.

---

## P1-24 — MVP acceptance criteria

**Status:** Draft  
**Category:** Product definition  
**Priority:** Critical

Finalize testable acceptance criteria after the behavior and feasibility issues are resolved.

### Initial draft

- [ ] A session cannot start while a detected, identified, non-whitelisted app is open.
- [ ] Missing or unknown app IDs are visibly labeled and allowed as an MVP limitation.
- [ ] Terminal-hosted processes are ignored; the terminal window is classified by its Wayland app ID.
- [ ] The focus app and MVP system components are implicitly allowed.
- [ ] A session cannot start unless the paired Firefox extension is connected and healthy.
- [ ] Visiting a non-whitelisted domain during a session displays the blocked experience.
- [ ] Existing non-whitelisted tabs become inaccessible when a session starts.
- [ ] Opening a non-whitelisted app during a session triggers the selected Standard-mode response.
- [ ] Restrictions are removed when a session ends.
- [ ] Restrictions remain active while the timer is paused.
- [ ] The whitelist cannot be changed during an active or paused session.
- [ ] Malformed whitelist input is rejected completely, never saved, and shown with a clear validation error.
- [ ] Blocked tabs automatically return to their original URLs when a session ends.
- [ ] Any restriction-component runtime failure ends the session and removes all restrictions.
- [ ] Suspend, clock change, crash, logout, reboot, or shutdown ends and interrupts an active session rather than resuming it.
- [ ] Only one desktop-app instance can run.
- [ ] Session history remains entirely local and stores deep-work time, blocked-attempt count, and finish reason.
- [ ] Noctalia Do Not Disturb is enabled during a session and its previous state is restored afterward.
- [ ] Duration is required; session intention and music are optional.
- [ ] Session completion sends a desktop notification.
- [ ] The setup wizard can be rerun from Settings.
- [ ] Local MP3 playback works without internet access and is optional.
- [ ] ID3 metadata takes precedence, with `Artist - Title.mp3` and filename-stem fallbacks.
- [ ] Live folder playlists rescan recursively and reconcile the queue deterministically after file changes.
- [ ] Audio-device loss enters a visible waiting state and playback can recover onto an available device.
- [ ] No account or sign-in is displayed or required.
- [ ] The default UI is minimal and uses the Obsidian-inspired scheme.
- [ ] The selected MVP color scheme persists between launches.
- [ ] Restriction failures are reported clearly and are never hidden from the user.

### Resolution

_To be finalized._

---

## Suggested resolution order

Resolve issues in this order to minimize rework:

1. **P1-21:** Niri feasibility spike — completed
2. **P1-22:** Firefox extension feasibility spike — completed
3. **P1-01–P1-17:** Product behavior and primary flows — completed for MVP, except the implementation-level P1-12 recovery tests
4. **P1-18:** Whitelist validation UX — completed
5. **P1-19/P1-23:** Local audio details and feasibility — completed for MVP feasibility; hardware device switching remains an implementation test
6. **P1-20:** Low-fidelity wireframes — open
7. **P1-24:** Final MVP acceptance criteria — draft
