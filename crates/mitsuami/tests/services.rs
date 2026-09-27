//! Platform services through the scripted fake: dialogs are answered by the
//! test, the clipboard and menus are inspected. The same code runs against
//! the real services in apps.

use std::path::PathBuf;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn editor() -> impl View {
    let text = signal(String::new());
    let status = signal(String::new());
    let dirty = computed(move || !text.get().is_empty());
    set_menu(
        MenuBar::new().menu(
            Menu::new("File")
                .item(MenuItem::new("Clear").on_select(move || text.set(String::new())).enabled(dirty))
                .separator()
                .item(MenuItem::new("Save…").shortcut(Shortcut::primary('s')).on_select(move || {
                    spawn_local(async move {
                        match save_file(SaveFile::new().name("notes.txt")).await {
                            Some(path) => status.set(format!("saved to {}", path.display())),
                            None => status.set("save cancelled".into()),
                        }
                    });
                })),
        ),
    );
    Column::new().gap(8).children((
        TextInput::new().a11y_label("Notes").bind(text),
        Row::new().gap(8).children((
            Button::new("Copy").on_click(move || {
                let written = set_clipboard_text(&text.get_untracked());
                spawn_local(async move {
                    if let Err(e) = written.await {
                        status.set(format!("copy failed: {e}"));
                    }
                });
            }),
            Button::new("Paste").on_click(move || {
                spawn_local(async move { text.set(clipboard_text().await.unwrap_or_default()) });
            }),
            Button::new("Discard").on_click(move || {
                spawn_local(async move {
                    let choice = alert(
                        Alert::new("Discard your notes?")
                            .message("This can't be undone.")
                            .button("Discard")
                            .button("Cancel")
                            .style(AlertStyle::Warning),
                    )
                    .await;
                    if choice == 0 {
                        text.set(String::new());
                    }
                });
            }),
            Button::new("Import…").on_click(move || {
                spawn_local(async move {
                    let files = open_file(OpenFile::new().multiple().filter(FileFilter::new("Text", ["txt", "md"])));
                    if let Some(paths) = files.await {
                        status.set(format!("importing {} file(s)", paths.len()));
                    }
                });
            }),
        )),
        Text::new(status).test_id("status"),
    ))
}

#[mitsuami_test::test]
async fn copy_and_paste_use_the_clipboard(app: TestApp) {
    app.mount(editor);
    app.get_by_label("Notes").fill("hello").await;
    app.get_by_role(Role::Button, "Copy").click().await;
    assert_eq!(app.services().clipboard().as_deref(), Some("hello"));

    app.services().set_clipboard("from another app");
    app.get_by_role(Role::Button, "Paste").click().await;
    app.expect(by_label("Notes")).to_have_value("from another app").await;
}

#[mitsuami_test::test]
async fn alerts_wait_for_an_answer(app: TestApp) {
    app.mount(editor);
    app.get_by_label("Notes").fill("draft").await;

    app.get_by_role(Role::Button, "Discard").click().await;
    let alert = app.services().take_alert().expect("an alert is showing");
    assert_eq!(alert.request.title, "Discard your notes?");
    assert_eq!(alert.request.buttons, ["Discard", "Cancel"]);
    assert_eq!(alert.request.style, AlertStyle::Warning);

    alert.respond(1); // Cancel
    app.settle().await;
    app.expect(by_label("Notes")).to_have_value("draft").await;

    app.get_by_role(Role::Button, "Discard").click().await;
    app.services().take_alert().unwrap().respond(0); // Discard
    app.expect(by_label("Notes")).to_have_value("").await;
}

#[mitsuami_test::test]
async fn file_dialogs_return_paths_or_nothing(app: TestApp) {
    app.mount(editor);
    app.get_by_role(Role::Button, "Import…").click().await;
    let open = app.services().take_open_file().expect("an open panel is showing");
    assert!(open.request.multiple);
    assert_eq!(open.request.filters[0].extensions, ["txt", "md"]);
    open.respond(Some(vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b.md")]));
    app.expect(by_text("importing 2 file(s)")).to_exist().await;
}

#[mitsuami_test::test]
async fn menus_run_their_handlers_and_follow_reactive_state(app: TestApp) {
    app.mount(editor);
    let menu = app.services().menu().expect("the app installed its menus");
    assert_eq!(menu.menus[0].title, "File");

    // "Clear" is disabled while there's nothing to clear.
    assert!(!app.services().choose_menu_item(&["File", "Clear"]));
    app.get_by_label("Notes").fill("something").await;
    assert!(app.services().choose_menu_item(&["File", "Clear"]));
    app.expect(by_label("Notes")).to_have_value("").await;

    assert!(app.services().choose_menu_item(&["File", "Save…"]));
    app.settle().await;
    app.services().take_save_file().expect("a save panel").respond(None);
    app.expect(by_text("save cancelled")).to_exist().await;
}

mitsuami_test::main!();
