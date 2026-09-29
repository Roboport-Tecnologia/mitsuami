//! Tabs: `cargo run -p mitsuami --example tabs`.
//!
//! A preferences window with its panes in the platform's tab view.
//!
//! - Pick a tab with the pointer, or with the keyboard where the platform
//!   lets tabs take focus: the page follows.
//! - Type a name, pick another tab and come back: it's still there, as
//!   hidden pages stay built.
//! - "Next tab" picks from the app's side: the tabs follow.
//! - On GNOME and KDE, "Tabs" switches between the two ways they show
//!   tabs: navigation tabs (libadwaita's view switcher, Kirigami's
//!   navigation bar) and a tab bar (GTK's notebook, Qt's tab bar). The
//!   page shown stays.
//! - "Icons" puts an icon on each tab, where the platform's tabs show one:
//!   libadwaita's view switcher, WinUI's selector bar, Qt's tabs. AppKit's
//!   tab view and GTK's notebook show only titles.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pane {
    General,
    Appearance,
    Advanced,
}

pub fn preferences() -> impl View {
    let pane = signal(Pane::General);
    let next = move || {
        pane.set(match pane.get() {
            Pane::General => Pane::Appearance,
            Pane::Appearance => Pane::Advanced,
            Pane::Advanced => Pane::General,
        })
    };
    // Only GNOME and KDE have two ways; the others show theirs.
    let two_ways = platform! { linux => true, _ => false };
    let way = signal(0);
    let style = move || if way.get() == 0 { TabsStyle::Navigation } else { TabsStyle::TabBar };
    let icons = signal(true);
    let (general, appearance, advanced) = platform! {
        macos => ("gearshape", "paintbrush", "wrench.and.screwdriver"),
        gtk => ("emblem-system-symbolic", "applications-graphics-symbolic", "applications-engineering-symbolic"),
        kde => ("configure", "preferences-desktop-theme", "preferences-other"),
        windows => ("\u{E713}", "\u{E790}", "\u{E90F}"),
    };
    let icon = move |name: &'static str| move || if icons.get() { name.to_string() } else { String::new() };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Tabs selection=pane tabs_style=style grow=1.0>
                <Tab title="General" value=Pane::General icon=icon(general) padding=Spacing::Lg gap=Spacing::Md>
                    <Text>"Name"</Text>
                    <TextInput placeholder="Untitled"/>
                    <Checkbox>"Open at login"</Checkbox>
                </Tab>
                <Tab title="Appearance" value=Pane::Appearance icon=icon(appearance) padding=Spacing::Lg gap=Spacing::Md>
                    <Switch>"Dark mode"</Switch>
                    <Checkbox>"Show the status bar"</Checkbox>
                </Tab>
                <Tab title="Advanced" value=Pane::Advanced icon=icon(advanced) padding=Spacing::Lg gap=Spacing::Md>
                    <Checkbox>"Verbose logging"</Checkbox>
                    <Text>"Changes here take effect after a restart."</Text>
                </Tab>
            </Tabs>
            <Row gap=Spacing::Md align=Align::Center>
                <Button @click=next>"Next tab"</Button>
                <Text>{move || format!("Showing {:?}", pane.get())}</Text>
                <Text>"Icons"</Text>
                <Switch bind=icons>"Icons"</Switch>
                <Show when=two_ways>
                    <Row gap=Spacing::Sm align=Align::Center>
                        <Text>"Tabs"</Text>
                        <Select label="Tabs" options=["Navigation tabs", "Tab bar"] bind=way/>
                    </Row>
                </Show>
            </Row>
        </Column>
    }
}

fn main() {
    App::new().window("Preferences", Size::new(480.0, 360.0), preferences).run();
}
