//! `Switch`: on or off, reported when the user flips it and not when the
//! app does. It has no semantic options past that: a `Tweak` reaches the
//! native switch for what one platform has.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn props(app: &TestApp) -> Vec<Prop> {
    app.get_by_role(Role::Switch, "Wi-Fi").native_state().props
}

#[mitsuami_test::test]
async fn flipping_it_reports_and_setting_it_doesnt(app: TestApp) {
    let on = signal(false);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let log = changes.clone();
    app.mount(move || Switch::new("Wi-Fi").checked(on).on_change(move |v| log.borrow_mut().push(v)));

    on.set(true);
    app.settle().await;
    assert!(props(&app).contains(&Prop::Checked(true)));
    assert!(changes.borrow().is_empty(), "{:?}", changes.borrow());

    app.get_by_role(Role::Switch, "Wi-Fi").click().await;
    assert_eq!(*changes.borrow(), [false]);
    app.expect(by_role(Role::Switch, "Wi-Fi")).not_to_be_checked().await;
}

/// Logs whether the native switch is on, each time the tweak runs.
fn log_on(log: Rc<RefCell<Vec<bool>>>) -> Tweak<Switch> {
    platform! {
        macos => mitsuami::appkit::tweak(move |s: &mitsuami::appkit::objc2_app_kit::NSSwitch| {
            log.borrow_mut().push(s.state() == mitsuami::appkit::objc2_app_kit::NSControlStateValueOn)
        }),
        gtk => mitsuami::gtk::tweak(move |s: &mitsuami::gtk::gtk::Switch| log.borrow_mut().push(s.is_active())),
        kde => mitsuami::kirigami::tweak(move |s: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(s.bool("checked"))
        }),
        windows => mitsuami::winui::tweak(move |s: &mitsuami::winui::bindings::ToggleSwitch| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(s.cast::<mitsuami::winui::bindings::IToggleSwitch>()?.IsOn()?);
            Ok(())
        }),
    }
}

/// Given before `checked`, the tweak still runs after it, and again when
/// it changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_switch_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let on = signal(true);
    let tweak = log_on(log.clone());
    app.mount(move || Switch::new("Wi-Fi").native(tweak).checked(on));

    assert!(props(&app).iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&true));
    on.set(false);
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&false));
}

mitsuami_test::main!();
