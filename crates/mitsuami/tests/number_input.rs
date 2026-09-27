//! `NumberInput`: a whole number in a range, as the platform's spin box
//! shows it. It reports what the user enters, follows reactive values and
//! ranges, and reads as a spin button named by its label whose value is its
//! number. Its buttons add or take away its step on every platform; what
//! happens past an end is the platform's: AppKit's stepper wraps round, the
//! others stop.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

fn memory() -> Query {
    by_role(Role::SpinButton, "Memory")
}

#[mitsuami_test::test]
async fn shows_its_range_and_value(app: TestApp) {
    app.mount(|| NumberInput::new("Memory").range(16, 512).value(64));

    assert_eq!(app.get(memory()).native_state().kind, WidgetKind::NumberInput);
    assert!(has(&app, memory(), Prop::Range { min: 16.0, max: 512.0 }));
    assert!(has(&app, memory(), Prop::Number(64.0)));
    app.expect(memory()).to_have_value("64").await;
}

#[mitsuami_test::test]
async fn reports_what_the_user_enters(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        NumberInput::new("Memory").range(16, 512).step(16).value(64).on_change(move |v| log.borrow_mut().push(v))
    });

    app.get(memory()).set_number(100.0).await;
    assert_eq!(*seen.borrow(), [100]);
    assert!(has(&app, memory(), Prop::Number(100.0)));
}

/// Every platform's buttons add exactly the step.
#[mitsuami_test::test]
async fn steps_by_its_step(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        NumberInput::new("Memory").range(16, 512).step(16).value(64).on_change(move |v| log.borrow_mut().push(v))
    });

    app.get(memory()).increment().await;
    app.get(memory()).increment().await;
    app.get(memory()).decrement().await;
    assert_eq!(*seen.borrow(), [80, 96, 80]);
    app.expect(memory()).to_have_value("80").await;
    assert!(has(&app, memory(), Prop::Number(80.0)));
}

/// Without a step, the platform's, which is 1 everywhere.
#[mitsuami_test::test]
async fn steps_by_one_by_default(app: TestApp) {
    app.mount(|| NumberInput::new("Memory").value(5));

    app.get(memory()).increment().await;
    app.expect(memory()).to_have_value("6").await;
}

/// GTK's, Qt's and WinUI's spin boxes stop at their ends; AppKit's stepper
/// wraps round to the other end, as `NSStepper.valueWraps` does unless an
/// app turns it off.
#[mitsuami_test::test]
async fn stops_or_wraps_at_its_ends_as_the_platform_does(app: TestApp) {
    app.mount(|| NumberInput::new("Memory").range(1, 10).value(10));

    app.get(memory()).increment().await;
    let wrapped = app.backend_name() == "appkit";
    app.expect(memory()).to_have_value(if wrapped { "1" } else { "10" }).await;

    app.get(memory()).set_number(1.0).await;
    app.get(memory()).decrement().await;
    app.expect(memory()).to_have_value(if wrapped { "10" } else { "1" }).await;
}

#[mitsuami_test::test]
async fn keeps_to_whole_numbers(app: TestApp) {
    let memory_mb = signal(0);
    app.mount(move || NumberInput::new("Memory").bind(memory_mb));

    app.get(memory()).set_number(12.4).await;
    assert_eq!(memory_mb.get_untracked(), 12);
    assert!(has(&app, memory(), Prop::Number(12.0)));
}

#[mitsuami_test::test]
async fn binds_both_ways(app: TestApp) {
    let memory_mb = signal(64);
    app.mount(move || {
        Column::new().children((
            NumberInput::new("Memory").range(16, 512).bind(memory_mb),
            Text::new(move || format!("Memory: {} MB", memory_mb.get())),
        ))
    });

    app.get(memory()).set_number(128.0).await;
    assert_eq!(memory_mb.get_untracked(), 128);
    app.expect(by_text("Memory: 128 MB")).to_exist().await;

    memory_mb.set(256);
    app.expect(memory()).to_have_value("256").await;
    assert!(has(&app, memory(), Prop::Number(256.0)));
}

/// Values the app sets aren't reported back as the user's.
#[mitsuami_test::test]
async fn does_not_report_the_apps_own_changes(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    let value = signal(64);
    let range = signal((16, 512));
    app.mount(move || {
        NumberInput::new("Memory").range_with(range).value(value).on_change(move |v| log.borrow_mut().push(v))
    });

    value.set(100);
    app.settle().await;
    range.set((16, 32));
    app.settle().await;
    assert!(has(&app, memory(), Prop::Number(32.0)));
    assert_eq!(*seen.borrow(), [] as [i32; 0]);
}

#[mitsuami_test::test]
async fn keeps_its_value_in_its_range(app: TestApp) {
    let range = signal((0, 100));
    app.mount(move || {
        Column::new().children((
            NumberInput::new("Past the end").value(150),
            NumberInput::new("Memory").range_with(range).value(80),
        ))
    });
    assert!(has(&app, by_role(Role::SpinButton, "Past the end"), Prop::Number(100.0)));

    // The value follows a new range, whichever the platform applies first.
    range.set((0, 50));
    app.expect(memory()).to_have_value("50").await;
    assert!(has(&app, memory(), Prop::Range { min: 0.0, max: 50.0 }));
    assert!(has(&app, memory(), Prop::Number(50.0)));

    range.set((0, 1000));
    app.expect(memory()).to_have_value("80").await;
    assert!(has(&app, memory(), Prop::Number(80.0)));

    // Past the end, a typed or spoken number stops at it.
    app.get(memory()).set_number(5000.0).await;
    app.expect(memory()).to_have_value("1000").await;
}

#[mitsuami_test::test]
async fn refuses_changes_while_disabled(app: TestApp) {
    app.mount(|| NumberInput::new("Memory").value(10).enabled(false));
    let id = app.get(memory()).id();

    assert_eq!(app.ui().perform(id, &A11yAction::Increment), Err(ActionError::Disabled));
    assert_eq!(app.ui().perform(id, &A11yAction::SetValue("20".into())), Err(ActionError::Disabled));
    app.settle().await;
    assert!(has(&app, memory(), Prop::Enabled(false)));
    assert!(has(&app, memory(), Prop::Number(10.0)));
}

#[mitsuami_test::test]
async fn has_a_size_and_takes_focus(app: TestApp) {
    app.mount(|| Column::new().children((TextInput::new().a11y_label("Name"), NumberInput::new("Memory"))));

    let frame = app.get(memory()).frame();
    assert!(!frame.size.is_empty());
    assert!(frame.width() > frame.height(), "{frame} is taller than wide");
    app.get(memory()).focus().await;
    app.expect(memory()).to_be_focused().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let copies = signal(3);
    app.mount(move || view! { <NumberInput label="Copies" bind=copies/> });

    app.expect(by_role(Role::SpinButton, "Copies")).to_have_value("3").await;
    app.get(by_role(Role::SpinButton, "Copies")).set_number(7.0).await;
    assert_eq!(copies.get_untracked(), 7);
}

/// Logs the native spin box's value, each time the tweak runs.
fn log_value(log: Rc<RefCell<Vec<f64>>>) -> Tweak<NumberInput> {
    platform! {
        macos => mitsuami::appkit::tweak(move |n: &mitsuami::appkit::NumberField| {
            log.borrow_mut().push(n.stepper().doubleValue())
        }),
        gtk => mitsuami::gtk::tweak(move |s: &mitsuami::gtk::gtk::SpinButton| log.borrow_mut().push(s.value())),
        kde => mitsuami::kirigami::tweak(move |s: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(s.real("value"))),
        windows => mitsuami::winui::tweak(move |n: &mitsuami::winui::bindings::NumberBox| {
            log.borrow_mut().push(n.Value()?);
            Ok(())
        }),
    }
}

/// Given before the value, the tweak still runs after it, and again when
/// it changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_spin_box_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let value = signal(30);
    let tweak = log_value(log.clone());
    app.mount(move || NumberInput::new("Memory").native(tweak).value(value));

    let props = app.get(memory()).native_state().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&30.0));
    value.set(70);
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&70.0));
}

mitsuami_test::main!();
