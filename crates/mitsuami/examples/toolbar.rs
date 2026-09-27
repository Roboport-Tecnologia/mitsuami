//! A window's toolbar: `cargo run -p mitsuami --example toolbar`.
//!
//! 2ksbox's launcher header: the window's title in the bar across the
//! top, and at its trailing end a download's progress (only while it
//! runs) and a status line cut off at one line.
//!
//! - Start and stop the download: its item comes and goes.
//! - Make the status long: it's cut off, and its tooltip has all of it.
//! - The toolbar's button works like any other.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

const LONG: &str = "Couldn't start the player: /Applications/2ksbox.app/Contents/MacOS/player \
                    exited with status 1 before its window opened.";

fn launcher() -> impl View {
    let busy = signal(false);
    let status = signal(String::from("Ready"));
    let machines = signal(0);
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Lg>
            <Toolbar>
                <Show when=busy>
                    <Row gap=Spacing::Sm align=Align::Center>
                        <Spinner label="Downloading presets"/>
                        <Text>"Downloading presets… 12 MB"</Text>
                    </Row>
                </Show>
                <Text max_lines=1 max_width=320 tooltip=status>{status}</Text>
                <Button @click=move || machines.update(|m| *m += 1)>"New machine"</Button>
            </Toolbar>
            <Text>
                {move || match machines.get() {
                    0 => "No machines yet.".to_owned(),
                    1 => "1 machine.".to_owned(),
                    n => format!("{n} machines."),
                }}
            </Text>
            <Row gap=Spacing::Md>
                <Button @click=move || busy.update(|b| *b = !*b)>
                    {move || if busy.get() { "Stop the download" } else { "Start a download" }.to_owned()}
                </Button>
                <Button @click=move || status.set(LONG.to_owned())>"Long status"</Button>
                <Button @click=move || status.set("Ready".to_owned())>"Short status"</Button>
            </Row>
        </Column>
    }
}

fn main() {
    App::new().window("Machines", WindowSize::FitHeight(800.0), launcher).run();
}
