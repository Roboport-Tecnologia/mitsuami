//! `GpuSurface`: a child window (HWND) of the XAML window, over a `Canvas`
//! that keeps the space, which the app presents to (Direct3D, Vulkan).
//! Nothing of XAML draws over a child window. It takes no input
//! (`HTTRANSPARENT`), so XAML keeps the pointer. It's placed before each of
//! XAML's frames (`CompositionTarget.Rendering`), so it follows the canvas
//! wherever layout or scrolling moves it.
//!
//! Input goes through XAML, on the canvas under the child window: with
//! `TakesInput` it's a tab stop with a clear background (hit-testable), a
//! click focuses it, and its key and pointer events are reported. The
//! pointer lock hides the cursor (`ShowCursor`), clips it to the canvas
//! (`ClipCursor`) and puts it back in the middle after each move
//! (`SetCursorPos`), reporting the move. The keyboard grab is a low-level
//! keyboard hook (`WH_KEYBOARD_LL`), as remote desktop clients and browsers
//! take Alt+Tab and the Windows key: while the window is the foreground
//! one and the canvas has focus it swallows every key and reports it.

use std::cell::RefCell;
use std::collections::HashSet;
use std::num::NonZeroIsize;
use std::rc::{Rc, Weak};
use std::sync::Once;

use mitsuami_core::raw_window_handle::{
    HandleError, RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use mitsuami_core::{
    Key, KeyCode, Modifiers, MouseButton, NativeSurface, NodeId, Point, Prop, ScrollDelta, SurfaceHandle, SurfaceInput,
    SurfaceSize, SyntheticInput, UiEvent,
};
use windows_core::{EventRevoker, Interface, PCWSTR, w as wide};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

const CLASS: PCWSTR = wide!("MitsuamiGpuSurface");

thread_local! {
    /// Live surfaces, for their windows' deactivation.
    static HOSTS: RefCell<Vec<Weak<RefCell<HostState>>>> = const { RefCell::new(Vec::new()) };
    /// Surfaces that take input: Tab is theirs while they have focus.
    static TAKES_TAB: RefCell<HashSet<NodeId>> = RefCell::new(HashSet::new());
    /// The surface holding the keyboard grab, and its hook.
    static GRAB: RefCell<Option<(w::HHOOK, Weak<RefCell<HostState>>)>> = const { RefCell::new(None) };
}

/// Whether Tab goes to `focused`, a surface that takes input, rather than
/// moving focus on.
pub(crate) fn takes_tab(focused: Option<NodeId>) -> bool {
    focused.is_some_and(|id| TAKES_TAB.with(|t| t.borrow().contains(&id)))
}

/// A window stopped being the active one: its surfaces' pointer lock and
/// keyboard grab end, as the platform ends them.
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
    pending_lock: bool,
    pending_grab: bool,
    focused: bool,
    /// Keys down while grabbed, by scan code: repeats, and modifiers.
    keys_down: HashSet<u32>,
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
                pending_lock: false,
                pending_grab: false,
                focused: false,
                keys_down: HashSet::new(),
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
        let handle = SurfaceHandle::new(ChildWindow(hwnd as isize));
        state.hwnd = hwnd;
        state.window = window;
        state.child = Some(handle.clone());
        let s = Rc::downgrade(&self.state);
        state.rendering = Some(w::CompositionTarget::Rendering(move |_, _| HostState::place(&s))?);
        state.emitter.emit(state.id, UiEvent::SurfaceReady(handle));
        Ok(())
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
                let code = match key {
                    Key::Char(c) => KeyCode::from_us_char(*c),
                    Key::Enter => KeyCode::Enter,
                    Key::Escape => KeyCode::Escape,
                    Key::Tab => KeyCode::Tab,
                    Key::Backspace => KeyCode::Backspace,
                    Key::Up => KeyCode::ArrowUp,
                    Key::Down => KeyCode::ArrowDown,
                    Key::Home => KeyCode::Home,
                    Key::End => KeyCode::End,
                };
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
    let s = weak();
    revokers.push(element.PointerMoved(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        let handled = with(&s, |state| state.pointer_moved(args).is_ok());
        if handled {
            _ = args.cast::<w::IPointerRoutedEventArgs>().and_then(|a| a.SetHandled(true));
        }
    })?);
    let s = weak();
    revokers.push(element.PointerExited(move |_, _| {
        with(&s, |state| {
            if !state.locked {
                state.report(SurfaceInput::PointerLeft);
            }
            true
        });
    })?);
    for pressed in [true, false] {
        let s = weak();
        let handler = move |_: windows_core::Ref<windows_core::IInspectable>,
                            args: windows_core::Ref<w::PointerRoutedEventArgs>| {
            let Some(args) = args.as_ref() else { return };
            let handled = with(&s, |state| state.button(args, pressed).is_ok());
            if handled {
                _ = args.cast::<w::IPointerRoutedEventArgs>().and_then(|a| a.SetHandled(true));
            }
        };
        revokers.push(if pressed { element.PointerPressed(handler)? } else { element.PointerReleased(handler)? });
    }
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
        {
            state.focused = false;
            // A grab lasts while the surface has focus.
            state.end_grab();
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

    fn pointer_moved(&mut self, args: &w::PointerRoutedEventArgs) -> R<()> {
        let args: w::IPointerRoutedEventArgs = args.cast()?;
        let position = args.GetCurrentPoint(&self.element)?.cast::<w::IPointerPoint>()?.Position()?;
        if self.locked {
            // How far from the middle, in pixels; the warp back to it
            // comes as a move too, of nothing.
            let Some((_, _, width, height)) = self.placed else { return Ok(()) };
            let scale = self.scale();
            let dx = (position.x as f64 * scale).round() as i32 - width / 2;
            let dy = (position.y as f64 * scale).round() as i32 - height / 2;
            if dx != 0 || dy != 0 {
                self.report(SurfaceInput::Motion { dx: (dx as f64 / scale) as f32, dy: (dy as f64 / scale) as f32 });
                self.warp_to_middle();
            }
        } else {
            let position = Point::new(position.x, position.y);
            self.report(SurfaceInput::PointerMoved { position, modifiers: pointer_modifiers(&args) });
        }
        Ok(())
    }

    fn button(&mut self, args: &w::PointerRoutedEventArgs, pressed: bool) -> R<()> {
        let args: w::IPointerRoutedEventArgs = args.cast()?;
        let point = args.GetCurrentPoint(&self.element)?.cast::<w::IPointerPoint>()?;
        let kind = point.Properties()?.cast::<w::IPointerPointProperties>()?.PointerUpdateKind()?;
        let button = match kind {
            w::PointerUpdateKind::LeftButtonPressed | w::PointerUpdateKind::LeftButtonReleased => MouseButton::Primary,
            w::PointerUpdateKind::RightButtonPressed | w::PointerUpdateKind::RightButtonReleased => {
                MouseButton::Secondary
            }
            w::PointerUpdateKind::MiddleButtonPressed | w::PointerUpdateKind::MiddleButtonReleased => {
                MouseButton::Middle
            }
            w::PointerUpdateKind::XButton1Pressed | w::PointerUpdateKind::XButton1Released => MouseButton::Back,
            w::PointerUpdateKind::XButton2Pressed | w::PointerUpdateKind::XButton2Released => MouseButton::Forward,
            _ => MouseButton::Other(kind.0 as u16),
        };
        if pressed {
            let element: w::IUIElement = self.element.cast()?;
            _ = element.Focus(w::FocusState::Pointer);
            // Its moves keep coming while a button is held, wherever the
            // pointer goes; XAML lets go when the button is released.
            _ = element.CapturePointer(&args.Pointer()?);
        }
        let position = Point::new(point.Position()?.x, point.Position()?.y);
        let modifiers = pointer_modifiers(&args);
        self.report(SurfaceInput::Button { button, pressed, position, modifiers });
        Ok(())
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
        self.report(SurfaceInput::Key { code, native, pressed, repeat, modifiers: key_modifiers() });
        Ok(())
    }

    /// Puts the child window over the canvas, or hides it while the canvas
    /// has no size, and reports its size in pixels.
    fn place(this: &Weak<RefCell<HostState>>) {
        let Some(this) = this.upgrade() else { return };
        let Ok(mut state) = this.try_borrow_mut() else { return };
        let Some(handle) = state.child.clone() else { return };
        // The lock holds only while the window is the foreground one.
        if state.locked && unsafe { w::GetForegroundWindow() } != state.window {
            state.end_lock();
        }
        let at = state.rect().ok().flatten();
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
        state.try_pending();
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
        }
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
        self.keys_down.clear();
        GRAB.with(|g| *g.borrow_mut() = Some((hook, self.me.clone())));
    }

    /// Lets go of the keyboard, without a report.
    fn release_grab(&mut self) {
        if !std::mem::take(&mut self.grabbed) {
            return;
        }
        self.keys_down.clear();
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

/// Clicks and the pointer go to XAML's window under it.
unsafe extern "system" fn window_proc(hwnd: w::HWND, message: u32, wparam: w::WPARAM, lparam: w::LPARAM) -> w::LRESULT {
    if message == w::WM_NCHITTEST as u32 {
        return w::HTTRANSPARENT as w::LRESULT;
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

impl NativeSurface for ChildWindow {
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
