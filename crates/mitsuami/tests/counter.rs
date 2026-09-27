//! The canonical counter: reactive text, events, conditional rendering, and
//! the exact commands a click produces.

use mitsuami::core::{Command, Point, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn counter(initial: i32) -> impl View {
    let count = signal(initial);
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Text::new(move || format!("Count: {}", count.get())),
        Row::new().gap(8).children((
            Button::new("Increment").role(ButtonRole::Default).on_click(move || count.update(|c| *c += 1)),
            Button::new("Reset").on_click(move || count.set(0)),
        )),
        Show::new(move || count.get() >= 3, || Text::new("That's a lot of clicks")),
    ))
}

#[mitsuami_test::test]
async fn clicking_increments_the_count(app: TestApp) {
    app.mount(|| counter(0));

    app.get_by_role(Role::Button, "Increment").click().await;
    app.get_by_role(Role::Button, "Increment").click().await;

    app.expect(by_text("Count: 2")).to_be_visible().await;
}

#[mitsuami_test::test]
async fn conditional_text_appears_and_goes_away(app: TestApp) {
    app.mount(|| counter(2));
    app.expect(by_text("That's a lot of clicks")).not_to_exist().await;

    app.get_by_role(Role::Button, "Increment").click().await;
    app.expect(by_text("That's a lot of clicks")).to_be_visible().await;

    app.get_by_role(Role::Button, "Reset").click().await;
    app.expect(by_text("That's a lot of clicks")).not_to_exist().await;
    app.expect(by_text("Count: 0")).to_exist().await;
}

#[mitsuami_test::test]
async fn layout_uses_padding_gap_and_intrinsic_sizes(app: TestApp) {
    app.mount(|| counter(0));
    let text = app.get_by_text("Count: 0").frame();
    let increment = app.get_by_role(Role::Button, "Increment").frame();
    let reset = app.get_by_role(Role::Button, "Reset").frame();

    // Padding, gaps and alignment hold whatever the platform's text metrics.
    assert_eq!(text.origin, Point::new(16.0, 16.0));
    assert!(!text.size.is_empty() && !increment.size.is_empty());
    assert_eq!(increment.x(), 16.0);
    assert_eq!(increment.y(), text.max_y() + 8.0);
    assert_eq!(reset.x(), increment.max_x() + 8.0);
    assert_eq!(reset.y(), increment.y());
}

#[mitsuami_test::test(headless)]
async fn layout_matches_headless_metrics_exactly(app: TestApp) {
    app.mount(|| counter(0));

    // Headless metrics: 16px body text, 8px per character, 20px lines;
    // buttons add 12px padding per side and are 28px tall.
    app.expect(by_text("Count: 0")).to_have_frame(Rect::new(16.0, 16.0, 64.0, 20.0)).await;
    app.expect(by_role(Role::Button, "Increment")).to_have_frame(Rect::new(16.0, 44.0, 96.0, 28.0)).await;
    app.expect(by_role(Role::Button, "Reset")).to_have_frame(Rect::new(120.0, 44.0, 64.0, 28.0)).await;
}

#[mitsuami_test::test]
async fn a_click_sends_only_the_prop_that_changed(app: TestApp) {
    app.mount(|| counter(0));
    let text = app.get_by_text("Count: 0").id();
    app.take_command_log();

    app.get_by_role(Role::Button, "Increment").click().await;

    // Only the text changes (frames may follow if the platform's digits
    // differ in width); nothing is created, destroyed or re-sent.
    let log = app.take_command_log();
    let changes: Vec<&Command> = log.iter().filter(|c| !matches!(c, Command::SetFrame { .. })).collect();
    assert_eq!(changes, vec![&Command::SetProp { id: text, prop: Prop::Text("Count: 1".into()) }]);
}

#[mitsuami_test::test]
async fn keyboard_activation_works_like_a_click(app: TestApp) {
    app.mount(|| counter(0));
    app.get_by_role(Role::Button, "Increment").press(Key::Enter).await;
    app.get_by_role(Role::Button, "Increment").press(Key::Char(' ')).await;
    app.expect(by_text("Count: 2")).to_exist().await;
}

#[mitsuami_test::test]
async fn initial_render_snapshots(app: TestApp) {
    app.mount(|| counter(5));
    app.assert_tree_snapshot("initial");
    app.assert_a11y_snapshot("initial");
    app.assert_wireframe_snapshot("initial");
    app.assert_visual_snapshot("initial").await;
}

mitsuami_test::main!();
