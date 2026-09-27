//! Select: `cargo run -p mitsuami --example select`.
//!
//! - A few selects: the first option chosen, another chosen, disabled, and
//!   one with a long option (AppKit sizes pop-ups for their widest option,
//!   the others for the chosen one).
//! - A playground: edit the options, choose one, turn it off.
//! - A raw platform setting, through `.native()`. Selects have no semantic
//!   options past their options and choice: what platforms offer is each
//!   one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

/// A select beside its caption: selects draw none, so the label is only
/// the accessible name.
fn labelled(caption: &'static str, select: Select) -> (Text, impl View) {
    (Text::new(caption), Row::new().child(select))
}

fn gallery() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("Selects"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                labelled("First option", Select::new("First option").options(SIZES)),
                labelled("Chosen", Select::new("Chosen").options(SIZES).selected(2)),
                labelled("Disabled", Select::new("Disabled").options(SIZES).selected(1).enabled(false)),
                labelled(
                    "A long option",
                    Select::new("A long option").options(["Short", "A much, much longer option"]),
                ),
            )),
    ))
}

/// One select, and its props to change.
fn playground() -> impl View {
    let text = signal("Red, Green, Blue".to_string());
    let options =
        move || text.get().split(',').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect::<Vec<_>>();
    let chosen = signal(0);
    let enabled = signal(true);
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Options"),
                TextInput::new().a11y_label("Options").placeholder("Comma-separated").bind(text),
                Text::new("Enabled"),
                Switch::new("Enabled").bind(enabled),
                Text::new("Color"),
                Row::new().child(Select::new("Color").options(options).bind(chosen).enabled(enabled)),
            )),
        Text::new(move || match options().get(chosen.get()) {
            Some(option) => format!("Chosen: {option} (option {})", chosen.get() + 1),
            None => "No options".to_string(),
        })
        .text_style(TextStyle::Caption),
    ))
}

/// A setting only this platform has, straight on the native select.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Select>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|p: &mitsuami::appkit::objc2_app_kit::NSPopUpButton| p.setBordered(false)),
            "AppKit: bordered off makes a borderless pop-up, as in toolbars and \"Sort by\" menus.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|d: &mitsuami::gtk::gtk::DropDown| d.set_enable_search(true)),
            "GTK: enable-search puts a search entry in the pop-up: open it to see.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|c: &mitsuami::kirigami::QmlObject| c.set_bool("flat", true)),
            "Qt Quick: flat draws the combo box without a frame until hovered.",
        ),
        windows => (
            mitsuami::winui::tweak(|c: &mitsuami::winui::bindings::ComboBox| {
                use mitsuami::winui::bindings::{IComboBox, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                c.cast::<IComboBox>()?.SetHeader(&PropertyValue::CreateString("Size")?)
            }),
            "WinUI: Header draws a caption above the combo box, WinUI's own way to label it.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        Row::new().gap(Spacing::Lg).align(Align::Center).children((
            Select::new("Plain").options(SIZES).selected(1),
            Select::new("Tweaked").options(SIZES).selected(1).native(tweak),
        )),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Select", WindowSize::FitHeight(560.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
