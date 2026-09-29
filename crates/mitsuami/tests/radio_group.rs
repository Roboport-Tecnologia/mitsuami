//! `RadioGroup`: native radio buttons, one for each option, down a column.
//! It shows its options and the chosen one (or none), reports what the
//! user chooses, follows reactive options and choices, and reads as a
//! radio group named by its label, holding a radio button for each option.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

fn options(names: &[&str]) -> Prop {
    Prop::Options(names.iter().map(|n| n.to_string()).collect())
}

/// A group's radio buttons, as `option` lines, the checked one starred.
fn buttons(app: &TestApp, label: &str) -> Vec<String> {
    let tree = app.a11y_tree();
    let group = tree.walk().into_iter().find(|n| n.role == Role::RadioGroup && n.name.as_deref() == Some(label));
    group
        .expect("a radio group")
        .children
        .iter()
        .map(|n| {
            assert_eq!(n.role, Role::RadioButton);
            format!("{}{}", n.name.clone().unwrap_or_default(), if n.checked == Some(true) { " *" } else { "" })
        })
        .collect()
}

#[mitsuami_test::test]
async fn shows_its_options_and_the_chosen_one(app: TestApp) {
    app.mount(|| RadioGroup::new("Size").options(SIZES).selected(Some(1)));
    let size = by_role(Role::RadioGroup, "Size");

    assert_eq!(app.get(size.clone()).native_state().kind, WidgetKind::RadioGroup);
    assert!(has(&app, size.clone(), options(&SIZES)));
    assert!(has(&app, size.clone(), Prop::SelectedIndex(Some(1))));
    assert_eq!(buttons(&app, "Size"), ["Small", "Medium *", "Large"]);
    assert!(app.get_by_role(Role::RadioButton, "Medium").is_checked());
    assert!(!app.get_by_role(Role::RadioButton, "Small").is_checked());
}

/// Every platform's radio buttons can all be off, as HTML's start.
#[mitsuami_test::test]
async fn chooses_none_unless_told_otherwise(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            RadioGroup::new("Size").options(SIZES),
            RadioGroup::new("Past the end").options(["A", "B"]).selected(Some(7)),
            RadioGroup::new("Empty"),
        ))
    });

    assert!(has(&app, by_role(Role::RadioGroup, "Size"), Prop::SelectedIndex(None)));
    assert_eq!(buttons(&app, "Size"), SIZES);
    assert!(has(&app, by_role(Role::RadioGroup, "Past the end"), Prop::SelectedIndex(None)));
    assert_eq!(buttons(&app, "Past the end"), ["A", "B"]);
    assert!(has(&app, by_role(Role::RadioGroup, "Empty"), options(&[])));
    assert!(buttons(&app, "Empty").is_empty());
}

#[mitsuami_test::test]
async fn reports_the_option_the_user_chooses_and_shows_it(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || RadioGroup::new("Size").options(SIZES).on_change(move |i| log.borrow_mut().push(i)));

    app.get_by_role(Role::RadioButton, "Large").click().await;
    app.get_by_role(Role::RadioButton, "Small").click().await;
    // The chosen one again changes nothing, on every platform.
    app.get_by_role(Role::RadioButton, "Small").click().await;

    assert_eq!(*seen.borrow(), [2, 0]);
    assert!(has(&app, by_role(Role::RadioGroup, "Size"), Prop::SelectedIndex(Some(0))));
    assert_eq!(buttons(&app, "Size"), ["Small *", "Medium", "Large"]);
}

#[mitsuami_test::test]
async fn binds_both_ways(app: TestApp) {
    let chosen = signal(None);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        Column::new().children((
            RadioGroup::new("Size").options(SIZES).bind(chosen).on_change(move |i| log.borrow_mut().push(i)),
            Text::new(move || format!("Chosen: {:?}", chosen.get())),
        ))
    });

    app.get_by_role(Role::RadioGroup, "Size").select_option("Medium").await;
    assert_eq!(chosen.get_untracked(), Some(1));
    app.expect(by_text("Chosen: Some(1)")).to_exist().await;

    // Set by the app, it's shown, and not reported back.
    chosen.set(Some(2));
    app.settle().await;
    assert_eq!(buttons(&app, "Size"), ["Small", "Medium", "Large *"]);
    assert!(has(&app, by_role(Role::RadioGroup, "Size"), Prop::SelectedIndex(Some(2))));
    chosen.set(None);
    app.settle().await;
    assert_eq!(buttons(&app, "Size"), SIZES);
    assert!(has(&app, by_role(Role::RadioGroup, "Size"), Prop::SelectedIndex(None)));
    assert_eq!(*seen.borrow(), [1]);
}

#[mitsuami_test::test]
async fn follows_reactive_options(app: TestApp) {
    let names = signal(vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()]);
    app.mount(move || RadioGroup::new("Color").options(names).selected(Some(2)));
    let color = by_role(Role::RadioGroup, "Color");
    assert_eq!(buttons(&app, "Color"), ["Red", "Green", "Blue *"]);

    // The chosen index stays when the options change around it.
    names.set(vec!["Cyan".into(), "Magenta".into(), "Yellow".into(), "Black".into()]);
    app.settle().await;
    assert_eq!(buttons(&app, "Color"), ["Cyan", "Magenta", "Yellow *", "Black"]);
    assert!(has(&app, color.clone(), options(&["Cyan", "Magenta", "Yellow", "Black"])));
    assert!(has(&app, color.clone(), Prop::SelectedIndex(Some(2))));

    // Past the options, none is chosen.
    names.set(vec!["Black".into(), "White".into()]);
    app.settle().await;
    assert_eq!(buttons(&app, "Color"), ["Black", "White"]);
    assert!(has(&app, color.clone(), options(&["Black", "White"])));
    assert!(has(&app, color.clone(), Prop::SelectedIndex(None)));

    // With them back, it's chosen again, as the app still says.
    names.set(vec!["Cyan".into(), "Magenta".into(), "Yellow".into()]);
    app.settle().await;
    assert_eq!(buttons(&app, "Color"), ["Cyan", "Magenta", "Yellow *"]);
    assert!(has(&app, color, Prop::SelectedIndex(Some(2))));
}

#[mitsuami_test::test]
async fn keeps_options_with_the_same_text_apart(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        RadioGroup::new("Size")
            .options(["Small", "Large", "Small"])
            .selected(Some(2))
            .on_change(move |i| log.borrow_mut().push(i))
    });

    assert_eq!(buttons(&app, "Size"), ["Small", "Large", "Small *"]);
    app.get_by_role(Role::RadioGroup, "Size").select_option("Large").await;
    assert_eq!(*seen.borrow(), [1]);
    assert_eq!(buttons(&app, "Size"), ["Small", "Large *", "Small"]);
}

#[mitsuami_test::test]
async fn refuses_options_it_does_not_have_and_disabled_choices(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            RadioGroup::new("Size").options(SIZES),
            RadioGroup::new("Off").options(SIZES).selected(Some(0)).enabled(false),
        ))
    });
    let size = app.get_by_role(Role::RadioGroup, "Size").id();
    let off = app.get_by_role(Role::RadioGroup, "Off").id();

    assert_eq!(app.ui().perform(size, &A11yAction::SetValue("Huge".into())), Err(ActionError::Unsupported));
    assert_eq!(app.ui().perform(off, &A11yAction::SetValue("Large".into())), Err(ActionError::Disabled));
    app.settle().await;
    assert!(has(&app, by_role(Role::RadioGroup, "Off"), Prop::Enabled(false)));
    assert!(has(&app, by_role(Role::RadioGroup, "Off"), Prop::SelectedIndex(Some(0))));
    let tree = app.a11y_tree();
    let group = tree.walk().into_iter().find(|n| n.role == Role::RadioGroup && n.name.as_deref() == Some("Off"));
    assert!(group.unwrap().children.iter().all(|b| !b.enabled));
    assert!(has(&app, by_role(Role::RadioGroup, "Size"), Prop::SelectedIndex(None)));
}

/// A button per option, down a column: each option makes the group taller,
/// and the longest decides its width. How far apart the buttons are is
/// the platform's. The third option is an "I", narrower than "A" and "B" in
/// any font ("C" is wider than both in AppKit's).
#[mitsuami_test::test]
async fn is_sized_for_its_options(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            RadioGroup::new("Two").options(["A", "B"]),
            RadioGroup::new("Three").options(["A", "B", "I"]),
            RadioGroup::new("Long").options(["A considerably longer option", "B"]),
        ))
    });
    let two = app.get_by_role(Role::RadioGroup, "Two").frame();
    let three = app.get_by_role(Role::RadioGroup, "Three").frame();
    let long = app.get_by_role(Role::RadioGroup, "Long").frame();

    assert!(!two.size.is_empty());
    assert!(three.height() > two.height(), "{three} is not taller than {two}");
    assert_eq!(three.width(), two.width());
    assert!(long.width() > two.width(), "{long} is not wider than {two}");
    assert_eq!(long.height(), two.height());
}

// Whether Tab reaches it is the platform's call: macOS skips radio buttons
// unless Full Keyboard Access is on, as it skips buttons.
#[mitsuami_test::test]
async fn takes_focus(app: TestApp) {
    app.mount(|| {
        Column::new()
            .children((TextInput::new().a11y_label("Name"), RadioGroup::new("Size").options(SIZES).selected(Some(1))))
    });

    app.get_by_role(Role::RadioGroup, "Size").focus().await;
    app.expect(by_role(Role::RadioGroup, "Size")).to_be_focused().await;

    app.get_by_label("Name").focus().await;
    app.expect(by_role(Role::RadioGroup, "Size")).not_to_be_focused().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let chosen = signal(Some(1));
    app.mount(move || view! { <RadioGroup label="Size" options=SIZES bind=chosen/> });

    assert_eq!(buttons(&app, "Size"), ["Small", "Medium *", "Large"]);
    app.get_by_role(Role::RadioButton, "Large").click().await;
    assert_eq!(chosen.get_untracked(), Some(2));
}

/// Logs how many radio buttons the native group holds, each time the tweak
/// runs.
fn log_buttons(log: Rc<RefCell<Vec<usize>>>) -> Tweak<RadioGroup> {
    platform! {
        macos => mitsuami::appkit::tweak(move |s: &mitsuami::appkit::objc2_app_kit::NSStackView| {
            log.borrow_mut().push(s.arrangedSubviews().len())
        }),
        gtk => mitsuami::gtk::tweak(move |b: &mitsuami::gtk::gtk::Box| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(b.observe_children().n_items() as usize)
        }),
        kde => mitsuami::kirigami::tweak(move |c: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(c.int("mitsuamiCount") as usize)
        }),
        windows => mitsuami::winui::tweak(move |r: &mitsuami::winui::bindings::RadioButtons| {
            log.borrow_mut().push(r.Items()?.Size()? as usize);
            Ok(())
        }),
    }
}

/// Given before the options, the tweak still runs after them, and again
/// when they change.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_group_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let options = signal(vec!["Small".to_string(), "Large".to_string()]);
    let tweak = log_buttons(log.clone());
    app.mount(move || RadioGroup::new("Size").native(tweak).options(options));

    let props = app.get_by_role(Role::RadioGroup, "Size").native_state().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&2));
    options.update(|o| o.push("Huge".to_string()));
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&3));
}

mitsuami_test::main!();
