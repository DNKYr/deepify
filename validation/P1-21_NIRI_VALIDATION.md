# P1-21 Niri Feasibility Validation

Date: 2026-08-25

## Environment

- Niri 26.04 (Nixpkgs)
- Wayland session with `WAYLAND_DISPLAY=wayland-1`
- Noctalia Shell revision `fe6fa12`, `noctalia-qs` 0.0.12

## Results

| Requirement | Evidence | Result |
| --- | --- | --- |
| Query all relevant open windows | `niri msg -j windows` returned windows across focused and unfocused workspaces, including `id`, `title`, `app_id`, `pid`, workspace, focus, floating, urgent, and layout fields. | Pass |
| Read stable Wayland app IDs | The inventory returned `Alacritty`, `zen-beta`, `dev.zed.Zed`, and `steam`. Two test windows created through one Alacritty process both retained `app_id=p1-21-multi`. | Pass |
| Detect newly created windows | `niri msg -j event-stream` emitted `WindowOpenedOrChanged` for the disposable `p1-21-test` window, with its ID, app ID, PID, workspace, and state. | Pass |
| Detect focus changes | Focusing window 5 and restoring window 3 emitted `WindowFocusChanged` events for IDs 5 and 3, plus `WorkspaceActivated` events. | Pass |
| Request a window close safely | `niri msg action close-window --id 12` closed a disposable Alacritty window. The request-to-absence measurement was 42.449 ms. | Pass |
| Determine whether focus can be prevented or reversed | Niri has no focus-prevention action. Focus can be reversed with `niri msg action focus-window --id <ID>`; closing the test window returned focus to the prior terminal. | Reversal only |
| Receive updates without excessive polling | One persistent event-stream connection delivered compositor updates. It also emitted repeated `WindowOpenedOrChanged` events while the terminal title changed, so consumers must deduplicate by window ID and relevant fields. | Pass with deduplication |
| Detect and report loss of Niri IPC | Using a missing socket returned a non-zero status and `Error: error connecting to the niri socket` with `No such file or directory`. | Pass |
| Read and change Noctalia Do Not Disturb state | `noctalia-shell ipc show` exposed `notifications.enableDND`, `notifications.disableDND`, and `state.all`. `state.all` reported `doNotDisturb=false`, then `true` after enable, then `false` after disable. | Pass |
| Restore prior Do Not Disturb state | Failure cleanup restored `false` after a temporary enable. A prior-enabled simulation retained `true` through recovery, and the final cleanup restored the original `false`. | Pass |

## Close-confirmation test

An isolated Zen Browser process and temporary profile loaded a local page with a `beforeunload` handler. After `niri msg action close-window --id 16`, the window remained present after one second, demonstrating that the close request is cooperative and can be held by an application confirmation dialog. The temporary process and profile were then terminated and removed from the compositor inventory.

## Multi-window test

Two disposable Alacritty windows were created through the same Alacritty IPC socket. Niri reported both windows with `app_id=p1-21-multi` and the same PID `102733` (window IDs 13 and 14). Closing each ID independently succeeded.

## Timing and limitations

- The `p1-21-test` launch request produced its matching event in approximately 252 ms in this run. This includes application startup and compositor scheduling, not just monitor processing.
- A blocked application can appear and receive focus before the event is consumed; Niri does not provide process-launch prevention through this IPC interface.
- `close-window` sends a safe close request, not a forced termination. Unsaved-work prompts must remain visible and be reported as an unresolved restriction until the user closes them.
- The event stream reports compositor-visible windows. The prototype did not create a hidden or scratchpad-specific fixture, so those cases remain dependent on what the running Niri version exposes through `windows` and `event-stream`.
- A user-level focus app cannot guarantee focus lock against the same desktop account. It can immediately refocus the last allowed window when that window still exists.

## Recommended MVP enforcement

1. Before a session, query `niri msg -j windows`, classify every returned window by `app_id`, and block session start until non-whitelisted windows are closed.
2. During a session, keep one `event-stream` connection. On a new or changed non-whitelisted window, deduplicate the event, record the blocked attempt, request `close-window --id <ID>`, and re-query the window list.
3. If the window remains, show a persistent warning requiring manual closure. Do not terminate the process or repeatedly issue close requests.
4. Store the last allowed window ID and use `focus-window --id <ID>` for best-effort focus recovery after a blocked window opens or closes.
5. Treat a non-zero event-stream read or Niri IPC error as a restriction-component failure: end the session, remove browser restrictions, restore Noctalia state, and report the failure.
6. At session start, call `noctalia-shell ipc call state all`, store `state.doNotDisturb`, call `notifications enableDND`, and restore the stored value through `enableDND` or `disableDND` on completion, failure, and startup recovery.

