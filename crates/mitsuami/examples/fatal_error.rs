//! When the app can't run: `cargo run -p mitsuami --example fatal_error`.
//!
//! - The example is a viewer that needs a file to show, given as its
//!   argument. Without one, or with one that isn't there, it can't start:
//!   it says why in the platform's error dialog (`fatal_error`), before any
//!   window and without `App::run`, and exits once it's dismissed.
//! - `cargo run -p mitsuami --example fatal_error -- Cargo.toml` starts it:
//!   the window shows the file's first lines.
//! - The message goes to stderr too, as it should in any app: launched from
//!   a terminal, that's where it's read; from the desktop, the dialog is.
//!   Started without a display (GTK), there's no dialog, only stderr.
//! - Build it with `--release` to run it as a Windows GUI app, with no
//!   console: launched from Explorer, the dialog is all there is.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mitsuami::prelude::*;

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let text = match &path {
        None => Err("Give it the file to show: fatal_error <file>.".to_owned()),
        Some(path) => std::fs::read_to_string(path).map_err(|error| format!("{}: {error}.", path.display())),
    };
    let text = match text {
        Ok(text) => text,
        Err(message) => {
            eprintln!("The viewer can't start: {message}");
            fatal_error("The viewer can't start", &message);
            std::process::exit(1);
        }
    };
    let first_lines: String = text.lines().take(12).collect::<Vec<_>>().join("\n");
    let title = path.and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_default();
    App::new()
        .name("Viewer")
        .window(title, Size::new(480.0, 280.0), move || {
            Column::new().padding(16.px()).child(Text::new(first_lines.clone()).selectable(true))
        })
        .run();
}
