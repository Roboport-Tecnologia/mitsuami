//! `Tabs`: pages, one shown at a time, with a tab for each to pick it. The
//! platform draws the tab strip and places the pages; every page stays
//! mounted while it's hidden.

use mitsuami::core::backend::CaptureError;
use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Page {
    General,
    Network,
    Advanced,
}

/// Three pages of different sizes, the first with a field.
fn settings(page: Signal<Page>) -> Tabs<Page> {
    Tabs::new(page).children((
        Tab::new("General", Page::General).padding(8).gap(8).children((
            Text::new("General settings"),
            TextInput::new().placeholder("Name"),
            Button::new("Apply"),
        )),
        Tab::new("Network", Page::Network).padding(8).child(Text::new("Network settings")),
        Tab::new("Advanced", Page::Advanced)
            .padding(8)
            .children((Text::new("Advanced settings"), Checkbox::new("Verbose logging"))),
    ))
}

/// The tab view, as the core has it.
fn tabs(app: &TestApp) -> NodeId {
    let tree = app.a11y_tree();
    tree.walk().into_iter().find(|n| n.role == Role::TabGroup).expect("a tab view").id
}

/// Its pages' hosts, in order.
fn pages(app: &TestApp) -> Vec<NodeId> {
    app.ui().native_children(tabs(app))
}

/// The tabs, as `title` lines, the selected one starred.
fn strip(app: &TestApp) -> Vec<String> {
    let tree = app.a11y_tree();
    tree.walk()
        .into_iter()
        .filter(|n| n.role == Role::Tab)
        .map(|n| format!("{}{}", n.name.clone().unwrap_or_default(), if n.selected == Some(true) { " *" } else { "" }))
        .collect()
}

#[mitsuami_test::test]
async fn it_reads_as_its_tabs_then_the_page_shown(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    assert_eq!(strip(&app), ["General *", "Network", "Advanced"]);
    let group = app.a11y_tree().walk().into_iter().find(|n| n.role == Role::TabGroup).unwrap().clone();
    let roles: Vec<Role> = group.children.iter().map(|c| c.role).collect();
    assert_eq!(roles[..3], [Role::Tab, Role::Tab, Role::Tab]);
    app.expect(by_text("General settings")).to_be_visible().await;
    // The others aren't shown, so assistive technology doesn't see them.
    app.expect(by_text("Network settings")).not_to_exist().await;
    app.expect(by_text("Advanced settings")).not_to_exist().await;
}

#[mitsuami_test::test]
async fn picking_a_tab_shows_its_page(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    app.get_by_role(Role::Tab, "Advanced").select().await;
    assert_eq!(page.get(), Page::Advanced);
    assert_eq!(strip(&app), ["General", "Network", "Advanced *"]);
    app.expect(by_text("Advanced settings")).to_be_visible().await;
    app.expect(by_text("General settings")).not_to_exist().await;
}

#[mitsuami_test::test]
async fn setting_the_selection_shows_its_page(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    page.set(Page::Network);
    app.settle().await;
    assert_eq!(strip(&app), ["General", "Network *", "Advanced"]);
    app.expect(by_text("Network settings")).to_be_visible().await;
}

/// Hidden pages stay mounted, as every platform's tab view keeps them:
/// what's typed in one is still there when it's picked again.
#[mitsuami_test::test]
async fn hidden_pages_stay_mounted(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));
    let field = app.get_by_label("Name");
    let id = field.id();
    field.fill("Ada").await;

    app.get_by_role(Role::Tab, "Network").select().await;
    assert!(app.ui().exists(id));
    app.get_by_role(Role::Tab, "General").select().await;
    assert_eq!(app.get_by_label("Name").id(), id);
    app.expect(by_label("Name")).to_have_value("Ada").await;
}

/// Every platform's tab view shows a page: a value no tab has shows the
/// first, and leaves the selection as it is.
#[mitsuami_test::test]
async fn a_value_no_tab_has_shows_the_first(app: TestApp) {
    let page = signal(Page::Advanced);
    app.mount(move || {
        Tabs::new(page).children((
            Tab::new("General", Page::General).child(Text::new("General settings")),
            Tab::new("Network", Page::Network).child(Text::new("Network settings")),
        ))
    });

    assert_eq!(strip(&app), ["General *", "Network"]);
    app.expect(by_text("General settings")).to_be_visible().await;
    assert_eq!(page.get(), Page::Advanced);
}

/// The pages share one place inside the strip and border, all as big as
/// the biggest: the view doesn't change size from page to page.
#[mitsuami_test::test]
async fn every_page_is_as_big_as_the_biggest(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || Row::new().align(Align::Start).child(settings(page)));
    let view = app.ui().window_frame(tabs(&app)).unwrap();

    let mut shown = Vec::new();
    for title in ["General", "Network", "Advanced"] {
        app.get_by_role(Role::Tab, title).select().await;
        let page = pages(&app).into_iter().find(|p| app.ui().visible_rect(*p).is_some()).expect("a page shown");
        shown.push(app.ui().window_frame(page).unwrap());
        assert_eq!(app.ui().window_frame(tabs(&app)).unwrap(), view, "the view keeps its size");
    }
    assert!(shown.iter().all(|f| *f == shown[0]), "{shown:?}");
    let page = shown[0];
    // Inside the view, below its tab strip.
    assert!(page.x() >= view.x() && page.max_x() <= view.max_x(), "{page:?} in {view:?}");
    assert!(page.y() > view.y() && page.max_y() <= view.max_y(), "{page:?} in {view:?}");
    // The biggest page's content decides: the General page's field and
    // button make it the tallest.
    let field = app.get_by_label("Name");
    app.get_by_role(Role::Tab, "General").select().await;
    assert!(field.frame().max_y() <= page.max_y(), "{:?} in {page:?}", field.frame());
}

/// Only the page shown is shown: the others have no frame.
#[mitsuami_test::test]
async fn one_page_is_shown(app: TestApp) {
    let page = signal(Page::Network);
    app.mount(move || settings(page));

    let shown: Vec<bool> = pages(&app).iter().map(|p| app.ui().visible_rect(*p).is_some()).collect();
    assert_eq!(shown, [false, true, false]);
}

/// Grown, it gives its pages the room: they fill it.
#[mitsuami_test::test]
async fn its_pages_fill_it(app: TestApp) {
    let page = signal(Page::Network);
    app.mount(move || Column::new().grow(1.0).child(settings(page).grow(1.0)));

    let view = app.ui().window_frame(tabs(&app)).unwrap();
    let size = app.ui().window_size(app.window()).unwrap();
    assert!((view.height() - size.height).abs() < 1.0, "{view:?} in {size:?}");
    let shown = pages(&app)[1];
    let frame = app.ui().window_frame(shown).unwrap();
    let text = app.get_by_text("Network settings").frame();
    assert!(frame.height() > text.height() * 4.0, "{frame:?} for {text:?}");
    assert!(frame.width() > view.width() * 0.8, "{frame:?} in {view:?}");
}

/// A page as tall as its content no longer fits is cut to the view, as
/// the platform's tab view clips it: a scroll view in it scrolls.
#[mitsuami_test::test]
async fn a_page_taller_than_it_scrolls(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || {
        let rows: Vec<Container> = (0..50).map(|i| Container::new().height(20).test_id(format!("row{i}"))).collect();
        Column::new().child(
            Tabs::new(page).height(200).children((Tab::new("General", Page::General)
                .child(ScrollView::new().grow(1.0).min_height(0).test_id("scroller").children(rows)),)),
        )
    });

    let view = app.ui().window_frame(tabs(&app)).unwrap();
    let shown = app.ui().window_frame(pages(&app)[0]).unwrap();
    assert!(shown.max_y() <= view.max_y(), "{shown:?} in {view:?}");
    let scroller = app.get_by_test_id("scroller").frame();
    assert!(scroller.height() < 200.0, "{scroller:?}");
    assert!(scroller.max_y() <= view.max_y() + 0.5, "{scroller:?} in {view:?}");
}

/// However small its pages, it's as wide as its tab strip.
#[mitsuami_test::test]
async fn it_is_as_wide_as_its_tabs(app: TestApp) {
    let (short, long) = (signal(0), signal(0));
    app.mount(move || {
        Row::new().align(Align::Start).gap(8).children((
            Tabs::new(short).children((Tab::new("A", 0), Tab::new("B", 1))).test_id("short"),
            Tabs::new(long)
                .children((
                    Tab::new("General settings", 0),
                    Tab::new("Network and sharing", 1),
                    Tab::new("Advanced options", 2),
                ))
                .test_id("long"),
        ))
    });

    let frame = |id: &str| {
        let node = app.ui().inspect(app.window()).unwrap();
        fn find(node: &mitsuami::core::NodeInfo, id: &str) -> Option<Rect> {
            if node.test_id.as_deref() == Some(id) {
                return Some(node.frame);
            }
            node.children.iter().find_map(|c| find(c, id))
        }
        find(&node, id).unwrap()
    };
    let (short, long) = (frame("short"), frame("long"));
    assert!(long.width() > short.width() * 2.0, "{long:?} vs {short:?}");
}

/// Tab reaches the tab view, then the page shown: the hidden pages'
/// controls aren't in the order until their page is.
#[mitsuami_test::test]
async fn tab_reaches_only_the_page_shown(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    let apply = app.get_by_role(Role::Button, "Apply").id();
    let name = app.get_by_label("Name").id();
    assert_eq!(app.ui().focus_order(app.window()), [tabs(&app), name, apply]);

    app.get_by_role(Role::Tab, "Advanced").select().await;
    let verbose = app.get_by_role(Role::Checkbox, "Verbose logging").id();
    assert_eq!(app.ui().focus_order(app.window()), [tabs(&app), verbose]);
}

#[mitsuami_test::test]
async fn it_takes_focus(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    app.get_by_role(Role::Tab, "Network").focus().await;
    assert_eq!(app.ui().focused(app.window()), Some(tabs(&app)));
}

/// Titles take a signal.
#[mitsuami_test::test]
async fn titles_follow_their_values(app: TestApp) {
    let page = signal(Page::General);
    let title = signal("Network".to_owned());
    app.mount(move || {
        Tabs::new(page).children((
            Tab::new("General", Page::General).child(Text::new("General settings")),
            Tab::new(title, Page::Network).child(Text::new("Network settings")),
        ))
    });

    title.set("Wireless".into());
    app.settle().await;
    assert_eq!(strip(&app), ["General *", "Wireless"]);
    app.get_by_role(Role::Tab, "Wireless").select().await;
    assert_eq!(page.get(), Page::Network);
}

/// A page's node is a plain host: a `Container` under the tab view.
#[mitsuami_test::test]
async fn its_pages_are_hosts(app: TestApp) {
    let page = signal(Page::General);
    app.mount(move || settings(page));

    let kinds: Vec<_> = pages(&app).iter().map(|p| app.ui().kind(*p)).collect();
    assert_eq!(kinds, [Some(WidgetKind::Container); 3]);
}

/// Either way KDE shows tabs (Kirigami's navigation bar, the default, or
/// Qt's tab bar) picks and shows pages. The others show their own, and
/// keep the app's choice.
async fn picks_and_shows_pages_with(app: TestApp, style: TabsStyle) {
    let page = signal(Page::General);
    app.mount(move || settings(page).tabs_style(style));

    let props = app.ui().native_state(tabs(&app)).unwrap().props;
    assert!(props.contains(&Prop::TabsStyle(style)), "{props:?}");
    assert_eq!(strip(&app), ["General *", "Network", "Advanced"]);
    app.get_by_role(Role::Tab, "Advanced").select().await;
    assert_eq!(page.get(), Page::Advanced);
    app.expect(by_text("Advanced settings")).to_be_visible().await;
    page.set(Page::Network);
    app.settle().await;
    assert_eq!(strip(&app), ["General", "Network *", "Advanced"]);
    app.expect(by_text("Network settings")).to_be_visible().await;
}

#[mitsuami_test::test]
async fn a_tab_bar_picks_and_shows_pages(app: TestApp) {
    picks_and_shows_pages_with(app, TabsStyle::TabBar).await;
}

#[mitsuami_test::test]
async fn navigation_tabs_pick_and_show_pages(app: TestApp) {
    picks_and_shows_pages_with(app, TabsStyle::Navigation).await;
}

/// A style can change while the tab view shows: the page shown stays.
#[mitsuami_test::test]
async fn its_style_can_change(app: TestApp) {
    let page = signal(Page::Network);
    let style = signal(TabsStyle::Automatic);
    app.mount(move || settings(page).tabs_style(style));

    style.set(TabsStyle::TabBar);
    app.settle().await;
    assert_eq!(strip(&app), ["General", "Network *", "Advanced"]);
    app.expect(by_text("Network settings")).to_be_visible().await;
    let view = app.ui().window_frame(tabs(&app)).unwrap();
    let shown = pages(&app).into_iter().find(|p| app.ui().visible_rect(*p).is_some()).expect("a page shown");
    let page_frame = app.ui().window_frame(shown).unwrap();
    assert!(page_frame.y() > view.y() && page_frame.max_y() <= view.max_y(), "{page_frame:?} in {view:?}");
}

/// A field keeps the focus when the style changes, where the platform
/// makes a new view for it (GTK) or shows the same pages under another
/// strip (KDE), and the window is drawn: on GTK, a focused field moved to
/// the new view left the window never drawn.
#[mitsuami_test::test]
async fn a_new_style_keeps_the_focus(app: TestApp) {
    let page = signal(Page::General);
    let style = signal(TabsStyle::Navigation);
    app.mount(move || settings(page).tabs_style(style));
    app.get_by_label("Name").focus().await;
    app.expect(by_label("Name")).to_be_focused().await;

    style.set(TabsStyle::TabBar);
    app.settle().await;
    app.expect(by_label("Name")).to_be_focused().await;
    match app.ui().capture(app.window()).await {
        Err(CaptureError::Unsupported) => assert!(app.is_headless(), "only headless may lack capture"),
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(_) => {}
    }
}

/// A tab can have an icon, named in the platform's set; the platform shows
/// it where its tabs show one (AppKit's tab view shows only titles), and
/// the titles still name the tabs.
#[mitsuami_test::test]
async fn tabs_can_have_icons(app: TestApp) {
    let gear = platform! {
        macos => "gearshape",
        gtk => "emblem-system-symbolic",
        kde => "configure",
        windows => "\u{E713}",
        _ => "gear",
    };
    let page = signal(Page::General);
    app.mount(move || {
        Tabs::new(page).children((
            Tab::new("General", Page::General).icon(gear).child(Text::new("General settings")),
            Tab::new("Network", Page::Network).child(Text::new("Network settings")),
        ))
    });

    let props = app.ui().native_state(tabs(&app)).expect("a tab view").props;
    assert!(props.contains(&Prop::TabIcons(vec![gear.to_owned(), String::new()])), "{props:?}");
    assert_eq!(strip(&app), ["General *", "Network"]);
    app.get_by_role(Role::Tab, "Network").select().await;
    assert_eq!(page.get_untracked(), Page::Network);
}

/// Tabs hidden with what's around them, then shown again, are laid out as
/// they were, their tabs no narrower. macOS 26's tab strip (SwiftUI's segmented control in the
/// tab view) laid its tabs out for no room while hidden, and kept them
/// that narrow, their titles cut off, until another tab was picked
/// (SourceGit's commit details, under a page the rail hides,
/// 2026-10-23). Every platform keeps the tab picked.
#[mitsuami_test::test]
async fn tabs_shown_again_are_laid_out_as_they_were(app: TestApp) {
    use std::cell::RefCell;
    use std::rc::Rc;

    let page = signal(Page::General);
    let shown = signal(true);
    // A button on the first page, to find the tab view around it.
    #[cfg(target_os = "macos")]
    let seen: Rc<
        RefCell<Option<mitsuami::appkit::objc2::rc::Retained<mitsuami::appkit::objc2_app_kit::NSButton>>>,
    > = Rc::default();
    #[cfg(not(target_os = "macos"))]
    let seen: Rc<RefCell<Option<()>>> = Rc::default();
    let mounted = seen.clone();
    app.mount(move || {
        let _ = &mounted;
        let tweak = platform! {
            macos => {
                let seen = mounted.clone();
                mitsuami::appkit::tweak(move |button: &mitsuami::appkit::objc2_app_kit::NSButton| {
                    use mitsuami::appkit::objc2::Message;
                    *seen.borrow_mut() = Some(button.retain());
                })
            },
            _ => Tweak::none(),
        };
        let divider = signal(120.0);
        let tabs = Tabs::new(page).children((
            Tab::new("General", Page::General).child(Button::new("Apply").native(tweak)),
            Tab::new("Network", Page::Network).child(Text::new("Network settings")),
            Tab::new("Advanced", Page::Advanced).child(Text::new("Advanced settings")),
        ));
        // Under a history, in a split, beside a sidebar, under a header,
        // as SourceGit's details are.
        let sidebar = signal(160.0);
        let history = Column::new().hidden(move || !shown.get()).grow(1.0).align(Align::Stretch).child(
            Split::new().orientation(Orientation::Vertical).bind(divider).grow(1.0).children((
                ScrollView::new().child(Text::new("History")).grow(1.0).min_height(0),
                tabs.min_height(100),
            )),
        );
        Column::new().grow(1.0).align(Align::Stretch).children((
            Row::new().header(true).padding(8.0).child(Text::new("Toolbar")),
            Split::new().bind(sidebar).grow(1.0).min_height(0).children((
                Column::new().min_width(100).child(Text::new("Branches")),
                Column::new()
                    .min_width(100)
                    .align(Align::Stretch)
                    .children((history, Show::new(move || !shown.get(), || Text::new("Local changes")))),
            )),
        ))
    });
    app.settle().await;

    // The frames inside the tab view's strip, as the platform laid it out.
    #[cfg(target_os = "macos")]
    let strip_layout = || -> Vec<f64> {
        use mitsuami::appkit::objc2_app_kit::{NSSegmentedControl, NSTabView, NSView};
        fn walk(view: &NSView, inside: bool, out: &mut Vec<f64>) {
            let inside = inside || view.downcast_ref::<NSSegmentedControl>().is_some();
            if inside {
                out.push(view.frame().size.width);
            }
            for sub in view.subviews().iter() {
                walk(&sub, inside, out);
            }
        }
        let mut out = Vec::new();
        // Headless has no strip to look into.
        let Some(button) = seen.borrow().clone() else { return out };
        let mut view = unsafe { button.superview() };
        while let Some(v) = view {
            if v.downcast_ref::<NSTabView>().is_some() {
                // As laid out, not as it was when last drawn.
                v.layoutSubtreeIfNeeded();
                walk(&v, false, &mut out);
                break;
            }
            view = unsafe { v.superview() };
        }
        out
    };
    #[cfg(not(target_os = "macos"))]
    let strip_layout = Vec::<f64>::new;

    // Read once it's drawn, as the screen draws it.
    let _ = app.ui().capture(app.window()).await;
    let before = strip_layout();
    shown.set(false);
    app.settle().await;
    // Hidden a while, and drawn so, as a page the user left is.
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        app.settle().await;
    }
    let _ = app.ui().capture(app.window()).await;
    shown.set(true);
    app.settle().await;
    let _ = app.ui().capture(app.window()).await;
    assert_eq!(strip(&app), ["General *", "Network", "Advanced"]);
    // Its parts are back, none narrower than they were (SwiftUI may give
    // the tabs more of the strip's room after a hide, but never less).
    let after = strip_layout();
    assert_eq!(after.len(), before.len(), "the strip's parts: {before:?}, then {after:?}");
    assert!(after.iter().zip(&before).all(|(a, b)| a >= b), "the strip's widths: {before:?}, then {after:?}");
}

mitsuami_test::main!();
