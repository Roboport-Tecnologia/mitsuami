//! The app's `GApplication`: what GNOME apps send notifications from and
//! keep to one instance with. mitsuami runs no `GtkApplication`, so this
//! one is only registered on the session bus, where GNOME Shell and later
//! launches find it; GLib's loop, which mitsuami already runs, delivers
//! its calls. A process has one, as `g_application_get_default` has it:
//! `run` registers it for a single-instance app, and notifications on
//! first use otherwise.
//!
//! A later launch of a single-instance app doesn't go through GIO's own
//! remote calls, which are on `org.gtk.Application`: it calls
//! `org.freedesktop.Application.Activate` itself, which Kirigami's
//! backend exports too, so either build of an app finds the other. Its
//! arguments and folder go in the platform data, next to the activation
//! token.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use mitsuami_core::services::Reopen;

use crate::backend::GtkHandle;

/// The action a notification's click runs, with its id: `app.` + this.
pub(crate) const NOTIFICATION_ACTION: &str = "mitsuami-notification";

/// The platform data keys a later launch sends its arguments (`as`) and
/// working folder (`s`) in. Kirigami's backend reads and sends the same.
const ARGS: &str = "mitsuami-args";
const CWD: &str = "mitsuami-cwd";

thread_local! {
    /// The app's id and its application, once registered; `None` if it
    /// couldn't be (no session bus, or another process has the id).
    static APP: RefCell<Option<(String, Option<MitsuamiApp>)>> = const { RefCell::new(None) };
}

/// The registered application for this id, made on first use.
pub(crate) fn app(id: &str) -> Option<MitsuamiApp> {
    APP.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some((known, app)) = slot.as_ref()
            && known == id
        {
            return app.clone();
        }
        let app = match register(id) {
            Registered::Primary(app) => Some(app),
            Registered::Remote | Registered::Failed(_) => None,
        };
        *slot = Some((id.to_owned(), app.clone()));
        app
    })
}

enum Registered {
    Primary(MitsuamiApp),
    /// Another process has the id.
    Remote,
    Failed(String),
}

fn register(id: &str) -> Registered {
    if !gio::Application::id_is_valid(id) {
        return Registered::Failed(format!("{id:?} isn't a valid application id"));
    }
    let app: MitsuamiApp = glib::Object::builder()
        .property("application-id", id)
        // `Open`, from a D-Bus activated `.desktop` file's `%F`.
        .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let action = gio::SimpleAction::new(NOTIFICATION_ACTION, Some(glib::VariantTy::STRING));
    let weak = app.downgrade();
    action.connect_activate(move |_, id| {
        let (Some(app), Some(id)) = (weak.upgrade(), id.and_then(|id| id.str())) else { return };
        app.imp().raise();
        let activated = app.imp().activated.borrow().clone();
        if let Some(activated) = activated {
            activated(id.to_owned());
        }
    });
    app.add_action(&action);
    let registered = app.register(None::<&gio::Cancellable>);
    // Kept for the whole process, as `g_application_run` keeps its app:
    // GIO has no public unregister, so a registered app finalized at exit
    // (the thread-local's destructor) unregisters from D-Bus inside
    // `finalize`, through the subclass's `dbus_unregister`, which gtk-rs
    // can't run on an object whose last reference is gone.
    if app.is_registered() {
        std::mem::forget(app.clone());
    }
    match registered {
        Ok(()) if app.is_remote() => Registered::Remote,
        Ok(()) => Registered::Primary(app),
        Err(error) => Registered::Failed(error.to_string()),
    }
}

/// What `run` does first for a single-instance app: registers its
/// application, and in a later launch hands the launch to the running one.
/// Returns whether this process should go on and run the app.
///
/// Whether another process has the id is asked of the bus first: GIO's
/// registration of a remote instance reads the primary's actions
/// (`org.gtk.Actions`), which Kirigami's doesn't export, and fails then.
pub(crate) fn claim(id: &str) -> bool {
    let bus = match gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        Ok(bus) => bus,
        Err(error) => {
            eprintln!("mitsuami: {id} runs as its own instance, since it can't look for others: {error}");
            APP.with(|slot| *slot.borrow_mut() = Some((id.to_owned(), None)));
            return true;
        }
    };
    if has_owner(&bus, id) {
        hand_over_or_warn(&bus, id);
        return false;
    }
    let app = match register(id) {
        Registered::Primary(app) => Some(app),
        Registered::Remote => {
            hand_over_or_warn(&bus, id);
            return false;
        }
        // Another process took the id meanwhile.
        Registered::Failed(_) if has_owner(&bus, id) => {
            hand_over_or_warn(&bus, id);
            return false;
        }
        Registered::Failed(why) => {
            eprintln!("mitsuami: {id} runs as its own instance, since it can't look for others: {why}");
            None
        }
    };
    APP.with(|slot| *slot.borrow_mut() = Some((id.to_owned(), app)));
    true
}

/// Whether a process owns `name` on the bus.
fn has_owner(bus: &gio::DBusConnection, name: &str) -> bool {
    let reply = bus.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some(&(name,).to_variant()),
        Some(glib::VariantTy::new("(b)").unwrap()),
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
    );
    reply.ok().and_then(|reply| reply.get::<(bool,)>()).is_some_and(|(owned,)| owned)
}

fn hand_over_or_warn(bus: &gio::DBusConnection, id: &str) {
    // The running one is there but didn't take it: running a second
    // instance would do what single instance prevents.
    if let Err(error) = hand_over(bus, id) {
        eprintln!("mitsuami: couldn't hand this launch to the running {id}: {error}");
    }
}

/// Calls the running instance's `org.freedesktop.Application.Activate`
/// with this launch's arguments, folder and activation token, and waits
/// for it to answer.
fn hand_over(bus: &gio::DBusConnection, id: &str) -> Result<(), glib::Error> {
    let data = glib::VariantDict::new(None);
    // As `GtkApplication` sends it from a remote instance.
    if let Some(token) = std::env::var("XDG_ACTIVATION_TOKEN").ok().filter(|t| !t.is_empty()) {
        data.insert("activation-token", token);
    }
    if let Some(id) = std::env::var("DESKTOP_STARTUP_ID").ok().filter(|t| !t.is_empty()) {
        data.insert("desktop-startup-id", id);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    data.insert(ARGS, args);
    if let Ok(cwd) = std::env::current_dir() {
        data.insert(CWD, cwd.to_string_lossy().into_owned());
    }
    bus.call_sync(
        Some(id),
        &object_path(id),
        "org.freedesktop.Application",
        "Activate",
        Some(&glib::Variant::tuple_from_iter([data.end()])),
        None,
        gio::DBusCallFlags::NONE,
        // Long enough for a busy app's loop to come round.
        10_000,
        None::<&gio::Cancellable>,
    )?;
    Ok(())
}

/// The object path GIO exports an application at: its id with `.` as `/`
/// and `-` as `_`.
fn object_path(id: &str) -> String {
    format!("/{}", id.replace('.', "/").replace('-', "_"))
}

/// Connects the registered application, if `run` registered one, to the
/// app: later launches bring its window forward and reach `reopened`.
pub(crate) fn connect(backend: &GtkHandle, reopened: Rc<dyn Fn(Reopen)>) {
    let Some(app) = APP.with(|slot| slot.borrow().as_ref().and_then(|(_, app)| app.clone())) else { return };
    *app.imp().backend.borrow_mut() = Some(backend.downgrade());
    *app.imp().reopened.borrow_mut() = Some(reopened);
}

mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gio, glib};
    use mitsuami_core::services::Reopen;

    use crate::backend::WeakGtkHandle;

    type Activated = Rc<dyn Fn(String)>;
    type Reopened = Rc<dyn Fn(Reopen)>;

    /// What came in the platform data of the call being handled.
    #[derive(Default)]
    struct Platform {
        token: Option<String>,
        args: Vec<String>,
        cwd: Option<PathBuf>,
    }

    #[derive(Default)]
    pub(crate) struct MitsuamiApp {
        pub(crate) activated: RefCell<Option<Activated>>,
        pub(crate) reopened: RefCell<Option<Reopened>>,
        pub(crate) backend: RefCell<Option<WeakGtkHandle>>,
        /// The platform data of the call being handled, while it is.
        platform: RefCell<Platform>,
    }

    impl MitsuamiApp {
        /// Brings the app's window forward as `GtkApplication` does with
        /// the token: on Wayland the compositor only raises a window for a
        /// token from the click or launch, and on X11 it carries its time.
        /// With no window, the token waits for the one the app opens.
        pub(crate) fn raise(&self) {
            let Some(backend) = self.backend.borrow().as_ref().and_then(WeakGtkHandle::upgrade) else { return };
            let token = self.platform.borrow().token.clone();
            match backend.last_active_window() {
                Some(window) => {
                    if let Some(token) = token.as_deref() {
                        window.set_startup_id(token);
                    }
                    window.present();
                }
                None => backend.set_activation_token(token),
            }
        }

        /// A later launch: the window comes forward, then the app hears.
        fn reopen(&self, files: Vec<PathBuf>) {
            self.raise();
            let reopen = {
                let platform = self.platform.borrow();
                Reopen::new(platform.args.clone(), platform.cwd.clone(), files)
            };
            let reopened = self.reopened.borrow().clone();
            if let Some(reopened) = reopened {
                reopened(reopen);
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MitsuamiApp {
        const NAME: &'static str = "MitsuamiApp";
        type Type = super::MitsuamiApp;
        type ParentType = gio::Application;
    }

    impl ObjectImpl for MitsuamiApp {}

    impl ApplicationImpl for MitsuamiApp {
        /// Activated over D-Bus: a later launch, or GNOME Shell launching
        /// an app that's D-Bus activatable again.
        fn activate(&self) {
            self.reopen(Vec::new());
        }

        fn open(&self, files: &[gio::File], _hint: &str) {
            self.reopen(files.iter().filter_map(|file| file.path()).collect());
        }

        /// GIO hands the platform data around an action, `Activate` or
        /// `Open`, as `GtkApplication` reads the activation token from it.
        fn before_emit(&self, platform_data: &glib::Variant) {
            self.parent_before_emit(platform_data);
            let data = glib::VariantDict::new(Some(platform_data));
            let lookup = |key: &str| data.lookup::<String>(key).ok().flatten();
            *self.platform.borrow_mut() = Platform {
                token: lookup("activation-token").or_else(|| lookup("desktop-startup-id")),
                args: data.lookup::<Vec<String>>(super::ARGS).ok().flatten().unwrap_or_default(),
                cwd: lookup(super::CWD).map(PathBuf::from),
            };
        }

        fn after_emit(&self, platform_data: &glib::Variant) {
            self.platform.take();
            self.parent_after_emit(platform_data);
        }
    }
}

glib::wrapper! {
    pub(crate) struct MitsuamiApp(ObjectSubclass<imp::MitsuamiApp>)
        @extends gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}
