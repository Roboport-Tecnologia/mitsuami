//! `ToggleButton`: a button that stays pressed until it's clicked again,
//! as the platform makes one. It reads as a toggle button, checked while
//! it's pressed, reports the user's clicks, and follows the app's value
//! without reporting it.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn bold() -> Query {
    by_role(Role::ToggleButton, "Bold")
}

fn has(app: &TestApp, prop: Prop) -> bool {
    app.get(bold()).native_state().props.contains(&prop)
}

#[mitsuami_test::test]
async fn reads_as_a_toggle_button(app: TestApp) {
    app.mount(|| ToggleButton::new("Bold"));

    assert_eq!(app.get(bold()).native_state().kind, WidgetKind::ToggleButton);
    assert!(!app.get(bold()).is_checked());
    assert!(!has(&app, Prop::Checked(true)));
}

#[mitsuami_test::test]
async fn a_click_presses_it_and_another_lets_it_go(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || ToggleButton::new("Bold").on_change(move |on| log.borrow_mut().push(on)));

    app.get(bold()).click().await;
    app.expect(bold()).to_be_checked().await;
    assert!(has(&app, Prop::Checked(true)));

    app.get(bold()).click().await;
    assert!(!app.get(bold()).is_checked());
    assert_eq!(*seen.borrow(), [true, false]);
}

#[mitsuami_test::test]
async fn space_presses_it(app: TestApp) {
    let bold_on = signal(false);
    app.mount(move || ToggleButton::new("Bold").bind(bold_on));

    app.get(bold()).press(Key::Char(' ')).await;
    assert!(bold_on.get_untracked());
}

/// Values the app sets are shown, and not reported back as the user's.
#[mitsuami_test::test]
async fn follows_the_app_without_reporting_it(app: TestApp) {
    let on = signal(false);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || ToggleButton::new("Bold").checked(on).on_change(move |c| log.borrow_mut().push(c)));

    on.set(true);
    app.settle().await;
    app.expect(bold()).to_be_checked().await;
    assert!(has(&app, Prop::Checked(true)));
    on.set(false);
    app.settle().await;
    assert!(has(&app, Prop::Checked(false)));
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
}

#[mitsuami_test::test]
async fn refuses_clicks_while_disabled(app: TestApp) {
    let on = signal(false);
    app.mount(move || ToggleButton::new("Bold").bind(on).enabled(false));

    app.expect(bold()).to_be_disabled().await;
    let id = app.get(bold()).id();
    assert_eq!(app.ui().perform(id, &A11yAction::Activate), Err(ActionError::Disabled));
    assert!(!on.get_untracked());
}

/// Its caption stays its name when it shows only its icon, as a button's.
#[mitsuami_test::test]
async fn shows_an_icon(app: TestApp) {
    let icon = platform! {
        macos => "bold",
        gtk => "format-text-bold-symbolic",
        kde => "format-text-bold",
        windows => "\u{E8DD}",
        _ => "bold",
    };
    app.mount(move || Row::new().child(ToggleButton::new("Bold").icon(icon).icon_only(true)));

    assert!(has(&app, Prop::Icon(icon.to_string())));
    assert!(has(&app, Prop::IconOnly(true)));
    let frame = app.ui().frame(app.get(bold()).id()).unwrap();
    assert!(frame.width() > 0.0 && frame.height() > 0.0, "{frame:?}");
}

#[mitsuami_test::test]
async fn takes_focus(app: TestApp) {
    app.mount(|| ToggleButton::new("Bold"));

    app.get(bold()).focus().await;
    app.expect(bold()).to_be_focused().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let on = signal(false);
    app.mount(move || view! { <ToggleButton bind=on>"Bold"</ToggleButton> });

    app.get(bold()).click().await;
    assert!(on.get_untracked());
}

mitsuami_test::main!();
