//! Dragging files out: `cargo run -p mitsuami --example file_drag`.
//!
//! - The window makes a few files and a folder in a folder of its own
//!   (`mitsuami drag example`, in the user's cache folder on Linux, the
//!   temporary folder elsewhere) and lists them, in a list and in a table.
//!   Drag a row to the file manager, the desktop, Mail or another app: it
//!   gets a copy of the file. Select several and drag one of them to drag
//!   them all, as a file manager drags a selection.
//! - "Notes (no file)" has no file, so it doesn't drag.
//! - Dragging isn't reachable from the keyboard or a screen reader, so Copy
//!   Paths puts the selection's paths on the clipboard, as another way out.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mitsuami::prelude::*;

/// The folder the example's files go in. Linux's temporary folder is shared
/// by every user, who could make this folder first and plant links in it,
/// so there it's the user's cache folder; macOS's and Windows' are per user.
fn folder() -> PathBuf {
    #[cfg(not(any(target_os = "macos", windows)))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join(".cache"));
    #[cfg(any(target_os = "macos", windows))]
    let base = std::env::temp_dir();
    base.join("mitsuami drag example")
}

#[derive(Clone, Debug, PartialEq)]
struct Item {
    name: &'static str,
    kind: &'static str,
    /// What dragging its row carries.
    file: Option<PathBuf>,
}

/// The items, their files made if missing.
fn items() -> Vec<Item> {
    let folder = folder();
    _ = std::fs::create_dir_all(folder.join("Holiday photos"));
    let files = [("Report.txt", "Plain text"), ("Budget.csv", "CSV document"), ("Draft.md", "Markdown")];
    let mut items: Vec<Item> = files
        .iter()
        .map(|&(name, kind)| {
            let path = folder.join(name);
            if !path.exists() {
                _ = std::fs::write(&path, format!("This is {name}, dragged out of mitsuami.\n"));
            }
            Item { name, kind, file: Some(path) }
        })
        .collect();
    items.push(Item { name: "Holiday photos", kind: "Folder", file: Some(folder.join("Holiday photos")) });
    items.push(Item { name: "Notes (no file)", kind: "Nothing to drag", file: None });
    items
}

pub fn page() -> impl View {
    let all = items();
    let (list_items, table_items) = (all.clone(), all);
    let list_selected = signal(Vec::<&'static str>::new());
    let table_selected = signal(Vec::<&'static str>::new());
    let status = signal(String::new());
    let copy_paths = move || {
        let chosen: Vec<&str> = list_selected.get_untracked();
        let paths: Vec<String> = items()
            .into_iter()
            .filter(|i| chosen.contains(&i.name))
            .filter_map(|i| i.file.map(|f| f.display().to_string()))
            .collect();
        let count = paths.len();
        let written = set_clipboard_text(&paths.join("\n"));
        spawn_local(async move {
            status.set(match written.await {
                Ok(()) => format!("Copied {count} path(s)."),
                Err(error) => format!("Couldn't copy: {error}"),
            })
        });
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md grow=1.0>
            <Text color=Color::SecondaryLabel>{format!("In {}", folder().display())}</Text>
            <Text text_style=TextStyle::Headline>"List"</Text>
            <List
                each=move || list_items.clone()
                key=|i: &Item| i.name
                selection_mode=SelectionMode::Multiple
                selected=list_selected
                drag_files=|i: &Item| i.file.clone()
                list_style=ListStyle::Framed
                height=150
                let:item
            >
                <Row padding_x=Spacing::Md padding_y=Spacing::Xs>
                    <Text>{item.name}</Text>
                </Row>
            </List>
            <Row gap=Spacing::Sm align=Align::Center>
                <Button enabled=move || !list_selected.get().is_empty() @click=copy_paths>"Copy Paths"</Button>
                <Text test_id="status">{status}</Text>
            </Row>
            <Text text_style=TextStyle::Headline>"Table"</Text>
            {Table::new(move || table_items.clone(), |i: &Item| i.name)
                .column(TableColumn::new("Name", |i: Item| Text::new(i.name)).expand())
                .column(TableColumn::new("Kind", |i: Item| Text::new(i.kind)).width(140))
                .selection_mode(SelectionMode::Multiple)
                .selected(table_selected)
                .drag_files(|i: &Item| i.file.clone())
                .list_style(ListStyle::Framed)
                .height(170)}
        </Column>
    }
}

fn main() {
    App::new().window("Dragging files out", Size::new(520.0, 560.0), page).run();
}
