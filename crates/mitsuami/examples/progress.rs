//! Progress: `cargo run -p mitsuami --example progress`.
//!
//! - Bars at a few values, and one for work of unknown length.
//! - A playground: set the value, or make it indeterminate, or run a
//!   pretend download that connects first.
//! - A raw platform setting, through `.native()`. Progress bars have no
//!   semantic options past the value: what platforms offer is each one's
//!   own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

fn gallery() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("Values"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Md)
            .align(Align::Center)
            .children((
                Text::new("Not started"),
                Progress::new("Not started").value(0.0),
                Text::new("A third"),
                Progress::new("A third").value(1.0 / 3.0),
                Text::new("Done"),
                Progress::new("Done").value(1.0),
                // Animated as the platform animates it.
                Text::new("Unknown"),
                Progress::new("Unknown"),
            )),
    ))
}

/// One bar, its props to change, and a task to drive it.
fn playground() -> impl View {
    let value = signal(40.0_f64);
    let indeterminate = signal(false);
    let running = signal(false);
    let download = move || {
        running.set(true);
        indeterminate.set(true);
        value.set(0.0);
        spawn_local(async move {
            sleep(Duration::from_secs(1)).await;
            indeterminate.set(false);
            for step in 1..=20 {
                sleep(Duration::from_millis(100)).await;
                value.set(step as f64 * 5.0);
            }
            running.set(false);
        });
    };
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new(move || format!("Value ({:.0}%)", value.get())),
                Slider::new("Value").range(0.0, 100.0).bind(value).enabled(move || !running.get()),
                Text::new("Indeterminate"),
                Switch::new("Indeterminate").bind(indeterminate).enabled(move || !running.get()),
            )),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Progress::new("Download").value(move || value.get() / 100.0).indeterminate(indeterminate).grow(1.0),
            Button::new("Download").enabled(move || !running.get()).on_click(download),
        )),
    ))
}

/// A setting only this platform has, straight on the native bar.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Progress>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|p: &mitsuami::appkit::objc2_app_kit::NSProgressIndicator| {
                p.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Small)
            }),
            "AppKit: controlSize small draws a thinner bar, as in a status bar or a list row.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|p: &mitsuami::gtk::gtk::ProgressBar| p.set_show_text(true)),
            "GTK: show-text draws the percentage with the bar.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|p: &mitsuami::kirigami::QmlObject| {
                if let Some(palette) = p.object("palette") {
                    palette.set_str("highlight", "#8e44ad");
                }
            }),
            "Qt Quick: palette.highlight colours the filled part, in styles that draw it with the palette.",
        ),
        windows => (
            mitsuami::winui::tweak(|p: &mitsuami::winui::bindings::ProgressBar| {
                use mitsuami::winui::windows_core::Interface;
                p.cast::<mitsuami::winui::bindings::IProgressBar>()?.SetShowPaused(true)
            }),
            "WinUI: ShowPaused draws the bar in its paused state.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        Progress::new("Plain").value(0.6),
        Progress::new("Tweaked").value(0.6).native(tweak),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Progress", WindowSize::FitHeight(560.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
