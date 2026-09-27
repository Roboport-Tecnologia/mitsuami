//! Qt Quick and Kirigami (KDE Plasma) backend for mitsuami.
//!
//! - Windows are `Kirigami.ApplicationWindow`s with one page, whose title
//!   shows in Kirigami's toolbar. The page's content is a plain item that
//!   does no layout: children sit at the frames the core computed.
//! - Leaves are Qt Quick Controls in the `org.kde.desktop` style (Breeze on
//!   Plasma), created from QML and measured by their implicit sizes.
//! - Escape hatches: [`tweak`] for raw settings of built-in controls,
//!   [`NativeRender`] for custom widgets, [`NativeView`] for any QML item,
//!   and a `QPainter` item for drawn custom widgets.
//! - Signals become [`UiEvent`](mitsuami_core::UiEvent)s.
//! - [`run`] drives the app: the UI ticks whenever Qt's event loop is about
//!   to sleep.
//! - Services: Qt's clipboard, `Kirigami.PromptDialog`, Qt Quick's file
//!   dialogs, and the app's menus in a Kirigami global drawer.
//!
//! The backend is built with the `qt` feature (which `mitsuami`'s `kde`
//! feature turns on); without it this crate is empty, so workspaces build
//! where Qt isn't installed. It links Qt 6.5 or newer, and needs Kirigami
//! and `qqc2-desktop-style` at run time.

#[cfg(all(target_os = "linux", feature = "qt"))]
mod app;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod backend;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod custom;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod events;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod ffi;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod list;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod qml;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod services;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod theme;
#[cfg(all(target_os = "linux", feature = "qt"))]
mod tweak;

#[cfg(all(target_os = "linux", feature = "qt"))]
pub use app::{init_for_tests, run};
#[cfg(all(target_os = "linux", feature = "qt"))]
pub use backend::{BackendOptions, KirigamiBackend, KirigamiHandle};
#[cfg(all(target_os = "linux", feature = "qt"))]
pub use custom::{Emitter, KirigamiCx, NativeRender, NativeView, ad_hoc, native};
#[cfg(all(target_os = "linux", feature = "qt"))]
pub use ffi::{IMPORTS, QmlObject};
#[cfg(all(target_os = "linux", feature = "qt"))]
pub use services::KirigamiServices;
#[cfg(all(target_os = "linux", feature = "qt"))]
pub use tweak::{Tweakable, tweak, tweak_with};
