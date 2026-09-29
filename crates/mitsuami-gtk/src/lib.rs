//! GTK 4 (Linux) backend for mitsuami.
//!
//! - Containers are a small widget subclass that does no layout of its own;
//!   it allocates every child at the frame the core computed.
//! - Leaf widgets are stock GTK widgets, measured with `gtk_widget_measure`.
//! - Lists are `gtk::ListView`s whose items hold the core's row hosts.
//! - Escape hatches: [`tweak`] for raw settings of built-in widgets,
//!   [`NativeRender`] for custom widgets, [`NativeView`] for any GTK widget,
//!   and a Cairo rasterizer for drawn custom widgets.
//! - Signals become [`UiEvent`](mitsuami_core::UiEvent)s.
//! - [`run`] drives the app: a GLib idle source calls [`Ui::tick`](mitsuami_core::Ui::tick)
//!   whenever there is work, ahead of GTK's own layout and drawing.
//! - Services: the GDK clipboard, `AlertDialog`, `FileDialog`, and the app's
//!   menus in a header bar menu button.

#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod backend;
#[cfg(target_os = "linux")]
mod custom;
#[cfg(target_os = "linux")]
mod display;
#[cfg(target_os = "linux")]
mod file_drop;
#[cfg(target_os = "linux")]
mod group;
#[cfg(target_os = "linux")]
mod host;
#[cfg(target_os = "linux")]
mod list;
#[cfg(target_os = "linux")]
mod radio;
#[cfg(target_os = "linux")]
mod services;
#[cfg(target_os = "linux")]
mod session;
#[cfg(target_os = "linux")]
mod sidebar;
#[cfg(target_os = "linux")]
mod surface;
#[cfg(target_os = "linux")]
mod tabs;
#[cfg(target_os = "linux")]
mod tweak;

#[cfg(target_os = "linux")]
pub use app::{init_for_tests, run};
#[cfg(target_os = "linux")]
pub use backend::{BackendOptions, GtkBackend, GtkHandle};
#[cfg(target_os = "linux")]
pub use custom::{Emitter, GtkCx, NativeRender, NativeView, ad_hoc, native};
#[cfg(target_os = "linux")]
pub use services::GtkServices;
#[cfg(target_os = "linux")]
pub use tweak::{Tweakable, show_step_marks, tweak, tweak_with};

// The bindings native renders and native views are written with, at the
// version the backend uses.
#[cfg(target_os = "linux")]
pub use gtk;
