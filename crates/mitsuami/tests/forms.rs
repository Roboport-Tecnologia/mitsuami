//! Text input, checkboxes, switches, two-way binding, and enabled and
//! read-only state.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Command, Prop, SyntheticInput};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn signup(submitted: Rc<RefCell<Vec<String>>>) -> impl View {
    let name = signal(String::new());
    let agreed = signal(false);
    let newsletter = signal(true);
    let submit = move || submitted.borrow_mut().push(name.get_untracked());
    let submit_on_enter = submit.clone();
    Column::new().padding(16).gap(8).children((
        TextInput::new().a11y_label("Name").placeholder("Your name").bind(name).on_submit(submit_on_enter),
        Text::new(move || {
            let name = name.get();
            if name.is_empty() { "Hello, stranger".to_string() } else { format!("Hello, {name}") }
        }),
        Checkbox::new("I agree to the terms").bind(agreed),
        Switch::new("Newsletter").bind(newsletter),
        Text::new(move || format!("newsletter: {}", newsletter.get())),
        Button::new("Sign up").role(ButtonRole::Default).enabled(agreed).on_click(submit),
    ))
}

fn mount(app: &TestApp) -> Rc<RefCell<Vec<String>>> {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let s = submitted.clone();
    app.mount(move || signup(s));
    submitted
}

#[mitsuami_test::test]
async fn text_input_binds_both_ways(app: TestApp) {
    mount(&app);
    let name = app.get_by_label("Name");
    name.fill("Ada").await;

    app.expect(by_text("Hello, Ada")).to_be_visible().await;
    app.expect(by_label("Name")).to_have_value("Ada").await;
}

#[mitsuami_test::test]
async fn typing_updates_on_every_keystroke(app: TestApp) {
    mount(&app);
    let name = app.get_by_label("Name");

    name.type_text("Grace").await;
    app.expect(by_text("Hello, Grace")).to_exist().await;

    name.press(Key::Backspace).await;
    app.expect(by_text("Hello, Grac")).to_exist().await;
}

#[mitsuami_test::test]
async fn native_edits_are_not_echoed_back_to_the_widget(app: TestApp) {
    mount(&app);
    let input = app.get_by_label("Name").id();
    app.take_command_log();

    app.get_by_label("Name").type_text("Al").await;

    let echoes: Vec<Command> = app
        .take_command_log()
        .into_iter()
        .filter(|c| matches!(c, Command::SetProp { id, .. } if *id == input))
        .collect();
    assert_eq!(echoes, vec![], "the native text field already shows what the user typed");
}

#[mitsuami_test::test]
async fn placeholder_names_an_unlabelled_field(app: TestApp) {
    app.mount(|| TextInput::new().placeholder("Search"));
    app.expect(by_role(Role::TextField, "Search")).to_exist().await;
}

#[mitsuami_test::test]
async fn button_is_enabled_by_a_bound_checkbox(app: TestApp) {
    let submitted = mount(&app);
    app.get_by_label("Name").fill("Ada").await;
    app.expect(by_role(Role::Button, "Sign up")).to_be_disabled().await;

    let sign_up = app.get_by_role(Role::Button, "Sign up").id();
    assert_eq!(
        app.ui().perform(sign_up, &mitsuami::core::A11yAction::Activate),
        Err(mitsuami::core::ActionError::Disabled)
    );

    app.get_by_role(Role::Checkbox, "I agree to the terms").check().await;
    app.expect(by_role(Role::Checkbox, "I agree to the terms")).to_be_checked().await;
    app.get_by_role(Role::Button, "Sign up").click().await;

    assert_eq!(*submitted.borrow(), ["Ada"]);
}

#[mitsuami_test::test]
async fn enter_submits_the_text_field(app: TestApp) {
    let submitted = mount(&app);
    app.get_by_label("Name").type_text("Linus").await;
    app.get_by_label("Name").press(Key::Enter).await;
    assert_eq!(*submitted.borrow(), ["Linus"]);
}

#[mitsuami_test::test]
async fn switch_toggles_its_bound_signal(app: TestApp) {
    mount(&app);
    app.expect(by_role(Role::Switch, "Newsletter")).to_be_checked().await;

    app.get_by_role(Role::Switch, "Newsletter").click().await;

    app.expect(by_role(Role::Switch, "Newsletter")).not_to_be_checked().await;
    app.expect(by_text("newsletter: false")).to_exist().await;
}

#[mitsuami_test::test]
async fn programmatic_changes_reach_the_native_widget(app: TestApp) {
    let value = signal("first".to_string());
    app.mount(move || TextInput::new().a11y_label("Field").bind(value));

    value.set("second".into());
    app.settle().await;

    assert!(app.get_by_label("Field").native_state().props.contains(&Prop::Value("second".into())));
}

/// A read-only field keeps its text: nothing can be typed into it, and
/// assistive technology can't set it. (Whether it takes keyboard focus is
/// the platform's: AppKit's don't, unless Full Keyboard Access is on.)
#[mitsuami_test::test]
async fn a_read_only_field_takes_no_edits(app: TestApp) {
    let edits = Rc::new(RefCell::new(Vec::new()));
    let log = edits.clone();
    app.mount(move || {
        TextInput::new()
            .a11y_label("Key")
            .value("ABCD-1234")
            .read_only(true)
            .on_input(move |text| log.borrow_mut().push(text))
    });

    let field = app.get_by_label("Key");
    app.expect(by_label("Key")).to_be_read_only().await;
    for key in [Key::Char('x'), Key::Backspace] {
        assert_eq!(app.ui().synthesize(field.id(), &SyntheticInput::Key(key)), Err(ActionError::ReadOnly));
    }
    assert_eq!(app.ui().perform(field.id(), &A11yAction::SetValue("y".into())), Err(ActionError::ReadOnly));
    app.settle().await;
    assert_eq!(field.value().as_deref(), Some("ABCD-1234"));
    assert!(field.native_state().props.contains(&Prop::Value("ABCD-1234".into())));
    assert!(edits.borrow().is_empty());
}

/// Read-only can change, and the app can still set the text of a read-only
/// field.
#[mitsuami_test::test]
async fn read_only_follows_its_signal(app: TestApp) {
    let read_only = signal(true);
    let value = signal("first".to_string());
    app.mount(move || TextInput::new().a11y_label("Field").bind(value).read_only(read_only));

    value.set("second".into());
    app.settle().await;
    assert!(app.get_by_label("Field").native_state().props.contains(&Prop::Value("second".into())));

    read_only.set(false);
    app.expect(by_label("Field")).to_be_editable().await;
    assert!(app.get_by_label("Field").native_state().props.contains(&Prop::ReadOnly(false)));
    app.get_by_label("Field").type_text("!").await;
    assert_eq!(value.get_untracked(), "second!");
}

/// Logs the text the native field shows, each time the tweak runs.
fn log_text(log: Rc<RefCell<Vec<String>>>) -> Tweak<TextInput> {
    platform! {
        macos => mitsuami::appkit::tweak(move |f: &mitsuami::appkit::objc2_app_kit::NSTextField| {
            log.borrow_mut().push(f.stringValue().to_string())
        }),
        gtk => mitsuami::gtk::tweak(move |e: &mitsuami::gtk::gtk::Entry| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(e.text().to_string())
        }),
        kde => mitsuami::kirigami::tweak(move |f: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(f.str("text"))),
        windows => mitsuami::winui::tweak(move |f: &mitsuami::winui::bindings::TextBox| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(f.cast::<mitsuami::winui::bindings::ITextBox>()?.Text()?);
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
    app.mount(move || TextInput::new().a11y_label("Field").native(tweak).value(value));

    let props = app.get_by_label("Field").native_state().props;
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

#[mitsuami_test::test]
async fn form_a11y_snapshot(app: TestApp) {
    mount(&app);
    app.get_by_label("Name").fill("Ada").await;
    app.assert_a11y_snapshot("filled");
}

mitsuami_test::main!();
