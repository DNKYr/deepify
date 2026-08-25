# Deep Work Focus App — Design Document

## Status

- **Stage:** Product definition
- **Document type:** Living design document
- **Last updated:** 2026-08-25

This document captures the current product direction. Decisions and details will be revised incrementally as the design develops. Remaining product-definition work is tracked in [`PHASE1_ISSUES.md`](PHASE1_ISSUES.md).

---

## 1. Product Overview

A calm desktop focus application built around three connected systems:

1. A focus timer
2. A local music playlist
3. Restrictions on apps and websites outside a user-defined whitelist

The product should help users enter a focused state, stay there, and review their work patterns without introducing unnecessary complexity or distraction.

---

## 2. Core Capabilities

### 2.1 Focus Timer

- One manually configured work timer per session
- No automatic break or focus/break cycle in the MVP
- A written intention for each session
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

- **Desktop shell:** Tauri
- **Interface:** React and TypeScript
- **Native integration:** Rust and platform-specific helpers
- **Website filtering:** Companion Firefox extension
- **Local storage:** SQLite or local application files
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
- Redirect blocked pages to a calm focus screen while retaining each original URL
- When any session ends, immediately restore each surviving blocked tab to its original URL

### 5.3 Application Rules

- Identify applications by a stable executable or desktop-entry ID rather than display name
- Keep essential operating-system, security, accessibility, and recovery tools available
- Run a pre-session check across all windows reported by Niri, including hidden and scratchpad windows
- Require the user to close every non-whitelisted window before the session can begin
- Treat a background process with no distracting window as closed for focus purposes
- Enable Noctalia Do Not Disturb during a session and restore its previous state afterward
- When a non-whitelisted window opens during a session, immediately request that Niri close that window and show a warning identifying the blocked app
- If the window refuses the close request or presents an unsaved-work prompt, require the user to close it manually
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
- A normal completion fades out music, removes restrictions, restores tabs and Do Not Disturb, then shows deep-work time, blocked-attempt count, and finish reason before returning to the Focus Room

---

## 6. Delivery Plan

### Phase 1 — Product Definition

- Confirm Linux as the first supported operating system (completed)
- Define the behavior of each restriction level
- Map primary user flows and edge cases
- Produce low-fidelity wireframes

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
- Optional local MP3 file/folder import and persistent playback queue
- Noctalia Do Not Disturb during sessions
- Basic daily and weekly history containing deep-work time, blocked attempts, and finish reason
- A minimal, vanilla Obsidian-inspired interface
- Simple color-scheme customization built on reusable color tokens

Gentle and Strict modes may be designed after the Standard-mode MVP is validated. Strict mode's emergency recovery flow will be considered at that time.

---

## 8. Open Decisions

Remaining decisions and deferred discussions are tracked individually in [`PHASE1_ISSUES.md`](PHASE1_ISSUES.md). The next major deferred topic is P1-10: application identity and implicit system allowances.

---

## 9. Decision Log

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

### 2026-08-25 — Blocked apps during an active session

- **Decision:** When Niri reports a new non-whitelisted window, the app immediately requests that Niri close the window and shows a warning. If the close request is refused, the warning requires the user to close the window manually.
- **Safety:** The MVP does not forcibly terminate processes because that may destroy unsaved data.
- **Limitation:** This is best-effort window enforcement, not process-launch prevention. A blocked app may open or receive focus briefly before detection.

### 2026-08-25 — Application validation and Do Not Disturb

- **Decision:** Pre-session validation includes every Niri-reported window. An app with no user-facing window is considered closed for focus purposes.
- **Decision:** A session enables Noctalia Do Not Disturb and restores the user's previous state afterward.
- **Deferred:** Application identity, implicit system allowances, and unknown app IDs require a dedicated P1-10 discussion.

### 2026-08-25 — Timer lifecycle

- **Decision:** The MVP has no break cycle. Its states are Not working, Working, and Paused; restrictions remain active while paused.
- **Decision:** Suspend, clock change, crash, force-close, logout, reboot, or shutdown interrupts and ends an active session. Sessions do not resume afterward, and only one app instance may run.

### 2026-08-25 — Session completion and history

- **Decision:** On completion, music fades out; restrictions and Do Not Disturb are removed; tabs are restored; and the app shows deep-work time, blocked-attempt count, and finish reason before returning to the Focus Room.

### 2026-08-25 — Local music MVP

- **Decision:** Music is optional. The MVP imports individual MP3 files and folders, treats folders as live playlists, persists the queue, and starts selected music with a session.
- **Deferred:** Shuffle, repeat, and non-MP3 formats.

### 2026-08-25 — Local-only data

- **Decision:** All data remains local. The product requires no account, authentication, or sign-in.

### 2026-08-25 — Visual direction and theming

- **Decision:** The interface will be straightforward and minimal, using vanilla Obsidian as the primary visual reference.
- **MVP scope:** Use an Obsidian-like default color scheme and support simple color-scheme customization.
- **Deferred:** Full theme and palette customization will be considered after the MVP.
