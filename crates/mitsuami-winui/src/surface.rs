//! `GpuSurface`: a child window (HWND) of the XAML window, over a `Canvas`
//! that keeps the space, which the app presents to (Direct3D, Vulkan).
//! Nothing of XAML draws over a child window. It's placed before each of
//! XAML's frames (`CompositionTarget.Rendering`), so it follows the canvas
//! wherever layout or scrolling moves it.
//!
//! The pointer over it goes to the child window: `HTTRANSPARENT` would pass
//! it to windows under it, but XAML's content window never gets it. With
//! `TakesInput` the child window reports the pointer's moves, buttons and
//! wheel, and holds the pointer while a button is down (`SetCapture`); a
//! click focuses the canvas, a tab stop, and XAML reports its keys. The
//! wheel goes to the focused window when Windows doesn't scroll what's
//! under the pointer; XAML then finds the canvas, with a clear background.
//! The pointer lock hides the cursor (`ShowCursor`), clips it to the canvas
//! (`ClipCursor`) and puts it back in the middle after each move
//! (`SetCursorPos`), reporting the move. Meanwhile the mouse is also
//! registered for Raw Input, to the child window, whose `WM_INPUT` gives
//! the device's own counts before the pointer's acceleration. The keyboard grab is a low-level
//! keyboard hook (`WH_KEYBOARD_LL`), as remote desktop clients and browsers
//! take Alt+Tab and the Windows key: while the window is the foreground
//! one and the canvas has focus it swallows every key and reports it.
//!
//! The app's cursor (none: no cursor) is set on `WM_SETCURSOR`: by the
//! child window over a surface that takes input, and otherwise where XAML
//! sets its own: the XAML window, and the windows in it, are subclassed,
//! and over a surface with a cursor of the app's they set that one instead
//! of letting XAML set its arrow. An `InputCursor` can't be made from
//! pixels, so `UIElement.ProtectedCursor` (set from outside for a table's
//! grippers) only has the system's shapes.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroIsize;
use std::rc::{Rc, Weak};
use std::sync::Once;

use mitsuami_core::raw_window_handle::{
    HandleError, RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use mitsuami_core::{
    Cursor, KeyCode, Modifiers, MouseButton, NativeSurface, NodeId, Pixels, Point, Prop, ScrollDelta, SurfaceHandle,
    SurfaceInput, SurfaceSize, SyntheticInput, UiEvent,
};
use windows_core::{EventRevoker, Interface, PCWSTR, w as wide};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

const CLASS: PCWSTR = wide!("MitsuamiGpuSurface");

thread_local! {
    /// Live surfaces, for their windows' deactivation.
    static HOSTS: RefCell<Vec<Weak<RefCell<HostState>>>> = const { RefCell::new(Vec::new()) };
    /// Surfaces by their child windows, for the pointer's messages.
    static CHILDREN: RefCell<HashMap<isize, Weak<RefCell<HostState>>>> = RefCell::new(HashMap::new());
    /// Surfaces that take input: Tab is theirs while they have focus.
    static TAKES_TAB: RefCell<HashSet<NodeId>> = RefCell::new(HashSet::new());
    /// The surface holding the keyboard grab, and its hook.
    static GRAB: RefCell<Option<(w::HHOOK, Weak<RefCell<HostState>>)>> = const { RefCell::new(None) };
    /// Windows subclassed for their `WM_SETCURSOR`.
    static SUBCLASSED: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
    /// XAML windows showing a `ContentDialog`, whose surfaces hide.
    static COVERED: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
}

/// A `ContentDialog` went up in `window`, or came down. XAML draws nothing
/// over a child window, so the dialog would be under its surfaces, and,
/// modal, leave the window darkened with nothing to answer: they hide
/// while it is up, and come back when it goes.
pub(crate) fn set_covered(window: w::HWND, covered: bool) {
    let changed = COVERED.with(|c| {
        let mut c = c.borrow_mut();
        if covered { c.insert(window as isize) } else { c.remove(&(window as isize)) }
    });
    if !changed {
        return;
    }
    let hosts: Vec<_> = HOSTS.with(|h| h.borrow().iter().filter_map(Weak::upgrade).collect());
    for host in hosts {
        if host.try_borrow().is_ok_and(|s| s.window == window) {
            HostState::place(&Rc::downgrade(&host));
        }
    }
}

/// Whether Tab goes to `focused`, a surface that takes input, rather than
/// moving focus on.
pub(crate) fn takes_tab(focused: Option<NodeId>) -> bool {
    focused.is_some_and(|id| TAKES_TAB.with(|t| t.borrow().contains(&id)))
}

/// A window stopped being the active one: its surfaces' pointer lock and
/// keyboard grab end, as the platform ends them, and keys they have down
/// are let go (their releases go to another window; XAML's focus doesn't
/// move, so `LostFocus` doesn't come).
pub(crate) fn window_deactivated(window: w::HWND) {
    let hosts: Vec<_> = HOSTS.with(|h| {
        let mut hosts = h.borrow_mut();
        hosts.retain(|host| host.strong_count() > 0);
        hosts.iter().filter_map(Weak::upgrade).collect()
    });
    for host in hosts {
        let Ok(mut state) = host.try_borrow_mut() else { continue };
        if state.window == window {
            state.end_lock();
            state.end_grab();
            state.release_keys();
        }
    }
}

/// The canvas that keeps the surface's space, and what it reports.
pub(crate) struct SurfaceHost {
    pub(crate) canvas: w::Canvas,
    state: Rc<RefCell<HostState>>,
}

struct HostState {
    me: Weak<RefCell<HostState>>,
    id: NodeId,
    emitter: Events,
    element: w::UIElement,
    /// Made when the node is first in a window.
    child: Option<SurfaceHandle>,
    hwnd: w::HWND,
    /// The XAML window the child window is in.
    window: w::HWND,
    rendering: Option<EventRevoker>,
    input: Vec<EventRevoker>,
    /// Where it was put, in the window's client pixels; `None` while hidden.
    placed: Option<(i32, i32, i32, i32)>,
    /// What the core set, kept to report it back; `None` until it did.
    takes_input: Option<bool>,
    lock: Option<bool>,
    grab: Option<bool>,
    /// In effect, or wanted until the canvas is laid out in a window.
    locked: bool,
    grabbed: bool,
    /// The mouse is registered for Raw Input, to the child window.
    raw: bool,
    pending_lock: bool,
    pending_grab: bool,
    focused: bool,
    /// Whether the child window asked to hear when the pointer leaves.
    tracking: bool,
    /// Keys down, by scan code: for the grab's repeats and modifiers, and
    /// to let go of them when focus goes.
    keys_down: HashSet<u32>,
    /// The cursor the core set, and the one made from its image, at the
    /// scale it was made for.
    cursor: Option<Cursor>,
    made: Option<(f64, w::HCURSOR)>,
}

impl SurfaceHost {
    pub(crate) fn new(id: NodeId, emitter: Events) -> R<SurfaceHost> {
        let canvas = w::Canvas::new()?;
        let element: w::UIElement = canvas.cast()?;
        let state = Rc::new_cyclic(|me| {
            RefCell::new(HostState {
                me: me.clone(),
                id,
                emitter,
                element: element.clone(),
                child: None,
                hwnd: std::ptr::null_mut(),
                window: std::ptr::null_mut(),
                rendering: None,
                input: Vec::new(),
                placed: None,
                takes_input: None,
                lock: None,
                grab: None,
                locked: false,
                grabbed: false,
                raw: false,
                pending_lock: false,
                pending_grab: false,
                focused: false,
                tracking: false,
                keys_down: HashSet::new(),
                cursor: None,
                made: None,
            })
        });
        let input = listen(&element.cast()?, &state)?;
        state.borrow_mut().input = input;
        HOSTS.with(|h| h.borrow_mut().push(Rc::downgrade(&state)));
        Ok(SurfaceHost { canvas, state })
    }

    /// The node is in `window`: make its child window, once.
    pub(crate) fn attach(&self, window: w::HWND) -> R<()> {
        let mut state = self.state.borrow_mut();
        if state.child.is_some() {
            return Ok(());
        }
        let hwnd = create_child(window)?;
        subclass_tree(window);
        let handle = SurfaceHandle::new(ChildWindow(hwnd as isize));
        state.hwnd = hwnd;
        state.window = window;
        CHILDREN.with(|c| c.borrow_mut().insert(hwnd as isize, Rc::downgrade(&self.state)));
        state.child = Some(handle.clone());
        let s = Rc::downgrade(&self.state);
        state.rendering = Some(w::CompositionTarget::Rendering(move |_, _| HostState::place(&s))?);
        state.emitter.emit(state.id, UiEvent::SurfaceReady(handle));
        Ok(())
    }

    /// Lays out and places the child window now, as XAML's next frame
    /// would.
    pub(crate) fn place_now(&self) {
        let element = self.state.borrow().element.cast::<w::IUIElement>();
        if let Ok(element) = element {
            _ = element.UpdateLayout();
        }
        HostState::place(&Rc::downgrade(&self.state));
    }

    pub(crate) fn is_attached(&self) -> bool {
        self.state.borrow().child.is_some()
    }

    /// The node is gone: stop showing and reporting. The app's handle may
    /// keep the child window, so it leaves the XAML window, which would
    /// destroy it with itself.
    pub(crate) fn detach(&self) {
        let mut state = self.state.borrow_mut();
        state.release_lock();
        state.release_grab();
        state.rendering = None;
        state.input.clear();
        TAKES_TAB.with(|t| t.borrow_mut().remove(&state.id));
        CHILDREN.with(|c| c.borrow_mut().remove(&(state.hwnd as isize)));
        // Gone ones too: every `WM_SETCURSOR` goes through the list.
        let me = state.me.clone();
        HOSTS.with(|h| h.borrow_mut().retain(|host| host.strong_count() > 0 && !host.ptr_eq(&me)));
        if state.child.take().is_some() {
            unsafe {
                _ = w::SetWindowPos(state.hwnd, w::HWND_TOP, 0, 0, 0, 0, w::SWP_HIDEWINDOW as u32);
                w::SetParent(state.hwnd, w::HWND_MESSAGE);
            }
        }
    }

    /// Takes input: a tab stop, and hit-testable (a clear background).
    pub(crate) fn set_takes_input(&self, on: bool) -> R<()> {
        let mut state = self.state.borrow_mut();
        state.takes_input = Some(on);
        TAKES_TAB.with(|t| {
            let mut t = t.borrow_mut();
            if on { t.insert(state.id) } else { t.remove(&state.id) }
        });
        state.element.cast::<w::IUIElement>()?.SetIsTabStop(on)?;
        let panel = self.canvas.cast::<w::IPanel>()?;
        if on {
            let clear = w::SolidColorBrush::CreateInstanceWithColor(w::Color { a: 0, r: 0, g: 0, b: 0 })?;
            panel.SetBackground(&clear)
        } else {
            panel.SetBackground(None::<&w::Brush>)
        }
    }

    pub(crate) fn takes_input(&self) -> bool {
        self.state.borrow().takes_input == Some(true)
    }

    pub(crate) fn set_pointer_lock(&self, on: bool) {
        let mut state = self.state.borrow_mut();
        state.lock = Some(on);
        if on {
            if !state.locked {
                state.pending_lock = true;
                state.try_pending();
            }
        } else {
            state.pending_lock = false;
            state.release_lock();
        }
    }

    /// Shown over the canvas from the pointer's next move; at once if
    /// it's there.
    pub(crate) fn set_cursor(&self, cursor: &Cursor) {
        let mut state = self.state.borrow_mut();
        state.cursor = Some(cursor.clone());
        state.drop_made();
        if let Some(shown) = state.cursor_at(cursor_pos()) {
            unsafe { w::SetCursor(shown) };
        }
    }

    pub(crate) fn set_keyboard_grab(&self, on: bool) {
        let mut state = self.state.borrow_mut();
        state.grab = Some(on);
        if on {
            if !state.grabbed {
                state.pending_grab = true;
                state.try_pending();
            }
        } else {
            state.pending_grab = false;
            state.release_grab();
        }
    }

    /// What the core set that the canvas can't report: whether it takes
    /// input, and the lock and grab in effect (or about to be, until it's
    /// laid out in a window).
    pub(crate) fn props(&self) -> Vec<Prop> {
        let state = self.state.borrow();
        let mut props = Vec::new();
        props.extend(state.takes_input.map(Prop::TakesInput));
        props.extend(state.lock.map(|_| Prop::PointerLock(state.locked || state.pending_lock)));
        props.extend(state.grab.map(|_| Prop::KeyboardGrab(state.grabbed || state.pending_grab)));
        props.extend(state.cursor.clone().map(Prop::Cursor));
        props
    }

    /// Input as a test gives it, reported as XAML's events would be (the
    /// backend focuses the canvas for a click). XAML can't be sent real
    /// pointer input.
    pub(crate) fn synthesize(&self, input: &SyntheticInput) {
        let state = self.state.borrow();
        let modifiers = Modifiers::default();
        match input {
            SyntheticInput::Click(position) => {
                for pressed in [true, false] {
                    let (button, position) = (MouseButton::Primary, *position);
                    state.report(SurfaceInput::Button { button, pressed, position, modifiers });
                }
            }
            SyntheticInput::Key(key) => {
                let code = KeyCode::from_key(*key);
                let native = (1..0x80)
                    .chain(0xE001..0xE080)
                    .find(|scan| KeyCode::from_windows_scancode(*scan) == code)
                    .unwrap_or(0);
                for pressed in [true, false] {
                    state.report(SurfaceInput::Key { code, native, pressed, repeat: false, modifiers });
                }
            }
            SyntheticInput::Scroll { dx, dy } => {
                let delta = ScrollDelta::Points { x: *dx, y: *dy };
                state.report(SurfaceInput::Scroll { delta, modifiers });
            }
            // A surface takes no dropped files, and keys with modifiers
            // aren't simulated on one; the backend refuses them.
            SyntheticInput::Shortcut(_)
            | SyntheticInput::DragFiles(_)
            | SyntheticInput::DragLeave
            | SyntheticInput::DropFiles(_)
            | SyntheticInput::PointerEnter
            | SyntheticInput::PointerLeave
            | SyntheticInput::DoubleClick => {}
        }
    }
}

/// The canvas's input events, reported while it takes input.
fn listen(element: &w::IUIElement, state: &Rc<RefCell<HostState>>) -> R<Vec<EventRevoker>> {
    let mut revokers = Vec::new();
    let weak = || Rc::downgrade(state);
    // Runs `f` on the state if the surface takes input; `f` says whether
    // it handled the event.
    fn with(s: &Weak<RefCell<HostState>>, f: impl FnOnce(&mut HostState) -> bool) -> bool {
        let Some(s) = s.upgrade() else { return false };
        let Ok(mut state) = s.try_borrow_mut() else { return false };
        state.takes_input == Some(true) && f(&mut state)
    }
    // The pointer's other events go to the child window over the canvas.
    let s = weak();
    revokers.push(element.PointerWheelChanged(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        let handled = with(&s, |state| state.wheel(args).is_ok());
        if handled {
            _ = args.cast::<w::IPointerRoutedEventArgs>().and_then(|a| a.SetHandled(true));
        }
    })?);
    // While grabbed, keys the hook didn't take (it takes them only while
    // the window is the foreground one) never reach accelerators.
    let s = weak();
    revokers.push(element.PreviewKeyDown(move |_, args| {
        let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
        if with(&s, |state| state.grabbed && state.key(&args, true).is_ok()) {
            _ = args.SetHandled(true);
        }
    })?);
    for pressed in [true, false] {
        let s = weak();
        let handler = move |_: windows_core::Ref<windows_core::IInspectable>,
                            args: windows_core::Ref<w::KeyRoutedEventArgs>| {
            let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
            // XAML looks for accelerators off the focused element's path
            // (the menus') only while `KeyDown` goes unhandled: keys with
            // Control or Alt are reported and left to them.
            let modifiers = key_modifiers();
            let shortcut = pressed && (modifiers.control || modifiers.alt);
            if with(&s, |state| state.key(&args, pressed).is_ok() && (!shortcut || state.grabbed)) {
                _ = args.SetHandled(true);
            }
        };
        revokers.push(if pressed { element.KeyDown(handler)? } else { element.KeyUp(handler)? });
    }
    let s = weak();
    revokers.push(element.GotFocus(move |_, _| {
        if let Some(s) = s.upgrade()
            && let Ok(mut state) = s.try_borrow_mut()
        {
            state.focused = true;
        }
    })?);
    let s = weak();
    revokers.push(element.LostFocus(move |_, _| {
        if let Some(s) = s.upgrade()
            && let Ok(mut state) = s.try_borrow_mut()
            // XAML raises it too when its island loses Win32 focus (to
            // the window itself, where NVIDIA's overlay puts it), while
            // the canvas stays XAML's focused element; `place` gives the
            // island focus back.
            && !state.is_xaml_focus()
        {
            state.focused = false;
            // A grab lasts while the surface has focus.
            state.end_grab();
            // Keys held are let go: their releases go wherever focus went.
            state.release_keys();
        }
    })?);
    Ok(revokers)
}

impl HostState {
    fn report(&self, input: SurfaceInput) {
        self.emitter.emit(self.id, UiEvent::SurfaceInput(input));
    }

    fn scale(&self) -> f64 {
        self.element.XamlRoot().and_then(|r| r.RasterizationScale()).unwrap_or(1.0)
    }

    /// A pointer message to the child window, reported; `false` if it
    /// isn't one.
    fn mouse(&mut self, message: u32, wparam: w::WPARAM, lparam: w::LPARAM) -> bool {
        // In the child window's pixels; the wheel's are on screen.
        let (x, y) = (lparam as i16 as i32, (lparam >> 16) as i16 as i32);
        let scale = self.scale();
        let position = Point::new((x as f64 / scale) as f32, (y as f64 / scale) as f32);
        let modifiers = key_modifiers();
        let (button, pressed) = match message as i32 {
            w::WM_MOUSEMOVE => {
                if !self.tracking {
                    let mut track = w::TRACKMOUSEEVENT {
                        cbSize: size_of::<w::TRACKMOUSEEVENT>() as u32,
                        dwFlags: w::TME_LEAVE as u32,
                        hwndTrack: self.hwnd,
                        dwHoverTime: 0,
                    };
                    self.tracking = unsafe { w::TrackMouseEvent(&mut track) }.as_bool();
                }
                if self.locked {
                    // How far from the middle; the warp back to it comes as
                    // a move too, of nothing.
                    let Some((_, _, width, height)) = self.placed else { return true };
                    let (dx, dy) = (x - width / 2, y - height / 2);
                    if dx != 0 || dy != 0 {
                        let (dx, dy) = ((dx as f64 / scale) as f32, (dy as f64 / scale) as f32);
                        self.report(SurfaceInput::Motion { dx, dy });
                        self.warp_to_middle();
                    }
                } else {
                    self.report(SurfaceInput::PointerMoved { position, modifiers });
                    // A full-screen menu bar at the top edge.
                    crate::backend::reveal::pointer_moved(self.hwnd);
                }
                return true;
            }
            w::WM_MOUSELEAVE => {
                self.tracking = false;
                if !self.locked {
                    self.report(SurfaceInput::PointerLeft);
                }
                return true;
            }
            w::WM_MOUSEWHEEL | w::WM_MOUSEHWHEEL => {
                // A notch is 120; up (away) is positive, and right is.
                let notches = (wparam >> 16) as i16 as f32 / 120.0;
                let delta = if message as i32 == w::WM_MOUSEHWHEEL {
                    ScrollDelta::Lines { x: notches, y: 0.0 }
                } else {
                    ScrollDelta::Lines { x: 0.0, y: -notches }
                };
                self.report(SurfaceInput::Scroll { delta, modifiers });
                return true;
            }
            w::WM_LBUTTONDOWN => (MouseButton::Primary, true),
            w::WM_LBUTTONUP => (MouseButton::Primary, false),
            w::WM_RBUTTONDOWN => (MouseButton::Secondary, true),
            w::WM_RBUTTONUP => (MouseButton::Secondary, false),
            w::WM_MBUTTONDOWN => (MouseButton::Middle, true),
            w::WM_MBUTTONUP => (MouseButton::Middle, false),
            w::WM_XBUTTONDOWN | w::WM_XBUTTONUP => {
                let back = (wparam >> 16) as u16 as i32 == w::XBUTTON1;
                (if back { MouseButton::Back } else { MouseButton::Forward }, message as i32 == w::WM_XBUTTONDOWN)
            }
            _ => return false,
        };
        let held = w::MK_LBUTTON | w::MK_RBUTTON | w::MK_MBUTTON | w::MK_XBUTTON1 | w::MK_XBUTTON2;
        if pressed {
            if let Ok(element) = self.element.cast::<w::IUIElement>() {
                _ = element.Focus(w::FocusState::Pointer);
            }
            // As a click outside a menu anywhere else closes it.
            crate::backend::reveal::dismiss_menus(self.hwnd);
            // Its moves keep coming while a button is held, wherever the
            // pointer goes.
            unsafe { w::SetCapture(self.hwnd) };
        } else if wparam as i32 & held == 0 {
            unsafe { _ = w::ReleaseCapture() };
        }
        self.report(SurfaceInput::Button { button, pressed, position, modifiers });
        true
    }

    fn wheel(&mut self, args: &w::PointerRoutedEventArgs) -> R<()> {
        let args: w::IPointerRoutedEventArgs = args.cast()?;
        let point = args.GetCurrentPoint(&self.element)?.cast::<w::IPointerPoint>()?;
        let properties = point.Properties()?.cast::<w::IPointerPointProperties>()?;
        // A notch is 120; up (away) is positive, and right is.
        let notches = properties.MouseWheelDelta()? as f32 / 120.0;
        let delta = if properties.IsHorizontalMouseWheel()? {
            ScrollDelta::Lines { x: notches, y: 0.0 }
        } else {
            ScrollDelta::Lines { x: 0.0, y: -notches }
        };
        self.report(SurfaceInput::Scroll { delta, modifiers: pointer_modifiers(&args) });
        Ok(())
    }

    fn key(&mut self, args: &w::IKeyRoutedEventArgs, pressed: bool) -> R<()> {
        let status = args.KeyStatus()?;
        let native = status.scan_code | if status.is_extended_key { 0xE000 } else { 0 };
        let code = KeyCode::from_windows_scancode(native);
        let repeat = pressed && status.was_key_down;
        if pressed {
            self.keys_down.insert(native);
        } else if !self.keys_down.remove(&native) {
            // A release for a key it never saw go down (it went down elsewhere).
            return Ok(());
        }
        self.report(SurfaceInput::Key { code, native, pressed, repeat, modifiers: key_modifiers() });
        Ok(())
    }

    /// Reports every key it has down as released: focus left, or the
    /// window stopped being the active one, so their releases go elsewhere.
    fn release_keys(&mut self) {
        let modifiers = Modifiers::default();
        for native in std::mem::take(&mut self.keys_down) {
            let code = KeyCode::from_windows_scancode(native);
            self.report(SurfaceInput::Key { code, native, pressed: false, repeat: false, modifiers });
        }
    }

    /// The cursor to show at `at` (on screen), if it's over the canvas and
    /// the app has one of its own there: `None` leaves it to XAML, a null
    /// cursor hides it.
    fn cursor_at(&mut self, at: w::POINT) -> Option<w::HCURSOR> {
        let rect = self.screen_rect()?;
        let inside = at.x >= rect.left && at.x < rect.right && at.y >= rect.top && at.y < rect.bottom;
        match self.cursor.clone()? {
            Cursor::Default => None,
            _ if !inside => None,
            Cursor::Hidden => Some(std::ptr::null_mut()),
            Cursor::Image { pixels, hotspot } => {
                let scale = self.scale();
                if self.made.is_none_or(|(made, _)| made != scale) {
                    self.drop_made();
                    self.made = make_cursor(&pixels, hotspot, scale).map(|c| (scale, c));
                }
                self.made.map(|(_, cursor)| cursor)
            }
        }
    }

    fn drop_made(&mut self) {
        if let Some((_, cursor)) = self.made.take() {
            unsafe { _ = w::DestroyIcon(cursor) };
        }
    }

    /// Puts the child window over the canvas, or hides it while the canvas
    /// has no size or a dialog covers its window, and reports its size in
    /// pixels.
    fn place(this: &Weak<RefCell<HostState>>) {
        let Some(this) = this.upgrade() else { return };
        let Ok(mut state) = this.try_borrow_mut() else { return };
        let Some(handle) = state.child.clone() else { return };
        // The lock holds only while the window is the foreground one.
        if state.locked && unsafe { w::GetForegroundWindow() } != state.window {
            state.end_lock();
        }
        // Keys go to the window with Win32 focus, which XAML keeps on its
        // content island. NVIDIA's overlay, which loads into apps that
        // present, leaves it on the XAML window itself, where keys reach
        // nothing, until the window is activated again and XAML moves it
        // back: move it back now.
        if unsafe { w::GetFocus() } == state.window {
            _ = state.focus_island();
        }
        let covered = COVERED.with(|c| c.borrow().contains(&(state.window as isize)));
        let at = if covered { None } else { state.rect().ok().flatten() };
        if at != state.placed {
            state.placed = at;
            let flags = (w::SWP_NOACTIVATE | if at.is_some() { w::SWP_SHOWWINDOW } else { w::SWP_HIDEWINDOW }) as u32;
            let (x, y, width, height) = at.unwrap_or_default();
            unsafe { _ = w::SetWindowPos(state.hwnd, w::HWND_TOP, x, y, width, height, flags) };
            if let Some((_, _, width, height)) = at {
                let scale = state.scale();
                let size = SurfaceSize { width: width as u32, height: height as u32, scale: scale as f32 };
                if handle.set_size(size) {
                    state.emitter.emit(state.id, UiEvent::SurfaceResized(size));
                }
            }
            match at {
                // Hidden, it can't hold the pointer.
                None => state.end_lock(),
                Some(_) if state.locked => state.clip(),
                Some(_) => {}
            }
        }
        state.raise();
        state.try_pending();
    }

    /// Whether the canvas is XAML's focused element.
    fn is_xaml_focus(&self) -> bool {
        let focused = self.element.XamlRoot().and_then(|r| w::FocusManager::GetFocusedElementWithRoot(&r));
        let Ok(focused) = focused.and_then(|f| f.cast::<windows_core::IUnknown>()) else { return false };
        // COM identity: the same object answers the same `IUnknown`.
        self.element.cast::<windows_core::IUnknown>().is_ok_and(|e| e.as_raw() == focused.as_raw())
    }

    /// Gives Win32 focus to XAML's content island, which gives it back to
    /// its focused element.
    fn focus_island(&self) -> R<bool> {
        let island = self.element.XamlRoot()?.cast::<w::IXamlRoot4>()?.ContentIsland()?;
        w::InputFocusController::GetForIsland(&island)?.cast::<w::IInputFocusController>()?.TrySetFocus()
    }

    /// Makes the lock and grab wanted once the canvas is laid out in a
    /// window, or reports they can't be.
    fn try_pending(&mut self) {
        if self.child.is_none() || self.placed.is_none() {
            return;
        }
        let foreground = unsafe { w::GetForegroundWindow() } == self.window;
        if std::mem::take(&mut self.pending_lock) {
            if foreground {
                self.locked = true;
                unsafe { w::ShowCursor(false.into()) };
                self.clip();
                self.warp_to_middle();
                self.register_raw(true);
                self.raise();
                // The pointer can't go to an open menu any more.
                crate::backend::reveal::dismiss_menus(self.hwnd);
            } else {
                self.emitter.emit(self.id, UiEvent::PointerLockEnded);
            }
        }
        if std::mem::take(&mut self.pending_grab) {
            let focused = foreground
                && self
                    .element
                    .cast::<w::IUIElement>()
                    .and_then(|e| e.Focus(w::FocusState::Programmatic))
                    .unwrap_or(false);
            if focused {
                self.focused = true;
                self.hook();
                self.raise();
            } else {
                self.emitter.emit(self.id, UiEvent::KeyboardGrabEnded);
            }
        }
    }

    /// The canvas on screen, in pixels.
    fn screen_rect(&self) -> Option<w::RECT> {
        let (x, y, width, height) = self.placed?;
        let mut origin = w::POINT { x, y };
        unsafe { _ = w::ClientToScreen(self.window, &mut origin) };
        Some(w::RECT { left: origin.x, top: origin.y, right: origin.x + width, bottom: origin.y + height })
    }

    fn clip(&self) {
        if let Some(rect) = self.screen_rect() {
            unsafe { _ = w::ClipCursor(&rect) };
        }
    }

    /// Puts the child window back above its siblings. XAML raises its
    /// title bar's input window (`InputNonClientPointerSource`) when the
    /// window is activated, which a lock does; in full screen that window
    /// still spans the title bar's old strip across the top, over the
    /// surface, and took the pointer and the clicks there.
    fn raise(&self) {
        if self.placed.is_some() && unsafe { w::GetWindow(self.window, w::GW_CHILD as u32) } != self.hwnd {
            let flags = (w::SWP_NOMOVE | w::SWP_NOSIZE | w::SWP_NOACTIVATE) as u32;
            unsafe { _ = w::SetWindowPos(self.hwnd, w::HWND_TOP, 0, 0, 0, 0, flags) };
        }
    }

    fn warp_to_middle(&self) {
        if let Some(rect) = self.screen_rect() {
            let (x, y) = (rect.left + (rect.right - rect.left) / 2, rect.top + (rect.bottom - rect.top) / 2);
            unsafe { _ = w::SetCursorPos(x, y) };
        }
    }

    /// Lets go of the pointer, without a report: the app asked, or the
    /// node went.
    fn release_lock(&mut self) {
        if std::mem::take(&mut self.locked) {
            unsafe {
                _ = w::ClipCursor(std::ptr::null());
                w::ShowCursor(true.into());
            }
            self.register_raw(false);
            self.raise();
        }
    }

    /// Registers the mouse (generic desktop page, mouse usage) for Raw
    /// Input to the child window, or takes it off. A process has one
    /// registration per device kind, and only the lock makes it. Without
    /// `RIDEV_INPUTSINK`: the lock only holds while the window is the
    /// foreground one. The legacy mouse messages XAML reads still come.
    fn register_raw(&mut self, on: bool) {
        if on == self.raw || (on && self.hwnd.is_null()) {
            return;
        }
        let device = w::RAWINPUTDEVICE {
            usUsagePage: 0x01,
            usUsage: 0x02,
            dwFlags: if on { 0 } else { w::RIDEV_REMOVE as u32 },
            // Taking it off names no window.
            hwndTarget: if on { self.hwnd } else { std::ptr::null_mut() },
        };
        let done = unsafe { w::RegisterRawInputDevices(&device, 1, size_of::<w::RAWINPUTDEVICE>() as u32) };
        self.raw = on && done.as_bool();
    }

    /// A mouse's `WM_INPUT` while locked: its relative move, in counts.
    /// Absolute moves (remote desktop, tablets, some virtual machines'
    /// mice) have no counts to give, so they're left to `Motion`.
    fn raw_input(&self, input: w::HRAWINPUT) {
        let mut raw = w::RAWINPUT::default();
        let mut size = size_of::<w::RAWINPUT>() as u32;
        let header = size_of::<w::RAWINPUTHEADER>() as u32;
        let read = unsafe {
            w::GetRawInputData(input, w::RID_INPUT as u32, (&mut raw as *mut w::RAWINPUT).cast(), &mut size, header)
        };
        if read == u32::MAX || raw.header.dwType != w::RIM_TYPEMOUSE as u32 {
            return;
        }
        // SAFETY: a mouse's input, as its header says.
        let mouse = unsafe { raw.data.mouse };
        if mouse.usFlags & w::MOUSE_MOVE_ABSOLUTE as u16 != 0 || (mouse.lLastX == 0 && mouse.lLastY == 0) {
            return;
        }
        self.report(SurfaceInput::RawMotion { dx: mouse.lLastX as f32, dy: mouse.lLastY as f32 });
    }

    /// The platform ended the lock.
    fn end_lock(&mut self) {
        if self.locked {
            self.release_lock();
            self.emitter.emit(self.id, UiEvent::PointerLockEnded);
        }
    }

    /// Installs the keyboard hook, taking the grab from another surface.
    fn hook(&mut self) {
        let other = GRAB.with(|g| g.borrow().as_ref().and_then(|(_, s)| s.upgrade()));
        if let Some(other) = other
            && let Ok(mut other) = other.try_borrow_mut()
        {
            other.end_grab();
        }
        let hook = unsafe {
            w::SetWindowsHookExW(w::WH_KEYBOARD_LL, Some(keyboard_hook), w::GetModuleHandleW(PCWSTR::null()), 0)
        };
        if hook.is_null() {
            self.emitter.emit(self.id, UiEvent::KeyboardGrabEnded);
            return;
        }
        self.grabbed = true;
        GRAB.with(|g| *g.borrow_mut() = Some((hook, self.me.clone())));
    }

    /// Lets go of the keyboard, without a report.
    fn release_grab(&mut self) {
        if !std::mem::take(&mut self.grabbed) {
            return;
        }
        if let Some((hook, _)) = GRAB.with(|g| g.borrow_mut().take()) {
            unsafe { _ = w::UnhookWindowsHookEx(hook) };
        }
    }

    /// The platform ended the grab.
    fn end_grab(&mut self) {
        if self.grabbed {
            self.release_grab();
            self.emitter.emit(self.id, UiEvent::KeyboardGrabEnded);
        }
    }

    /// A key the hook took: reported, with its repeat and the modifiers
    /// held, from the keys it saw go down.
    fn hooked_key(&mut self, key: &w::KBDLLHOOKSTRUCT) {
        let native = key.scanCode | if key.flags & w::LLKHF_EXTENDED as u32 != 0 { 0xE000 } else { 0 };
        let pressed = key.flags & w::LLKHF_UP as u32 == 0;
        let repeat = pressed && !self.keys_down.insert(native);
        if !pressed {
            self.keys_down.remove(&native);
        }
        let held = |codes: [KeyCode; 2]| {
            self.keys_down.iter().any(|scan| codes.contains(&KeyCode::from_windows_scancode(*scan)))
        };
        let modifiers = Modifiers {
            shift: held([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            control: held([KeyCode::ControlLeft, KeyCode::ControlRight]),
            alt: held([KeyCode::AltLeft, KeyCode::AltRight]),
            meta: held([KeyCode::MetaLeft, KeyCode::MetaRight]),
        };
        let code = KeyCode::from_windows_scancode(native);
        self.report(SurfaceInput::Key { code, native, pressed, repeat, modifiers });
    }

    /// The canvas in the window's client area, in pixels; `None` while it
    /// isn't laid out or has no size. XAML's root fills the client area.
    fn rect(&self) -> R<Option<(i32, i32, i32, i32)>> {
        let fe: w::IFrameworkElement = self.element.cast()?;
        let (width, height) = (fe.ActualWidth()?, fe.ActualHeight()?);
        if !fe.IsLoaded()? || width <= 0.0 || height <= 0.0 {
            return Ok(None);
        }
        let transform = self.element.cast::<w::IUIElement>()?.TransformToVisual(None::<&w::UIElement>)?;
        let origin = transform.cast::<w::IGeneralTransform>()?.TransformPoint(w::Point { x: 0.0, y: 0.0 })?;
        let scale = self.scale();
        let px = |v: f64| (v * scale).round() as i32;
        Ok(Some((px(origin.x as f64), px(origin.y as f64), px(width), px(height))))
    }
}

/// The low-level keyboard hook of a grab: every key, before the system's
/// shortcuts and the window's, while the window is the foreground one and
/// the surface has focus.
unsafe extern "system" fn keyboard_hook(code: i32, wparam: w::WPARAM, lparam: w::LPARAM) -> w::LRESULT {
    const HC_ACTION: i32 = 0;
    if code == HC_ACTION {
        let host = GRAB.with(|g| g.borrow().as_ref().and_then(|(_, s)| s.upgrade()));
        if let Some(host) = host
            && let Ok(mut state) = host.try_borrow_mut()
            && state.grabbed
            && state.focused
            && unsafe { w::GetForegroundWindow() } == state.window
        {
            // SAFETY: a low-level keyboard hook's `lparam` is its event.
            let key = unsafe { &*(lparam as *const w::KBDLLHOOKSTRUCT) };
            state.hooked_key(key);
            return 1;
        }
    }
    unsafe { w::CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// Modifier keys held with a pointer event.
fn pointer_modifiers(args: &w::IPointerRoutedEventArgs) -> Modifiers {
    let held = args.KeyModifiers().unwrap_or(w::VirtualKeyModifiers::None).0;
    Modifiers {
        shift: held & w::VirtualKeyModifiers::Shift.0 != 0,
        control: held & w::VirtualKeyModifiers::Control.0 != 0,
        alt: held & w::VirtualKeyModifiers::Menu.0 != 0,
        meta: held & w::VirtualKeyModifiers::Windows.0 != 0,
    }
}

/// Modifier keys held with a key event, as the thread's key state has
/// them.
fn key_modifiers() -> Modifiers {
    let down = |key: i32| unsafe { w::GetKeyState(key) } < 0;
    Modifiers {
        shift: down(w::VK_SHIFT),
        control: down(w::VK_CONTROL),
        alt: down(w::VK_MENU),
        meta: down(w::VK_LWIN) || down(w::VK_RWIN),
    }
}

impl Drop for HostState {
    fn drop(&mut self) {
        self.drop_made();
    }
}

fn cursor_pos() -> w::POINT {
    let mut at = w::POINT { x: i32::MIN, y: i32::MIN };
    unsafe { _ = w::GetCursorPos(&mut at) };
    at
}

/// A cursor from the app's image, drawn at the display's scale (nearest
/// pixel), its hotspot in points scaled with it. Straight alpha, as a
/// 32-bit cursor's colour bitmap has it; the mask is unused.
fn make_cursor(pixels: &Pixels, hotspot: Point, scale: f64) -> Option<w::HCURSOR> {
    // No pixels to sample: XAML's cursor stays, as GTK shows none of its
    // own. Sampling an empty image panicked inside `WM_SETCURSOR`.
    if pixels.width() == 0 || pixels.height() == 0 {
        return None;
    }
    let size = pixels.size();
    let width = ((size.width as f64 * scale).round() as i32).max(1);
    let height = ((size.height as f64 * scale).round() as i32).max(1);
    let info = w::BITMAPINFO {
        bmiHeader: w::BITMAPINFOHEADER {
            biSize: size_of::<w::BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Top-down rows, as the app's are.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: w::BI_RGB as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let colour = unsafe {
        w::CreateDIBSection(std::ptr::null_mut(), &info, w::DIB_RGB_COLORS as u32, &mut bits, std::ptr::null_mut(), 0)
    };
    if colour.is_null() || bits.is_null() {
        return None;
    }
    // SAFETY: the section holds `width × height` 32-bit pixels. Counted
    // in `usize`: in `i32` a large image's count wrapped.
    let len = width as usize * height as usize * 4;
    let out = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), len) };
    let (from_w, from_h) = (pixels.width() as usize, pixels.height() as usize);
    let rgba = pixels.rgba();
    for y in 0..height as usize {
        for x in 0..width as usize {
            let from = ((y * from_h / height as usize) * from_w + x * from_w / width as usize) * 4;
            let to = (y * width as usize + x) * 4;
            out[to..to + 4].copy_from_slice(&[rgba[from + 2], rgba[from + 1], rgba[from], rgba[from + 3]]);
        }
    }
    let mask_row = (width as usize).div_ceil(16) * 2;
    let zeros = vec![0u8; mask_row * height as usize];
    let mask = unsafe { w::CreateBitmap(width, height, 1, 1, zeros.as_ptr().cast()) };
    let spot = |v: f32, max: i32| ((v as f64 * scale).round() as i32).clamp(0, max - 1) as u32;
    let icon = w::ICONINFO {
        fIcon: false.into(),
        xHotspot: spot(hotspot.x, width),
        yHotspot: spot(hotspot.y, height),
        hbmMask: mask,
        hbmColor: colour,
    };
    let cursor = unsafe { w::CreateIconIndirect(&icon) };
    unsafe {
        _ = w::DeleteObject(colour);
        _ = w::DeleteObject(mask);
    }
    (!cursor.is_null()).then_some(cursor)
}

/// Subclasses a XAML window and the windows in it (not ours), for their
/// `WM_SETCURSOR`.
fn subclass_tree(window: w::HWND) {
    unsafe extern "system" fn each(hwnd: w::HWND, _: w::LPARAM) -> windows_core::BOOL {
        subclass(hwnd);
        true.into()
    }
    subclass(window);
    unsafe { _ = w::EnumChildWindows(window, Some(each), 0) };
}

fn subclass(hwnd: w::HWND) {
    if hwnd.is_null() || SUBCLASSED.with(|s| s.borrow().contains(&(hwnd as isize))) {
        return;
    }
    let mut class = [0u16; 32];
    let len = unsafe { w::GetClassNameW(hwnd, windows_core::PWSTR(class.as_mut_ptr()), class.len() as i32) };
    if class[..len.max(0) as usize] == *unsafe { CLASS.as_wide() } {
        return;
    }
    if unsafe { w::SetWindowSubclass(hwnd, Some(set_cursor_proc), 0, 0) }.as_bool() {
        SUBCLASSED.with(|s| s.borrow_mut().insert(hwnd as isize));
    }
}

/// Over a surface with a cursor of the app's, sets it rather than letting
/// XAML set its own. A window asks its parent first only in
/// `DefWindowProc`, which XAML's input window doesn't reach: each is
/// subclassed.
unsafe extern "system" fn set_cursor_proc(
    hwnd: w::HWND,
    message: u32,
    wparam: w::WPARAM,
    lparam: w::LPARAM,
    _: usize,
    _: usize,
) -> w::LRESULT {
    if message == w::WM_NCDESTROY as u32 {
        unsafe { _ = w::RemoveWindowSubclass(hwnd, Some(set_cursor_proc), 0) };
        SUBCLASSED.with(|s| s.borrow_mut().remove(&(hwnd as isize)));
    } else if message == w::WM_SETCURSOR as u32 && (lparam & 0xFFFF) as i32 == w::HTCLIENT {
        let root = unsafe { w::GetAncestor(hwnd, w::GA_ROOT as u32) };
        let at = cursor_pos();
        let hosts: Vec<_> = HOSTS.with(|h| h.borrow().iter().filter_map(Weak::upgrade).collect());
        for host in hosts {
            let Ok(mut state) = host.try_borrow_mut() else { continue };
            if state.window == root
                && let Some(cursor) = state.cursor_at(at)
            {
                unsafe { w::SetCursor(cursor) };
                return 1;
            }
        }
    }
    unsafe { w::DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Registers the child windows' class, once.
fn register_class() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| unsafe {
        let class = w::WNDCLASSEXW {
            cbSize: size_of::<w::WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: w::GetModuleHandleW(PCWSTR::null()),
            lpszClassName: CLASS,
            ..Default::default()
        };
        w::RegisterClassExW(&class);
    });
}

/// The pointer over a surface that takes input is reported, under the
/// app's cursor; over one that doesn't, it goes to the windows under it.
/// The locked mouse's Raw Input comes here, and goes on to
/// `DefWindowProc`, which frees it.
unsafe extern "system" fn window_proc(hwnd: w::HWND, message: u32, wparam: w::WPARAM, lparam: w::LPARAM) -> w::LRESULT {
    let host = CHILDREN.with(|c| c.borrow().get(&(hwnd as isize)).and_then(Weak::upgrade));
    let takes_input = host.as_ref().is_some_and(|h| h.try_borrow().is_ok_and(|s| s.takes_input == Some(true)));
    if !takes_input {
        if message == w::WM_NCHITTEST as u32 {
            return w::HTTRANSPARENT as w::LRESULT;
        }
    } else if message == w::WM_SETCURSOR as u32 && lparam as u16 as i32 == w::HTCLIENT {
        let app = host.as_ref().and_then(|h| h.try_borrow_mut().ok()?.cursor_at(cursor_pos()));
        let cursor = app.unwrap_or_else(|| unsafe { w::LoadCursorW(std::ptr::null_mut(), w::IDC_ARROW) });
        unsafe { w::SetCursor(cursor) };
        return 1;
    } else if let Some(host) = &host
        && let Ok(mut state) = host.try_borrow_mut()
        && state.mouse(message, wparam, lparam)
    {
        // The X buttons' messages answer `TRUE`.
        return (message == w::WM_XBUTTONDOWN as u32 || message == w::WM_XBUTTONUP as u32) as w::LRESULT;
    }
    // The surface found above: a locked mouse sends up to a thousand a
    // second.
    if message == w::WM_INPUT as u32
        && let Some(host) = &host
        && let Ok(state) = host.try_borrow()
        && state.locked
    {
        state.raw_input(lparam as w::HRAWINPUT);
    }
    unsafe { w::DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn create_child(parent: w::HWND) -> R<w::HWND> {
    register_class();
    let hwnd = unsafe {
        w::CreateWindowExW(
            0,
            CLASS,
            PCWSTR::null(),
            (w::WS_CHILD | w::WS_CLIPSIBLINGS) as u32,
            0,
            0,
            0,
            0,
            parent,
            std::ptr::null_mut(),
            w::GetModuleHandleW(PCWSTR::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() { Err(windows_core::Error::from_thread()) } else { Ok(hwnd) }
}

/// The app's share of the child window. Windows are destroyed on their own
/// thread: the last handle asks it to close, from wherever it's dropped.
struct ChildWindow(isize);

// SAFETY: the child window leaves its window for a message-only parent
// when its widget goes (`detach`), and is closed only once this is dropped.
unsafe impl NativeSurface for ChildWindow {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError> {
        let mut handle = Win32WindowHandle::new(NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?);
        handle.hinstance = NonZeroIsize::new(unsafe { w::GetModuleHandleW(PCWSTR::null()) } as isize);
        Ok(RawWindowHandle::Win32(handle))
    }

    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError> {
        Ok(RawDisplayHandle::Windows(WindowsDisplayHandle::new()))
    }
}

impl Drop for ChildWindow {
    fn drop(&mut self) {
        unsafe { _ = w::PostMessageW(self.0 as w::HWND, w::WM_CLOSE as u32, 0, 0) };
    }
}
