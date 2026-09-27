//! `GpuSurface` on Wayland, for the Linux backends (GTK, Kirigami): a
//! `wl_surface` of our own, a desync subsurface of the window's surface,
//! over the widget that keeps its space. The app presents to it directly.
//!
//! The toolkits' own routes wait for their frame clocks: GTK's
//! `GtkGraphicsOffload` showed a frame more latency than a window of the
//! app's own (2ksbox measured it); a desync subsurface doesn't wait for the
//! window's commits. It takes no input (an empty input region), so the
//! toolkit keeps the pointer, and the backend places it over its widget
//! after each of the window's frames.
//!
//! It lives as long as the app's handle, so a GPU surface made on it never
//! outlives it; without its subsurface role it isn't shown.

#[cfg(target_os = "linux")]
mod subsurface;

#[cfg(target_os = "linux")]
pub use subsurface::Subsurface;
