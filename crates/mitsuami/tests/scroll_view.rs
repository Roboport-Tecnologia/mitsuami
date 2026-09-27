//! ScrollView options: scroll bars, shown as the platform shows them or
//! not at all, and raw platform settings. How scroll views lay out and
//! scroll is in the conformance suite.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{Prop, ScrollAxes};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn rows(count: usize) -> Vec<Container> {
    (0..count).map(|i| Container::new().height(20).test_id(format!("row{i}"))).collect()
}

fn shows(app: &TestApp, prop: Prop) -> bool {
    app.get_by_test_id("scroller").native_state().props.contains(&prop)
}

/// Without scroll bars, it still scrolls, along the same axes.
#[mitsuami_test::test]
async fn without_scroll_bars_it_still_scrolls(app: TestApp) {
    app.mount(|| ScrollView::new().scroll_bars(false).height(100).test_id("scroller").children(rows(20)));
    assert!(shows(&app, Prop::ScrollBars(false)));
    assert!(shows(&app, Prop::ScrollAxes(ScrollAxes::Vertical)));

    app.get_by_test_id("scroller").scroll_by(0.0, 50.0).await;

    let scroller = app.get_by_test_id("scroller").id();
    assert_eq!(app.ui().scroll_offset(scroller), Some(Point::new(0.0, 50.0)));
}

#[mitsuami_test::test]
async fn a_horizontal_scroll_view_without_scroll_bars_scrolls_sideways_only(app: TestApp) {
    app.mount(|| {
        ScrollView::horizontal()
            .scroll_bars(false)
            .width(100)
            .height(40)
            .test_id("scroller")
            .children((0..10).map(|_| Container::new().width(30)).collect::<Vec<_>>())
    });
    assert!(shows(&app, Prop::ScrollAxes(ScrollAxes::Horizontal)));

    app.get_by_test_id("scroller").scroll_by(1000.0, 1000.0).await;

    let scroller = app.get_by_test_id("scroller").id();
    assert_eq!(app.ui().scroll_offset(scroller), Some(Point::new(200.0, 0.0)));
}

/// Scroll bars come and go, and the scroll view keeps its offset.
#[mitsuami_test::test]
async fn scroll_bars_follow_their_signal(app: TestApp) {
    let bars = signal(true);
    app.mount(move || ScrollView::new().scroll_bars(bars).height(100).test_id("scroller").children(rows(20)));
    assert!(shows(&app, Prop::ScrollBars(true)));
    app.get_by_test_id("scroller").scroll_by(0.0, 30.0).await;

    bars.set(false);
    app.settle().await;
    assert!(shows(&app, Prop::ScrollBars(false)));
    assert!(shows(&app, Prop::ScrollAxes(ScrollAxes::Vertical)));
    let scroller = app.get_by_test_id("scroller").id();
    assert_eq!(app.ui().scroll_offset(scroller), Some(Point::new(0.0, 30.0)));

    bars.set(true);
    app.settle().await;
    assert!(shows(&app, Prop::ScrollBars(true)));
}

/// Logs whether the native scroll view shows a vertical scroll bar, each
/// time the tweak runs.
fn log_bars(log: Rc<RefCell<Vec<bool>>>) -> Tweak<ScrollView> {
    platform! {
        macos => mitsuami::appkit::tweak(move |s: &mitsuami::appkit::objc2_app_kit::NSScrollView| {
            log.borrow_mut().push(s.hasVerticalScroller())
        }),
        gtk => mitsuami::gtk::tweak(move |s: &mitsuami::gtk::gtk::ScrolledWindow| {
            log.borrow_mut().push(s.policy().1 == mitsuami::gtk::gtk::PolicyType::Automatic)
        }),
        kde => mitsuami::kirigami::tweak(move |s: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(s.bool("mitsuamiBars"))
        }),
        windows => mitsuami::winui::tweak(move |s: &mitsuami::winui::bindings::ScrollViewer| {
            use mitsuami::winui::bindings::{IScrollViewer, ScrollBarVisibility};
            use mitsuami::winui::windows_core::Interface;
            let visibility = s.cast::<IScrollViewer>()?.VerticalScrollBarVisibility()?;
            log.borrow_mut().push(visibility != ScrollBarVisibility::Hidden);
            Ok(())
        }),
    }
}

/// Given before the scroll bars, the tweak still runs after them, and again
/// when they change.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_scroll_view_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let bars = signal(false);
    let tweak = log_bars(log.clone());
    app.mount(move || {
        ScrollView::new().native(tweak).scroll_bars(bars).height(100).test_id("scroller").children(rows(20))
    });

    assert!(app.get_by_test_id("scroller").native_state().props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&false));
    bars.set(true);
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&true));
}

mitsuami_test::main!();
