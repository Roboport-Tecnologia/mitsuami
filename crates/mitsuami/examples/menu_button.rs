//! Menu buttons: `cargo run -p mitsuami --example menu_button`.
//!
//! Click a button to open its menu, as the platform shows it (a pull-down
//! on macOS, a popover on GNOME, a flyout on Windows, a menu on KDE).
//!
//! - A gallery: a caption, an icon before it, the icon alone, borderless,
//!   and disabled. Each platform draws its own arrow.
//! - A machine's actions: items that change with its state (Start or
//!   Stop, a disabled Reset while it's off), a submenu of radio items, a
//!   check item and separators. The log under it says what was chosen.
//! - A raw platform setting, through `.native()`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

fn plus() -> &'static str {
    platform! {
        macos => "plus",
        gtk => "list-add-symbolic",
        kde => "list-add",
        windows => "\u{E710}",
    }
}

fn gallery() -> impl View {
    let menu = || (MenuItem::new("Disc image…"), MenuItem::new("Folder…"), MenuSeparator, MenuItem::new("Guest tools"));
    view! {
        <Column gap=Spacing::Md>
            {heading("Menu buttons")}
            <Row gap=Spacing::Md align=Align::Center>
                <MenuButton menu=menu()>"Add"</MenuButton>
                <MenuButton icon=plus() menu=menu()>"Add"</MenuButton>
                <MenuButton icon=plus() icon_only=true tooltip="Add" menu=menu()>"Add"</MenuButton>
                <MenuButton icon=plus() button_style=ButtonStyle::Borderless menu=menu()>"Add"</MenuButton>
                <MenuButton enabled=false menu=menu()>"Add"</MenuButton>
            </Row>
        </Column>
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Speed {
    Slow,
    Normal,
    Fast,
}

/// A machine's actions, which follow its state.
fn machine() -> impl View {
    let running = signal(false);
    let speed = signal(Speed::Normal);
    let sound = signal(true);
    let log = signal(String::from("Nothing chosen yet"));
    let say = move |what: String| log.set(what);
    view! {
        <Column gap=Spacing::Md>
            {heading("A machine")}
            <Row gap=Spacing::Md align=Align::Center>
                <Text grow=1.0>{move || if running.get() { "Windows 98 is running" } else { "Windows 98 is off" }.to_string()}</Text>
                <MenuButton menu=(
                    MenuItem::new(move || if running.get() { "Stop" } else { "Start" }.to_string()).on_select(move || {
                        running.update(|r| *r = !*r);
                        say(if running.get_untracked() { "Started".into() } else { "Stopped".into() });
                    }),
                    MenuItem::new("Reset").enabled(running).on_select(move || say("Reset".into())),
                    MenuSeparator,
                    Menu::new("Speed")
                        .item(MenuItem::new("Slow").radio((speed, Speed::Slow)))
                        .item(MenuItem::new("Normal").radio((speed, Speed::Normal)))
                        .item(MenuItem::new("Fast").radio((speed, Speed::Fast))),
                    MenuItem::new("Sound").bind(sound),
                )>"Actions"</MenuButton>
            </Row>
            <Text text_style=TextStyle::Caption>
                {move || format!("{} · speed {:?} · sound {}", log.get(), speed.get(), if sound.get() { "on" } else { "off" })}
            </Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native button.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<MenuButton>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|p: &mitsuami::appkit::objc2_app_kit::NSPopUpButton| {
                p.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Large)
            }),
            "AppKit: a large control size, as prominent toolbar and sheet buttons use.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::MenuButton| b.set_direction(mitsuami::gtk::gtk::ArrowType::Up)),
            "GTK: direction up opens the popover above the button, and turns its arrow.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_bool("flat", true)),
            "Qt Quick: flat draws the button without a frame until hovered.",
        ),
        windows => (
            mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::DropDownButton| {
                use mitsuami::winui::bindings::IControl;
                use mitsuami::winui::windows_core::Interface;
                b.cast::<IControl>()?.SetCornerRadius(mitsuami::winui::bindings::CornerRadius {
                    top_left: 16.0,
                    top_right: 16.0,
                    bottom_right: 16.0,
                    bottom_left: 16.0,
                })
            }),
            "WinUI: CornerRadius rounds the button into a pill.",
        ),
    };
    let menu = || (MenuItem::new("Disc image…"), MenuItem::new("Folder…"));
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Lg align=Align::Center>
                <MenuButton menu=menu()>"Plain"</MenuButton>
                <MenuButton native=tweak menu=menu()>"Tweaked"</MenuButton>
            </Row>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Menu buttons", WindowSize::FitHeight(560.0), || {
            view! {
                <Column padding=Spacing::Xl gap=Spacing::Xl>
                    {gallery()}
                    {machine()}
                    {platform_option()}
                </Column>
            }
        })
        .run();
}
