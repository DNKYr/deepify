# Phase 4 handoff

The Niri/Noctalia production integration is implemented and verified on the target
NixOS/Niri desktop. Phase 5 owns final release packaging and the signed-browser
installation gate. Historical Phase 2/3 evidence remains unchanged.

Niri preflight inspects every reported window by exact Wayland app ID. New
blocked windows receive one targeted cooperative close request; repeated events
do not repeat it. Refusal stays visible and blocks the next start. Missing IDs
are explicitly allowed and labeled; terminal children and window titles are not
used. A guarded live test verified restoration of focus to the last allowed
fixture window. No existing user window was targeted by validation.

Noctalia's prior DND boolean is saved in SQLite before enabling it, and restoration
is verified before clearing the checkpoint. Completion, early end, runtime failure,
exit and startup recovery use the same cleanup coordinator. Failed cleanup stays
visible and retryable in both the summary and history.

The target supports `noctalia-shell ipc call state all` and
`ipc call notifications enableDND/disableDND`. The verified version is the legacy
`fe6fa12` shell, with Niri 26.04. Other CLI variants report integration failure.
The actual native file chooser uses `com.deepify.desktop`, already implicitly
allowed. Shell, portal and authentication allowances are exact identifiers; no
arbitrary `org.*` or executable-prefix exemption is used.

Evidence is in [validation/PHASE4_CHECKS.md](validation/PHASE4_CHECKS.md) and
[validation/PHASE5_CHECKS.md](validation/PHASE5_CHECKS.md): real close/refusal,
multiple-window and focus scenarios; prior-DND false/true restoration; combined
browser/Niri/Noctalia sessions; and actual desktop monitor/DND failure and recovery.
The earlier Phase 4 source snapshot built successfully. The current Phase 5
package has its own final build/install verification and is not inferred from it.
