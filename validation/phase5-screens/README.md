# Actual Phase 5 desktop captures

Captured on 2026-09-25 by `validation/phase5-desktop-lifecycle.py` with
`DEEPIFY_SCREENSHOTS` set to this directory. These are embedded-frontend Tauri
screens, with disposable app data and a protocol browser peer. Niri and Noctalia
are real, guarded as described in `../PHASE5_CHECKS.md`.

01: setup. 02: default Obsidian idle. 03: Mist idle. 04: working. 05: paused.
06: early-end summary. 07: disconnected-browser failure with incomplete cleanup.
08: parent desktop while the native file chooser is open (WebDriver captures
only the webview, so this is not an image of the chooser itself).

The current recovery logic is validated separately; screenshots establish visual
presentation, not restriction enforcement. Later nonvisual validation-harness
changes do not alter these screens.
