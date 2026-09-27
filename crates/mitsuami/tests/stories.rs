//! The built-in widgets as stories: captured on each platform at each size,
//! in light and dark, and compared with their baselines. Heights fit the
//! content, since control heights differ per platform.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[mitsuami_test::story(sizes = [(340, fit)])]
fn buttons() -> impl View {
    Row::new().padding(16).gap(8).align(Align::Start).children((
        Button::new("Default"),
        Button::new("Primary").variant(ButtonVariant::Primary),
        Button::new("Disabled").enabled(false),
    ))
}

#[mitsuami_test::story(sizes = [(240, fit)])]
fn text_styles() -> impl View {
    Column::new().padding(16).gap(4).children((
        Text::new("Large title").text_style(TextStyle::LargeTitle),
        Text::new("Title").text_style(TextStyle::Title),
        Text::new("Headline").text_style(TextStyle::Headline),
        Text::new("Body"),
        Text::new("Callout").text_style(TextStyle::Callout),
        Text::new("Caption").text_style(TextStyle::Caption),
        Text::new("Monospace").text_style(TextStyle::Monospace),
    ))
}

#[mitsuami_test::story(sizes = [(200, fit)])]
fn toggles() -> impl View {
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Checkbox::new("Unchecked"),
        Checkbox::new("Checked").checked(true),
        Checkbox::new("Disabled").enabled(false),
        Switch::new("Off"),
        Switch::new("On").checked(true),
    ))
}

/// Sized for their widest option, whichever is chosen.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn selects() -> impl View {
    let sizes = ["Small", "Medium", "Extra large"];
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Select::new("Size").options(sizes),
        Select::new("Size, chosen").options(sizes).selected(2),
        Select::new("Size, disabled").options(sizes).selected(1).enabled(false),
    ))
}

fn signup() -> impl View {
    let agreed = signal(false);
    Column::new().padding(16).gap(8).children((
        TextInput::new().a11y_label("Name").placeholder("Your name"),
        TextInput::new().a11y_label("Email").value("ada@example.com"),
        Checkbox::new("I agree to the terms").bind(agreed),
        Row::new().justify(Justify::End).child(Button::new("Sign up").variant(ButtonVariant::Primary).enabled(agreed)),
    ))
}

/// The form at a phone's width and a window's: inputs stretch, the button
/// stays at the end.
#[mitsuami_test::story(sizes = [(280, fit), (480, fit)], play = agree)]
fn signup_ready() -> impl View {
    signup()
}

async fn agree(app: &TestApp) {
    app.get_by_role(Role::Checkbox, "I agree to the terms").click().await;
    app.expect(by_role(Role::Button, "Sign up")).to_be_enabled().await;
}

#[derive(Clone)]
struct Contact {
    id: u32,
    name: &'static str,
    email: &'static str,
}

const CONTACTS: [Contact; 8] = [
    Contact { id: 1, name: "Ada Lovelace", email: "ada@example.com" },
    Contact { id: 2, name: "Alan Turing", email: "alan@example.com" },
    Contact { id: 3, name: "Grace Hopper", email: "grace@example.com" },
    Contact { id: 4, name: "Edsger Dijkstra", email: "edsger@example.com" },
    Contact { id: 5, name: "Barbara Liskov", email: "barbara@example.com" },
    Contact { id: 6, name: "Donald Knuth", email: "don@example.com" },
    Contact { id: 7, name: "Frances Allen", email: "fran@example.com" },
    Contact { id: 8, name: "John Backus", email: "john@example.com" },
];

/// A list with a selected row, cut off at the bottom: the platform draws the
/// rows' selection and the scroll bar.
#[mitsuami_test::story(sizes = [(280, fit)], play = select_grace)]
fn list() -> impl View {
    let selected = signal(Vec::<u32>::new());
    Column::new().child(
        List::new(
            || CONTACTS.to_vec(),
            |c: &Contact| c.id,
            |c| {
                Column::new()
                    .padding_x(12)
                    .padding_y(6)
                    .children((Text::new(c.name), Text::new(c.email).text_style(TextStyle::Caption)))
            },
        )
        .selected(selected)
        .height(180),
    )
}

async fn select_grace(app: &TestApp) {
    app.get_by_role(Role::ListItem, "Grace Hopper grace@example.com").select().await;
}

mitsuami_test::main!();
