//! `Show` and keyed `For`: structure changes, identity, state and disposal.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, PartialEq, Debug)]
struct Item {
    id: u32,
    name: &'static str,
}

fn item(id: u32, name: &'static str) -> Item {
    Item { id, name }
}

/// A row with local state (its checkbox) and a cleanup counter.
fn row(item: Item, disposed: Rc<Cell<u32>>) -> impl View {
    let done = signal(false);
    on_cleanup(move || disposed.set(disposed.get() + 1));
    Row::new().gap(8).children((Text::new(item.name), Checkbox::new(format!("{} done", item.name)).bind(done)))
}

fn list(items: Signal<Vec<Item>>, disposed: Rc<Cell<u32>>) -> impl View {
    Column::new().child(For::new(items, |i: &Item| i.id, move |i| row(i, disposed.clone())))
}

fn texts(app: &TestApp) -> Vec<String> {
    app.a11y_tree().walk().into_iter().filter(|n| n.role == Role::StaticText).filter_map(|n| n.name.clone()).collect()
}

#[mitsuami_test::test]
async fn show_builds_and_disposes_its_branch(app: TestApp) {
    let open = signal(false);
    let disposed = Rc::new(Cell::new(0));
    let d = disposed.clone();
    app.mount(move || {
        Column::new().child(Show::new(open, move || {
            let d = d.clone();
            on_cleanup(move || d.set(d.get() + 1));
            Text::new("Details")
        }))
    });
    let baseline = app.native_node_count();

    open.set(true);
    app.expect(by_text("Details")).to_be_visible().await;
    assert_eq!(app.native_node_count(), baseline + 1);

    open.set(false);
    app.expect(by_text("Details")).not_to_exist().await;
    assert_eq!(disposed.get(), 1, "branch state was disposed");
    assert_eq!(app.native_node_count(), baseline, "native widget was destroyed");
}

#[mitsuami_test::test]
async fn show_renders_the_fallback_when_false(app: TestApp) {
    let logged_in = signal(false);
    app.mount(move || Show::new(logged_in, || Text::new("Welcome back")).fallback(|| Button::new("Log in")));
    app.expect(by_role(Role::Button, "Log in")).to_exist().await;

    logged_in.set(true);
    app.expect(by_text("Welcome back")).to_exist().await;
    app.expect(by_role(Role::Button, "Log in")).not_to_exist().await;
}

/// Several children take their places in the parent, as if written there:
/// laid out along its row, and all disposed together.
#[mitsuami_test::test]
async fn show_takes_several_children(app: TestApp) {
    let open = signal(true);
    app.mount(move || {
        view! {
            <Row gap=8>
                <Text>"Name"</Text>
                <Show when=open fallback=|| view! { <Text>"Nothing"</Text> }>
                    <Separator orientation=Orientation::Vertical/>
                    <Text>"Preview"</Text>
                </Show>
            </Row>
        }
    });
    assert_eq!(texts(&app), ["Name", "Preview"]);
    let (name, preview) = (app.get_by_text("Name").frame(), app.get_by_text("Preview").frame());
    assert!(preview.origin.x > name.origin.x + name.size.width, "not along the row: {name:?} and {preview:?}");

    open.set(false);
    app.expect(by_text("Preview")).not_to_exist().await;
    assert_eq!(texts(&app), ["Name", "Nothing"]);
    assert!(app.a11y_tree().walk().iter().all(|n| n.role != Role::Separator));
}

#[mitsuami_test::test]
async fn for_renders_items_in_order(app: TestApp) {
    let items = signal(vec![item(1, "a"), item(2, "b"), item(3, "c")]);
    app.mount(move || list(items, Rc::default()));
    assert_eq!(texts(&app), ["a", "b", "c"]);
}

#[mitsuami_test::test]
async fn reordering_keeps_row_identity_and_state(app: TestApp) {
    let items = signal(vec![item(1, "a"), item(2, "b"), item(3, "c")]);
    let disposed = Rc::new(Cell::new(0));
    let d = disposed.clone();
    app.mount(move || list(items, d));
    app.get_by_role(Role::Checkbox, "b done").check().await;
    let b_text = app.get_by_text("b").id();

    items.update(|v| v.reverse());
    app.settle().await;

    assert_eq!(texts(&app), ["c", "b", "a"]);
    assert_eq!(app.get_by_text("b").id(), b_text, "same native widget, moved");
    app.expect(by_role(Role::Checkbox, "b done")).to_be_checked().await;
    assert_eq!(disposed.get(), 0, "no row was rebuilt");
}

#[mitsuami_test::test]
async fn removed_rows_are_disposed_and_new_rows_rendered(app: TestApp) {
    let items = signal(vec![item(1, "a"), item(2, "b")]);
    let disposed = Rc::new(Cell::new(0));
    let d = disposed.clone();
    app.mount(move || list(items, d));
    let before = app.native_node_count();

    items.set(vec![item(2, "b"), item(4, "d")]);
    app.settle().await;

    assert_eq!(texts(&app), ["b", "d"]);
    assert_eq!(disposed.get(), 1);
    assert_eq!(app.native_node_count(), before, "one row out, one row in");
}

#[mitsuami_test::test]
async fn rows_are_laid_out_where_they_appear(app: TestApp) {
    let items = signal(vec![item(1, "first"), item(2, "second")]);
    app.mount(move || list(items, Rc::default()));

    items.update(|v| v.insert(0, item(0, "zeroth")));

    app.settle().await;
    let zeroth = app.get_by_role(Role::Checkbox, "zeroth done").frame();
    let first = app.get_by_role(Role::Checkbox, "first done").frame();
    let second = app.get_by_role(Role::Checkbox, "second done").frame();
    assert_eq!(app.get_by_text("zeroth").frame().y(), 0.0);
    assert!(zeroth.y() < first.y() && first.y() < second.y());
}

#[mitsuami_test::test]
async fn unmounting_releases_every_native_widget_and_all_state(app: TestApp) {
    let items = signal(vec![item(1, "a"), item(2, "b")]);
    let disposed = Rc::new(Cell::new(0));
    let d = disposed.clone();
    app.mount(move || list(items, d));

    app.unmount();

    assert_eq!(app.native_node_count(), 0);
    assert_eq!(disposed.get(), 2);
}

#[derive(Clone)]
struct Session {
    user: Signal<Option<String>>,
}

fn greeting() -> impl View {
    let session = inject::<Session>().expect("a Session is provided");
    Text::new(move || match session.user.get() {
        Some(user) => format!("Signed in as {user}"),
        None => "Signed out".to_string(),
    })
}

#[mitsuami_test::test]
async fn tests_can_provide_fake_stores(app: TestApp) {
    let user = signal(Some("ada".to_string()));
    app.provide(Session { user });
    app.mount(greeting);
    app.expect(by_text("Signed in as ada")).to_exist().await;

    user.set(None);
    app.expect(by_text("Signed out")).to_exist().await;
}

mitsuami_test::main!();
