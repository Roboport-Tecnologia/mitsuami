//! `Container::header`: the part at the top of a window's content that
//! stays while what's under it scrolls. On AppKit it goes in the title bar,
//! in the bar's glass, and the scroll view under it runs up under it; the
//! others leave it at the content's top.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A header with a count and a button, and 60 rows scrolling under it.
fn shelf(added: Signal<u32>, button: Tweak<Button>) -> impl View {
    Column::new().grow(1.0).min_height(0.0).children((
        Row::new().header(true).test_id("header").padding(12.0).gap(8.0).children((
            Text::new(move || format!("{} added", added.get())).grow(1.0),
            Button::new("Add").native(button).on_click(move || added.update(|n| *n += 1)),
        )),
        ScrollView::new()
            .child(Column::new().children((0..60).map(|i| Text::new(format!("Disc {i}"))).collect::<Vec<_>>()))
            .grow(1.0)
            .min_height(0.0)
            .test_id("discs"),
    ))
}

/// Where the core put it: the content's top, and what scrolls right under
/// it, on every platform. It stays there, and works, however far that
/// scrolled.
#[mitsuami_test::test]
async fn it_stays_above_what_scrolls_under_it(app: TestApp) {
    let added = signal(0u32);
    app.mount(move || shelf(added, Tweak::none()));
    app.settle().await;

    let header = app.get_by_test_id("header").frame();
    let discs = app.get_by_test_id("discs").frame();
    assert_eq!(header.y(), 0.0);
    assert!(header.height() > 0.0);
    assert_eq!(discs.y(), header.max_y(), "{discs:?} under {header:?}");

    app.get_by_test_id("discs").scroll_by(0.0, 1e6).await;
    app.expect(by_text("Disc 59")).to_be_visible().await;
    assert_eq!(app.get_by_test_id("header").frame(), header);
    app.get_by_role(Role::Button, "Add").click().await;
    app.expect(by_text("1 added")).to_be_visible().await;
}

/// On AppKit the header's controls are in the title bar, not the content
/// view, and the scroll view's content starts below them; turned off, the
/// header is back in the content.
#[mitsuami_test::test]
async fn on_appkit_it_is_in_the_title_bar(app: TestApp) {
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    #[cfg(target_os = "macos")]
    if app.backend_name() == "appkit" {
        use mitsuami::appkit::objc2::Message;
        use mitsuami::appkit::objc2::rc::Retained;
        use mitsuami::appkit::objc2_app_kit::{NSButton, NSView};
        use std::cell::RefCell;
        use std::rc::Rc;

        let seen: Rc<RefCell<Option<Retained<NSButton>>>> = Rc::default();
        let on = signal(true);
        let mounted = seen.clone();
        app.mount(move || {
            let seen = mounted.clone();
            let tweak = mitsuami::appkit::tweak(move |button: &NSButton| *seen.borrow_mut() = Some(button.retain()));
            Column::new().grow(1.0).min_height(0.0).children((
                Row::new()
                    .header(on)
                    .header_edge(HeaderEdge::Hard)
                    .padding(12.0)
                    .child(Button::new("Add").native(tweak)),
                ScrollView::new()
                    .child(Column::new().children((0..60).map(|i| Text::new(format!("Disc {i}"))).collect::<Vec<_>>()))
                    .grow(1.0)
                    .min_height(0.0),
            ))
        });
        app.settle().await;

        let button = seen.borrow().clone().expect("tweaked");
        let window = button.window().expect("in a window");
        let content: Retained<NSView> = window.contentView().expect("content");
        assert_eq!(window.titlebarAccessoryViewControllers().count(), 1);
        assert!(!button.isDescendantOf(&content), "the header is in the title bar");

        // Its edge is the hard one, where macOS has a choice (26.1).
        if let Some(class) = mitsuami::appkit::objc2::runtime::AnyClass::get(c"NSScrollEdgeEffectStyle") {
            use mitsuami::appkit::objc2::msg_send;
            use mitsuami::appkit::objc2::runtime::AnyObject;
            let accessory = window.titlebarAccessoryViewControllers().objectAtIndex(0);
            let shown: Retained<AnyObject> = unsafe { msg_send![&*accessory, preferredScrollEdgeEffectStyle] };
            let hard: Retained<AnyObject> = unsafe { msg_send![class, hardStyle] };
            assert!(std::ptr::eq(&*shown, &*hard), "the hard edge");
        }

        // A capture draws it where the core put it, and leaves it there.
        let image = app.ui().capture(app.window()).await.expect("captured");
        let rows = (button.frame().size.height as f32 * image.scale_factor) as usize;
        let header = &image.rgba[..image.width as usize * 4 * rows];
        assert!(header.chunks(4).any(|p| p != &header[..4]), "the capture shows the header");
        assert!(!button.isDescendantOf(&content), "the header is back in the title bar");
        assert_eq!(window.titlebarAccessoryViewControllers().count(), 1);

        on.set(false);
        app.settle().await;
        assert_eq!(window.titlebarAccessoryViewControllers().count(), 0);
        assert!(button.isDescendantOf(&content), "the header is back in the content");
    }
}

/// On GTK the header's controls are in the window's top bars, under the
/// header bar, in a toolbar view whose content is the rest; turned off, the
/// header is back in the content and the window has its own title bar again.
#[mitsuami_test::test]
async fn on_gtk_it_is_under_the_header_bar(app: TestApp) {
    #[cfg(not(all(target_os = "linux", not(feature = "kde"))))]
    let _ = app;
    #[cfg(all(target_os = "linux", not(feature = "kde")))]
    if app.backend_name() == "gtk" {
        use mitsuami::gtk::gtk;
        use mitsuami::gtk::gtk::glib;
        use mitsuami::gtk::gtk::prelude::*;
        use std::cell::RefCell;
        use std::rc::Rc;

        let seen: Rc<RefCell<Option<gtk::Button>>> = Rc::default();
        let on = signal(true);
        let mounted = seen.clone();
        app.mount(move || {
            let seen = mounted.clone();
            let tweak = mitsuami::gtk::tweak(move |button: &gtk::Button| *seen.borrow_mut() = Some(button.clone()));
            Column::new().grow(1.0).min_height(0.0).children((
                Row::new().header(on).padding(12.0).child(Button::new("Add").native(tweak)),
                ScrollView::new()
                    .child(Column::new().children((0..60).map(|i| Text::new(format!("Disc {i}"))).collect::<Vec<_>>()))
                    .grow(1.0)
                    .min_height(0.0)
                    .test_id("discs"),
            ))
        });
        app.settle().await;

        let toolbar_view = glib::Type::from_name("AdwToolbarView").expect("libadwaita");
        let button = seen.borrow().clone().expect("tweaked");
        let view = button.ancestor(toolbar_view).expect("the header is in a toolbar view");
        let content: gtk::Widget = view.property("content");
        assert!(!button.is_ancestor(&content), "the header is in the top bars");
        let discs = app.get_by_test_id("discs").frame();
        app.get_by_role(Role::Button, "Add").click().await;

        on.set(false);
        app.settle().await;
        assert!(button.ancestor(toolbar_view).is_none(), "the header is back in the content");
        assert_eq!(app.get_by_test_id("discs").frame(), discs, "what's under it stays where it was");
    }
}

mitsuami_test::main!();
