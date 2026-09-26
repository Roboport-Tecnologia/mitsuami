//! A virtualised list: `cargo run -p mitsuami --example contacts`.
//!
//! Ten thousand contacts in the platform's own list control. Only the rows
//! in view (and a screenful either side) exist; the rest are keys and
//! heights. The search field filters them, the selection survives
//! filtering while its row stays, and activating a row (double-click or
//! Return) opens it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod screen;

use mitsuami::prelude::*;

fn main() {
    App::new().window("mitsuami contacts", Size::new(720.0, 520.0), screen::Contacts::new).run();
}
