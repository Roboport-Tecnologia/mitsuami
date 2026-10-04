//! `GpuSurface`: a widget that keeps the space, and over it a surface of
//! our own (`mitsuami-linux`) which the app presents to: a desync
//! subsurface of the window's surface on Wayland, a child window of the
//! window's on X11. It's placed after each of the window's frames, so it
//! follows the widget wherever layout or scrolling moves it.
//!
//! The surface takes no input, so keys and the pointer reach the widget,
//! which reports them when the app asked (`TakesInput`). GTK has no
//! pointer lock: `mitsuami-linux` makes it (a locked pointer on Wayland, a
//! pointer grab on X11), and reports from a thread of its own, sent here
//! through GLib's main context. The keyboard grab is GTK's own
//! `gdk_toplevel_inhibit_system_shortcuts`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::{Rc, Weak};

use gtk::glib::translate::{IntoGlib, ToGlibPtr};
use gtk::prelude::*;
use gtk::{gdk, glib, graphene};
use mitsuami_core::{
    ActionError, Cursor, KeyCode, Modifiers, MouseButton, NodeId, Pixels, Point, Prop, ScrollDelta, SurfaceHandle,
    SurfaceInput, SurfaceSize, SyntheticInput, UiEvent,
};
use mitsuami_linux::wayland::{self, Subsurface};
use mitsuami_linux::{LockEvent, NoSurface, x11};

use crate::host::Events;

unsafe extern "C" {
    fn gdk_wayland_display_get_wl_display(display: *mut gdk::ffi::GdkDisplay) -> *mut c_void;
    fn gdk_wayland_surface_get_wl_surface(surface: *mut gdk::ffi::GdkSurface) -> *mut c_void;
    fn gdk_x11_surface_get_xid(surface: *mut gdk::ffi::GdkSurface) -> std::ffi::c_ulong;
}

/// The widget that keeps the surface's space, and what it reports.
pub(crate) struct SurfaceArea {
    pub(crate) area: gtk::DrawingArea,
    state: Rc<RefCell<AreaState>>,
}

/// The surface over the widget, on the window's display server.
enum Native {
    Wayland(Subsurface),
    X11(x11::ChildWindow),
    /// Another display (Broadway), which has none.
    Other,
}

/// The window's own surface, which ours goes over.
#[derive(Clone, Copy, PartialEq)]
enum Parent {
    Wayland(*mut c_void),
    X11(u32),
    Other,
}

/// A pointer lock in effect (`mitsuami-linux`'s), kept for its `Drop`,
/// which lets it go.
type Lock = Box<dyn std::any::Any>;

struct AreaState {
    id: NodeId,
    events: Events,
    area: glib::WeakRef<gtk::DrawingArea>,
    /// Its number, for what the locks report from their threads.
    key: u64,
    /// Made when the widget is first mapped in a window.
    native: Option<Native>,
    /// Given up when the node is destroyed; the app may keep its own.
    handle: Option<SurfaceHandle>,
    after_paint: Option<(gdk::FrameClock, glib::SignalHandlerId)>,
    /// The window it's in while mapped, and its `is-active` handler.
    window: Option<(gtk::Window, glib::SignalHandlerId)>,
    /// What the app set; `None` until it did.
    takes_input: Option<bool>,
    pointer_lock: Option<bool>,
    keyboard_grab: Option<bool>,
    /// The lock in effect, and its number (a lock's late reports are
    /// dropped once it's gone).
    lock: Option<Lock>,
    locks: u32,
    /// The keyboard grab in effect: the toplevel inhibiting the system's
    /// shortcuts, its `shortcuts-inhibited` handler, and the key
    /// controller that takes keys from the window first.
    grab: Option<Grab>,
    /// Keys down whose press was reported (evdev codes), and the code it
    /// was reported as (a remapped key's, by its keysym): what makes a
    /// repeat, which releases are the surface's, and what to let go.
    pressed: HashMap<u32, KeyCode>,
    /// What the app set, which a `gdk::Cursor` can't give back.
    cursor: Option<Cursor>,
}

struct Grab {
    toplevel: gdk::Toplevel,
    inhibited: glib::SignalHandlerId,
    window: gtk::Window,
    keys: gtk::EventControllerKey,
}

thread_local! {
    /// Surfaces by number, for what their locks report from other threads.
    static AREAS: RefCell<HashMap<u64, Weak<RefCell<AreaState>>>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

impl SurfaceArea {
    pub(crate) fn new(id: NodeId, events: Events) -> SurfaceArea {
        let area = gtk::DrawingArea::new();
        area.set_accessible_role(gtk::AccessibleRole::Img);
        let key = NEXT.with(|n| n.replace(n.get() + 1));
        let state = Rc::new(RefCell::new(AreaState {
            id,
            events,
            area: area.downgrade(),
            key,
            native: None,
            handle: None,
            after_paint: None,
            window: None,
            takes_input: None,
            pointer_lock: None,
            keyboard_grab: None,
            lock: None,
            locks: 0,
            grab: None,
            pressed: HashMap::new(),
            cursor: None,
        }));
        AREAS.with(|a| a.borrow_mut().insert(key, Rc::downgrade(&state)));
        let s = state.clone();
        area.connect_map(move |area| AreaState::mapped(&s, area));
        let s = state.clone();
        area.connect_unmap(move |_| s.borrow_mut().unmapped());
        // Its new size as soon as it's allocated, not after the frame's
        // paint: the app draws the next frame at it.
        let s = Rc::downgrade(&state);
        area.connect_resize(move |area, _, _| {
            if let Some(s) = s.upgrade() {
                s.borrow().place(area);
            }
        });
        input_controllers(&area, &state);
        SurfaceArea { area, state }
    }

    /// The node is gone: stop showing and reporting.
    pub(crate) fn detach(&self) {
        let mut state = self.state.borrow_mut();
        state.unmapped();
        state.pointer_lock = None;
        state.keyboard_grab = None;
        state.handle = None;
        AREAS.with(|a| a.borrow_mut().remove(&state.key));
    }

    pub(crate) fn set_prop(&self, prop: &Prop) {
        match prop {
            Prop::TakesInput(on) => {
                self.state.borrow_mut().takes_input = Some(*on);
                self.area.set_focusable(*on);
                self.area.set_focus_on_click(*on);
            }
            Prop::PointerLock(on) => {
                self.state.borrow_mut().pointer_lock = Some(*on);
                AreaState::sync_lock(&self.state);
            }
            Prop::KeyboardGrab(on) => {
                self.state.borrow_mut().keyboard_grab = Some(*on);
                AreaState::sync_grab(&self.state);
            }
            // On the widget: the surface over it takes no input, so the
            // pointer, and its cursor, are GTK's. "none" is what GTK's own
            // video widget hides it with.
            Prop::Cursor(cursor) => {
                let gdk_cursor = match cursor {
                    Cursor::Default => None,
                    Cursor::Hidden => gdk::Cursor::from_name("none", None),
                    Cursor::Image { pixels, hotspot } => cursor_image(pixels, *hotspot),
                };
                self.area.set_cursor(gdk_cursor.as_ref());
                self.state.borrow_mut().cursor = Some(cursor.clone());
            }
            _ => {}
        }
    }

    /// The props GTK can't report: whether it takes input, the lock and
    /// grab in effect, and the cursor.
    pub(crate) fn props(&self) -> Vec<Prop> {
        let state = self.state.borrow();
        let mut props = Vec::new();
        props.extend(state.takes_input.map(Prop::TakesInput));
        props.extend(state.pointer_lock.map(|_| Prop::PointerLock(state.lock.is_some())));
        props.extend(state.keyboard_grab.map(|_| Prop::KeyboardGrab(state.grab.is_some())));
        props.extend(state.cursor.clone().map(Prop::Cursor));
        props
    }

    /// Input as the user would give it: GTK 4 can't inject events, so
    /// what the widget's controllers would report is reported.
    pub(crate) fn synthesize(&self, input: &SyntheticInput) -> Result<(), ActionError> {
        if self.state.borrow().takes_input != Some(true) {
            return Err(ActionError::Unsupported);
        }
        let modifiers = Modifiers::default();
        let state = self.state.borrow();
        match input {
            SyntheticInput::Click(position) => {
                drop(state);
                self.area.grab_focus();
                let state = self.state.borrow();
                for pressed in [true, false] {
                    let button = MouseButton::Primary;
                    let input = SurfaceInput::Button { button, pressed, position: *position, modifiers };
                    state.report(input);
                }
            }
            SyntheticInput::Key(key) => {
                // Focused in its window: a test's window may not be the
                // active one, which `has_focus` also asks.
                if !self.area.is_focus() {
                    return Err(ActionError::Unsupported);
                }
                let code = KeyCode::from_key(*key);
                let native = (1..=255).find(|c| KeyCode::from_evdev(*c) == code).unwrap_or(0);
                for pressed in [true, false] {
                    state.report(SurfaceInput::Key { code, native, pressed, repeat: false, modifiers });
                }
            }
            SyntheticInput::Scroll { dx, dy } => {
                let delta = ScrollDelta::Points { x: *dx, y: *dy };
                state.report(SurfaceInput::Scroll { delta, modifiers });
            }
            // A surface takes no files, and keys with modifiers aren't
            // simulated on one.
            SyntheticInput::Shortcut(_)
            | SyntheticInput::DragFiles(_)
            | SyntheticInput::DragLeave
            | SyntheticInput::DropFiles(_)
            | SyntheticInput::PointerEnter
            | SyntheticInput::PointerLeave
            | SyntheticInput::DoubleClick => {
                return Err(ActionError::Unsupported);
            }
        }
        Ok(())
    }
}

impl AreaState {
    fn report(&self, input: SurfaceInput) {
        self.events.emit(self.id, UiEvent::SurfaceInput(input));
    }

    fn mapped(this: &Rc<RefCell<AreaState>>, area: &gtk::DrawingArea) {
        {
            let mut state = this.borrow_mut();
            let Some(parent) = window_surface(area) else { return };
            if state.native.is_none() {
                match make_native(area, parent) {
                    Ok(native) => {
                        let handle = match &native {
                            Native::Wayland(subsurface) => subsurface.handle(),
                            Native::X11(child) => child.handle(),
                            Native::Other => NoSurface::handle(),
                        };
                        state.native = Some(native);
                        state.handle = Some(handle.clone());
                        state.events.emit(state.id, UiEvent::SurfaceReady(handle));
                    }
                    Err(problem) => {
                        glib::g_warning!("mitsuami", "no GPU surface: {problem}");
                        return;
                    }
                }
            }
            match (state.native.as_ref().unwrap(), parent) {
                // SAFETY: the window's live surface, on GDK's connection.
                (Native::Wayland(subsurface), Parent::Wayland(surface)) => unsafe { subsurface.attach(surface) },
                (Native::X11(child), Parent::X11(xid)) => child.attach(xid),
                _ => {}
            }
            if let Some(clock) = area.frame_clock() {
                let (s, a) = (Rc::downgrade(this), area.downgrade());
                let handler = clock.connect_after_paint(move |_| {
                    if let (Some(s), Some(a)) = (s.upgrade(), a.upgrade()) {
                        s.borrow().place(&a);
                    }
                });
                state.after_paint = Some((clock, handler));
            }
            // The lock and the grab last while the window is the active one.
            if let Some(window) = area.root().and_downcast::<gtk::Window>() {
                let s = Rc::downgrade(this);
                let handler = window.connect_is_active_notify(move |window| {
                    if !window.is_active()
                        && let Some(s) = s.upgrade()
                    {
                        AreaState::end_lock(&s);
                        AreaState::end_grab(&s);
                        // Their releases go to another window. The focus
                        // controller's `leave` should come too (GTK
                        // crosses focus out of an inactive window); this
                        // doesn't depend on it.
                        s.borrow_mut().release_keys();
                    }
                });
                state.window = Some((window, handler));
            }
            state.place(area);
        }
        // Asked for before it was in a window.
        AreaState::sync_lock(this);
        AreaState::sync_grab(this);
    }

    fn unmapped(&mut self) {
        if let Some((clock, handler)) = self.after_paint.take() {
            clock.disconnect(handler);
        }
        if let Some((window, handler)) = self.window.take() {
            window.disconnect(handler);
        }
        self.release_lock();
        self.release_grab();
        match &self.native {
            Some(Native::Wayland(subsurface)) => subsurface.detach(),
            Some(Native::X11(child)) => child.detach(),
            Some(Native::Other) | None => {}
        }
    }

    /// Puts the surface over the widget, and reports its size in pixels.
    fn place(&self, area: &gtk::DrawingArea) {
        let (Some(native_surface), Some(handle)) = (&self.native, &self.handle) else { return };
        let (Some(native), Some(root)) = (area.native(), area.root()) else { return };
        let Some(p) = area.compute_point(&root, &graphene::Point::new(0.0, 0.0)) else { return };
        let (tx, ty) = native.surface_transform();
        let (x, y) = (p.x() as f64 + tx, p.y() as f64 + ty);
        let (width, height) = (area.width(), area.height());
        let scale = area.scale_factor();
        let size = SurfaceSize { width: (width * scale) as u32, height: (height * scale) as u32, scale: scale as f32 };
        if !size.is_empty() && handle.set_size(size) {
            self.events.emit(self.id, UiEvent::SurfaceResized(size));
        }
        match native_surface {
            // A move takes effect with the window's next commit.
            Native::Wayland(subsurface) => {
                if subsurface.place(x as i32, y as i32, width, height) {
                    area.queue_draw();
                }
            }
            // X11 is in pixels; GDK's scale is a whole number there.
            Native::X11(child) => {
                let s = scale as f64;
                child.place((x * s).round() as i32, (y * s).round() as i32, width * scale, height * scale);
            }
            Native::Other => {}
        }
    }

    // ---------------------------------------------------------- the lock

    /// Locks or unlocks the pointer as the app asked, once mapped. It
    /// can't be locked while the window isn't the active one.
    fn sync_lock(this: &Rc<RefCell<AreaState>>) {
        let mut state = this.borrow_mut();
        let wanted = state.pointer_lock == Some(true);
        if !wanted {
            state.release_lock();
            return;
        }
        if state.lock.is_some() {
            return;
        }
        let Some(area) = state.area.upgrade() else { return };
        // Not in a window yet: locked once it is.
        if state.window.is_none() {
            return;
        }
        let active = state.window.as_ref().is_some_and(|(w, _)| w.is_active());
        let lock = if active { state.lock_pointer(&area) } else { Err("the window isn't active".into()) };
        match lock {
            Ok(lock) => {
                state.lock = Some(lock);
                area.set_cursor_from_name(Some("none"));
            }
            Err(problem) => {
                if active {
                    glib::g_warning!("mitsuami", "no pointer lock: {problem}");
                }
                state.pointer_lock = Some(false);
                state.events.emit(state.id, UiEvent::PointerLockEnded);
            }
        }
    }

    fn lock_pointer(&mut self, area: &gtk::DrawingArea) -> Result<Lock, String> {
        self.locks += 1;
        let (key, number) = (self.key, self.locks);
        // Always a source on the default context, which the UI thread runs:
        // `MainContext::invoke` runs it on the calling thread when the UI
        // thread isn't holding the context (between iterations), where
        // `AREAS` is empty.
        let sink = Box::new(move |event: LockEvent| {
            glib::idle_add_full(glib::Priority::DEFAULT, move || {
                AreaState::from_lock(key, number, event);
                glib::ControlFlow::Break
            });
        });
        match self.native.as_ref().ok_or("no surface")? {
            Native::Wayland(_) => {
                let Some(Parent::Wayland(surface)) = window_surface(area) else { return Err("no wl_surface".into()) };
                let display = area.display();
                // SAFETY: a Wayland window's display and live surface.
                let lock = unsafe {
                    wayland::PointerLock::new(
                        gdk_wayland_display_get_wl_display(display.to_glib_none().0),
                        surface,
                        sink,
                    )
                };
                lock.map(|lock| Box::new(lock) as Lock)
            }
            Native::X11(child) => Ok(Box::new(child.lock_pointer(area.scale_factor() as f32, sink))),
            Native::Other => Err("the display has no pointer lock".into()),
        }
    }

    /// What a lock reported, on the UI thread.
    fn from_lock(key: u64, number: u32, event: LockEvent) {
        let Some(this) = AREAS.with(|a| a.borrow().get(&key).and_then(Weak::upgrade)) else { return };
        let state = this.borrow();
        if state.lock.is_none() || state.locks != number {
            return;
        }
        match event {
            LockEvent::Input(input) => state.report(input),
            LockEvent::Ended => {
                drop(state);
                AreaState::end_lock(&this);
            }
        }
    }

    /// The platform ended the lock.
    fn end_lock(this: &Rc<RefCell<AreaState>>) {
        let mut state = this.borrow_mut();
        if state.lock.is_some() {
            state.release_lock();
            state.pointer_lock = Some(false);
            state.events.emit(state.id, UiEvent::PointerLockEnded);
        }
    }

    fn release_lock(&mut self) {
        if self.lock.take().is_some()
            && let Some(area) = self.area.upgrade()
        {
            area.set_cursor(None);
        }
    }

    // ---------------------------------------------------------- the grab

    /// Grabs the keyboard or lets it go, as the app asked, once mapped:
    /// focuses the surface and has the compositor (or X server) send its
    /// shortcuts as keys.
    fn sync_grab(this: &Rc<RefCell<AreaState>>) {
        let mut state = this.borrow_mut();
        if state.keyboard_grab != Some(true) {
            state.release_grab();
            return;
        }
        if state.grab.is_some() || state.window.is_none() {
            return;
        }
        let Some(area) = state.area.upgrade() else { return };
        let window = state.window.as_ref().unwrap().0.clone();
        let toplevel = window.surface().and_downcast::<gdk::Toplevel>();
        let Some(toplevel) = toplevel.filter(|_| window.is_active()) else {
            state.keyboard_grab = Some(false);
            state.events.emit(state.id, UiEvent::KeyboardGrabEnded);
            return;
        };
        drop(state);
        area.grab_focus();
        let mut state = this.borrow_mut();
        // Before the window's own shortcuts: its capture phase comes first.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let s = Rc::downgrade(this);
        keys.connect_key_pressed(move |keys, _, keycode, modifiers| {
            let Some(s) = s.upgrade() else { return glib::Propagation::Proceed };
            let mut state = s.borrow_mut();
            if state.grab.is_none() || !state.area.upgrade().is_some_and(|a| a.has_focus()) {
                return glib::Propagation::Proceed;
            }
            state.key(key_code(keys, keycode), keycode, true, modifiers);
            glib::Propagation::Stop
        });
        let s = Rc::downgrade(this);
        keys.connect_key_released(move |keys, _, keycode, modifiers| {
            if let Some(s) = s.upgrade() {
                s.borrow_mut().key(key_code(keys, keycode), keycode, false, modifiers);
            }
        });
        window.add_controller(keys.clone());
        let s = Rc::downgrade(this);
        let inhibited = toplevel.connect_shortcuts_inhibited_notify(move |toplevel| {
            if !toplevel.is_shortcuts_inhibited()
                && let Some(s) = s.upgrade()
            {
                AreaState::end_grab(&s);
            }
        });
        toplevel.inhibit_system_shortcuts(None::<&gdk::Event>);
        state.grab = Some(Grab { toplevel, inhibited, window, keys });
    }

    /// The platform ended the grab.
    fn end_grab(this: &Rc<RefCell<AreaState>>) {
        let mut state = this.borrow_mut();
        if state.grab.is_some() {
            state.release_grab();
            state.keyboard_grab = Some(false);
            state.events.emit(state.id, UiEvent::KeyboardGrabEnded);
        }
    }

    fn release_grab(&mut self) {
        if let Some(grab) = self.grab.take() {
            grab.toplevel.disconnect(grab.inhibited);
            grab.toplevel.restore_system_shortcuts();
            grab.window.remove_controller(&grab.keys);
        }
    }

    // ------------------------------------------------------------ input

    /// A key down or up: reported when down, and when up if its press
    /// was. Returns whether it was reported.
    fn key(&mut self, code: KeyCode, keycode: u32, pressed: bool, state: gdk::ModifierType) -> bool {
        let native = keycode.saturating_sub(8);
        let repeat = if pressed {
            self.pressed.insert(native, code).is_some()
        } else if self.pressed.remove(&native).is_some() {
            false
        } else {
            return false;
        };
        self.report(SurfaceInput::Key { code, native, pressed, repeat, modifiers: modifiers(state) });
        true
    }

    /// Reports every key it has down as released: focus left, or the
    /// window stopped being the active one, so their releases go
    /// elsewhere.
    fn release_keys(&mut self) {
        for (native, code) in std::mem::take(&mut self.pressed) {
            let modifiers = Modifiers::default();
            self.report(SurfaceInput::Key { code, native, pressed: false, repeat: false, modifiers });
        }
    }

    fn takes_input(&self) -> bool {
        self.takes_input == Some(true)
    }
}

/// Where a key is, or what the keymap made it: keys that type no
/// character go by their keysym, so the keymap's remaps (Caps Lock and
/// Control swapped) apply. The unshifted one, in the event's layout.
fn key_code(keys: &gtk::EventControllerKey, keycode: u32) -> KeyCode {
    let layout = keys.current_event().and_then(|e| e.downcast::<gdk::KeyEvent>().ok()).map_or(0, |e| e.layout() as i32);
    let keysym = keys
        .widget()
        .and_then(|w| w.display().map_keycode(keycode))
        .and_then(|keys| keys.into_iter().find(|(k, _)| k.group() == layout && k.level() == 0))
        .map(|(_, keysym)| keysym.into_glib());
    keysym.and_then(KeyCode::from_keysym).unwrap_or_else(|| KeyCode::from_evdev(keycode.saturating_sub(8)))
}

/// A cursor from the app's pixels. GDK shows a texture's pixels one to a
/// point (GTK 4.16's `Cursor::from_callback` is the scale-aware way): an
/// image made at another scale is resampled to its size in points, so it
/// shows at the size the app meant.
fn cursor_image(pixels: &Pixels, hotspot: Point) -> Option<gdk::Cursor> {
    let scale = pixels.scale_factor();
    let (width, height) = (pixels.width(), pixels.height());
    // An image without pixels shows nothing, as a hidden cursor does; GDK
    // makes no texture of it.
    if width == 0 || height == 0 {
        return gdk::Cursor::from_name("none", None);
    }
    let (w, h) = if scale == 1.0 {
        (width, height)
    } else {
        (((width as f32 / scale).round() as u32).max(1), ((height as f32 / scale).round() as u32).max(1))
    };
    let rgba = pixels.rgba();
    let bytes: Vec<u8> = if (w, h) == (width, height) {
        rgba.to_vec()
    } else {
        // The pixel under each point's middle.
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for y in 0..h {
            let sy = (((y as f32 + 0.5) * scale) as u32).min(height - 1);
            for x in 0..w {
                let sx = (((x as f32 + 0.5) * scale) as u32).min(width - 1);
                let at = (sy as usize * width as usize + sx as usize) * 4;
                out.extend_from_slice(&rgba[at..at + 4]);
            }
        }
        out
    };
    let texture = gdk::MemoryTexture::new(
        w as i32,
        h as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(bytes),
        w as usize * 4,
    );
    let (x, y) = (hotspot.x.round() as i32, hotspot.y.round() as i32);
    Some(gdk::Cursor::from_texture(&texture, x.clamp(0, w as i32 - 1), y.clamp(0, h as i32 - 1), None))
}

/// The widget's controllers: they report while it takes input.
fn input_controllers(area: &gtk::DrawingArea, state: &Rc<RefCell<AreaState>>) {
    let keys = gtk::EventControllerKey::new();
    let s = Rc::downgrade(state);
    keys.connect_key_pressed(move |keys, keyval, keycode, modifiers| {
        let Some(s) = s.upgrade() else { return glib::Propagation::Proceed };
        if !s.borrow().takes_input() {
            return glib::Propagation::Proceed;
        }
        // Control+Tab leaves it, as it leaves a text view (the window's
        // move-focus binding).
        let tab = matches!(keyval, gdk::Key::Tab | gdk::Key::ISO_Left_Tab | gdk::Key::KP_Tab);
        if tab && modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }
        // The window's shortcuts come first, though they'd run after the
        // focused widget: leave them to it.
        if let Some(event) = keys.current_event()
            && window_takes(keys.widget().as_ref(), &event, modifiers)
        {
            return glib::Propagation::Proceed;
        }
        s.borrow_mut().key(key_code(keys, keycode), keycode, true, modifiers);
        glib::Propagation::Stop
    });
    let s = Rc::downgrade(state);
    keys.connect_key_released(move |keys, _, keycode, modifiers| {
        if let Some(s) = s.upgrade() {
            s.borrow_mut().key(key_code(keys, keycode), keycode, false, modifiers);
        }
    });
    area.add_controller(keys);

    let motion = gtk::EventControllerMotion::new();
    let s = Rc::downgrade(state);
    motion.connect_motion(move |motion, x, y| {
        let Some(s) = s.upgrade() else { return };
        let state = s.borrow();
        if state.takes_input() && state.lock.is_none() {
            let position = Point::new(x as f32, y as f32);
            state.report(SurfaceInput::PointerMoved { position, modifiers: modifiers(motion.current_event_state()) });
        }
    });
    let s = Rc::downgrade(state);
    motion.connect_leave(move |_| {
        if let Some(s) = s.upgrade()
            && s.borrow().takes_input()
        {
            s.borrow().report(SurfaceInput::PointerLeft);
        }
    });
    area.add_controller(motion);

    // Every button, each on its own: a click gesture tracks one at a time.
    let buttons = gtk::EventControllerLegacy::new();
    let s = Rc::downgrade(state);
    buttons.connect_event(move |buttons, event| {
        let pressed = match event.event_type() {
            gdk::EventType::ButtonPress => true,
            gdk::EventType::ButtonRelease => false,
            _ => return glib::Propagation::Proceed,
        };
        let Some(s) = s.upgrade() else { return glib::Propagation::Proceed };
        let Some(area) = buttons.widget() else { return glib::Propagation::Proceed };
        if !s.borrow().takes_input() {
            return glib::Propagation::Proceed;
        }
        let Some(button) = event.downcast_ref::<gdk::ButtonEvent>().map(|b| b.button()) else {
            return glib::Propagation::Proceed;
        };
        if pressed {
            area.grab_focus();
        }
        let position = event_position(&area, event).unwrap_or(Point::ZERO);
        let button = match button {
            1 => MouseButton::Primary,
            2 => MouseButton::Middle,
            3 => MouseButton::Secondary,
            8 => MouseButton::Back,
            9 => MouseButton::Forward,
            n => MouseButton::Other(n as u16),
        };
        let modifiers = modifiers(event.modifier_state());
        s.borrow().report(SurfaceInput::Button { button, pressed, position, modifiers });
        // Others still see it: a context menu's gesture, for one.
        glib::Propagation::Proceed
    });
    area.add_controller(buttons);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    let s = Rc::downgrade(state);
    scroll.connect_scroll(move |scroll, dx, dy| {
        let Some(s) = s.upgrade() else { return glib::Propagation::Proceed };
        if !s.borrow().takes_input() {
            return glib::Propagation::Proceed;
        }
        let (x, y) = (dx as f32, dy as f32);
        let delta = match scroll.unit() {
            gdk::ScrollUnit::Surface => ScrollDelta::Points { x, y },
            _ => ScrollDelta::Lines { x, y },
        };
        let modifiers = modifiers(scroll.current_event_state());
        s.borrow().report(SurfaceInput::Scroll { delta, modifiers });
        glib::Propagation::Stop
    });
    area.add_controller(scroll);

    let focus = gtk::EventControllerFocus::new();
    let s = Rc::downgrade(state);
    focus.connect_leave(move |_| {
        if let Some(s) = s.upgrade() {
            // Keys held are let go wherever focus went.
            s.borrow_mut().release_keys();
            AreaState::end_grab(&s);
        }
    });
    area.add_controller(focus);
}

/// Whether one of the window's shortcuts (the app's menus', Escape in a
/// dialog) takes this key. Only keys with a modifier: the window's own
/// bindings for plain keys (Tab moving focus) are the surface's to take.
fn window_takes(widget: Option<&gtk::Widget>, event: &gdk::Event, state: gdk::ModifierType) -> bool {
    let held = gdk::ModifierType::CONTROL_MASK
        | gdk::ModifierType::ALT_MASK
        | gdk::ModifierType::SUPER_MASK
        | gdk::ModifierType::META_MASK;
    if !state.intersects(held) {
        return false;
    }
    let Some(root) = widget.and_then(|w| w.root()) else { return false };
    let controllers = root.observe_controllers();
    (0..controllers.n_items()).filter_map(|i| controllers.item(i).and_downcast::<gtk::ShortcutController>()).any(
        |controller| {
            (0..controller.n_items())
                .filter_map(|i| controller.item(i).and_downcast::<gtk::Shortcut>())
                .filter_map(|shortcut| shortcut.trigger())
                .any(|trigger| trigger.trigger(event, false) == gdk::KeyMatch::Exact)
        },
    )
}

/// An event's position in the widget: events come in their surface's
/// coordinates.
fn event_position(area: &gtk::Widget, event: &gdk::Event) -> Option<Point> {
    let (x, y) = event.position()?;
    let native = area.native()?;
    let (tx, ty) = native.surface_transform();
    let p = native.compute_point(area, &graphene::Point::new((x - tx) as f32, (y - ty) as f32))?;
    Some(Point::new(p.x(), p.y()))
}

fn modifiers(state: gdk::ModifierType) -> Modifiers {
    Modifiers {
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        control: state.contains(gdk::ModifierType::CONTROL_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
        meta: state.intersects(gdk::ModifierType::SUPER_MASK | gdk::ModifierType::META_MASK),
    }
}

fn make_native(area: &gtk::DrawingArea, parent: Parent) -> Result<Native, String> {
    let display = area.display();
    match parent {
        Parent::Wayland(_) => {
            // SAFETY: a Wayland window's display is a Wayland display, and
            // stays connected.
            let made = unsafe { Subsurface::new(gdk_wayland_display_get_wl_display(display.to_glib_none().0)) };
            made.map(Native::Wayland)
        }
        Parent::X11(xid) => x11::ChildWindow::new(Some(display.name().as_str()), xid).map(Native::X11),
        Parent::Other => Ok(Native::Other),
    }
}

/// The window's own surface: its `wl_surface` on Wayland, its window on
/// X11.
fn window_surface(widget: &impl IsA<gtk::Widget>) -> Option<Parent> {
    let surface = widget.native()?.surface()?;
    match surface.display().type_().name() {
        "GdkWaylandDisplay" => {
            // SAFETY: a Wayland display's surfaces are Wayland surfaces.
            let parent = unsafe { gdk_wayland_surface_get_wl_surface(surface.to_glib_none().0) };
            (!parent.is_null()).then_some(Parent::Wayland(parent))
        }
        "GdkX11Display" => {
            // SAFETY: an X11 display's surfaces are X11 surfaces.
            let xid = unsafe { gdk_x11_surface_get_xid(surface.to_glib_none().0) };
            (xid != 0).then_some(Parent::X11(xid as u32))
        }
        _ => Some(Parent::Other),
    }
}
