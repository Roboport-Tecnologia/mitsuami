//! `GpuSurface`: an item that keeps the space, and over it a surface of our
//! own (`mitsuami-linux`), which the app presents to: on Wayland a desync
//! subsurface of its window's surface, on X11 a child window of its
//! window. Qt's own routes (a `QQuickRhiItem`, a child `QWindow`) wait for
//! the scene graph, or are destroyed and made again with their window;
//! this one lives as long as the app's handle. It's placed after each of
//! the window's frames, so it follows the item wherever layout or
//! scrolling moves it.
//!
//! Its input goes through Qt: the surface takes none, so the pointer and
//! keys reach an input item filling the item (`SurfaceInputItem` in the
//! shim). The pointer lock is `mitsuami-linux`'s (Qt has none), the
//! keyboard grab the compositor's shortcuts inhibitor on Wayland and
//! `QWindow::setKeyboardGrabEnabled` on X11; the input item takes the
//! window's shortcuts (`ShortcutOverride`) while it's grabbed.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::sync::Mutex;

use mitsuami_core::{
    Cursor, KeyCode, Modifiers, MouseButton, NodeId, Point, ScrollDelta, SurfaceHandle, SurfaceInput, SurfaceSize,
    UiEvent,
};
use mitsuami_linux::wayland::{self, Subsurface};
use mitsuami_linux::x11::ChildWindow;
use mitsuami_linux::{LockEvent, LockSink};

use crate::events::Events;
use crate::ffi::{self, Callback, QmlObject, SurfaceEvent};
use crate::qml;

/// The item that keeps the surface's space, and what it reports.
pub(crate) struct SurfaceItem {
    pub(crate) item: QmlObject,
    /// Takes the input, filling the item.
    pub(crate) input: QmlObject,
    state: Rc<RefCell<ItemState>>,
    /// The keys down, by evdev code: let go when it loses focus.
    held: Rc<Keys>,
}

/// Keys down on the surface, by evdev code, and where to report them.
struct Keys {
    id: NodeId,
    events: Events,
    /// Evdev codes, and the code their press was reported as (a remapped
    /// key's, by its keysym).
    down: RefCell<Vec<(u32, KeyCode)>>,
}

/// The surface of our own, by display server.
enum Native {
    Wayland(Subsurface),
    X11(ChildWindow),
}

/// The window surface it's over.
#[derive(Clone, Copy, PartialEq)]
enum Parent {
    Wayland(NonNull<c_void>),
    X11(u32),
}

/// A pointer lock or keyboard grab of `mitsuami-linux`'s, which lets go
/// when dropped.
type Held = Box<dyn std::any::Any>;

enum KeyboardGrab {
    Wayland(#[allow(dead_code, reason = "let go when dropped")] Held),
    X11(QmlObject),
}

impl Drop for KeyboardGrab {
    fn drop(&mut self) {
        if let KeyboardGrab::X11(window) = self {
            window.set_keyboard_grab(false);
        }
    }
}

struct ItemState {
    id: NodeId,
    events: Events,
    item: QmlObject,
    input: QmlObject,
    /// Its number in `SURFACES`, for what locks report.
    number: u64,
    /// Made when the item is first shown in a window.
    native: Option<Native>,
    /// Given up when the node is destroyed; the app may keep its own.
    handle: Option<SurfaceHandle>,
    /// The windows followed, and the window surface it's over, if shown.
    windows: Vec<QmlObject>,
    parent: Option<Parent>,
    live: bool,
    /// The props, as the core last set them (`None`: never). A lock or
    /// grab the platform ended is `false` again.
    takes_input: Option<bool>,
    pointer_lock: Option<bool>,
    keyboard_grab: Option<bool>,
    /// Qt gives no cursor back as it was set.
    cursor: Option<Cursor>,
    keys: Rc<Keys>,
    /// What's in effect, and which of them (a lock's reports carry it, so
    /// a late one from a lock let go is dropped).
    lock: Option<(u64, Held)>,
    grab: Option<(u64, KeyboardGrab)>,
    generation: u64,
}

/// What a lock's thread reported, for the UI thread: the surface's
/// number, the lock's generation, and the event.
static REPORTED: Mutex<Vec<(u64, u64, LockEvent)>> = Mutex::new(Vec::new());

thread_local! {
    /// The surfaces, by number, for what their locks report.
    static SURFACES: RefCell<HashMap<u64, Weak<RefCell<ItemState>>>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

impl SurfaceItem {
    pub(crate) fn new(id: NodeId, events: Events) -> SurfaceItem {
        let item = QmlObject::load(&qml::gpu_surface());
        let number = NEXT.with(|n| n.replace(n.get() + 1));
        if number == 1 {
            // What locks report comes in on other threads, which wake the
            // loop; it's handled before the loop sleeps again.
            ffi::watch_loop(take_reported);
        }
        let held = Rc::new(Keys { id, events: events.clone(), down: RefCell::default() });
        let keys = held.clone();
        let input = item.surface_input(ffi::register(move |callback| {
            if let Callback::Input(event) = callback
                && let Some(input) = surface_input(event)
            {
                if let SurfaceInput::Key { code, native, pressed, .. } = input {
                    let mut down = keys.down.borrow_mut();
                    down.retain(|(k, _)| *k != native);
                    if pressed {
                        down.push((native, code));
                    }
                }
                keys.events.emit(id, UiEvent::SurfaceInput(input));
            }
        }));
        let state = Rc::new(RefCell::new(ItemState {
            id,
            events,
            item,
            input,
            number,
            native: None,
            handle: None,
            windows: Vec::new(),
            parent: None,
            live: true,
            takes_input: None,
            pointer_lock: None,
            keyboard_grab: None,
            cursor: None,
            keys: held.clone(),
            lock: None,
            grab: None,
            generation: 0,
        }));
        SURFACES.with(|s| s.borrow_mut().insert(number, Rc::downgrade(&state)));
        for signal in ["windowChanged(QQuickWindow*)", "visibleChanged()"] {
            let s = Rc::downgrade(&state);
            item.connect(signal, move || ItemState::sync(&s));
        }
        // A grab lasts while the surface has focus, and keys held are let
        // go: their releases go elsewhere. Qt Quick takes active focus
        // away when the window stops being the active one, too.
        let (s, keys) = (Rc::downgrade(&state), Rc::downgrade(&held));
        input.connect("activeFocusChanged(bool)", move || {
            if let Some(keys) = keys.upgrade()
                && !input.bool("activeFocus")
            {
                keys.release();
            }
            // Not while the state is busy: its own changes don't move focus.
            if let Some(s) = s.upgrade()
                && let Ok(mut state) = s.try_borrow_mut()
                && !state.input.bool("activeFocus")
            {
                state.end_grab();
            }
        });
        SurfaceItem { item, input, state, held }
    }

    /// The node is gone: stop showing and reporting.
    pub(crate) fn detach(&self) {
        self.forget_keys();
        let mut state = self.state.borrow_mut();
        state.live = false;
        state.handle = None;
        state.hide();
        SURFACES.with(|s| s.borrow_mut().remove(&state.number));
    }

    pub(crate) fn set_takes_input(&self, takes: bool) {
        let mut state = self.state.borrow_mut();
        state.takes_input = Some(takes);
        state.configure();
    }

    pub(crate) fn set_pointer_lock(&self, on: bool) {
        let mut state = self.state.borrow_mut();
        state.pointer_lock = Some(on);
        if on { state.lock_pointer() } else { state.unlock_pointer() }
    }

    pub(crate) fn set_keyboard_grab(&self, on: bool) {
        if on {
            // Focus first: the grab lasts while it has it.
            let input = self.state.borrow().input;
            input.force_focus();
        }
        let mut state = self.state.borrow_mut();
        state.keyboard_grab = Some(on);
        if on { state.grab_keyboard() } else { state.release_keyboard() }
    }

    pub(crate) fn set_cursor(&self, cursor: &Cursor) {
        let mut state = self.state.borrow_mut();
        state.input.set_surface_cursor(cursor);
        state.cursor = Some(cursor.clone());
    }

    /// The props kept on the node, as the core set them, the lock and
    /// grab as they are.
    pub(crate) fn props(&self) -> (Option<bool>, Option<bool>, Option<bool>, Option<Cursor>) {
        let state = self.state.borrow();
        (state.takes_input, state.pointer_lock, state.keyboard_grab, state.cursor.clone())
    }

    /// Lets go of the keys down without reporting them: the node is gone.
    fn forget_keys(&self) {
        self.held.down.borrow_mut().clear();
    }

    pub(crate) fn takes_input(&self) -> bool {
        self.state.borrow().takes_input == Some(true)
    }
}

impl Keys {
    /// Reports every key down as let go.
    fn release(&self) {
        let down = std::mem::take(&mut *self.down.borrow_mut());
        for (native, code) in down {
            let modifiers = Modifiers::default();
            let key = SurfaceInput::Key { code, native, pressed: false, repeat: false, modifiers };
            self.events.emit(self.id, UiEvent::SurfaceInput(key));
        }
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
            // The platform ends the lock and grab when the window stops
            // being the active one, and keys held are let go (active focus
            // leaving the input item does it too).
            let s = Rc::downgrade(&this);
            window.connect("activeChanged()", move || {
                if let Some(s) = s.upgrade()
                    && let Ok(mut state) = s.try_borrow_mut()
                    && !window.is_active()
                {
                    state.keys.release();
                    state.end_lock();
                    state.end_grab();
                }
            });
        }
        let parent = match window.wl_surface() {
            Some(surface) => Some(Parent::Wayland(surface)),
            None => window.xid().map(Parent::X11),
        };
        if !state.item.bool("visible") || !window.bool("visible") || parent.is_none() {
            state.hide();
            return;
        }
        let parent = parent.unwrap();
        if state.native.is_none() {
            let made = match parent {
                Parent::Wayland(_) => match ffi::wayland_display() {
                    // SAFETY: Qt's display, connected as long as the app runs.
                    Some(display) => unsafe { Subsurface::new(display.as_ptr()) }.map(Native::Wayland),
                    None => return,
                },
                // On Qt's display, which it found as xcb does ($DISPLAY).
                Parent::X11(xid) => ChildWindow::new(None, xid).map(Native::X11),
            };
            match made {
                Ok(native) => {
                    let handle = match &native {
                        Native::Wayland(subsurface) => subsurface.handle(),
                        Native::X11(child) => child.handle(),
                    };
                    state.native = Some(native);
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
        if state.parent != Some(parent) {
            state.parent = Some(parent);
            match (state.native.as_ref().unwrap(), parent) {
                // SAFETY: the window's live surface, on Qt's connection.
                (Native::Wayland(subsurface), Parent::Wayland(surface)) => unsafe {
                    subsurface.attach(surface.as_ptr())
                },
                (Native::X11(child), Parent::X11(xid)) => child.attach(xid),
                _ => {}
            }
        }
        state.place(window);
        // A lock or grab asked for before the surface was shown.
        if state.pointer_lock == Some(true) && state.lock.is_none() {
            state.lock_pointer();
        }
        if state.keyboard_grab == Some(true) && state.grab.is_none() {
            state.grab_keyboard();
        }
    }

    /// Takes the surface off its window; a lock or grab is let go, and
    /// made again when it's shown.
    fn hide(&mut self) {
        self.parent = None;
        self.lock = None;
        self.grab = None;
        self.configure();
        match &self.native {
            Some(Native::Wayland(subsurface)) => subsurface.detach(),
            Some(Native::X11(child)) => child.detach(),
            None => {}
        }
    }

    /// Puts the surface over the item, and reports its size in pixels.
    fn place(&self, window: QmlObject) {
        let (Some(native), Some(handle)) = (&self.native, &self.handle) else { return };
        let origin = self.item.map_to_scene(Point::ZERO);
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
        match native {
            Native::Wayland(subsurface) => {
                // Qt's own decorations are part of the window's surface.
                let (left, top) = window.content_origin();
                let (x, y) = (origin.x.round() as i32 + left, origin.y.round() as i32 + top);
                // A move takes effect with the window's next commit.
                if subsurface.place(x, y, width, height) {
                    window.invoke("update");
                }
            }
            // In the window's own pixels; the window manager's frame is
            // outside it.
            Native::X11(child) => {
                let px = |v: f64| (v * scale).round() as i32;
                child.place(px(origin.x as f64), px(origin.y as f64), size.width as i32, size.height as i32);
            }
        }
    }

    /// Tells the input item what it takes.
    fn configure(&self) {
        self.input.configure_surface_input(self.takes_input == Some(true), self.grab.is_some(), self.lock.is_some());
    }

    /// Where a lock's thread reports, tagged with its generation.
    fn sink(&mut self) -> (u64, LockSink) {
        self.generation += 1;
        let (number, generation) = (self.number, self.generation);
        let sink: LockSink = Box::new(move |event| {
            REPORTED.lock().unwrap().push((number, generation, event));
            ffi::wake();
        });
        (generation, sink)
    }

    fn active_window(&self) -> Option<QmlObject> {
        self.item.item_window().filter(|w| w.is_active())
    }

    fn lock_pointer(&mut self) {
        // Locked already, or not shown yet: `sync` locks when it is.
        if self.lock.is_some() || self.parent.is_none() {
            return;
        }
        let (Some(window), Some(parent)) = (self.active_window(), self.parent) else { return self.end_lock_now() };
        let (generation, sink) = self.sink();
        let lock = match (&self.native, parent) {
            (Some(Native::Wayland(_)), Parent::Wayland(surface)) => match ffi::wayland_display() {
                // SAFETY: Qt's display, and its window's live surface on it.
                Some(display) => unsafe { wayland::PointerLock::new(display.as_ptr(), surface.as_ptr(), sink) }
                    .map(|lock| Box::new(lock) as Held)
                    .map_err(|problem| eprintln!("mitsuami: no pointer lock: {problem}"))
                    .ok(),
                None => None,
            },
            (Some(Native::X11(child)), Parent::X11(_)) => {
                Some(Box::new(child.lock_pointer(window.device_pixel_ratio() as f32, sink)) as Held)
            }
            _ => None,
        };
        match lock {
            Some(lock) => {
                self.lock = Some((generation, lock));
                self.configure();
            }
            None => self.end_lock_now(),
        }
    }

    fn unlock_pointer(&mut self) {
        self.lock = None;
        self.configure();
    }

    /// The platform ended the lock, or it couldn't be made.
    fn end_lock_now(&mut self) {
        self.lock = None;
        self.configure();
        if self.pointer_lock == Some(true) {
            self.pointer_lock = Some(false);
            self.events.emit(self.id, UiEvent::PointerLockEnded);
        }
    }

    /// Ends the lock, if there's one or one was asked for.
    fn end_lock(&mut self) {
        if self.pointer_lock == Some(true) {
            self.end_lock_now();
        }
    }

    fn grab_keyboard(&mut self) {
        if self.grab.is_some() || self.parent.is_none() {
            return;
        }
        let (Some(window), Some(parent)) = (self.active_window(), self.parent) else { return self.end_grab() };
        if !self.input.bool("activeFocus") {
            return self.end_grab();
        }
        let (generation, sink) = self.sink();
        let grab = match parent {
            Parent::Wayland(surface) => match ffi::wayland_display() {
                // SAFETY: Qt's display, and its window's live surface on it.
                Some(display) => unsafe { wayland::ShortcutsInhibitor::new(display.as_ptr(), surface.as_ptr(), sink) }
                    .map(|inhibitor| KeyboardGrab::Wayland(Box::new(inhibitor)))
                    .map_err(|problem| eprintln!("mitsuami: no keyboard grab: {problem}"))
                    .ok(),
                None => None,
            },
            Parent::X11(_) => window.set_keyboard_grab(true).then_some(KeyboardGrab::X11(window)),
        };
        match grab {
            Some(grab) => {
                self.grab = Some((generation, grab));
                self.configure();
            }
            None => self.end_grab(),
        }
    }

    fn release_keyboard(&mut self) {
        self.grab = None;
        self.configure();
    }

    /// The platform ended the grab (or focus left the surface), or it
    /// couldn't be made.
    fn end_grab(&mut self) {
        self.grab = None;
        self.configure();
        if self.keyboard_grab == Some(true) {
            self.keyboard_grab = Some(false);
            self.events.emit(self.id, UiEvent::KeyboardGrabEnded);
        }
    }

    /// What a lock's thread reported.
    fn reported(&mut self, generation: u64, event: LockEvent) {
        let lock = self.lock.as_ref().is_some_and(|(g, _)| *g == generation);
        let grab = self.grab.as_ref().is_some_and(|(g, _)| *g == generation);
        match event {
            LockEvent::Input(input) if lock => self.events.emit(self.id, UiEvent::SurfaceInput(input)),
            LockEvent::Ended if lock => self.end_lock_now(),
            LockEvent::Ended if grab => self.end_grab(),
            // From a lock or grab let go since.
            _ => {}
        }
    }
}

/// Hands what locks reported to their surfaces, on the UI thread.
fn take_reported() {
    let reported = std::mem::take(&mut *REPORTED.lock().unwrap());
    for (number, generation, event) in reported {
        let state = SURFACES.with(|s| s.borrow().get(&number).and_then(Weak::upgrade));
        if let Some(state) = state {
            state.borrow_mut().reported(generation, event);
        }
    }
}

/// What the input item reported, as the core has it.
fn surface_input(event: SurfaceEvent) -> Option<SurfaceInput> {
    let modifiers = Modifiers {
        shift: event.flags & 1 != 0,
        control: event.flags & 2 != 0,
        alt: event.flags & 4 != 0,
        meta: event.flags & 8 != 0,
    };
    let position = Point::new(event.x as f32, event.y as f32);
    let (x, y) = (event.x as f32, event.y as f32);
    Some(match event.kind {
        0 | 1 => {
            // XKB key codes are evdev's plus 8, on X11 and Wayland. Keys
            // that type no character go by their keysym, so the keymap's
            // remaps (Caps Lock and Control swapped) apply.
            let native = (event.code as u32).saturating_sub(8);
            SurfaceInput::Key {
                code: KeyCode::from_keysym(event.x as u32).unwrap_or_else(|| KeyCode::from_evdev(native)),
                native,
                pressed: event.kind == 0,
                repeat: event.flags & 16 != 0,
                modifiers,
            }
        }
        2 => SurfaceInput::PointerMoved { position, modifiers },
        3 => SurfaceInput::PointerLeft,
        4 | 5 => {
            let button = match event.code {
                0 => MouseButton::Primary,
                1 => MouseButton::Secondary,
                2 => MouseButton::Middle,
                3 => MouseButton::Back,
                4 => MouseButton::Forward,
                n => MouseButton::Other(n as u16),
            };
            SurfaceInput::Button { button, pressed: event.kind == 4, position, modifiers }
        }
        6 => SurfaceInput::Scroll { delta: ScrollDelta::Lines { x, y }, modifiers },
        7 => SurfaceInput::Scroll { delta: ScrollDelta::Points { x, y }, modifiers },
        _ => return None,
    })
}

/// The evdev code of a key, for tests' key events.
pub(crate) fn evdev_code(code: KeyCode) -> u32 {
    (1..=127).find(|c| KeyCode::from_evdev(*c) == code).unwrap_or(0)
}
