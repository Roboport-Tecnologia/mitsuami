//! Slider: `cargo run -p mitsuami --example slider`.
//!
//! - Horizontal sliders (without a step, with one, disabled) and vertical
//!   ones, as each platform draws them.
//! - A playground: pick the orientation and whether it's enabled, and move
//!   it.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

fn gallery() -> impl View {
    let vertical = |name: &str, value: f64| Slider::new(name).orientation(Orientation::Vertical).value(value);
    Column::new().gap(Spacing::Md).children((
        heading("Horizontal"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("No step"),
                Slider::new("No step").value(30.0),
                Text::new("A step of 20"),
                // What a step does is the platform's: tick marks the knob
                // stops at on AppKit, snapping on WinUI, keyboard moves on
                // GTK and Qt.
                Slider::new("A step of 20").step(20.0).value(40.0),
                Text::new("Disabled"),
                Slider::new("Disabled").value(70.0).enabled(false),
            )),
        heading("Vertical"),
        Row::new().gap(Spacing::Xl).height(140).children((
            vertical("Bass", 20.0),
            vertical("Mid", 50.0),
            vertical("Treble", 80.0),
        )),
    ))
}

/// One slider, and its props to change.
fn playground() -> impl View {
    let vertical = signal(0);
    let enabled = signal(true);
    let value = signal(50.0);
    let is_vertical = move || vertical.get() == 1;
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Orientation"),
                Row::new().child(Select::new("Orientation").options(["Horizontal", "Vertical"]).bind(vertical)),
                Text::new("Enabled"),
                Switch::new("Enabled").bind(enabled),
            )),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            // Tall when vertical, as wide as the row when horizontal:
            // sliders take their length from the layout.
            Slider::new("Value")
                .orientation(move || if is_vertical() { Orientation::Vertical } else { Orientation::Horizontal })
                .enabled(enabled)
                .bind(value)
                .height(move || if is_vertical() { Length::Px(140.0) } else { Length::Auto })
                .grow(move || if is_vertical() { 0.0 } else { 1.0 }),
            Text::new(move || format!("{:.0}", value.get())),
        )),
    ))
}

/// A setting only this platform has, straight on the native slider.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Slider>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSSlider| {
                s.setSliderType(mitsuami::appkit::objc2_app_kit::NSSliderType::Circular)
            }),
            "AppKit: sliderType circular makes a dial, turned around its centre.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Scale| {
            use mitsuami::gtk::gtk::prelude::*;
            s.set_draw_value(true)
        }),
            "GTK: draw-value shows the value beside the scale.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_int("snapMode", 1)),
            "Qt Quick: snapMode SnapAlways makes the handle jump from step to step while dragged.",
        ),
        windows => (
            mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::Slider| {
                use mitsuami::winui::bindings::{ISlider, TickPlacement};
                use mitsuami::winui::windows_core::Interface;
                let s = s.cast::<ISlider>()?;
                s.SetTickFrequency(1.0)?;
                s.SetTickPlacement(TickPlacement::Outside)
            }),
            "WinUI: TickFrequency and TickPlacement draw tick marks at every step.",
        ),
    };
    let value = signal(4.0);
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Slider::new("Tweaked").range(0.0, 10.0).step(1.0).bind(value).native(tweak),
            Text::new(move || format!("{:.0}", value.get())),
        )),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Slider", WindowSize::FitHeight(640.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
