//! Checkbox: `cargo run -p mitsuami --example checkbox`.
//!
//! - Every state, enabled and disabled: what the semantic props look like
//!   on this platform.
//! - A playground: pick the label, and whether it's checked, mixed and
//!   enabled. Click the box to see where it lands from the mixed state.
//! - "Select all": what the mixed state is for.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

/// Unchecked, checked and mixed, then the same disabled.
fn gallery() -> impl View {
    let row = |enabled: bool| {
        Row::new().gap(Spacing::Lg).children((
            Checkbox::new("Unchecked").enabled(enabled),
            Checkbox::new("Checked").checked(true).enabled(enabled),
            Checkbox::new("Mixed").mixed(true).enabled(enabled),
        ))
    };
    Column::new().gap(Spacing::Md).children((heading("States"), row(true), row(false)))
}

/// One checkbox, and its props to change.
fn playground() -> impl View {
    let label = signal("Show hidden files".to_string());
    let checked = signal(false);
    let mixed = signal(true);
    let enabled = signal(true);
    let last = signal(None::<bool>);
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Label"),
                TextInput::new().a11y_label("Label").bind(label),
                Text::new("Checked"),
                Switch::new("Checked").bind(checked),
                Text::new("Mixed"),
                Switch::new("Mixed").bind(mixed),
                Text::new("Enabled"),
                Switch::new("Enabled").bind(enabled),
            )),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            // A click leaves the mixed state wherever the platform lands.
            Checkbox::new(label).checked(checked).mixed(mixed).enabled(enabled).on_change(move |value| {
                checked.set(value);
                mixed.set(false);
                last.set(Some(value));
            }),
            Text::new(move || match last.get() {
                None => "Not clicked yet".to_string(),
                Some(true) => "Last click checked it".to_string(),
                Some(false) => "Last click unchecked it".to_string(),
            })
            .text_style(TextStyle::Caption),
        )),
    ))
}

/// A box that checks the others: mixed while only some are.
fn select_all() -> impl View {
    let names = ["Apples", "Pears", "Plums"];
    let fruit = [signal(true), signal(false), signal(false)];
    let all = move || fruit.iter().all(|f| f.get());
    let some = move || fruit.iter().any(|f| f.get()) && !all();
    Column::new().gap(Spacing::Sm).children((
        heading("Select all"),
        Checkbox::new("All fruit").checked(all).mixed(some).on_change(move |checked| {
            fruit.iter().for_each(|f| f.set(checked));
        }),
        Column::new()
            .padding_x(Spacing::Xl)
            .gap(Spacing::Sm)
            .children(names.iter().zip(fruit).map(|(name, f)| Checkbox::new(*name).bind(f)).collect::<Vec<_>>()),
    ))
}

/// A setting only this platform has, straight on the native checkbox.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Checkbox>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSButton| {
                b.setImagePosition(mitsuami::appkit::objc2_app_kit::NSCellImagePosition::ImageTrailing)
            }),
            "AppKit: imagePosition trailing puts the box after its label.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::CheckButton| {
                use mitsuami::gtk::gtk::prelude::*;
                b.add_css_class("selection-mode")
            }),
            "GTK: the selection-mode style class, which themes draw as a round check.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_real("spacing", 24.0)),
            "Qt Quick: a spacing of 24 puts more room between the box and its label.",
        ),
        windows => (
            mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::CheckBox| {
                use mitsuami::winui::bindings::{CornerRadius, IControl};
                use mitsuami::winui::windows_core::Interface;
                let round = CornerRadius { top_left: 10.0, top_right: 10.0, bottom_right: 10.0, bottom_left: 10.0 };
                b.cast::<IControl>()?.SetCornerRadius(round)
            }),
            "WinUI: a CornerRadius of 10 makes the box round.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        Row::new()
            .gap(Spacing::Lg)
            .align(Align::Center)
            .children((Checkbox::new("Plain").checked(true), Checkbox::new("Tweaked").checked(true).native(tweak))),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Checkbox", WindowSize::FitHeight(640.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((
                gallery(),
                playground(),
                select_all(),
                platform_option(),
            ))
        })
        .run();
}
