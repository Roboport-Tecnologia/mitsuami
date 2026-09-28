//! The C++ layer (`cpp/shim.h`) and safe wrappers around it.
//!
//! Qt objects are [`QmlObject`] handles. Qt calls back into Rust through one
//! function, with a key naming a closure registered here; a connection's
//! closure is forgotten when its object is destroyed.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::fmt;
use std::ptr::NonNull;
use std::rc::Rc;

use mitsuami_core::{Cursor, Point, PointerKind};

type Raw = *mut c_void;

unsafe extern "C" {
    fn mq_init(callback: extern "C" fn(u64, i32, f64, f64));
    fn mq_is_initialized() -> i32;
    fn mq_is_exiting() -> i32;
    fn mq_process_events();
    fn mq_exec();
    fn mq_quit();
    fn mq_watch_loop(key: u64);
    fn mq_wake();
    fn mq_timer_new(key: u64) -> Raw;
    fn mq_timer_start(timer: Raw, ms: i32);
    fn mq_timer_stop(timer: Raw);
    fn mq_set_app_font(family: *const c_char, point_size: f64);
    fn mq_set_color_scheme(path: *const c_char);
    fn mq_device_pixel_ratio() -> f64;

    fn mq_load_in(qml: *const c_char, parent: Raw, error: *mut *mut c_char) -> Raw;
    fn mq_destroy(object: Raw);
    fn mq_delete_later(object: Raw);
    fn mq_find_child(object: Raw, name: *const c_char) -> Raw;
    fn mq_find_by_str(object: Raw, property: *const c_char, value: *const c_char) -> Raw;
    fn mq_set_parent_item(item: Raw, parent: Raw, index: i32);
    fn mq_child_count(item: Raw) -> i32;
    fn mq_child_at(item: Raw, index: i32) -> Raw;
    fn mq_set_geometry(item: Raw, x: f64, y: f64, w: f64, h: f64);
    fn mq_polish_items(window: Raw);
    fn mq_map_to_scene(item: Raw, x: *mut f64, y: *mut f64);
    fn mq_invoke(object: Raw, method: *const c_char) -> i32;
    fn mq_set_node(object: Raw, node: u64);
    fn mq_node_of(object: Raw) -> u64;

    fn mq_set_str(o: Raw, name: *const c_char, value: *const c_char);
    fn mq_get_str(o: Raw, name: *const c_char) -> *mut c_char;
    fn mq_free(s: *mut c_char);
    fn mq_set_bool(o: Raw, name: *const c_char, value: i32);
    fn mq_get_bool(o: Raw, name: *const c_char) -> i32;
    fn mq_set_real(o: Raw, name: *const c_char, value: f64);
    fn mq_get_real(o: Raw, name: *const c_char) -> f64;
    fn mq_set_int(o: Raw, name: *const c_char, value: i32);
    fn mq_get_int(o: Raw, name: *const c_char) -> i32;
    fn mq_set_object(o: Raw, name: *const c_char, value: Raw);
    fn mq_get_object(o: Raw, name: *const c_char) -> Raw;
    fn mq_set_str_list(o: Raw, name: *const c_char, items: *const *const c_char, count: i32);
    fn mq_set_url(o: Raw, name: *const c_char, path: *const c_char);
    fn mq_get_paths(o: Raw, name: *const c_char) -> *mut c_char;
    fn mq_font_px(o: Raw, name: *const c_char) -> f64;

    fn mq_connect(object: Raw, signal: *const c_char, key: u64) -> i32;
    fn mq_watch_close(window: Raw, key: u64);
    fn mq_focus_item(window: Raw) -> Raw;
    fn mq_force_focus(item: Raw);
    fn mq_set_tab_order(window: Raw, items: *const Raw, count: i32);
    fn mq_a11y_action(item: Raw, action: *const c_char) -> i32;
    fn mq_key(window: Raw, key: i32, shift: i32, text: *const c_char);
    fn mq_click(window: Raw, x: f64, y: f64);

    fn mq_drawn_new(key: u64) -> Raw;
    fn mq_drawn_set_ops(item: Raw, ops: *const f32, count: i32);
    fn mq_grab(
        window: Raw,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rgba: *mut *mut u8,
        width: *mut i32,
        height: *mut i32,
        scale: *mut f64,
    ) -> i32;
    fn mq_free_pixels(rgba: *mut u8);
    fn mq_pixels_set(key: u64, rgba: *const u8, width: i32, height: i32);
    fn mq_pixels_remove(key: u64);
    fn mq_set_url_str(o: Raw, name: *const c_char, url: *const c_char);

    fn mq_clipboard_text() -> *mut c_char;
    fn mq_set_clipboard_text(text: *const c_char);

    fn mq_wayland_display() -> Raw;
    fn mq_window_wl_surface(window: Raw) -> Raw;
    fn mq_item_window(item: Raw) -> Raw;
    fn mq_window_dpr(window: Raw) -> f64;
    fn mq_window_margins(window: Raw, left: *mut i32, top: *mut i32);
    fn mq_window_xid(window: Raw) -> u64;
    fn mq_window_keyboard_grab(window: Raw, on: i32) -> i32;
    fn mq_window_active(window: Raw) -> i32;

    fn mq_set_input_callback(callback: extern "C" fn(u64, i32, i32, i32, f64, f64));
    fn mq_surface_input_new(parent: Raw, key: u64) -> Raw;
    fn mq_surface_input_configure(item: Raw, takes: i32, grabbed: i32, locked: i32);
    fn mq_surface_key(window: Raw, key: i32, scan_code: u32, text: *const c_char);
    #[allow(clippy::too_many_arguments)]
    fn mq_surface_input_cursor(
        item: Raw,
        kind: i32,
        rgba: *const u8,
        width: i32,
        height: i32,
        scale: f64,
        hot_x: i32,
        hot_y: i32,
    );
    fn mq_window_states(window: Raw) -> i32;
    fn mq_window_set_states(window: Raw, states: i32);
}

// ------------------------------------------------------------- callbacks

/// What Qt reports to a registered closure.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Callback {
    Signal,
    Pointer(PointerKind, Point),
    Close,
    BeforeWait,
    Timer,
    /// Input on a GPU surface's input item (`MQ_KEY_DOWN`…).
    Input(SurfaceEvent),
}

/// What a GPU surface's input item reports: see `mq_input_callback`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SurfaceEvent {
    pub kind: i32,
    pub code: i32,
    pub flags: i32,
    pub x: f64,
    pub y: f64,
}

type Handler = Rc<dyn Fn(Callback)>;

thread_local! {
    static HANDLERS: RefCell<HashMap<u64, Handler>> = RefCell::new(HashMap::new());
    static NEXT_KEY: Cell<u64> = const { Cell::new(1) };
}

pub(crate) fn register(handler: impl Fn(Callback) + 'static) -> u64 {
    let key = NEXT_KEY.with(|k| k.replace(k.get() + 1));
    HANDLERS.with(|h| h.borrow_mut().insert(key, Rc::new(handler)));
    key
}

pub(crate) fn unregister(key: u64) {
    let _ = HANDLERS.try_with(|h| h.borrow_mut().remove(&key));
}

extern "C" fn dispatch(key: u64, kind: i32, x: f64, y: f64) {
    let point = Point::new(x as f32, y as f32);
    let callback = match kind {
        0 => Callback::Signal,
        1 => return unregister(key),
        2 => Callback::Pointer(PointerKind::Down, point),
        3 => Callback::Pointer(PointerKind::Up, point),
        4 => Callback::Close,
        5 => Callback::BeforeWait,
        6 => Callback::Timer,
        _ => return,
    };
    // Qt may call back while the process tears down, after thread-locals.
    let Ok(Some(handler)) = HANDLERS.try_with(|h| h.borrow().get(&key).cloned()) else { return };
    // Not borrowed while it runs: handlers may connect or disconnect.
    handler(callback);
}

extern "C" fn dispatch_input(key: u64, kind: i32, code: i32, flags: i32, x: f64, y: f64) {
    let Ok(Some(handler)) = HANDLERS.try_with(|h| h.borrow().get(&key).cloned()) else { return };
    handler(Callback::Input(SurfaceEvent { kind, code, flags, x, y }));
}

// ------------------------------------------------------------ application

pub(crate) fn init() {
    unsafe {
        mq_init(dispatch);
        mq_set_input_callback(dispatch_input);
    }
}

pub(crate) fn is_initialized() -> bool {
    unsafe { mq_is_initialized() != 0 }
}

/// The process is exiting: Qt is being torn down, and must not be touched.
pub(crate) fn is_exiting() -> bool {
    unsafe { mq_is_exiting() != 0 }
}

pub(crate) fn process_events() {
    unsafe { mq_process_events() }
}

pub(crate) fn exec() {
    unsafe { mq_exec() }
}

pub(crate) fn quit() {
    unsafe { mq_quit() }
}

/// Calls `f` whenever the event loop is about to sleep.
pub(crate) fn watch_loop(f: impl Fn() + 'static) {
    let key = register(move |_| f());
    unsafe { mq_watch_loop(key) }
}

/// Makes the event loop turn. Callable from any thread.
pub(crate) fn wake() {
    unsafe { mq_wake() }
}

/// A single-shot timer. Stopped and freed when dropped.
pub(crate) struct Timer {
    timer: Raw,
    key: u64,
}

impl Timer {
    pub(crate) fn new(f: impl Fn() + 'static) -> Timer {
        let key = register(move |_| f());
        Timer { timer: unsafe { mq_timer_new(key) }, key }
    }

    pub(crate) fn start(&self, ms: i32) {
        unsafe { mq_timer_start(self.timer, ms) }
    }

    pub(crate) fn stop(&self) {
        unsafe { mq_timer_stop(self.timer) }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        unsafe { mq_destroy(self.timer) };
        unregister(self.key);
    }
}

pub(crate) fn set_app_font(family: &str, point_size: f64) {
    unsafe { mq_set_app_font(c(family).as_ptr(), point_size) }
}

pub(crate) fn set_color_scheme(path: &std::path::Path) {
    unsafe { mq_set_color_scheme(c(&path.to_string_lossy()).as_ptr()) }
}

pub(crate) fn device_pixel_ratio() -> f64 {
    unsafe { mq_device_pixel_ratio() }
}

/// Qt's `wl_display`, on Wayland.
pub(crate) fn wayland_display() -> Option<NonNull<c_void>> {
    NonNull::new(unsafe { mq_wayland_display() })
}

pub(crate) fn clipboard_text() -> Option<String> {
    let text = unsafe { mq_clipboard_text() };
    (!text.is_null()).then(|| owned(text))
}

pub(crate) fn set_clipboard_text(text: &str) {
    unsafe { mq_set_clipboard_text(c(text).as_ptr()) }
}

// ---------------------------------------------------------------- objects

fn c(s: &str) -> CString {
    // Qt strings may hold NULs; C strings can't. Cut there.
    CString::new(s.split('\0').next().unwrap_or_default()).unwrap_or_default()
}

fn owned(s: *mut c_char) -> String {
    let text = unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned();
    unsafe { mq_free(s) };
    text
}

/// A Qt object: a QML item, a window, an action. A handle, not an owner:
/// the backend creates and destroys the objects of its nodes, and a handle
/// is only valid while its object lives.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct QmlObject(NonNull<c_void>);

impl fmt::Debug for QmlObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QmlObject({:p})", self.0)
    }
}

/// Imports every QML snippet gets: Qt Quick, its controls as `QQC2`, its
/// layouts, and Kirigami as `Kirigami`. A snippet may start with more.
pub const IMPORTS: &str =
    "import QtQuick\nimport QtQuick.Controls as QQC2\nimport QtQuick.Layouts\nimport org.kde.kirigami as Kirigami\n";

impl QmlObject {
    fn from_raw(raw: Raw) -> Option<QmlObject> {
        NonNull::new(raw).map(QmlObject)
    }

    fn raw(self) -> Raw {
        self.0.as_ptr()
    }

    /// Creates an object from QML text, with [`IMPORTS`] in front. Each
    /// distinct text is compiled once.
    ///
    /// # Panics
    ///
    /// If the QML doesn't compile or create: that's a bug in the snippet.
    pub fn load(qml: &str) -> QmlObject {
        QmlObject::create(qml, None)
    }

    /// Like [`QmlObject::load`], with `parent` (an item) as the object's
    /// `parent` from the start. Popups (drawers, dialogs) need it: they
    /// evaluate bindings on their parent as they're created.
    pub fn load_in(qml: &str, parent: QmlObject) -> QmlObject {
        QmlObject::create(qml, Some(parent))
    }

    fn create(qml: &str, parent: Option<QmlObject>) -> QmlObject {
        let text = format!("{IMPORTS}{qml}");
        let parent = parent.map_or(std::ptr::null_mut(), QmlObject::raw);
        let mut error = std::ptr::null_mut();
        let object = unsafe { mq_load_in(c(&text).as_ptr(), parent, &mut error) };
        match QmlObject::from_raw(object) {
            Some(object) => object,
            None => {
                let error = if error.is_null() { String::new() } else { owned(error) };
                panic!("mitsuami-kirigami: QML error: {error}\n{qml}")
            }
        }
    }

    pub(crate) fn drawn(key: u64) -> QmlObject {
        QmlObject::from_raw(unsafe { mq_drawn_new(key) }).expect("a new drawn item")
    }

    pub(crate) fn destroy(self) {
        unsafe { mq_destroy(self.raw()) }
    }

    pub(crate) fn delete_later(self) {
        unsafe { mq_delete_later(self.raw()) }
    }

    /// A descendant (QObject child) by `objectName`.
    pub fn child(self, name: &str) -> Option<QmlObject> {
        QmlObject::from_raw(unsafe { mq_find_child(self.raw(), c(name).as_ptr()) })
    }

    /// A descendant (QObject child) whose property reads as `value`, e.g.
    /// the action whose `text` is `"Quit"`.
    pub fn find(self, property: &str, value: &str) -> Option<QmlObject> {
        QmlObject::from_raw(unsafe { mq_find_by_str(self.raw(), c(property).as_ptr(), c(value).as_ptr()) })
    }

    /// Calls a method, signal or slot that takes no arguments.
    pub fn invoke(self, method: &str) -> bool {
        unsafe { mq_invoke(self.raw(), c(method).as_ptr()) != 0 }
    }

    pub fn set_str(self, name: &str, value: &str) {
        unsafe { mq_set_str(self.raw(), c(name).as_ptr(), c(value).as_ptr()) }
    }

    pub fn str(self, name: &str) -> String {
        owned(unsafe { mq_get_str(self.raw(), c(name).as_ptr()) })
    }

    pub fn set_bool(self, name: &str, value: bool) {
        unsafe { mq_set_bool(self.raw(), c(name).as_ptr(), value as i32) }
    }

    pub fn bool(self, name: &str) -> bool {
        unsafe { mq_get_bool(self.raw(), c(name).as_ptr()) != 0 }
    }

    pub fn set_real(self, name: &str, value: f64) {
        unsafe { mq_set_real(self.raw(), c(name).as_ptr(), value) }
    }

    pub fn real(self, name: &str) -> f64 {
        unsafe { mq_get_real(self.raw(), c(name).as_ptr()) }
    }

    pub fn set_int(self, name: &str, value: i32) {
        unsafe { mq_set_int(self.raw(), c(name).as_ptr(), value) }
    }

    pub fn int(self, name: &str) -> i32 {
        unsafe { mq_get_int(self.raw(), c(name).as_ptr()) }
    }

    pub fn set_object(self, name: &str, value: Option<QmlObject>) {
        let value = value.map_or(std::ptr::null_mut(), QmlObject::raw);
        unsafe { mq_set_object(self.raw(), c(name).as_ptr(), value) }
    }

    pub fn object(self, name: &str) -> Option<QmlObject> {
        QmlObject::from_raw(unsafe { mq_get_object(self.raw(), c(name).as_ptr()) })
    }

    pub fn set_str_list(self, name: &str, items: &[String]) {
        let items: Vec<CString> = items.iter().map(|s| c(s)).collect();
        let pointers: Vec<*const c_char> = items.iter().map(|s| s.as_ptr()).collect();
        unsafe { mq_set_str_list(self.raw(), c(name).as_ptr(), pointers.as_ptr(), pointers.len() as i32) }
    }

    pub(crate) fn set_url(self, name: &str, path: &std::path::Path) {
        unsafe { mq_set_url(self.raw(), c(name).as_ptr(), c(&path.to_string_lossy()).as_ptr()) }
    }

    /// Sets a url property from a url, not a path.
    pub(crate) fn set_url_str(self, name: &str, url: &str) {
        unsafe { mq_set_url_str(self.raw(), c(name).as_ptr(), c(url).as_ptr()) }
    }

    /// A `url` or `list<url>` property as local paths.
    pub(crate) fn paths(self, name: &str) -> Vec<std::path::PathBuf> {
        let joined = owned(unsafe { mq_get_paths(self.raw(), c(name).as_ptr()) });
        joined.lines().filter(|l| !l.is_empty()).map(Into::into).collect()
    }

    /// A `font` property's size, in logical pixels.
    pub(crate) fn font_px(self, name: &str) -> f64 {
        unsafe { mq_font_px(self.raw(), c(name).as_ptr()) }
    }

    /// Calls `f` whenever the signal fires, for as long as the object
    /// lives. `signal` is a signature: `"clicked()"`. Returns false if the
    /// object has no such signal.
    pub fn connect(self, signal: &str, f: impl Fn() + 'static) -> bool {
        let key = register(move |_| f());
        let connected = unsafe { mq_connect(self.raw(), c(signal).as_ptr(), key) != 0 };
        if !connected {
            unregister(key);
        }
        connected
    }

    // Items.

    /// Makes `self` the visual child of `parent` at `index` (stacking order),
    /// or detaches it.
    pub(crate) fn set_parent_item(self, parent: Option<QmlObject>, index: usize) {
        let parent = parent.map_or(std::ptr::null_mut(), QmlObject::raw);
        unsafe { mq_set_parent_item(self.raw(), parent, index as i32) }
    }

    /// Visual children, in stacking order.
    pub fn child_items(self) -> Vec<QmlObject> {
        let count = unsafe { mq_child_count(self.raw()) };
        (0..count).filter_map(|i| QmlObject::from_raw(unsafe { mq_child_at(self.raw(), i) })).collect()
    }

    pub(crate) fn set_geometry(self, x: f64, y: f64, width: f64, height: f64) {
        unsafe { mq_set_geometry(self.raw(), x, y, width, height) }
    }

    /// An item's window, once it's in one.
    pub(crate) fn item_window(self) -> Option<QmlObject> {
        QmlObject::from_raw(unsafe { mq_item_window(self.raw()) })
    }

    /// A window's `wl_surface`, on Wayland, while it has one.
    pub(crate) fn wl_surface(self) -> Option<NonNull<c_void>> {
        NonNull::new(unsafe { mq_window_wl_surface(self.raw()) })
    }

    /// A window's scale: device pixels to a logical one.
    pub(crate) fn device_pixel_ratio(self) -> f64 {
        unsafe { mq_window_dpr(self.raw()) }
    }

    /// Where a window's content starts in its surface, past decorations
    /// Qt draws itself.
    pub(crate) fn content_origin(self) -> (i32, i32) {
        let (mut left, mut top) = (0, 0);
        unsafe { mq_window_margins(self.raw(), &mut left, &mut top) };
        (left, top)
    }

    /// A window's XID, on X11.
    pub(crate) fn xid(self) -> Option<u32> {
        match unsafe { mq_window_xid(self.raw()) } {
            0 => None,
            xid => u32::try_from(xid).ok(),
        }
    }

    /// Grabs (or lets go of) the keyboard for a window, on X11; whether it
    /// took.
    pub(crate) fn set_keyboard_grab(self, on: bool) -> bool {
        unsafe { mq_window_keyboard_grab(self.raw(), on as i32) != 0 }
    }

    /// Whether a window is the active one.
    pub(crate) fn is_active(self) -> bool {
        unsafe { mq_window_active(self.raw()) != 0 }
    }

    /// A GPU surface's input item, filling `self` (a focus scope), which
    /// reports to `key`'s closure as [`Callback::Input`].
    pub(crate) fn surface_input(self, key: u64) -> QmlObject {
        QmlObject::from_raw(unsafe { mq_surface_input_new(self.raw(), key) }).expect("an input item")
    }

    pub(crate) fn configure_surface_input(self, takes: bool, grabbed: bool, locked: bool) {
        unsafe { mq_surface_input_configure(self.raw(), takes as i32, grabbed as i32, locked as i32) }
    }

    /// The cursor over a GPU surface's input item.
    pub(crate) fn set_surface_cursor(self, cursor: &Cursor) {
        let raw = self.raw();
        unsafe {
            match cursor {
                Cursor::Default => mq_surface_input_cursor(raw, 0, std::ptr::null(), 0, 0, 1.0, 0, 0),
                Cursor::Hidden => mq_surface_input_cursor(raw, 1, std::ptr::null(), 0, 0, 1.0, 0, 0),
                Cursor::Image { pixels, hotspot } => mq_surface_input_cursor(
                    raw,
                    2,
                    pixels.rgba().as_ptr(),
                    pixels.width() as i32,
                    pixels.height() as i32,
                    pixels.scale_factor() as f64,
                    hotspot.x.round() as i32,
                    hotspot.y.round() as i32,
                ),
            }
        }
    }

    /// A window's `Qt::WindowStates`.
    pub(crate) fn window_states(self) -> i32 {
        unsafe { mq_window_states(self.raw()) }
    }

    pub(crate) fn set_window_states(self, states: i32) {
        unsafe { mq_window_set_states(self.raw(), states) }
    }

    /// A real key press and release with a native scan code, delivered to
    /// the focused item.
    pub(crate) fn surface_key(self, key: i32, scan_code: u32, text: &str) {
        unsafe { mq_surface_key(self.raw(), key, scan_code, c(text).as_ptr()) }
    }

    pub(crate) fn map_to_scene(self, point: Point) -> Point {
        let (mut x, mut y) = (point.x as f64, point.y as f64);
        unsafe { mq_map_to_scene(self.raw(), &mut x, &mut y) };
        Point::new(x as f32, y as f32)
    }

    pub(crate) fn set_node(self, node: u64) {
        unsafe { mq_set_node(self.raw(), node) }
    }

    /// The node of the nearest item up the tree that stands for one.
    pub(crate) fn node(self) -> Option<u64> {
        match unsafe { mq_node_of(self.raw()) } {
            0 => None,
            node => Some(node),
        }
    }

    pub(crate) fn force_focus(self) {
        unsafe { mq_force_focus(self.raw()) }
    }

    /// Runs one of the item's accessible actions (`"Press"`, `"Toggle"`,
    /// `"Increase"`, …), as a screen reader would. False if it has none by
    /// that name.
    pub fn accessible_action(self, action: &str) -> bool {
        unsafe { mq_a11y_action(self.raw(), c(action).as_ptr()) == 0 }
    }

    pub(crate) fn set_drawn_ops(self, ops: &[f32]) {
        unsafe { mq_drawn_set_ops(self.raw(), ops.as_ptr(), ops.len() as i32) }
    }

    // Windows.

    /// Polishes every item of the window now, as Qt does before a frame:
    /// layouts (Kirigami's page stack among them) take their sizes.
    pub(crate) fn polish_items(self) {
        unsafe { mq_polish_items(self.raw()) }
    }

    pub(crate) fn watch_close(self, f: impl Fn() + 'static) {
        let key = register(move |_| f());
        unsafe { mq_watch_close(self.raw(), key) }
    }

    pub(crate) fn focus_item(self) -> Option<QmlObject> {
        QmlObject::from_raw(unsafe { mq_focus_item(self.raw()) })
    }

    pub(crate) fn set_tab_order(self, items: &[QmlObject]) {
        let raw: Vec<Raw> = items.iter().map(|i| i.raw()).collect();
        unsafe { mq_set_tab_order(self.raw(), raw.as_ptr(), raw.len() as i32) }
    }

    /// A real key press and release, delivered to the focused item.
    pub(crate) fn key(self, key: i32, shift: bool, text: &str) {
        unsafe { mq_key(self.raw(), key, shift as i32, c(text).as_ptr()) }
    }

    /// A real primary-button click at a point of the window's scene.
    pub(crate) fn click(self, point: Point) {
        unsafe { mq_click(self.raw(), point.x as f64, point.y as f64) }
    }

    /// Renders the window now; `rect` (logical, scene coordinates) crops.
    pub(crate) fn grab(self, rect: Option<mitsuami_core::Rect>) -> Option<(Vec<u8>, u32, u32, f32)> {
        let (x, y, w, h) =
            rect.map_or((0.0, 0.0, -1.0, -1.0), |r| (r.x() as f64, r.y() as f64, r.width() as f64, r.height() as f64));
        let (mut pixels, mut width, mut height, mut scale) = (std::ptr::null_mut(), 0, 0, 1.0);
        let ok = unsafe { mq_grab(self.raw(), x, y, w, h, &mut pixels, &mut width, &mut height, &mut scale) };
        if ok == 0 {
            return None;
        }
        let len = width as usize * height as usize * 4;
        let rgba = unsafe { std::slice::from_raw_parts(pixels, len) }.to_vec();
        unsafe { mq_free_pixels(pixels) };
        Some((rgba, width as u32, height as u32, scale as f32))
    }
}

/// A JavaScript string literal, for values spliced into QML text.
pub(crate) fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------- pixels

thread_local! {
    static NEXT_PIXELS: Cell<u64> = const { Cell::new(1) };
}

/// Pixels handed to QML's image provider, shown by an `Image` whose source
/// is [`url`](Self::url). Each is stored under a key of its own, so a new
/// one is a new url, which QML loads afresh. Dropped with the handle.
pub(crate) struct ProvidedPixels {
    key: u64,
}

impl ProvidedPixels {
    pub(crate) fn new(pixels: &mitsuami_core::Pixels) -> ProvidedPixels {
        let key = NEXT_PIXELS.with(|k| k.replace(k.get() + 1));
        let (width, height) = (pixels.width() as i32, pixels.height() as i32);
        unsafe { mq_pixels_set(key, pixels.rgba().as_ptr(), width, height) };
        ProvidedPixels { key }
    }

    pub(crate) fn url(&self) -> String {
        format!("image://mitsuami/{}", self.key)
    }
}

impl Drop for ProvidedPixels {
    fn drop(&mut self) {
        // Qt is gone at exit.
        if !is_exiting() {
            unsafe { mq_pixels_remove(self.key) }
        }
    }
}
