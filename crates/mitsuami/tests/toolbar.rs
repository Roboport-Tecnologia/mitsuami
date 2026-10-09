//! `Toolbar`: items in the bar across the top of a window, the one that
//! shows its title. The platform places them at the bar's trailing end;
//! the core sizes each to its content.

use mitsuami::core::WidgetKind;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// The window's toolbar items, as the core has them.
fn items(app: &TestApp) -> Vec<NodeId> {
    let ui = app.ui();
    ui.native_children(app.window()).into_iter().filter(|c| ui.kind(*c) == Some(WidgetKind::ToolbarItem)).collect()
}

#[mitsuami_test::test]
async fn its_items_are_above_the_content(app: TestApp) {
    app.mount(|| Column::new().children((Toolbar::new().child(Text::new("3 machines")), Text::new("Windows 98"))));

    app.expect(by_text("3 machines")).to_be_visible().await;
    app.expect(by_text("Windows 98")).to_be_visible().await;
    // Every platform's bar is above the window's content, which starts
    // at 0.
    let item = app.get_by_text("3 machines").frame();
    assert!(item.max_y() <= 0.0, "{item:?}");
    assert!(item.x() >= 0.0 && item.max_x() <= app.ui().window_size(app.window()).unwrap().width, "{item:?}");
}

/// The window's content keeps the whole size the core asked for: the bar
/// isn't taken out of it.
#[mitsuami_test::test]
async fn the_content_keeps_the_window_size(app: TestApp) {
    app.mount(|| {
        Column::new()
            .grow(1.0)
            .test_id("content")
            .children((Toolbar::new().child(Text::new("Ready")), Text::new("Body")))
    });

    let size = app.ui().window_size(app.window()).expect("sized");
    app.expect(by_test_id("content")).to_have_frame(Rect::new(0.0, 0.0, size.width, size.height)).await;
}

#[mitsuami_test::test]
async fn its_items_are_at_the_trailing_end_in_order(app: TestApp) {
    app.mount(|| Toolbar::new().children((Text::new("First"), Text::new("Second"))));

    let (first, second) = (app.get_by_text("First").frame(), app.get_by_text("Second").frame());
    assert!(first.max_x() <= second.x(), "{first:?} {second:?}");
    // Trailing: the last item is in the bar's second half.
    let width = app.ui().window_size(app.window()).unwrap().width;
    assert!(second.x() > width / 2.0, "{second:?} in {width}");
}

/// Every platform's bar centres its items, whatever their heights.
#[mitsuami_test::test]
async fn its_items_are_centred_in_it(app: TestApp) {
    app.mount(|| Toolbar::new().children((Text::new("Ready"), Button::new("Add"))));

    let (text, button) = (app.get_by_text("Ready").frame(), app.get_by_role(Role::Button, "Add").frame());
    assert!(text.height() < button.height(), "{text:?} {button:?}");
    let centre = |r: Rect| r.y() + r.height() / 2.0;
    assert!((centre(text) - centre(button)).abs() <= 1.0, "{text:?} {button:?}");
}

/// Each item is as big as what's in it, and follows it.
#[mitsuami_test::test]
async fn its_items_are_as_big_as_their_content(app: TestApp) {
    let status = signal("Ready".to_owned());
    app.mount(move || Toolbar::new().child(Text::new(status)));
    let item = items(&app)[0];
    let short = app.ui().frame(item).unwrap();
    assert_eq!(app.get_by_text("Ready").frame().size, short.size);

    status.set("Downloading presets".into());
    app.settle().await;
    let long = app.ui().frame(item).unwrap();
    assert_eq!(app.get_by_text("Downloading presets").frame().size, long.size);
    assert!(long.width() > short.width(), "{short:?} {long:?}");
}

/// An item with nothing to show is hidden, as 2ksbox's download status is
/// until a download runs.
#[mitsuami_test::test]
async fn an_empty_item_is_hidden(app: TestApp) {
    let busy = signal(false);
    app.mount(move || {
        Toolbar::new().children((
            Show::new(busy, || {
                Row::new().gap(Spacing::Sm).align(Align::Center).children((Spinner::new("Downloading"), "Downloading"))
            }),
            Text::new("Ready"),
        ))
    });
    app.expect(by_text("Downloading")).not_to_exist().await;
    app.expect(by_text("Ready")).to_be_visible().await;
    assert!(app.ui().frame(items(&app)[0]).unwrap().size.is_empty());

    busy.set(true);
    app.settle().await;
    app.expect(by_text("Downloading")).to_be_visible().await;
    app.expect(by_role(Role::ProgressBar, "Downloading")).to_be_visible().await;
    // In order. How close is the platform's: macOS 26 groups items in one
    // glass capsule, where their views overlap by a point or two.
    let (downloading, ready) = (app.get_by_text("Downloading").frame(), app.get_by_text("Ready").frame());
    assert!(downloading.x() < ready.x(), "{downloading:?} {ready:?}");

    busy.set(false);
    app.settle().await;
    app.expect(by_text("Downloading")).not_to_exist().await;
    app.expect(by_text("Ready")).to_be_visible().await;
}

#[mitsuami_test::test]
async fn its_controls_work(app: TestApp) {
    let count = signal(0);
    app.mount(move || {
        Column::new().children((
            Toolbar::new().child(Button::new("Add").on_click(move || count.update(|c| *c += 1))),
            Text::new(move || format!("{} machines", count.get())),
        ))
    });
    app.expect(by_role(Role::Button, "Add")).to_be_visible().await;

    app.get_by_role(Role::Button, "Add").click().await;
    app.get_by_role(Role::Button, "Add").click().await;
    app.expect(by_text("2 machines")).to_exist().await;
}

/// A row of buttons is one item: on macOS 26 one capsule, a segment per
/// button. Each button still does what it does, and follows its props.
#[mitsuami_test::test]
async fn a_row_of_buttons_is_one_item(app: TestApp) {
    let (shelf, shaders) = (signal(0), signal(0));
    let enabled = signal(true);
    app.mount(move || {
        Column::new().children((
            Toolbar::new().children((
                Button::new("New"),
                Row::new().children((
                    Button::new("Shelf").on_click(move || shelf.update(|c| *c += 1)),
                    Button::new("Shaders").enabled(enabled).on_click(move || shaders.update(|c| *c += 1)),
                )),
            )),
            Text::new(move || format!("{} {}", shelf.get(), shaders.get())),
        ))
    });
    assert_eq!(items(&app).len(), 2);
    app.expect(by_role(Role::Button, "Shaders")).to_be_visible().await;

    app.get_by_role(Role::Button, "Shelf").click().await;
    app.get_by_role(Role::Button, "Shaders").click().await;
    app.get_by_role(Role::Button, "Shaders").click().await;
    app.expect(by_text("1 2")).to_exist().await;

    enabled.set(false);
    app.settle().await;
    app.expect(by_role(Role::Button, "Shaders")).to_be_disabled().await;
}

/// An item with no control in it (a status, a progress bar) shows as the
/// others do, at its size: macOS 26 leaves it out of the glass.
#[mitsuami_test::test]
async fn an_item_without_controls_keeps_its_size(app: TestApp) {
    app.mount(|| Toolbar::new().children((Progress::new("Downloading").width(160), Button::new("Add"))));

    let bar = app.get_by_role(Role::ProgressBar, "Downloading").frame();
    assert_eq!(bar.width(), 160.0, "{bar:?}");
    assert_eq!(app.ui().frame(items(&app)[0]).unwrap().size, bar.size);
    assert!(bar.max_x() <= app.get_by_role(Role::Button, "Add").frame().x(), "{bar:?}");
}

/// Whether Tab reaches the toolbar is the platform's call: the core's
/// order is the content's.
#[mitsuami_test::test]
async fn its_controls_are_outside_the_contents_tab_order(app: TestApp) {
    app.mount(|| Column::new().children((Toolbar::new().child(Button::new("Add")), Button::new("Start"))));

    let order = app.ui().focus_order(app.window());
    assert_eq!(order, vec![app.get_by_role(Role::Button, "Start").id()]);
}

/// Declared in a part of the tree that goes away, its items go with it.
#[mitsuami_test::test]
async fn its_items_go_with_the_scope_that_declared_them(app: TestApp) {
    let shown = signal(true);
    app.mount(move || {
        Column::new().children((
            Text::new("Body"),
            Show::new(shown, || Toolbar::new().children((Text::new("Ready"), Button::new("Add")))),
        ))
    });
    assert_eq!(items(&app).len(), 2);

    shown.set(false);
    app.settle().await;
    assert!(items(&app).is_empty());
    app.expect(by_text("Ready")).not_to_exist().await;
    app.expect(by_text("Body")).to_be_visible().await;

    shown.set(true);
    app.settle().await;
    app.expect(by_text("Ready")).to_be_visible().await;
}

/// A window opened while the app runs has a toolbar of its own.
#[mitsuami_test::test]
async fn each_window_has_its_own(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Toolbar::new().child(Text::new("Launcher status")),
            Window::new("Machine").content(|| {
                Column::new().children((Toolbar::new().child(Text::new("Running")), Text::new("Settings")))
            }),
        ))
    });
    let machine = app.window_titled("Machine").expect("open");

    assert_eq!(app.ui().window_of(app.get_by_text("Running").id()), Some(machine));
    assert_eq!(app.ui().window_of(app.get_by_text("Launcher status").id()), Some(app.window()));
    assert!(app.get_by_text("Running").frame().max_y() <= 0.0);
}

mitsuami_test::main!();

/// A table at the top of a window with a toolbar starts there, its header
/// above its rows, and scrolls to its last row, on every platform. On
/// AppKit it runs up under the title bar and toolbar, as Finder's content
/// does, outside its frame, as beside a sidebar (`tests/sidebar.rs`).
#[mitsuami_test::test]
async fn a_table_at_the_contents_top_keeps_its_header_above_its_rows(app: TestApp) {
    let data = signal((0..100).collect::<Vec<u32>>());
    app.mount(move || {
        Column::new().grow(1.0).children((
            Toolbar::new().child(Button::new("Take snapshot")),
            Table::new(data, |n: &u32| *n)
                .column(TableColumn::new("Name", |n: u32| Text::new(format!("File {n}"))).expand())
                .grow(1.0)
                .test_id("table"),
        ))
    });
    app.settle().await;

    let table = app.get_by_test_id("table").frame();
    assert_eq!(table.y(), 0.0);
    let first = app.get_by_role(Role::Cell, "File 0").frame();
    assert!(first.y() > table.y() && first.max_y() < table.y() + 60.0, "{first:?} in {table:?}");
    app.get_by_test_id("table").scroll_by(0.0, 1e6).await;
    let last = app.get_by_role(Role::Cell, "File 99").frame();
    assert!(last.max_y() <= table.max_y() && last.max_y() > table.max_y() - 30.0, "{last:?} in {table:?}");
}
