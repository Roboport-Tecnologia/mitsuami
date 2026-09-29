//! Win32 (Windows) backend for mitsuami: the classic common controls, the
//! ones Windows' own control panels and dialogs are made of.
//!
//! - Every node is a window (`HWND`). Containers are child windows of our
//!   own class that do no layout; the core places every child with
//!   `SetWindowPos`, in pixels at the window's DPI.
//! - Controls are the system's classes (`BUTTON`, `COMBOBOX`,
//!   `msctls_trackbar32`, `msctls_progress32`, `STATIC`), themed: the
//!   backend activates the common controls 6 at run time, as an app's
//!   manifest would.
//! - Window procedures only queue events; a thread-local registry maps a
//!   control's `HWND` to its node.
//! - Only some widgets are native so far (windows, containers, text,
//!   buttons, checkboxes, sliders, selects and progress bars). The others
//!   are empty hosts that keep their props, so an app still runs.
//! - [`run`] owns the message loop, and ticks the UI before it sleeps.
//!
//! Chosen with `mitsuami`'s `win32` feature, in place of WinUI 3.

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod keys;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod runtime;
#[cfg(windows)]
mod services;
#[cfg(windows)]
mod tweak;

#[cfg(windows)]
pub use app::{init_for_tests, pump, run};
#[cfg(windows)]
pub use backend::{BackendOptions, Win32Backend, Win32Handle};
#[cfg(windows)]
pub use services::Win32Services;
#[cfg(windows)]
pub use tweak::{CheckBox, ComboBox, Control, ProgressBar, PushButton, Static, Trackbar, Tweakable, tweak, tweak_with};

/// The bindings tweaks are written with, at the version the backend uses.
#[cfg(windows)]
pub use windows_sys;
