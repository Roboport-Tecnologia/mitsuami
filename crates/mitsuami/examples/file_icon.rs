//! Files' icons: `cargo run -p mitsuami --example file_icon`.
//!
//! - A folder's items (the home folder's to start with; Choose Folder…
//!   picks another), each with the icon the platform's file manager gives
//!   it: an app's own, a folder's (a custom one too), a document's by its
//!   type. On Linux they're the icon theme's, by the file's content type.
//! - Size makes them larger, as a file manager's zoom does.
//! - Thumbnails shows previews of what's in the files (pictures, PDFs,
//!   documents) where the platform makes them: QuickLook's on macOS, the
//!   shell's on Windows, and on GNOME those the desktop has made already
//!   (open the folder in Files first). KDE's are KIO's, which a Qt Quick
//!   app doesn't have, so there it shows icons.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mitsuami::prelude::*;

const SIZES: [(&str, f32); 4] = [("Small", 16.0), ("Medium", 32.0), ("Large", 64.0), ("Huge", 128.0)];

/// The folder's items, by name, dot files left out.
fn listing(folder: &PathBuf) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> =
        std::fs::read_dir(folder).map(|dir| dir.filter_map(|e| Some(e.ok()?.path())).collect()).unwrap_or_default();
    paths.retain(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')));
    paths.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    paths
}

pub fn page() -> impl View {
    let folder = signal(std::env::home_dir().unwrap_or_else(|| PathBuf::from("/")));
    let files = computed(move || listing(&folder.get()));
    let size = signal(1usize);
    let thumbnails = signal(false);
    let side = move || SIZES[size.get()].1;
    let choose = move || {
        spawn_local(async move {
            let chosen = open_file(OpenFile::new().directories().start_folder(folder.get_untracked())).await;
            if let Some(chosen) = chosen.and_then(|paths| paths.into_iter().next()) {
                folder.set(chosen);
            }
        });
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md grow=1.0>
            <Row gap=Spacing::Md align=Align::Center>
                <Text grow=1.0 max_lines=1u32 min_width=0>{move || folder.get().display().to_string()}</Text>
                <Button @click=choose>"Choose Folder…"</Button>
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Select label="Size" options=SIZES.map(|(name, _)| name) bind=size/>
                <Switch bind=thumbnails>"Thumbnails"</Switch>
            </Row>
            <List each=files key=|p: &PathBuf| p.clone() list_style=ListStyle::Framed grow=1.0 min_height=160 let:path>
                <Row padding_x=Spacing::Md padding_y=Spacing::Xs gap=Spacing::Sm align=Align::Center>
                    {FileIcon::new(&path).icon_size(side).thumbnail(thumbnails)}
                    <Text max_lines=1u32>{path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()}</Text>
                </Row>
            </List>
        </Column>
    }
}

fn main() {
    App::new().window("Files' icons", Size::new(520.0, 560.0), page).run();
}
