//! Menus: `cargo run -p mitsuami --example menus`.
//!
//! 2ksbox's launcher with a menu bar: the app's menus, and the machine
//! window's own. Where they show is the platform's: macOS's menu bar,
//! GNOME's primary menu, a menu bar in each window on Windows, and KDE's
//! hamburger menu.
//!
//! - Machine › Start becomes Pause while it runs, and Reset appears.
//! - View › Show Sidebar and the zoom choices show check marks.
//! - File › Open Recent grows as machines are started.
//! - Settings… and About go where the platform puts them (the app menu
//!   on macOS, the end of the menu on GNOME and KDE); on macOS, Quit is
//!   the app's own, which asks first.
//! - The Editor window has a Format menu of its own: on macOS it's in the
//!   menu bar while the Editor is the main window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Zoom {
    Small,
    Large,
}

fn launcher() -> impl View {
    let running = signal(false);
    let sidebar = signal(true);
    let zoom = signal(Zoom::Small);
    let recent = signal(Vec::<String>::new());
    let editing = signal(false);
    let log = signal(String::from("Nothing chosen yet."));
    let say = move |what: &'static str| move || log.set(format!("Chose {what}."));
    let start = move || {
        running.update(|r| *r = !*r);
        if running.get_untracked() {
            recent.update(|r| r.insert(0, format!("Machine {}", r.len() + 1)));
        }
    };
    let quit = move || {
        spawn_local(async move {
            let answer = alert(Alert::new("Quit the launcher?").button("Quit").button("Cancel")).await;
            if answer == 0 {
                std::process::exit(0);
            }
        });
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <MenuBar>
                <Menu title="File">
                    <MenuItem shortcut=Shortcut::primary('e') @select=move || editing.set(true)>"Edit Notes…"</MenuItem>
                    <Menu
                        title="Open Recent"
                        visible=move || !recent.get().is_empty()
                        children_with=move || recent.get().into_iter().map(MenuItem::new).collect::<Vec<_>>()
                    />
                    <MenuSeparator/>
                    <MenuItem role=MenuRole::Settings @select=say("Settings")>"Settings…"</MenuItem>
                    <MenuItem role=MenuRole::Quit @select=quit>"Exit"</MenuItem>
                </Menu>
                <Menu title="Machine">
                    <MenuItem shortcut=Shortcut::primary('r') @select=start>
                        {move || if running.get() { "Pause" } else { "Start" }.to_owned()}
                    </MenuItem>
                    <MenuItem visible=running @select=say("Reset")>"Reset"</MenuItem>
                </Menu>
                <Menu title="View">
                    <MenuItem bind=sidebar>"Show Sidebar"</MenuItem>
                    <MenuSeparator/>
                    <MenuItem radio=(zoom, Zoom::Small)>"Small"</MenuItem>
                    <MenuItem radio=(zoom, Zoom::Large)>"Large"</MenuItem>
                </Menu>
                <Menu title="Help">
                    <MenuItem role=MenuRole::About @select=say("About")>"About the Launcher"</MenuItem>
                </Menu>
            </MenuBar>
            <Text>{move || if running.get() { "Running." } else { "Stopped." }.to_owned()}</Text>
            <Text>{move || format!("Sidebar {}, zoom {}.", if sidebar.get() { "shown" } else { "hidden" }, if zoom.get() == Zoom::Large { "large" } else { "small" })}</Text>
            <Text>{log}</Text>
            <Window title="Editor" bind=editing>
                <Column padding=Spacing::Xl gap=Spacing::Md>
                    <MenuBar>
                        <Menu title="Format">
                            <MenuItem shortcut=Shortcut::primary('b') @select=say("Bold")>"Bold"</MenuItem>
                        </Menu>
                    </MenuBar>
                    <TextInput a11y_label="Notes" placeholder="Notes"/>
                </Column>
            </Window>
        </Column>
    }
}

fn main() {
    App::new().window("Launcher", WindowSize::FitHeight(480.0), launcher).run();
}
