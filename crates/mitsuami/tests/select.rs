//! `Select`: a native pop-up menu of text options. It shows its options and
//! the chosen one (always one, unless it has none), reports what the user
//! chooses, follows reactive options and choices, and reads as a combo box
//! named by its label whose value is the chosen option.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

const COLORS: [&str; 3] = ["Red", "Green", "Blue"];

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

fn options(names: &[&str]) -> Prop {
    Prop::Options(names.iter().map(|n| n.to_string()).collect())
}

#[mitsuami_test::test]
async fn shows_its_options_and_the_chosen_one(app: TestApp) {
    app.mount(|| Select::new("Color").options(COLORS).selected(1));
    let color = by_role(Role::ComboBox, "Color");

    assert_eq!(app.get(color.clone()).native_state().kind, WidgetKind::Select);
    assert!(has(&app, color.clone(), options(&COLORS)));
    assert!(has(&app, color.clone(), Prop::SelectedIndex(Some(1))));
    app.expect(color).to_have_value("Green").await;
}

#[mitsuami_test::test]
async fn chooses_the_first_option_unless_told_otherwise(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Select::new("Color").options(COLORS),
            Select::new("Past the end").options(COLORS).selected(7),
            Select::new("Empty"),
        ))
    });

    assert!(has(&app, by_role(Role::ComboBox, "Color"), Prop::SelectedIndex(Some(0))));
    app.expect(by_role(Role::ComboBox, "Color")).to_have_value("Red").await;
    assert!(has(&app, by_role(Role::ComboBox, "Past the end"), Prop::SelectedIndex(Some(0))));
    assert!(has(&app, by_role(Role::ComboBox, "Empty"), options(&[])));
    assert!(has(&app, by_role(Role::ComboBox, "Empty"), Prop::SelectedIndex(None)));
    app.expect(by_role(Role::ComboBox, "Empty")).to_have_value("").await;
}

#[mitsuami_test::test]
async fn reports_the_option_the_user_chooses_and_shows_it(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || Select::new("Color").options(COLORS).selected(1).on_change(move |i| log.borrow_mut().push(i)));
    let color = app.get_by_role(Role::ComboBox, "Color");

    color.select_option("Blue").await;
    color.select_option("Red").await;

    assert_eq!(*seen.borrow(), [2, 0]);
    assert!(has(&app, by_role(Role::ComboBox, "Color"), Prop::SelectedIndex(Some(0))));
    app.expect(by_role(Role::ComboBox, "Color")).to_have_value("Red").await;
}

#[mitsuami_test::test]
async fn binds_both_ways(app: TestApp) {
    let chosen = signal(0);
    app.mount(move || {
        Column::new().children((
            Select::new("Color").options(COLORS).bind(chosen),
            Text::new(move || format!("Chosen: {}", chosen.get())),
        ))
    });

    app.get_by_role(Role::ComboBox, "Color").select_option("Green").await;
    assert_eq!(chosen.get_untracked(), 1);
    app.expect(by_text("Chosen: 1")).to_exist().await;

    chosen.set(2);
    app.expect(by_role(Role::ComboBox, "Color")).to_have_value("Blue").await;
    assert!(has(&app, by_role(Role::ComboBox, "Color"), Prop::SelectedIndex(Some(2))));
}

#[mitsuami_test::test]
async fn follows_reactive_options(app: TestApp) {
    let names = signal(vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()]);
    app.mount(move || Select::new("Color").options(names).selected(2));
    let color = by_role(Role::ComboBox, "Color");
    app.expect(color.clone()).to_have_value("Blue").await;

    // The chosen index stays when the options change around it.
    names.set(vec!["Cyan".into(), "Magenta".into(), "Yellow".into(), "Black".into()]);
    app.expect(color.clone()).to_have_value("Yellow").await;
    assert!(has(&app, color.clone(), options(&["Cyan", "Magenta", "Yellow", "Black"])));
    assert!(has(&app, color.clone(), Prop::SelectedIndex(Some(2))));

    // Past the options, the first is chosen.
    names.set(vec!["Black".into(), "White".into()]);
    app.expect(color.clone()).to_have_value("Black").await;
    assert!(has(&app, color.clone(), options(&["Black", "White"])));
    assert!(has(&app, color.clone(), Prop::SelectedIndex(Some(0))));

    // Without options, nothing is; with them back, the first is.
    names.set(Vec::new());
    app.expect(color.clone()).to_have_value("").await;
    assert!(has(&app, color.clone(), Prop::SelectedIndex(None)));
    names.set(vec!["Cyan".into(), "Magenta".into(), "Yellow".into()]);
    app.expect(color.clone()).to_have_value("Yellow").await;
}

#[mitsuami_test::test]
async fn keeps_options_with_the_same_text_apart(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        Select::new("Size")
            .options(["Small", "Large", "Small"])
            .selected(2)
            .on_change(move |i| log.borrow_mut().push(i))
    });
    let size = by_role(Role::ComboBox, "Size");

    assert!(has(&app, size.clone(), options(&["Small", "Large", "Small"])));
    assert!(has(&app, size.clone(), Prop::SelectedIndex(Some(2))));
    app.get(size).select_option("Large").await;
    assert_eq!(*seen.borrow(), [1]);
}

#[mitsuami_test::test]
async fn refuses_options_it_does_not_have_and_disabled_choices(app: TestApp) {
    app.mount(|| {
        Column::new()
            .children((Select::new("Color").options(COLORS), Select::new("Off").options(COLORS).enabled(false)))
    });
    let color = app.get_by_role(Role::ComboBox, "Color").id();
    let off = app.get_by_role(Role::ComboBox, "Off").id();

    assert_eq!(app.ui().perform(color, &A11yAction::SetValue("Purple".into())), Err(ActionError::Unsupported));
    assert_eq!(app.ui().perform(off, &A11yAction::SetValue("Red".into())), Err(ActionError::Disabled));
    app.settle().await;
    assert!(has(&app, by_role(Role::ComboBox, "Off"), Prop::Enabled(false)));
    app.expect(by_role(Role::ComboBox, "Color")).to_have_value("Red").await;
}

// Whether it's sized for its widest option (AppKit) or the chosen one
// (GTK, XAML, Qt) is the platform's call.
#[mitsuami_test::test]
async fn is_sized_for_its_options(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            Select::new("Short").options(["A", "B"]),
            Select::new("Long").options(["A considerably longer option", "A"]),
        ))
    });
    let short = app.get_by_role(Role::ComboBox, "Short").frame();
    let long = app.get_by_role(Role::ComboBox, "Long").frame();

    assert!(!short.size.is_empty());
    assert!(long.width() > short.width(), "{long} is not wider than {short}");
}

// Whether Tab reaches it is the platform's call: macOS skips pop-up buttons
// unless Full Keyboard Access is on, as it skips buttons.
#[mitsuami_test::test]
async fn takes_focus(app: TestApp) {
    app.mount(|| Column::new().children((TextInput::new().a11y_label("Name"), Select::new("Color").options(COLORS))));

    app.get_by_role(Role::ComboBox, "Color").focus().await;
    app.expect(by_role(Role::ComboBox, "Color")).to_be_focused().await;

    app.get_by_label("Name").focus().await;
    app.expect(by_role(Role::ComboBox, "Color")).not_to_be_focused().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let chosen = signal(1);
    app.mount(move || view! { <Select label="Color" options=COLORS bind=chosen/> });

    app.expect(by_role(Role::ComboBox, "Color")).to_have_value("Green").await;
    app.get_by_role(Role::ComboBox, "Color").select_option("Red").await;
    assert_eq!(chosen.get_untracked(), 0);
}

/// Logs how many options the native select shows, each time the tweak runs.
fn log_options(log: Rc<RefCell<Vec<usize>>>) -> Tweak<Select> {
    platform! {
        macos => mitsuami::appkit::tweak(move |p: &mitsuami::appkit::objc2_app_kit::NSPopUpButton| {
            log.borrow_mut().push(p.numberOfItems() as usize)
        }),
        gtk => mitsuami::gtk::tweak(move |d: &mitsuami::gtk::gtk::DropDown| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(d.model().map_or(0, |m| m.n_items() as usize))
        }),
        kde => mitsuami::kirigami::tweak(move |c: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(c.int("count") as usize)
        }),
        windows => mitsuami::winui::tweak(move |c: &mitsuami::winui::bindings::ComboBox| {
            use mitsuami::winui::windows_core::Interface;
            let items = c.cast::<mitsuami::winui::bindings::IItemsControl>()?.Items()?;
            log.borrow_mut().push(items.Size()? as usize);
            Ok(())
        }),
    }
}

/// Given before the options, the tweak still runs after them, and again
/// when they change.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_select_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let options = signal(vec!["Red".to_string(), "Green".to_string()]);
    let tweak = log_options(log.clone());
    app.mount(move || Select::new("Color").native(tweak).options(options));

    let props = app.get_by_role(Role::ComboBox, "Color").native_state().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&2));
    options.update(|o| o.push("Blue".to_string()));
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&3));
}

mitsuami_test::main!();
