//! A text field's error (`Prop::Error`): what's wrong with its text. The
//! platform marks the field where it has a way to (GTK's `error` style;
//! AppKit, WinUI and Qt's fields have no error state), and assistive
//! technology reads it as invalid, with the message as its description.
//! The app shows the message by the field.

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn error(app: &TestApp, query: Query) -> Option<Option<String>> {
    app.get(query).native_state().props.into_iter().find_map(|p| match p {
        Prop::Error(e) => Some(e),
        _ => None,
    })
}

/// Whether it's read as invalid, and its description.
fn read(app: &TestApp, query: Query) -> (bool, Option<String>) {
    let id = app.get(query).id();
    let tree = app.ui().a11y_tree(app.window()).expect("a window");
    let node = tree.walk().into_iter().find(|n| n.id == id).expect("in the tree").clone();
    (node.invalid, node.description)
}

#[mitsuami_test::test]
async fn an_error_is_carried_and_read_as_invalid(app: TestApp) {
    let name = signal(Some("Remote name is required".to_owned()));
    app.mount(move || {
        Column::new().children((
            TextInput::new().test_id("name").tooltip("The remote's name").error(name),
            PasswordInput::new().test_id("password").error(Some("Too short".to_owned())),
            TextArea::new().test_id("message").error(Some("Subject is too long".to_owned())),
        ))
    });
    app.expect(by_test_id("name")).to_be_visible().await;

    assert_eq!(error(&app, by_test_id("name")), Some(Some("Remote name is required".into())));
    assert_eq!(read(&app, by_test_id("name")), (true, Some("Remote name is required".into())));
    assert_eq!(error(&app, by_test_id("password")), Some(Some("Too short".into())));
    assert_eq!(read(&app, by_test_id("password")), (true, Some("Too short".into())));
    assert_eq!(error(&app, by_test_id("message")), Some(Some("Subject is too long".into())));
    assert_eq!(read(&app, by_test_id("message")), (true, Some("Subject is too long".into())));

    // Fixed: valid again, and its tooltip is its description once more.
    name.set(None);
    app.settle().await;
    assert_eq!(error(&app, by_test_id("name")), Some(None));
    assert_eq!(read(&app, by_test_id("name")), (false, Some("The remote's name".into())));
}

mitsuami_test::main!();
