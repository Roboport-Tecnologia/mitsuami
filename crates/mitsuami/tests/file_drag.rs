//! Dragging files out: a list's or table's rows carry their files to the
//! file manager, another app or a folder, through the platform's own drag.
//! Nothing here can start a real drag, so tests ask each backend what its
//! drag source would carry (`dragged_files`): the selected rows' files
//! when a selected row is dragged, else the row's own.

use std::path::PathBuf;

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A few files and a folder, in a folder of the test's own.
fn files(app: &TestApp) -> PathBuf {
    let root = std::env::temp_dir().join("mitsuami-file-drag").join(app.test_name());
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Photos")).unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(root.join(name), name).unwrap();
    }
    root
}

const NAMES: [&str; 4] = ["a.txt", "b.txt", "c.txt", "Photos"];

fn row(name: &'static str) -> impl View {
    Row::new().padding(4).child(Text::new(name))
}

/// A list of the names, dragging their files (the folder too, unless
/// `folders` is off), with the selection bound.
fn list(root: PathBuf, selected: Signal<Vec<&'static str>>, folders: bool) -> impl View {
    List::new(|| NAMES.to_vec(), |n| *n, row)
        .selection_mode(SelectionMode::Multiple)
        .selected(selected)
        .drag_files(move |name| (folders || !name.starts_with('P')).then(|| root.join(name)))
        .height(200)
}

fn item(name: &str) -> Query {
    by_role(Role::ListItem, name)
}

#[mitsuami_test::test]
async fn a_row_drags_its_file(app: TestApp) {
    let root = files(&app);
    let selected = signal(Vec::new());
    let r = root.clone();
    app.mount(move || list(r.clone(), selected, true));
    app.expect(item("b.txt")).to_exist().await;

    assert_eq!(app.get(item("b.txt")).dragged_files(), [root.join("b.txt")]);
    assert_eq!(app.get(item("Photos")).dragged_files(), [root.join("Photos")]);
}

/// Dragging a selected row drags every selected row's file, in row order,
/// as the platform drags a selection; an unselected row drags alone.
#[mitsuami_test::test]
async fn a_selected_row_drags_the_selection(app: TestApp) {
    let root = files(&app);
    let selected = signal(Vec::new());
    let r = root.clone();
    app.mount(move || list(r.clone(), selected, true));
    app.expect(item("c.txt")).to_exist().await;

    selected.set(vec!["c.txt", "a.txt"]);
    app.settle().await;
    assert_eq!(app.get(item("c.txt")).dragged_files(), [root.join("a.txt"), root.join("c.txt")]);
    assert_eq!(app.get(item("b.txt")).dragged_files(), [root.join("b.txt")]);

    app.get_by_role(Role::ListItem, "b.txt").select().await;
    assert_eq!(app.get(item("b.txt")).dragged_files(), [root.join("b.txt")]);
}

/// A row without a file doesn't drag; in a selection, only the rows with
/// one are carried.
#[mitsuami_test::test]
async fn rows_without_a_file_do_not_drag(app: TestApp) {
    let root = files(&app);
    let selected = signal(Vec::new());
    let r = root.clone();
    app.mount(move || list(r.clone(), selected, false));
    app.expect(item("Photos")).to_exist().await;

    assert!(app.get(item("Photos")).dragged_files().is_empty());
    selected.set(vec!["b.txt", "Photos"]);
    app.settle().await;
    assert_eq!(app.get(item("Photos")).dragged_files(), [root.join("b.txt")]);
}

/// The platform has the rows' files, and follows the data.
#[mitsuami_test::test]
async fn the_files_follow_the_data(app: TestApp) {
    let root = files(&app);
    let names = signal(vec!["a.txt", "b.txt"]);
    let r = root.clone();
    app.mount(move || {
        let r = r.clone();
        List::new(names, |n| *n, row).drag_files(move |name| Some(r.join(name))).a11y_label("Files").height(200)
    });
    app.expect(item("b.txt")).to_exist().await;
    let list = app.get(by_role(Role::List, "Files"));
    let files = || mitsuami::core::find_prop!(list.native_state().props, RowFiles).unwrap_or_default();
    assert_eq!(files().len(), 2);

    names.set(vec!["b.txt", "c.txt", "a.txt"]);
    app.settle().await;
    let paths: Vec<PathBuf> = files().into_iter().map(|(_, path)| path).collect();
    assert_eq!(paths, [root.join("b.txt"), root.join("c.txt"), root.join("a.txt")]);
    assert_eq!(app.get(item("c.txt")).dragged_files(), [root.join("c.txt")]);
    assert!(list.native_state().props.iter().any(|p| matches!(p, Prop::RowFiles(_))));
}

/// A table's rows drag as a list's do.
#[mitsuami_test::test]
async fn a_table_s_rows_drag_their_files(app: TestApp) {
    let root = files(&app);
    let selected = signal(Vec::new());
    let r = root.clone();
    app.mount(move || {
        let r = r.clone();
        Table::new(|| NAMES.to_vec(), |n| *n)
            .column(TableColumn::new("Name", |n: &'static str| Text::new(n)).expand())
            .selection_mode(SelectionMode::Multiple)
            .selected(selected)
            .drag_files(move |name| Some(r.join(name)))
            .height(200)
    });
    let row = |name: &str| by_role(Role::Row, name);
    app.expect(row("a.txt")).to_exist().await;

    assert_eq!(app.get(row("a.txt")).dragged_files(), [root.join("a.txt")]);
    selected.set(vec!["a.txt", "b.txt"]);
    app.settle().await;
    assert_eq!(app.get(row("b.txt")).dragged_files(), [root.join("a.txt"), root.join("b.txt")]);
}

mitsuami_test::main!();
