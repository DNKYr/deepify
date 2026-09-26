//! Normalize an unspecified GTK font DPI before WebKit creates its first page.

use gtk::gdk;

fn ensure_valid_resolution(screen: &gdk::Screen) {
    let resolution = screen.resolution();
    if !resolution.is_finite() || resolution <= 0.0 {
        // GDK uses -1 for "unspecified". WebKitGTK 2.52.5 can use this
        // sentinel as a real DPI, yielding a negative devicePixelRatio and
        // viewport. 96 is GDK's documented default; the monitor scale is
        // applied separately. This changes only the current process.
        screen.set_resolution(96.0);
    }
}

/// Call on the main thread before constructing Tauri's webviews.
pub fn initialize() -> Result<(), gtk::glib::BoolError> {
    gtk::init()?;
    let screen = gdk::Screen::default()
        .ok_or_else(|| gtk::glib::bool_error!("GTK did not provide a default screen"))?;
    ensure_valid_resolution(&screen);
    // Keep valid user DPI/accessibility changes, but also handle a settings
    // provider disappearing and resetting the value to "unspecified".
    screen.connect_resolution_notify(ensure_valid_resolution);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a live GTK display; run with --ignored --test-threads=1"]
    fn gtk_missing_dpi_recovers_and_valid_dpi_changes_are_preserved() {
        gtk::init().unwrap();
        let screen = gdk::Screen::default().unwrap();
        screen.set_resolution(-1.0);
        initialize().unwrap();
        assert_eq!(screen.resolution(), 96.0);

        for resolution in [120.0, 144.0, 192.0] {
            screen.set_resolution(resolution);
            assert_eq!(screen.resolution(), resolution);
        }
        for resolution in [-1.0, 0.0] {
            screen.set_resolution(resolution);
            assert_eq!(screen.resolution(), 96.0);
        }
    }
}
