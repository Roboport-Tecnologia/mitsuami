//! AppKit (macOS) backend for mitsuami.
//!
//! - Containers are flipped `NSView`s that do no layout of their own; the
//!   core positions every child with `setFrame:`.
//! - Leaf widgets are stock AppKit controls, measured through their cells.
//! - Lists are view-based `NSTableView`s whose cells are the core's row hosts.
//! - Escape hatches: [`tweak`] for raw settings of built-in controls,
//!   [`NativeRender`] for custom widgets, [`NativeView`] for any `NSView`,
//!   and a rasterizer for drawn custom widgets.
//! - Control actions and delegate callbacks become [`UiEvent`](mitsuami_core::UiEvent)s.
//! - [`run`] drives the app: a run-loop observer calls [`Ui::tick`](mitsuami_core::Ui::tick)
//!   before the loop sleeps, in all common modes (so live resize relayouts).

#[cfg(target_os = "macos")]
mod app;
#[cfg(target_os = "macos")]
mod backend;
#[cfg(target_os = "macos")]
mod classes;
#[cfg(target_os = "macos")]
mod custom;
#[cfg(target_os = "macos")]
mod list;
#[cfg(target_os = "macos")]
mod number_field;
#[cfg(target_os = "macos")]
mod services;
#[cfg(target_os = "macos")]
mod tweak;

#[cfg(target_os = "macos")]
pub use app::{init_for_tests, run};
#[cfg(target_os = "macos")]
pub use backend::{AppKitBackend, AppKitHandle, BackendOptions};
#[cfg(target_os = "macos")]
pub use custom::{AppKitCx, Emitter, NativeRender, NativeView, ad_hoc, native};
#[cfg(target_os = "macos")]
pub use number_field::NumberField;
#[cfg(target_os = "macos")]
pub use services::AppKitServices;
#[cfg(target_os = "macos")]
pub use tweak::{Tweakable, tweak, tweak_with};

// The bindings native renders and native views are written with, at the
// versions the backend uses.
#[cfg(target_os = "macos")]
pub use {objc2, objc2_app_kit, objc2_foundation};
