//! Text input: `cargo run -p mitsuami --example text_input`.
//!
//! - A few fields: empty with a placeholder, with text, read-only,
//!   disabled, and one with more text than fits.
//! - A playground: type, change the placeholder, make it read-only or turn
//!   it off, press Return to submit.
//! - A raw platform setting, through `.native()`. Read-only is the one
//!   option every platform's text field has: the rest is each one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

fn gallery() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("Fields"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Empty"),
                TextInput::new().a11y_label("Empty").placeholder("A placeholder"),
                Text::new("With text"),
                TextInput::new().a11y_label("With text").value("Ada Lovelace"),
                // Selectable and copyable, and read as usual.
                Text::new("Read-only"),
                TextInput::new().a11y_label("Read-only").value("ABCD-1234-EFGH").read_only(true),
                Text::new("Disabled"),
                TextInput::new().a11y_label("Disabled").value("Can't edit this").enabled(false),
                // Scrolls as the platform scrolls it.
                Text::new("Long"),
                TextInput::new()
                    .a11y_label("Long")
                    .value("A line of text much longer than the field is wide, to scroll through"),
            )),
    ))
}

/// One field, and its props to change.
fn playground() -> impl View {
    let text = signal(String::new());
    let placeholder = signal("Your name".to_string());
    let read_only = signal(false);
    let enabled = signal(true);
    let submitted = signal(None::<String>);
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Placeholder"),
                TextInput::new().a11y_label("Placeholder").bind(placeholder),
                Text::new("Read-only"),
                Switch::new("Read-only").bind(read_only),
                Text::new("Enabled"),
                Switch::new("Enabled").bind(enabled),
                Text::new("Name"),
                TextInput::new()
                    .a11y_label("Name")
                    .placeholder(placeholder)
                    .bind(text)
                    .read_only(read_only)
                    .enabled(enabled)
                    .on_submit(move || submitted.set(Some(text.get_untracked()))),
            )),
        Text::new(move || {
            let typed = format!("{} characters", text.get().chars().count());
            match submitted.get() {
                Some(name) => format!("{typed}; submitted \"{name}\""),
                None => format!("{typed}; press Return to submit"),
            }
        })
        .text_style(TextStyle::Caption),
    ))
}

/// A setting only this platform has, straight on the native field.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<TextInput>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|f: &mitsuami::appkit::objc2_app_kit::NSTextField| f.setBezeled(false)),
            "AppKit: bezeled off draws the field without its border, as fields edited in place are.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|e: &mitsuami::gtk::gtk::Entry| {
                use mitsuami::gtk::gtk::prelude::*;
                e.set_icon_from_icon_name(mitsuami::gtk::gtk::EntryIconPosition::Primary, Some("system-search-symbolic"))
            }),
            "GTK: an entry can show icons at either end; this one a search icon.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|f: &mitsuami::kirigami::QmlObject| f.set_int("maximumLength", 8)),
            "Qt Quick: maximumLength stops the field at 8 characters: type to see.",
        ),
        windows => (
            mitsuami::winui::tweak(|f: &mitsuami::winui::bindings::TextBox| {
                use mitsuami::winui::bindings::{ITextBox, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                f.cast::<ITextBox>()?.SetHeader(&PropertyValue::CreateString("Name")?)
            }),
            "WinUI: Header draws a caption above the text box, WinUI's own way to label it.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        TextInput::new().a11y_label("Plain").placeholder("Plain"),
        TextInput::new().a11y_label("Tweaked").placeholder("Tweaked").native(tweak),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Text input", WindowSize::FitHeight(560.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
