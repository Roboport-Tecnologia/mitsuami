//! `List`: the rows the platform shows are mounted, and only those; rows
//! keep their identity and state across data changes; selection,
//! activation, keyboard navigation and scrolling to a row.
//!
//! Rows here are 20px high, in a 100px list: 5 rows show. Which others the
//! platform prepares is its business (headless prepares none, AppKit a
//! few, GTK about 200), so the suite checks what every platform does: the
//! rows in view are mounted, and rows far away aren't. It runs natively as well, so it only
//! relies on sizes the rows set themselves.

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

fn mounted(app: &TestApp, id: u32) -> bool {
    names(app).contains(&format!("Item {id}"))
}

/// The rows in view, top to bottom.
fn in_view(app: &TestApp) -> Vec<u32> {
    app.a11y_tree()
        .walk()
        .into_iter()
        .filter(|n| n.role == Role::ListItem && app.ui().visible_rect(n.id).is_some_and(|r| !r.size.is_empty()))
        .filter_map(|n| n.name.as_deref()?.strip_prefix("Item ")?.parse().ok())
        .collect()
}

/// Mounted, and in view.
fn shows(app: &TestApp, id: u32) -> bool {
    mounted(app, id) && app.get_by_role(Role::ListItem, format!("Item {id}")).is_visible()
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

    assert!((0..5).all(|id| shows(&app, id)), "the 5 rows in view");
    assert!(!shows(&app, 5));
    assert!(names(&app).len() < 250, "{} rows mounted", names(&app).len());
    assert!(!mounted(&app, 500));
    assert_eq!(place(&app, "Item 3"), (0.0, 60.0, 20.0));
}

#[mitsuami_test::test]
async fn items_sharing_a_key_each_get_a_row(app: TestApp) {
    // A key that comes twice gets a second row, as `For` does, and keeps it
    // through later updates instead of the two sharing one.
    let data = signal(vec![(1u32, "a"), (1, "b"), (2, "c")]);
    app.mount(move || {
        List::new(data, |i: &(u32, &'static str)| i.0, |i| Container::new().height(20).child(Text::new(i.1)))
            .height(100)
    });
    app.settle().await;
    assert_eq!(names(&app), ["a", "b", "c"]);
    data.set(vec![(1, "a"), (1, "b"), (2, "c"), (3, "d")]);
    app.settle().await;
    assert_eq!(names(&app), ["a", "b", "c", "d"]);
}

#[mitsuami_test::test]
async fn scrolling_mounts_the_rows_coming_into_view_and_drops_the_rest(app: TestApp) {
    let data = signal(items(1000));
    app.mount(move || simple_list(data));
    app.settle().await;

    // Far down: which rows exactly depends on the platform's guesses for
    // the rows it skipped (AppKit approximates long tables).
    app.get_by_test_id("list").scroll_by(0.0, 10_000.0).await;

    let rows = in_view(&app);
    assert!(rows.len() >= 5 && rows[0] > 400, "rows in view: {rows:?}");
    assert!(rows.windows(2).all(|w| w[1] == w[0] + 1), "rows in view: {rows:?}");
    let top = app.get_by_role(Role::ListItem, format!("Item {}", rows[0])).frame();
    assert!(top.y() <= 0.0 && top.y() + top.height() > 0.0, "the first covers the top edge, in window coordinates");
    // Far from the view, gone with its native widgets. (Not row 0: GTK
    // keeps its cursor row, and selected rows, bound wherever it scrolls.)
    assert!(!mounted(&app, 250), "gone, with its native widgets");

    // To the end: the last row sits on the list's bottom edge. (The offset
    // is the platform's: it depends on its guesses for the rows skipped.)
    app.get_by_test_id("list").scroll_by(0.0, 1e6).await;
    assert!((995..1000).all(|id| shows(&app, id)));
    let (list, last) = (app.get_by_test_id("list").frame(), app.get_by_role(Role::ListItem, "Item 999").frame());
    assert_eq!(last.y() + last.height(), list.y() + list.height());
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

    // The same row, with its state. (Where it is now is the platform's
    // call: GTK keeps the rows in view where they were.)
    let item_2 = app.get_by_role(Role::Checkbox, "Item 2");
    assert_eq!(item_2.id(), checkbox);
    assert!(item_2.is_checked());

    let before = disposed.get();
    data.update(|items| items.retain(|i| i.id != 2));
    app.settle().await;
    app.expect(by_role(Role::Checkbox, "Item 2")).not_to_exist().await;
    // Its row was disposed (with any others the platform let go).
    assert!(disposed.get() > before, "its row was disposed");
}

#[mitsuami_test::test]
async fn unmounting_releases_every_row(app: TestApp) {
    let data = signal(items(1000));
    let disposed = Rc::new(Cell::new(0));
    let d = disposed.clone();
    app.mount(move || stateful_list(data, d));
    app.settle().await;
    let rows = app.a11y_tree().walk().into_iter().filter(|n| n.role == Role::ListItem).count() as u32;
    assert!(rows < 250, "{rows} rows mounted");

    app.unmount();

    assert_eq!(app.native_node_count(), 0);
    assert_eq!(disposed.get(), rows, "every mounted row, and only those, had state");
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
async fn changing_the_selection_mode_keeps_what_it_can_hold(app: TestApp) {
    let data = signal(items(10));
    let selected = signal(vec![1]);
    let mode = signal(SelectionMode::Single);
    app.mount(move || simple_list(data).selected(selected).selection_mode(mode));
    app.settle().await;
    let is_selected = |id: u32| app.get_by_role(Role::ListItem, format!("Item {id}")).node().selected == Some(true);

    // A mode that holds more keeps the selection, except on WinUI: XAML
    // clears it on every mode change.
    mode.set(SelectionMode::Multiple);
    app.settle().await;
    let kept = selected.get();
    assert!(kept == [1] || kept.is_empty(), "kept {kept:?}");
    selected.set(vec![1, 3]);
    app.settle().await;
    assert!(is_selected(1) && is_selected(3));

    // One that holds fewer keeps what the platform keeps: AppKit the row
    // selected last, GTK, Qt and headless the first, WinUI none.
    mode.set(SelectionMode::Single);
    app.settle().await;
    let kept = selected.get();
    assert!(kept.is_empty() || kept == [1] || kept == [3], "kept {kept:?}");
    for id in [1, 3] {
        assert_eq!(is_selected(id), kept.contains(&id));
    }

    selected.set(vec![2]);
    mode.set(SelectionMode::None);
    app.settle().await;
    assert_eq!(selected.get(), Vec::<u32>::new());
    assert!(!is_selected(2));
    let row = app.get_by_role(Role::ListItem, "Item 2").id();
    assert_eq!(app.ui().perform(row, &A11yAction::Select), Err(mitsuami::core::ActionError::Unsupported));
}

#[mitsuami_test::test]
async fn activating_a_row_reports_its_key(app: TestApp) {
    let data = signal(items(1000));
    let activated = Rc::new(RefCell::new(Vec::new()));
    let a = activated.clone();
    app.mount(move || simple_list(data).on_activate(move |id| a.borrow_mut().push(id)));

    app.get_by_role(Role::ListItem, "Item 3").click().await;

    assert_eq!(*activated.borrow(), [3]);
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
    // Scrolled just enough: it's the last row in view (to within float
    // rounding, ten thousand points down).
    assert!((row.frame().y() - 80.0).abs() < 0.01, "at {}", row.frame().y());
}

#[mitsuami_test::test]
async fn a_handle_scrolls_to_rows_coming_in_the_same_change(app: TestApp) {
    let data = signal(items(10));
    let handle = ListHandle::new();
    let h = handle.clone();
    app.mount(move || simple_list(data).handle(h));
    app.settle().await;

    // Before the list's effect takes the items, as an app's effect
    // reacting to the same change can be.
    batch(|| {
        data.set(items(1000));
        handle.scroll_to(&500);
    });
    app.settle().await;
    let row = app.get_by_role(Role::ListItem, "Item 500");
    assert!(row.is_visible());
    assert!((row.frame().y() - 80.0).abs() < 0.01, "at {}", row.frame().y());
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

    // At the end, the last row sits on the list's bottom edge, however
    // high the platform guessed the rows it skipped.
    app.get_by_test_id("list").scroll_by(0.0, 1e6).await;
    let list = app.get_by_test_id("list").frame();
    let last = app.get_by_role(Role::ListItem, "Item 29").frame();
    assert_eq!((last.height(), last.y() + last.height()), (20.0, list.y() + list.height()));
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
    assert!((0..5).all(|id| shows(&app, id)));
}

#[mitsuami_test::test]
async fn the_a11y_tree_shows_a_list_of_rows(app: TestApp) {
    let data = signal(items(3));
    let selected = signal(vec![1u32]);
    app.mount(move || simple_list(data).selected(selected));
    app.assert_a11y_snapshot("list");
}

/// A frame takes room from the rows (on platforms that draw one), and rows
/// are laid out at what's left.
#[mitsuami_test::test]
async fn a_framed_lists_rows_fit_inside_it(app: TestApp) {
    let data = signal(items(3));
    app.mount(move || simple_list(data).list_style(ListStyle::Framed));
    app.settle().await;

    let list = app.get_by_test_id("list").frame();
    let row = app.get_by_role(Role::ListItem, "Item 0").frame();
    assert!(row.width() <= list.width(), "a {}px row in a {}px list", row.width(), list.width());
}

/// Logs how many rows the native list view has, each time the tweak runs.
fn log_rows(log: Rc<RefCell<Vec<usize>>>) -> Tweak<List> {
    platform! {
        macos => mitsuami::appkit::tweak(move |t: &mitsuami::appkit::objc2_app_kit::NSTableView| {
            log.borrow_mut().push(t.numberOfRows() as usize)
        }),
        gtk => mitsuami::gtk::tweak(move |v: &mitsuami::gtk::gtk::ListView| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(v.model().map_or(0, |m| m.n_items() as usize))
        }),
        kde => mitsuami::kirigami::tweak(move |v: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(v.int("count") as usize)),
        windows => mitsuami::winui::tweak(move |v: &mitsuami::winui::bindings::ListView| {
            use mitsuami::winui::windows_core::Interface;
            let items = v.cast::<mitsuami::winui::bindings::IItemsControl>()?.Items()?;
            log.borrow_mut().push(items.Size()? as usize);
            Ok(())
        }),
    }
}

/// The tweak runs on the list view (not the scroll view around it), after
/// the rows, and again when they change.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_list_view_after_its_rows(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let data = signal(items(3));
    let tweak = log_rows(log.clone());
    app.mount(move || simple_list(data).native(tweak));

    let props = app.get_by_test_id("list").native_state().props;
    assert!(props.iter().any(|p| matches!(p, mitsuami::core::Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&3));
    data.update(|d| d.push(Item { id: 3, name: "Item 3".into() }));
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&4));
}

/// WinUI's list view animates rows in; an app can turn that off for one
/// list, and the others keep theirs. The other platforms' lists don't
/// animate rows in.
#[mitsuami_test::test]
async fn winui_item_animations_can_be_turned_off(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (still, animated) = (signal(items(3)), signal(items(3)));
    let transitions = |log: Rc<RefCell<Vec<(&'static str, u32)>>>, list: &'static str, off: bool| -> Tweak<List> {
        platform! {
            windows => mitsuami::winui::tweak(move |v: &mitsuami::winui::bindings::ListView| {
                use mitsuami::winui::windows_core::Interface;
                if off {
                    mitsuami::winui::remove_item_animations(v)?;
                }
                let items = v.cast::<mitsuami::winui::bindings::IItemsControl>()?;
                log.borrow_mut().push((list, items.ItemContainerTransitions()?.Size()?));
                Ok(())
            }),
            _ => {
                let _ = (log, list, off);
                Tweak::none()
            }
        }
    };
    let (a, b) = (transitions(log.clone(), "still", true), transitions(log.clone(), "animated", false));
    app.mount(move || {
        Column::new().children((
            List::new(still, |i: &Item| i.id, |i| row(i.name, 20.0)).height(100).native(a),
            List::new(animated, |i: &Item| i.id, |i| row(i.name, 20.0)).height(100).native(b),
        ))
    });
    app.settle().await;
    if app.is_headless() || !cfg!(windows) {
        return;
    }
    // Read again once the lists are in the window, with their style's
    // transitions: the tweaks run again as the rows change.
    for list in [still, animated] {
        list.update(|d| d.push(Item { id: 3, name: "Item 3".into() }));
    }
    app.settle().await;
    let last = |list| log.borrow().iter().rev().find(|(l, _)| *l == list).map(|(_, n)| *n);
    assert_eq!(last("still"), Some(0));
    assert!(last("animated").is_some_and(|n| n > 0), "{:?}", log.borrow());
}

/// A list that grows taller than itself gets its vertical scroll bar. Where
/// the platform's scroll bars take room (AppKit's legacy scrollers, with no
/// trackpad), the rows get narrower though the list's frame doesn't change:
/// the mirror check after each settle compares the rows' frames.
#[mitsuami_test::test]
async fn rows_make_room_for_a_scroll_bar_that_comes_later(app: TestApp) {
    let data = signal(items(2));
    app.mount(move || simple_list(data));
    app.settle().await;
    data.set(items(50));
    app.settle().await;

    let list = app.get_by_test_id("list").frame();
    let row = app.get_by_role(Role::ListItem, "Item 0").frame();
    assert!(row.width() <= list.width(), "row {row:?} in list {list:?}");
}

/// Rows follow the list's width as the window gets narrower, as well as
/// wider: they never hold the list at a width it had.
#[mitsuami_test::test]
async fn rows_follow_the_list_as_the_window_narrows(app: TestApp) {
    let data = signal(items(3));
    app.mount(move || List::new(data, |i: &Item| i.id, |i| row(i.name, 20.0)).height(100).test_id("list"));
    app.resize(600.0, 300.0).await;
    app.resize(300.0, 300.0).await;

    let list = app.get_by_test_id("list").frame();
    let row = app.get_by_role(Role::ListItem, "Item 0").frame();
    let native = app.get_by_test_id("list").native_state().frame;
    assert!(list.width() <= 300.0, "list {list:?}");
    assert_eq!(native.size, list.size);
    assert!(row.width() <= list.width(), "row {row:?} in list {list:?}");
}

mitsuami_test::main!();
