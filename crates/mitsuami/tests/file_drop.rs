//! Dropping files: a container or group takes the files and folders its
//! `FileDrop` says when they're dragged from the file manager and dropped
//! on it. Tests drag and drop through each platform's own drag handling,
//! with the paths the drag would carry: nothing here can drag from a file
//! manager.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mitsuami::core::backend::SyntheticInput;
use mitsuami::core::{ActionError, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A folder of its own for each test, with a few files and a folder in it.
fn fixtures(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mitsuami-file-drop-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("Photos")).unwrap();
    for file in ["game.iso", "DISC.CUE", "notes.txt"] {
        std::fs::write(dir.join(file), b"").unwrap();
    }
    dir
}

fn disc_drop() -> FileDrop {
    FileDrop::extensions(["iso", "cue"]).and_folders()
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect()
}

fn all(dir: &Path) -> Vec<PathBuf> {
    ["game.iso", "notes.txt", "Photos", "DISC.CUE"].iter().map(|f| dir.join(f)).collect()
}

#[mitsuami_test::test]
async fn takes_the_files_and_folders_it_accepts(app: TestApp) {
    let dir = fixtures("accepts");
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let log = dropped.clone();
    app.mount(move || {
        Column::new()
            .test_id("drop")
            .file_drop(disc_drop())
            .on_drop(move |paths| log.borrow_mut().push(paths))
            .child(Text::new("Drop .iso, .cue files or folders here"))
    });

    app.get(by_test_id("drop")).drop_files(&all(&dir)).await;
    // In the order the drag gave them; extensions ignore case.
    let dropped = dropped.borrow();
    assert_eq!(dropped.len(), 1);
    assert_eq!(names(&dropped[0]), ["game.iso", "Photos", "DISC.CUE"]);
}

/// Files it takes show while they're over it, for the app's highlight;
/// others don't, and dropping them does nothing.
#[mitsuami_test::test]
async fn says_when_files_it_takes_are_over_it(app: TestApp) {
    let dir = fixtures("hover");
    let over = signal(false);
    let drops = Rc::new(RefCell::new(0));
    let count = drops.clone();
    app.mount(move || {
        Column::new()
            .test_id("drop")
            .file_drop(disc_drop())
            .on_drop(move |_| *count.borrow_mut() += 1)
            .on_drop_hover(move |hover| over.set(hover))
            .child(Text::new(move || if over.get() { "Release to add" } else { "Drop here" }.to_owned()))
    });
    let zone = app.get(by_test_id("drop"));

    zone.drag_files(&[dir.join("notes.txt")]).await;
    assert!(!over.get_untracked(), "a text file isn't taken");
    zone.drag_files(&[dir.join("game.iso")]).await;
    app.expect(by_text("Release to add")).to_be_visible().await;
    zone.drag_leave().await;
    app.expect(by_text("Drop here")).to_be_visible().await;

    zone.drop_files(&[dir.join("notes.txt")]).await;
    assert_eq!(*drops.borrow(), 0);
    assert!(!over.get_untracked());
    zone.drop_files(&[dir.join("game.iso")]).await;
    assert_eq!(*drops.borrow(), 1);
    app.expect(by_text("Drop here")).to_be_visible().await;
}

/// Any file, but no folders; or only folders.
#[mitsuami_test::test]
async fn takes_any_file_or_only_folders(app: TestApp) {
    let dir = fixtures("kinds");
    let files = Rc::new(RefCell::new(Vec::new()));
    let folders = Rc::new(RefCell::new(Vec::new()));
    let (f, d) = (files.clone(), folders.clone());
    app.mount(move || {
        Column::new().children((
            Column::new().test_id("files").file_drop(FileDrop::files()).on_drop(move |p| f.borrow_mut().extend(p)),
            Column::new().test_id("folders").file_drop(FileDrop::folders()).on_drop(move |p| d.borrow_mut().extend(p)),
        ))
    });

    app.get(by_test_id("files")).drop_files(&all(&dir)).await;
    app.get(by_test_id("folders")).drop_files(&all(&dir)).await;
    assert_eq!(names(&files.borrow()), ["game.iso", "notes.txt", "DISC.CUE"]);
    assert_eq!(names(&folders.borrow()), ["Photos"]);
}

/// A group takes drops as a container does, and what it takes can change.
#[mitsuami_test::test]
async fn a_group_takes_them_too_and_follows_its_filter(app: TestApp) {
    let dir = fixtures("group");
    let drop = signal(FileDrop::extensions(["iso"]));
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let log = dropped.clone();
    app.mount(move || {
        Column::new().child(
            Group::new()
                .title("Library")
                .file_drop(drop)
                .on_drop(move |p| log.borrow_mut().extend(p))
                .child(Text::new("Discs")),
        )
    });
    let library = by_role(Role::Group, "Library");
    app.expect(library.clone()).to_exist().await;
    assert!(
        app.get(library.clone()).native_state().props.contains(&Prop::FileDrop(Some(FileDrop::extensions(["iso"]))))
    );

    drop.set(FileDrop::extensions(["cue"]));
    app.settle().await;
    app.get(library.clone()).drop_files(&all(&dir)).await;
    assert_eq!(names(&dropped.borrow()), ["DISC.CUE"]);
}

/// Without a `FileDrop`, a node refuses drops.
#[mitsuami_test::test]
async fn a_container_without_one_takes_none(app: TestApp) {
    let dir = fixtures("none");
    app.mount(|| Column::new().test_id("plain").child(Text::new("Nothing to drop on")));
    app.expect(by_text("Nothing to drop on")).to_be_visible().await;

    let id = app.get(by_test_id("plain")).id();
    let drop = SyntheticInput::DropFiles(vec![dir.join("game.iso")]);
    assert_eq!(app.ui().synthesize(id, &drop), Err(ActionError::Unsupported));
}

mitsuami_test::main!();
