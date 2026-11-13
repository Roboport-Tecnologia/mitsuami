//! An app that runs without windows: `cargo run -p mitsuami --example background`.
//!
//! - It starts with no window, posts a notification, and waits.
//! - Clicking the notification opens its window. Where nothing shows
//!   notifications, the window opens after two seconds instead.
//! - Closing the window leaves it running; Quit ends it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

const ID: &str = "background";

fn window() -> Window {
    let open = signal(false);
    on_notification_activated(move |id| {
        if id == ID {
            open.set(true);
        }
    });
    spawn_local(async move {
        let notification = Notification::new("Waiting in the background").body("Click to open its window.").id(ID);
        if notify(notification).await.is_err() {
            sleep(Duration::from_secs(2)).await;
            open.set(true);
        }
    });
    Window::new("Background").size(WindowSize::FitHeight(360.0)).bind(open).content(|| {
        view! {
            <Column padding=Spacing::Xl gap=Spacing::Md>
                <Text>"Opened from the background. Close this window and the app keeps running."</Text>
                <Row justify=Justify::End>
                    <Button @click=quit>"Quit"</Button>
                </Row>
            </Column>
        }
    })
}

fn main() {
    App::new().id("br.com.roboport.mitsuami.Background").name("Background").runs_without_windows().open(window).run();
}
