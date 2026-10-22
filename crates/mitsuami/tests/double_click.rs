//! Clicks and double clicks: a container or group with `on_click` hears a
//! click on it, or on a label inside it, as a header opening its section;
//! with `on_double_click`, a
//! double click on it, or on a label inside it, as the platform counts
//! one (AppKit's click recognizer, GTK's click gesture, Qt Quick's tap
//! handler, XAML's `DoubleTapped`); a control inside keeps its own clicks.
//! Tests send it through what each backend's handling reports; a real
//! mouse is for trying by hand (`examples/hover.rs`, its rename row).

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn reports_a_double_click(app: TestApp) {
    let count = signal(0);
    app.mount(move || {
        Column::new().children((
            Row::new()
                .test_id("row")
                .on_double_click(move || count.update(|c| *c += 1))
                .children((Text::new("Windows 98"), Button::new("Delete"))),
            Text::new(move || format!("{} double clicks", count.get())),
        ))
    });
    assert!(app.get(by_test_id("row")).native_state().props.contains(&Prop::DoubleClick(true)));

    app.get(by_test_id("row")).double_click().await;
    app.get(by_test_id("row")).double_click().await;
    app.expect(by_text("2 double clicks")).to_exist().await;
}

/// It's the app's to ask for: a container that didn't has nothing to
/// report.
#[mitsuami_test::test]
async fn only_where_asked(app: TestApp) {
    app.mount(|| Column::new().child(Row::new().test_id("row").child(Text::new("Windows 98"))));

    let row = app.get(by_test_id("row"));
    assert!(!row.native_state().props.iter().any(|p| matches!(p, Prop::DoubleClick(_))));
    assert!(app.ui().synthesize(row.id(), &mitsuami::core::backend::SyntheticInput::DoubleClick).is_err());
}

/// A group reports it too.
#[mitsuami_test::test]
async fn a_group_reports_it(app: TestApp) {
    let count = signal(0);
    app.mount(move || {
        Column::new().children((
            Group::new()
                .title("Storage")
                .test_id("group")
                .on_double_click(move || count.update(|c| *c += 1))
                .child(Text::new("disk.qcow2")),
            Text::new(move || format!("{} double clicks", count.get())),
        ))
    });

    app.get(by_test_id("group")).double_click().await;
    app.expect(by_text("1 double clicks")).to_exist().await;
}

/// A header hears a click on itself or its label, as an expander's does,
/// and a button inside it keeps its own.
#[mitsuami_test::test]
async fn reports_a_click(app: TestApp) {
    let (headers, adds) = (signal(0), signal(0));
    app.mount(move || {
        Column::new().children((
            Row::new()
                .test_id("header")
                .on_click(move || headers.update(|c| *c += 1))
                .children((Text::new("LOCAL BRANCHES"), Button::new("Add").on_click(move || adds.update(|c| *c += 1)))),
            Text::new(move || format!("{} clicks, {} adds", headers.get(), adds.get())),
        ))
    });
    assert!(app.get(by_test_id("header")).native_state().props.contains(&Prop::Click(true)));

    app.get(by_test_id("header")).click().await;
    app.get(by_role(Role::Button, "Add")).click().await;
    app.expect(by_text("1 clicks, 1 adds")).to_exist().await;
}

/// Only where asked, as double clicks.
#[mitsuami_test::test]
async fn clicks_only_where_asked(app: TestApp) {
    app.mount(|| Column::new().child(Row::new().test_id("row").child(Text::new("main"))));

    let row = app.get(by_test_id("row"));
    assert!(!row.native_state().props.iter().any(|p| matches!(p, Prop::Click(_))));
    assert!(app.ui().synthesize(row.id(), &mitsuami::core::backend::SyntheticInput::SingleClick).is_err());
}

mitsuami_test::main!();
