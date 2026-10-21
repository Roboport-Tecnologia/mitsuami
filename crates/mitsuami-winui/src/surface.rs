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
//!
//! Text comes through the same subclass, from the window with Win32 focus
//! (XAML's content island): `TranslateMessage` queues a key's `WM_CHAR`s
//! before the key is dispatched, so at its `WM_KEYDOWN` they're taken off
//! the queue (as winit does) and go out with the key, which XAML reports
//! as the canvas's `PreviewKeyDown` while the message is handled. Input
//! methods compose in a transparent text box in the canvas, which has
//! XAML's focus for it: XAML gives them a text services document only for
//! its text controls, and on the canvas alone they took no keys. What they
//! commit to it is the surface's text, and the box sits at the app's caret,
//! where they open their windows. Keys never reach the box. Characters no
//! key down came with (`WM_CHAR`s of their own) are committed text too.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroIsize;
use std::rc::{Rc, Weak};
use std::sync::Once;

use mitsuami_core::backend::Key;
use mitsuami_core::raw_window_handle::{
    HandleError, RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use mitsuami_core::{
    Cursor, KeyCode, Modifiers, MouseButton, NativeSurface, NodeId, Pixels, Point, Prop, Rect, ScrollDelta,
    SurfaceHandle, SurfaceInput, SurfaceSize, SyntheticInput, SystemCursor, UiEvent,
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
    /// What the core set: whether it takes text, whether input methods
    /// compose in it, and where its caret is.
    text_input: Option<bool>,
    input_method: Option<bool>,
    text_cursor: Option<Option<Rect>>,
    /// What the key going down typed: its `WM_CHAR`s, taken off the queue
    /// before XAML reports the key.
    typed: Option<String>,
    /// The accent of a dead key going down (its `WM_DEADCHAR`), reported
    /// as the preedit after the key.
    dead: Option<char>,
    /// A dead key's accent is the preedit, until the next key composes it.
    dead_shown: bool,
    /// A `WM_CHAR`'s high surrogate, waiting for the low one.
    surrogate: Option<u16>,
    /// While it takes text, a text box in the canvas, under the surface,
    /// that has XAML's focus for it: XAML gives input methods a document
    /// only for its text controls, so they compose in this one, and what
    /// they commit to it is the surface's text. Keys never reach it.
    text_box: Option<TextBox>,
}

/// The text box an input method composes in, and its events.
struct TextBox {
    control: w::TextBox,
    /// A composition is going on: what the box holds is its preedit.
    composing: Cell<bool>,
    /// The preedit last reported, so an unchanged one isn't again.
    preedit: RefCell<(String, Option<usize>)>,
    _events: Vec<EventRevoker>,
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
                text_input: None,
                input_method: None,
                text_cursor: None,
                typed: None,
                dead: None,
                dead_shown: false,
                surrogate: None,
                text_box: None,
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

    /// Places the child window at the frame just set, without waiting for
    /// XAML's next frame, so the app hears of its new size at once.
    pub(crate) fn follow_frame(&self) {
        HostState::place(&Rc::downgrade(&self.state));
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
        // The text box goes now, while XAML runs, out of the canvas: kept to the
        // end, input methods' text services failed fast in `CoreUIComponents`
        // (`0xc0000602`) as the app quit, seconds after its window closed.
        if let Some(text_box) = state.text_box.take() {
            if let (Ok(panel), Ok(control)) =
                (state.element.cast::<w::IPanel>(), text_box.control.cast::<w::UIElement>())
                && let Ok(children) = panel.Children()
            {
                let mut at = 0;
                if children.IndexOf(&control, &mut at).unwrap_or(false) {
                    _ = children.RemoveAt(at);
                }
            }
            drop(text_box);
        }
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

    /// Takes text: its keys report what they type, and the input method
    /// follows its caret.
    pub(crate) fn set_text_input(&self, on: bool) {
        self.state.borrow_mut().text_input = Some(on);
        self.sync_text();
    }

    /// Whether input methods compose in it while it takes text: off, the
    /// canvas has focus rather than its text box, and keys type as they
    /// would with no input method.
    pub(crate) fn set_input_method(&self, on: bool) {
        self.state.borrow_mut().input_method = Some(on);
        self.sync_text();
    }

    /// The text box comes with text input, and has focus, rather than the
    /// canvas, while input methods compose in it.
    fn sync_text(&self) {
        let mut state = self.state.borrow_mut();
        if state.text_input == Some(true) && state.text_box.is_none() {
            match make_text_box(&self.state, &state.element) {
                Ok(text_box) => state.text_box = Some(text_box),
                Err(e) => eprintln!("mitsuami: a GpuSurface's text box: {e}"),
            }
        }
        // Focus moves to the text box, or back from it to the canvas.
        if state.is_xaml_focus() {
            let target = state.focus_target();
            drop(state);
            if let Some(target) = target {
                _ = target.Focus(w::FocusState::Programmatic);
            }
            state = self.state.borrow_mut();
        }
        state.place_input_method();
    }

    pub(crate) fn set_text_cursor(&self, caret: Option<Rect>) {
        let mut state = self.state.borrow_mut();
        state.text_cursor = Some(caret);
        state.place_input_method();
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
        props.extend(state.text_input.map(Prop::TextInput));
        props.extend(state.input_method.map(Prop::InputMethod));
        props.extend(state.text_cursor.map(Prop::TextCursor));
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
                let unmodified = code.us_char();
                for pressed in [true, false] {
                    let text = match key {
                        Key::Char(c) if pressed && state.takes_text() && !c.is_control() => Some(c.to_string()),
                        _ => None,
                    };
                    state.report(SurfaceInput::Key {
                        code,
                        native,
                        pressed,
                        repeat: false,
                        modifiers,
                        text,
                        unmodified,
                    });
                }
            }
            // As an input method commits: into the text box, whose text is
            // taken (its `TextChanged` would come while this holds the state).
            SyntheticInput::Text(text) => {
                if let (true, Some(text_box)) = (state.composes(), &state.text_box) {
                    text_box.composing.set(false);
                    state.report_preedit();
                    _ = text_box.control.SetText(text);
                    state.take_committed();
                }
            }
            // A composition's text so far, in the text box, as an input
            // method writes it there; empty ends it.
            SyntheticInput::Compose(text) => {
                if let (true, Some(text_box)) = (state.composes(), &state.text_box) {
                    text_box.composing.set(!text.is_empty());
                    _ = text_box.control.SetText(text);
                    _ = text_box.control.SetSelectionStart(text.encode_utf16().count() as i32);
                    state.report_preedit();
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
            | SyntheticInput::MoveRows { .. }
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
    // Before the focused element hears of a key: while grabbed, keys the
    // hook didn't take (it takes them only while the window is the
    // foreground one) never reach accelerators; while it takes text, keys
    // are the surface's, not the text box's, which only an input method
    // writes to. Keys with Control or Alt are left to accelerators, as
    // below; an input method's own to it, unreported.
    for pressed in [true, false] {
        let s = weak();
        let handler = move |_: windows_core::Ref<windows_core::IInspectable>,
                            args: windows_core::Ref<w::KeyRoutedEventArgs>| {
            let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
            let modifiers = key_modifiers();
            let shortcut = pressed && (modifiers.control || modifiers.alt);
            let input_method = args.Key().is_ok_and(is_input_method_key);
            let handled = with(&s, |state| {
                // Taking text, an input method's own keys (toggles,
                // conversion) are its, as in a text field.
                let text = state.takes_text() && !input_method;
                (state.grabbed || text) && state.key(&args, pressed).is_ok() && (state.grabbed || !shortcut)
            });
            if handled {
                _ = args.SetHandled(true);
            }
        };
        revokers.push(if pressed { element.PreviewKeyDown(handler)? } else { element.PreviewKeyUp(handler)? });
    }
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
            // Taking text, they were reported before the text box had them.
            if with(&s, |state| {
                !state.takes_text() && state.key(&args, pressed).is_ok() && (!shortcut || state.grabbed)
            }) {
                _ = args.SetHandled(true);
            }
        };
        revokers.push(if pressed { element.KeyDown(handler)? } else { element.KeyUp(handler)? });
    }
    // Focus coming to the canvas goes on to its text box while input
    // methods compose in it, and coming to the box goes back to the canvas
    // while they don't (the box's own focus comes through here too). Not
    // while the state is busy: what focuses then picks the target itself.
    let s = weak();
    revokers.push(element.GettingFocus(move |_, args| {
        let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IGettingFocusEventArgs>().ok()) else { return };
        let Some(s) = s.upgrade() else { return };
        let Ok(state) = s.try_borrow() else { return };
        let Some(text_box) = &state.text_box else { return };
        let Ok(new) = args.NewFocusedElement() else { return };
        let ours = same(&new, &state.element) || same(&new, &text_box.control);
        if ours
            && let Some(target) = state.focus_target()
            && !same(&new, &target)
            && let Ok(target) = target.cast::<w::DependencyObject>()
        {
            _ = args.TrySetNewFocusedElement(&target);
        }
    })?);
    let s = weak();
    revokers.push(element.GotFocus(move |_, _| {
        if let Some(s) = s.upgrade()
            && let Ok(mut state) = s.try_borrow_mut()
        {
            state.focused = true;
            // Its keys and characters come to the window with Win32
            // focus, which may have been made after the surface was.
            subclass(unsafe { w::GetFocus() });
            state.place_input_method();
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
            // Its next key goes elsewhere.
            state.dead = None;
            state.end_dead();
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
            // The text box, while input methods compose in it: `GettingFocus`
            // can't send focus on there while this holds the state, and a
            // click that focused the canvas left them nothing to compose in.
            if let Some(target) = self.focus_target() {
                _ = target.Focus(w::FocusState::Pointer);
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
        let virtual_key = args.Key()?.0 as u32;
        let native = scan_code(&status, virtual_key);
        let code = KeyCode::from_windows_scancode(native);
        let repeat = pressed && status.was_key_down;
        if pressed {
            self.keys_down.insert(native);
        } else if !self.keys_down.remove(&native) {
            // A release for a key it never saw go down (it went down elsewhere).
            return Ok(());
        }
        let typed = if pressed { self.typed.take() } else { None };
        let dead = if pressed { self.dead.take() } else { None };
        let text = typed.filter(|t| !t.is_empty() && self.takes_text());
        let unmodified = unmodified(virtual_key, native & 0xFF);
        let modifiers = key_modifiers();
        // A dead key's accent shows as a composition, as it does in a text
        // field, until the next key (not a modifier, which may be what it
        // composes with) composes it in or types it on its own: it ends
        // before that key's text.
        if pressed && self.dead_shown && (text.is_some() || dead.is_some() || !is_modifier(code)) {
            self.end_dead();
        }
        self.report(SurfaceInput::Key { code, native, pressed, repeat, modifiers, text, unmodified });
        if let Some(accent) = dead.filter(|_| self.takes_text()) {
            self.dead_shown = true;
            self.report(SurfaceInput::Preedit { text: accent.to_string(), cursor: Some(accent.len_utf8()) });
        }
        Ok(())
    }

    /// Ends a dead key's composition, if one shows.
    fn end_dead(&mut self) {
        if std::mem::take(&mut self.dead_shown) {
            self.report(SurfaceInput::Preedit { text: String::new(), cursor: None });
        }
    }

    fn takes_text(&self) -> bool {
        self.takes_input == Some(true) && self.text_input == Some(true)
    }

    /// Takes text, and input methods compose in its text box.
    fn composes(&self) -> bool {
        self.takes_text() && self.input_method != Some(false) && self.text_box.is_some()
    }

    /// What has focus for it: the text box while input methods compose in
    /// it, else the canvas.
    fn focus_target(&self) -> Option<w::IUIElement> {
        match &self.text_box {
            Some(text_box) if self.composes() => text_box.control.cast().ok(),
            _ => self.element.cast().ok(),
        }
    }

    /// A `WM_CHAR` no key down came with, an input method's: one UTF-16
    /// unit at a time.
    fn char_typed(&mut self, unit: u16) {
        let units = match (self.surrogate.take(), unit) {
            (_, 0xD800..=0xDBFF) => {
                self.surrogate = Some(unit);
                return;
            }
            (Some(high), 0xDC00..=0xDFFF) => vec![high, unit],
            _ => vec![unit],
        };
        let text = typed_text(&units);
        if !text.is_empty() {
            self.report(SurfaceInput::Text(text));
        }
    }

    /// Opens the input method's composition and candidate windows by the
    /// app's caret: the text box it composes in goes there (Windows' input
    /// methods place theirs by its text), and IMM32 is told too, in the
    /// focused window's client pixels, for the input methods that ask it.
    fn place_input_method(&self) {
        let (Some(Some(caret)), Some((x, y, _, _))) = (self.text_cursor, self.placed) else { return };
        if let Some(text_box) = &self.text_box
            && let Ok(element) = text_box.control.cast::<w::UIElement>()
        {
            _ = w::Canvas::SetLeft(&element, caret.origin.x as f64);
            _ = w::Canvas::SetTop(&element, caret.origin.y as f64);
        }
        if !self.composes() || !self.is_xaml_focus() {
            return;
        }
        let focus = unsafe { w::GetFocus() };
        if focus.is_null() {
            return;
        }
        let scale = self.scale();
        let px = |v: f32| (v as f64 * scale).round() as i32;
        let mut origin = w::POINT { x: x + px(caret.origin.x), y: y + px(caret.origin.y) };
        unsafe {
            _ = w::ClientToScreen(self.window, &mut origin);
            _ = w::ScreenToClient(focus, &mut origin);
        }
        let (width, height) = (px(caret.size.width).max(1), px(caret.size.height).max(1));
        let area = w::RECT { left: origin.x, top: origin.y, right: origin.x + width, bottom: origin.y + height };
        let context = unsafe { w::ImmGetContext(focus) };
        if context.is_null() {
            return;
        }
        let composition = w::COMPOSITIONFORM { dwStyle: w::CFS_POINT as u32, ptCurrentPos: origin, rcArea: area };
        // Below the caret, never over it.
        let candidates = w::CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: w::CFS_EXCLUDE as u32,
            ptCurrentPos: w::POINT { x: origin.x, y: area.bottom },
            rcArea: area,
        };
        unsafe {
            _ = w::ImmSetCompositionWindow(context, &composition);
            _ = w::ImmSetCandidateWindow(context, &candidates);
            _ = w::ImmReleaseContext(focus, context);
        }
    }

    /// Reports every key it has down as released: focus left, or the
    /// window stopped being the active one, so their releases go elsewhere.
    fn release_keys(&mut self) {
        let modifiers = Modifiers::default();
        for native in std::mem::take(&mut self.keys_down) {
            let code = KeyCode::from_windows_scancode(native);
            let (text, unmodified) = (None, None);
            self.report(SurfaceInput::Key { code, native, pressed: false, repeat: false, modifiers, text, unmodified });
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
            // A system cursor is shared: never destroyed.
            Cursor::System(shape) => Some(unsafe { w::LoadCursorW(std::ptr::null_mut(), system_cursor_id(shape)) }),
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
            state.place_input_method();
        }
        state.raise();
        state.try_pending();
    }

    /// Whether the canvas, or its text box, is XAML's focused element.
    fn is_xaml_focus(&self) -> bool {
        let focused = self.element.XamlRoot().and_then(|r| w::FocusManager::GetFocusedElementWithRoot(&r));
        let Ok(focused) = focused else { return false };
        same(&focused, &self.element) || self.text_box.as_ref().is_some_and(|b| same(&focused, &b.control))
    }

    /// Reports the composition going on in the text box, with its caret
    /// (the box's, in UTF-16 units, made bytes), or that there's none, if
    /// that changed.
    fn report_preedit(&self) {
        let Some(text_box) = &self.text_box else { return };
        let preedit = if text_box.composing.get() {
            let text = text_box.control.Text().unwrap_or_default();
            let units = text_box.control.SelectionStart().unwrap_or(0).max(0) as usize;
            let cursor =
                char::decode_utf16(text.encode_utf16().take(units)).filter_map(Result::ok).map(char::len_utf8).sum();
            (text, Some(cursor))
        } else {
            (String::new(), None)
        };
        if *text_box.preedit.borrow() == preedit || !self.takes_text() {
            return;
        }
        *text_box.preedit.borrow_mut() = preedit.clone();
        let (text, cursor) = preedit;
        self.report(SurfaceInput::Preedit { text, cursor });
    }

    /// Takes what an input method committed to the text box, and empties
    /// it for the next.
    fn take_committed(&self) {
        let Some(text_box) = &self.text_box else { return };
        let Ok(text) = text_box.control.Text() else { return };
        if text.is_empty() {
            return;
        }
        _ = text_box.control.SetText("");
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if !text.is_empty() && self.takes_text() {
            self.report(SurfaceInput::Text(text));
        }
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
                && self.focus_target().and_then(|e| e.Focus(w::FocusState::Programmatic).ok()).unwrap_or(false);
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
        // The hook comes before keys are translated: they type nothing.
        let (text, unmodified) = (None, unmodified(key.vkCode, key.scanCode));
        self.report(SurfaceInput::Key { code, native, pressed, repeat, modifiers, text, unmodified });
    }

    /// The canvas in the window's client area, in pixels; `None` while it
    /// isn't laid out or has no size. XAML's root fills the client area.
    /// Its size is the frame `SetFrame` set: XAML lays it out at that size
    /// only at its next frame, which may not come for a while (none came
    /// after the window was resized while the canvas held its text box,
    /// until a click elsewhere).
    fn rect(&self) -> R<Option<(i32, i32, i32, i32)>> {
        let fe: w::IFrameworkElement = self.element.cast()?;
        let set = |set: f64, actual: f64| if set.is_finite() { set } else { actual };
        let (width, height) = (set(fe.Width()?, fe.ActualWidth()?), set(fe.Height()?, fe.ActualHeight()?));
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

/// A key an input method takes for itself: `ProcessKey` (one it's
/// composing with), and the keys that switch it on and off or between
/// modes (Kana, Kanji — Alt+` on a US keyboard — Convert, NonConvert…),
/// which a text field doesn't type either.
fn is_input_method_key(key: w::VirtualKey) -> bool {
    matches!(key.0, 0x15..=0x1A | 0x1C..=0x1F | 229)
}

/// COM identity: the same object answers the same `IUnknown`.
fn same(a: &impl Interface, b: &impl Interface) -> bool {
    let (Ok(a), Ok(b)) = (a.cast::<windows_core::IUnknown>(), b.cast::<windows_core::IUnknown>()) else { return false };
    a.as_raw() == b.as_raw()
}

/// The text box an input method composes in, in `canvas`, under the
/// surface: transparent and out of the pointer's way (a tab stop, since XAML
/// keeps focus only on one; Tab is the surface's while it has it). What
/// it holds when no composition is going on was committed: it's taken.
/// Pastes are refused, since the keys are the surface's.
fn make_text_box(state: &Rc<RefCell<HostState>>, canvas: &w::UIElement) -> R<TextBox> {
    let control = w::TextBox::new()?;
    let element: w::IUIElement = control.cast()?;
    element.SetOpacity(0.0)?;
    element.SetIsHitTestVisible(false)?;
    let text_box: w::ITextBox = control.cast()?;
    text_box.SetIsSpellCheckEnabled(false)?;
    text_box.SetIsTextPredictionEnabled(false)?;
    canvas.cast::<w::IPanel>()?.Children()?.Append(&control.cast::<w::UIElement>()?)?;
    let mut events = Vec::new();
    // Each runs on the state, if it isn't busy (a synthesized input holds it
    // while the box raises events) and the box is there.
    fn with(s: &Weak<RefCell<HostState>>, f: impl FnOnce(&HostState, &TextBox)) {
        if let Some(s) = s.upgrade()
            && let Ok(state) = s.try_borrow()
            && let Some(text_box) = &state.text_box
        {
            f(&state, text_box);
        }
    }
    let s = Rc::downgrade(state);
    events.push(text_box.TextCompositionStarted(move |_, _| {
        with(&s, |_, text_box| text_box.composing.set(true));
    })?);
    // The composition's text changes as it's typed and converted.
    let s = Rc::downgrade(state);
    events.push(text_box.TextCompositionChanged(move |_, _| with(&s, |state, _| state.report_preedit()))?);
    let s = Rc::downgrade(state);
    events.push(text_box.TextCompositionEnded(move |_, _| {
        with(&s, |state, text_box| {
            text_box.composing.set(false);
            state.report_preedit();
            state.take_committed();
        });
    })?);
    let s = Rc::downgrade(state);
    events.push(text_box.TextChanged(move |_, _| {
        with(&s, |state, text_box| {
            if text_box.composing.get() {
                state.report_preedit();
            } else {
                state.take_committed();
            }
        });
    })?);
    events.push(text_box.Paste(move |_, args| {
        if let Some(args) = args.as_ref().and_then(|a| a.cast::<w::ITextControlPasteEventArgs>().ok()) {
            _ = args.SetHandled(true);
        }
    })?);
    Ok(TextBox { control, composing: Cell::new(false), preedit: RefCell::default(), _events: events })
}

/// Where a key is: its scan code, with `0xE000` for an extended key. XAML
/// reports Tab with none (it raises it from its focus navigation), so a key
/// without one takes the scan code the layout maps its virtual key to.
fn scan_code(status: &w::CorePhysicalKeyStatus, virtual_key: u32) -> u32 {
    if status.scan_code != 0 {
        return status.scan_code | if status.is_extended_key { 0xE000 } else { 0 };
    }
    let mapped = unsafe { w::MapVirtualKeyW(virtual_key, w::MAPVK_VK_TO_VSC_EX as u32) };
    // An extended key comes back with an `0xE0` (or `0xE1`) prefix byte.
    (mapped & 0xFF) | if mapped & 0xFF00 != 0 { 0xE000 } else { 0 }
}

/// The character a key types with no modifiers in the thread's layout:
/// `ToUnicodeEx` with nothing held, leaving the keyboard's state (a dead
/// key pressed before) as it is. A dead key gives its accent.
fn unmodified(virtual_key: u32, scan_code: u32) -> Option<char> {
    // Bit 2: the keyboard's state stays as it is (Windows 10 1607).
    const KEEP_STATE: u32 = 0x4;
    let held = [0u8; 256];
    let mut out = [0u16; 8];
    let layout = unsafe { w::GetKeyboardLayout(0) };
    let written = unsafe {
        w::ToUnicodeEx(
            virtual_key,
            scan_code,
            held.as_ptr(),
            windows_core::PWSTR(out.as_mut_ptr()),
            out.len() as i32,
            KEEP_STATE,
            layout,
        )
    };
    let len = (written.unsigned_abs() as usize).min(out.len());
    char::decode_utf16(out[..len].iter().copied()).next()?.ok().filter(|c| !c.is_control())
}

/// Text from `WM_CHAR`s: control characters (Control+letters, Enter, Tab,
/// Backspace, Escape) aren't text.
fn typed_text(units: &[u16]) -> String {
    char::decode_utf16(units.iter().copied()).filter_map(Result::ok).filter(|c| !c.is_control()).collect()
}

/// Takes off the queue the `WM_CHAR`s `TranslateMessage` queued for the
/// key going down in `hwnd` (two for a dead key that didn't compose, or a
/// character outside the BMP), and a dead key's `WM_DEADCHAR`, whose accent
/// comes back on its own.
fn typed_after_key(hwnd: w::HWND) -> (String, Option<char>) {
    let mut units = Vec::new();
    let mut dead = None;
    loop {
        let mut message = w::MSG::default();
        let (first, last) = (w::WM_KEYFIRST as u32, w::WM_KEYLAST as u32);
        let queued = unsafe { w::PeekMessageW(&mut message, hwnd, first, last, w::PM_NOREMOVE as u32) };
        let kind = message.message;
        if !queued.as_bool() || (kind != w::WM_CHAR as u32 && kind != w::WM_DEADCHAR as u32) {
            break;
        }
        unsafe { _ = w::PeekMessageW(&mut message, hwnd, kind, kind, w::PM_REMOVE as u32) };
        if kind == w::WM_CHAR as u32 {
            units.push(message.wParam as u16);
        } else {
            dead = char::from_u32(message.wParam as u32).filter(|c| !c.is_control());
        }
    }
    (typed_text(&units), dead)
}

/// Keys that change what others type, rather than typing.
fn is_modifier(code: KeyCode) -> bool {
    use KeyCode::*;
    matches!(
        code,
        ShiftLeft | ShiftRight | ControlLeft | ControlRight | AltLeft | AltRight | MetaLeft | MetaRight | CapsLock
    )
}

/// The surface in `hwnd`'s window that takes text and is XAML's focused
/// element. Asked of XAML, since `GotFocus` comes after focus moved: the
/// first key after a click came before it.
fn text_host(hwnd: w::HWND) -> Option<Rc<RefCell<HostState>>> {
    let root = unsafe { w::GetAncestor(hwnd, w::GA_ROOT as u32) };
    let hosts: Vec<_> = HOSTS.with(|h| h.borrow().iter().filter_map(Weak::upgrade).collect());
    hosts
        .into_iter()
        .find(|host| host.try_borrow().is_ok_and(|s| s.window == root && s.takes_text() && s.is_xaml_focus()))
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
/// `WM_SETCURSOR`, and for the keys and characters of the one with focus.
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
    if unsafe { w::SetWindowSubclass(hwnd, Some(subclass_proc), 0, 0) }.as_bool() {
        SUBCLASSED.with(|s| s.borrow_mut().insert(hwnd as isize));
    }
}

/// Over a surface with a cursor of the app's, sets it rather than letting
/// XAML set its own. A window asks its parent first only in
/// `DefWindowProc`, which XAML's input window doesn't reach: each is
/// subclassed. While a surface that takes text has focus, keys take their
/// characters with them, and characters on their own are text.
unsafe extern "system" fn subclass_proc(
    hwnd: w::HWND,
    message: u32,
    wparam: w::WPARAM,
    lparam: w::LPARAM,
    _: usize,
    _: usize,
) -> w::LRESULT {
    let key_down = message == w::WM_KEYDOWN as u32 || message == w::WM_SYSKEYDOWN as u32;
    if key_down && let Some(host) = text_host(hwnd) {
        let (typed, dead) = typed_after_key(hwnd);
        {
            let mut state = host.borrow_mut();
            state.typed = Some(typed);
            state.dead = dead;
        }
        let result = unsafe { w::DefSubclassProc(hwnd, message, wparam, lparam) };
        // XAML took the key itself (an accelerator) and the canvas never
        // heard of it: what it typed still goes to the surface.
        host.borrow_mut().dead = None;
        let left = host.borrow_mut().typed.take();
        if let Some(text) = left.filter(|t| !t.is_empty()) {
            host.borrow().report(SurfaceInput::Text(text));
        }
        return result;
    }
    if message == w::WM_CHAR as u32
        && let Some(host) = text_host(hwnd)
    {
        host.borrow_mut().char_typed(wparam as u16);
        return 0;
    }
    if message == w::WM_IME_STARTCOMPOSITION as u32
        && let Some(host) = text_host(hwnd)
    {
        host.borrow().place_input_method();
    }
    if message == w::WM_NCDESTROY as u32 {
        unsafe { _ = w::RemoveWindowSubclass(hwnd, Some(subclass_proc), 0) };
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

/// Win32's `IDC_*` for a platform cursor, as `MAKEINTRESOURCE` makes it.
fn system_cursor_id(shape: SystemCursor) -> PCWSTR {
    let id: u16 = match shape {
        SystemCursor::Text => 32513,                     // IDC_IBEAM
        SystemCursor::Wait => 32514,                     // IDC_WAIT
        SystemCursor::Crosshair => 32515,                // IDC_CROSS
        SystemCursor::ResizeNorthWestSouthEast => 32642, // IDC_SIZENWSE
        SystemCursor::ResizeNorthEastSouthWest => 32643, // IDC_SIZENESW
        SystemCursor::ResizeEastWest => 32644,           // IDC_SIZEWE
        SystemCursor::ResizeNorthSouth => 32645,         // IDC_SIZENS
        SystemCursor::Move => 32646,                     // IDC_SIZEALL
        SystemCursor::NotAllowed => 32648,               // IDC_NO
        SystemCursor::Pointer => 32649,                  // IDC_HAND
        SystemCursor::Progress => 32650,                 // IDC_APPSTARTING
        SystemCursor::Help => 32651,                     // IDC_HELP
    };
    PCWSTR(id as usize as _)
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
