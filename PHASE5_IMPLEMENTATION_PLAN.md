# Deepify — Phase 5 refinement and release

Status: in progress after the Phase 4 production integration work. Scope remains
the full MVP in DESIGN.md and the P1-24 acceptance checklist; passing a subset
does not complete this phase.

| Item | Required outcome | Status |
| --- | --- | --- |
| P5-01 | Monotonic timer, suspend/clock-change detection, planned exit/shutdown cleanup, abandoned-session recovery and single-instance verification | Implemented; actual desktop and injected lifecycle matrix pass |
| P5-02 | Real audio-device failure/retry, missing-file behavior, ordered folder reconciliation and safe recursive scanning | Implemented; real default output and isolated PipeWire loss/retry pass |
| P5-03 | Keyboard/focus/accessibility review, accurate recovery diagnostics and current browser regression scenarios | Verified; see Phase 5 validation |
| P5-04 | Privacy/security/performance audit and reproducible installable Nix artifacts | In progress |
| P5-05 | Requirement-by-requirement P1-24 audit, final handoff, external signed-XPI install gate | Audit and handoff drafted; consent/counting decision and signed install open |

## Findings addressed in the current implementation

- `SystemClock` still used wall-clock seconds despite the monotonic architecture
  requirement. Clock and suspend events must interrupt without inflating focus time.
- Planned desktop exit did not invoke the session cleanup coordinator.
- The audio adapter had no stream-error callback feeding its visible waiting state.
- Recursive audio scanning followed directory symlinks and queue reconciliation
  sorted surviving entries instead of preserving their order.
- Complete acceptance must include actual combined browser/Niri/Noctalia scenarios,
  not infer them solely from individual adapters or historic validation documents.

Hardware-destructive checks such as suspending or rebooting the user's live
desktop must use isolated event injection or a disposable environment; do not
interrupt the user's machine just to satisfy a validation checkbox. Distinguish
such evidence from real target-machine sleep/reboot validation.

Current evidence: [validation/PHASE5_CHECKS.md](validation/PHASE5_CHECKS.md). The full outstanding acceptance map is [validation/MVP_ACCEPTANCE.md](validation/MVP_ACCEPTANCE.md).
