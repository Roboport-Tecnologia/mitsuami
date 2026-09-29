//! `Button`: its semantic props (a role, a style) reach the native button
//! and don't change what it is, and a `Tweak` reaches the native button
//! itself, after the other props, whenever they change.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{Command, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn has(app: &TestApp, name: &str, prop: Prop) -> bool {
    app.get_by_role(Role::Button, name).native_state().props.contains(&prop)
}

#[mitsuami_test::test]
async fn roles_and_styles_reach_the_native_button(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Button::new("Save").role(ButtonRole::Default),
            Button::new("Cancel").role(ButtonRole::Cancel),
            Button::new("Delete").role(ButtonRole::Destructive).button_style(ButtonStyle::Bordered),
            Button::new("More").button_style(ButtonStyle::Borderless),
        ))
    });

    assert!(has(&app, "Save", Prop::ButtonRole(ButtonRole::Default)));
    assert!(has(&app, "Cancel", Prop::ButtonRole(ButtonRole::Cancel)));
    assert!(has(&app, "Delete", Prop::ButtonRole(ButtonRole::Destructive)));
    assert!(has(&app, "Delete", Prop::ButtonStyle(ButtonStyle::Bordered)));
    assert!(has(&app, "More", Prop::ButtonStyle(ButtonStyle::Borderless)));
}

/// Borderless buttons lose their bezel on AppKit and keep their padding
/// elsewhere: never larger than a bordered one.
#[mitsuami_test::test]
async fn a_borderless_button_is_no_larger_than_a_bordered_one(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            Button::new("Bordered"),
            Button::new("Borderless").a11y_label("Plain").button_style(ButtonStyle::Borderless),
        ))
    });

    let bordered = app.get_by_role(Role::Button, "Bordered").frame();
    let borderless = app.get_by_role(Role::Button, "Plain").frame();
    assert!(borderless.height() <= bordered.height(), "{borderless:?} is taller than {bordered:?}");
}

#[mitsuami_test::test]
async fn changing_the_role_or_style_doesnt_click(app: TestApp) {
    let role = signal(ButtonRole::Normal);
    let style = signal(ButtonStyle::Automatic);
    let clicks = signal(0);
    app.mount(move || Button::new("Save").role(role).button_style(style).on_click(move || clicks.update(|c| *c += 1)));

    role.set(ButtonRole::Default);
    style.set(ButtonStyle::Borderless);
    app.settle().await;
    role.set(ButtonRole::Destructive);
    app.settle().await;

    assert!(has(&app, "Save", Prop::ButtonRole(ButtonRole::Destructive)));
    assert!(has(&app, "Save", Prop::ButtonStyle(ButtonStyle::Borderless)));
    assert_eq!(clicks.get_untracked(), 0);
    app.expect(by_role(Role::Button, "Save")).to_be_enabled().await;

    // It's still a button, and clicks.
    app.get_by_role(Role::Button, "Save").click().await;
    assert_eq!(clicks.get_untracked(), 1);
}

/// Logs whether the native button is enabled, each time the tweak runs.
fn log_enabled(log: Rc<RefCell<Vec<bool>>>) -> Tweak<Button> {
    platform! {
        macos => mitsuami::appkit::tweak(move |b: &mitsuami::appkit::objc2_app_kit::NSButton| {
            log.borrow_mut().push(b.isEnabled())
        }),
        gtk => mitsuami::gtk::tweak(move |b: &mitsuami::gtk::gtk::Button| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(b.is_sensitive())
        }),
        kde => mitsuami::kirigami::tweak(move |b: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(b.bool("enabled"))
        }),
        win32 => mitsuami::win32::tweak(move |b: &mitsuami::win32::PushButton| {
            use mitsuami::win32::Control;
            log.borrow_mut().push(b.is_enabled())
        }),
        windows => mitsuami::winui::tweak(move |b: &mitsuami::winui::bindings::Button| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(b.cast::<mitsuami::winui::bindings::IControl>()?.IsEnabled()?);
            Ok(())
        }),
    }
}

/// Given before `enabled`, the tweak still runs after it: it sees the
/// button disabled, then enabled.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_button_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let enabled = signal(false);
    let tweak = log_enabled(log.clone());
    app.mount(move || Button::new("Save").native(tweak).enabled(enabled));

    // The tweak travels with the button either way.
    assert!(app.get_by_role(Role::Button, "Save").native_state().props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        // No native button to run it on.
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&false));

    enabled.set(true);
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&true));
}

#[mitsuami_test::test]
async fn a_tweak_with_a_value_runs_again_when_it_changes(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    let size = signal(1u8);
    let record = move |size: &u8| log.borrow_mut().push(*size);
    let tweak: Tweak<Button> = platform! {
        macos => mitsuami::appkit::tweak_with(size, move |_: &mitsuami::appkit::objc2_app_kit::NSButton, s| record(s)),
        gtk => mitsuami::gtk::tweak_with(size, move |_: &mitsuami::gtk::gtk::Button, s| record(s)),
        kde => mitsuami::kirigami::tweak_with(size, move |_, s| record(s)),
        win32 => mitsuami::win32::tweak_with(size, move |_: &mitsuami::win32::PushButton, s| record(s)),
        windows => mitsuami::winui::tweak_with(size, move |_: &mitsuami::winui::bindings::Button, s| {
            record(s);
            Ok(())
        }),
    };
    app.mount(move || Button::new("Save").native(tweak));
    app.take_command_log();

    size.set(2);
    app.settle().await;

    // Only the tweak is sent again.
    let sent: Vec<Prop> = app
        .take_command_log()
        .into_iter()
        .filter_map(|c| match c {
            Command::SetProp { prop, .. } => Some(prop),
            _ => None,
        })
        .collect();
    assert!(!sent.is_empty() && sent.iter().all(|p| matches!(p, Prop::Tweak(_))), "{sent:?}");
    if app.is_headless() {
        assert!(seen.borrow().is_empty());
        return;
    }
    assert_eq!(seen.borrow().first(), Some(&1));
    assert_eq!(seen.borrow().last(), Some(&2));
}

/// `Tweak::none()` is for platforms the app leaves alone: nothing is sent.
#[mitsuami_test::test]
async fn no_tweak_sends_nothing(app: TestApp) {
    app.mount(|| Button::new("Save").native(Tweak::none()));
    let props = app.get_by_role(Role::Button, "Save").native_state().props;
    assert!(!props.iter().any(|p| matches!(p, Prop::Tweak(_))));
}

mitsuami_test::main!();
