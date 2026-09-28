//! Dropping files: `cargo run -p mitsuami --example file_drop`.
//!
//! Drag files and folders from Finder, Files, Dolphin or Explorer onto the
//! window's groups.
//!
//! - "Disc images" takes .iso and .cue files and folders: while they're
//!   over it the platform shows it'll copy them, and its heading says so;
//!   other files are refused, and it doesn't light up for them.
//! - "Anything" takes any file, but no folders.
//! - What was dropped is listed under them, newest first. Dragging isn't
//!   reachable from the keyboard or a screen reader, so "Add…" opens the
//!   platform's file dialog as the other way in.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mitsuami::prelude::*;

fn zone(title: &'static str, hint: &'static str, drop: FileDrop, dropped: Signal<Vec<String>>) -> impl View {
    let over = signal(false);
    let heading = move || if over.get() { format!("{title}: release to add") } else { title.to_string() };
    Group::new()
        .title(heading)
        .file_drop(drop)
        .on_drop(move |paths: Vec<PathBuf>| {
            dropped.update(|d| {
                for path in paths.iter().rev() {
                    d.insert(0, format!("{title}: {}", path.display()));
                }
            })
        })
        .on_drop_hover(move |hover| over.set(hover))
        .grow(1.0)
        .child(Text::new(hint).color(Color::SecondaryLabel))
}

fn window() -> impl View {
    let dropped = signal(Vec::<String>::new());
    let add = move || {
        spawn_local(async move {
            let files = open_file(
                OpenFile::new()
                    .title("Add disc images")
                    .multiple()
                    .filter(FileFilter::new("Disc images", ["iso", "cue"])),
            )
            .await;
            if let Some(files) = files {
                dropped.update(|d| {
                    for path in files.iter().rev() {
                        d.insert(0, format!("Added: {}", path.display()));
                    }
                })
            }
        });
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Lg>
            <Row gap=Spacing::Lg>
                {zone("Disc images", "Drop .iso, .cue files or folders here", FileDrop::extensions(["iso", "cue"]).and_folders(), dropped)}
                {zone("Anything", "Drop any file here", FileDrop::files(), dropped)}
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Text text_style=TextStyle::Headline grow=1.0>"Dropped"</Text>
                <Button @click=add>"Add…"</Button>
            </Row>
            <ScrollView grow=1.0>
                <Column gap=Spacing::Xs>
                    <Show when=move || dropped.get().is_empty()>
                        <Text color=Color::SecondaryLabel>"Nothing yet"</Text>
                    </Show>
                    <For each=dropped key=|line: &String| line.clone() let:line>
                        <Text max_lines=1u32>{line}</Text>
                    </For>
                </Column>
            </ScrollView>
        </Column>
    }
}

fn main() {
    App::new().window("Dropping files", Size::new(640.0, 420.0), window).run();
}
