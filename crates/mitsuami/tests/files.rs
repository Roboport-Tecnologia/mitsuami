//! The `files` example: a file browser on a folder of the test's own, read
//! and changed off the UI thread.

#[path = "../examples/files/browser.rs"]
mod browser;
#[path = "../examples/files/file_icon/mod.rs"]
mod file_icon;
#[path = "../examples/files/fs.rs"]
mod fs;
#[path = "../examples/files/path_bar/mod.rs"]
mod path_bar;
#[path = "../examples/files/screen.rs"]
mod screen;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

use browser::{Browser, Outside};
use fs::Trash;
use path_bar::PathBar;
use screen::Finder;

thread_local! {
    static LAUNCHED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

fn launch(path: &Path) -> Result<(), String> {
    LAUNCHED.with(|l| l.borrow_mut().push(path.to_owned()));
    Ok(())
}

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
        LAUNCHED.with(|l| l.borrow_mut().clear());
        Fixture { root, trash }
    }

    fn mount(&self, app: &TestApp) {
        let (root, trash) = (self.root.clone(), self.trash.clone());
        app.mount(move || {
            provide(Browser::at(root, Outside { trash: Trash::Folder(trash), launch }));
            Finder::new()
        });
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.root.parent().unwrap());
    }
}

/// The rows the list shows, in order: each item's name, and the row's
/// own name (its columns' text, the item's name first). An empty folder
/// hides the list.
fn rows(app: &TestApp) -> Vec<(String, String)> {
    let list = app.get_by_role(Role::List, "Items");
    if !list.exists() {
        return Vec::new();
    }
    let list = list.node();
    list.walk()
        .into_iter()
        .filter(|n| n.role == Role::ListItem)
        .filter_map(|n| {
            let text = n.children.iter().find(|c| c.role == Role::StaticText)?;
            Some((text.name.clone()?, n.name.clone()?))
        })
        .collect()
}

fn names(app: &TestApp) -> Vec<String> {
    rows(app).into_iter().map(|(name, _)| name).collect()
}

/// The row of the item named `name`.
fn row<'a>(app: &'a TestApp, name: &str) -> Locator<'a> {
    let Some((_, row)) = rows(app).into_iter().find(|(n, _)| n == name) else {
        panic!("no row for {name:?} in {:?}", names(app))
    };
    app.get_by_role(Role::ListItem, row)
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

    // Sorted by kind, then by the Name column's title the other way round.
    app.get_by_label("View Options").choose_menu_item(&["Sort By", "Kind"]).await;
    listed(&app, &[".secret", "Photos", "notes 2.txt", "notes 10.txt", "a.rs"]).await;
    app.get_by_role(Role::Button, "Name").click().await;
    app.get_by_role(Role::Button, "Name").click().await;
    listed(&app, &["Photos", "notes 10.txt", "notes 2.txt", "a.rs", ".secret"]).await;
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
    let here = fixture.root.join("Projects/mitsuami");
    if PathBar::renderer().is_native() {
        app.expect(by_label("Path")).to_have_value(&here.display().to_string()).await;
    } else {
        app.expect(by_role(Role::Button, "Projects")).to_exist().await;
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
    LAUNCHED.with(|l| assert_eq!(*l.borrow(), [fixture.root.join("todo.txt")]));
}

#[mitsuami_test::test]
async fn a_new_folder_is_named_right_away(app: TestApp) {
    let fixture = Fixture::new(&app, &["todo.txt", "untitled folder/"]);
    fixture.mount(&app);
    listed(&app, &["todo.txt", "untitled folder"]).await;

    app.get_by_role(Role::Button, "New Folder").click().await;
    app.expect(by_label("New name")).to_have_value("untitled folder 2").await;
    app.get_by_label("New name").fill("Invoices").await;
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
    app.get_by_label("New name").fill("b.txt").await;
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
    listed(&app, &["Old", "report copy.txt", "report.txt"]).await;
    assert!(fixture.trash.join("Old copy/draft.txt").is_file());
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

#[mitsuami_test::test]
async fn the_preview_shows_the_selection(app: TestApp) {
    let fixture = Fixture::new(&app, &["readme.txt", "Stuff/one", "Stuff/two"]);
    fixture.mount(&app);
    listed(&app, &["readme.txt", "Stuff"]).await;
    app.expect(by_role(Role::StaticText, "Home")).to_exist().await;

    row(&app, "readme.txt").select().await;
    app.expect(by_text("This is readme.txt.\nSecond line.")).to_exist().await;
    assert_eq!(app.get_by_text("Plain text").count(), 2, "its row's kind, and the preview's");

    row(&app, "Stuff").select().await;
    app.expect(by_text("2 items")).to_exist().await;
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
