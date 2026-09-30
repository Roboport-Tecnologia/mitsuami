//! Windows opened while the app runs: `cargo run -p mitsuami --example windows`.
//!
//! - A launcher whose "Edit…" buttons open one machine window each, shown
//!   while a flag is true, as 2ksbox's launcher opens its windows.
//! - Closing a machine window with unsaved changes asks first; without
//!   changes, it closes.
//! - "Advanced…" in a machine window opens a dialog on that window, nested
//!   in it: OK applies its changes to the machine window's form, Cancel
//!   (or Escape, or its close button) drops them. It closes with the
//!   machine window.
//! - A machine window's "Maximized" and "Resizable" switches maximize it
//!   (on macOS, zoom it) and keep the user from resizing it. Maximizing it
//!   from its title bar sets the switch.
//! - "About" opens a window that its close button closes (`bind`).
//! - "Open machines as" picks how the machine windows open: plain windows,
//!   sheets on the launcher (`Modality::Window`), or windows that block the
//!   whole app (`Modality::Application`). It applies from the next opening.
//!   Escape closes a modal one, as it closes dialogs (asking first, with
//!   unsaved changes); a plain one ignores it.
//!   A dialog follows its content's height; a plain window has a fixed
//!   size, since macOS can open it as a tab of the launcher (always in
//!   full screen), and a tab takes its window's size.
//! - The app's id, name and icon (`App::id`, `name`, `icon`): the Dock's
//!   icon and "Quit Machines" on macOS, the windows' icon on Windows,
//!   " — Machines" after each title on KDE. GTK looks for the icon named
//!   after the id in the theme, which an installed app puts there.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

/// What "Open machines as" offers, in its order.
const MODALITIES: [Option<Modality>; 3] = [None, Some(Modality::Window), Some(Modality::Application)];

/// A machine's settings, one signal each, so fields bind to them.
#[derive(Clone, Copy)]
struct Settings {
    memory: Signal<i32>,
    cpus: Signal<i32>,
}

impl Settings {
    fn new(memory: i32, cpus: i32) -> Settings {
        Settings { memory: signal(memory), cpus: signal(cpus) }
    }

    fn copy_from(self, other: Settings) {
        self.memory.set(other.memory.get_untracked());
        self.cpus.set(other.cpus.get_untracked());
    }

    fn differs_from(self, other: Settings) -> bool {
        self.memory.get() != other.memory.get() || self.cpus.get() != other.cpus.get()
    }
}

/// The advanced settings of a machine window's form, in a dialog on that
/// window. Its own draft starts from the form at each opening; OK copies it
/// back, anything else drops it.
fn advanced_window(form: Settings, open: Signal<bool>) -> impl View {
    let cpus = signal(0);
    view! {
        <Window title="Advanced" modal=Modality::Window bind=open @open=move || cpus.set(form.cpus.get_untracked())>
            <Column padding=Spacing::Xl gap=Spacing::Lg>
                <Row gap=Spacing::Md align=Align::Center>
                    <Text>"Processors"</Text>
                    <NumberInput label="Processors" range_with=(1, 16) bind=cpus/>
                </Row>
                <Row gap=Spacing::Sm justify=Justify::End>
                    <Button role=ButtonRole::Cancel @click=move || open.set(false)>"Cancel"</Button>
                    <Button role=ButtonRole::Default @click=move || {
                        form.cpus.set(cpus.get_untracked());
                        open.set(false);
                    }>"OK"</Button>
                </Row>
            </Column>
        </Window>
    }
}

/// One machine's settings, edited in a window of its own.
fn machine_window(name: &'static str, editing: Signal<bool>, saved: Settings, open_as: Signal<usize>) -> impl View {
    // What the form shows, apart from what's saved until "Save".
    let form = Settings::new(0, 0);
    let advanced = signal(false);
    let (maximized, resizable) = (signal(false), signal(true));
    let unsaved = move || form.differs_from(saved);
    let ask_to_close = move || {
        if !unsaved() {
            editing.set(false);
            return;
        }
        spawn_local(async move {
            let answer = alert(
                Alert::new(format!("Save the changes to {name}?"))
                    .message("Your changes are lost if you don't save them.")
                    .button("Save")
                    .button("Cancel")
                    .button("Don't Save")
                    .style(AlertStyle::Warning),
            )
            .await;
            match answer {
                0 => {
                    saved.copy_from(form);
                    editing.set(false);
                }
                2 => editing.set(false),
                _ => {}
            }
        });
    };
    view! {
        <Window
            title=move || if unsaved() { format!("{name} (edited)") } else { name.to_owned() }
            // A dialog is as tall as what it shows: it grows for the note
            // below. A plain window has a size of its own: macOS can open
            // it as a tab of the launcher, which would follow its height.
            size=move || match MODALITIES[open_as.get()] {
                None => WindowSize::Fixed(Size::new(360.0, 260.0)),
                Some(_) => WindowSize::FollowHeight(360.0),
            }
            modality=move || MODALITIES[open_as.get()]
            maximized=maximized
            resizable=resizable
            open=editing
            // Each opening starts from what's saved.
            @open=move || form.copy_from(saved)
            @close_request=ask_to_close
        >
            <Column padding=Spacing::Xl gap=Spacing::Lg>
                <Row gap=Spacing::Md align=Align::Center>
                    <Text>"Memory (MB)"</Text>
                    <NumberInput label="Memory (MB)" range_with=(16, 512) step=16 bind=form.memory/>
                </Row>
                <Row gap=Spacing::Md align=Align::Center>
                    <Text grow=1.0>{move || format!("Processors: {}", form.cpus.get())}</Text>
                    <Button @click=move || advanced.set(true)>"Advanced…"</Button>
                </Row>
                <Row gap=Spacing::Md align=Align::Center>
                    <Text>"Maximized"</Text>
                    <Switch bind=maximized>"Maximized"</Switch>
                    <Text>"Resizable"</Text>
                    <Switch bind=resizable>"Resizable"</Switch>
                </Row>
                <Show when=unsaved>
                    <Text>"Changes take effect the next time the machine starts."</Text>
                </Show>
                <Row gap=Spacing::Sm justify=Justify::End>
                    <Button role=ButtonRole::Cancel @click=ask_to_close>"Cancel"</Button>
                    <Button role=ButtonRole::Default enabled=unsaved @click=move || {
                        saved.copy_from(form);
                        editing.set(false);
                    }>"Save"</Button>
                </Row>
                // Declared in the machine window: it's a sheet on it, and
                // closes with it.
                {advanced_window(form, advanced)}
            </Column>
        </Window>
    }
}

fn machine_row(name: &'static str, saved: Settings, open_as: Signal<usize>) -> impl View {
    let editing = signal(false);
    view! {
        <Row gap=Spacing::Md align=Align::Center>
            <Text grow=1.0>{name}</Text>
            <Text>{move || format!("{} MB, {} CPU", saved.memory.get(), saved.cpus.get())}</Text>
            <Button enabled=move || !editing.get() @click=move || editing.set(true)>"Edit…"</Button>
            {machine_window(name, editing, saved, open_as)}
        </Row>
    }
}

pub fn page() -> impl View {
    let about = signal(false);
    let open_as = signal(1);
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Text text_style=TextStyle::Title>"Machines"</Text>
            <Row gap=Spacing::Md align=Align::Center>
                <Text>"Open machines as"</Text>
                // A sheet on macOS; elsewhere a dialog that blocks
                // this window (GTK's block the whole app).
                <Select
                    label="Open machines as"
                    options=["Plain windows", "Blocking this window", "Blocking the app"]
                    bind=open_as
                />
            </Row>
            {machine_row("Windows 98", Settings::new(64, 1), open_as)}
            {machine_row("Windows XP", Settings::new(256, 2), open_as)}
            <Row justify=Justify::End>
                <Button @click=move || about.set(true)>"About"</Button>
            </Row>
            <Window title="About" bind=about>
                <Column padding=Spacing::Xl gap=Spacing::Sm>
                    <Text text_style=TextStyle::Headline>"mitsuami"</Text>
                    <Text>"Windows opened while the app runs."</Text>
                </Column>
            </Window>
        </Column>
    }
}

fn main() {
    App::new()
        .id("br.com.roboport.mitsuami.Machines")
        .name("Machines")
        .icon(AppIcon::bytes(include_bytes!("../tests/assets/blue-red-20x10.png").as_slice()))
        .window("Machines", WindowSize::FitHeight(420.0), page)
        .run();
}
