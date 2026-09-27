//! NumberInput: `cargo run -p mitsuami --example number_input`.
//!
//! - Spin boxes (small and large ranges, a step, disabled), as each
//!   platform draws them.
//! - A playground: set the range and step, and whether it's enabled.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

fn gallery() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("Spin boxes"),
        Grid::new()
            .columns([Track::MaxContent, Track::MaxContent])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Copies (1 to 99)"),
                NumberInput::new("Copies").range(1, 99).value(2),
                // GTK sizes a spin box for its range's widest number.
                Text::new("Memory, MB (16 to 4096, by 16)"),
                NumberInput::new("Memory").range(16, 4096).step(16).value(512),
                Text::new("Disabled"),
                NumberInput::new("Disabled").value(7).enabled(false),
            )),
    ))
}

/// One spin box, and its props to change.
fn playground() -> impl View {
    let max = signal(100);
    let step = signal(1);
    let enabled = signal(true);
    let value = signal(10);
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::MaxContent])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Maximum"),
                NumberInput::new("Maximum").range(1, 100_000).bind(max),
                Text::new("Step"),
                NumberInput::new("Step").range(1, 1000).bind(step),
                Text::new("Enabled"),
                Switch::new("Enabled").bind(enabled),
                Text::new("Value"),
                NumberInput::new("Value").range_with(move || (0, max.get())).step(step).enabled(enabled).bind(value),
            )),
        Text::new(move || format!("The app has {}", value.get())),
    ))
}

/// A setting only this platform has, straight on the native spin box.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<NumberInput>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|n: &mitsuami::appkit::NumberField| n.stepper().setValueWraps(false)),
            "AppKit: a stepper wraps round at its ends; valueWraps off stops it there instead.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::SpinButton| s.set_wrap(true)),
            "GTK: wrap makes the buttons go round from one end to the other.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_bool("wrap", true)),
            "Qt Quick: wrap makes the buttons go round from one end to the other.",
        ),
        windows => (
            mitsuami::winui::tweak(|n: &mitsuami::winui::bindings::NumberBox| {
                n.SetSpinButtonPlacementMode(mitsuami::winui::bindings::NumberBoxSpinButtonPlacementMode::Compact)
            }),
            "WinUI: compact spin buttons show in a pop-up while the box has focus.",
        ),
    };
    let value = signal(5);
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            NumberInput::new("Tweaked").range(1, 10).bind(value).native(tweak),
            Text::new(move || format!("{}", value.get())),
        )),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("NumberInput", WindowSize::FitHeight(480.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
