//! `Sidebar`: the list down a window's leading side that picks what the
//! window shows. The platform draws, places and sizes it; the window's
//! content is what's beside it.

use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Page {
    General,
    WiFi,
    Bluetooth,
    Keyboard,
}

/// The pages, in two sections after one of their own, and the page shown.
fn settings(page: Signal<Page>) -> impl View {
    Column::new().grow(1.0).test_id("content").children((
        Sidebar::new(page).children((
            SidebarItem::new("General", Page::General),
            SidebarSection::new("Network")
                .children((SidebarItem::new("Wi-Fi", Page::WiFi), SidebarItem::new("Bluetooth", Page::Bluetooth))),
            SidebarSection::new("Devices").item(SidebarItem::new("Keyboard", Page::Keyboard)),
        )),
        Text::new(move || format!("{:?} settings", page.get())),
    ))
}

/// The window's sidebar, as the core has it.
fn sidebar(app: &TestApp) -> Option<NodeId> {
    let ui = app.ui();
    ui.native_children(app.window()).into_iter().find(|c| ui.kind(*c) == Some(WidgetKind::Sidebar))
}

/// The a11y tree's items and headings, as `Role name` lines.
fn outline(app: &TestApp) -> Vec<String> {
    let tree = app.a11y_tree();
    let Some(sidebar) = sidebar(app) else { return Vec::new() };
    tree.walk()
        .into_iter()
        .filter(|n| n.id == sidebar && n.role != Role::List)
        .map(|n| {
            format!(
                "{:?} {}{}",
                n.role,
                n.name.clone().unwrap_or_default(),
                if n.selected == Some(true) { " *" } else { "" }
            )
        })
        .collect()
}

#[mitsuami_test::test]
async fn its_items_read_as_a_list_under_their_sections_headings(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    assert_eq!(
        outline(&app),
        [
            "ListItem General *",
            "Heading Network",
            "ListItem Wi-Fi",
            "ListItem Bluetooth",
            "Heading Devices",
            "ListItem Keyboard",
        ]
    );
    app.expect(by_role(Role::ListItem, "Wi-Fi")).to_be_visible().await;
}

/// On the leading side of the content, which starts at 0, and as high as
/// it, less the platform's insets: on AppKit it's full height, under the
/// title bar, and macOS 26 floats it 8pt from the window's edges.
#[mitsuami_test::test]
async fn it_is_beside_the_content(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    let frame = app.ui().frame(sidebar(&app).expect("a sidebar")).unwrap();
    let size = app.ui().window_size(app.window()).unwrap();
    assert!(frame.width() > 0.0 && frame.max_x() <= 0.0, "{frame:?}");
    assert!(frame.y() <= 0.0 && frame.max_y() >= size.height - 16.0, "{frame:?} beside {size:?}");
}

/// The window's content keeps the size the app asked for: the sidebar is
/// added to the window, as a toolbar is.
#[mitsuami_test::test]
async fn the_content_keeps_the_window_size(app: TestApp) {
    let page = signal(Page::General);
    let before = app.ui().window_size(app.window()).unwrap();
    app.mount(move || settings(page));

    let size = app.ui().window_size(app.window()).unwrap();
    assert!((size.width - before.width).abs() < 1.0 && (size.height - before.height).abs() < 1.0, "{size:?}");
    app.expect(by_test_id("content")).to_have_frame(Rect::new(0.0, 0.0, size.width, size.height)).await;

    app.resize(640.0, 480.0).await;
    let size = app.ui().window_size(app.window()).unwrap();
    assert!((size.width - 640.0).abs() < 1.0 && (size.height - 480.0).abs() < 1.0, "{size:?}");
}

#[mitsuami_test::test]
async fn choosing_an_item_sets_the_selection(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    app.get_by_role(Role::ListItem, "Bluetooth").select().await;
    assert_eq!(page.get(), Page::Bluetooth);
    app.expect(by_text("Bluetooth settings")).to_be_visible().await;
    assert!(outline(&app).contains(&"ListItem Bluetooth *".to_owned()));

    // Across sections.
    app.get_by_role(Role::ListItem, "Keyboard").select().await;
    assert_eq!(page.get(), Page::Keyboard);
}

#[mitsuami_test::test]
async fn setting_the_selection_chooses_its_item(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    page.set(Page::WiFi);
    app.settle().await;
    assert!(outline(&app).contains(&"ListItem Wi-Fi *".to_owned()));
    assert_eq!(outline(&app).iter().filter(|l| l.ends_with('*')).count(), 1);
    app.expect(by_text("WiFi settings")).to_be_visible().await;
}

/// A toolbar's items stay over the content, not the sidebar: AppKit's
/// unified toolbar follows the sidebar's divider.
#[mitsuami_test::test]
async fn toolbar_items_are_over_the_content(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || Column::new().children((Toolbar::new().child(Text::new("Ready")), settings(page))));

    let item = app.get_by_text("Ready").frame();
    assert!(item.max_y() <= 0.0 && item.x() >= 0.0, "{item:?}");
}

/// The window's minimum is its content's, as without a sidebar.
#[mitsuami_test::test]
async fn the_minimum_size_is_the_contents(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || {
        Column::new().children((Sidebar::new(page).item(SidebarItem::new("General", Page::General)), Text::new("Body")))
    });
    app.ui().set_prop(app.window(), Prop::MinSize(Size::new(500.0, 400.0)));
    app.settle().await;

    app.resize(300.0, 200.0).await;
    let size = app.ui().window_size(app.window()).unwrap();
    assert!(size.width >= 499.0 && size.height >= 399.0, "{size:?}");
}

/// A value no item has chooses none, and choosing one sets it again.
#[mitsuami_test::test]
async fn a_value_no_item_has_chooses_none(app: TestApp) {
    let page = signal(Page::Keyboard);
    app.mount(move || {
        Sidebar::new(page).children((SidebarItem::new("General", Page::General), SidebarItem::new("Wi-Fi", Page::WiFi)))
    });
    assert!(outline(&app).iter().all(|l| !l.ends_with('*')), "{:?}", outline(&app));

    app.get_by_role(Role::ListItem, "Wi-Fi").select().await;
    assert_eq!(page.get(), Page::WiFi);
}

/// Titles take a signal, as a menu item's do.
#[mitsuami_test::test]
async fn titles_follow_their_values(app: TestApp) {
    let page = signal(Page::General);
    let title = signal("Wi-Fi".to_owned());
    app.mount(move || {
        Sidebar::new(page).children((SidebarItem::new("General", Page::General), SidebarItem::new(title, Page::WiFi)))
    });

    title.set("Wireless".into());
    app.settle().await;
    app.expect(by_role(Role::ListItem, "Wireless")).to_exist().await;
    app.get_by_role(Role::ListItem, "Wireless").select().await;
    assert_eq!(page.get(), Page::WiFi);
}

/// Declared in a part of the tree that goes away, it goes with it, and the
/// content keeps its size.
#[mitsuami_test::test]
async fn it_goes_with_the_scope_that_declared_it(app: TestApp) {
    let (page, shown) = (signal(Page::General), signal(true));
    app.mount(move || {
        Column::new().grow(1.0).children((
            Show::new(shown, move || Sidebar::new(page).item(SidebarItem::new("General", Page::General))),
            Text::new("Body"),
        ))
    });
    assert!(sidebar(&app).is_some());
    let size = app.ui().window_size(app.window()).unwrap();

    shown.set(false);
    app.settle().await;
    assert!(sidebar(&app).is_none());
    app.expect(by_role(Role::ListItem, "General")).not_to_exist().await;
    app.expect(by_text("Body")).to_be_visible().await;
    let now = app.ui().window_size(app.window()).unwrap();
    assert!((now.width - size.width).abs() < 1.0 && (now.height - size.height).abs() < 1.0, "{size:?} {now:?}");

    shown.set(true);
    app.settle().await;
    app.expect(by_role(Role::ListItem, "General")).to_be_visible().await;
}

/// It's on the leading side, and picks what the content shows: Tab
/// reaches it first.
#[mitsuami_test::test]
async fn it_is_first_in_the_tab_order(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || {
        Column::new()
            .children((Button::new("Apply"), Sidebar::new(page).item(SidebarItem::new("General", Page::General))))
    });

    let order = app.ui().focus_order(app.window());
    assert_eq!(order, vec![sidebar(&app).unwrap(), app.get_by_role(Role::Button, "Apply").id()]);
}

#[mitsuami_test::test]
async fn it_takes_focus(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    app.get_by_role(Role::ListItem, "General").focus().await;
    assert_eq!(app.ui().focused(app.window()), sidebar(&app));
}

/// A window opened while the app runs can have one of its own.
#[mitsuami_test::test]
async fn each_window_has_its_own(app: TestApp) {
    let (page, other) = (signal(Page::General), signal(Page::Keyboard));
    app.mount(move || {
        Column::new().children((
            Sidebar::new(page).item(SidebarItem::new("General", Page::General)),
            Window::new("Devices").content(move || {
                Column::new().children((
                    Sidebar::new(other).item(SidebarItem::new("Keyboard", Page::Keyboard)),
                    Text::new("Keys"),
                ))
            }),
        ))
    });
    let devices = app.window_titled("Devices").expect("open");

    assert_eq!(app.ui().window_of(app.get_by_role(Role::ListItem, "Keyboard").id()), Some(devices));
    assert_eq!(app.ui().window_of(app.get_by_role(Role::ListItem, "General").id()), Some(app.window()));
}

mitsuami_test::main!();
