//! Tooltips: text the platform shows when the pointer rests on a widget,
//! its own way (delay, placement, look). Any widget or container can have
//! one; the native widget carries it, and assistive technology reads it as
//! the description. Showing it on hover isn't tested: nothing here can
//! rest a pointer on a native widget.

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn tooltip(app: &TestApp, query: Query) -> Option<String> {
    app.get(query).native_state().props.into_iter().find_map(|p| match p {
        Prop::Tooltip(t) => Some(t),
        _ => None,
    })
}

fn description(app: &TestApp, query: Query) -> Option<String> {
    let id = app.get(query).id();
    let tree = app.ui().a11y_tree(app.window()).expect("a window");
    tree.walk().into_iter().find(|n| n.id == id).and_then(|n| n.description.clone())
}

#[mitsuami_test::test]
async fn any_widget_carries_one(app: TestApp) {
    let red = Pixels::new(1, 1, [255, 0, 0, 255]);
    app.mount(move || {
        Column::new().test_id("box").tooltip("The box").children((
            Text::new("Status").tooltip("The whole status"),
            Button::new("Play").tooltip("Starts the machine"),
            TextInput::new().a11y_label("Name").tooltip("What the machine is called"),
            Checkbox::new("Sound").tooltip("Emulates a Sound Blaster"),
            Select::new("Family").options(["98", "XP"]).tooltip("Which Windows"),
            NumberInput::new("Memory").tooltip("In MB"),
            Image::pixels(red.clone()).label("Preview").tooltip("The shader's preview"),
        ))
    });

    for (query, text) in [
        (by_test_id("box"), "The box"),
        (by_text("Status"), "The whole status"),
        (by_role(Role::Button, "Play"), "Starts the machine"),
        (by_label("Name"), "What the machine is called"),
        (by_role(Role::Checkbox, "Sound"), "Emulates a Sound Blaster"),
        (by_role(Role::ComboBox, "Family"), "Which Windows"),
        (by_role(Role::SpinButton, "Memory"), "In MB"),
        (by_role(Role::Image, "Preview"), "The shader's preview"),
    ] {
        assert_eq!(tooltip(&app, query), Some(text.to_owned()), "{text}");
    }
}

#[mitsuami_test::test]
async fn follows_changes_and_goes_away_when_empty(app: TestApp) {
    let text = signal("Starts the machine".to_owned());
    app.mount(move || Button::new("Play").tooltip(text));
    let play = || by_role(Role::Button, "Play");

    text.set("Already running".into());
    app.settle().await;
    assert_eq!(tooltip(&app, play()), Some("Already running".into()));
    assert_eq!(description(&app, play()), Some("Already running".into()));

    text.set(String::new());
    app.settle().await;
    assert_eq!(tooltip(&app, play()), Some(String::new()));
    assert_eq!(description(&app, play()), None);
}

/// Read as the description, unless the app gives one of its own.
#[mitsuami_test::test]
async fn reads_as_the_description(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Button::new("Play").tooltip("Starts the machine"),
            Button::new("Edit").tooltip("Opens the editor").a11y_description("Edits the machine's settings"),
        ))
    });

    assert_eq!(description(&app, by_role(Role::Button, "Play")), Some("Starts the machine".into()));
    assert_eq!(description(&app, by_role(Role::Button, "Edit")), Some("Edits the machine's settings".into()));
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    app.mount(|| view! { <Button tooltip="Starts the machine">"Play"</Button> });

    assert_eq!(tooltip(&app, by_role(Role::Button, "Play")), Some("Starts the machine".into()));
}

mitsuami_test::main!();
