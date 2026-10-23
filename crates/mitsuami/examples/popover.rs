//! Popovers: `cargo run -p mitsuami --example popover`.
//!
//! - Contains in: a button whose popover lists branches, under it, as the
//!   platform shows popovers (an `NSPopover` with its arrow on macOS, a
//!   popover on GNOME, a flyout on Windows, a popup on KDE). A click
//!   outside or Escape dismisses it, and the status says so.
//! - A form in one: a field and a button that closes it, which says what
//!   was typed.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

pub fn page() -> impl View {
    let branches = signal(false);
    let rename = signal(false);
    let name = signal(String::from("feature/popover"));
    let last = signal(String::from("Nothing open."));
    let status = move || match (branches.get(), rename.get()) {
        (true, _) => "Branches open: click outside or press Escape.".to_string(),
        (_, true) => "Rename open.".to_string(),
        _ => last.get(),
    };
    Column::new().padding(Spacing::Xl).gap(Spacing::Lg).align(Align::Start).children((
        Row::new().children((
            Button::new("Contains in").on_click(move || branches.set(true)),
            Popover::new(branches).padding(Spacing::Md).gap(Spacing::Sm).children((
                Text::new("Branches").text_style(TextStyle::Headline),
                Text::new("main"),
                Text::new("develop"),
                Text::new("release/2026.22"),
            )),
        )),
        Row::new().children((
            Button::new("Rename…").on_click(move || rename.set(true)),
            Popover::new(rename).padding(Spacing::Md).gap(Spacing::Sm).children((
                TextInput::new().a11y_label("Name").value(name).width(240),
                Button::new("Rename").on_click(move || {
                    rename.set(false);
                    last.set(format!("Renamed to {}.", name.get_untracked()));
                }),
            )),
        )),
        Text::new(status).test_id("status"),
    ))
}

fn main() {
    App::new().window("Popovers", WindowSize::FitHeight(480.0), page).run();
}
