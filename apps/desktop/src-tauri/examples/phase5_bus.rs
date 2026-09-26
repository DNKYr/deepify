//! Private-bus fixture for desktop lifecycle and notification validation.
//! Run only inside dbus-run-session; never owns names on the user's buses.
use glib::variant::ToVariant;
use gtk::{gio, glib};
use std::io::{BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        std::env::var("DEEPIFY_PRIVATE_BUS_FIXTURE").as_deref(),
        Ok("1")
    );
    let bus = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)?;
    for name in ["org.freedesktop.login1", "org.freedesktop.Notifications"] {
        let result = bus.call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "RequestName",
            Some(&(name, 4u32).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            1000,
            None::<&gio::Cancellable>,
        )?;
        assert_eq!(result.get::<(u32,)>(), Some((1,)));
    }
    let node = gio::DBusNodeInfo::for_xml(
        r#"<node><interface name="org.freedesktop.Notifications">
      <method name="GetCapabilities"><arg type="as" direction="out"/></method>
      <method name="GetServerInformation"><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/></method>
      <method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
      <method name="CloseNotification"><arg type="u" direction="in"/></method>
    </interface></node>"#,
    )?;
    let _registration = bus.register_object(
        "/org/freedesktop/Notifications",
        &node
            .lookup_interface("org.freedesktop.Notifications")
            .unwrap(),
        |_, _, _, _, method, _, invocation| {
            let result = match method {
                "GetCapabilities" => (vec!["body"],).to_variant(),
                "GetServerInformation" => ("Deepify fixture", "Deepify", "1", "1.2").to_variant(),
                "Notify" => {
                    println!("PHASE5_NOTIFICATION");
                    (1u32,).to_variant()
                }
                _ => ().to_variant(),
            };
            invocation.return_value(Some(&result));
        },
        |_, _, _, _, _| ().to_variant(),
        |_, _, _, _, _, _| false,
    )?;
    let emitter = bus.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            let member = match line.as_str() {
                "sleep" => "PrepareForSleep",
                "shutdown" => "PrepareForShutdown",
                _ => continue,
            };
            emitter
                .emit_signal(
                    None,
                    "/org/freedesktop/login1",
                    "org.freedesktop.login1.Manager",
                    member,
                    Some(&(true,).to_variant()),
                )
                .unwrap();
            emitter.flush_sync(None::<&gio::Cancellable>).unwrap();
        }
    });
    println!("PHASE5_BUS_READY");
    std::io::stdout().flush()?;
    glib::MainLoop::new(None, false).run();
    Ok(())
}
