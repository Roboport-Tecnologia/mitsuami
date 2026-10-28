//! `Tree`: rows that nest, opened and closed by the platform's own tree.
//! Rows show while their folders are open, one level below them; which
//! rows are open is the app's set, both ways; children can come later; and
//! a tree's rows are a list's for selection and activation.
//!
//! How far each platform indents its levels is its own, so the suite checks
//! that a row's content starts further in, and is narrower, than its
//! folder's. It runs natively as well.

use std::time::Duration;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, PartialEq, Debug)]
struct Node {
    name: &'static str,
    kids: Option<Vec<Node>>,
}

fn file(name: &'static str) -> Node {
    Node { name, kids: None }
}

fn folder(name: &'static str, kids: Vec<Node>) -> Node {
    Node { name, kids: Some(kids) }
}

/// src/ (main.rs, ui/ (button.rs, list.rs)), docs/ (), README.
fn project() -> Vec<Node> {
    vec![
        folder("src", vec![file("main.rs"), folder("ui", vec![file("button.rs"), file("list.rs")])]),
        folder("docs", vec![]),
        file("README"),
    ]
}

/// The tree's items that show, in order.
fn names(app: &TestApp) -> Vec<String> {
    app.a11y_tree().walk().into_iter().filter(|n| n.role == Role::TreeItem).filter_map(|n| n.name.clone()).collect()
}

fn row(name: &'static str) -> Container {
    Container::new().height(20).child(Text::new(name))
}

fn tree(roots: Signal<Vec<Node>>, open: Signal<Vec<&'static str>>) -> Tree<Node, &'static str> {
    Tree::new(roots, |n: &Node| n.name, |n| n.kids.clone(), |n| row(n.name))
        .expanded(open)
        .height(200)
        .width(300)
        .test_id("tree")
}

#[mitsuami_test::test]
async fn folders_show_closed_and_open_one_level_down(app: TestApp) {
    let (roots, open) = (signal(project()), signal(Vec::new()));
    app.mount(move || tree(roots, open));
    app.settle().await;
    assert_eq!(names(&app), ["src", "docs", "README"]);
    let src = app.get_by_role(Role::TreeItem, "src");
    app.expect(by_role(Role::TreeItem, "src")).to_be_collapsed().await;
    app.expect(by_role(Role::TreeItem, "src")).to_have_level(1).await;
    // A file never opens.
    assert_eq!(app.get_by_role(Role::TreeItem, "README").node().expanded, None);

    src.expand().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "docs", "README"]);
    app.expect(by_role(Role::TreeItem, "src")).to_be_expanded().await;
    app.expect(by_role(Role::TreeItem, "main.rs")).to_have_level(2).await;
    assert_eq!(open.get(), ["src"], "the user's opening is in the app's set");

    app.get_by_role(Role::TreeItem, "ui").expand().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "button.rs", "list.rs", "docs", "README"]);
    app.expect(by_role(Role::TreeItem, "list.rs")).to_have_level(3).await;

    // An empty folder opens, and shows nothing below it.
    app.get_by_role(Role::TreeItem, "docs").expand().await;
    app.expect(by_role(Role::TreeItem, "docs")).to_be_expanded().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "button.rs", "list.rs", "docs", "README"]);
}

#[mitsuami_test::test]
async fn closing_a_folder_drops_its_rows_and_keeps_its_folders_open(app: TestApp) {
    let (roots, open) = (signal(project()), signal(vec!["src", "ui"]));
    app.mount(move || tree(roots, open));
    app.settle().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "button.rs", "list.rs", "docs", "README"]);

    app.get_by_role(Role::TreeItem, "src").collapse().await;
    assert_eq!(names(&app), ["src", "docs", "README"]);
    assert_eq!(open.get(), ["ui"], "a folder inside stays in the set");

    // Reopened, the folder inside shows open again, as the app's set says.
    app.get_by_role(Role::TreeItem, "src").expand().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "button.rs", "list.rs", "docs", "README"]);
    app.expect(by_role(Role::TreeItem, "ui")).to_be_expanded().await;
}

#[mitsuami_test::test]
async fn the_app_opens_and_closes_rows_through_its_set(app: TestApp) {
    let (roots, open) = (signal(project()), signal(Vec::new()));
    app.mount(move || tree(roots, open));
    app.settle().await;

    open.set(vec!["src"]);
    app.settle().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "docs", "README"]);
    app.expect(by_role(Role::TreeItem, "src")).to_be_expanded().await;

    // A key of a row that doesn't show yet opens it once it does.
    open.set(vec!["src", "ui"]);
    app.settle().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "button.rs", "list.rs", "docs", "README"]);

    open.set(vec!["ui"]);
    app.settle().await;
    assert_eq!(names(&app), ["src", "docs", "README"]);
    app.expect(by_role(Role::TreeItem, "src")).to_be_collapsed().await;
}

#[mitsuami_test::test]
async fn children_that_come_later_show_in_their_open_folder(app: TestApp) {
    // A folder read as it opens: empty until its children come.
    let (read, open) = (signal(None::<Vec<Node>>), signal(Vec::new()));
    let roots = vec![folder("remote", vec![]), file("local")];
    app.mount(move || {
        Tree::new(
            roots.clone(),
            |n: &Node| n.name,
            move |n| match n.name {
                "remote" => Some(read.get().unwrap_or_default()),
                _ => n.kids.clone(),
            },
            |n| row(n.name),
        )
        .expanded(open)
        .height(200)
    });
    app.settle().await;

    app.get_by_role(Role::TreeItem, "remote").expand().await;
    assert_eq!(names(&app), ["remote", "local"]);
    read.set(Some(vec![folder("a", vec![file("c")]), file("b")]));
    app.settle().await;
    assert_eq!(names(&app), ["remote", "a", "b", "local"]);
    app.expect(by_role(Role::TreeItem, "b")).to_have_level(2).await;
    // A folder that came later opens too.
    app.expect(by_role(Role::TreeItem, "a")).to_be_collapsed().await;
    app.get_by_role(Role::TreeItem, "a").expand().await;
    assert_eq!(names(&app), ["remote", "a", "c", "b", "local"]);
    app.expect(by_role(Role::TreeItem, "remote")).to_be_expanded().await;
}

/// A tree whose "remote" folder is read as it opens, saying so until it's
/// read.
fn remote_tree(read: Signal<Option<Vec<Node>>>, open: Signal<Vec<&'static str>>) -> Tree<Node, &'static str> {
    let roots = vec![folder("remote", vec![]), folder("local", vec![file("a")])];
    Tree::new(
        roots,
        |n: &Node| n.name,
        move |n| match n.name {
            "remote" => Some(read.get().unwrap_or_default()),
            _ => n.kids.clone(),
        },
        |n| row(n.name),
    )
    .loading(move |n| n.name == "remote" && open.get().contains(&"remote") && read.get().is_none())
    .expanded(open)
    .height(200)
}

#[mitsuami_test::test]
async fn a_folder_being_read_shows_its_spinner_until_its_children_come(app: TestApp) {
    let (read, open) = (signal(None::<Vec<Node>>), signal(Vec::<&'static str>::new()));
    app.mount(move || remote_tree(read, open));
    app.settle().await;
    app.expect(by_role(Role::TreeItem, "remote")).not_to_be_busy().await;

    // The spinner waits half a second, so a quick read doesn't flash one.
    app.get_by_role(Role::TreeItem, "remote").expand().await;
    app.expect(by_role(Role::TreeItem, "remote")).to_be_expanded().await;
    app.advance(Duration::from_millis(400)).await;
    app.expect(by_role(Role::TreeItem, "remote")).not_to_be_busy().await;
    app.advance(Duration::from_millis(100)).await;
    app.expect(by_role(Role::TreeItem, "remote")).to_be_busy().await;
    app.expect(by_role(Role::TreeItem, "local")).not_to_be_busy().await;

    // Closed while it's read, and opened again: still read, and waits again.
    app.get_by_role(Role::TreeItem, "remote").collapse().await;
    app.expect(by_role(Role::TreeItem, "remote")).not_to_be_busy().await;
    app.get_by_role(Role::TreeItem, "remote").expand().await;
    app.expect(by_role(Role::TreeItem, "remote")).not_to_be_busy().await;
    app.advance(Duration::from_millis(500)).await;
    app.expect(by_role(Role::TreeItem, "remote")).to_be_busy().await;

    read.set(Some(vec![file("b")]));
    app.settle().await;
    assert_eq!(names(&app), ["remote", "b", "local"]);
    app.expect(by_role(Role::TreeItem, "remote")).not_to_be_busy().await;
}

#[mitsuami_test::test]
async fn a_folder_read_quickly_shows_no_spinner(app: TestApp) {
    let (read, open) = (signal(None::<Vec<Node>>), signal(Vec::<&'static str>::new()));
    app.mount(move || remote_tree(read, open));
    app.settle().await;
    app.get_by_role(Role::TreeItem, "remote").expand().await;
    app.advance(Duration::from_millis(300)).await;
    read.set(Some(vec![file("b")]));
    app.settle().await;
    app.advance(Duration::from_millis(300)).await;
    app.expect(by_role(Role::TreeItem, "remote")).not_to_be_busy().await;
    assert_eq!(names(&app), ["remote", "b", "local"]);
}

#[mitsuami_test::test]
async fn rows_further_down_start_further_in_and_are_narrower(app: TestApp) {
    let (roots, open) = (signal(project()), signal(vec!["src", "ui"]));
    app.mount(move || tree(roots, open));
    app.settle().await;
    let frame = |name| app.get_by_role(Role::TreeItem, name).frame();
    let (top, middle, bottom) = (frame("src"), frame("ui"), frame("list.rs"));
    assert!(top.x() < middle.x() && middle.x() < bottom.x(), "{top:?}, {middle:?}, {bottom:?}");
    assert!(top.width() > middle.width() && middle.width() > bottom.width(), "{top:?}, {middle:?}, {bottom:?}");
    // Each level adds the same.
    assert!(((middle.x() - top.x()) - (bottom.x() - middle.x())).abs() < 1.0, "{top:?}, {middle:?}, {bottom:?}");
    // The content still ends at the rows' end.
    assert!((top.x() + top.width() - (bottom.x() + bottom.width())).abs() < 1.0, "{top:?}, {bottom:?}");
}

#[mitsuami_test::test]
async fn closing_a_folder_deselects_its_rows(app: TestApp) {
    let (roots, open, selected) = (signal(project()), signal(vec!["src"]), signal(Vec::new()));
    app.mount(move || tree(roots, open).selected(selected));
    app.settle().await;

    app.get_by_role(Role::TreeItem, "main.rs").select().await;
    assert_eq!(selected.get(), ["main.rs"]);
    assert_eq!(app.get_by_role(Role::TreeItem, "main.rs").node().selected, Some(true));

    app.get_by_role(Role::TreeItem, "src").collapse().await;
    assert_eq!(selected.get(), Vec::<&str>::new());
}

#[mitsuami_test::test]
async fn rows_activate_without_opening(app: TestApp) {
    let (roots, open, activated) = (signal(project()), signal(Vec::new()), signal(Vec::new()));
    app.mount(move || tree(roots, open).on_activate(move |name| activated.update(|a| a.push(name))));
    app.settle().await;

    app.get_by_role(Role::TreeItem, "src").click().await;
    assert_eq!(activated.get(), ["src"]);
    assert_eq!(names(&app), ["src", "docs", "README"], "what a folder's activation does is the app's");
}

#[mitsuami_test::test]
async fn a_row_inside_itself_does_not_open(app: TestApp) {
    // A folder whose children hold a folder with its own key: the data
    // loops, and the tree stops at the second one.
    fn looped() -> Vec<Node> {
        vec![folder("a", vec![folder("a", vec![file("b")])])]
    }
    let (roots, open) = (signal(looped()), signal(vec!["a"]));
    app.mount(move || tree(roots, open));
    app.settle().await;
    assert_eq!(names(&app), ["a", "a"]);
}

/// The moves reported, as the handler got them.
type Moves = Signal<Vec<(Vec<&'static str>, RowDrop<&'static str>)>>;

fn movable(roots: Signal<Vec<Node>>, open: Signal<Vec<&'static str>>, moved: Moves) -> Tree<Node, &'static str> {
    tree(roots, open)
        .on_move(RowMoves::onto_rows().and_between_rows(), move |rows, to| moved.update(|m| m.push((rows, to))))
}

#[mitsuami_test::test]
async fn rows_dragged_land_in_a_folder_among_its_rows_or_at_the_end(app: TestApp) {
    let (roots, open, moved) = (signal(project()), signal(vec!["src", "ui"]), signal(Vec::new()));
    app.mount(move || movable(roots, open, moved));
    app.settle().await;
    let row = |name| app.get_by_role(Role::TreeItem, name);

    row("README").drag_onto(&row("docs")).await;
    row("README").drag_before(&row("list.rs")).await;
    row("main.rs").drag_to_end().await;
    assert_eq!(
        moved.get(),
        [
            (vec!["README"], RowDrop::On("docs")),
            (vec!["README"], RowDrop::Before("list.rs")),
            (vec!["main.rs"], RowDrop::End),
        ],
    );
    // Nothing moves until the app moves its data.
    assert_eq!(names(&app), ["src", "main.rs", "ui", "button.rs", "list.rs", "docs", "README"]);
}

/// Several rows, in different folders, stay selected as another folder
/// opens and closes.
#[mitsuami_test::test]
async fn several_rows_stay_selected_as_folders_open_and_close(app: TestApp) {
    let (roots, open, selected) = (signal(project()), signal(vec!["src"]), signal(Vec::new()));
    app.mount(move || tree(roots, open).selected(selected).selection_mode(SelectionMode::Multiple));
    app.settle().await;
    selected.set(vec!["main.rs", "README"]);
    app.settle().await;

    app.get_by_role(Role::TreeItem, "ui").expand().await;
    assert_eq!(selected.get(), ["main.rs", "README"]);
    app.get_by_role(Role::TreeItem, "ui").collapse().await;
    assert_eq!(selected.get(), ["main.rs", "README"]);
    assert_eq!(app.get_by_role(Role::TreeItem, "README").node().selected, Some(true));
    assert_eq!(app.get_by_role(Role::TreeItem, "main.rs").node().selected, Some(true));
}

/// A folder is selected on its own, as any row is: its rows aren't
/// selected with it, and its rows all selected don't select it. XAML's
/// tree view does both with its check boxes, so WinUI selects several rows
/// in its inner list instead.
#[mitsuami_test::test]
async fn a_folder_and_its_rows_are_selected_apart(app: TestApp) {
    let (roots, open, selected) = (signal(project()), signal(vec!["src", "ui"]), signal(Vec::new()));
    app.mount(move || tree(roots, open).selected(selected).selection_mode(SelectionMode::Multiple));
    app.settle().await;
    selected.set(vec!["ui"]);
    app.settle().await;
    assert_eq!(selected.get(), ["ui"]);
    assert_eq!(app.get_by_role(Role::TreeItem, "button.rs").node().selected, Some(false));

    selected.set(vec!["button.rs", "list.rs"]);
    app.settle().await;
    assert_eq!(selected.get(), ["button.rs", "list.rs"]);
    assert_eq!(app.get_by_role(Role::TreeItem, "ui").node().selected, Some(false));
}

#[mitsuami_test::test]
async fn a_selection_dragged_goes_in_the_trees_order(app: TestApp) {
    let (roots, open, moved, selected) =
        (signal(project()), signal(vec!["src"]), signal(Vec::new()), signal(Vec::new()));
    app.mount(move || movable(roots, open, moved).selected(selected).selection_mode(SelectionMode::Multiple));
    app.settle().await;
    selected.set(vec!["README", "main.rs"]);
    app.settle().await;
    app.get_by_role(Role::TreeItem, "README").drag_onto(&app.get_by_role(Role::TreeItem, "docs")).await;
    assert_eq!(moved.get(), [(vec!["main.rs", "README"], RowDrop::On("docs"))]);
}

#[mitsuami_test::test]
async fn a_folder_does_not_go_inside_itself(app: TestApp) {
    let (roots, open, moved) = (signal(project()), signal(vec!["src", "ui"]), signal(Vec::new()));
    app.mount(move || movable(roots, open, moved));
    app.settle().await;
    let row = |name| app.get_by_role(Role::TreeItem, name);

    row("src").drag_onto(&row("src")).await;
    row("src").drag_onto(&row("ui")).await;
    row("src").drag_before(&row("button.rs")).await;
    assert_eq!(moved.get(), []);
    // Out of its folder, or into another, it goes.
    row("ui").drag_onto(&row("docs")).await;
    assert_eq!(moved.get(), [(vec!["ui"], RowDrop::On("docs"))]);
}

#[mitsuami_test::test]
async fn only_folders_take_rows_onto_them_and_nothing_lands_on_the_rows_dragged(app: TestApp) {
    let (roots, open, moved) = (signal(project()), signal(vec!["src"]), signal(Vec::new()));
    app.mount(move || movable(roots, open, moved));
    app.settle().await;
    let row = |name| app.get_by_role(Role::TreeItem, name);

    // A file opens to nothing: rows go before or after it, not onto it.
    row("README").drag_onto(&row("main.rs")).await;
    // Over the row dragged, it would stay where it is.
    row("README").drag_before(&row("README")).await;
    assert_eq!(moved.get(), []);
    row("README").drag_before(&row("main.rs")).await;
    assert_eq!(moved.get(), [(vec!["README"], RowDrop::Before("main.rs"))]);
}

/// A tree the app doesn't size is as high as the rows that show: opening a
/// folder makes it higher, closing it brings it back. How high each
/// platform's rows are, around the app's, is its own.
#[mitsuami_test::test]
async fn opening_and_closing_rows_changes_a_trees_height(app: TestApp) {
    let (roots, open) = (signal(project()), signal(Vec::new()));
    app.mount(move || {
        Column::new().children((
            Tree::new(roots, |n: &Node| n.name, |n| n.kids.clone(), |n| row(n.name))
                .expanded(open)
                .width(300)
                .test_id("tree"),
            Text::new("Below").test_id("below"),
        ))
    });
    app.settle().await;
    let closed = app.get_by_test_id("tree").frame();
    assert!(closed.height() >= 60.0, "three rows in {closed:?}");
    assert_eq!(app.get_by_test_id("below").frame().y(), closed.max_y());

    open.set(vec!["src"]);
    app.settle().await;
    assert_eq!(names(&app), ["src", "main.rs", "ui", "docs", "README"]);
    let opened = app.get_by_test_id("tree").frame();
    assert!(opened.height() >= closed.height() + 40.0, "open in {opened:?}, closed in {closed:?}");
    assert_eq!(app.get_by_test_id("below").frame().y(), opened.max_y());

    open.set(Vec::new());
    app.settle().await;
    assert_eq!(app.get_by_test_id("tree").frame().height(), closed.height());
}

/// A row's content ends where the platform's row does, at every level: the
/// rows' width is where a row's content ends in the platform's item, not
/// the tree's width (WinUI's `TreeViewItem` keeps room at its sides, and
/// rows laid out as wide as the tree had their end cut off).
#[mitsuami_test::test]
async fn rows_end_inside_the_tree(app: TestApp) {
    let (roots, open) = (signal(project()), signal(vec!["src", "ui"]));
    app.mount(move || tree(roots, open));
    app.settle().await;
    let tree = app.get_by_test_id("tree").frame();
    for name in ["src", "main.rs", "ui", "button.rs", "docs", "README"] {
        let row = app.get_by_role(Role::TreeItem, name).frame();
        assert!(row.max_x() <= tree.max_x(), "{name} at {row:?} in {tree:?}");
    }
}

/// A tree on a tab that isn't chosen shows its rows when its tab is: the
/// platform lays it out first then, on its own (WinUI applies the tree
/// view's template, and realises its rows, only once its page shows, and
/// they'd stayed empty: SourceGit's changes, 2026-10-27).
#[mitsuami_test::test]
async fn a_tree_on_a_hidden_tab_shows_its_rows_when_the_tab_does(app: TestApp) {
    let (roots, open, page) = (signal(project()), signal(Vec::new()), signal(0u8));
    app.mount(move || {
        Tabs::new(page).height(300).children((
            Tab::new("Information", 0).child(Text::new("A commit")),
            Tab::new("Changes", 1).child(tree(roots, open)),
        ))
    });
    app.settle().await;
    page.set(1);
    app.settle().await;
    assert_eq!(names(&app), ["src", "docs", "README"]);
    for name in ["src", "docs", "README"] {
        app.expect(by_role(Role::TreeItem, name)).to_be_visible().await;
    }
}

mitsuami_test::main!();
