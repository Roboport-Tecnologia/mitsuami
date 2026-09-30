//! The session ending (logging out, shutting down), asked of the app as its
//! Quit is: `Ui::request_quit`, and if windows are left, the session's end
//! is held the way GNOME holds it for an app that asks first. The routes
//! are the ones `GtkApplication` takes, without it: GNOME's session manager
//! (a client registered on it, answering `QueryEndSession`), or in a
//! Flatpak sandbox, where that isn't reachable, the portal's session
//! monitor with a logout inhibit. Elsewhere (no GNOME session) nothing
//! answers, and the session ends as it would have.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

/// Asks the app to quit, as its Quit does; returns whether it did (no
/// window is left).
pub(crate) type Quit = Rc<dyn Fn() -> bool>;

/// What gnome-session and the portal show for an app holding the end, in
/// the app's language.
fn reason() -> String {
    mitsuami_core::l10n::tr("mitsuami-quit-reason", &[])
}

const SM: &str = "org.gnome.SessionManager";
const SM_PATH: &str = "/org/gnome/SessionManager";
const SM_CLIENT: &str = "org.gnome.SessionManager.ClientPrivate";

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const INHIBIT: &str = "org.freedesktop.portal.Inhibit";

/// The session's end, watched for the app's run; dropping it stops.
pub(crate) struct Session {
    _subscriptions: Rc<RefCell<Vec<gio::SignalSubscription>>>,
}

/// `quit` asks the app; `stop` ends the run once the session tells the app
/// to go.
pub(crate) fn watch(quit: Quit, stop: Rc<dyn Fn()>) -> Session {
    let subscriptions: Rc<RefCell<Vec<gio::SignalSubscription>>> = Rc::default();
    let subs = subscriptions.clone();
    let sandboxed = std::path::Path::new("/.flatpak-info").exists();
    gio::bus_get(gio::BusType::Session, None::<&gio::Cancellable>, move |bus| {
        let Ok(bus) = bus else { return };
        if sandboxed { portal(bus, quit, subs) } else { gnome_session(bus, quit, stop, subs) }
    });
    Session { _subscriptions: subscriptions }
}

fn call(bus: &gio::DBusConnection, name: &str, path: &str, interface: &str, method: &str, args: glib::Variant) {
    bus.call(
        Some(name),
        path,
        interface,
        method,
        Some(&args),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
        |_| {},
    );
}

/// A client of gnome-session: `QueryEndSession` asks the app, and answers
/// no, with a reason, if it kept windows; gnome-session then lists the app
/// in its log out dialog, with "Log Out Anyway". `Stop` ends the run.
fn gnome_session(
    bus: gio::DBusConnection,
    quit: Quit,
    stop: Rc<dyn Fn()>,
    subs: Rc<RefCell<Vec<gio::SignalSubscription>>>,
) {
    let app_id = glib::prgname().map_or_else(|| "mitsuami".to_owned(), |name| name.to_string());
    // Given to the process by gnome-session when it autostarts it; not for
    // the processes this one starts.
    let startup_id = std::env::var("DESKTOP_AUTOSTART_ID").unwrap_or_default();
    // SAFETY: at startup, before the app starts threads of its own.
    unsafe { std::env::remove_var("DESKTOP_AUTOSTART_ID") };
    let b = bus.clone();
    bus.call(
        Some(SM),
        SM_PATH,
        SM,
        "RegisterClient",
        Some(&(app_id, startup_id).to_variant()),
        glib::VariantTy::new("(o)").ok(),
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
        move |reply| {
            // No GNOME session: nothing to register with.
            let Some(client) = reply.ok().and_then(|r| r.child_value(0).str().map(str::to_owned)) else { return };
            let bus = b.clone();
            let path = client.clone();
            let subscription = b.subscribe_to_signal(
                Some(SM),
                Some(SM_CLIENT),
                None,
                Some(&client),
                None,
                gio::DBusSignalFlags::NONE,
                move |signal| {
                    let respond = |ok: bool, reason: &str| {
                        call(&bus, SM, &path, SM_CLIENT, "EndSessionResponse", (ok, reason).to_variant())
                    };
                    match signal.signal_name {
                        "QueryEndSession" => {
                            let ok = quit();
                            respond(ok, &if ok { String::new() } else { reason() });
                        }
                        // The session ends whatever the app says now.
                        "EndSession" => respond(true, ""),
                        "Stop" => stop(),
                        _ => {}
                    }
                },
            );
            subs.borrow_mut().push(subscription);
        },
    );
}

/// The portal's session monitor: at `query-end` it asks the app, and holds
/// a logout inhibit while the app keeps windows, until the session goes
/// back to running (the user cancelled).
fn portal(bus: gio::DBusConnection, quit: Quit, subs: Rc<RefCell<Vec<gio::SignalSubscription>>>) {
    // Request and session handles are made from our unique name and a token.
    let Some(sender) = bus.unique_name().map(|n| n.trim_start_matches(':').replace('.', "_")) else { return };
    let session = format!("{PORTAL_PATH}/session/{sender}/mitsuami_session");
    let options = |pairs: &[(&str, &str)]| {
        let dict = glib::VariantDict::new(None);
        for (key, value) in pairs {
            dict.insert(key, value);
        }
        dict.end()
    };
    let held: Rc<RefCell<Option<String>>> = Rc::default();
    let (b, s) = (bus.clone(), session.clone());
    let subscription = bus.subscribe_to_signal(
        Some(PORTAL),
        Some(INHIBIT),
        Some("StateChanged"),
        Some(PORTAL_PATH),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            if signal.parameters.child_value(0).str() != Some(s.as_str()) {
                return;
            }
            let state = glib::VariantDict::new(Some(&signal.parameters.child_value(1)));
            match state.lookup::<u32>("session-state").ok().flatten() {
                // Query end: ask, hold the end if windows are left, then
                // say the app has answered.
                Some(2) => {
                    if !quit() && held.borrow().is_none() {
                        let args = glib::Variant::tuple_from_iter([
                            "".to_variant(),
                            // Logout.
                            1u32.to_variant(),
                            options(&[("reason", &reason()), ("handle_token", "mitsuami_inhibit")]),
                        ]);
                        let request = format!("{PORTAL_PATH}/request/{sender}/mitsuami_inhibit");
                        call(&b, PORTAL, PORTAL_PATH, INHIBIT, "Inhibit", args);
                        *held.borrow_mut() = Some(request);
                    }
                    let args = glib::Variant::tuple_from_iter([glib::variant::ObjectPath::try_from(s.clone())
                        .map(|p| p.to_variant())
                        .unwrap_or_else(|_| s.to_variant())]);
                    call(&b, PORTAL, PORTAL_PATH, INHIBIT, "QueryEndResponse", args);
                }
                // Running again: the user cancelled; let go.
                Some(1) => {
                    if let Some(request) = held.borrow_mut().take() {
                        call(&b, PORTAL, &request, "org.freedesktop.portal.Request", "Close", ().to_variant());
                    }
                }
                _ => {}
            }
        },
    );
    subs.borrow_mut().push(subscription);
    let args = glib::Variant::tuple_from_iter([
        "".to_variant(),
        options(&[("handle_token", "mitsuami_monitor"), ("session_handle_token", "mitsuami_session")]),
    ]);
    call(&bus, PORTAL, PORTAL_PATH, INHIBIT, "CreateMonitor", args);
}
