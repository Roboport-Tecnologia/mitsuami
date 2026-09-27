//! Windows opened while the app runs: `cargo run -p mitsuami --example windows`.
//!
//! - A launcher whose "Edit…" buttons open one machine window each, shown
//!   while a flag is true, as 2ksbox's launcher opens its windows.
//! - Closing a machine window with unsaved changes asks first; without
//!   changes, it closes.
//! - "About" opens a window that its close button closes (`bind`).
//! - "Open machines as" picks how the machine windows open: plain windows,
//!   sheets on the launcher (`Modality::Window`), or windows that block the
//!   whole app (`Modality::Application`). It applies from the next opening.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

/// What "Open machines as" offers, in its order.
const MODALITIES: [Option<Modality>; 3] = [None, Some(Modality::Window), Some(Modality::Application)];

/// One machine's settings, edited in a window of its own.
fn machine_window(name: &'static str, editing: Signal<bool>, memory: Signal<i32>, open_as: Signal<usize>) -> impl View {
    // What the form shows, apart from what's saved until "Save".
    let draft = signal(memory.get_untracked());
    let unsaved = move || draft.get() != memory.get();
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
                    memory.set(draft.get_untracked());
                    editing.set(false);
                }
                2 => editing.set(false),
                _ => {}
            }
        });
    };
    Window::new(move || if unsaved() { format!("{name} (edited)") } else { name.to_owned() })
        .size(WindowSize::FitHeight(360.0))
        .modality(move || MODALITIES[open_as.get()])
        .open(editing)
        .on_close_request(ask_to_close)
        .content(move || {
            // Each opening starts from what's saved.
            draft.set(memory.get_untracked());
            Column::new().padding(Spacing::Xl).gap(Spacing::Lg).children((
                Row::new().gap(Spacing::Md).align(Align::Center).children((
                    Text::new("Memory (MB)"),
                    NumberInput::new("Memory (MB)").range(16, 512).step(16).bind(draft),
                )),
                Row::new().gap(Spacing::Sm).justify(Justify::End).children((
                    Button::new("Cancel").role(ButtonRole::Cancel).on_click(ask_to_close),
                    Button::new("Save").role(ButtonRole::Default).enabled(unsaved).on_click(move || {
                        memory.set(draft.get_untracked());
                        editing.set(false);
                    }),
                )),
            ))
        })
}

fn machine_row(name: &'static str, memory: i32, open_as: Signal<usize>) -> impl View {
    let editing = signal(false);
    let memory = signal(memory);
    Row::new().gap(Spacing::Md).align(Align::Center).children((
        Text::new(name).grow(1.0),
        Text::new(move || format!("{} MB", memory.get())),
        Button::new("Edit…").enabled(move || !editing.get()).on_click(move || editing.set(true)),
        machine_window(name, editing, memory, open_as),
    ))
}

fn main() {
    App::new()
        .window("Machines", WindowSize::FitHeight(420.0), || {
            let about = signal(false);
            let open_as = signal(1);
            Column::new().padding(Spacing::Xl).gap(Spacing::Md).children((
                Text::new("Machines").text_style(TextStyle::Title),
                Row::new().gap(Spacing::Md).align(Align::Center).children((
                    Text::new("Open machines as"),
                    // A sheet on macOS; elsewhere a dialog that blocks this
                    // window (GTK's block the whole app).
                    Select::new("Open machines as")
                        .options(["Plain windows", "Blocking this window", "Blocking the app"])
                        .bind(open_as),
                )),
                machine_row("Windows 98", 64, open_as),
                machine_row("Windows XP", 256, open_as),
                Row::new().justify(Justify::End).child(Button::new("About").on_click(move || about.set(true))),
                Window::new("About").bind(about).content(|| {
                    Column::new().padding(Spacing::Xl).gap(Spacing::Sm).children((
                        Text::new("mitsuami").text_style(TextStyle::Headline),
                        Text::new("Windows opened while the app runs."),
                    ))
                }),
            ))
        })
        .run();
}
