//! Notifications, as GNOME apps send them: a `GNotification` from a
//! `GApplication` with the app's id. GIO hands it to GNOME Shell, to the
//! portal in a sandbox, or to the freedesktop notification server
//! elsewhere. mitsuami runs no `GtkApplication`, so this one is only
//! registered on the session bus, where the shell finds it when a
//! notification is clicked; GLib's loop, which mitsuami already runs,
//! delivers the click. GIO doesn't say whether the user turned the app's
//! notifications off, so it never answers `Cancelled`: GNOME Shell drops
//! them, as it does every GNOME app's.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use mitsuami_core::services::{Notification, ServiceError};

use crate::backend::GtkHandle;

/// The action a notification's click runs, with its id: `app.` + this.
const ACTION: &str = "mitsuami-notification";

thread_local! {
    /// The app's id and its application, once registered; `None` if it
    /// couldn't be (no session bus, or another process has the id).
    static APP: RefCell<Option<(String, Option<NotifyingApp>)>> = const { RefCell::new(None) };
}

/// The registered application for this id, made on first use: GIO
/// sends a process's notifications from one application, as
/// `g_application_get_default` has it.
fn app(id: &str) -> Option<NotifyingApp> {
    APP.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some((known, app)) = slot.as_ref()
            && known == id
        {
            return app.clone();
        }
        let app = register(id);
        *slot = Some((id.to_owned(), app.clone()));
        app
    })
}

fn register(id: &str) -> Option<NotifyingApp> {
    if !gio::Application::id_is_valid(id) {
        return None;
    }
    let app: NotifyingApp = glib::Object::builder()
        .property("application-id", id)
        .property("flags", gio::ApplicationFlags::empty())
        .build();
    let action = gio::SimpleAction::new(ACTION, Some(glib::VariantTy::STRING));
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
    // A remote one would only forward to the process that has the id:
    // GIO sends notifications only from the primary instance.
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
        Ok(()) if !app.is_remote() => Some(app),
        _ => None,
    }
}

pub(crate) fn notify(
    backend: &GtkHandle,
    notification: &Notification,
    activated: Rc<dyn Fn(String)>,
) -> Result<(), ServiceError> {
    let Some(app_id) = backend.app_id() else { return Err(ServiceError::Unavailable) };
    let Some(app) = app(&app_id) else { return Err(ServiceError::Unavailable) };
    *app.imp().activated.borrow_mut() = Some(activated);
    *app.imp().backend.borrow_mut() = Some(backend.downgrade());
    let id = notification.id.clone().unwrap_or_default();
    let shown = gio::Notification::new(&notification.title);
    shown.set_body(notification.body.as_deref());
    shown.set_default_action_and_target_value(&format!("app.{ACTION}"), Some(&id.to_variant()));
    app.send_notification(Some(&id), &shown);
    Ok(())
}

pub(crate) fn withdraw(backend: &GtkHandle, id: &str) {
    if let Some(app) = backend.app_id().and_then(|app_id| app(&app_id)) {
        app.withdraw_notification(id);
    }
}

mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gio, glib};

    use crate::backend::WeakGtkHandle;

    type Activated = Rc<dyn Fn(String)>;

    #[derive(Default)]
    pub(crate) struct NotifyingApp {
        pub(super) activated: RefCell<Option<Activated>>,
        pub(super) backend: RefCell<Option<WeakGtkHandle>>,
        /// The token the shell gave with the click, while it's handled.
        token: RefCell<Option<String>>,
    }

    impl NotifyingApp {
        /// Brings the app's window forward as `GtkApplication` does with
        /// the shell's token: on Wayland the compositor only raises a
        /// window for a token from the click, and on X11 it carries the
        /// click's time.
        pub(super) fn raise(&self) {
            let backend = self.backend.borrow().as_ref().and_then(WeakGtkHandle::upgrade);
            let Some(window) = backend.and_then(|backend| backend.last_active_window()) else { return };
            if let Some(token) = self.token.borrow().as_deref() {
                window.set_startup_id(token);
            }
            window.present();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NotifyingApp {
        const NAME: &'static str = "MitsuamiNotifyingApp";
        type Type = super::NotifyingApp;
        type ParentType = gio::Application;
    }

    impl ObjectImpl for NotifyingApp {}

    impl ApplicationImpl for NotifyingApp {
        /// Activated over D-Bus, as GNOME Shell does an app that's D-Bus
        /// activatable when it's launched again: GTK apps come forward.
        fn activate(&self) {
            self.raise();
        }

        /// GIO hands the activation token in the platform data around an
        /// action or `activate`, as `GtkApplication` reads it.
        fn before_emit(&self, platform_data: &glib::Variant) {
            self.parent_before_emit(platform_data);
            let data = glib::VariantDict::new(Some(platform_data));
            let lookup = |key: &str| data.lookup::<String>(key).ok().flatten();
            *self.token.borrow_mut() = lookup("activation-token").or_else(|| lookup("desktop-startup-id"));
        }

        fn after_emit(&self, platform_data: &glib::Variant) {
            self.token.take();
            self.parent_after_emit(platform_data);
        }
    }
}

glib::wrapper! {
    pub(crate) struct NotifyingApp(ObjectSubclass<imp::NotifyingApp>)
        @extends gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}
