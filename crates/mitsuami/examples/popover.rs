//! Popovers: `cargo run -p mitsuami --example popover`.
//!
//! - Contains in: a button whose popover lists branches, under it, as the
//!   platform shows popovers (an `NSPopover` with its arrow on macOS, a
//!   popover on GNOME, a flyout on Windows, a popup on KDE). A click
//!   outside or Escape dismisses it, and the status says so.
//! - A form in one: Rename… opens a field with the branch's name, and its
//!   button renames it. Dismissing the popover (a click outside, Escape)
//!   drops what was typed, as a cancelled edit; the next opening starts
//!   from the name again.
//! - Close and Reopen, in the Rename popover: closes it, then opens it
//!   again a tenth of a second later, while it's still animating out (on
//!   macOS). It should close and come back.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

pub fn page() -> impl View {
    let branches = signal(false);
    let rename = signal(false);
    let name = signal(String::from("feature/popover"));
    // What's typed, applied only by the Rename button.
    let draft = signal(String::new());
    let status = move || match (branches.get(), rename.get()) {
        (true, _) => "Branches open: click outside or press Escape.".to_string(),
        (_, true) => "Rename open: Escape cancels.".to_string(),
        _ => format!("The branch is {}.", name.get()),
    };
    Column::new().padding(Spacing::Xl).gap(Spacing::Lg).align(Align::Start).children((
        Row::new().children((
            Button::new("Contains in").on_click(move || branches.set(true)),
            Popover::new(branches).gap(Spacing::Sm).children((
                Text::new("Branches").text_style(TextStyle::Headline),
                Text::new("main"),
                Text::new("develop"),
                Text::new("release/2026.22"),
            )),
        )),
        Row::new().children((
            Button::new("Rename…").on_click(move || {
                draft.set(name.get_untracked());
                rename.set(true);
            }),
            Popover::new(rename).gap(Spacing::Sm).children((
                TextInput::new().a11y_label("Name").bind(draft).width(240),
                Button::new("Rename").on_click(move || {
                    name.set(draft.get_untracked());
                    rename.set(false);
                }),
                Button::new("Close and Reopen").on_click(move || {
                    rename.set(false);
                    spawn_local(async move {
                        sleep(Duration::from_millis(100)).await;
                        rename.set(true);
                    });
                }),
            )),
        )),
        Text::new(status).test_id("status"),
    ))
}

fn main() {
    App::new().window("Popovers", WindowSize::FitHeight(480.0), page).run();
}
