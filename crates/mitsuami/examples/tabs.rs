//! Tabs: `cargo run -p mitsuami --example tabs`.
//!
//! A preferences window with its panes in the platform's tab view.
//!
//! - Pick a tab with the pointer, or with the keyboard where the platform
//!   lets tabs take focus: the page follows.
//! - Type a name, pick another tab and come back: it's still there, as
//!   hidden pages stay built.
//! - "Next tab" picks from the app's side: the tabs follow.

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
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Tabs selection=pane grow=1.0>
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
            <Row gap=Spacing::Md>
                <Button @click=next>"Next tab"</Button>
                <Text>{move || format!("Showing {:?}", pane.get())}</Text>
            </Row>
        </Column>
    }
}

fn main() {
    App::new().window("Preferences", Size::new(480.0, 360.0), preferences).run();
}
