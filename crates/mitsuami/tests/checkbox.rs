//! `Checkbox`: checked or not, or mixed (some of what it stands for is
//! checked). A click leaves the mixed state, landing where the platform
//! lands, and the app works out `mixed` again from there. A `Tweak` reaches
//! the native checkbox.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn props(app: &TestApp, name: &str) -> Vec<Prop> {
    app.get_by_role(Role::Checkbox, name).native_state().props
}

#[mitsuami_test::test]
async fn the_mixed_state_shows_over_checked(app: TestApp) {
    let checked = signal(false);
    let mixed = signal(true);
    app.mount(move || Checkbox::new("All").checked(checked).mixed(mixed));
    let all = app.get_by_role(Role::Checkbox, "All");
    assert!(props(&app, "All").contains(&Prop::Mixed(true)));
    assert!(all.node().mixed);

    // Checked changes underneath; the box stays mixed.
    checked.set(true);
    app.settle().await;
    assert!(props(&app, "All").contains(&Prop::Mixed(true)));

    // Out of the mixed state, it shows what checked says.
    mixed.set(false);
    app.settle().await;
    assert!(props(&app, "All").contains(&Prop::Mixed(false)));
    assert!(props(&app, "All").contains(&Prop::Checked(true)));
    assert!(!all.node().mixed);
    app.expect(by_role(Role::Checkbox, "All")).to_be_checked().await;
}

#[mitsuami_test::test]
async fn setting_it_mixed_reports_nothing(app: TestApp) {
    let mixed = signal(false);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let log = changes.clone();
    app.mount(move || Checkbox::new("All").mixed(mixed).on_change(move |c| log.borrow_mut().push(c)));

    mixed.set(true);
    app.settle().await;
    mixed.set(false);
    app.settle().await;
    assert!(changes.borrow().is_empty(), "{:?}", changes.borrow());
}

/// Where a click from the mixed state lands is the platform's call; the
/// box reports it and shows it, and later clicks just toggle.
#[mitsuami_test::test]
async fn a_click_leaves_the_mixed_state(app: TestApp) {
    let checked = signal(false);
    app.mount(move || Checkbox::new("All").bind(checked).mixed(true));
    let all = app.get_by_role(Role::Checkbox, "All");

    all.click().await;
    let landed = checked.get_untracked();
    assert!(props(&app, "All").contains(&Prop::Mixed(false)));
    assert!(props(&app, "All").contains(&Prop::Checked(landed)));
    assert!(!all.node().mixed);

    all.click().await;
    all.click().await;
    assert_eq!(checked.get_untracked(), landed);
    assert!(props(&app, "All").contains(&Prop::Mixed(false)));
}

/// The case the mixed state is for: a box that checks all the others.
#[mitsuami_test::test]
async fn a_select_all_box_follows_the_boxes_it_stands_for(app: TestApp) {
    let fruit = [signal(true), signal(false), signal(false)];
    app.mount(move || {
        let all_checked = move || fruit.iter().all(|f| f.get());
        let some_checked = move || fruit.iter().any(|f| f.get()) && !all_checked();
        Column::new().children((
            Checkbox::new("All fruit")
                .checked(all_checked)
                .mixed(some_checked)
                .on_change(move |checked| fruit.iter().for_each(|f| f.set(checked))),
            Checkbox::new("Apples").bind(fruit[0]),
            Checkbox::new("Pears").bind(fruit[1]),
            Checkbox::new("Plums").bind(fruit[2]),
        ))
    });
    let all = app.get_by_role(Role::Checkbox, "All fruit");
    assert!(all.node().mixed);

    // However the platform lands, every box follows the "All" box.
    all.click().await;
    let landed = fruit[0].get_untracked();
    assert!(fruit.iter().all(|f| f.get_untracked() == landed));
    assert!(!all.node().mixed);
    assert_eq!(all.node().checked, Some(landed));

    // Unchecking one makes it mixed again, from either side.
    if !landed {
        app.get_by_role(Role::Checkbox, "Pears").click().await;
    } else {
        app.get_by_role(Role::Checkbox, "Plums").click().await;
    }
    assert!(all.node().mixed);
    assert!(props(&app, "All fruit").contains(&Prop::Mixed(true)));
}

/// Logs whether the native checkbox is enabled, each time the tweak runs.
fn log_enabled(log: Rc<RefCell<Vec<bool>>>) -> Tweak<Checkbox> {
    platform! {
        macos => mitsuami::appkit::tweak(move |b: &mitsuami::appkit::objc2_app_kit::NSButton| {
            log.borrow_mut().push(b.isEnabled())
        }),
        gtk => mitsuami::gtk::tweak(move |b: &mitsuami::gtk::gtk::CheckButton| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(b.is_sensitive())
        }),
        kde => mitsuami::kirigami::tweak(move |b: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(b.bool("enabled"))
        }),
        win32 => mitsuami::win32::tweak(move |b: &mitsuami::win32::CheckBox| {
            use mitsuami::win32::Control;
            log.borrow_mut().push(b.is_enabled())
        }),
        windows => mitsuami::winui::tweak(move |b: &mitsuami::winui::bindings::CheckBox| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(b.cast::<mitsuami::winui::bindings::IControl>()?.IsEnabled()?);
            Ok(())
        }),
    }
}

#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_checkbox_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let enabled = signal(false);
    let tweak = log_enabled(log.clone());
    app.mount(move || Checkbox::new("All").native(tweak).enabled(enabled));

    assert!(props(&app, "All").iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&false));
    enabled.set(true);
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&true));
}

mitsuami_test::main!();
