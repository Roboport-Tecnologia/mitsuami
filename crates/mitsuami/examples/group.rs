//! Groups: `cargo run -p mitsuami --example group`.
//!
//! A machine's settings in groups, as each platform groups settings: a
//! box with its heading inside on macOS and KDE, a card under a heading
//! on GNOME and Windows.
//!
//! - "Show headings" takes the headings away: the boxes stay, and their
//!   content moves up into the room the heading had.
//! - The CD drive's group has padding of the app's, on top of the
//!   platform's margins.
//! - A raw platform setting, through `.native()`, on the last group.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

pub fn settings() -> impl View {
    let headings = signal(true);
    let heading = move |title: &'static str| move || if headings.get() { title.to_string() } else { String::new() };
    let memory = signal(64);
    let (tweak, about): (Tweak<Group>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSBox| {
                b.setTitlePosition(mitsuami::appkit::objc2_app_kit::NSTitlePosition::AtBottom)
            }),
            "AppKit: titlePosition puts the heading at the bottom of the box.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|card: &mitsuami::gtk::gtk::Box| {
                use mitsuami::gtk::gtk::prelude::*;
                card.add_css_class("activatable")
            }),
            "GTK: the activatable class highlights the card under the pointer.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_bool("flat", true)),
            "Qt Quick: flat draws the group box without its frame.",
        ),
        windows => (
            mitsuami::winui::tweak(|card: &mitsuami::winui::bindings::Border| {
                use mitsuami::winui::bindings::IBorder;
                use mitsuami::winui::windows_core::Interface;
                card.cast::<IBorder>()?.SetCornerRadius(mitsuami::winui::bindings::CornerRadius {
                    top_left: 0.0,
                    top_right: 0.0,
                    bottom_right: 0.0,
                    bottom_left: 0.0,
                })
            }),
            "WinUI: CornerRadius squares the card's corners.",
        ),
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Lg>
            <Switch bind=headings>"Show headings"</Switch>
            <Group title=heading("CD drive") padding=Spacing::Sm>
                <Row gap=Spacing::Md align=Align::Center>
                    <Column grow=1.0>
                        <Text text_style=TextStyle::Headline>"Total Annihilation (1997)"</Text>
                        <Text color=Color::SecondaryLabel>"ISO image"</Text>
                    </Column>
                    <Button>"Eject"</Button>
                </Row>
            </Group>
            <Group title=heading("Machine") gap=Spacing::Sm>
                <Row gap=Spacing::Md align=Align::Center>
                    <Text grow=1.0>"Memory (MB)"</Text>
                    <NumberInput label="Memory" range_with=(16, 512) step=16 bind=memory/>
                </Row>
                <Checkbox>"Start with sound"</Checkbox>
                <Checkbox>"Pause in the background"</Checkbox>
            </Group>
            <Group title=heading("A platform option") native=tweak>
                <Text>{about}</Text>
            </Group>
        </Column>
    }
}

fn main() {
    App::new().window("Groups", WindowSize::FitHeight(480.0), settings).run();
}
