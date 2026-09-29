//! Focus from code: a `NodeRef` focuses its control, and selects part of
//! a text field's text, as file managers select a name without its
//! extension to rename it. Asked before the control is built (a window's
//! `on_open`), it's done once it is.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn a_node_ref_focuses_its_control(app: TestApp) {
    app.mount(move || {
        let second = node_ref();
        Column::new().children((
            TextInput::new().a11y_label("First"),
            TextInput::new().a11y_label("Second").node_ref(second),
            Button::new("Next").on_click(move || second.focus()),
        ))
    });
    app.get_by_role(Role::Button, "Next").click().await;
    app.expect(by_label("Second")).to_be_focused().await;
}

/// A file's name without its extension, and what's typed replaces it.
#[mitsuami_test::test]
async fn a_selection_selects_part_of_a_field(app: TestApp) {
    let name = signal("notes.txt".to_owned());
    app.mount(move || {
        let field = node_ref();
        Column::new().children((
            TextInput::new().bind(name).a11y_label("Name").node_ref(field),
            Button::new("Rename").on_click(move || field.select_text(0..5)),
        ))
    });
    app.get_by_role(Role::Button, "Rename").click().await;
    let field = app.get_by_label("Name");
    assert!(field.is_focused());
    assert_eq!(field.text_selection(), Some(0..5));

    field.type_text("todo").await;
    assert_eq!(name.get_untracked(), "todo.txt");
}

/// Characters, not bytes nor UTF-16 units: an emoji is one; a range past
/// the end is cut to it; a text area selects too.
#[mitsuami_test::test]
async fn selections_count_characters(app: TestApp) {
    let (name, notes) = (signal("📁 Photos.zip".to_owned()), signal("first\nsecond".to_owned()));
    app.mount(move || {
        let (field, area) = (node_ref(), node_ref());
        Column::new().children((
            TextInput::new().bind(name).a11y_label("Name").node_ref(field),
            TextArea::new().bind(notes).a11y_label("Notes").node_ref(area),
            Button::new("Photos").on_click(move || field.select_text(2..8)),
            Button::new("Past the end").on_click(move || field.select_text(9..99)),
            Button::new("Second line").on_click(move || area.select_text(6..12)),
        ))
    });
    app.get_by_role(Role::Button, "Photos").click().await;
    assert_eq!(app.get_by_label("Name").text_selection(), Some(2..8));
    app.get_by_role(Role::Button, "Past the end").click().await;
    assert_eq!(app.get_by_label("Name").text_selection(), Some(9..12));

    app.get_by_role(Role::Button, "Second line").click().await;
    let area = app.get_by_label("Notes");
    assert!(area.is_focused());
    assert_eq!(area.text_selection(), Some(6..12));
    area.type_text("2nd").await;
    assert_eq!(notes.get_untracked(), "first\n2nd");
}

/// A dialog asks in `on_open`, before its content is built: its field is
/// focused, with its new text selected, once it's there.
#[mitsuami_test::test]
async fn asked_before_the_control_is_built(app: TestApp) {
    let (open, draft) = (signal(false), signal(String::new()));
    app.mount(move || {
        let field = node_ref();
        Column::new().children((
            Button::new("Rename…").on_click(move || open.set(true)),
            Window::new("Rename")
                .bind(open)
                .on_open(move || {
                    draft.set("report.pdf".into());
                    field.select_text(0..6);
                })
                .content(move || {
                    Column::new().padding(12).children((
                        Text::new("New name:"),
                        TextInput::new().bind(draft).a11y_label("New name").node_ref(field),
                    ))
                }),
        ))
    });
    app.get_by_role(Role::Button, "Rename…").click().await;
    let field = app.get_by_label("New name");
    app.expect(by_label("New name")).to_be_focused().await;
    assert_eq!(field.text_selection(), Some(0..6));
}

mitsuami_test::main!();
