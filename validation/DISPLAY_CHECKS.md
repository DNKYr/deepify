# Linux display validation — 2026-09-20

## Cause and fix

On the target Niri desktop (1920×1200 output, compositor scale 1.25), GTK 3
reported `gtk-xft-dpi=-1` and `GdkScreen.resolution=-1`. GDK documents `-1` as
[an unspecified resolution](https://docs.gtk.org/gdk3/method.Screen.get_resolution.html).
With WebKitGTK 2.52.5, this produced a negative device-pixel ratio and negative
layout dimensions, even in an isolated WebKit window without React or Tauri.

Deepify now initializes GTK before constructing Tauri's webviews, supplies
[GDK's default 96 DPI](https://docs.gtk.org/gdk3/method.Screen.set_resolution.html)
only for an invalid/unspecified screen resolution, and repeats that check on
resolution notifications. Valid positive DPI changes and the monitor scale are
preserved. The adjustment is process-local; it does not write desktop settings.

The original CSS widths and viewport meta tag are retained. The experimental
width workaround, viewport removal, and on-screen layout probe were removed.

## Runtime evidence

An ephemeral WebKitGTK window using the same libraries, display, HTML, and
`width=device-width` meta tag produced:

| Measurement | Original GTK resolution | Process-local 96 DPI fallback |
| --- | --- | --- |
| GTK screen resolution | -1 | 96 |
| GTK widget scale / page zoom | 2 / 1 | 2 / 1 |
| JavaScript devicePixelRatio | -0.02083333395421505 | 2 |
| JavaScript viewport | -69216×-87744 | 721×914 |
| Shell width | 0 | 681 |

The actual desktop was then launched using `nix develop -c npm run dev` on
the same Niri session. Its readout measured a **972×685** viewport, **DPR 2**,
and **932px** shell. A window-only screenshot confirmed readable navigation,
headings, form fields, and controls. The native window remained 972×732 including
decorations. No forced X11 backend or global scale override was needed.

## Regression check

The real GTK regression test exercises initial unset DPI, subsequent valid
120/144/192 DPI changes, and later resets to -1 or 0. It is ignored in headless
test runs because it needs a display; all changes stay within the test process.

```sh
nix develop -c cargo test -p deepify-desktop --lib display::tests -- --ignored --test-threads=1
```

Result: PASS on the target desktop.

Also passed: `npm test`, frontend typecheck/lint/format/build, Rust formatting,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace --all-targets` (40 passed; display and audio tests skipped
by default, with the display test run explicitly above). The final window was
captured again after removing the temporary probe and retained the correct layout.
