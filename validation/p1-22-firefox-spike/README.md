# P1-22 Firefox extension feasibility spike

This is a disposable prototype, not production code. It exercises the browser-side contract needed by the Focus Room:

- enumerate tabs and report URL, private-window state, and `cookieStoreId`;
- redirect non-whitelisted HTTP(S) tabs to a blocked page while retaining the original URL;
- exercise a diagnostic restore action and restore all surviving blocked tabs on stop (the manual action is spike-only and must not ship while an MVP session is active);
- keep a persistent Firefox native-messaging connection and exchange health/heartbeat messages;
- restore tabs when the native host disconnects.

The extension is deliberately `incognito: "not_allowed"`. Firefox therefore does not expose private-window tabs to it; private windows are an intentional MVP bypass. Firefox runtime validation confirmed that a private-window URL remained unrestricted.

## Local test

The harness creates a new profile under `/tmp`, installs the extension temporarily through Firefox Marionette, and uses the native host manifest in that same profile. It does not modify the user's Firefox or Zen profiles.

```sh
node test/marionette-test.js
```

The test requires the `firefox` executable and a graphical or headless Firefox session. Set `FIREFOX_BIN` to override the executable. The harness writes its native-host log under `/tmp`.

The validated runs on 2026-08-26 were:

- Firefox 154.0: start/stop PASS, native-disconnect PASS, private-window bypass PASS.
- Zen Beta 1.21.15b: start/stop PASS and native-disconnect PASS. The Zen Marionette probe
  cannot create a private window because Zen reports `openWindow() not supported`; this is a
  test-automation limitation that still requires manual Zen private-window validation.

## Installation findings

For a real release, publish a signed XPI through Firefox Add-ons. A local unsigned XPI is only suitable for temporary development installation; it is not an end-user installation path.

For NixOS/Home Manager, package the native host and its host manifest with
`programs.firefox.nativeMessagingHosts.packages`. The manifest must contain the fixed
extension ID in `allowed_extensions`; the browser extension and native host are separate
installables. The checked-in Nix expression is
[`nixos/native-messaging-host.nix`](nixos/native-messaging-host.nix).

Zen Browser uses a separate profile root on this machine (`~/.config/zen`). The installed Zen
Beta 1.21.15b executable passed the core WebExtension/native-messaging restriction and restore
scenarios. Packaging should still verify the native-host manifest location for the target Zen
package; the harness writes both the conventional `~/.mozilla/native-messaging-hosts` path and
the XDG Mozilla path in its disposable home.

## Open limitations

- Native messaging starts the desktop-side host as a browser child process; a Tauri app must implement the same length-prefixed JSON stdio protocol or delegate it to a helper. The spike does not pretend that Node is Tauri.
- A native-host EOF/disconnect is observable immediately by the host, and the extension's `runtime.Port.onDisconnect` is used for local fail-open restoration. The desktop app should still retain the one-minute health heartbeat as a second line of defense.
- Container tabs expose `cookieStoreId` in the tab inventory and share one extension/native connection within a profile. Creating a tab in a container requires the `cookies` permission; the spike intentionally omits it and observed `No permission for cookieStoreId`. There is no per-container whitelist policy, so containers are treated as part of the one configured profile.
- Private windows are unsupported and intentionally bypass restrictions.
- The profile pairing token is generated in `storage.local` and sent in `hello`. The
  production desktop app must enforce the pairing token; the native-host manifest alone only
  restricts which extension ID may connect, not which browser profile is connecting.
