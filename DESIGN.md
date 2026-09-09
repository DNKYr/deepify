# Deepify — Design Document

## Status

- **Stage:** Phase 1 complete; Phase 2 functional prototype complete
- **Document type:** Living design document
- **Last updated:** 2026-08-28

This document captures the current product direction. Decisions and details will be revised incrementally as the design develops. Phase 1 decisions, approved wireframes, and acceptance criteria are recorded in [`PHASE1_ISSUES.md`](PHASE1_ISSUES.md). Production boundaries and Phase 2 work are defined in [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`PHASE2_IMPLEMENTATION_PLAN.md`](PHASE2_IMPLEMENTATION_PLAN.md).

---

## 1. Product Overview

**Deepify** is a calm desktop focus application built around three connected systems:

1. A focus timer
2. A local music playlist
3. Restrictions on apps and websites outside a user-defined whitelist

The product should help users enter a focused state, stay there, and review their work patterns without introducing unnecessary complexity or distraction.

---

## 2. Core Capabilities

### 2.1 Focus Timer

- One manually configured work timer per session
- No automatic break or focus/break cycle in the MVP
- A required duration with an optional written intention and playlist
- Start, pause, resume, reset, and end controls
- Restrictions remain active while paused
- Confirmation before ending a session early
- Session history, daily focus totals, blocked-attempt count, and finish reason

### 2.2 Local Playlist

- Import individual MP3 files or local folders
- Treat an imported folder as a live playlist whose contents follow that folder
- Play, pause, skip, and adjust volume
- Maintain a visible playback queue across application restarts
- Start selected music automatically with a session and fade it out when the session ends
- Allow sessions without music
- Defer shuffle, repeat, and non-MP3 formats until after the MVP
- Operate without a streaming service or internet connection

### 2.3 Focus Restrictions

- Maintain a whitelist of permitted apps and websites
- Restrict destinations outside the whitelist during a focus session
- Offer multiple levels of restriction
- Clearly communicate permissions and active restrictions
- Provide a safe recovery mechanism if restrictions are configured incorrectly

---

## 3. Recommended Technical Direction

A normal website cannot reliably restrict other applications or websites. The product should therefore be built as a desktop application.

### Proposed stack

- **Desktop shell:** Tauri 2
- **Interface:** React, TypeScript, and Vite
- **Native integration:** Rust and platform-specific helpers
- **Website filtering:** Companion Firefox extension and separate Rust native-messaging host
- **Local storage:** Backend-owned SQLite plus direct references to local audio files/folders
- **Accounts and cloud services:** None; all data remains on the user's device
- **Initial package format:** Nix package

Application blocking will require platform-specific implementations. The first version targets the developer's own Linux environment: **NixOS, Niri, and Wayland**. Broader Linux and operating-system support may follow later.

---

## 4. Primary Screens

### 4.1 Focus Room

The main working screen should include:

- A prominent timer
- The current session intention
- Start, pause, resume, and end controls
- Current local track and compact queue
- Focus Shield status
- The active whitelist
- A visually quiet, low-distraction layout

### 4.2 Sound Library

- Import individual MP3 files or local folders
- Represent each imported folder as a live playlist
- Search and sort tracks
- Show that MP3 is the MVP format
- Reflect files added, moved, renamed, or removed from imported folders

### 4.3 Whitelist and Restrictions

- Select allowed applications
- Add allowed website domains
- Strictly reject malformed whitelist entries without saving or partially applying them
- Show a clear validation error while retaining invalid input for correction
- Select the restriction level
- Review required system permissions
- Test the configuration before beginning a session

### 4.4 Session History

- Daily and weekly focus time
- Completed and interrupted sessions
- Commonly used focus durations
- Optional history of session intentions

### 4.5 Settings

- Default work-timer duration
- Notification and sound preferences
- Application launch behavior
- Browser-extension connection status
- Operating-system permissions
- Emergency recovery options
- An option to rerun the complete setup wizard
- MVP color-scheme selection

### 4.6 Visual Direction

The interface should be straightforward, minimal, and optimized for deep work. Vanilla Obsidian is the primary visual reference for the MVP.

#### MVP appearance

- Use an Obsidian-like dark neutral color scheme
- Favor simple panels, restrained borders, clear typography, and compact controls
- Keep visual hierarchy clear without decorative elements or attention-grabbing animation
- Show only information and actions relevant to the current task
- Use a small set of semantic color tokens so the scheme can be changed without redesigning components
- Provide simple color-scheme selection; this may initially be limited to a small set of built-in schemes or core accent and surface colors

#### Later customization

Full visual customization is deferred until after the MVP. A later version may support custom palettes, broader theme controls, and user-created themes.

---

## 5. Restriction Model

### 5.1 Restriction Levels

#### Standard — MVP

Close, hide, or redirect apps and websites that are outside the active whitelist. Standard mode is the only restriction mode included in the MVP. Users may end a session early after confirmation, but the whitelist is read-only while a session is active. Pausing the timer does not pause or remove restrictions.

#### Gentle — Later

Warn the user when they attempt to open a blocked app or website, but do not forcibly prevent access. Gentle mode is deferred until after the MVP.

#### Strict — Later

Apply enforced restrictions and prevent changes to the whitelist or early session termination until the timer ends. Strict mode, including its emergency recovery behavior, is deferred until after the MVP.

### 5.2 Website Rules

- Support Firefox and Zen Browser in the MVP
- Integrate with one configured browser profile; additional profiles are not monitored
- Private windows are unsupported and may bypass restrictions in the MVP; disclose this limitation clearly
- Restrict only HTTP and HTTPS websites; allow all non-HTTP(S) schemes by default
- Apply a default-deny policy to public HTTP(S) destinations: only explicitly whitelisted websites are accessible
- A rule for `google.com` includes the domain and all subdomains, such as `docs.google.com`; unrelated domains remain blocked
- Use prefix matching for path-specific rules; allowing `/docs` also allows `/docs/page`
- Ignore ports during matching; a host rule applies on every port
- Treat HTTP and HTTPS as the same destination for matching
- Allow `localhost`, loopback IPs such as `127.0.0.1` and `::1`, and local-network addresses by default
- Require explicit rules for public IPv4 and IPv6 addresses
- Allow all Firefox `about:` pages and other non-HTTP(S) destinations in the MVP, intentionally preserving an emergency exit that can disable the extension; whether to keep this backdoor later is unsure
- Block new navigation to non-whitelisted destinations
- Prevent access to any existing non-whitelisted tabs when a session starts
- Redirect blocked pages to a calm focus screen while retaining each original URL; do not offer restoration while the session remains active
- When any session ends, immediately restore each surviving blocked tab to its original URL

### 5.3 Application Rules

- Use Niri's Wayland `app_id` as the MVP whitelist identity; distinct app IDs are distinct entries
- Allow windows with missing or unknown app IDs and label them **Unidentified app — allowed in MVP**
- Ignore commands and processes hosted inside terminals; classify only the terminal window by its emulator app ID
- Keep the focus app permanently and implicitly whitelisted
- Implicitly allow system components in the MVP; the exact built-in list is tentative pending inventory of the target environment
- Do not specially group progressive web apps or applications exposing multiple app IDs
- Allow unmonitored Firefox and Zen Browser profiles as a documented MVP bypass
- Run a pre-session check across all windows reported by Niri, including hidden and scratchpad windows
- Require the user to close every non-whitelisted window before the session can begin
- Treat a background process with no distracting window as closed for focus purposes
- Enable Noctalia Do Not Disturb during a session and restore its previous state afterward
- Monitor one persistent Niri event stream and deduplicate repeated events by window ID and relevant fields
- When a non-whitelisted window opens during a session, record the attempt, immediately request that Niri close that specific window, and re-query the inventory
- Store the last allowed window ID and restore focus to it on a best-effort basis; Niri cannot prevent the blocked window from receiving initial focus
- If the window refuses the cooperative close request or presents an unsaved-work prompt, require the user to close it manually; do not repeatedly send close requests
- Never forcibly terminate a blocked process in the MVP, because that could cause data loss
- Treat this as best-effort window enforcement: an app may launch and appear briefly before Niri reports and closes its window
- Avoid restricting the tools required to disable or recover the product safely
- Initially target NixOS, the Niri compositor, and Wayland

### 5.4 Session Lifecycle

- The user-facing states are **Not working**, **Working**, and **Paused**
- Pausing stops the timer but leaves all restrictions active
- The whitelist is read-only during Working and Paused states
- Ending early requires confirmation but no reason or delay; store `finish_reason: "ended_early"`
- A restriction component failure ends the session, removes every restriction, restores Do Not Disturb and browser tabs, and stores `finish_reason: "extension_or_app_crash"`
- Suspend, clock change, crash, force-close, logout, reboot, or shutdown interrupts and ends the session; sessions do not resume afterward
- Enforce one running desktop-app instance
- A normal completion fades out music, removes restrictions, restores tabs and Do Not Disturb, sends a desktop notification, then shows deep-work time, blocked-attempt count, and finish reason before returning to the Focus Room

---

## 6. Delivery Plan

### Phase 1 — Product Definition (complete)

- Confirm Linux as the first supported operating system (completed)
- Define the behavior of each restriction level (completed)
- Map primary user flows and edge cases (completed)
- Produce and approve low-fidelity wireframes (completed)
- Approve MVP acceptance criteria (completed)

### Phase 2 — Functional Prototype

- Build the Focus Room interface
- Implement timer state and notifications
- Import and play local audio
- Persist preferences and session history locally
- Simulate restriction status without enforcing restrictions

### Phase 3 — Website Restriction

- Build the Firefox extension
- Synchronize session and whitelist state
- Create the blocked-page experience
- Disable access to existing non-whitelisted tabs when a session starts
- Test navigation, original-tab restoration, one-minute extension-health monitoring, and browser restarts in Firefox and Zen Browser

### Phase 4 — Application Restriction

- Implement for NixOS, Niri, and Wayland first
- Add operating-system permission onboarding
- Integrate Noctalia Do Not Disturb with previous-state restoration
- Detect every Niri-reported window before starting a session
- Block session start until non-whitelisted applications are closed
- Detect and restrict newly launched non-whitelisted applications
- Enforce whitelist rules
- Package the application with Nix
- Add permission diagnostics

### Phase 5 — Refinement and Release

- Improve accessibility and keyboard navigation
- Handle sleep, wake, shutdown, and restart recovery
- Handle missing audio files and audio-device changes
- Review performance, privacy, and security
- Package the application for installation

---

## 7. Current Recommendation for the MVP

The agreed MVP is:

- Linux support targeting NixOS, Niri, and Wayland
- Distribution as a Nix package
- Firefox-based browser support only
- Entirely local user data with no authentication or sign-in
- Standard restriction mode only; Gentle and Strict modes are deferred
- Pre-session checks requiring all non-whitelisted applications to be closed
- Existing non-whitelisted browser tabs made inaccessible during a session and automatically restored afterward
- Browser integration with one configured Firefox or Zen Browser profile; private windows are an explicitly disclosed bypass in the MVP
- One manual work timer per session with no break cycle
- Optional local MP3 file/folder import and persistent playback queue. Folders are live,
  recursive playlists sorted by relative path; ID3 metadata wins over the documented
  `Artist - Title.mp3` filename fallback. Queue reconciliation retains existing paths,
  removes deleted paths, and appends new or renamed/moved paths deterministically.
- Noctalia Do Not Disturb during sessions
- Basic daily and weekly history containing deep-work time, blocked attempts, and finish reason
- A minimal, vanilla Obsidian-inspired interface
- Simple color-scheme customization built on reusable color tokens

Gentle and Strict modes may be designed after the Standard-mode MVP is validated. Strict mode's emergency recovery flow will be considered at that time.

---

## 8. Next Work

Phase 2 is complete. The runnable desktop prototype implements the approved flow,
local persistence, production local MP3 playback, notifications, recovery, and
explicitly simulated restriction adapters. Verification and the P1-24 prototype
mapping are recorded in [`validation/PHASE2_CHECKS.md`](validation/PHASE2_CHECKS.md).

Next work is the sequential Phase 3 production browser integration defined in
[`PHASE3_IMPLEMENTATION_PLAN.md`](PHASE3_IMPLEMENTATION_PLAN.md), followed by
Phase 4 Niri/Noctalia integration. Release follow-ups remain hard-crash and
lifecycle validation, real PipeWire device switching, signed browser publication,
and the target system-component inventory.

---

## 9. Decision Log

### 2026-08-26 — MVP name and executable

- **Decision:** The MVP product name is **Deepify**.
- **Decision:** The desktop executable uses the normalized lowercase name `deepify`.
- **Toolchain baseline:** nixpkgs 26.05 is locked at `f4f698677b11021a8f84f452e23ae9ef2427bec3`.
- **Permanent identifiers:** Tauri `com.deepify.desktop`, native host
  `com.deepify.browser`, Firefox extension `focus@deepify.local`.

### 2026-08-25 — Initial operating system

- **Decision:** Linux will be the first supported operating system.
- **Reason:** It is the developer's current operating system and therefore the most practical environment for development and testing.

### 2026-08-25 — Initial Linux environment

- **Decision:** The first version targets NixOS with Niri on Wayland and will be distributed as a Nix package.
- **Reason:** This is the developer's own desktop environment and the immediate development and testing target.

### 2026-08-25 — Browser support

- **Decision:** The MVP supports Firefox-based browsers only.
- **Reason:** The developer uses Firefox-based browsers exclusively.

### 2026-08-25 — Restriction mode and session entry

- **Correction:** Standard mode—not Strict mode—is the only MVP restriction mode. Gentle and Strict modes are deferred until after the MVP.
- **Decision:** Before a session begins, every non-whitelisted app must be closed. Existing non-whitelisted browser tabs become inaccessible for the duration of the session.
- **Standard-mode behavior:** Users can end a session early after confirmation, with no reason prompt or delay. The session is stored with `finish_reason: "ended_early"` and displayed as **Ended early**.
- **Pause behavior:** Restrictions remain active while the timer is paused.
- **Whitelist behavior:** The whitelist cannot be changed during an active or paused session.

### 2026-08-25 — Website matching and browser scope

- **Decision:** The MVP targets Firefox and Zen Browser and integrates with one configured browser profile.
- **Known limitation:** Private windows are unsupported and may bypass website restriction in the MVP.
- **Matching:** Only HTTP(S) websites are restricted. Public HTTP(S) access is default-deny; allowing a domain automatically allows all its subdomains; paths use prefix matching; ports are ignored; and HTTP/HTTPS are equivalent.
- **Exceptions:** Non-HTTP(S) destinations, `localhost`, loopback IPs, and local-network addresses are allowed by default. Public IP addresses require explicit rules.
- **Emergency exit:** All Firefox `about:` pages remain allowed in the MVP, including settings that can disable the extension. Whether this remains available in later versions is unsure.

### 2026-08-25 — Blocked-tab restoration

- **Decision:** The extension retains each blocked tab's original URL and immediately restores surviving tabs to those URLs whenever the session ends, regardless of finish reason.

### 2026-08-25 — Browser and restriction integration failure

- **Decision:** The desktop app checks extension health at least once per minute. Any required restriction-component failure stops the session, removes all restrictions, restores tabs and the previous Do Not Disturb state, and reports the failed component.
- **History:** Store `finish_reason: "extension_or_app_crash"` and display **Extension/app crashed**. Crash recovery must apply this reason on the next launch when it cannot be written immediately.

### 2026-08-26 — Firefox extension feasibility

- **Decision:** Use Firefox native messaging for the browser/desktop integration. The spike
  used `tabs`, `webNavigation`, `nativeMessaging`, `storage`, and `contextualIdentities` and
  intentionally omitted `cookies`; production packaging must remove any permission unused by
  the final implementation.
- **Decision:** Keep one configured profile per browser integration. The extension sends a
  profile-specific pairing token in its hello message; production desktop code must enforce
  the paired token because the native-host manifest only identifies the extension.
- **Validation:** Firefox 154.0 and Zen Beta 1.21.15b passed restriction, restore, and native
  disconnect scenarios. Private windows remain an intentional bypass; Zen private-window
  automation needs manual validation because its Marionette implementation does not support
  the probe's `openWindow()` command.
- **Limitations:** Container tabs expose `cookieStoreId`, but the MVP has no per-container
  whitelist policy. NixOS packages the native host through
  `programs.firefox.nativeMessagingHosts.packages`; the extension remains a separate signed
  XPI/AMO installable. The target Zen package's native-host manifest location needs release
  validation.
- **Blocked-page constraint:** The production blocked page cannot restore a URL while the
  session is active. The spike's manual restore button is diagnostic only.
- **Evidence:** [`validation/p1-22-firefox-spike/README.md`](validation/p1-22-firefox-spike/README.md)

### 2026-08-25 — Blocked apps during an active session

- **Decision:** When Niri reports a new non-whitelisted window, the app immediately requests that Niri close the window and shows a warning. If the close request is refused, the warning requires the user to close the window manually.
- **Safety:** The MVP does not forcibly terminate processes because that may destroy unsaved data.
- **Limitation:** This is best-effort window enforcement, not process-launch prevention. A blocked app may open or receive focus briefly before detection.

### 2026-08-25 — Application validation and Do Not Disturb

- **Decision:** Pre-session validation includes every Niri-reported window. An app with no user-facing window is considered closed for focus purposes.
- **Decision:** A session enables Noctalia Do Not Disturb and restores the user's previous state afterward.
- **P1-21 validation:** Niri window inventory, event monitoring, targeted cooperative close, focus reversal, IPC failure detection, and Noctalia state preservation all passed. Focus prevention is unavailable, so enforcement remains best-effort. Repeated change events require deduplication, and unsaved-work prompts can hold a window open.
- **Evidence:** [`validation/P1-21_NIRI_VALIDATION.md`](validation/P1-21_NIRI_VALIDATION.md)
- **P1-10 identity policy:** Use Wayland `app_id`; allow missing/unknown IDs; ignore terminal-hosted processes; permanently allow the focus app; and allow unmonitored browser profiles. System components are implicitly allowed, but their exact built-in list remains tentative.
- **Known bypasses:** Unknown IDs, terminal-hosted commands, unmonitored profiles, and private browser windows are outside complete MVP enforcement.

### 2026-08-25 — Timer lifecycle

- **Decision:** The MVP has no break cycle. Its states are Not working, Working, and Paused; restrictions remain active while paused.
- **Decision:** Suspend, clock change, crash, force-close, logout, reboot, or shutdown interrupts and ends an active session. Sessions do not resume afterward, and only one app instance may run.

### 2026-08-26 — Low-fidelity UX structure

- **Approved:** P1-20 defines the shared desktop shell, setup wizard, idle and active Focus Room, validation and blocked-app states, browser blocked page, session summary, Sound Library, whitelist, Settings, Session History, and lifecycle transitions.
- **Active-session hierarchy:** Keep the timer and intention dominant; show compact integration health, blocked-attempt count, latest warning, and audio controls without a scrolling activity feed.
- **Navigation:** Hide nonessential primary navigation during Working and Paused states. Restrictions remain visible and active while paused.
- **Review status:** Approved for the MVP.
- **Details:** [`PHASE1_ISSUES.md#p1-20--low-fidelity-wireframes`](PHASE1_ISSUES.md#p1-20--low-fidelity-wireframes)

### 2026-08-26 — Phase 1 approval

- **Decision:** P1-20 wireframes and P1-24 acceptance criteria are approved.
- **Outcome:** Phase 1 product definition is complete. Work may proceed to Phase 2 functional prototype planning and implementation.

### 2026-08-26 — Whitelist validation

- **Decision:** Malformed domains, paths, IP addresses, and manually entered app IDs are rejected completely. Invalid entries are not saved or partially applied; the UI retains the input and shows a clear validation error.

### 2026-08-26 — Session inputs and setup

- **Decision:** Timer duration is required; intention and playlist are optional.
- **Decision:** The complete first-launch setup wizard can be rerun from Settings.

### 2026-08-25 — Session completion and history

- **Decision:** On completion, music fades out; restrictions and Do Not Disturb are removed; tabs are restored; a desktop notification is sent; and the app shows deep-work time, blocked-attempt count, and finish reason before returning to the Focus Room.

### 2026-08-25 — Local music MVP

- **Decision:** Music is optional. The MVP imports individual MP3 files and folders, treats folders as live playlists, persists the queue, and starts selected music with a session.
- **P1-23 validation:** ID3 title/artist/album tags take precedence over filename metadata. Without tags, `Artist - Title.mp3` supplies artist and title; otherwise the filename stem is the title. Folder scans are recursive and sorted by relative path. Existing queue paths are retained, deleted paths removed, and newly discovered paths appended.
- **Storage:** Persist direct file paths, folder paths, and queue order locally; do not copy audio into app storage. Missing direct files remain unavailable.
- **Output devices:** A device-loss/recovery state contract was validated. Real PipeWire device switching and production Tauri player integration remain implementation/release tests.
- **Deferred:** Shuffle, repeat, and non-MP3 formats.
- **Evidence:** [`validation/p1-23-local-audio-spike/README.md`](validation/p1-23-local-audio-spike/README.md)

### 2026-08-25 — Local-only data

- **Decision:** All data remains local. The product requires no account, authentication, or sign-in.

### 2026-08-25 — Visual direction and theming

- **Decision:** The interface will be straightforward and minimal, using vanilla Obsidian as the primary visual reference.
- **MVP scope:** Use an Obsidian-like default color scheme and support simple color-scheme customization.
- **Deferred:** Full theme and palette customization will be considered after the MVP.
