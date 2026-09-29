//! Moving files to the trash, through the `trash` example. The scripted
//! services stand in for the platform's trash: the test moves nothing, or
//! deletes what the platform would have moved, then answers. The real
//! trash is checked in each backend's `tests/services.rs`.

#[allow(dead_code)]
#[path = "../examples/trash.rs"]
mod example;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// The example's folder, as it starts.
fn fresh(app: &TestApp) -> std::path::PathBuf {
    let folder = std::env::temp_dir().join("mitsuami trash example");
    _ = std::fs::remove_dir_all(&folder);
    app.mount(example::page);
    folder
}

#[mitsuami_test::test]
async fn trashes_the_selected_items(app: TestApp) {
    let folder = fresh(&app);
    app.expect(by_role(Role::Button, "Move to Trash")).to_be_disabled().await;
    app.get_by_role(Role::ListItem, "Draft.md").select().await;
    app.get_by_role(Role::Button, "Move to Trash").click().await;

    let trash = app.services().take_trash().expect("a request to trash");
    assert_eq!(trash.request, [folder.join("Draft.md")]);
    std::fs::remove_file(folder.join("Draft.md")).unwrap();
    trash.respond(Ok(()));
    app.expect(by_test_id("status")).to_have_text("Moved 1 item to the trash.").await;
    app.expect(by_role(Role::ListItem, "Draft.md")).not_to_exist().await;
    app.expect(by_role(Role::ListItem, "Notes.txt")).to_exist().await;
}

#[mitsuami_test::test]
async fn cancelling_leaves_the_items(app: TestApp) {
    let folder = fresh(&app);
    app.get_by_role(Role::ListItem, "Notes.txt").select().await;
    app.get_by_role(Role::Button, "Move to Trash").click().await;
    app.services().take_trash().unwrap().respond(Err(ServiceError::Cancelled));
    app.expect(by_test_id("status")).to_have_text("Nothing was moved.").await;
    assert!(folder.join("Notes.txt").exists());
    app.expect(by_role(Role::ListItem, "Notes.txt")).to_exist().await;
}

#[mitsuami_test::test]
async fn a_disk_without_a_trash_asks_before_deleting(app: TestApp) {
    let folder = fresh(&app);
    app.get_by_role(Role::ListItem, "Old photos").select().await;
    app.get_by_role(Role::Button, "Move to Trash").click().await;
    app.services().take_trash().unwrap().respond(Err(ServiceError::Unavailable));
    app.settle().await;

    let alert = app.services().take_alert().expect("the app asks first");
    assert_eq!(alert.request.title, "Delete 1 item immediately?");
    alert.respond(0);
    app.expect(by_test_id("status")).to_have_text("Deleted them.").await;
    assert!(!folder.join("Old photos").exists());
    app.expect(by_role(Role::ListItem, "Old photos")).not_to_exist().await;
}

mitsuami_test::main!();
