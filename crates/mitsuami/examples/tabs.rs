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
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Tabs selection=pane tabs_style=style grow=1.0>
                <Tab title="General" value=Pane::General padding=Spacing::Lg gap=Spacing::Md>
                    <Text>"Name"</Text>
                    <TextInput placeholder="Untitled"/>
                    <Checkbox>"Open at login"</Checkbox>
                </Tab>
                <Tab title="Appearance" value=Pane::Appearance padding=Spacing::Lg gap=Spacing::Md>
                    <Switch>"Dark mode"</Switch>
                    <Checkbox>"Show the status bar"</Checkbox>
                </Tab>
                <Tab title="Advanced" value=Pane::Advanced padding=Spacing::Lg gap=Spacing::Md>
                    <Checkbox>"Verbose logging"</Checkbox>
                    <Text>"Changes here take effect after a restart."</Text>
                </Tab>
            </Tabs>
            <Row gap=Spacing::Md align=Align::Center>
                <Button @click=next>"Next tab"</Button>
                <Text>{move || format!("Showing {:?}", pane.get())}</Text>
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
