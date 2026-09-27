//! `GpuSurface`: a widget that keeps the space, and on Wayland a desync
//! subsurface of the window's surface over it (`mitsuami-wayland`), which
//! the app presents to. It's placed after each of the window's frames, so
//! it follows the widget wherever layout or scrolling moves it.

use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use gtk::{gdk, glib, graphene};
use mitsuami_core::{NodeId, SurfaceHandle, SurfaceSize, UiEvent};
use mitsuami_wayland::Subsurface;

use crate::host::Events;

unsafe extern "C" {
    fn gdk_wayland_display_get_wl_display(display: *mut gdk::ffi::GdkDisplay) -> *mut c_void;
    fn gdk_wayland_surface_get_wl_surface(surface: *mut gdk::ffi::GdkSurface) -> *mut c_void;
}

/// The widget that keeps the surface's space, and what it reports.
pub(crate) struct SurfaceArea {
    pub(crate) area: gtk::DrawingArea,
    state: Rc<RefCell<AreaState>>,
}

struct AreaState {
    id: NodeId,
    events: Events,
    /// Made when the widget is first mapped in a Wayland window.
    subsurface: Option<Subsurface>,
    /// Given up when the node is destroyed; the app may keep its own.
    handle: Option<SurfaceHandle>,
    after_paint: Option<(gdk::FrameClock, glib::SignalHandlerId)>,
}

impl SurfaceArea {
    pub(crate) fn new(id: NodeId, events: Events) -> SurfaceArea {
        let area = gtk::DrawingArea::new();
        area.set_accessible_role(gtk::AccessibleRole::Img);
        let state = Rc::new(RefCell::new(AreaState { id, events, subsurface: None, handle: None, after_paint: None }));
        let s = state.clone();
        area.connect_map(move |area| AreaState::mapped(&s, area));
        let s = state.clone();
        area.connect_unmap(move |_| s.borrow_mut().unmapped());
        SurfaceArea { area, state }
    }

    /// The node is gone: stop showing and reporting.
    pub(crate) fn detach(&self) {
        let mut state = self.state.borrow_mut();
        state.unmapped();
        state.handle = None;
    }
}

impl AreaState {
    fn mapped(this: &Rc<RefCell<AreaState>>, area: &gtk::DrawingArea) {
        let mut state = this.borrow_mut();
        let Some(parent) = window_surface(area) else {
            // X11 has no route yet.
            glib::g_warning!("mitsuami", "GpuSurface needs a Wayland window");
            return;
        };
        if state.subsurface.is_none() {
            let display = area.display();
            // SAFETY: a Wayland window's display is a Wayland display, and
            // stays connected.
            let made = unsafe { Subsurface::new(gdk_wayland_display_get_wl_display(display.to_glib_none().0)) };
            match made {
                Ok(subsurface) => {
                    let handle = subsurface.handle();
                    state.subsurface = Some(subsurface);
                    state.handle = Some(handle.clone());
                    state.events.emit(state.id, UiEvent::SurfaceReady(handle));
                }
                Err(problem) => {
                    glib::g_warning!("mitsuami", "no GPU surface: {problem}");
                    return;
                }
            }
        }
        // SAFETY: the window's live surface, on GDK's connection.
        unsafe { state.subsurface.as_ref().unwrap().attach(parent) };
        if let Some(clock) = area.frame_clock() {
            let (s, a) = (Rc::downgrade(this), area.downgrade());
            let handler = clock.connect_after_paint(move |_| {
                if let (Some(s), Some(a)) = (s.upgrade(), a.upgrade()) {
                    s.borrow().place(&a);
                }
            });
            state.after_paint = Some((clock, handler));
        }
        state.place(area);
    }

    fn unmapped(&mut self) {
        if let Some((clock, handler)) = self.after_paint.take() {
            clock.disconnect(handler);
        }
        if let Some(subsurface) = &self.subsurface {
            subsurface.detach();
        }
    }

    /// Puts the subsurface over the widget, and reports its size in pixels.
    fn place(&self, area: &gtk::DrawingArea) {
        let (Some(subsurface), Some(handle)) = (&self.subsurface, &self.handle) else { return };
        let (Some(native), Some(root)) = (area.native(), area.root()) else { return };
        let Some(p) = area.compute_point(&root, &graphene::Point::new(0.0, 0.0)) else { return };
        let (tx, ty) = native.surface_transform();
        let (width, height) = (area.width(), area.height());
        let scale = area.scale_factor();
        let size = SurfaceSize { width: (width * scale) as u32, height: (height * scale) as u32, scale: scale as f32 };
        if !size.is_empty() && handle.set_size(size) {
            self.events.emit(self.id, UiEvent::SurfaceResized(size));
        }
        if subsurface.place((p.x() as f64 + tx) as i32, (p.y() as f64 + ty) as i32, width, height) {
            area.queue_draw();
        }
    }
}

/// The window's own `wl_surface`, if it's a Wayland window.
fn window_surface(widget: &impl IsA<gtk::Widget>) -> Option<*mut c_void> {
    let surface = widget.native()?.surface()?;
    if surface.display().type_().name() != "GdkWaylandDisplay" {
        return None;
    }
    // SAFETY: a Wayland display's surfaces are Wayland surfaces.
    let parent = unsafe { gdk_wayland_surface_get_wl_surface(surface.to_glib_none().0) };
    (!parent.is_null()).then_some(parent)
}
