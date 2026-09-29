//! The showcase for components and stores:
//! `cargo run -p mitsuami --example todos`.
//!
//! A todo list written with `view!` and `#[component]`:
//!
//! - `Todos` (`store.rs`) is a `Store`: the app's one instance, which every
//!   component takes with `use_store`.
//! - The screen (`screen.rs`) is made of components with required,
//!   optional, reactive, callback and children props, and uses `Show` and
//!   keyed `For`.
//! - The quote loads with a `resource` (loading, error, refetch), and the
//!   sync button runs an `action` (pending, latest value).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod screen;
mod store;

use mitsuami::prelude::*;

fn main() {
    App::new()
        .window("mitsuami todos", Size::new(400.0, 680.0), || {
            view! {
                <Column padding=Spacing::Xl grow=1.0>
                    <screen::Screen/>
                </Column>
            }
        })
        .run();
}
