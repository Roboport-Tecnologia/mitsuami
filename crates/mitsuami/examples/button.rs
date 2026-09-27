//! Button: `cargo run -p mitsuami --example button`.
//!
//! - Every role in every style, and disabled: what the semantic props look
//!   like on this platform.
//! - A playground: pick the role, style, label and whether it's enabled.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::core::AnyView;
use mitsuami::prelude::*;

const ROLES: [(&str, ButtonRole); 4] = [
    ("Normal", ButtonRole::Normal),
    ("Default", ButtonRole::Default),
    ("Cancel", ButtonRole::Cancel),
    ("Destructive", ButtonRole::Destructive),
];

const STYLES: [(&str, ButtonStyle); 3] = [
    ("Automatic", ButtonStyle::Automatic),
    ("Bordered", ButtonStyle::Bordered),
    ("Borderless", ButtonStyle::Borderless),
];

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

/// A row per role, a column per style, and the same disabled.
fn gallery() -> impl View {
    let mut cells: Vec<AnyView> = ["", "Bordered", "Borderless", "Disabled"].map(|h| AnyView::new(Text::new(h))).into();
    for (name, role) in ROLES {
        cells.extend([
            AnyView::new(Text::new(name)),
            AnyView::new(Row::new().child(Button::new(name).role(role))),
            AnyView::new(Row::new().child(Button::new(name).role(role).button_style(ButtonStyle::Borderless))),
            AnyView::new(Row::new().child(Button::new(name).role(role).enabled(false))),
        ]);
    }
    Column::new().gap(Spacing::Md).children((
        heading("Roles and styles"),
        Grid::new()
            .columns([Track::MaxContent, Track::MaxContent, Track::MaxContent, Track::MaxContent])
            .column_gap(Spacing::Lg)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children(cells),
    ))
}

/// One button, and its props to change.
fn playground() -> impl View {
    let role = signal(1);
    let style = signal(0);
    let label = signal("Save".to_string());
    let enabled = signal(true);
    let clicks = signal(0);
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Role"),
                Row::new().child(Select::new("Role").options(ROLES.map(|(name, _)| name)).bind(role)),
                Text::new("Style"),
                Row::new().child(Select::new("Style").options(STYLES.map(|(name, _)| name)).bind(style)),
                Text::new("Label"),
                TextInput::new().a11y_label("Label").bind(label),
                Text::new("Enabled"),
                Switch::new("Enabled").bind(enabled),
            )),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Button::new(label)
                .role(move || ROLES[role.get()].1)
                .button_style(move || STYLES[style.get()].1)
                .enabled(enabled)
                .on_click(move || clicks.update(|c| *c += 1)),
            Text::new(move || match clicks.get() {
                0 => "Not clicked yet".to_string(),
                1 => "Clicked once".to_string(),
                n => format!("Clicked {n} times"),
            }),
        )),
    ))
}

/// A setting only this platform has, straight on the native button.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Button>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSButton| {
                b.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Large)
            }),
            "AppKit: controlSize large makes a larger button, as macOS uses in sheets and onboarding.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::Button| {
                use mitsuami::gtk::gtk::prelude::*;
                b.add_css_class("circular")
            }),
            "GTK: the circular style class rounds the button's ends.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_bool("checkable", true)),
            "Qt Quick: checkable makes the button stay down when clicked, as a toggle.",
        ),
        windows => (
            mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::Button| {
                use mitsuami::winui::bindings::{CornerRadius, IControl};
                use mitsuami::winui::windows_core::Interface;
                let round = CornerRadius { top_left: 16.0, top_right: 16.0, bottom_right: 16.0, bottom_left: 16.0 };
                b.cast::<IControl>()?.SetCornerRadius(round)
            }),
            "WinUI: a CornerRadius of 16 rounds the button's ends.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Button::new("Default").role(ButtonRole::Default),
            Button::new("Tweaked").role(ButtonRole::Default).native(tweak),
        )),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Button", WindowSize::FitHeight(560.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
