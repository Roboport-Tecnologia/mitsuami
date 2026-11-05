//! Notifications, through the `notifications` example. The scripted
//! services stand in for the platform: they keep what's showing, and the
//! test clicks it as the user would.

#[allow(dead_code)]
#[path = "../examples/notifications.rs"]
mod example;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn a_notification_shows_and_the_next_replaces_it(app: TestApp) {
    app.mount(example::page);
    app.get_by_role(Role::Button, "Notify").click().await;
    app.expect(by_test_id("status")).to_have_text("Sent.").await;
    let shown = app.services().notifications();
    assert_eq!(shown, [Notification::new("Download finished").body("Ubuntu 26.04 is in Downloads.").id(example::ID)]);

    app.get_by_label("Title").fill("Upload finished").await;
    app.get_by_role(Role::Button, "Notify").click().await;
    let shown = app.services().notifications();
    assert_eq!(shown.len(), 1, "the same id replaces it");
    assert_eq!(shown[0].title, "Upload finished");

    app.get_by_role(Role::Button, "Withdraw").click().await;
    assert!(app.services().notifications().is_empty());
}

#[mitsuami_test::test]
async fn clicking_it_tells_the_app(app: TestApp) {
    app.mount(example::page);
    app.get_by_role(Role::Button, "Notify").click().await;
    assert!(app.services().click_notification(example::ID));
    app.expect(by_test_id("status")).to_have_text("Clicked 1 times.").await;
    assert!(app.services().notifications().is_empty(), "a click takes it from the list");
    assert!(!app.services().click_notification(example::ID));
}

#[mitsuami_test::test]
async fn it_can_wait(app: TestApp) {
    app.mount(example::page);
    app.get_by_role(Role::Button, "Notify in 5 Seconds").click().await;
    app.expect(by_test_id("status")).to_have_text("Sending in 5 seconds…").await;
    assert!(app.services().notifications().is_empty());
    app.advance(std::time::Duration::from_secs(5)).await;
    app.expect(by_test_id("status")).to_have_text("Sent.").await;
    assert_eq!(app.services().notifications().len(), 1);
}

#[mitsuami_test::test]
async fn the_user_can_turn_them_off(app: TestApp) {
    app.mount(example::page);
    app.services().set_notification_error(Some(ServiceError::Cancelled));
    app.get_by_role(Role::Button, "Notify").click().await;
    app.expect(by_test_id("status")).to_have_text("Notifications are turned off for this app.").await;
    assert!(app.services().notifications().is_empty());

    app.services().set_notification_error(Some(ServiceError::Unavailable));
    app.get_by_role(Role::Button, "Notify").click().await;
    app.expect(by_test_id("status")).to_have_text("There's nothing to show notifications.").await;
}

#[mitsuami_test::test]
async fn ids_are_given_to_notifications_without_one(app: TestApp) {
    app.mount(|| Text::new(""));
    let ui = app.ui().clone();
    let first = ui.notify(Notification::new("One"));
    let second = ui.notify(Notification::new("Two"));
    assert_eq!(first.await, Ok(()));
    assert_eq!(second.await, Ok(()));
    let ids: Vec<_> = app.services().notifications().into_iter().map(|n| n.id).collect();
    assert!(ids.iter().all(Option::is_some), "{ids:?}");
    assert_ne!(ids[0], ids[1], "both show");
}

mitsuami_test::main!();
