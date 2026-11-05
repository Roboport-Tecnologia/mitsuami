//! Notifications: `cargo run -p mitsuami --example notifications`.
//!
//! - Notify shows the title and text as the platform shows its apps'
//!   notifications: Notification Center on macOS, GNOME's or Plasma's
//!   notifications, Windows' toasts. Each replaces the one before, since
//!   they share an id; Withdraw takes it away.
//! - Notify in 5 Seconds waits first, to switch to another app: macOS
//!   shows no banner for the app in front, only puts it in its list.
//! - Clicking the notification brings the app forward and says so here.
//! - The platform may ask first (macOS, the first time) and the user can
//!   turn the app's notifications off: that's reported as turned off.
//! - On macOS the app has to be in a bundle: `cargo run` reports that
//!   there's nothing to show them. Put the executable in an app bundle,
//!   not in a temporary folder (`docs/ARCHITECTURE.md` §14.15), and open
//!   that.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

/// The example's one notification, replaced each time.
pub const ID: &str = "example";

fn outcome(result: Result<(), ServiceError>) -> String {
    match result {
        Ok(()) => "Sent.".to_owned(),
        Err(ServiceError::Cancelled) => "Notifications are turned off for this app.".to_owned(),
        Err(ServiceError::Unavailable) => "There's nothing to show notifications.".to_owned(),
        Err(error) => format!("It wasn't sent: {error}"),
    }
}

pub fn page() -> impl View {
    let title = signal("Download finished".to_string());
    let body = signal("Ubuntu 26.04 is in Downloads.".to_string());
    let status = signal(String::new());
    let clicks = signal(0);
    on_notification_activated(move |id| {
        if id == ID {
            clicks.update(|n| *n += 1);
            status.set(format!("Clicked {} times.", clicks.get_untracked()));
        }
    });
    let send = move |after: Option<Duration>| {
        let notification = Notification::new(title.get_untracked()).body(body.get_untracked()).id(ID);
        spawn_local(async move {
            if let Some(after) = after {
                status.set("Sending in 5 seconds…".to_owned());
                sleep(after).await;
            }
            status.set(outcome(notify(notification).await));
        });
    };
    let withdraw = move || {
        withdraw_notification(ID);
        status.set("Withdrawn.".to_owned());
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <TextInput a11y_label="Title" bind=title/>
            <TextInput a11y_label="Text" bind=body/>
            <Row gap=Spacing::Sm>
                <Button enabled=move || !title.get().trim().is_empty() @click=move || send(None)>"Notify"</Button>
                <Button @click=move || send(Some(Duration::from_secs(5)))>"Notify in 5 Seconds"</Button>
                <Button @click=withdraw>"Withdraw"</Button>
            </Row>
            <Text test_id="status">{status}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .id("br.com.roboport.mitsuami.Notifications")
        .name("Notifications")
        .window("Notifications", WindowSize::FitHeight(420.0), page)
        .run();
}
