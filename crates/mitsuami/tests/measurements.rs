//! Measurements: the window's and a node's laid-out size as signals, for
//! views that adapt to the room they have.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn boxed(id: &str, width: impl IntoValue<Length>, height: impl IntoValue<Length>) -> Container {
    Container::new().size(width, height).test_id(id)
}

async fn size_of(app: &TestApp, id: &str) -> Size {
    app.settle().await;
    app.get_by_test_id(id).frame().size
}

#[mitsuami_test::test]
async fn the_viewport_follows_the_window(app: TestApp) {
    let seen = signal(Size::ZERO);
    app.mount(move || {
        let viewport = use_viewport();
        effect(move || seen.set(viewport.get()));
        boxed("box", 10, 10)
    });
    assert_eq!(seen.get_untracked(), Size::new(800.0, 600.0));

    app.resize(400.0, 300.0).await;

    assert_eq!(seen.get_untracked(), Size::new(400.0, 300.0));
}

/// A breakpoint: the view is laid out again with the other branch in the
/// same turn, so the window never shows the one that didn't fit.
#[mitsuami_test::test]
async fn views_switch_at_a_breakpoint_before_they_show(app: TestApp) {
    app.mount(|| {
        let viewport = use_viewport();
        Show::new(move || viewport.get().width >= 600.0, || boxed("wide", 500, 10))
            .fallback(|| boxed("narrow", 200, 10))
    });
    assert_eq!(size_of(&app, "wide").await, Size::new(500.0, 10.0));

    app.resize(400.0, 300.0).await;

    app.expect(by_test_id("wide")).not_to_exist().await;
    assert_eq!(size_of(&app, "narrow").await, Size::new(200.0, 10.0));
}

#[mitsuami_test::test]
async fn a_node_reports_its_laid_out_size(app: TestApp) {
    let seen = signal(Size::ZERO);
    app.mount(move || {
        let sidebar = node_ref();
        let size = use_size(sidebar);
        effect(move || seen.set(size.get()));
        Row::new()
            .align(Align::Start)
            .children((boxed("fixed", 200, 20), boxed("fill", Length::Auto, 30).grow(1.0).node_ref(sidebar)))
    });
    assert_eq!(seen.get_untracked(), size_of(&app, "fill").await);
    assert_eq!(seen.get_untracked(), Size::new(600.0, 30.0));

    app.resize(500.0, 300.0).await;

    assert_eq!(seen.get_untracked(), Size::new(300.0, 30.0));
}

/// A container query: the children lay out by the room their container has,
/// not the window's.
#[mitsuami_test::test]
async fn children_adapt_to_their_container(app: TestApp) {
    app.mount(|| {
        let panel = node_ref();
        let size = use_size(panel);
        Row::new().children((
            boxed("side", 500, 10).shrink(0.0),
            // No wider than the room left, whatever its content.
            Container::new().grow(1.0).min_width(0).node_ref(panel).child(
                Show::new(move || size.get().width >= 250.0, || boxed("two-columns", 250, 10))
                    .fallback(|| boxed("one-column", 100, 10)),
            ),
        ))
    });
    // 800 − 500 = 300 wide.
    app.expect(by_test_id("two-columns")).to_exist().await;

    app.resize(700.0, 300.0).await;

    app.expect(by_test_id("two-columns")).not_to_exist().await;
    app.expect(by_test_id("one-column")).to_exist().await;
}

#[mitsuami_test::test]
async fn a_ref_follows_the_node_built_for_it(app: TestApp) {
    let large = signal(true);
    let shown = signal(true);
    let seen = signal(Size::ZERO);
    app.mount(move || {
        let node = node_ref();
        let size = use_size(node);
        effect(move || seen.set(size.get()));
        Show::new(shown, move || {
            Show::new(large, move || boxed("large", 100, 50).node_ref(node))
                .fallback(move || boxed("small", 20, 10).node_ref(node))
        })
    });
    assert_eq!(seen.get_untracked(), Size::new(100.0, 50.0));

    large.set(false);
    app.settle().await;
    assert_eq!(seen.get_untracked(), Size::new(20.0, 10.0));

    // Nothing built: no size.
    shown.set(false);
    app.settle().await;
    assert_eq!(seen.get_untracked(), Size::ZERO);
}

/// A view whose size flips with its own size never settles; the run loop
/// still goes on.
#[mitsuami_test::test]
async fn a_view_that_never_settles_doesnt_hold_the_run_loop(app: TestApp) {
    app.mount(|| {
        let node = node_ref();
        let size = use_size(node);
        Column::new()
            .align(Align::Start)
            .child(boxed("flips", move || if size.get().width < 100.0 { 200.px() } else { 50.px() }, 10).node_ref(node))
    });
    app.settle().await;
    app.resize(400.0, 300.0).await;
    app.expect(by_test_id("flips")).to_exist().await;
}

mitsuami_test::main!();
