//! A virtualised list: `cargo run -p mitsuami --example contacts`.
//!
//! Ten thousand contacts in the platform's own list control, which decides
//! which rows to show; only those are built, and the rest are just keys.
//! The search field filters them, the selection survives filtering while
//! its row stays, and activating a row (double-click or Return) opens it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod screen;

use mitsuami::prelude::*;

fn main() {
    App::new().window("mitsuami contacts", Size::new(720.0, 520.0), screen::Contacts::new).run();
}
