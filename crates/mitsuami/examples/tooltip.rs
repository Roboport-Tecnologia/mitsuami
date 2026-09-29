//! Tooltips: `cargo run -p mitsuami --example tooltip`.
//!
//! Rest the pointer on anything here: each shows its tooltip as the
//! platform shows them (its delay, placement and look).
//!
//! - A status line cut off at one line, whose tooltip has the whole text.
//! - Tooltips on controls, an image and a whole box.
//! - A tooltip that changes, and one that goes away.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

const STATUS: &str = "Couldn't start the player: /Applications/2ksbox.app/Contents/MacOS/player \
                      exited with status 1 before its window opened.";

fn status() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("A long status")}
            // Cut off at one line; the whole of it is in the tooltip.
            <Text max_lines=1 tooltip=STATUS width=320>{STATUS}</Text>
        </Column>
    }
}

fn controls() -> impl View {
    let blue = Pixels::new(2, 2, [[40, 90, 200, 255]; 4].concat());
    view! {
        <Column gap=Spacing::Md>
            {heading("On controls")}
            <Row gap=Spacing::Md align=Align::Center>
                <Button tooltip="Starts the machine">"Play"</Button>
                <Checkbox tooltip="Emulates a Sound Blaster 16">"Sound"</Checkbox>
                <NumberInput label="Memory" range_with=(16, 512) step=16 value=64 tooltip="Memory in MB"/>
                <Image source=ImageSource::Pixels(blue.scale(0.1)) label="A blue square" tooltip="An image"/>
            </Row>
            <Column
                padding=Spacing::Md
                gap=Spacing::Xs
                tooltip="The whole box has a tooltip, where its children have none"
            >
                <Text>"A box"</Text>
                <Text>"with two lines"</Text>
            </Column>
        </Column>
    }
}

fn changing() -> impl View {
    let running = signal(false);
    let shown = signal(true);
    view! {
        <Column gap=Spacing::Md>
            {heading("Changing")}
            <Row gap=Spacing::Md align=Align::Center>
                <Button
                    tooltip=move || if running.get() { "Stops the machine" } else { "Starts the machine" }.to_owned()
                    @click=move || running.update(|r| *r = !*r)
                >
                    {move || if running.get() { "Stop" } else { "Play" }.to_owned()}
                </Button>
                <Switch bind=shown>"Tooltip"</Switch>
                <Text tooltip=move || if shown.get() { "Here I am".into() } else { String::new() }>"Hover me"</Text>
            </Row>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {status()}
            {controls()}
            {changing()}
        </Column>
    }
}

fn main() {
    App::new().window("Tooltips", WindowSize::FitHeight(480.0), page).run();
}
