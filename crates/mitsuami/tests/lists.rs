//! `List`: only the rows near the viewport are mounted; rows keep their
//! identity and state across data changes; selection, activation, keyboard
//! navigation and scrolling to a row.
//!
//! Rows here are 20px high, in a 100px list: 5 rows show, and a viewport's
//! worth on either side is mounted too. The suite runs natively as well,
//! so it only relies on sizes the rows set themselves.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, PartialEq, Debug)]
struct Item {
    id: u32,
    name: String,
}

fn items(n: u32) -> Vec<Item> {
    (0..n).map(|id| Item { id, name: format!("Item {id}") }).collect()
}

fn names(app: &TestApp) -> Vec<String> {
    app.a11y_tree().walk().into_iter().filter(|n| n.role == Role::ListItem).filter_map(|n| n.name.clone()).collect()
}

fn range(ids: std::ops::Range<u32>) -> Vec<String> {
    ids.map(|id| format!("Item {id}")).collect()
}

fn row(name: String, height: f32) -> Container {
    Container::new().height(height).child(Text::new(name))
}

fn simple_list(data: Signal<Vec<Item>>) -> List<Item, u32> {
    List::new(data, |i: &Item| i.id, |i| row(i.name, 20.0)).height(100).test_id("list")
}

/// Where a row is, leaving out its width: the platform's scroll bars may
/// take some from the rows.
fn place(app: &TestApp, name: &str) -> (f32, f32, f32) {
    let frame = app.get_by_role(Role::ListItem, name).frame();
    (frame.x(), frame.y(), frame.height())
}

#[mitsuami_test::test]
async fn only_the_rows_near_the_viewport_are_mounted(app: TestApp) {
    let data = signal(items(1000));
    app.mount(move || simple_list(data));
    app.settle().await;

    assert_eq!(names(&app), range(0..10), "the 5 rows in view, and 5 more below");
    assert!(app.get_by_role(Role::ListItem, "Item 4").is_visible());
    assert!(!app.get_by_role(Role::ListItem, "Item 5").is_visible());
    assert_eq!(place(&app, "Item 3"), (0.0, 60.0, 20.0));
}

#[mitsuami_test::test]
async fn scrolling_mounts_the_rows_coming_into_view_and_drops_the_rest(app: TestApp) {
    let data = signal(items(1000));
    app.mount(move || simple_list(data));
    let before = app.native_node_count();

    app.get_by_test_id("list").scroll_by(0.0, 400.0).await;

    assert_eq!(names(&app), range(15..30), "a viewport above, the rows in view, and a viewport below");
    assert!(app.get_by_role(Role::ListItem, "Item 20").is_visible());
    assert!(!app.get_by_role(Role::ListItem, "Item 19").is_visible());
    assert_eq!(app.get_by_role(Role::ListItem, "Item 20").frame().y(), 0.0, "in window coordinates");
    // Five more rows mounted, of three nodes each: host, container, text.
    assert_eq!(app.native_node_count(), before + 5 * 3);

    // To the end: the offset stops at the last row.
    app.get_by_test_id("list").scroll_by(0.0, 1e6).await;
    let list = app.get_by_test_id("list").id();
    assert_eq!(app.ui().scroll_offset(list), Some(Point::new(0.0, 19_900.0)));
    assert_eq!(names(&app), range(990..1000));
}

/// A row with local state (its checkbox) and a cleanup counter.
fn stateful_list(data: Signal<Vec<Item>>, disposed: Rc<Cell<u32>>) -> List<Item, u32> {
    List::new(
        data,
        |i: &Item| i.id,
        move |i| {
            let done = signal(false);
            let disposed = disposed.clone();
            on_cleanup(move || disposed.set(disposed.get() + 1));
            Row::new().height(20).child(Checkbox::new(i.name).bind(done))
        },
    )
    .height(100)
}

#[mitsuami_test::test]
async fn rows_keep_their_state_when_the_data_changes(app: TestApp) {
    let data = signal(items(20));
    let disposed = Rc::new(Cell::new(0));
    let d = disposed.clone();
    app.mount(move || stateful_list(data, d));
    app.get_by_role(Role::Checkbox, "Item 2").click().await;
    let checkbox = app.get_by_role(Role::Checkbox, "Item 2").id();

    data.update(|items| items.insert(0, Item { id: 100, name: "New".into() }));
    app.settle().await;

    let item_2 = app.get_by_role(Role::Checkbox, "Item 2");
    assert_eq!(item_2.id(), checkbox, "the same row, moved down");
    assert!(item_2.is_checked());
    assert_eq!(item_2.frame().y(), 60.0);
    assert_eq!(disposed.get(), 1, "one row pushed out of the mounted range");

    data.update(|items| items.retain(|i| i.id != 2));
    app.settle().await;
    app.expect(by_role(Role::Checkbox, "Item 2")).not_to_exist().await;
    assert_eq!(disposed.get(), 2);
}

#[mitsuami_test::test]
async fn unmounting_releases_every_row(app: TestApp) {
    let data = signal(items(1000));
    let disposed = Rc::new(Cell::new(0));
    let d = disposed.clone();
    app.mount(move || stateful_list(data, d));
    app.settle().await;

    app.unmount();

    assert_eq!(app.native_node_count(), 0);
    assert_eq!(disposed.get(), 10, "only mounted rows had state");
}

#[mitsuami_test::test]
async fn the_selection_binds_both_ways(app: TestApp) {
    let data = signal(items(1000));
    let selected = signal(Vec::<u32>::new());
    app.mount(move || simple_list(data).selected(selected));

    app.get_by_role(Role::ListItem, "Item 3").select().await;
    assert_eq!(selected.get(), [3]);
    assert!(app.get_by_role(Role::ListItem, "Item 3").node().selected == Some(true));

    selected.set(vec![1]);
    app.settle().await;
    assert!(app.get_by_role(Role::ListItem, "Item 1").node().selected == Some(true));
    assert!(app.get_by_role(Role::ListItem, "Item 3").node().selected == Some(false));

    // Removing a selected row deselects it, as native lists do.
    data.update(|items| items.retain(|i| i.id != 1));
    app.settle().await;
    assert_eq!(selected.get(), Vec::<u32>::new());
}

#[mitsuami_test::test]
async fn lists_without_a_selection_cant_select(app: TestApp) {
    let data = signal(items(10));
    app.mount(move || simple_list(data));
    app.settle().await;
    let row = app.get_by_role(Role::ListItem, "Item 1").id();
    assert_eq!(app.ui().perform(row, &A11yAction::Select), Err(mitsuami::core::ActionError::Unsupported));
}

#[mitsuami_test::test]
async fn activating_a_row_reports_its_key(app: TestApp) {
    let data = signal(items(1000));
    let activated = Rc::new(RefCell::new(Vec::new()));
    let a = activated.clone();
    app.mount(move || simple_list(data).on_activate(move |id| a.borrow_mut().push(id)));

    app.get_by_role(Role::ListItem, "Item 7").click().await;

    assert_eq!(*activated.borrow(), [7]);
}

#[mitsuami_test::test]
async fn the_keyboard_moves_the_selection_and_shows_it(app: TestApp) {
    let data = signal(items(1000));
    let selected = signal(Vec::<u32>::new());
    let activated = Rc::new(RefCell::new(Vec::new()));
    let a = activated.clone();
    app.mount(move || simple_list(data).selected(selected).on_activate(move |id| a.borrow_mut().push(id)));
    let list = app.get_by_test_id("list");

    list.press(Key::Down).await;
    list.press(Key::Down).await;
    assert_eq!(selected.get(), [1]);
    list.press(Key::Enter).await;
    assert_eq!(*activated.borrow(), [1]);

    assert!(list.is_focused());

    // Whether Home and End also select is the platform's call (AppKit's
    // lists only scroll); they always go to the end and back.
    list.press(Key::End).await;
    assert!(app.get_by_role(Role::ListItem, "Item 999").is_visible());
    list.press(Key::Home).await;
    assert!(app.get_by_role(Role::ListItem, "Item 0").is_visible());
}

#[mitsuami_test::test]
async fn a_handle_scrolls_to_rows_that_arent_mounted(app: TestApp) {
    let data = signal(items(1000));
    let handle = ListHandle::new();
    let h = handle.clone();
    app.mount(move || simple_list(data).handle(h));
    app.settle().await;

    handle.scroll_to(&500);
    app.settle().await;

    let row = app.get_by_role(Role::ListItem, "Item 500");
    assert!(row.is_visible());
    assert_eq!(row.frame().y(), 80.0, "scrolled just enough: it's the last row in view");
}

#[mitsuami_test::test]
async fn rows_are_as_high_as_their_content(app: TestApp) {
    // Every third row is twice as high.
    let data = signal(items(30));
    app.mount(move || {
        List::new(data, |i: &Item| i.id, |i| row(i.name, if i.id.is_multiple_of(3) { 40.0 } else { 20.0 }))
            .height(200)
            .test_id("list")
    });
    app.settle().await;

    assert_eq!(place(&app, "Item 0"), (0.0, 0.0, 40.0));
    assert_eq!(place(&app, "Item 1"), (0.0, 40.0, 20.0));
    assert_eq!(place(&app, "Item 3"), (0.0, 80.0, 40.0));

    // Scrolling to the end measures every row on the way.
    app.get_by_test_id("list").scroll_by(0.0, 1e6).await;
    let list = app.get_by_test_id("list").id();
    assert_eq!(app.ui().scroll_offset(list), Some(Point::new(0.0, 10.0 * 40.0 + 20.0 * 20.0 - 200.0)));
}

#[mitsuami_test::test]
async fn rows_mount_in_view_macro_form(app: TestApp) {
    let data = signal(items(100));
    let selected = signal(Vec::<u32>::new());
    app.mount(move || {
        view! {
            <List each=data key=|i: &Item| i.id selected=selected height=100 let:item>
                <Container height=20><Text>{item.name}</Text></Container>
            </List>
        }
    });
    app.get_by_role(Role::ListItem, "Item 2").select().await;
    assert_eq!(selected.get(), [2]);
    assert_eq!(names(&app), range(0..10));
}

#[mitsuami_test::test]
async fn the_a11y_tree_shows_a_list_of_rows(app: TestApp) {
    let data = signal(items(3));
    let selected = signal(vec![1u32]);
    app.mount(move || simple_list(data).selected(selected));
    app.assert_a11y_snapshot("list");
}

mitsuami_test::main!();
