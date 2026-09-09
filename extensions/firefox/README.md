# Deepify Firefox/Zen extension

This is the production Phase 3 WebExtension for one explicitly paired Firefox
or Zen profile. It has permanent ID `focus@deepify.local` and may be installed
temporarily only in a disposable development profile until a signed XPI is
available.

Its only broad host permission is `<all_urls>`, which is required to intercept
all top-level public HTTP(S) navigation for a default-deny focus policy. It does
not request cookies, history, downloads, filesystem, or private-window access.

## Native host installation on NixOS

Build `.#browser-native-host`, then expose its generated manifest at the
browser's native-messaging-host lookup directory. For Firefox Home Manager,
link:

```text
<browser-native-host>/lib/mozilla/native-messaging-hosts/com.deepify.browser.json
```

into `~/.mozilla/native-messaging-hosts/`. Do not modify an existing profile
automatically. Zen's target package lookup path must be confirmed on the target
package before release. Zen Beta 1.21.15b was verified with that same
`~/.mozilla/native-messaging-hosts/` path in the disposable Phase 3 harness;
it uses the same `com.deepify.browser.json` file and no additional allowed
extension IDs.

## Privacy and limits

The extension stores only its random pairing token in `storage.local`. Original
tab URLs remain in the background process memory while a focus session is
active, so they can be restored when it ends. URLs, hostnames, titles, query
strings, and fragments never cross native messaging or enter Deepify SQLite.

Private windows and unpaired/additional profiles are intentionally outside this
Phase 3 enforcement boundary. Container tabs in the paired profile share the
same policy and do not require the cookies permission.
