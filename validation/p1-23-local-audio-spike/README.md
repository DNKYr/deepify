# P1-23 local audio feasibility spike

This is a disposable validation harness for the local MP3 requirements in P1-19 and
P1-23. It uses only Node's standard library for the library model and the installed
FFmpeg/VLC native audio stack for an actual MP3 decode/playback smoke test. It does not claim to be
the production Tauri player.

Run it from this directory:

```sh
npm test
```

The test creates short MP3 fixtures in `/tmp`, including one with ID3v2 tags and one
whose metadata comes from `Artist - Title.mp3`. It then validates individual-file
selection, recursive folder selection, JSON persistence/restart, live folder rescans,
and deterministic queue reconciliation:

- ID3 title/artist/album tags win when present.
- Without tags, `Artist - Title.mp3` supplies artist and title; otherwise the filename
  stem is the title.
- Folder playlists recursively include `.mp3` files and sort by normalized relative
  path. Non-MP3 files are ignored.
- The library stores absolute direct-file and folder paths, not copied audio data.
- A rescan retains queue entries whose paths still exist, removes deleted paths, and
  appends newly discovered paths in lexical order. A move or rename is a new path and
  therefore appends; this avoids guessing whether it is the same track.
- A missing individually imported file remains a missing reference rather than being
  silently copied or recreated.

## Validation result

Validated on 2026-08-26 in the NixOS development environment:

| P1-23 requirement | Result | Evidence |
| --- | --- | --- |
| Select individual files | PASS | `test.js` creates and resolves a direct-file reference |
| Select directories | PASS | `test.js` recursively scans a live folder reference |
| Play MP3 files | PASS | FFmpeg creates the fixture; VLC plays the tagged MP3 with `--aout dummy` |
| Read metadata and filename fallback | PASS | ID3v2.3 text frames and `Composer - Filename title.mp3` assertions |
| Retain references across restart | PASS | JSON serialize/parse and re-resolve assertions |
| Rescan add/move/rename/delete | PASS | Filesystem mutations followed by a fresh folder scan |
| Persist queue and define source changes | PASS | `reconcileQueue` assertions and documented policy above |
| Handle output-device changes | PARTIAL | Device-loss/recovery state contract passes; no PipeWire output device is available in this headless run |

The native playback smoke test demonstrates that MP3 decoding is available on the
target machine, but not that a Tauri WebView or a Rust audio crate is the final choice.
Implementation should use the Tauri file dialog for selection, persist the references
and queue in the app's local store, and keep playback behind an adapter. The adapter
must expose output-device loss and re-selection as an explicit health state. A real
device-switch test remains an implementation/release test on a running PipeWire
session.
