//! Moving files to the trash: `cargo run -p mitsuami --example trash`.
//!
//! - The window makes a few files and a folder in a folder of its own
//!   (`mitsuami trash example`, in the system's temporary folder) and
//!   lists them. Select some and press Move to Trash: they go to the
//!   platform's trash, as its file manager's would, and Finder's Put Back,
//!   Files' and Dolphin's Restore, or the Recycle Bin's Restore bring them
//!   back. Refresh lists what's there again; Make Files puts back any that
//!   are missing.
//! - Where the disk has no trash, the app asks whether to delete them
//!   right away, as Finder and Files do.
//! - On Windows the shell asks first when the Recycle Bin's settings say
//!   so, as Explorer's Delete does. Saying no moves nothing.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mitsuami::prelude::*;

fn folder() -> PathBuf {
    std::env::temp_dir().join("mitsuami trash example")
}

/// What's in the folder, by name.
fn listing() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> =
        std::fs::read_dir(folder()).map(|dir| dir.filter_map(|e| Some(e.ok()?.path())).collect()).unwrap_or_default();
    paths.sort();
    paths
}

/// Puts back the files and the folder the example starts with.
fn make_files() {
    let folder = folder();
    for name in ["Notes.txt", "Shopping list.txt", "Draft.md"] {
        let path = folder.join(name);
        if !path.exists() {
            _ = std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&path, format!("This is {name}.\n")));
        }
    }
    _ = std::fs::create_dir_all(folder.join("Old photos"));
}

fn name(path: &std::path::Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn items(count: usize) -> String {
    if count == 1 { "1 item".into() } else { format!("{count} items") }
}

/// Moves `paths` to the trash, or deletes them for good if their disk has
/// none and the user agrees. Says what happened.
async fn move_to_trash(paths: Vec<PathBuf>) -> String {
    let count = paths.len();
    match trash(paths.clone()).await {
        Ok(()) => format!("Moved {} to the trash.", items(count)),
        Err(ServiceError::Cancelled) => "Nothing was moved.".into(),
        Err(ServiceError::Unavailable) => {
            // Those before the one that failed are in the trash already.
            let left: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
            let answer = alert(
                Alert::new(format!("Delete {} immediately?", items(left.len())))
                    .message("This disk has no trash. You can't undo this action.")
                    .button("Delete")
                    .button("Cancel")
                    .style(AlertStyle::Warning),
            )
            .await;
            if answer != 0 {
                return "Nothing was deleted.".into();
            }
            let deleted = spawn_blocking(move || {
                left.iter()
                    .try_for_each(|p| if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) })
            })
            .await;
            match deleted {
                Ok(()) => "Deleted them.".into(),
                Err(error) => format!("Couldn't delete them: {error}"),
            }
        }
        Err(error) => format!("Couldn't move them to the trash: {error}"),
    }
}

pub fn page() -> impl View {
    make_files();
    let files = signal(listing());
    let selected = signal(Vec::<PathBuf>::new());
    let status = signal(String::new());
    let refresh = move || {
        files.set(listing());
        selected.update(|s| s.retain(|p| p.exists()));
    };
    let trash_selected = move || {
        let paths = selected.get_untracked();
        spawn_local(async move {
            status.set(move_to_trash(paths).await);
            refresh();
        });
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md grow=1.0>
            <Text color=Color::SecondaryLabel>{format!("In {}", folder().display())}</Text>
            <List
                each=files
                key=|p: &PathBuf| p.clone()
                selection_mode=SelectionMode::Multiple
                selected=selected
                list_style=ListStyle::Framed
                grow=1.0
                min_height=120
                let:path
            >
                <Row padding_x=Spacing::Md padding_y=Spacing::Xs>
                    <Text>{name(&path)}</Text>
                </Row>
            </List>
            <Row gap=Spacing::Sm>
                <Button enabled=move || !selected.get().is_empty() @click=trash_selected>"Move to Trash"</Button>
                <Button @click=refresh>"Refresh"</Button>
                <Button @click=move || { make_files(); refresh(); }>"Make Files"</Button>
            </Row>
            <Text test_id="status">{status}</Text>
        </Column>
    }
}

fn main() {
    App::new().window("Trash", Size::new(480.0, 400.0), page).run();
}
