//! Keys: a container, group, list or table takes keys while it, or a
//! control inside it, has keyboard focus, and the focused control doesn't
//! use them itself. Tests press keys on the focused control, and they go
//! up as the platform sends them on: AppKit's responder chain, GTK's
//! bubbling, Qt's unaccepted key events, XAML's bubbling `KeyDown`.
//!
//! The focused control is a list here where the key must pass it: a list
//! takes focus on every platform (AppKit's buttons only with Full Keyboard
//! Access), and uses only its own keys.

use mitsuami::core::{ActionError, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn files() -> Vec<String> {
    ["notes.txt", "photo.png", "todo.md"].map(String::from).to_vec()
}

fn file_list(selected: Signal<Vec<String>>) -> List<String, String> {
    List::new(files(), |f: &String| f.clone(), |f| Container::new().padding(4).child(Text::new(f)))
        .selected(selected)
        .selection_mode(SelectionMode::Multiple)
        .a11y_label("Files")
        .height(120)
}

/// A handler that notes what it heard in `log`.
fn hear(log: Signal<Vec<String>>, what: &'static str) -> impl Fn() {
    move || log.update(|l| l.push(what.to_owned()))
}

#[mitsuami_test::test]
async fn a_container_takes_keys_the_focused_control_passes_on(app: TestApp) {
    let log = signal(Vec::new());
    let selected = signal(Vec::new());
    app.mount(move || {
        Column::new()
            .test_id("browser")
            .on_key(Key::F(2), hear(log, "rename"))
            .on_key(Shortcut::new(Key::Enter).alt(), hear(log, "properties"))
            .child(file_list(selected))
    });
    let list = app.get_by_role(Role::List, "Files");

    list.press(Key::F(2)).await;
    list.press(Shortcut::new(Key::Enter).alt()).await;
    assert_eq!(log.get_untracked(), ["rename", "properties"]);
    app.expect(by_role(Role::List, "Files")).to_be_focused().await;
}

/// The focused control's own keys stay its own: a list's arrows move the
/// selection (AppKit's table keeps them with ⌘ and ⌥ too, so Finder's
/// ⌘↑ and ⌘↓ are menu shortcuts), a text field's space is typed.
#[mitsuami_test::test]
async fn the_focused_control_uses_its_own_keys_first(app: TestApp) {
    let log = signal(Vec::new());
    let (selected, name) = (signal(vec!["notes.txt".to_owned()]), signal(String::new()));
    app.mount(move || {
        Column::new()
            .on_key(Key::Down, hear(log, "down"))
            .on_key(' ', hear(log, "space"))
            .children((file_list(selected), TextInput::new().bind(name).a11y_label("Name")))
    });

    app.get_by_role(Role::List, "Files").press(Key::Down).await;
    assert_eq!(selected.get_untracked(), ["photo.png"]);
    app.get_by_label("Name").type_text("a b").await;
    assert_eq!(name.get_untracked(), "a b");
    assert!(log.get_untracked().is_empty(), "{:?}", log.get_untracked());
}

/// A key goes to the nearest node around the focused control that takes
/// it; one it doesn't take goes on up.
#[mitsuami_test::test]
async fn the_nearest_node_that_takes_a_key_gets_it(app: TestApp) {
    let log = signal(Vec::new());
    let selected = signal(Vec::new());
    app.mount(move || {
        Column::new()
            .on_key(Key::Delete, hear(log, "outer delete"))
            .on_key(Key::F(5), hear(log, "outer refresh"))
            .child(
                Group::new()
                    .title("Documents")
                    .on_key(Key::Delete, hear(log, "group delete"))
                    .child(file_list(selected).on_key(Key::Delete, hear(log, "list delete"))),
            )
    });
    let list = app.get_by_role(Role::List, "Files");

    list.press(Key::Delete).await;
    list.press(Key::F(5)).await;
    assert_eq!(log.get_untracked(), ["list delete", "outer refresh"]);
}

/// Move to Trash on a list, with its rows selected: ⌘⌫ in Finder, Delete
/// in Nautilus, Dolphin and File Explorer. (Finder's Quick Look is on
/// Space, which GTK's and XAML's lists keep: it selects the focused row.)
#[mitsuami_test::test]
async fn a_list_takes_keys_for_its_selection(app: TestApp) {
    let selected = signal(vec!["photo.png".to_owned()]);
    let trashed = signal(Vec::<String>::new());
    let trash = platform! { macos => Shortcut::primary(Key::Backspace), _ => Shortcut::new(Key::Delete) };
    app.mount(move || file_list(selected).on_key(trash, move || trashed.set(selected.get_untracked())));

    app.get_by_role(Role::List, "Files").press(trash).await;
    assert_eq!(trashed.get_untracked(), ["photo.png"]);
}

/// A key nothing takes goes nowhere, as a platform beeps at it; the node
/// carries its keys, read back from the platform where it can.
#[mitsuami_test::test]
async fn keys_nothing_takes_go_nowhere(app: TestApp) {
    let log = signal(Vec::new());
    let selected = signal(Vec::new());
    app.mount(move || Column::new().test_id("box").on_key(Key::F(2), hear(log, "rename")).child(file_list(selected)));

    let list = app.get_by_role(Role::List, "Files").node().id;
    let result = app.ui().synthesize(list, &mitsuami::core::backend::SyntheticInput::Key(Key::F(3)));
    assert_eq!(result, Err(ActionError::Unsupported));
    assert!(log.get_untracked().is_empty());
    let props = app.get(by_test_id("box")).native_state().props;
    assert!(props.contains(&Prop::Keys(vec![Shortcut::new(Key::F(2))])), "{props:?}");
}

mitsuami_test::main!();
