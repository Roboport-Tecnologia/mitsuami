use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use mitsuami_core::raw_window_handle::{
    HandleError, RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use mitsuami_core::{NativeSurface, SurfaceHandle};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_compositor, wl_region, wl_registry, wl_subcompositor, wl_subsurface, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

/// A `wl_surface` of our own, and while shown the subsurface role that
/// puts it over a window. Clones share it. Wayland proxies may be used from
/// any thread; the backend uses it from its UI thread.
#[derive(Clone)]
pub struct Subsurface(Arc<Inner>);

struct Globals;

struct Inner {
    conn: Connection,
    /// The toolkit's `wl_display`, which the app's GPU API connects through.
    display: NonNull<c_void>,
    queue: Mutex<EventQueue<Globals>>,
    subcompositor: wl_subcompositor::WlSubcompositor,
    surface: wl_surface::WlSurface,
    viewport: wp_viewport::WpViewport,
    role: Mutex<Option<wl_subsurface::WlSubsurface>>,
    /// Where it was put, in the window surface's logical coordinates.
    placed: Mutex<Option<(i32, i32, i32, i32)>>,
}

// SAFETY: the display pointer is only handed out, never dereferenced here;
// `wl_display` connections are thread-safe.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl Subsurface {
    /// A surface on the toolkit's connection, not shown yet.
    ///
    /// # Safety
    ///
    /// `display` is the toolkit's live `wl_display`, which stays connected
    /// as long as the process uses it.
    pub unsafe fn new(display: *mut c_void) -> Result<Subsurface, String> {
        let display = NonNull::new(display).ok_or("no wl_display")?;
        // SAFETY: the caller's.
        let conn = unsafe { Connection::from_backend(Backend::from_foreign_display(display.as_ptr().cast())) };
        let (globals, queue) = registry_queue_init::<Globals>(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let compositor: wl_compositor::WlCompositor =
            globals.bind(&qh, 1..=4, ()).map_err(|e| format!("wl_compositor: {e}"))?;
        let subcompositor: wl_subcompositor::WlSubcompositor =
            globals.bind(&qh, 1..=1, ()).map_err(|e| format!("wl_subcompositor: {e}"))?;
        let viewporter: wp_viewporter::WpViewporter =
            globals.bind(&qh, 1..=1, ()).map_err(|e| format!("wp_viewporter: {e}"))?;
        let surface = compositor.create_surface(&qh, ());
        let region = compositor.create_region(&qh, ());
        surface.set_input_region(Some(&region));
        region.destroy();
        let viewport = viewporter.get_viewport(&surface, &qh, ());
        conn.flush().map_err(|e| e.to_string())?;
        Ok(Subsurface(Arc::new(Inner {
            conn,
            display,
            queue: Mutex::new(queue),
            subcompositor,
            surface,
            viewport,
            role: Mutex::new(None),
            placed: Mutex::new(None),
        })))
    }

    /// What the app gets: it keeps the surface alive.
    pub fn handle(&self) -> SurfaceHandle {
        SurfaceHandle::new(self.clone())
    }

    /// Puts the surface over `parent`, desync, so its commits show without
    /// waiting for the window's. A window's surface is new each time the
    /// window is mapped, so it's given each time.
    ///
    /// # Safety
    ///
    /// `parent` is a live `wl_surface` on the same connection.
    pub unsafe fn attach(&self, parent: *mut c_void) {
        self.detach();
        // SAFETY: the caller's.
        let Ok(parent) = (unsafe { ObjectId::from_ptr(wl_surface::WlSurface::interface(), parent.cast()) }) else {
            return;
        };
        let Ok(parent) = wl_surface::WlSurface::from_id(&self.0.conn, parent) else { return };
        let queue = self.0.queue.lock().unwrap();
        let role = self.0.subcompositor.get_subsurface(&self.0.surface, &parent, &queue.handle(), ());
        role.set_desync();
        *self.0.role.lock().unwrap() = Some(role);
        let _ = self.0.conn.flush();
    }

    /// Takes the surface off its window: without its role, it isn't shown.
    pub fn detach(&self) {
        *self.0.placed.lock().unwrap() = None;
        if let Some(role) = self.0.role.lock().unwrap().take() {
            role.destroy();
            let _ = self.0.conn.flush();
        }
    }

    /// Where it sits in the window's surface, and its size, in logical
    /// pixels. Returns whether that changed: a move takes effect with the
    /// window's next commit, so the backend then asks for a frame.
    pub fn place(&self, x: i32, y: i32, width: i32, height: i32) -> bool {
        // Events for our objects (outputs entered) are filed under our
        // queue as the toolkit reads the socket; nothing needs them.
        let _ = self.0.queue.lock().unwrap().dispatch_pending(&mut Globals);
        let role = self.0.role.lock().unwrap();
        let (Some(role), true) = (&*role, width > 0 && height > 0) else { return false };
        let mut placed = self.0.placed.lock().unwrap();
        if *placed == Some((x, y, width, height)) {
            return false;
        }
        *placed = Some((x, y, width, height));
        role.set_position(x, y);
        self.0.viewport.set_destination(width, height);
        let _ = self.0.conn.flush();
        true
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(role) = self.role.get_mut().unwrap().take() {
            role.destroy();
        }
        self.viewport.destroy();
        self.surface.destroy();
        let _ = self.conn.flush();
    }
}

impl NativeSurface for Subsurface {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError> {
        let surface = NonNull::new(self.0.surface.id().as_ptr().cast()).ok_or(HandleError::Unavailable)?;
        Ok(RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)))
    }

    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError> {
        Ok(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(self.0.display)))
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Globals {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(Globals: ignore wl_compositor::WlCompositor);
delegate_noop!(Globals: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(Globals: ignore wl_surface::WlSurface);
delegate_noop!(Globals: ignore wl_subsurface::WlSubsurface);
delegate_noop!(Globals: ignore wl_region::WlRegion);
delegate_noop!(Globals: ignore wp_viewporter::WpViewporter);
delegate_noop!(Globals: ignore wp_viewport::WpViewport);
