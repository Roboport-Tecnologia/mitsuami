//! Separator: `cargo run -p mitsuami --example separator`.
//!
//! - Lines between groups of settings, across the column.
//! - Vertical lines between groups of buttons in a row.
//! - A playground: turn a bar of buttons from a row to a column, and its
//!   separators with it.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform that has one.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Between groups")}
            <Checkbox>"Start with sound"</Checkbox>
            <Checkbox>"Pause in the background"</Checkbox>
            <Separator/>
            <Checkbox>"Check for updates"</Checkbox>
            <Separator/>
            <Row gap=Spacing::Sm>
                <Button>"Cut"</Button>
                <Button>"Copy"</Button>
                <Button>"Paste"</Button>
                <Separator orientation=Orientation::Vertical/>
                <Button>"Undo"</Button>
                <Button>"Redo"</Button>
            </Row>
        </Column>
    }
}

/// A bar of buttons that turns, and its separators with it.
fn playground() -> impl View {
    let vertical = signal(false);
    let orientation = move || if vertical.get() { Orientation::Horizontal } else { Orientation::Vertical };
    let direction = move || if vertical.get() { FlexDirection::Column } else { FlexDirection::Row };
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Switch bind=vertical>"Buttons in a column"</Switch>
            <Container flex_direction=direction gap=Spacing::Sm align_self=Align::Start>
                <Button>"Back"</Button>
                <Button>"Forward"</Button>
                <Separator orientation=orientation/>
                <Button>"Reload"</Button>
            </Container>
        </Column>
    }
}

/// A setting only this platform has, straight on the native separator.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Separator>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSBox| b.setTransparent(true)),
            "AppKit: transparent keeps the separator's room but draws nothing.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Separator| {
                use mitsuami::gtk::gtk::prelude::*;
                s.add_css_class("spacer")
            }),
            "GTK: the spacer class keeps the separator's room but draws nothing, as libadwaita spaces groups.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_int("weight", 0)),
            "Kirigami: the light weight, as inside lists and cards.",
        ),
        _ => (Tweak::none(), "WinUI: none; Fluent's divider is a plain line."),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Text>"Plain"</Text>
            <Separator/>
            <Text>"Tweaked"</Text>
            <Separator native=tweak/>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {gallery()}
            {playground()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Separator", WindowSize::FitHeight(560.0), page).run();
}
