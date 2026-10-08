//! `Sidebar`: the list down a window's leading side that picks what the
//! window shows. The platform draws, places and sizes it; the window's
//! content is what's beside it.

use std::cell::RefCell;
use std::rc::Rc;

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
/// added to the window, as a toolbar is. That needs a display with room
/// for both: Windows makes a window no wider than its display, so on a
/// 1024 × 768 one (CI's Windows runners) the 800 wide content beside
/// WinUI's open pane (320) gets 707.
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

/// A toolbar that comes after the sidebar goes between the content and
/// the sidebar, as the page of a sidebar app that has one.
#[mitsuami_test::test]
async fn a_toolbar_can_come_after_it(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || {
        Column::new().children((
            settings(page),
            Show::new(move || page.get() == Page::WiFi, || Toolbar::new().child(Text::new("Scanning"))),
        ))
    });

    app.get_by_role(Role::ListItem, "Wi-Fi").select().await;
    let item = app.get_by_text("Scanning").frame();
    assert!(item.max_y() <= 0.0 && item.x() >= 0.0, "{item:?}");
    app.get_by_role(Role::ListItem, "General").select().await;
    app.expect(by_text("Scanning")).to_be_hidden().await;
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
    if app.backend_name() == "gtk" {
        // GTK focuses a window's first control when it shows it, and focus
        // that enters a GTK list box selects the row it lands on: the first
        // item is chosen, as by the user, and the app hears it.
        assert_eq!(outline(&app), ["ListItem General *", "ListItem Wi-Fi"]);
        assert_eq!(page.get(), Page::General);
    } else {
        assert!(outline(&app).iter().all(|l| !l.ends_with('*')), "{:?}", outline(&app));
    }

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

/// Logs each run of the tweak, on the platform's own sidebar: whether it
/// could reach what sizes it (on Kirigami, the page row the window gives
/// the page; elsewhere, just that it ran).
fn log_runs(log: Rc<RefCell<Vec<bool>>>) -> Tweak<Sidebar<Page>> {
    platform! {
        macos => mitsuami::appkit::tweak(move |_: &mitsuami::appkit::objc2_app_kit::NSTableView| {
            log.borrow_mut().push(true)
        }),
        gtk => mitsuami::gtk::tweak(move |_: &mitsuami::gtk::gtk::ListBox| log.borrow_mut().push(true)),
        kde => mitsuami::kirigami::tweak(move |page: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(page.object("mitsuamiStack").is_some())
        }),
        windows => mitsuami::winui::tweak(move |_: &mitsuami::winui::bindings::NavigationView| {
            log.borrow_mut().push(true);
            Ok(())
        }),
    }
}

#[mitsuami_test::test]
async fn a_tweak_reaches_the_native_sidebar_in_its_window(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let tweak = log_runs(log.clone());
    let page = signal(Page::General);
    app.mount(move || {
        Column::new().grow(1.0).children((
            Sidebar::new(page).native(tweak).item(SidebarItem::new("General", Page::General)),
            Text::new("General settings"),
        ))
    });
    let props = app.ui().native_state(sidebar(&app).unwrap()).unwrap().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&true), "{:?}", log.borrow());
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

/// The pages, with the sidebar shown while `shown` is.
fn hideable(page: Signal<Page>, shown: Signal<bool>) -> impl View {
    Column::new().grow(1.0).children((
        Sidebar::new(page)
            .shown(shown)
            .children((SidebarItem::new("General", Page::General), SidebarItem::new("Wi-Fi", Page::WiFi))),
        Text::new(move || format!("{:?} settings", page.get())),
    ))
}

/// Whether a hidden sidebar is out of sight: libadwaita's split view shows
/// its sidebar whenever it isn't collapsed, so GTK hides it only in a
/// narrow window.
fn hides_when_wide(app: &TestApp) -> bool {
    app.backend_name() != "gtk"
}

/// The app hides it and shows it again: hidden, it takes no room beside
/// the content.
#[mitsuami_test::test]
async fn the_app_shows_and_hides_it(app: TestApp) {
    let (page, shown) = (signal(Page::General), signal(true));
    app.mount(move || hideable(page, shown));
    let id = sidebar(&app).expect("a sidebar");

    shown.set(false);
    app.settle().await;
    assert!(app.ui().native_state(id).unwrap().props.contains(&Prop::SidebarShown(false)));
    if hides_when_wide(&app) {
        assert!(app.ui().frame(id).unwrap().size.is_empty(), "{:?}", app.ui().frame(id));
    }

    shown.set(true);
    app.settle().await;
    assert!(app.ui().native_state(id).unwrap().props.contains(&Prop::SidebarShown(true)));
    assert!(app.ui().frame(id).unwrap().width() > 0.0);
    assert!(shown.get_untracked());
}

/// Hidden from the start, it's hidden once the window has it.
#[mitsuami_test::test]
async fn it_can_start_hidden(app: TestApp) {
    let (page, shown) = (signal(Page::General), signal(false));
    app.mount(move || hideable(page, shown));
    let id = sidebar(&app).expect("a sidebar");

    assert!(app.ui().native_state(id).unwrap().props.contains(&Prop::SidebarShown(false)));
    if hides_when_wide(&app) {
        assert!(app.ui().frame(id).unwrap().size.is_empty());
    }
}

/// The user hides it or shows it the platform's way: the app's signal
/// follows.
#[mitsuami_test::test(headless)]
async fn the_user_shows_and_hides_it_too(app: TestApp) {
    let (page, shown) = (signal(Page::General), signal(true));
    app.mount(move || hideable(page, shown));
    let id = sidebar(&app).expect("a sidebar");

    app.headless().set_sidebar_shown(id, false);
    app.settle().await;
    assert!(!shown.get_untracked());

    app.headless().set_sidebar_shown(id, true);
    app.settle().await;
    assert!(shown.get_untracked());
}

/// The pages again, with this beside the sidebar, at the content's top.
fn beside(page: Signal<Page>, content: impl View + 'static) -> impl View {
    Column::new().grow(1.0).children((
        Sidebar::new(page)
            .children((SidebarItem::new("General", Page::General), SidebarItem::new("Wi-Fi", Page::WiFi))),
        content,
    ))
}

fn numbered(n: u32) -> Vec<u32> {
    (0..n).collect()
}

/// A list at the top of the content starts there, and scrolls from its
/// first row to its last, on every platform. On AppKit it runs up under
/// the title bar and toolbar, as Finder's content does, outside its frame:
/// its rows still start below the bar and end on its bottom edge.
#[mitsuami_test::test]
async fn a_list_at_the_contents_top_scrolls_from_its_first_row_to_its_last(app: TestApp) {
    let page = signal(Page::General);
    let data = signal(numbered(100));
    app.mount(move || {
        beside(
            page,
            List::new(data, |n: &u32| *n, |n| Container::new().height(20).child(Text::new(format!("Item {n}"))))
                .grow(1.0)
                .test_id("list"),
        )
    });
    app.settle().await;

    let list = app.get_by_test_id("list").frame();
    assert_eq!(list.y(), 0.0);
    assert_eq!(app.get_by_role(Role::ListItem, "Item 0").frame().y(), list.y());
    app.get_by_test_id("list").scroll_by(0.0, 1e6).await;
    let last = app.get_by_role(Role::ListItem, "Item 99").frame();
    assert_eq!(last.max_y(), list.max_y());
    app.get_by_test_id("list").scroll_by(0.0, -1e6).await;
    assert_eq!(app.get_by_role(Role::ListItem, "Item 0").frame().y(), list.y());
}

/// A table there keeps its header at its top, and its rows below it.
#[mitsuami_test::test]
async fn a_table_at_the_contents_top_keeps_its_header_above_its_rows(app: TestApp) {
    let page = signal(Page::General);
    let data = signal(numbered(100));
    app.mount(move || {
        beside(
            page,
            Table::new(data, |n: &u32| *n)
                .column(TableColumn::new("Name", |n: u32| Text::new(format!("File {n}"))).expand())
                .grow(1.0)
                .test_id("table"),
        )
    });
    app.settle().await;

    let table = app.get_by_test_id("table").frame();
    let first = app.get_by_role(Role::Cell, "File 0").frame();
    assert!(first.y() > table.y() && first.max_y() < table.y() + 60.0, "{first:?} in {table:?}");
    app.get_by_test_id("table").scroll_by(0.0, 1e6).await;
    let last = app.get_by_role(Role::Cell, "File 99").frame();
    assert!(last.max_y() <= table.max_y() && last.max_y() > table.max_y() - 30.0, "{last:?} in {table:?}");
}

/// A scroll view there scrolls its content from its top to its end, as
/// far as it would anywhere else.
#[mitsuami_test::test]
async fn a_scroll_view_at_the_contents_top_scrolls_from_its_top_to_its_end(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || {
        let rows = (0..50).map(|_| Container::new().height(20)).collect::<Vec<_>>();
        beside(page, ScrollView::new().height(200).test_id("scroll").children(rows))
    });
    app.settle().await;

    let (scroll, id) = (app.get_by_test_id("scroll").frame(), app.get_by_test_id("scroll").id());
    assert_eq!(scroll.y(), 0.0);
    assert_eq!(app.ui().scroll_offset(id), Some(Point::new(0.0, 0.0)));
    app.get_by_test_id("scroll").scroll_by(0.0, 100.0).await;
    assert_eq!(app.ui().scroll_offset(id), Some(Point::new(0.0, 100.0)));
    app.get_by_test_id("scroll").scroll_by(0.0, 1e6).await;
    assert_eq!(app.ui().scroll_offset(id), Some(Point::new(0.0, 1000.0 - scroll.height())));
}

/// A sidebar of things rather than pages, as a VM manager's machines: a
/// subtitle each, a context menu each, and double-click to open one.
fn machines(
    names: Signal<Vec<&'static str>>,
    chosen: Signal<Option<&'static str>>,
    log: Rc<RefCell<Vec<String>>>,
) -> impl View {
    let (deleted, started) = (log.clone(), log);
    Sidebar::new(chosen)
        .children_with(move || {
            let deleted = deleted.clone();
            names
                .get()
                .into_iter()
                .map(|name| {
                    let deleted = deleted.clone();
                    SidebarItem::new(name, Some(name)).subtitle(format!("{name} is stopped")).context_menu((
                        MenuItem::new("Delete").on_select(move || deleted.borrow_mut().push(format!("delete {name}"))),
                        MenuItem::new("Rename").enabled(false),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .on_activate(move |name| started.borrow_mut().push(format!("start {}", name.unwrap_or_default())))
}

/// Items built by `children_with` follow what it reads, and the chosen
/// one stays chosen by its value when others come and go before it.
#[mitsuami_test::test]
async fn items_built_again_follow_their_data(app: TestApp) {
    let (names, chosen) = (signal(vec!["Alpha", "Beta"]), signal(Some("Beta")));
    app.mount(move || machines(names, chosen, Rc::default()));
    assert_eq!(outline(&app), ["ListItem Alpha", "ListItem Beta *"]);

    names.set(vec!["Aardvark", "Alpha", "Beta", "Gamma"]);
    app.settle().await;
    assert_eq!(outline(&app), ["ListItem Aardvark", "ListItem Alpha", "ListItem Beta *", "ListItem Gamma"]);

    names.set(vec!["Gamma"]);
    app.settle().await;
    assert_eq!(outline(&app), ["ListItem Gamma"]);
    assert_eq!(chosen.get(), Some("Beta"));

    app.get_by_role(Role::ListItem, "Gamma").select().await;
    assert_eq!(chosen.get(), Some("Gamma"));
}

/// An item's subtitle is its description to a screen reader, as a row's
/// second line is.
#[mitsuami_test::test]
async fn an_items_subtitle_describes_it(app: TestApp) {
    let (names, chosen) = (signal(vec!["Alpha", "Beta"]), signal(None));
    app.mount(move || machines(names, chosen, Rc::default()));

    let beta = app.get_by_role(Role::ListItem, "Beta").node();
    assert_eq!(beta.description.as_deref(), Some("Beta is stopped"));
}

/// Each item's context menu runs its own handlers, and a disabled item
/// can't be chosen.
#[mitsuami_test::test]
async fn an_items_context_menu_is_its_own(app: TestApp) {
    let (names, chosen, log) = (signal(vec!["Alpha", "Beta"]), signal(None), Rc::new(RefCell::new(Vec::new())));
    let shown = log.clone();
    app.mount(move || machines(names, chosen, shown.clone()));

    app.get_by_role(Role::ListItem, "Beta").choose_menu_item(&["Delete"]).await;
    app.get_by_role(Role::ListItem, "Alpha").choose_menu_item(&["Delete"]).await;
    assert_eq!(*log.borrow(), ["delete Beta", "delete Alpha"]);
    let (_, menu) = app.get_by_role(Role::ListItem, "Alpha").context_menu().expect("a menu");
    let rename = mitsuami::core::services::find_menu_item(&menu, &["Rename"]).expect("Rename");
    assert!(!rename.enabled);
}

/// A double-click on an item chooses it and activates it.
#[mitsuami_test::test]
async fn a_double_click_activates_an_item(app: TestApp) {
    let (names, chosen, log) = (signal(vec!["Alpha", "Beta"]), signal(None), Rc::new(RefCell::new(Vec::new())));
    let shown = log.clone();
    app.mount(move || machines(names, chosen, shown.clone()));

    app.get_by_role(Role::ListItem, "Beta").double_click().await;
    assert_eq!(chosen.get(), Some("Beta"));
    assert_eq!(*log.borrow(), ["start Beta"]);
}

mitsuami_test::main!();
