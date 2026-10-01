//! The `files` example: a file browser on a folder of the test's own, read
//! and changed off the UI thread.

#[path = "../examples/files/browser.rs"]
mod browser;
#[path = "../examples/files/fs.rs"]
mod fs;
#[path = "../examples/files/path_bar/mod.rs"]
mod path_bar;
#[path = "../examples/files/screen.rs"]
mod screen;

use std::path::{Path, PathBuf};

use mitsuami::core::services::MenuCheck;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

use browser::Browser;
use path_bar::PathBar;
use screen::Finder;

/// A folder of the test's own, with `files` in it (a trailing `/` makes a
/// folder), and a trash beside it.
struct Fixture {
    root: PathBuf,
    trash: PathBuf,
}

impl Fixture {
    fn new(app: &TestApp, files: &[&str]) -> Fixture {
        // The same path every run, as the visual captures show it.
        let base = std::env::temp_dir().join("mitsuami-files").join(app.test_name());
        let _ = std::fs::remove_dir_all(&base);
        let (root, trash) = (base.join("Home"), base.join("Trash"));
        std::fs::create_dir_all(&root).unwrap();
        for file in files {
            let path = root.join(file);
            if file.ends_with('/') {
                std::fs::create_dir_all(&path).unwrap();
            } else {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, format!("This is {file}.\nSecond line.\n")).unwrap();
            }
        }
        // A fixed date (2026-09-01 09:30 UTC), for the same captures.
        let date = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_788_255_000);
        for file in files {
            let mut options = std::fs::OpenOptions::new();
            // Windows opens a folder only with backup semantics, and sets
            // its date with the right to write attributes.
            #[cfg(windows)]
            std::os::windows::fs::OpenOptionsExt::custom_flags(
                std::os::windows::fs::OpenOptionsExt::access_mode(&mut options, 0x100),
                0x0200_0000,
            );
            #[cfg(not(windows))]
            options.read(true);
            options.open(root.join(file)).unwrap().set_modified(date).unwrap();
        }
        Fixture { root, trash }
    }

    fn mount(&self, app: &TestApp) {
        let root = self.root.clone();
        app.mount(move || {
            provide(Browser::at(root));
            Finder::new()
        });
    }
}

impl Fixture {
    /// Does what the platform's trash would: moves the items asked for
    /// into the fixture's trash folder, then answers.
    async fn trash_as_asked(&self, app: &TestApp) {
        app.settle().await;
        let trash = app.services().take_trash().expect("a request to trash");
        std::fs::create_dir_all(&self.trash).unwrap();
        for path in &trash.request {
            std::fs::rename(path, self.trash.join(path.file_name().unwrap())).unwrap();
        }
        trash.respond(Ok(()));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.root.parent().unwrap());
    }
}

/// The items the table shows, in order: their Name cells' text. An empty
/// folder hides the table.
fn names(app: &TestApp) -> Vec<String> {
    let table = app.get_by_role(Role::Table, "Items");
    if !table.exists() {
        return Vec::new();
    }
    let table = table.node();
    table
        .walk()
        .into_iter()
        .filter(|n| n.role == Role::Row)
        .filter_map(|n| n.children.iter().find(|c| c.role == Role::Cell)?.name.clone())
        .collect()
}

/// The Name cell of the item named `name`, which acts for its row.
fn row<'a>(app: &'a TestApp, name: &str) -> Locator<'a> {
    if !names(app).iter().any(|n| n == name) {
        panic!("no row for {name:?} in {:?}", names(app))
    }
    app.get_by_role(Role::Cell, name)
}

/// Chooses a folder in the path bar as a user would: the platform's own
/// control through its accessibility action, the composed one's button.
async fn choose_in_path_bar(app: &TestApp, folder: &Path) {
    if PathBar::renderer().is_native() {
        app.get_by_label("Path").fill(&folder.display().to_string()).await;
    } else {
        app.get_by_role(Role::Button, path_bar::name(folder)).click().await;
    }
}

/// Waits for the folder to be read (and changed), then checks its rows.
async fn listed(app: &TestApp, expected: &[&str]) {
    app.wait_for_tasks().await;
    assert_eq!(names(app), expected);
}

#[mitsuami_test::test]
async fn lists_the_folder_as_people_count_hiding_dot_files(app: TestApp) {
    let fixture = Fixture::new(&app, &["notes 10.txt", "notes 2.txt", "Photos/", ".secret", "a.rs"]);
    fixture.mount(&app);
    listed(&app, &["a.rs", "notes 2.txt", "notes 10.txt", "Photos"]).await;
    app.expect(by_text("4 items")).to_exist().await;

    app.get_by_label("View Options").choose_menu_item(&["Show Hidden Files"]).await;
    listed(&app, &[".secret", "a.rs", "notes 2.txt", "notes 10.txt", "Photos"]).await;

    // Sorted by kind, then by the Name column's header the other way round,
    // which the menus follow.
    app.get_by_label("View Options").choose_menu_item(&["Sort By", "Kind"]).await;
    listed(&app, &[".secret", "Photos", "notes 2.txt", "notes 10.txt", "a.rs"]).await;
    app.get_by_role(Role::ColumnHeader, "Name").click().await;
    app.get_by_role(Role::ColumnHeader, "Name").click().await;
    listed(&app, &["Photos", "notes 10.txt", "notes 2.txt", "a.rs", ".secret"]).await;
    let check = |path: &[&str]| app.services().menu_item(path).map(|item| item.check);
    assert_eq!(check(&["View", "Sort By", "Name"]), Some(MenuCheck::Radio(true)));
    assert_eq!(check(&["View", "Sort By", "Reversed"]), Some(MenuCheck::Check(true)));
}

#[mitsuami_test::test]
async fn a_folder_opens_at_its_top(app: TestApp) {
    let files: Vec<String> = (0..60).flat_map(|i| [format!("a{i:02}.txt"), format!("Sub/b{i:02}.txt")]).collect();
    let fixture = Fixture::new(&app, &files.iter().map(String::as_str).collect::<Vec<_>>());
    fixture.mount(&app);
    app.wait_for_tasks().await;
    // Down to the subfolder, listed last, and into it.
    app.get_by_role(Role::Table, "Items").scroll_by(0.0, 1e6).await;
    assert!(!app.get_by_role(Role::Cell, "a00.txt").is_visible());
    row(&app, "Sub").click().await;
    app.wait_for_tasks().await;
    assert!(app.get_by_role(Role::Cell, "b00.txt").is_visible());
}

#[mitsuami_test::test]
async fn folders_open_in_place_and_history_goes_back(app: TestApp) {
    let fixture = Fixture::new(&app, &["Projects/mitsuami/README.md", "todo.txt"]);
    fixture.mount(&app);
    listed(&app, &["Projects", "todo.txt"]).await;
    assert!(!app.get_by_role(Role::Button, "Back").is_enabled());

    row(&app, "Projects").click().await;
    listed(&app, &["mitsuami"]).await;
    row(&app, "mitsuami").click().await;
    listed(&app, &["README.md"]).await;
    // The path bar has every folder up to this one.
    let here = fixture.root.join("Projects").join("mitsuami");
    if PathBar::renderer().is_native() {
        app.expect(by_label("Path")).to_have_value(&here.display().to_string()).await;
    } else {
        app.expect(by_role(Role::Button, "Projects")).to_exist().await;
        assert!(!app.get_by_role(Role::Button, "mitsuami").is_enabled());
    }

    app.get_by_role(Role::Button, "Back").click().await;
    listed(&app, &["mitsuami"]).await;
    app.get_by_role(Role::Button, "Forward").click().await;
    listed(&app, &["README.md"]).await;

    // Up through the path bar: the folder come from is selected.
    choose_in_path_bar(&app, &fixture.root).await;
    listed(&app, &["Projects", "todo.txt"]).await;
    app.expect(by_text("1 of 2 items selected")).to_exist().await;
    assert!(!app.get_by_role(Role::Button, "Forward").is_enabled());
}

#[mitsuami_test::test]
async fn files_open_in_their_apps(app: TestApp) {
    let fixture = Fixture::new(&app, &["todo.txt"]);
    fixture.mount(&app);
    listed(&app, &["todo.txt"]).await;
    row(&app, "todo.txt").click().await;
    let launch = app.services().take_launch().expect("the file is opened");
    assert_eq!(launch.request, Launch::Path(fixture.root.join("todo.txt")));

    // Where no app opens it and the platform says nothing, the app does.
    launch.respond(Err(ServiceError::Unavailable));
    app.settle().await;
    let warning = app.services().take_alert().expect("a warning");
    assert_eq!(warning.request.title, "\u{201C}todo.txt\u{201D} couldn't be opened.");
    assert_eq!(warning.request.message.as_deref(), Some("No app is set to open it."));
    warning.respond(0);
}

#[mitsuami_test::test]
async fn a_new_folder_is_named_right_away(app: TestApp) {
    let fixture = Fixture::new(&app, &["todo.txt", "untitled folder/"]);
    fixture.mount(&app);
    listed(&app, &["todo.txt", "untitled folder"]).await;

    app.get_by_role(Role::Button, "New Folder").click().await;
    app.expect(by_label("New name")).to_have_value("untitled folder 2").await;
    // All of a folder's name is selected, and what's typed replaces it.
    app.expect(by_label("New name")).to_be_focused().await;
    assert_eq!(app.get_by_label("New name").text_selection(), Some(0..17));
    app.get_by_label("New name").type_text("Invoices").await;
    app.get_by_label("New name").press(Key::Enter).await;
    listed(&app, &["Invoices", "todo.txt", "untitled folder"]).await;
    app.expect(by_text("1 of 3 items selected")).to_exist().await;
    assert!(fixture.root.join("Invoices").is_dir());
}

#[mitsuami_test::test]
async fn renaming_to_a_taken_name_says_why(app: TestApp) {
    let fixture = Fixture::new(&app, &["a.txt", "b.txt"]);
    fixture.mount(&app);
    listed(&app, &["a.txt", "b.txt"]).await;

    row(&app, "a.txt").choose_menu_item(&["Rename…"]).await;
    app.expect(by_text("Rename \u{201C}a.txt\u{201D} to:")).to_exist().await;
    // The name without its extension is selected, as file managers do.
    app.expect(by_label("New name")).to_be_focused().await;
    assert_eq!(app.get_by_label("New name").text_selection(), Some(0..1));
    app.get_by_label("New name").type_text("b").await;
    app.expect(by_label("New name")).to_have_value("b.txt").await;
    app.get_by_role(Role::Button, "Rename").click().await;

    // The rename fails on a worker thread; the warning follows.
    let warning = loop {
        app.settle().await;
        if let Some(warning) = app.services().take_alert() {
            break warning;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(warning.request.title, "The item couldn't be renamed.");
    assert_eq!(warning.request.message.as_deref(), Some("The name \u{201C}b.txt\u{201D} is already taken."));
    warning.respond(0);
    listed(&app, &["a.txt", "b.txt"]).await;
}

#[mitsuami_test::test]
async fn duplicates_and_trash(app: TestApp) {
    let fixture = Fixture::new(&app, &["report.txt", "Old/draft.txt"]);
    fixture.mount(&app);
    listed(&app, &["Old", "report.txt"]).await;

    row(&app, "report.txt").choose_menu_item(&["Duplicate"]).await;
    listed(&app, &["Old", "report copy.txt", "report.txt"]).await;
    row(&app, "Old").choose_menu_item(&["Duplicate"]).await;
    listed(&app, &["Old", "Old copy", "report copy.txt", "report.txt"]).await;
    assert!(fixture.root.join("Old copy/draft.txt").is_file(), "folders are copied with what's in them");

    // A row's menu acts on that row when it isn't in the selection.
    row(&app, "Old").select().await;
    row(&app, "Old copy").choose_menu_item(&["Move to Trash"]).await;
    fixture.trash_as_asked(&app).await;
    listed(&app, &["Old", "report copy.txt", "report.txt"]).await;
    assert!(fixture.trash.join("Old copy/draft.txt").is_file());
}

/// The menus have the platform's file manager's shortcuts, keys that type
/// nothing included.
#[mitsuami_test::test]
async fn menus_have_the_file_manager_s_shortcuts(app: TestApp) {
    let fixture = Fixture::new(&app, &["Projects/todo.txt"]);
    fixture.mount(&app);
    listed(&app, &["Projects"]).await;
    let shortcut = |path: &[&str]| app.services().menu_item(path).and_then(|item| item.shortcut);
    let (trash, up) = platform! {
        macos => (Shortcut::primary(Key::Backspace), Shortcut::primary(Key::Up)),
        _ => (Shortcut::new(Key::Delete), Shortcut::new(Key::Up).alt()),
    };
    assert_eq!(shortcut(&["File", "Move to Trash"]), Some(trash));
    assert_eq!(shortcut(&["Go", "Enclosing Folder"]), Some(up));
    assert_eq!(
        shortcut(&["File", "Rename…"]),
        platform! { macos => None, _ => Some(Shortcut::new(Key::F(2))) },
        "Finder renames with Return"
    );

    row(&app, "Projects").click().await;
    listed(&app, &["todo.txt"]).await;
    assert!(app.services().choose_menu_item(&["Go", "Enclosing Folder"]));
    listed(&app, &["Projects"]).await;
}

/// Space on the table shows and hides the preview, where the platform's
/// file manager has it there and its table doesn't keep Space (Finder).
/// The toolbar's toggle does everywhere.
#[mitsuami_test::test]
async fn the_preview_key_shows_and_hides_the_preview(app: TestApp) {
    let fixture = Fixture::new(&app, &["notes.txt"]);
    fixture.mount(&app);
    listed(&app, &["notes.txt"]).await;
    row(&app, "notes.txt").select().await;
    assert!(app.get_by_label("Preview").exists());
    app.get_by_role(Role::ToggleButton, "Show Preview").click().await;
    assert!(!app.get_by_label("Preview").exists());
    app.get_by_role(Role::ToggleButton, "Show Preview").click().await;
    app.expect(by_text("This is notes.txt.\nSecond line.")).to_exist().await;

    let Some(key) = screen::preview_key() else { return };
    app.get_by_role(Role::Table, "Items").press(key).await;
    assert!(!app.get_by_label("Preview").exists());
    assert!(!app.get_by_role(Role::ToggleButton, "Show Preview").is_checked());
    app.get_by_role(Role::Table, "Items").press(key).await;
    app.expect(by_text("This is notes.txt.\nSecond line.")).to_exist().await;
}

/// Go to Folder… opens with its field focused, and again with the path
/// typed last selected, as Finder's does.
#[mitsuami_test::test]
async fn go_to_folder_starts_from_the_path_typed_last(app: TestApp) {
    let fixture = Fixture::new(&app, &["Projects/todo.txt"]);
    fixture.mount(&app);
    listed(&app, &["Projects"]).await;
    let projects = fixture.root.join("Projects").display().to_string();

    assert!(app.services().choose_menu_item(&["Go", "Go to Folder…"]));
    app.expect(by_role(Role::TextField, "Folder")).to_be_focused().await;
    app.get_by_role(Role::TextField, "Folder").type_text(&projects).await;
    app.get_by_role(Role::TextField, "Folder").press(Key::Enter).await;
    listed(&app, &["todo.txt"]).await;

    assert!(app.services().choose_menu_item(&["Go", "Go to Folder…"]));
    app.expect(by_role(Role::TextField, "Folder")).to_be_focused().await;
    assert_eq!(app.get_by_role(Role::TextField, "Folder").text_selection(), Some(0..projects.chars().count()));
}

#[mitsuami_test::test]
async fn search_filters_the_folder(app: TestApp) {
    let fixture = Fixture::new(&app, &["alpha.txt", "beta.txt", "alphabet.md"]);
    fixture.mount(&app);
    listed(&app, &["alpha.txt", "alphabet.md", "beta.txt"]).await;

    app.get_by_label("Search").fill("alpha").await;
    listed(&app, &["alpha.txt", "alphabet.md"]).await;
    app.get_by_label("Search").fill("zeta").await;
    app.expect(by_text("No matches")).to_exist().await;
    app.get_by_label("Search").fill("").await;
    listed(&app, &["alpha.txt", "alphabet.md", "beta.txt"]).await;
}

#[mitsuami_test::test]
async fn dropped_files_are_copied_in(app: TestApp) {
    let fixture = Fixture::new(&app, &["Inbox/", "elsewhere/photo.png", "elsewhere/notes.txt"]);
    fixture.mount(&app);
    listed(&app, &["elsewhere", "Inbox"]).await;
    row(&app, "Inbox").click().await;
    listed(&app, &[]).await;
    app.expect(by_text("No items")).to_exist().await;

    let dropped = [fixture.root.join("elsewhere/photo.png"), fixture.root.join("elsewhere/notes.txt")];
    app.get_by_label("Contents").drop_files(&dropped).await;
    listed(&app, &["notes.txt", "photo.png"]).await;
    assert!(fixture.root.join("elsewhere/notes.txt").is_file(), "copied, not moved");
}

/// Rows drag their files out (the selection's, from a selected row);
/// dropped back where they are, they stay as they are.
#[mitsuami_test::test]
async fn items_drag_out_as_their_files(app: TestApp) {
    let fixture = Fixture::new(&app, &["notes.txt", "photo.png", "Stuff/one"]);
    fixture.mount(&app);
    listed(&app, &["notes.txt", "photo.png", "Stuff"]).await;

    assert_eq!(row(&app, "Stuff").dragged_files(), [fixture.root.join("Stuff")]);
    row(&app, "photo.png").select().await;
    assert_eq!(row(&app, "photo.png").dragged_files(), [fixture.root.join("photo.png")]);

    let dragged = row(&app, "notes.txt").dragged_files();
    app.get_by_label("Contents").drop_files(&dragged).await;
    listed(&app, &["notes.txt", "photo.png", "Stuff"]).await;
}

#[mitsuami_test::test]
async fn the_preview_shows_the_selection(app: TestApp) {
    let fixture = Fixture::new(&app, &["readme.txt", "Stuff/one", "Stuff/two"]);
    fixture.mount(&app);
    listed(&app, &["readme.txt", "Stuff"]).await;
    // With nothing selected, the preview asks for a selection.
    let preview = app.get_by_label("Preview").node();
    assert!(preview.walk().iter().any(|n| n.role == Role::StaticText && n.name.as_deref() == Some("Select an item")));

    row(&app, "readme.txt").select().await;
    app.expect(by_text("This is readme.txt.\nSecond line.")).to_exist().await;
    assert_eq!(app.get_by_text("Plain text").count(), 2, "its row's kind, and the preview's");

    row(&app, "Stuff").select().await;
    app.expect(by_text("2 items")).to_exist().await;
}

#[mitsuami_test::test]
async fn long_names_are_cut_off_in_their_column(app: TestApp) {
    let long = "a name much too long for its column, which goes on and on and on.txt";
    let fixture = Fixture::new(&app, &[long]);
    fixture.mount(&app);
    listed(&app, &[long]).await;
    let (cell, name) = (row(&app, long).frame(), app.get_by_text(long).frame());
    assert!(name.max_x() <= cell.max_x(), "{name:?} runs out of its cell, {cell:?}");
}

/// A name's tooltip is the item's whole path, cut off or not.
#[mitsuami_test::test]
async fn names_show_their_path_in_a_tooltip(app: TestApp) {
    let fixture = Fixture::new(&app, &["notes.txt"]);
    fixture.mount(&app);
    listed(&app, &["notes.txt"]).await;
    let tooltip = app.get_by_text("notes.txt").native_state().props.into_iter().find_map(|p| match p {
        mitsuami::core::Prop::Tooltip(t) => Some(t),
        _ => None,
    });
    assert_eq!(tooltip, Some(fixture.root.join("notes.txt").display().to_string()));
}

#[mitsuami_test::test]
async fn looks_like_a_file_browser(app: TestApp) {
    let fixture = Fixture::new(&app, &["Documents/", "Pictures/", "notes.txt", "todo.md", "build.rs"]);
    fixture.mount(&app);
    listed(&app, &["build.rs", "Documents", "notes.txt", "Pictures", "todo.md"]).await;
    row(&app, "notes.txt").select().await;
    app.expect(by_text("This is notes.txt.\nSecond line.")).to_exist().await;
    app.assert_visual_snapshot("notes_selected").await;
}

mitsuami_test::main!();
