//! The `contacts` example: ten thousand rows, searched, selected and
//! opened.

#[path = "../examples/contacts/screen.rs"]
mod screen;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

use screen::Contacts;

fn rows(app: &TestApp) -> usize {
    app.a11y_tree().walk().into_iter().filter(|n| n.role == Role::ListItem).count()
}

#[mitsuami_test::test]
async fn ten_thousand_contacts_mount_only_a_few_rows(app: TestApp) {
    app.mount(Contacts::new);
    app.expect(by_text("10000 of 10000")).to_exist().await;
    assert!(rows(&app) < 60, "{} rows mounted", rows(&app));
    app.expect(by_text("No contact selected")).to_exist().await;
}

#[mitsuami_test::test]
async fn searching_filters_and_keeps_the_selection(app: TestApp) {
    app.mount(Contacts::new);
    app.get_by_label("Search").fill("grace").await;
    app.expect(by_text("500 of 10000")).to_exist().await;

    app.get_by_role(Role::ListItem, "Grace Hopper grace.hopper42@example.com").select().await;
    app.expect(by_text("Tokyo")).to_exist().await;

    // Still there with a narrower search; gone, with the details, when
    // the search leaves it out.
    app.get_by_label("Search").fill("grace hopper").await;
    app.expect(by_text("20 of 10000")).to_exist().await;
    assert_eq!(app.get_by_role(Role::ListItem, "Grace Hopper grace.hopper42@example.com").node().selected, Some(true));
    app.get_by_label("Search").fill("ada").await;
    app.expect(by_text("No contact selected")).to_exist().await;
}

#[mitsuami_test::test]
async fn activating_a_row_opens_it(app: TestApp) {
    app.mount(Contacts::new);
    app.get_by_role(Role::ListItem, "Alan Lovelace alan.lovelace1@example.com").click().await;
    app.expect(by_text("Opened #1")).to_exist().await;
}

#[mitsuami_test::test]
async fn show_in_list_scrolls_back_to_the_selection(app: TestApp) {
    app.mount(Contacts::new);
    let first = "Ada Lovelace ada.lovelace0@example.com";
    app.get_by_role(Role::ListItem, first).select().await;
    app.get_by_role(Role::List, "Contacts").scroll_by(0.0, 100_000.0).await;
    app.expect(by_role(Role::ListItem, first)).not_to_exist().await;

    app.get_by_role(Role::Button, "Show in list").click().await;
    assert!(app.get_by_role(Role::ListItem, first).is_visible());
}

mitsuami_test::main!();
