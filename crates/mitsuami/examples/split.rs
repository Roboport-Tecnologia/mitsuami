//! Splits: `cargo run -p mitsuami --example split`.
//!
//! A Git client's window: branches beside the history, and the history
//! over a commit's details, each split by the platform's divider.
//!
//! - Drag a divider: the label under the branches follows it.
//! - The branches are at least 160 wide and the history 200: the divider
//!   stops there.
//! - "Reset" puts the sidebar back at 220, which isn't reported as a move.
//! - "Hide Branches" hides the sidebar: its pane collapses with the
//!   divider, and the history fills the window. Shown again, it comes
//!   back at the width it had.
//! - The branches are a sidebar pane (`sidebar_pane`): on Windows, under
//!   tabs in the title bar, it's on the layer WinUI puts navigation on.
//!   Elsewhere it's as it was.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

pub fn page() -> impl View {
    let sidebar = signal(220.0);
    let details = signal(260.0);
    let branches = signal(true);
    Split::new().bind(sidebar).grow(1.0).children((
        Column::new().sidebar_pane(true).hidden(move || !branches.get()).padding(12).gap(6).min_width(160).children((
            Text::new("LOCAL BRANCHES").text_style(TextStyle::Caption).color(Color::SecondaryLabel),
            Text::new("main"),
            Text::new("feature/split"),
            Text::new(move || format!("{:.0} wide", sidebar.get())).color(Color::SecondaryLabel),
            Button::new("Reset").on_click(move || sidebar.set(220.0)),
        )),
        Split::new().orientation(Orientation::Vertical).bind(details).min_width(200).children((
            Column::new().padding(12).gap(6).min_height(80).children((
                Row::new().child(
                    Button::new(move || if branches.get() { "Hide Branches" } else { "Show Branches" }.to_string())
                        .on_click(move || branches.update(|b| *b = !*b)),
                ),
                Text::new("Add splits"),
                Text::new("Colour buttons' and toggle buttons' icons"),
                Text::new("Send every node's first frame"),
            )),
            Column::new().padding(12).min_height(80).child(Text::new("Commit 54b9153").color(Color::SecondaryLabel)),
        )),
    ))
}

fn main() {
    App::new().window("Splits", Size::new(800.0, 520.0), page).run();
}
