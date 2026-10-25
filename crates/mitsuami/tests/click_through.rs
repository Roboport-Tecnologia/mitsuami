//! `click_through`: a container the pointer passes through, with
//! everything in it, to what's under it, as a hint laid over a text area
//! lets clicks reach the text. Each platform hit-tests its own way (AppKit's
//! `hitTest:`, GTK's `can-target`, XAML's `IsHitTestVisible`); the tests ask
//! the backend what a click reaches.

use mitsuami::core::{ActionError, Point, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A text area with a hint laid over its first line, in the placeholder
/// colour, as mitsuami-extras' text area lays one where the platform has
/// none.
fn hinted(through: bool) -> impl View {
    Column::new().padding(16).child(
        Column::new().width(240).height(120).test_id("box").children((
            TextArea::new().a11y_label("Message").grow(1.0).test_id("area"),
            Column::new()
                .absolute()
                .top(4)
                .start(4)
                .click_through(through)
                .test_id("hint")
                .child(Text::new("Write a message").color(Color::Placeholder).test_id("hint-text")),
        )),
    )
}

/// Where a click lands on the hint's text.
fn on_hint(app: &TestApp) -> Point {
    let text = app.get_by_test_id("hint-text").frame();
    Point::new(text.x() + text.width() / 2.0, text.y() + text.height() / 2.0)
}

/// Some backends can't tell yet: they only type-check this.
fn supported(app: &TestApp, hit: &Result<Option<mitsuami::core::NodeId>, ActionError>) -> bool {
    if matches!(hit, Err(ActionError::Unsupported)) {
        assert!(!app.is_headless(), "headless hit-tests");
        return false;
    }
    true
}

#[mitsuami_test::test]
async fn a_click_on_the_hint_reaches_the_text_area(app: TestApp) {
    app.mount(|| hinted(true));
    app.expect(by_label("Message")).to_be_visible().await;
    assert!(app.get_by_test_id("hint").native_state().props.contains(&Prop::ClickThrough(true)));

    let hit = app.hit(on_hint(&app));
    if !supported(&app, &hit) {
        return;
    }
    assert_eq!(hit.unwrap(), Some(app.get_by_test_id("area").id()));
}

/// Without it, the hint takes the click, as anything laid over a control
/// does.
#[mitsuami_test::test]
async fn without_it_the_hint_takes_the_click(app: TestApp) {
    app.mount(|| hinted(false));
    app.expect(by_label("Message")).to_be_visible().await;

    let hit = app.hit(on_hint(&app));
    if !supported(&app, &hit) {
        return;
    }
    let hit = hit.unwrap();
    let hint = [app.get_by_test_id("hint").id(), app.get_by_test_id("hint-text").id()];
    assert!(hit.is_some_and(|h| hint.contains(&h)), "{hit:?}");
}

mitsuami_test::main!();
