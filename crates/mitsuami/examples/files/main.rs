//! A file browser: `cargo run -p mitsuami --example files`.
//!
//! A simplified Finder, on the real file system:
//!
//! - The sidebar's places (home, Desktop, Documents…, the disk) choose the
//!   folder; it follows along as you go elsewhere, choosing none.
//! - The folder's items are in the platform's own list: select several,
//!   double-click (or Return) a folder to go in, a file to open it in its
//!   app. The column titles sort; again, the other way round.
//! - The toolbar goes back and forward, makes folders, has the view
//!   options (sort, hidden files) and searches the folder as you type.
//! - Right-click an item to open, rename, duplicate, trash it, or copy its
//!   path; right-click the list's background to make a folder.
//! - Drop files from another file manager on the list: they're copied in.
//!   Drag items out to the file manager, Mail or another app: a copy.
//! - The preview shows the selection: a picture, a text file's first
//!   lines, a folder's item count.
//! - The path bar along the bottom goes to any folder above this one:
//!   `NSPathControl` on macOS, `BreadcrumbBar` on Windows, and buttons as
//!   Nautilus and Dolphin have them elsewhere (`path_bar/`).
//! - Items show the icons the platform's file manager gives them, and the
//!   preview a document's thumbnail (mitsuami's `FileIcon`). Trashing puts
//!   them in the platform's trash, and opening asks the platform
//!   (mitsuami's `trash` and `launch` services).
//! - The window's menus have it all again, with the platform's file
//!   manager's shortcuts (⌘⌫ and ⌘↑ in Finder; Delete, F2 and Alt+↑
//!   elsewhere), and Go to Folder… (⌘⇧G, Ctrl+Shift+G) for a typed path.
//! - Space on the list shows or hides the preview on macOS, where Quick
//!   Look is on Space.
//!
//! Reading folders and changing them happen off the UI thread.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod browser;
mod fs;
mod path_bar;
mod screen;

use mitsuami::prelude::*;

use browser::Browser;

fn main() {
    App::new()
        .id("org.mitsuami.Files")
        .name("Files")
        .open(|| {
            let browser = use_store::<Browser>();
            Window::new(move || browser.title())
                .size(Size::new(900.0, 560.0))
                .min_size(Size::new(560.0, 320.0))
                .bind(signal(true))
                .content(screen::Finder::new)
        })
        .run();
}
