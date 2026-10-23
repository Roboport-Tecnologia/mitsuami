//! `Split`: two panes with a divider the user drags, as the platform splits
//! a window (an `NSSplitView`, a `GtkPaned`, Qt's `SplitView`, a grip on
//! the panes' edge on Windows). The core lays the panes out around the
//! platform's divider, whose thickness differs per platform: tests read it
//! from the metrics, and compare the rest.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::AnyView;
use mitsuami::core::Prop;
use mitsuami::core::backend::CaptureError;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn divider(app: &TestApp) -> f32 {
    app.ui().metrics().split_divider
}

fn pane(id: &str) -> Container {
    Column::new().child(Text::new(id)).test_id(id)
}

#[mitsuami_test::test]
async fn lays_out_the_first_pane_at_its_position_and_the_second_beside_it(app: TestApp) {
    app.mount(|| Split::new().position(200.0).test_id("split").children((pane("first"), pane("second"))));
    app.expect(by_text("second")).to_be_visible().await;

    let split = app.get_by_test_id("split").frame();
    let (first, second) = (app.get_by_test_id("first").frame(), app.get_by_test_id("second").frame());
    assert_eq!(first.width(), 200.0);
    assert_eq!(second.x() - split.x(), 200.0 + divider(&app));
    assert_eq!(second.width(), split.width() - 200.0 - divider(&app));
    // Both fill it across.
    assert_eq!((first.height(), second.height()), (split.height(), split.height()));
}

#[mitsuami_test::test]
async fn stacks_the_panes_when_vertical(app: TestApp) {
    app.mount(|| {
        Split::new()
            .orientation(Orientation::Vertical)
            .position(120.0)
            .test_id("split")
            .children((pane("first"), pane("second")))
    });
    app.expect(by_text("second")).to_be_visible().await;

    let split = app.get_by_test_id("split").frame();
    let (first, second) = (app.get_by_test_id("first").frame(), app.get_by_test_id("second").frame());
    assert_eq!(first.height(), 120.0);
    assert_eq!(second.y() - split.y(), 120.0 + divider(&app));
    assert_eq!((first.width(), second.width()), (split.width(), split.width()));
}

/// Stacked in a column it grows in, the second pane takes the height the
/// first leaves, whatever it is: a column, or tabs (SourceGit's commit
/// details under its history, 2026-10-23).
async fn stacked_second_pane_takes_the_rest(app: TestApp, tabs: bool) {
    let page = signal(0u32);
    app.mount(move || {
        let second: AnyView = match tabs {
            false => AnyView::new(pane("second").min_height(28)),
            true => AnyView::new(
                Tabs::new(page)
                    .children(Tab::new("Information", 0u32).child(Text::new("second")))
                    .min_height(28)
                    .test_id("second"),
            ),
        };
        Column::new().grow(1.0).align(Align::Stretch).child(
            Split::new()
                .orientation(Orientation::Vertical)
                .position(120.0)
                .grow(1.0)
                .min_height(0)
                .test_id("split")
                .children((pane("first").min_height(100), second)),
        )
    });
    app.resize(600.0, 500.0).await;
    let split = app.get_by_test_id("split").frame();
    let second = app.get_by_test_id("second").frame();
    assert_eq!(split.height(), 500.0, "the split fills the column");
    assert_eq!(second.max_y(), split.max_y(), "{second:?} in {split:?}");
}

#[mitsuami_test::test]
async fn stacked_in_a_column_a_second_pane_takes_the_rest(app: TestApp) {
    stacked_second_pane_takes_the_rest(app, false).await;
}

#[mitsuami_test::test]
async fn stacked_in_a_column_tabs_take_the_rest(app: TestApp) {
    stacked_second_pane_takes_the_rest(app, true).await;
}

/// The first pane keeps its size as the window grows, as a sidebar does;
/// the second takes the room.
#[mitsuami_test::test]
async fn the_second_pane_takes_the_room_the_window_gains(app: TestApp) {
    app.mount(|| Split::new().position(200.0).children((pane("first"), pane("second"))));
    app.resize(800.0, 500.0).await;
    let before = app.get_by_test_id("second").frame().width();

    app.resize(1000.0, 500.0).await;
    assert_eq!(app.get_by_test_id("first").frame().width(), 200.0);
    assert_eq!(app.get_by_test_id("second").frame().width(), before + 200.0);
}

/// Dragging the divider is the user's: reported, and the panes follow.
#[mitsuami_test::test]
async fn moving_the_divider_reports_where_it_went(app: TestApp) {
    let position = signal(200.0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        Split::new()
            .bind(position)
            .on_change(move |p| log.borrow_mut().push(p))
            .test_id("split")
            .children((pane("first"), pane("second")))
    });
    app.expect(by_text("second")).to_be_visible().await;

    app.get_by_test_id("split").set_number(300.0).await;
    assert_eq!(*seen.borrow(), [300.0]);
    assert_eq!(position.get_untracked(), 300.0);
    assert_eq!(app.get_by_test_id("first").frame().width(), 300.0);
    assert!(app.get_by_test_id("split").native_state().props.contains(&Prop::Number(300.0)));

    // The app's own moves aren't reported.
    position.set(250.0);
    app.settle().await;
    assert_eq!(app.get_by_test_id("first").frame().width(), 250.0);
    assert_eq!(*seen.borrow(), [300.0]);
}

/// A pane's least size in its style is as far as the divider goes, from
/// either side.
#[mitsuami_test::test]
async fn the_divider_stops_at_the_panes_least_sizes(app: TestApp) {
    app.mount(|| {
        Split::new()
            .position(200.0)
            .test_id("split")
            .children((pane("first").min_width(150), pane("second").min_width(100)))
    });
    app.resize(800.0, 500.0).await;
    let split = app.get_by_test_id("split");
    assert!(split.native_state().props.contains(&Prop::PaneMinimums([150.0, 100.0])));

    split.set_number(20.0).await;
    assert_eq!(app.get_by_test_id("first").frame().width(), 150.0);
    split.set_number(5000.0).await;
    let length = split.frame().width();
    assert_eq!(app.get_by_test_id("second").frame().width(), 100.0);
    assert_eq!(app.get_by_test_id("first").frame().width(), length - divider(&app) - 100.0);
}

/// Without a position, the first pane is as big as its content.
#[mitsuami_test::test]
async fn without_a_position_the_first_pane_fits_its_content(app: TestApp) {
    app.mount(|| {
        Split::new()
            .children((Column::new().child(Text::new("Branches").test_id("label")).test_id("first"), pane("second")))
    });
    app.expect(by_text("second")).to_be_visible().await;
    assert_eq!(app.get_by_test_id("first").frame().width(), app.get_by_test_id("label").frame().width());
}

/// The divider is drawn where the panes are after the app moves them, not
/// where it was: AppKit works out where its divider goes in a layout pass,
/// which panes moved from outside didn't ask for (the split example's
/// Reset, after the window was resized).
#[mitsuami_test::test]
async fn the_divider_is_drawn_where_the_app_moves_it(app: TestApp) {
    let position = signal(200.0);
    app.mount(move || {
        Split::new().position(position).grow(1.0).test_id("split").children((pane("first"), pane("second")))
    });
    app.resize(600.0, 300.0).await;
    // Shown as resized, as the screen would (a split view's first capture
    // works out its divider; the next draws it).
    for _ in 0..2 {
        let _ = app.ui().capture(app.window()).await;
    }
    position.set(320.0);
    app.settle().await;

    let image = match app.ui().capture(app.window()).await {
        Err(CaptureError::Unsupported) => return assert!(app.is_headless(), "only headless may lack capture"),
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(image) => image,
    };
    // A pixel down the middle of the split, at a point along it.
    let split = app.get_by_test_id("split").frame();
    let at = |x: f32| {
        let scale = image.scale_factor;
        let (x, y) = ((x * scale) as usize, ((split.y() + split.height() / 2.0) * scale) as usize);
        let i = (y * image.width as usize + x) * 4;
        image.rgba[i..i + 3].to_vec()
    };
    let background = at(split.x() + 100.0);
    assert_ne!(at(split.x() + 320.0 + divider(&app) / 2.0), background, "no divider where the panes now meet");
    assert_eq!(at(split.x() + 200.0 + divider(&app) / 2.0), background, "a divider left where they met");
}

mitsuami_test::main!();
