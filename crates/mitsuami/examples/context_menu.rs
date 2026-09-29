//! Context menus: `cargo run -p mitsuami --example context_menu`.
//!
//! Right-click (Control-click on a Mac, press and hold on a touch screen,
//! the menu key or Shift+F10 on a focused control) to show each menu as
//! the platform shows them.
//!
//! - A machine list whose rows each have a menu: a title that changes, an
//!   item disabled while the machine runs, a submenu of radio items, a
//!   check item, separators.
//! - A box with a menu, over its text but not over the button in it,
//!   which has its own.
//! - A text field with none of ours: the platform's Cut, Copy and Paste.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, PartialEq)]
struct Machine {
    id: u32,
    name: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Name,
    Newest,
}

fn machines() -> impl View {
    let machines = signal(vec![
        Machine { id: 1, name: "Windows 98".into() },
        Machine { id: 2, name: "Windows 2000".into() },
        Machine { id: 3, name: "Windows XP".into() },
    ]);
    let running = signal(None::<u32>);
    let sort = signal(Sort::Name);
    let details = signal(false);
    let sorted = move || {
        let mut all = machines.get();
        match sort.get() {
            Sort::Name => all.sort_by(|a, b| a.name.cmp(&b.name)),
            Sort::Newest => all.sort_by_key(|m| std::cmp::Reverse(m.id)),
        }
        all
    };
    let row = move |machine: Machine| {
        let id = machine.id;
        let is_running = move || running.get() == Some(id);
        let name = machine.name.clone();
        Row::new()
            .padding_x(8)
            .height(28)
            .align(Align::Center)
            .gap(8)
            .children((
                Text::new(machine.name.clone()).grow(1.0),
                Show::new(move || details.get(), move || Text::new(format!("#{id}")).text_style(TextStyle::Caption)),
                Show::new(is_running, || Text::new("Running").text_style(TextStyle::Caption)),
            ))
            .context_menu((
                MenuItem::new(move || if is_running() { "Stop" } else { "Start" }.to_owned())
                    .on_select(move || running.set(if is_running() { None } else { Some(id) })),
                MenuItem::new("Duplicate").on_select(move || {
                    machines.update(|all| {
                        let next = all.iter().map(|m| m.id).max().unwrap_or(0) + 1;
                        all.push(Machine { id: next, name: format!("{name} copy") });
                    })
                }),
                MenuSeparator,
                Menu::new("Sort By")
                    .item(MenuItem::new("Name").radio((sort, Sort::Name)))
                    .item(MenuItem::new("Newest").radio((sort, Sort::Newest))),
                MenuItem::new("Show Details").bind(details),
                MenuSeparator,
                MenuItem::new("Delete")
                    .enabled(move || !is_running())
                    .on_select(move || machines.update(|all| all.retain(|m| m.id != id))),
            ))
    };
    Column::new().gap(Spacing::Sm).children((
        Text::new("Machines").text_style(TextStyle::Headline),
        List::new(computed(sorted), |m: &Machine| m.id, row).height(160),
    ))
}

fn boxes() -> impl View {
    let said = signal("Nothing chosen yet".to_owned());
    view! {
        <Column gap=Spacing::Sm>
            <Text text_style=TextStyle::Headline>"A box and a button"</Text>
            <Row
                gap=Spacing::Md
                padding=Spacing::Md
                align=Align::Center
                context_menu=(MenuItem::new("The Box's Item").on_select(move || said.set("The box's item".into())),)
            >
                <Text grow=1.0>{said}</Text>
                <Button context_menu=(MenuItem::new("The Button's Item").on_select(move || said.set("The button's item".into())),)>
                    "Button"
                </Button>
            </Row>
            <TextInput placeholder="The platform's own menu"/>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {machines()}
            {boxes()}
        </Column>
    }
}

fn main() {
    App::new().window("Context Menus", WindowSize::FitHeight(420.0), page).run();
}
