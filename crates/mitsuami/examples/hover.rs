//! Hover: `cargo run -p mitsuami --example hover`.
//!
//! Move the pointer over things here: each says when the platform reports
//! it over them.
//!
//! - A box that stays hovered while the pointer is on the button inside.
//! - A list whose rows show their Start button while hovered, as 2ksbox's
//!   machines do; their context menu has Start too, for the keyboard.
//! - A label that turns into a field when double-clicked, as a name is
//!   renamed in place (the button beside it keeps its own clicks).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn over(hovered: Signal<bool>) -> impl Fn() -> String {
    move || if hovered.get() { "Hovered" } else { "Not hovered" }.to_owned()
}

fn a_box() -> impl View {
    let hovered = signal(false);
    view! {
        <Column gap=Spacing::Md>
            {heading("A box and a button inside")}
            <Row gap=Spacing::Md padding=Spacing::Md align=Align::Center @hover=move |o| hovered.set(o)>
                <Text>{over(hovered)}</Text>
                <Button>"Inside"</Button>
            </Row>
        </Column>
    }
}

#[derive(Clone, PartialEq)]
struct Machine {
    id: u32,
    name: &'static str,
}

fn rows() -> impl View {
    let machines = signal(vec![
        Machine { id: 1, name: "Windows 98" },
        Machine { id: 2, name: "Windows XP" },
        Machine { id: 3, name: "Windows 2000" },
    ]);
    let hovered = signal(None::<u32>);
    let started = signal(String::new());
    let row = move |m: Machine| {
        let (id, name) = (m.id, m.name);
        Row::new()
            .gap(Spacing::Md)
            .padding_x(Spacing::Sm)
            .height(32)
            .align(Align::Center)
            .on_hover(move |o| hovered.update(|h| *h = if o { Some(id) } else { h.filter(|h| *h != id) }))
            .context_menu(MenuItem::new("Start").on_select(move || started.set(name.to_owned())))
            .children((
                Text::new(name).grow(1.0),
                Button::new("Start")
                    .hidden(move || hovered.get() != Some(id))
                    .on_click(move || started.set(name.to_owned())),
            ))
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Rows")}
            {List::new(machines, |m: &Machine| m.id, row).height(120).width(320)}
            <Text>{move || match started.get().as_str() {
                "" => "Nothing started".to_owned(),
                name => format!("Started {name}"),
            }}</Text>
        </Column>
    }
}

fn rename() -> impl View {
    let name = signal("Windows 98".to_owned());
    let editing = signal(false);
    view! {
        <Column gap=Spacing::Md>
            {heading("Double-click to rename")}
            <Row gap=Spacing::Md align=Align::Center @double_click=move || editing.set(true)>
                <Show when=editing fallback=move || view! { <Text>{name}</Text> }>
                    <TextInput
                        a11y_label="Name"
                        value=name
                        @input=move |text| name.set(text)
                        @submit=move || editing.set(false)
                    />
                </Show>
                <Button>"Inside"</Button>
            </Row>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {a_box()}
            {rows()}
            {rename()}
        </Column>
    }
}

fn main() {
    App::new().window("Hover", WindowSize::FitHeight(480.0), page).run();
}
