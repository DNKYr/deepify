# Phase 2 visual-state evidence

Images 01–13 are captured from the production React components through the
development-only `?demo=` snapshot provider. Image 14 is the packaged extension
blocked-page preview. The set covers setup, idle, validation, blocked-app
resolution, Working, Paused, early-end confirmation, successful and failed
summaries, Sound Library, Whitelist, Settings, Session History, and browser
blocked-page UI.

Restriction behavior remains simulated. The preview does not claim live browser
interception or restoration.

To regenerate the 1440×1000 images, serve the built frontend on
`127.0.0.1:4173`, then run:

```sh
node scripts/capture-phase2-screens.mjs
```
