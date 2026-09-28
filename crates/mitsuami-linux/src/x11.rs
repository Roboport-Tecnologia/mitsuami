//! X11: a child window of the toolkit's window, on an xcb connection of
//! our own, and the pointer lock, a pointer grab on it, with XInput 2's
//! raw motion for the mouse's moves before the server's acceleration.

use std::ffi::CString;
use std::num::NonZeroU32;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mitsuami_core::raw_window_handle::{
    HandleError, RawDisplayHandle, RawWindowHandle, XcbDisplayHandle, XcbWindowHandle,
};
use mitsuami_core::{Modifiers, MouseButton, NativeSurface, Point, ScrollDelta, SurfaceHandle, SurfaceInput};
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::Event;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ButtonPressEvent, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, EventMask,
    GrabMode, GrabStatus, Gravity, KeyButMask, StackMode, WindowClass,
};
use x11rb::xcb_ffi::XCBConnection;

use crate::{LockEvent, LockSink};

/// A window of our own, a child of the toolkit's window, over the widget.
/// It takes no input (an empty input shape), so the pointer goes to the
/// toolkit's window under it. Clones share it; xcb connections are
/// thread-safe, so it may be used, and dropped, from any thread.
#[derive(Clone)]
pub struct ChildWindow(Arc<Inner>);

struct Inner {
    conn: XCBConnection,
    screen: usize,
    root: u32,
    window: u32,
    /// The toolkit's window it's in, while shown.
    parent: Mutex<Option<u32>>,
    /// Where it was put in its parent, in pixels; `None` while hidden.
    placed: Mutex<Option<(i32, i32, u32, u32)>>,
    /// The pointer lock in effect, by its number; 0 for none.
    lock: AtomicU32,
    locks: AtomicU32,
}

impl ChildWindow {
    /// A window in `parent` (the toolkit's window, by its XID) on the
    /// display the toolkit uses, not shown yet.
    pub fn new(display_name: Option<&str>, parent: u32) -> Result<ChildWindow, String> {
        let name = display_name.map(CString::new).transpose().map_err(|e| e.to_string())?;
        let (conn, screen) = XCBConnection::connect(name.as_deref()).map_err(|e| e.to_string())?;
        let root = conn.setup().roots[screen].root;
        let window = conn.generate_id().map_err(|e| e.to_string())?;
        // No background: until the app presents, what was there shows.
        let aux = CreateWindowAux::new().background_pixmap(x11rb::NONE).bit_gravity(Gravity::NORTH_WEST);
        conn.create_window(0, window, parent, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
            .map_err(|e| e.to_string())?;
        if conn.extension_information(shape::X11_EXTENSION_NAME).ok().flatten().is_some() {
            let _ = conn.shape_rectangles(
                shape::SO::SET,
                shape::SK::INPUT,
                x11rb::protocol::xproto::ClipOrdering::UNSORTED,
                window,
                0,
                0,
                &[],
            );
        }
        conn.flush().map_err(|e| e.to_string())?;
        Ok(ChildWindow(Arc::new(Inner {
            conn,
            screen,
            root,
            window,
            parent: Mutex::new(Some(parent)),
            placed: Mutex::new(None),
            lock: AtomicU32::new(0),
            locks: AtomicU32::new(0),
        })))
    }

    /// What the app gets: it keeps the window alive.
    pub fn handle(&self) -> SurfaceHandle {
        SurfaceHandle::new(self.clone())
    }

    /// Puts it in `parent`, the toolkit's window, if it's elsewhere;
    /// hidden until it's placed.
    pub fn attach(&self, parent: u32) {
        let mut current = self.0.parent.lock().unwrap();
        if *current != Some(parent) {
            *self.0.placed.lock().unwrap() = None;
            let _ = self.0.conn.unmap_window(self.0.window);
            let _ = self.0.conn.reparent_window(self.0.window, parent, 0, 0);
            *current = Some(parent);
            let _ = self.0.conn.flush();
        }
    }

    /// Hides it, and takes it out of the toolkit's window, which would
    /// destroy it with itself: the app may still present to it.
    pub fn detach(&self) {
        *self.0.placed.lock().unwrap() = None;
        let mut parent = self.0.parent.lock().unwrap();
        let _ = self.0.conn.unmap_window(self.0.window);
        if parent.take().is_some() {
            let _ = self.0.conn.reparent_window(self.0.window, self.0.root, 0, 0);
        }
        let _ = self.0.conn.flush();
    }

    /// Where it sits in its parent, and its size, in pixels: hidden while
    /// it has no size. Returns whether that changed.
    pub fn place(&self, x: i32, y: i32, width: i32, height: i32) -> bool {
        if self.0.parent.lock().unwrap().is_none() {
            return false;
        }
        let at = (width > 0 && height > 0).then_some((x, y, width as u32, height as u32));
        let mut placed = self.0.placed.lock().unwrap();
        if *placed == at {
            return false;
        }
        let shown = placed.is_some();
        *placed = at;
        let conn = &self.0.conn;
        match at {
            Some((x, y, width, height)) => {
                let aux = ConfigureWindowAux::new().x(x).y(y).width(width).height(height).stack_mode(StackMode::ABOVE);
                let _ = conn.configure_window(self.0.window, &aux);
                if !shown {
                    let _ = conn.map_window(self.0.window);
                }
            }
            None => {
                let _ = conn.unmap_window(self.0.window);
            }
        }
        let _ = conn.flush();
        true
    }

    /// Grabs the pointer on the window: the cursor hidden, held in the
    /// window's middle, and its moves reported (in points, at `scale`
    /// pixels to a point), with its buttons and wheel, which the grab
    /// takes from the toolkit. A click that asked for it holds the
    /// toolkit's grab until its button is let go, so it's tried until
    /// then. Released when dropped.
    pub fn lock_pointer(&self, scale: f32, sink: LockSink) -> PointerLock {
        let number = self.0.locks.fetch_add(1, Ordering::Relaxed) + 1;
        self.0.lock.store(number, Ordering::Release);
        let inner = self.0.clone();
        let spawned = std::thread::Builder::new()
            .name("mitsuami-x11-lock".into())
            .spawn(move || inner.hold_pointer(number, scale.max(1.0), sink));
        if let Err(problem) = spawned {
            eprintln!("mitsuami: no thread for a pointer lock: {problem}");
        }
        PointerLock { surface: self.clone(), number }
    }
}

impl Inner {
    fn locked(&self, number: u32) -> bool {
        self.lock.load(Ordering::Acquire) == number
    }

    /// The middle of the window, in its own pixels.
    fn middle(&self) -> Option<(i16, i16)> {
        let (_, _, width, height) = (*self.placed.lock().unwrap())?;
        Some(((width / 2) as i16, (height / 2) as i16))
    }

    /// A lock's thread: grabs, then reports until the lock is dropped.
    fn hold_pointer(&self, number: u32, scale: f32, sink: LockSink) {
        let cursor = match self.blank_cursor() {
            Ok(cursor) => cursor,
            Err(_) => return sink(LockEvent::Ended),
        };
        let mask = EventMask::POINTER_MOTION | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE;
        let started = Instant::now();
        loop {
            if !self.locked(number) {
                let _ = self.conn.free_cursor(cursor);
                return;
            }
            let window = self.window;
            let grabbed = self
                .conn
                .grab_pointer(
                    false,
                    window,
                    mask,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                    window,
                    cursor,
                    x11rb::CURRENT_TIME,
                )
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|reply| reply.status);
            match grabbed {
                Some(GrabStatus::SUCCESS) => break,
                Some(GrabStatus::ALREADY_GRABBED | GrabStatus::FROZEN)
                    if started.elapsed() < Duration::from_secs(2) =>
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = self.conn.free_cursor(cursor);
                    let _ = self.conn.flush();
                    return sink(LockEvent::Ended);
                }
            }
        }
        let _ = self.conn.free_cursor(cursor);
        let raw = self.select_raw_motion(true);
        if !raw {
            eprintln!("mitsuami: no XInput 2 on this X server: no raw motion while the pointer is locked");
        }
        self.warp_to_middle();
        let point = |x: i16, y: i16| Point::new(x as f32 / scale, y as f32 / scale);
        loop {
            let Ok(event) = self.conn.wait_for_event() else { return };
            if !self.locked(number) {
                return;
            }
            match event {
                Event::MotionNotify(motion) => {
                    let Some((mx, my)) = self.middle() else { continue };
                    let (dx, dy) = (motion.event_x - mx, motion.event_y - my);
                    // The warp back to the middle comes as a move too.
                    if dx != 0 || dy != 0 {
                        let (dx, dy) = (dx as f32 / scale, dy as f32 / scale);
                        sink(LockEvent::Input(SurfaceInput::Motion { dx, dy }));
                        self.warp_to_middle();
                    }
                }
                Event::XinputRawMotion(raw) => {
                    if let Some((dx, dy)) = raw_motion(&raw) {
                        sink(LockEvent::Input(SurfaceInput::RawMotion { dx, dy }));
                    }
                }
                Event::ButtonPress(press) => {
                    if let Some(input) = button_input(&press, true, point) {
                        sink(LockEvent::Input(input));
                    }
                }
                Event::ButtonRelease(release) => {
                    if let Some(input) = button_input(&release, false, point) {
                        sink(LockEvent::Input(input));
                    }
                }
                _ => {}
            }
        }
    }

    /// Raw motion comes only to the root window, and to a client that
    /// said it speaks XInput 2. Returns whether it's selected (or, turning
    /// it off, was asked to be unselected).
    fn select_raw_motion(&self, on: bool) -> bool {
        if self.conn.extension_information(xinput::X11_EXTENSION_NAME).ok().flatten().is_none() {
            return false;
        }
        if on {
            let version = self.conn.xinput_xi_query_version(2, 0).ok().and_then(|cookie| cookie.reply().ok());
            if version.is_none_or(|v| v.major_version < 2) {
                return false;
            }
        }
        let mask = if on { xinput::XIEventMask::RAW_MOTION } else { xinput::XIEventMask::from(0u32) };
        let masks = [xinput::EventMask { deviceid: xinput::Device::ALL_MASTER.into(), mask: vec![mask] }];
        let selected = self.conn.xinput_xi_select_events(self.root, &masks).is_ok();
        let _ = self.conn.flush();
        selected
    }

    fn warp_to_middle(&self) {
        if let Some((x, y)) = self.middle() {
            let _ = self.conn.warp_pointer(x11rb::NONE, self.window, 0, 0, 0, 0, x, y);
            let _ = self.conn.flush();
        }
    }

    /// An invisible cursor: a one-pixel pixmap with nothing set, as its
    /// image and mask.
    fn blank_cursor(&self) -> Result<u32, x11rb::errors::ReplyOrIdError> {
        let pixmap = self.conn.generate_id()?;
        self.conn.create_pixmap(1, pixmap, self.window, 1, 1)?;
        let cursor = self.conn.generate_id()?;
        self.conn.create_cursor(cursor, pixmap, pixmap, 0, 0, 0, 0, 0, 0, 0, 0)?;
        self.conn.free_pixmap(pixmap)?;
        Ok(cursor)
    }
}

/// The x and y valuators (0 and 1) of a raw motion, unaccelerated: its
/// values come in the order of the valuators its mask has, and an axis it
/// doesn't have didn't move.
fn raw_motion(event: &xinput::RawMotionEvent) -> Option<(f32, f32)> {
    let mask = event.valuator_mask.first().copied().unwrap_or(0);
    let value = |valuator: u32| {
        if mask & (1 << valuator) == 0 {
            return 0.0;
        }
        let index = (mask & ((1 << valuator) - 1)).count_ones() as usize;
        event.axisvalues_raw.get(index).map_or(0.0, |v| v.integral as f64 + v.frac as f64 / 4_294_967_296.0)
    };
    let (dx, dy) = (value(0), value(1));
    (dx != 0.0 || dy != 0.0).then_some((dx as f32, dy as f32))
}

/// A button or a wheel's notch while the pointer is grabbed. X11 has the
/// wheel as buttons 4 to 7, pressed and released at once.
fn button_input(event: &ButtonPressEvent, pressed: bool, point: impl Fn(i16, i16) -> Point) -> Option<SurfaceInput> {
    let modifiers = modifiers(event.state);
    let scroll = |x: f32, y: f32| SurfaceInput::Scroll { delta: ScrollDelta::Lines { x, y }, modifiers };
    let button = match event.detail {
        1 => MouseButton::Primary,
        2 => MouseButton::Middle,
        3 => MouseButton::Secondary,
        4..=7 if !pressed => return None,
        4 => return Some(scroll(0.0, -1.0)),
        5 => return Some(scroll(0.0, 1.0)),
        6 => return Some(scroll(-1.0, 0.0)),
        7 => return Some(scroll(1.0, 0.0)),
        8 => MouseButton::Back,
        9 => MouseButton::Forward,
        n => MouseButton::Other(n as u16),
    };
    Some(SurfaceInput::Button { button, pressed, position: point(event.event_x, event.event_y), modifiers })
}

/// Modifier keys from an X11 event's state: Mod1 is Alt, Mod4 Super, as
/// desktops map them.
pub fn modifiers(state: KeyButMask) -> Modifiers {
    Modifiers {
        shift: state.contains(KeyButMask::SHIFT),
        control: state.contains(KeyButMask::CONTROL),
        alt: state.contains(KeyButMask::MOD1),
        meta: state.contains(KeyButMask::MOD4),
    }
}

/// A pointer grab on a [`ChildWindow`]; released when dropped.
pub struct PointerLock {
    surface: ChildWindow,
    number: u32,
}

impl Drop for PointerLock {
    fn drop(&mut self) {
        let inner = &self.surface.0;
        if inner.lock.compare_exchange(self.number, 0, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return;
        }
        let _ = inner.conn.ungrab_pointer(x11rb::CURRENT_TIME);
        inner.select_raw_motion(false);
        // Wakes the lock's thread, which sees it's no longer locked. An
        // event sent with no mask goes to the window's own client.
        let wake = ClientMessageEvent::new(32, inner.window, AtomEnum::INTEGER, [self.number, 0, 0, 0, 0]);
        let _ = inner.conn.send_event(false, inner.window, EventMask::NO_EVENT, wake);
        let _ = inner.conn.flush();
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.conn.destroy_window(self.window);
        let _ = self.conn.flush();
    }
}

impl NativeSurface for ChildWindow {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError> {
        let window = NonZeroU32::new(self.0.window).ok_or(HandleError::Unavailable)?;
        Ok(RawWindowHandle::Xcb(XcbWindowHandle::new(window)))
    }

    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError> {
        let conn = NonNull::new(self.0.conn.get_raw_xcb_connection());
        Ok(RawDisplayHandle::Xcb(XcbDisplayHandle::new(conn, self.0.screen as i32)))
    }
}
