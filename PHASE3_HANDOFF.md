# Deepify Phase 3 handoff

## Status

Implementation is complete for the production browser path, with Firefox and
Zen disposable-profile harnesses, local quality gates, deterministic XPI, and
locked Nix package builds passing. Phase 3 implementation and lifecycle
evidence are complete. Signed-XPI installation remains an external release
gate and is not reported as passed.

## Verified behavior

- The desktop broker accepts only same-UID Unix-socket peers, pairs one Firefox
  or Zen profile using a token hash, and never persists raw tokens or URLs.
- The Rust native host forwards bounded, versioned frames between Firefox native
  messaging and the desktop socket. Socket close causes an observable native
  disconnect.
- A paired healthy browser is required before session start. Existing and new
  public non-whitelisted HTTP(S) pages are blocked while Working or Paused.
- The extension retains original tab URLs in memory only and restores surviving
  tabs on normal stop and native disconnect. Blocked-attempt events are
  aggregate, URL-free counts.
- Firefox and Zen disposable-profile harnesses exercised real extension/native-
  host framing, pairing, existing/new restriction, Back/reload, paused
  enforcement, normal restoration, and native-disconnect fail-open restoration.
  See [validation/PHASE3_CHECKS.md](validation/PHASE3_CHECKS.md).
- The validation-only Rust broker harness additionally proves desktop process
  loss restoration, browser close/restart idle reconnect, and startup checkpoint
  recovery in both target browsers.

## Installation

Build `.#browser-native-host` and link its generated
`lib/mozilla/native-messaging-hosts/com.deepify.browser.json` into Firefox's
native-messaging-host lookup directory. Build `.#firefox-extension` for an
unsigned development XPI and install it temporarily in a disposable profile.
The manifest permits only `focus@deepify.local`.

Do not alter a normal browser profile automatically. Production installation
requires a signed XPI. Zen Beta 1.21.15b was validated with the same temporary
`~/.mozilla/native-messaging-hosts/` manifest path as Firefox.

## Explicit boundaries and Phase 4 work

Website protection is real only for the paired, healthy profile. Private
windows and additional profiles are bypasses by design. Niri application
restriction and Noctalia Do Not Disturb are still simulated; Phase 4 should
replace those adapters without changing browser protocol or session rules.

## Remaining release gates

- Install a signed XPI when AMO signing credentials/channel are available.
