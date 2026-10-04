//! Opening files, folders and links in other apps, through the `launch`
//! example. The scripted services stand in for the platform: they open
//! nothing, and the test answers as it would.

#[allow(dead_code)]
#[path = "../examples/launch.rs"]
mod example;

use example::folder;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn files_and_folders_open_in_their_apps(app: TestApp) {
    app.mount(example::page);
    app.get_by_role(Role::Button, "Open Text file").click().await;
    let launch = app.services().take_launch().expect("a request to open it");
    assert_eq!(launch.request, Launch::Path(folder().join("Notes.txt")));
    launch.respond(Ok(()));
    app.expect(by_test_id("status")).to_have_text("Opened Notes.txt.").await;

    app.get_by_role(Role::Button, "Open Folder").click().await;
    let launch = app.services().take_launch().unwrap();
    assert_eq!(launch.request, Launch::Path(folder().join("A folder")));
    launch.respond(Ok(()));
    app.expect(by_test_id("status")).to_have_text("Opened A folder.").await;
}

#[mitsuami_test::test]
async fn the_platform_may_have_no_app_or_be_dismissed(app: TestApp) {
    app.mount(example::page);
    app.get_by_role(Role::Button, "Open Unknown type").click().await;
    app.services().take_launch().unwrap().respond(Err(ServiceError::Unavailable));
    app.expect(by_test_id("status")).to_have_text("No app opens Mystery.mitsuami-unknown.").await;

    app.get_by_role(Role::Button, "Open Unknown type").click().await;
    app.services().take_launch().unwrap().respond(Err(ServiceError::Cancelled));
    app.expect(by_test_id("status")).to_have_text("Mystery.mitsuami-unknown: cancelled.").await;
}

#[mitsuami_test::test]
async fn links_open_in_the_app_for_their_scheme(app: TestApp) {
    app.mount(example::page);
    app.get_by_label("Address").fill("mailto:someone@example.com").await;
    app.get_by_role(Role::Button, "Open Link").click().await;
    let launch = app.services().take_launch().unwrap();
    assert_eq!(launch.request, Launch::Url("mailto:someone@example.com".into()));
    launch.respond(Ok(()));
    app.expect(by_test_id("status")).to_have_text("Opened mailto:someone@example.com.").await;

    app.get_by_label("Address").fill("  ").await;
    app.expect(by_role(Role::Button, "Open Link")).to_be_disabled().await;
}

mitsuami_test::main!();
