//! `GpuSurface`: an item that keeps the space, and on Wayland a desync
//! subsurface of its window's surface over it (`mitsuami-wayland`), which
//! the app presents to. Qt's own routes (a `QQuickRhiItem`, a child
//! `QWindow`) wait for the scene graph, or are destroyed and made again
//! with their window; this one lives as long as the app's handle. It's
//! placed after each of the window's frames, so it follows the item
//! wherever layout or scrolling moves it.

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};

use mitsuami_core::{NodeId, SurfaceHandle, SurfaceSize, UiEvent};
use mitsuami_wayland::Subsurface;

use crate::events::Events;
use crate::ffi::{self, QmlObject};
use crate::qml;

/// The item that keeps the surface's space, and what it reports.
pub(crate) struct SurfaceItem {
    pub(crate) item: QmlObject,
    state: Rc<RefCell<ItemState>>,
}

struct ItemState {
    id: NodeId,
    events: Events,
    item: QmlObject,
    /// Made when the item is first shown in a Wayland window.
    subsurface: Option<Subsurface>,
    /// Given up when the node is destroyed; the app may keep its own.
    handle: Option<SurfaceHandle>,
    /// The windows followed, and the window surface it's over, if shown.
    windows: Vec<QmlObject>,
    parent: Option<NonNull<c_void>>,
    live: bool,
}

impl SurfaceItem {
    pub(crate) fn new(id: NodeId, events: Events) -> SurfaceItem {
        let item = QmlObject::load(&qml::gpu_surface());
        let state = Rc::new(RefCell::new(ItemState {
            id,
            events,
            item,
            subsurface: None,
            handle: None,
            windows: Vec::new(),
            parent: None,
            live: true,
        }));
        for signal in ["windowChanged(QQuickWindow*)", "visibleChanged()"] {
            let s = Rc::downgrade(&state);
            item.connect(signal, move || ItemState::sync(&s));
        }
        SurfaceItem { item, state }
    }

    /// The node is gone: stop showing and reporting.
    pub(crate) fn detach(&self) {
        let mut state = self.state.borrow_mut();
        state.live = false;
        state.handle = None;
        state.hide();
    }
}

impl ItemState {
    /// Shows the surface over the item while the item and its window are
    /// shown, and places it. Run on every change that may move it and
    /// after each of the window's frames.
    fn sync(this: &Weak<RefCell<ItemState>>) {
        let Some(this) = this.upgrade() else { return };
        let mut state = this.borrow_mut();
        if !state.live {
            return;
        }
        let Some(window) = state.item.item_window() else {
            state.hide();
            return;
        };
        if !state.windows.contains(&window) {
            state.windows.push(window);
            for signal in ["afterAnimating()", "visibleChanged(bool)"] {
                let s = Rc::downgrade(&this);
                window.connect(signal, move || ItemState::sync(&s));
            }
        }
        let parent = window.wl_surface();
        if !state.item.bool("visible") || !window.bool("visible") || parent.is_none() {
            state.hide();
            return;
        }
        if state.subsurface.is_none() {
            let Some(display) = ffi::wayland_display() else { return };
            // SAFETY: Qt's display, connected as long as the app runs.
            match unsafe { Subsurface::new(display.as_ptr()) } {
                Ok(subsurface) => {
                    let handle = subsurface.handle();
                    state.subsurface = Some(subsurface);
                    state.handle = Some(handle.clone());
                    state.events.emit(state.id, UiEvent::SurfaceReady(handle));
                }
                Err(problem) => {
                    eprintln!("mitsuami: no GPU surface: {problem}");
                    state.live = false;
                    return;
                }
            }
        }
        // A window's surface is new each time it's shown.
        if state.parent != parent {
            state.parent = parent;
            // SAFETY: the window's live surface, on Qt's connection.
            unsafe { state.subsurface.as_ref().unwrap().attach(parent.unwrap().as_ptr()) };
        }
        state.place(window);
    }

    fn hide(&mut self) {
        self.parent = None;
        if let Some(subsurface) = &self.subsurface {
            subsurface.detach();
        }
    }

    /// Puts the subsurface over the item, and reports its size in pixels.
    fn place(&self, window: QmlObject) {
        let (Some(subsurface), Some(handle)) = (&self.subsurface, &self.handle) else { return };
        let origin = self.item.map_to_scene(mitsuami_core::Point::ZERO);
        let (left, top) = window.content_origin();
        let (width, height) = (self.item.real("width").round() as i32, self.item.real("height").round() as i32);
        let scale = window.device_pixel_ratio();
        let size = SurfaceSize {
            width: (width as f64 * scale).round() as u32,
            height: (height as f64 * scale).round() as u32,
            scale: scale as f32,
        };
        if !size.is_empty() && handle.set_size(size) {
            self.events.emit(self.id, UiEvent::SurfaceResized(size));
        }
        // A move takes effect with the window's next commit.
        if subsurface.place(origin.x.round() as i32 + left, origin.y.round() as i32 + top, width, height) {
            window.invoke("update");
        }
    }
}
