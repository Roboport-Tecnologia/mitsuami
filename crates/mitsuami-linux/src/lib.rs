//! `GpuSurface` on Linux, for the GTK and Kirigami backends: a surface of
//! our own over the widget that keeps its space, which the app presents to
//! directly, and the pointer lock the toolkits don't offer.
//!
//! - Wayland ([`wayland`]): a `wl_surface`, a desync subsurface of the
//!   window's surface, on the toolkit's connection. The toolkits' own
//!   routes wait for their frame clocks: GTK's `GtkGraphicsOffload` showed
//!   a frame more latency than a window of the app's own (2ksbox measured
//!   it); a desync subsurface doesn't wait for the window's commits. The
//!   pointer lock is `zwp_locked_pointer_v1` with relative motion, and Qt,
//!   which has no API for it, inhibits the compositor's shortcuts with
//!   `zwp_keyboard_shortcuts_inhibitor_v1`.
//! - X11 ([`x11`]): a child window of the window's, on an xcb connection of
//!   our own, which the app may use from any thread. The pointer lock is a
//!   pointer grab on it, the cursor warped back to its middle after each
//!   move.
//!
//! Both take no input (an empty input region, an empty input shape), so
//! the toolkit keeps the pointer, and the backend places them over their
//! widget after each of the window's frames. Each lives as long as the
//! app's handle, so a GPU surface made on it never outlives it. Other
//! displays (GTK's Broadway, Qt's offscreen platform) get a [`NoSurface`].
//!
//! Pointer locks report on a thread of their own ([`LockEvent`]); the
//! backend sends what they report to its UI thread.

#[cfg(target_os = "linux")]
pub mod wayland;
#[cfg(target_os = "linux")]
pub mod x11;

use mitsuami_core::raw_window_handle::{HandleError, RawDisplayHandle, RawWindowHandle};
use mitsuami_core::{NativeSurface, SurfaceHandle, SurfaceInput};

/// A surface on a display that has none to give (GTK's Broadway, Qt's
/// offscreen platform, which tests run on): the app is told of it and its
/// size, as headless tells it, but it has no handles to present to.
pub struct NoSurface;

impl NoSurface {
    pub fn handle() -> SurfaceHandle {
        SurfaceHandle::new(NoSurface)
    }
}

// SAFETY: it returns no handles.
unsafe impl NativeSurface for NoSurface {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError> {
        Err(HandleError::NotSupported)
    }

    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError> {
        Err(HandleError::NotSupported)
    }
}

/// What a pointer lock or a shortcuts inhibitor reports, from a thread of
/// its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockEvent {
    /// Input for the surface: the pointer's moves while locked (and on
    /// X11, whose grab takes them from the toolkit, its buttons and
    /// wheel).
    Input(SurfaceInput),
    /// The compositor or X server ended it, or it couldn't be made.
    Ended,
}

/// Where a lock reports, on its own thread.
pub type LockSink = Box<dyn Fn(LockEvent) + Send + Sync>;
