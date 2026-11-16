//! Notifications, as GNOME apps send them: a `GNotification` from the
//! app's `GApplication` (see `application`). GIO hands it to GNOME Shell,
//! to the portal in a sandbox, or to the freedesktop notification server
//! elsewhere. GIO doesn't say whether the user turned the app's
//! notifications off, so it never answers `Cancelled`: GNOME Shell drops
//! them, as it does every GNOME app's.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use mitsuami_core::services::{Notification, ServiceError};

use crate::application::{NOTIFICATION_ACTION, app};
use crate::backend::GtkHandle;

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
    shown.set_default_action_and_target_value(&format!("app.{NOTIFICATION_ACTION}"), Some(&id.to_variant()));
    app.send_notification(Some(&id), &shown);
    Ok(())
}

pub(crate) fn withdraw(backend: &GtkHandle, id: &str) {
    if let Some(app) = backend.app_id().and_then(|app_id| app(&app_id)) {
        app.withdraw_notification(id);
    }
}
