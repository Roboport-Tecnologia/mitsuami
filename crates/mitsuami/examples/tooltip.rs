//! Tooltips: `cargo run -p mitsuami --example tooltip`.
//!
//! Rest the pointer on anything here: each shows its tooltip as the
//! platform shows them (its delay, placement and look).
//!
//! - A status line cut off at one line, whose tooltip has the whole text.
//! - Tooltips on controls, an image and a whole box.
//! - A tooltip that changes, and one that goes away.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

const STATUS: &str = "Couldn't start the player: /Applications/2ksbox.app/Contents/MacOS/player \
                      exited with status 1 before its window opened.";

fn status() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("A long status"),
        // Cut off at one line; the whole of it is in the tooltip.
        Text::new(STATUS).max_lines(1).tooltip(STATUS).width(320),
    ))
}

fn controls() -> impl View {
    let blue = Pixels::new(2, 2, [[40, 90, 200, 255]; 4].concat());
    Column::new().gap(Spacing::Md).children((
        heading("On controls"),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Button::new("Play").tooltip("Starts the machine"),
            Checkbox::new("Sound").tooltip("Emulates a Sound Blaster 16"),
            NumberInput::new("Memory").range(16, 512).step(16).value(64).tooltip("Memory in MB"),
            Image::pixels(blue.scale(0.1)).label("A blue square").tooltip("An image"),
        )),
        Column::new()
            .padding(Spacing::Md)
            .gap(Spacing::Xs)
            .tooltip("The whole box has a tooltip, where its children have none")
            .children((Text::new("A box"), Text::new("with two lines"))),
    ))
}

fn changing() -> impl View {
    let running = signal(false);
    let shown = signal(true);
    Column::new().gap(Spacing::Md).children((
        heading("Changing"),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Button::new(move || if running.get() { "Stop" } else { "Play" }.to_owned())
                .tooltip(move || if running.get() { "Stops the machine" } else { "Starts the machine" }.to_owned())
                .on_click(move || running.update(|r| *r = !*r)),
            Switch::new("Tooltip").bind(shown),
            Text::new("Hover me").tooltip(move || if shown.get() { "Here I am".into() } else { String::new() }),
        )),
    ))
}

fn main() {
    App::new()
        .window("Tooltips", WindowSize::FitHeight(480.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((status(), controls(), changing()))
        })
        .run();
}
