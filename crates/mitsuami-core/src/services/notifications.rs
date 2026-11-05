//! Notifications: what an app tells the user outside its windows.

/// A message the platform shows outside the app's windows (macOS's
/// Notification Center, GNOME's and Plasma's notifications, Windows'
/// toasts), and keeps in its list until the user clears it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    /// Names it: a notification with the same id replaces it, and
    /// [`withdraw_notification`](super::withdraw_notification) takes it
    /// away. The core gives one to a notification the app didn't name,
    /// so backends always have one.
    pub id: Option<String>,
    pub title: String,
    pub body: Option<String>,
}

impl Notification {
    pub fn new(title: impl Into<String>) -> Notification {
        Notification { id: None, title: title.into(), body: None }
    }

    /// The text under the title.
    pub fn body(mut self, body: impl Into<String>) -> Notification {
        self.body = Some(body.into());
        self
    }

    /// Names it, so a later one with the same id replaces it (a download's
    /// progress, a count of unread messages) and the app can withdraw it
    /// or tell which one the user clicked.
    pub fn id(mut self, id: impl Into<String>) -> Notification {
        self.id = Some(id.into());
        self
    }
}
