//! PasswordInput: the platform's password field. It takes text as a text
//! field does, hides it as the platform does, and reads as a text field
//! named by its label or placeholder whose value is never exposed.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Command, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A sign-in form: the password signs in on Return, or with the button.
fn sign_in(signed_in: Rc<RefCell<Vec<(String, String)>>>) -> impl View {
    let user = signal(String::new());
    let password = signal(String::new());
    let submit = move || signed_in.borrow_mut().push((user.get_untracked(), password.get_untracked()));
    let submit_on_enter = submit.clone();
    Column::new().padding(16).gap(8).children((
        TextInput::new().a11y_label("User").bind(user),
        PasswordInput::new().a11y_label("Password").placeholder("Password").bind(password).on_submit(submit_on_enter),
        Text::new(move || format!("{} characters", password.get().chars().count())),
        Button::new("Sign in").enabled(move || !password.get().is_empty()).on_click(submit),
    ))
}

fn mount(app: &TestApp) -> Rc<RefCell<Vec<(String, String)>>> {
    let signed_in = Rc::new(RefCell::new(Vec::new()));
    let s = signed_in.clone();
    app.mount(move || sign_in(s));
    signed_in
}

#[mitsuami_test::test]
async fn binds_both_ways(app: TestApp) {
    let password = signal("first".to_string());
    app.mount(move || PasswordInput::new().a11y_label("Password").bind(password));
    let field = app.get_by_label("Password");
    assert!(field.native_state().props.contains(&Prop::Value("first".into())));

    field.fill("second").await;
    assert_eq!(password.get_untracked(), "second");

    password.set("third".into());
    app.settle().await;
    assert!(field.native_state().props.contains(&Prop::Value("third".into())));
}

#[mitsuami_test::test]
async fn typing_updates_on_every_keystroke(app: TestApp) {
    mount(&app);
    let field = app.get_by_label("Password");

    field.type_text("hunter").await;
    app.expect(by_text("6 characters")).to_exist().await;
    field.press(Key::Backspace).await;
    app.expect(by_text("5 characters")).to_exist().await;
    assert!(field.native_state().props.contains(&Prop::Value("hunte".into())));
}

#[mitsuami_test::test]
async fn native_edits_are_not_echoed_back_to_the_widget(app: TestApp) {
    mount(&app);
    let input = app.get_by_label("Password").id();
    app.take_command_log();

    app.get_by_label("Password").type_text("pw").await;

    let echoes: Vec<Command> = app
        .take_command_log()
        .into_iter()
        .filter(|c| matches!(c, Command::SetProp { id, .. } if *id == input))
        .collect();
    assert_eq!(echoes, vec![], "the native password field already shows what the user typed");
}

#[mitsuami_test::test]
async fn enter_submits(app: TestApp) {
    let signed_in = mount(&app);
    app.get_by_label("User").fill("ada").await;
    app.get_by_label("Password").type_text("pw").await;
    app.get_by_label("Password").press(Key::Enter).await;
    assert_eq!(*signed_in.borrow(), [("ada".to_string(), "pw".to_string())]);
}

/// Tab moves on from it, as from a text field, and Return in the user field
/// doesn't sign in.
#[mitsuami_test::test]
async fn tab_moves_focus_on(app: TestApp) {
    let signed_in = mount(&app);
    app.get_by_label("User").type_text("ada").await;
    app.get_by_label("User").press(Key::Tab).await;
    app.expect(by_label("Password")).to_be_focused().await;

    app.get_by_label("Password").type_text("pw").await;
    app.get_by_label("Password").press(Key::Tab).await;
    app.expect(by_label("Password")).not_to_be_focused().await;
    assert!(signed_in.borrow().is_empty());
}

/// A text field, named by its label or its placeholder, that never
/// exposes its text.
#[mitsuami_test::test]
async fn reads_as_a_text_field_that_hides_its_value(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            PasswordInput::new().a11y_label("Password").value("secret"),
            PasswordInput::new().placeholder("Confirm"),
        ))
    });
    let field = app.get_by_role(Role::TextField, "Password");
    assert!(field.node().password);
    assert_eq!(field.value(), None);
    app.expect(by_role(Role::TextField, "Confirm")).to_exist().await;
    app.assert_a11y_snapshot("hidden");
}

#[mitsuami_test::test]
async fn a_disabled_field_takes_no_input(app: TestApp) {
    app.mount(|| PasswordInput::new().a11y_label("Password").enabled(false));
    let field = app.get_by_label("Password");
    app.expect(by_label("Password")).to_be_disabled().await;
    assert_eq!(app.ui().perform(field.id(), &A11yAction::SetValue("pw".into())), Err(ActionError::Disabled));
}

#[mitsuami_test::test]
async fn has_a_size_like_a_text_field(app: TestApp) {
    app.mount(|| {
        Column::new()
            .align(Align::Start)
            .children((TextInput::new().a11y_label("Text"), PasswordInput::new().a11y_label("Password")))
    });
    let text = app.get_by_label("Text").frame().size;
    let password = app.get_by_label("Password").frame().size;
    assert!(!password.is_empty());
    // Some platforms add a button to show the password; none makes the
    // field shorter.
    assert!(password.height >= text.height, "{password:?} is shorter than {text:?}");
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let password = signal(String::new());
    app.mount(move || view! { <PasswordInput a11y_label="Password" bind=password/> });
    app.get_by_label("Password").fill("pw").await;
    assert_eq!(password.get_untracked(), "pw");
}

/// Logs the text the native field holds, each time the tweak runs.
fn log_text(log: Rc<RefCell<Vec<String>>>) -> Tweak<PasswordInput> {
    platform! {
        macos => mitsuami::appkit::tweak(move |f: &mitsuami::appkit::objc2_app_kit::NSSecureTextField| {
            log.borrow_mut().push(f.stringValue().to_string())
        }),
        gtk => mitsuami::gtk::tweak(move |e: &mitsuami::gtk::gtk::PasswordEntry| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(e.text().to_string())
        }),
        kde => mitsuami::kirigami::tweak(move |f: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(f.str("text"))),
        windows => mitsuami::winui::tweak(move |f: &mitsuami::winui::bindings::PasswordBox| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(f.cast::<mitsuami::winui::bindings::IPasswordBox>()?.Password()?);
            Ok(())
        }),
    }
}

/// Given before the text, the tweak still runs after it, and again when it
/// changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_field_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let value = signal("first".to_string());
    let tweak = log_text(log.clone());
    app.mount(move || PasswordInput::new().a11y_label("Password").native(tweak).value(value));

    let props = app.get_by_label("Password").native_state().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last().map(String::as_str), Some("first"));
    value.set("second".into());
    app.settle().await;
    assert_eq!(log.borrow().last().map(String::as_str), Some("second"));
}

mitsuami_test::main!();
