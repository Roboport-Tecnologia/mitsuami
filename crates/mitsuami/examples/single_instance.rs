//! One instance per user: `cargo run -p mitsuami --example single_instance`.
//!
//! - Run it again, with arguments or not (`-- hello`): the new process
//!   hands its launch to the running one and ends, and the running one
//!   comes forward and lists it.
//! - `-- stop` quits the running one.
//! - `-- --hidden` starts it without its window, as an app started at
//!   login would; launching it again opens the window. Closing the window
//!   leaves it running.
//!
//! Each launch is printed too, for a run without its window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

/// How a launch reads in the list.
pub fn describe(reopen: &Reopen) -> String {
    let mut text = match reopen.args() {
        [] => "No arguments".to_owned(),
        args => args.join(" "),
    };
    if let Some(cwd) = reopen.cwd() {
        text += &format!(", in {}", cwd.display());
    }
    for file in reopen.files() {
        text += &format!(", opening {}", file.display());
    }
    text
}

/// Lists the launches handed over so far.
pub fn page(launches: Signal<Vec<String>>) -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Text>"Launch this app again: it comes forward here instead of starting twice."</Text>
            <Text test_id="launches">
                {move || match launches.get() {
                    list if list.is_empty() => "Not launched again yet.".to_owned(),
                    list => list.join("\n"),
                }}
            </Text>
            <Row justify=Justify::End>
                <Button @click=quit>"Quit"</Button>
            </Row>
        </Column>
    }
}

/// The window, and what a later launch does: `stop` quits, anything else
/// opens the window and lists the launch.
pub fn window(hidden: bool) -> Window {
    let open = signal(!hidden);
    let launches = signal(Vec::<String>::new());
    on_reopen(move |reopen| {
        println!("Launched again: {}", describe(reopen));
        if reopen.args().first().map(String::as_str) == Some("stop") {
            quit();
            return;
        }
        launches.update(|list| list.push(describe(reopen)));
        open.set(true);
    });
    Window::new("Single Instance").size(WindowSize::FitHeight(420.0)).bind(open).content(move || page(launches))
}

fn main() {
    let hidden = std::env::args().any(|arg| arg == "--hidden");
    App::new()
        .id("br.com.roboport.mitsuami.SingleInstance")
        .name("Single Instance")
        .single_instance()
        .runs_without_windows()
        .open(move || window(hidden))
        .run();
}
