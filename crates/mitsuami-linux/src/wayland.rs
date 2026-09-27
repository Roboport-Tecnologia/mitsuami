//! Wayland: the subsurface, the pointer lock and the shortcuts inhibitor,
//! on the toolkit's own connection.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use mitsuami_core::raw_window_handle::{
    HandleError, RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use mitsuami_core::{NativeSurface, SurfaceHandle, SurfaceInput};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_callback, wl_compositor, wl_pointer, wl_region, wl_registry, wl_seat, wl_subcompositor, wl_subsurface,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1 as inhibit_manager, zwp_keyboard_shortcuts_inhibitor_v1 as inhibitor,
};
use wayland_protocols::wp::pointer_constraints::zv1::client::{
    zwp_locked_pointer_v1 as locked_pointer, zwp_pointer_constraints_v1 as constraints,
};
use wayland_protocols::wp::relative_pointer::zv1::client::{
    zwp_relative_pointer_manager_v1 as relative_manager, zwp_relative_pointer_v1 as relative_pointer,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

use crate::{LockEvent, LockSink};

/// A connection to the toolkit's display.
///
/// # Safety
///
/// `display` is the toolkit's live `wl_display`.
unsafe fn connection(display: NonNull<c_void>) -> Connection {
    // SAFETY: the caller's.
    unsafe { Connection::from_backend(Backend::from_foreign_display(display.as_ptr().cast())) }
}

/// A proxy for a surface the toolkit made, on `conn`.
///
/// # Safety
///
/// `surface` is a live `wl_surface` on that connection.
unsafe fn foreign_surface(conn: &Connection, surface: *mut c_void) -> Option<wl_surface::WlSurface> {
    // SAFETY: the caller's.
    let id = unsafe { ObjectId::from_ptr(wl_surface::WlSurface::interface(), surface.cast()) }.ok()?;
    wl_surface::WlSurface::from_id(conn, id).ok()
}

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
        let conn = unsafe { connection(display) };
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
        let Some(parent) = (unsafe { foreign_surface(&self.0.conn, parent) }) else { return };
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

// ------------------------------------------------------------- pointer lock

/// The pointer held on a window's surface (`zwp_locked_pointer_v1`, for as
/// long as the compositor keeps it), its moves reported as relative
/// motion. The compositor locks it once the pointer is over the surface,
/// and ends it as it sees fit (when the window loses focus): that's
/// reported as [`LockEvent::Ended`]. Unlocked when dropped.
///
/// It uses a seat and a pointer of our own: the compositor sends a
/// client's pointer events to each of its pointers.
pub struct PointerLock {
    listener: Listener,
    locked: locked_pointer::ZwpLockedPointerV1,
    relative: relative_pointer::ZwpRelativePointerV1,
    pointer: wl_pointer::WlPointer,
    seat: wl_seat::WlSeat,
}

impl PointerLock {
    /// Locks the pointer on `surface`, the window's surface, whose input
    /// region has the pointer.
    ///
    /// # Safety
    ///
    /// `display` is the toolkit's live `wl_display`, and `surface` a live
    /// `wl_surface` on it.
    pub unsafe fn new(display: *mut c_void, surface: *mut c_void, sink: LockSink) -> Result<PointerLock, String> {
        let display = NonNull::new(display).ok_or("no wl_display")?;
        // SAFETY: the caller's.
        let conn = unsafe { connection(display) };
        let (globals, queue) = registry_queue_init::<Watch>(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let seat = bind_seat(&globals, &qh)?;
        let constraints: constraints::ZwpPointerConstraintsV1 =
            globals.bind(&qh, 1..=1, ()).map_err(|e| format!("zwp_pointer_constraints_v1: {e}"))?;
        let manager: relative_manager::ZwpRelativePointerManagerV1 =
            globals.bind(&qh, 1..=1, ()).map_err(|e| format!("zwp_relative_pointer_manager_v1: {e}"))?;
        // SAFETY: the caller's.
        let surface = unsafe { foreign_surface(&conn, surface) }.ok_or("no wl_surface")?;
        let pointer = seat.get_pointer(&qh, ());
        let relative = manager.get_relative_pointer(&pointer, &qh, ());
        let locked = constraints.lock_pointer(&surface, &pointer, None, constraints::Lifetime::Oneshot, &qh, ());
        manager.destroy();
        constraints.destroy();
        conn.flush().map_err(|e| e.to_string())?;
        let listener = Listener::start(conn, queue, sink);
        Ok(PointerLock { listener, locked, relative, pointer, seat })
    }
}

impl Drop for PointerLock {
    fn drop(&mut self) {
        self.locked.destroy();
        self.relative.destroy();
        if self.pointer.version() >= 3 {
            self.pointer.release();
        }
        release_seat(&self.seat);
        self.listener.stop();
    }
}

// ---------------------------------------------------- shortcuts inhibitor

/// The compositor's own shortcuts sent to a window as keys
/// (`zwp_keyboard_shortcuts_inhibitor_v1`), while it has keyboard focus.
/// The compositor may end it (it keeps a shortcut of its own to): that's
/// reported as [`LockEvent::Ended`]. For toolkits without an API for it
/// (Qt); GTK has `gdk_toplevel_inhibit_system_shortcuts`.
pub struct ShortcutsInhibitor {
    listener: Listener,
    inhibitor: inhibitor::ZwpKeyboardShortcutsInhibitorV1,
    seat: wl_seat::WlSeat,
}

impl ShortcutsInhibitor {
    /// # Safety
    ///
    /// `display` is the toolkit's live `wl_display`, and `surface` a live
    /// `wl_surface` on it.
    pub unsafe fn new(
        display: *mut c_void,
        surface: *mut c_void,
        sink: LockSink,
    ) -> Result<ShortcutsInhibitor, String> {
        let display = NonNull::new(display).ok_or("no wl_display")?;
        // SAFETY: the caller's.
        let conn = unsafe { connection(display) };
        let (globals, queue) = registry_queue_init::<Watch>(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let seat = bind_seat(&globals, &qh)?;
        let manager: inhibit_manager::ZwpKeyboardShortcutsInhibitManagerV1 =
            globals.bind(&qh, 1..=1, ()).map_err(|e| format!("zwp_keyboard_shortcuts_inhibit_manager_v1: {e}"))?;
        // SAFETY: the caller's.
        let surface = unsafe { foreign_surface(&conn, surface) }.ok_or("no wl_surface")?;
        let inhibitor = manager.inhibit_shortcuts(&surface, &seat, &qh, ());
        manager.destroy();
        conn.flush().map_err(|e| e.to_string())?;
        let listener = Listener::start(conn, queue, sink);
        Ok(ShortcutsInhibitor { listener, inhibitor, seat })
    }
}

impl Drop for ShortcutsInhibitor {
    fn drop(&mut self) {
        self.inhibitor.destroy();
        release_seat(&self.seat);
        self.listener.stop();
    }
}

/// The first seat: desktops have one.
fn bind_seat(globals: &GlobalList, qh: &QueueHandle<Watch>) -> Result<wl_seat::WlSeat, String> {
    globals.bind(qh, 1..=5, ()).map_err(|e| format!("wl_seat: {e}"))
}

fn release_seat(seat: &wl_seat::WlSeat) {
    if seat.version() >= 5 {
        seat.release();
    }
}

/// Dispatches a lock's queue on a thread of its own, as the toolkit reads
/// the connection (libwayland lets several threads read it), until it's
/// stopped.
struct Listener {
    conn: Connection,
    qh: QueueHandle<Watch>,
    stopped: Arc<AtomicBool>,
}

/// What a lock's thread keeps.
struct Watch {
    sink: LockSink,
    stopped: Arc<AtomicBool>,
    locked: bool,
}

impl Watch {
    fn report(&self, event: LockEvent) {
        if !self.stopped.load(Ordering::Acquire) {
            (self.sink)(event);
        }
    }
}

impl Listener {
    fn start(conn: Connection, mut queue: EventQueue<Watch>, sink: LockSink) -> Listener {
        let stopped = Arc::new(AtomicBool::new(false));
        let qh = queue.handle();
        let mut watch = Watch { sink, stopped: stopped.clone(), locked: false };
        let spawned = std::thread::Builder::new().name("mitsuami-wayland-lock".into()).spawn(move || {
            while !watch.stopped.load(Ordering::Acquire) {
                if queue.blocking_dispatch(&mut watch).is_err() {
                    break;
                }
            }
        });
        if let Err(problem) = spawned {
            eprintln!("mitsuami: no thread for a pointer lock: {problem}");
        }
        Listener { conn, qh, stopped }
    }

    /// Stops reporting, and wakes the thread (a roundtrip's reply) so it
    /// sees it and ends.
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.conn.display().sync(&self.qh, ());
        let _ = self.conn.flush();
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Watch {
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

impl Dispatch<locked_pointer::ZwpLockedPointerV1, ()> for Watch {
    fn event(
        watch: &mut Self,
        _: &locked_pointer::ZwpLockedPointerV1,
        event: locked_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            locked_pointer::Event::Locked => watch.locked = true,
            // A one-shot lock, once let go, is gone.
            locked_pointer::Event::Unlocked => {
                watch.locked = false;
                watch.report(LockEvent::Ended);
            }
            _ => {}
        }
    }
}

impl Dispatch<relative_pointer::ZwpRelativePointerV1, ()> for Watch {
    fn event(
        watch: &mut Self,
        _: &relative_pointer::ZwpRelativePointerV1,
        event: relative_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Accelerated, in the surface's logical pixels; reported whenever
        // the pointer moves over the app's surfaces, locked or not.
        if let relative_pointer::Event::RelativeMotion { dx, dy, .. } = event
            && watch.locked
        {
            watch.report(LockEvent::Input(SurfaceInput::Motion { dx: dx as f32, dy: dy as f32 }));
        }
    }
}

impl Dispatch<inhibitor::ZwpKeyboardShortcutsInhibitorV1, ()> for Watch {
    fn event(
        watch: &mut Self,
        _: &inhibitor::ZwpKeyboardShortcutsInhibitorV1,
        event: inhibitor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let inhibitor::Event::Inactive = event {
            watch.report(LockEvent::Ended);
        }
    }
}

delegate_noop!(Watch: ignore wl_seat::WlSeat);
delegate_noop!(Watch: ignore wl_pointer::WlPointer);
delegate_noop!(Watch: ignore wl_callback::WlCallback);
delegate_noop!(Watch: ignore wl_surface::WlSurface);
delegate_noop!(Watch: constraints::ZwpPointerConstraintsV1);
delegate_noop!(Watch: relative_manager::ZwpRelativePointerManagerV1);
delegate_noop!(Watch: inhibit_manager::ZwpKeyboardShortcutsInhibitManagerV1);
