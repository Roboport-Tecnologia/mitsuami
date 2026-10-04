//! What a `GpuSurface` hands the app: a native surface to present to with
//! its own GPU API, from any thread.

use std::fmt;
use std::sync::{Arc, Mutex};

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
};

/// A backend's native surface: what `raw-window-handle` describes for a
/// GPU API to present to (an `NSView` backed by a `CAMetalLayer`, a
/// Wayland surface, a child window).
///
/// It lives as long as any [`SurfaceHandle`] to it, even after its widget
/// is gone, so a GPU surface made on it never outlives it. It may be
/// dropped on any thread: a backend whose native objects must be freed on
/// the UI thread sends them there.
///
/// # Safety
///
/// The handles it returns stay valid for as long as it lives:
/// [`SurfaceHandle`] lends them to GPU APIs on that promise.
pub unsafe trait NativeSurface: Send + Sync + 'static {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError>;
    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError>;
}

/// How large a `GpuSurface` is, in the physical pixels a GPU surface on it
/// should be configured with, and the scale they are at (pixels to a
/// point).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

impl SurfaceSize {
    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// A `GpuSurface`'s native surface, for the app to present to with its own
/// GPU API (wgpu, Vulkan, Metal, Direct3D), at its own pace and on its own
/// thread. It implements `raw-window-handle`'s traits, so wgpu takes it as
/// is: `instance.create_surface(handle.clone())`.
///
/// Clones share the surface, which lives until the last one is dropped.
/// [`size`](SurfaceHandle::size) is kept current by the backend, so a
/// render thread can read it before each frame.
#[derive(Clone)]
pub struct SurfaceHandle {
    native: Arc<dyn NativeSurface>,
    size: Arc<Mutex<SurfaceSize>>,
}

impl SurfaceHandle {
    /// For backends.
    pub fn new(native: impl NativeSurface) -> SurfaceHandle {
        SurfaceHandle { native: Arc::new(native), size: Arc::default() }
    }

    /// Its size now, in physical pixels.
    pub fn size(&self) -> SurfaceSize {
        *self.size.lock().unwrap()
    }

    /// For backends: the surface's new size. They report it as
    /// [`UiEvent::SurfaceResized`](crate::UiEvent::SurfaceResized) too.
    /// Returns whether it changed.
    pub fn set_size(&self, size: SurfaceSize) -> bool {
        let mut current = self.size.lock().unwrap();
        let changed = *current != size;
        *current = size;
        changed
    }
}

/// Compared by identity: two handles are equal if they share a surface.
impl PartialEq for SurfaceHandle {
    fn eq(&self, other: &SurfaceHandle) -> bool {
        Arc::ptr_eq(&self.size, &other.size)
    }
}

impl fmt::Debug for SurfaceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let SurfaceSize { width, height, scale } = self.size();
        write!(f, "SurfaceHandle({width}×{height} @{scale}x)")
    }
}

impl HasWindowHandle for SurfaceHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: the native surface lives as long as `self`.
        self.native.window_handle().map(|raw| unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for SurfaceHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: the native surface keeps its display connection open.
        self.native.display_handle().map(|raw| unsafe { DisplayHandle::borrow_raw(raw) })
    }
}
