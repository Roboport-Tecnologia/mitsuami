//! `Slider`: a number in a range, as the platform's slider shows it. It
//! reports the user's moves, follows reactive values and ranges, and reads
//! as a slider named by its label whose value is its number. How far a step
//! moves it, and whether it snaps, is the platform's: tests only check that
//! it moves the right way.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

fn number(app: &TestApp, query: Query) -> f64 {
    app.get(query)
        .native_state()
        .props
        .iter()
        .find_map(|p| match p {
            Prop::Number(n) => Some(*n),
            _ => None,
        })
        .expect("a slider has a number")
}

#[mitsuami_test::test]
async fn shows_its_range_and_value(app: TestApp) {
    app.mount(|| Slider::new("Volume").range(0.0, 10.0).value(3.0));
    let volume = by_role(Role::Slider, "Volume");

    assert_eq!(app.get(volume.clone()).native_state().kind, WidgetKind::Slider);
    assert!(has(&app, volume.clone(), Prop::Range { min: 0.0, max: 10.0 }));
    assert!(has(&app, volume.clone(), Prop::Number(3.0)));
    app.expect(volume).to_have_value("3").await;
}

#[mitsuami_test::test]
async fn reports_where_the_user_moves_it(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        Slider::new("Volume").range(0.0, 100.0).step(10.0).value(50.0).on_change(move |v| log.borrow_mut().push(v))
    });
    let volume = app.get_by_role(Role::Slider, "Volume");

    volume.set_number(80.0).await;
    assert_eq!(*seen.borrow(), [80.0]);
    assert!(has(&app, by_role(Role::Slider, "Volume"), Prop::Number(80.0)));

    volume.increment().await;
    let up = number(&app, by_role(Role::Slider, "Volume"));
    assert!(up > 80.0, "incrementing moved it to {up}");
    volume.decrement().await;
    volume.decrement().await;
    let down = number(&app, by_role(Role::Slider, "Volume"));
    assert!(down < 80.0, "decrementing moved it to {down}");
    assert_eq!(seen.borrow().last(), Some(&down));
}

#[mitsuami_test::test]
async fn binds_both_ways(app: TestApp) {
    let volume = signal(20.0);
    app.mount(move || {
        Column::new()
            .children((Slider::new("Volume").bind(volume), Text::new(move || format!("Volume: {}", volume.get()))))
    });

    app.get_by_role(Role::Slider, "Volume").set_number(40.0).await;
    assert_eq!(volume.get_untracked(), 40.0);
    app.expect(by_text("Volume: 40")).to_exist().await;

    volume.set(60.0);
    app.expect(by_role(Role::Slider, "Volume")).to_have_value("60").await;
    assert!(has(&app, by_role(Role::Slider, "Volume"), Prop::Number(60.0)));
}

#[mitsuami_test::test]
async fn keeps_its_value_in_its_range(app: TestApp) {
    let range = signal((0.0, 100.0));
    app.mount(move || {
        Column::new()
            .children((Slider::new("Past the end").value(150.0), Slider::new("Level").range_with(range).value(80.0)))
    });
    assert!(has(&app, by_role(Role::Slider, "Past the end"), Prop::Number(100.0)));

    // The value follows a new range, whichever the platform applies first.
    range.set((0.0, 50.0));
    app.expect(by_role(Role::Slider, "Level")).to_have_value("50").await;
    assert!(has(&app, by_role(Role::Slider, "Level"), Prop::Range { min: 0.0, max: 50.0 }));
    assert!(has(&app, by_role(Role::Slider, "Level"), Prop::Number(50.0)));

    range.set((0.0, 1000.0));
    app.expect(by_role(Role::Slider, "Level")).to_have_value("80").await;
    assert!(has(&app, by_role(Role::Slider, "Level"), Prop::Number(80.0)));
}

#[mitsuami_test::test]
async fn refuses_moves_while_disabled(app: TestApp) {
    app.mount(|| Slider::new("Off").value(10.0).enabled(false));
    let id = app.get_by_role(Role::Slider, "Off").id();

    assert_eq!(app.ui().perform(id, &A11yAction::Increment), Err(ActionError::Disabled));
    assert_eq!(app.ui().perform(id, &A11yAction::SetValue("20".into())), Err(ActionError::Disabled));
    app.settle().await;
    assert!(has(&app, by_role(Role::Slider, "Off"), Prop::Enabled(false)));
    assert!(has(&app, by_role(Role::Slider, "Off"), Prop::Number(10.0)));
}

#[mitsuami_test::test]
async fn has_a_size_and_takes_focus(app: TestApp) {
    app.mount(|| Column::new().children((TextInput::new().a11y_label("Name"), Slider::new("Volume"))));

    assert!(!app.get_by_role(Role::Slider, "Volume").frame().size.is_empty());
    app.get_by_role(Role::Slider, "Volume").focus().await;
    app.expect(by_role(Role::Slider, "Volume")).to_be_focused().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let volume = signal(30.0);
    app.mount(move || view! { <Slider a11y_label="Volume" bind=volume/> });

    app.expect(by_role(Role::Slider, "Volume")).to_have_value("30").await;
    app.get_by_role(Role::Slider, "Volume").set_number(70.0).await;
    assert_eq!(volume.get_untracked(), 70.0);
}

/// Vertical sliders are as tall as the layout makes them and as wide as the
/// platform draws them; moving one up the range still raises it.
#[mitsuami_test::test]
async fn runs_vertically(app: TestApp) {
    let volume = signal(50.0);
    let orientation = signal(Orientation::Vertical);
    app.mount(move || Row::new().height(160).child(Slider::new("Volume").orientation(orientation).bind(volume)));
    let slider = by_role(Role::Slider, "Volume");

    assert!(has(&app, slider.clone(), Prop::Orientation(Orientation::Vertical)));
    let frame = app.get(slider.clone()).frame();
    assert!(frame.height() > frame.width(), "{frame} isn't upright");

    app.get(slider.clone()).increment().await;
    assert!(volume.get_untracked() > 50.0);

    orientation.set(Orientation::Horizontal);
    app.settle().await;
    assert!(has(&app, slider, Prop::Orientation(Orientation::Horizontal)));
}

/// Logs the native slider's value, each time the tweak runs.
fn log_value(log: Rc<RefCell<Vec<f64>>>) -> Tweak<Slider> {
    platform! {
        macos => mitsuami::appkit::tweak(move |s: &mitsuami::appkit::objc2_app_kit::NSSlider| {
            log.borrow_mut().push(s.doubleValue())
        }),
        gtk => mitsuami::gtk::tweak(move |s: &mitsuami::gtk::gtk::Scale| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(s.value())
        }),
        kde => mitsuami::kirigami::tweak(move |s: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(s.real("value"))),
        windows => mitsuami::winui::tweak(move |s: &mitsuami::winui::bindings::Slider| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(s.cast::<mitsuami::winui::bindings::IRangeBase>()?.Value()?);
            Ok(())
        }),
    }
}

/// Given before the value, the tweak still runs after it, and again when
/// it changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_slider_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let volume = signal(30.0);
    let tweak = log_value(log.clone());
    app.mount(move || Slider::new("Volume").native(tweak).value(volume));

    let props = app.get_by_role(Role::Slider, "Volume").native_state().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&30.0));
    volume.set(70.0);
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&70.0));
}

mitsuami_test::main!();
