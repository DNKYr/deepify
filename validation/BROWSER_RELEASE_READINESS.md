# Browser release readiness

The production extension and native host pass current Firefox 156.0 and Zen
1.22.3b disposable-profile checks, including packaged-XPI installation in
**temporary** mode. The XPI is unsigned. Permanent signed installation has not
been claimed as passing.

A 2026-09-25 `web-ext lint --warnings-as-errors` run reported no code errors but
one release warning: `MISSING_DATA_COLLECTION_PERMISSIONS`. This is a real
release-readiness gap, not a reason to declare that the extension transmits no data.

Mozilla requires new submissions to declare data categories, and its guidance
explicitly includes data sent through native messaging. Deepify sends a random
local pairing credential, browser kind, health/state responses and aggregate
blocked/restored counts to its local desktop. It sends no destinations, browsing
history or website content, and has no remote service.

Sources checked on 2026-09-25:

- [Built-in data consent](https://extensionworkshop.com/documentation/develop/firefox-builtin-data-consent/): required declarations for new submissions; technical/interaction data must be optional.
- [Native messaging and consent](https://extensionworkshop.com/documentation/develop/best-practices-for-collecting-user-data-consents/): local native application transfers must be declared.
- [Add-on policy](https://extensionworkshop.com/documentation/publish/add-on-policies/): consent and control requirements apply to native messaging.

## Concrete proposed release behavior

Keep all blocking, pause, restoration and recovery behavior. Declare the local
pairing credential accurately. Make browser attempt counts and browser-identifying
metadata optional, with a clear “Browser counts not shared” state instead of
silently presenting an unavailable count as zero. Store each session's counting
mode so history remains interpretable. Declining these optional metrics must not
weaken protection or restoration. Operational request/reply data needs an explicit
classification in the signing review; it must not be mislabeled as remote analytics.

For older Firefox versions, preserve the existing minimum version with a clear
extension-owned consent page; alternatively raise the supported minimum to 140
and use Firefox's built-in experience. No minimum-version or counting-policy
change has yet been made. These choices affect the approved product boundary and
should be resolved with the release owner before changing it.

After that decision: implement and test consent/revocation, rerun the production
browser matrix and Mozilla lint, build the final XPI, then sign using the selected
channel. The harness supports `DEEPIFY_EXTENSION_PATH=/absolute/signed.xpi` and
`DEEPIFY_SIGNED_INSTALL=1`, which requests permanent installation with signature
enforcement enabled in a disposable profile. No signing credentials are stored
in this repository, and no extension has been submitted or published.
