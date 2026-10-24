//! `GpuSurface`: a native surface the app presents to with its own GPU API.
//! It's reported once it exists, then its size in pixels whenever that or
//! its scale changes; its handle keeps the native surface after the widget
//! is gone. It has no natural size, and reads as an image named by its
//! label. With `on_input` it takes focus, keys and the pointer, and with
//! `text_input` what the keys type; its pointer lock and keyboard grab end
//! as the platform ends them.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami::raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use mitsuami_test::prelude::*;

#[derive(Debug)]
enum Seen {
    Ready(SurfaceHandle),
    Resized(SurfaceSize),
}

fn surface(seen: &Rc<RefCell<Vec<Seen>>>) -> GpuSurface {
    let (ready, resized) = (seen.clone(), seen.clone());
    GpuSurface::new()
        .label("Screen")
        .on_ready(move |s| ready.borrow_mut().push(Seen::Ready(s)))
        .on_resize(move |size| resized.borrow_mut().push(Seen::Resized(size)))
}

fn screen() -> Query {
    by_role(Role::Image, "Screen")
}

fn handle(seen: &Rc<RefCell<Vec<Seen>>>) -> SurfaceHandle {
    seen.borrow()
        .iter()
        .find_map(|s| match s {
            Seen::Ready(handle) => Some(handle.clone()),
            Seen::Resized(_) => None,
        })
        .expect("the surface was reported")
}

fn last_size(seen: &Rc<RefCell<Vec<Seen>>>) -> Option<SurfaceSize> {
    seen.borrow().iter().rev().find_map(|s| match s {
        Seen::Resized(size) => Some(*size),
        Seen::Ready(_) => None,
    })
}

/// Its frame in pixels, at the scale it reports, within the pixel a
/// platform may round either way.
fn assert_fits(size: SurfaceSize, frame: Rect) {
    assert!(size.scale > 0.0, "{size:?}");
    for (pixels, points) in [(size.width, frame.width()), (size.height, frame.height())] {
        assert!((pixels as f32 - points * size.scale).abs() <= 1.0, "{size:?} for {frame:?}");
    }
}

#[mitsuami_test::test]
async fn reports_the_surface_then_its_size(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(200.px(), 150.px())));
    app.expect(screen()).to_have_frame(Rect::new(0.0, 0.0, 200.0, 150.0)).await;

    assert_eq!(app.get(screen()).native_state().kind, WidgetKind::GpuSurface);
    let readies = seen.borrow().iter().filter(|s| matches!(s, Seen::Ready(_))).count();
    assert_eq!(readies, 1, "{seen:?}");
    assert!(matches!(seen.borrow()[0], Seen::Ready(_)), "ready comes first: {seen:?}");
    let size = last_size(&seen).expect("a size");
    assert_fits(size, app.get(screen()).frame());
    assert_eq!(handle(&seen).size(), size, "the handle has the size too");
}

/// Its window resized with nothing else going on, as the user drags its
/// edge, it reports its new size: on WinUI XAML may draw no frame then, and
/// with its text box focused bemtvi's editor kept its old size until a click
/// elsewhere in the window.
#[mitsuami_test::test]
async fn follows_its_window_being_resized(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let (_log, on_input) = input_log();
    app.mount(move || Column::new().grow(1.0).child(surface(&s).grow(1.0).on_input(on_input).text_input(true)));
    app.settle().await;
    app.get(screen()).click_at(10.0, 10.0).await;
    let before = last_size(&seen).expect("a size");
    app.resize(700.0, 500.0).await;
    app.resize(900.0, 650.0).await;
    let after = last_size(&seen).expect("a size");
    assert_ne!(after, before, "{seen:?}");
    assert_fits(after, app.get(screen()).frame());
}

#[mitsuami_test::test]
async fn follows_its_frame(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let width = signal(200.0);
    let s = seen.clone();
    app.mount(move || {
        Column::new().child(surface(&s).style_with(move |st| {
            st.width = width.get().px();
            st.height = 100.px();
        }))
    });
    app.settle().await;
    let before = last_size(&seen).expect("a size");

    width.set(320.0);
    app.expect(screen()).to_have_frame(Rect::new(0.0, 0.0, 320.0, 100.0)).await;

    let after = last_size(&seen).expect("a size");
    assert!(after.width > before.width, "{before:?} → {after:?}");
    assert_eq!(after.height, before.height);
    assert_fits(after, app.get(screen()).frame());
}

/// As large as the layout makes it: nothing, without a size.
#[mitsuami_test::test]
async fn has_no_natural_size(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    app.mount(move || Column::new().align(Align::Start).children((surface(&s), Text::new("After"))));
    app.settle().await;

    assert!(app.get(screen()).frame().size.is_empty());
    assert!(last_size(&seen).is_none_or(|size| size.is_empty()), "{seen:?}");
}

/// A native backend hands out its platform's handles; headless, and the
/// private displays Linux tests run on, have none to give.
#[mitsuami_test::test]
async fn hands_out_raw_window_handles(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(64.px(), 64.px())));
    app.settle().await;

    let handle = handle(&seen);
    if !app.has_surface_handles() {
        assert!(matches!(handle.window_handle(), Err(HandleError::NotSupported)));
        return;
    }
    let window = handle.window_handle().expect("a window handle").as_raw();
    handle.display_handle().expect("a display handle");
    #[cfg(target_os = "macos")]
    {
        use mitsuami::appkit::objc2::runtime::AnyObject;
        use mitsuami::appkit::objc2::{msg_send, rc::Retained};
        use mitsuami::raw_window_handle::RawWindowHandle;
        let RawWindowHandle::AppKit(appkit) = window else { panic!("{window:?}") };
        // SAFETY: the handle keeps the view alive.
        let view: &AnyObject = unsafe { appkit.ns_view.cast().as_ref() };
        let layer: Retained<AnyObject> = unsafe { msg_send![view, layer] };
        assert_eq!(layer.class().name().to_str().unwrap(), "CAMetalLayer");
        // A resize doesn't stretch the last frame presented.
        let gravity: Retained<AnyObject> = unsafe { msg_send![&*layer, contentsGravity] };
        let gravity: Retained<mitsuami::appkit::objc2_foundation::NSString> =
            unsafe { Retained::cast_unchecked(gravity) };
        assert_ne!(gravity.to_string(), "resize");
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// The handle outlives the widget, so a GPU surface made on it never
/// presents to a freed one; it may be dropped on the app's render thread.
#[mitsuami_test::test]
async fn keeps_the_surface_for_the_app(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let shown = signal(true);
    let s = seen.clone();
    app.mount(move || Column::new().child(Show::new(shown, move || surface(&s).size(64.px(), 64.px()))));
    app.settle().await;
    let handle = handle(&seen);
    let nodes = app.native_node_count();

    shown.set(false);
    app.settle().await;

    app.expect(screen()).not_to_exist().await;
    assert!(app.native_node_count() < nodes);
    if app.has_surface_handles() {
        assert!(handle.window_handle().is_ok(), "still there for the app");
    }
    seen.borrow_mut().clear();
    std::thread::spawn(move || drop(handle)).join().unwrap();
    app.settle().await;
}

/// Its pixels follow the scale factor.
#[mitsuami_test::test(headless)]
async fn follows_the_scale_factor(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(100.px(), 50.px())));
    app.settle().await;
    assert_eq!(last_size(&seen), Some(SurfaceSize { width: 100, height: 50, scale: 1.0 }));

    let mut retina: PlatformMetrics = app.ui().metrics();
    retina.scale_factor = 2.0;
    app.headless().set_metrics(retina);
    app.settle().await;

    assert_eq!(last_size(&seen), Some(SurfaceSize { width: 200, height: 100, scale: 2.0 }));
}

/// What an input handler got.
fn input_log() -> (Rc<RefCell<Vec<SurfaceInput>>>, impl Fn(SurfaceInput) + 'static) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let l = log.clone();
    (log, move |input| l.borrow_mut().push(input))
}

fn taking_input(on_input: impl Fn(SurfaceInput) + 'static) -> GpuSurface {
    GpuSurface::new().label("Screen").size(200.px(), 100.px()).on_input(on_input)
}

/// Keys, without the moves and modifiers a platform reports around them.
fn keys(log: &Rc<RefCell<Vec<SurfaceInput>>>) -> Vec<(KeyCode, bool)> {
    log.borrow()
        .iter()
        .filter_map(|input| match input {
            SurfaceInput::Key { code, pressed, .. } => Some((*code, *pressed)),
            _ => None,
        })
        .collect()
}

/// A click focuses it and reports the button where it went down and up.
#[mitsuami_test::test]
async fn takes_clicks_and_focus(app: TestApp) {
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input)));
    app.settle().await;

    app.get(screen()).click_at(30.0, 40.0).await;

    app.expect(screen()).to_be_focused().await;
    let buttons: Vec<(MouseButton, bool, Point)> = log
        .borrow()
        .iter()
        .filter_map(|input| match input {
            SurfaceInput::Button { button, pressed, position, .. } => Some((*button, *pressed, *position)),
            _ => None,
        })
        .collect();
    let at = Point::new(30.0, 40.0);
    assert_eq!(buttons, [(MouseButton::Primary, true, at), (MouseButton::Primary, false, at)], "{log:?}");
}

/// Keys go down and up by where they are on the keyboard, Tab too: it
/// doesn't move focus on.
#[mitsuami_test::test]
async fn takes_keys_while_focused(app: TestApp) {
    let (log, on_input) = input_log();
    app.mount(move || Column::new().children((taking_input(on_input), TextInput::new().a11y_label("After"))));
    app.settle().await;
    app.get(screen()).click_at(10.0, 10.0).await;

    app.get(screen()).press(Key::Char('a')).await;
    app.get(screen()).press(Key::Tab).await;

    let expected = [(KeyCode::KeyA, true), (KeyCode::KeyA, false), (KeyCode::Tab, true), (KeyCode::Tab, false)];
    assert_eq!(keys(&log), expected, "{log:?}");
    app.expect(screen()).to_be_focused().await;
}

/// Modifier keys go down and up too. AppKit reports them as flag
/// changes, which aren't key events: asking one whether it repeats raised
/// an exception, and the keys were lost.
#[cfg(target_os = "macos")]
#[mitsuami_test::test]
async fn takes_modifier_keys(app: TestApp) {
    use mitsuami::appkit::objc2::rc::Retained;
    use mitsuami::appkit::objc2::runtime::AnyObject;
    use mitsuami::appkit::objc2::{Encoding, RefEncode, class, msg_send};
    use mitsuami::raw_window_handle::RawWindowHandle;

    #[repr(C)]
    struct CGEvent([u8; 0]);
    // SAFETY: Quartz's opaque event type, as AppKit encodes it.
    unsafe impl RefEncode for CGEvent {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
    }
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(source: *const std::ffi::c_void, key: u16, down: bool) -> *mut CGEvent;
        fn CGEventSetFlags(event: *mut CGEvent, flags: u64);
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    let seen = Rc::new(RefCell::new(Vec::new()));
    let (log, on_input) = input_log();
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(200.px(), 100.px()).on_input(on_input)));
    app.settle().await;
    if app.is_headless() {
        return;
    }
    let RawWindowHandle::AppKit(appkit) = handle(&seen).window_handle().unwrap().as_raw() else { panic!() };
    // SAFETY: the handle keeps the view alive.
    let view: &AnyObject = unsafe { appkit.ns_view.cast().as_ref() };
    // Left Control down (its device flag and Control), then up.
    for (flags, down) in [(0x40001, true), (0x100, false)] {
        unsafe {
            let cg = CGEventCreateKeyboardEvent(std::ptr::null(), 0x3B, down);
            CGEventSetFlags(cg, flags);
            let event: Retained<AnyObject> = msg_send![class!(NSEvent), eventWithCGEvent: cg];
            let _: () = msg_send![view, flagsChanged: &*event];
            CFRelease(cg.cast());
        }
    }
    app.settle().await;

    assert_eq!(keys(&log), [(KeyCode::ControlLeft, true), (KeyCode::ControlLeft, false)], "{log:?}");
}

/// Tab reaches it, as it reaches the window's controls.
#[mitsuami_test::test]
async fn is_in_the_tab_order(app: TestApp) {
    let (_log, on_input) = input_log();
    app.mount(move || Column::new().children((TextInput::new().a11y_label("Before"), taking_input(on_input))));
    app.settle().await;
    let order = app.ui().focus_order(app.window());
    assert_eq!(order, [app.get_by_label("Before").id(), app.get(screen()).id()]);

    app.get_by_label("Before").press(Key::Tab).await;
    app.expect(screen()).to_be_focused().await;
}

/// Without `on_input` it takes no focus.
#[mitsuami_test::test]
async fn takes_no_focus_without_input(app: TestApp) {
    app.mount(|| Column::new().children((TextInput::new().a11y_label("Before"), surface_only())));
    app.settle().await;
    assert_eq!(app.ui().focus_order(app.window()), [app.get_by_label("Before").id()]);
}

fn surface_only() -> GpuSurface {
    GpuSurface::new().label("Screen").size(64.px(), 64.px())
}

/// The wheel or a trackpad over it scrolls towards the end.
#[mitsuami_test::test]
async fn takes_scrolls(app: TestApp) {
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input)));
    app.settle().await;

    app.get(screen()).scroll_by(0.0, 30.0).await;

    let scrolled: Vec<(f32, f32)> = log
        .borrow()
        .iter()
        .filter_map(|input| match input {
            SurfaceInput::Scroll { delta: ScrollDelta::Points { x, y } | ScrollDelta::Lines { x, y }, .. } => {
                Some((*x, *y))
            }
            _ => None,
        })
        .collect();
    assert!(!scrolled.is_empty() && scrolled.iter().all(|(x, y)| *x == 0.0 && *y > 0.0), "{log:?}");
}

/// The lock and the grab hold until the platform ends them, which sets
/// their signals back; a grab focuses the surface, and ends when it
/// loses focus.
#[mitsuami_test::test(headless)]
async fn ends_the_lock_and_grab_as_the_platform_does(app: TestApp) {
    let (locked, grabbed) = (signal(false), signal(false));
    let (_log, on_input) = input_log();
    app.mount(move || {
        Column::new().children((
            taking_input(on_input).pointer_lock(locked).keyboard_grab(grabbed),
            TextInput::new().a11y_label("After"),
        ))
    });
    app.settle().await;

    locked.set(true);
    grabbed.set(true);
    app.settle().await;
    app.expect(screen()).to_be_focused().await;
    let props = app.get(screen()).native_state().props;
    assert!(props.contains(&Prop::PointerLock(true)) && props.contains(&Prop::KeyboardGrab(true)), "{props:?}");

    app.get_by_label("After").focus().await;
    assert!(!grabbed.get_untracked(), "focus left the surface");
    assert!(locked.get_untracked());

    app.headless().deactivate(app.window());
    app.settle().await;
    assert!(!locked.get_untracked(), "the window stopped being the active one");
    assert!(app.get(screen()).native_state().props.contains(&Prop::PointerLock(false)));
}

/// On a native backend the lock and grab hold only while the window is
/// the active one, which a test run may not have: either way the signals
/// say what the platform did (and the mirror check compares).
#[mitsuami_test::test]
async fn reports_the_lock_and_grab_it_has(app: TestApp) {
    let (locked, grabbed) = (signal(true), signal(true));
    let (_log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).pointer_lock(locked).keyboard_grab(grabbed)));
    app.settle().await;

    let props = app.get(screen()).native_state().props;
    assert!(props.contains(&Prop::PointerLock(locked.get_untracked())), "{props:?}");
    assert!(props.contains(&Prop::KeyboardGrab(grabbed.get_untracked())), "{props:?}");
    locked.set(false);
    grabbed.set(false);
    app.settle().await;
}

#[mitsuami_test::test]
async fn reads_as_an_image_named_by_its_label(app: TestApp) {
    app.mount(|| Column::new().child(GpuSurface::new().label("Screen").size(64.px(), 64.px())));

    app.expect(screen()).to_be_visible().await;
}

/// The app's cursor over it: the platform's, none, an image with its
/// hotspot, or one of the platform's by name (a busy one, which AppKit
/// has none of). The native surface carries it (the mirror check
/// compares).
#[mitsuami_test::test]
async fn shows_the_app_s_cursor(app: TestApp) {
    let arrow = Pixels::new(2, 2, vec![255; 16]).scale(2.0);
    let cursor = signal(Cursor::Default);
    app.mount(move || Column::new().child(surface_only().cursor(cursor)));
    app.settle().await;

    for next in [
        Cursor::Image { pixels: arrow.clone(), hotspot: Point::new(0.5, 0.5) },
        Cursor::System(SystemCursor::Text),
        Cursor::System(SystemCursor::Wait),
        Cursor::Hidden,
        Cursor::Default,
    ] {
        cursor.set(next.clone());
        app.settle().await;
        assert!(app.get(screen()).native_state().props.contains(&Prop::Cursor(next)));
    }
}

/// Keys it has down when its window stops being the key one are let go:
/// their releases would go to another window. Needs a key held, which
/// only AppKit's real events give here.
#[cfg(target_os = "macos")]
#[mitsuami_test::test]
async fn lets_go_of_its_keys_when_the_window_does(app: TestApp) {
    use mitsuami::appkit::objc2::rc::Retained;
    use mitsuami::appkit::objc2::runtime::AnyObject;
    use mitsuami::appkit::objc2::{Encoding, RefEncode, class, msg_send};
    use mitsuami::raw_window_handle::RawWindowHandle;

    #[repr(C)]
    struct CGEvent([u8; 0]);
    // SAFETY: Quartz's opaque event type, as AppKit encodes it.
    unsafe impl RefEncode for CGEvent {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
    }
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(source: *const std::ffi::c_void, key: u16, down: bool) -> *mut CGEvent;
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    let seen = Rc::new(RefCell::new(Vec::new()));
    let (log, on_input) = input_log();
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(200.px(), 100.px()).on_input(on_input)));
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.get(screen()).click_at(10.0, 10.0).await;
    let RawWindowHandle::AppKit(appkit) = handle(&seen).window_handle().unwrap().as_raw() else { panic!() };
    // SAFETY: the handle keeps the view alive.
    let view: &AnyObject = unsafe { appkit.ns_view.cast().as_ref() };
    unsafe {
        // A down, and no up.
        let cg = CGEventCreateKeyboardEvent(std::ptr::null(), 0x00, true);
        let event: Retained<AnyObject> = msg_send![class!(NSEvent), eventWithCGEvent: cg];
        let _: () = msg_send![view, keyDown: &*event];
        CFRelease(cg.cast());
        let window: Retained<AnyObject> = msg_send![view, window];
        let name = mitsuami::appkit::objc2_app_kit::NSWindowDidResignKeyNotification;
        let center: Retained<AnyObject> = msg_send![class!(NSNotificationCenter), defaultCenter];
        let _: () = msg_send![&*center, postNotificationName: name, object: &*window];
    }
    app.settle().await;

    assert_eq!(keys(&log), [(KeyCode::KeyA, true), (KeyCode::KeyA, false)], "{log:?}");
}

/// While it holds the pointer, the mouse's moves come twice: in points,
/// accelerated as the cursor would be, and in the device's counts, before
/// the host's acceleration.
#[mitsuami_test::test(headless)]
async fn reports_raw_motion_while_locked(app: TestApp) {
    let locked = signal(true);
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).pointer_lock(locked)));
    app.settle().await;

    app.headless().move_locked_pointer(app.get(screen()).id(), 3.0, -2.0);
    app.settle().await;

    let moves: Vec<SurfaceInput> = log
        .borrow()
        .iter()
        .filter(|input| matches!(input, SurfaceInput::Motion { .. } | SurfaceInput::RawMotion { .. }))
        .cloned()
        .collect();
    assert_eq!(moves, [SurfaceInput::Motion { dx: 3.0, dy: -2.0 }, SurfaceInput::RawMotion { dx: 3.0, dy: -2.0 }]);
}

/// What keys going down typed, and the character each types unmodified.
fn typed(log: &Rc<RefCell<Vec<SurfaceInput>>>) -> Vec<(KeyCode, Option<String>, Option<char>)> {
    log.borrow()
        .iter()
        .filter_map(|input| match input {
            SurfaceInput::Key { code, pressed: true, text, unmodified, .. } => Some((*code, text.clone(), *unmodified)),
            _ => None,
        })
        .collect()
}

/// Taking text, keys report what they type; keys that type nothing
/// (Enter) report none.
#[mitsuami_test::test]
async fn types_text_when_it_takes_text(app: TestApp) {
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).text_input(true)));
    app.settle().await;
    app.get(screen()).click_at(10.0, 10.0).await;

    app.get(screen()).type_text("aB").await;
    app.get(screen()).press(Key::Enter).await;

    let expected = [
        (KeyCode::KeyA, Some("a".to_owned()), Some('a')),
        (KeyCode::KeyB, Some("B".to_owned()), Some('b')),
        (KeyCode::Enter, None, None),
    ];
    assert_eq!(typed(&log), expected, "{log:?}");
}

/// Without `text_input` keys type nothing, as on a game's surface, but
/// still say what they'd type unmodified.
#[mitsuami_test::test]
async fn types_nothing_without_text_input(app: TestApp) {
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input)));
    app.settle().await;
    app.get(screen()).click_at(10.0, 10.0).await;

    app.get(screen()).type_text("a").await;

    assert_eq!(typed(&log), [(KeyCode::KeyA, None, Some('a'))], "{log:?}");
}

/// What an input method commits comes as text of its own: no key typed
/// it.
#[mitsuami_test::test]
async fn takes_an_input_method_s_text(app: TestApp) {
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).text_input(true)));
    app.settle().await;
    app.get(screen()).click_at(10.0, 10.0).await;

    app.get(screen()).commit_text("日本").await;

    let texts: Vec<SurfaceInput> =
        log.borrow().iter().filter(|input| matches!(input, SurfaceInput::Text(_))).cloned().collect();
    assert_eq!(texts, [SurfaceInput::Text("日本".to_owned())], "{log:?}");
}

/// A composition in progress comes as its preedit, for the app to draw at
/// its caret (the surface is the app's to draw, so the platform can't show
/// it there), and ends, empty, as it's committed.
#[mitsuami_test::test]
async fn reports_an_input_method_s_composition(app: TestApp) {
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).text_input(true)));
    app.settle().await;
    app.get(screen()).click_at(10.0, 10.0).await;

    app.get(screen()).compose_text("かん").await;
    app.get(screen()).compose_text("かんじ").await;
    app.get(screen()).commit_text("感じ").await;

    let text: Vec<SurfaceInput> = log
        .borrow()
        .iter()
        .filter(|input| matches!(input, SurfaceInput::Text(_) | SurfaceInput::Preedit { .. }))
        .cloned()
        .collect();
    let preedit = |text: &str, cursor| SurfaceInput::Preedit { text: text.to_owned(), cursor };
    let expected = [
        preedit("かん", Some(6)),
        preedit("かんじ", Some(9)),
        preedit("", None),
        SurfaceInput::Text("感じ".to_owned()),
    ];
    assert_eq!(text, expected, "{log:?}");
}

/// Transparent, it lets the window's backdrop show through, and stops
/// when it isn't. The native surface carries it (the mirror check
/// compares): AppKit by the backdrop under it, the others as set.
#[mitsuami_test::test]
async fn lets_the_window_s_backdrop_show_through(app: TestApp) {
    let transparent = signal(true);
    app.mount(move || Column::new().child(surface_only().transparent(transparent)));
    app.settle().await;
    assert!(app.get(screen()).native_state().props.contains(&Prop::Transparent(true)));

    transparent.set(false);
    app.settle().await;
    assert!(app.get(screen()).native_state().props.contains(&Prop::Transparent(false)));
}

/// On AppKit the backdrop is behind-window vibrancy just under the
/// surface, at its frame as that changes, and stays just under it as
/// views come in before and after it (those after above both); it goes
/// with the surface's transparency.
#[cfg(target_os = "macos")]
#[mitsuami_test::test]
async fn puts_appkit_s_vibrancy_just_under_it(app: TestApp) {
    use mitsuami::appkit::objc2::rc::Retained;
    use mitsuami::appkit::objc2_app_kit::{NSView, NSVisualEffectView};
    use mitsuami::raw_window_handle::RawWindowHandle;

    // Headless has no views.
    if app.is_headless() {
        return;
    }
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (transparent, width, before, after) = (signal(true), signal(200.0), signal(false), signal(false));
    let s = seen.clone();
    app.mount(move || {
        Column::new()
            .child(Show::new(before, || Text::new("Before")))
            .child(surface(&s).transparent(transparent).style_with(move |st| {
                st.width = width.get().px();
                st.height = 100.px();
            }))
            .child(Show::new(after, || Text::new("After")))
    });
    app.settle().await;
    let RawWindowHandle::AppKit(appkit) = handle(&seen).window_handle().unwrap().as_raw() else { panic!() };
    // SAFETY: the handle keeps the view alive, and so does this.
    let view: Retained<NSView> = unsafe { Retained::retain(appkit.ns_view.cast::<NSView>().as_ptr()) }.unwrap();
    let parent = unsafe { view.superview() }.expect("in the layout");
    let at = || parent.subviews().iter().position(|v| std::ptr::eq(&*v, &*view)).expect("among its siblings");
    let under = || {
        let siblings = parent.subviews();
        let backdrops = siblings.iter().filter(|v| v.downcast_ref::<NSVisualEffectView>().is_some()).count();
        let below = at().checked_sub(1).map(|i| siblings.objectAtIndex(i));
        (below.filter(|v| v.downcast_ref::<NSVisualEffectView>().is_some()), backdrops)
    };

    let (backdrop, count) = under();
    let backdrop = backdrop.expect("vibrancy just under it");
    assert_eq!(count, 1);
    assert_eq!(backdrop.frame(), view.frame());

    width.set(320.0);
    app.expect(screen()).to_have_frame(Rect::new(0.0, 0.0, 320.0, 100.0)).await;
    assert_eq!(backdrop.frame(), view.frame());

    for show in [after, before] {
        show.set(true);
        app.settle().await;
        let (still, count) = under();
        assert!(still.is_some_and(|v| std::ptr::eq(&*v, &*backdrop)), "the backdrop left the surface");
        assert_eq!(count, 1);
        assert_eq!(backdrop.frame(), view.frame());
    }
    assert_eq!(at(), parent.subviews().len() - 2, "the view after it went under it");

    transparent.set(false);
    app.settle().await;
    assert_eq!(under(), (None, 0));
}

/// On macOS, the view a surface is, for messages AppKit and its input
/// methods send it, and keys as AppKit makes them.
#[cfg(target_os = "macos")]
mod appkit_view {
    use std::cell::RefCell;

    use mitsuami::appkit::objc2::ffi::class_replaceMethod;
    use mitsuami::appkit::objc2::rc::Retained;
    use mitsuami::appkit::objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use mitsuami::appkit::objc2::{msg_send, sel};
    pub use mitsuami::appkit::objc2_app_kit::NSEventModifierFlags;
    use mitsuami::appkit::objc2_app_kit::{NSEvent, NSEventType, NSWindow};
    use mitsuami::appkit::objc2_foundation::{NSArray, NSPoint, NSRange, NSString};
    use mitsuami::core::SurfaceHandle;
    use mitsuami::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub const NOT_FOUND: usize = isize::MAX as usize;
    /// `kVK_ANSI_K`, `kVK_ANSI_X`, `kVK_Tab`, `kVK_Return`.
    pub const K: u16 = 0x28;
    pub const X: u16 = 0x07;
    pub const TAB: u16 = 0x30;
    pub const RETURN: u16 = 0x24;

    pub struct View(Retained<AnyObject>);

    impl View {
        pub fn of(handle: &SurfaceHandle) -> View {
            let RawWindowHandle::AppKit(appkit) = handle.window_handle().unwrap().as_raw() else { panic!() };
            // SAFETY: the handle keeps the view alive, and so does this.
            View(unsafe { Retained::retain(appkit.ns_view.cast::<AnyObject>().as_ptr()) }.unwrap())
        }

        /// A key at `code` (a macOS virtual key code) going down with
        /// `flags` held, then up, as AppKit makes it: typing `characters`,
        /// `unmodified` with no modifiers.
        pub fn press(&self, code: u16, flags: NSEventModifierFlags, characters: &str, unmodified: &str) {
            let (characters, unmodified) = (NSString::from_str(characters), NSString::from_str(unmodified));
            let window: Retained<NSWindow> = unsafe { msg_send![&*self.0, window] };
            for (kind, up) in [(NSEventType::KeyDown, false), (NSEventType::KeyUp, true)] {
                let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                    kind,
                    NSPoint::new(0.0, 0.0),
                    flags,
                    0.0,
                    window.windowNumber(),
                    None,
                    &characters,
                    &unmodified,
                    false,
                    code,
                )
                .unwrap();
                unsafe {
                    if up {
                        let _: () = msg_send![&*self.0, keyUp: &*event];
                    } else {
                        let _: () = msg_send![&*self.0, keyDown: &*event];
                    }
                }
            }
        }

        pub fn selected_range(&self) -> NSRange {
            unsafe { msg_send![&*self.0, selectedRange] }
        }

        pub fn set_marked_text(&self, text: &str, selected: NSRange, replacement: NSRange) {
            let text = NSString::from_str(text);
            unsafe {
                let _: () =
                    msg_send![&*self.0, setMarkedText: &*text, selectedRange: selected, replacementRange: replacement];
            }
        }

        pub fn insert_text(&self, text: &str, replacement: NSRange) {
            let text = NSString::from_str(text);
            unsafe {
                let _: () = msg_send![&*self.0, insertText: &*text, replacementRange: replacement];
            }
        }

        /// Its keys interpreted by [`kana`] instead of the keyboard's input
        /// source, until the guard goes: AppKit's own input methods don't
        /// compose a test's keys.
        pub fn with_kana(&self) -> Kana {
            let class: *mut AnyClass = (self.0.class() as *const AnyClass).cast_mut();
            let sel = sel!(interpretKeyEvents:);
            // SAFETY: `kana` has `interpretKeyEvents:`'s signature.
            unsafe {
                let before = self.0.class().instance_method(sel).unwrap().implementation();
                let kana: Imp = std::mem::transmute(kana as unsafe extern "C-unwind" fn(_, _, _));
                class_replaceMethod(class, sel, kana, c"v@:@".as_ptr());
                Kana { class, before }
            }
        }
    }

    /// Puts a view's own `interpretKeyEvents:` back.
    pub struct Kana {
        class: *mut AnyClass,
        before: Imp,
    }

    impl Drop for Kana {
        fn drop(&mut self) {
            COMPOSING.take();
            // SAFETY: what was there.
            unsafe { class_replaceMethod(self.class, sel!(interpretKeyEvents:), self.before, c"v@:@".as_ptr()) };
        }
    }

    thread_local! {
        static COMPOSING: RefCell<String> = const { RefCell::new(String::new()) };
    }

    /// An input method as Japanese ones are, for one kana: K composes its
    /// romaji, and while it composes, Tab converts it to か and Return
    /// commits it, as its own keys; with nothing composed, Tab and Return
    /// are commands, and other keys type their characters.
    unsafe extern "C-unwind" fn kana(view: &AnyObject, _cmd: Sel, events: &NSArray<NSEvent>) {
        let event = events.objectAtIndex(0);
        let composing = COMPOSING.with_borrow(Clone::clone);
        let mark = |text: &str| {
            COMPOSING.set(text.to_owned());
            let string = NSString::from_str(text);
            let caret = NSRange::new(text.encode_utf16().count(), 0);
            let nowhere = NSRange::new(NOT_FOUND, 0);
            let _: () =
                unsafe { msg_send![view, setMarkedText: &*string, selectedRange: caret, replacementRange: nowhere] };
        };
        let insert = |text: &str| {
            COMPOSING.take();
            let string = NSString::from_str(text);
            let nowhere = NSRange::new(NOT_FOUND, 0);
            let _: () = unsafe { msg_send![view, insertText: &*string, replacementRange: nowhere] };
        };
        let command = |selector: Sel| {
            let _: () = unsafe { msg_send![view, doCommandBySelector: selector] };
        };
        match (event.keyCode(), composing.is_empty()) {
            (K, _) => mark(&format!("{composing}k")),
            (TAB, false) => mark("か"),
            (RETURN, false) => insert(&composing),
            (TAB, true) => command(sel!(insertTab:)),
            (RETURN, true) => command(sel!(insertNewline:)),
            _ => insert(&event.characters().map(|c| c.to_string()).unwrap_or_default()),
        }
    }
}

/// Keys an input method takes for its composition are its own, as in a
/// text field: a Japanese one's to compose, convert (Tab) and commit
/// (Return) aren't keys the app gets, but Tab and Return with no
/// composition going on are, and so are keys typing.
#[cfg(target_os = "macos")]
#[mitsuami_test::test]
async fn leaves_an_input_method_its_keys(app: TestApp) {
    use appkit_view::{K, NSEventModifierFlags as Flags, RETURN, TAB, View, X};

    let seen = Rc::new(RefCell::new(Vec::new()));
    let (log, on_input) = input_log();
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(200.px(), 100.px()).on_input(on_input).text_input(true)));
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.get(screen()).click_at(10.0, 10.0).await;
    let view = View::of(&handle(&seen));
    let _kana = view.with_kana();

    for (code, typed) in [(K, "k"), (TAB, "\t"), (RETURN, "\r"), (TAB, "\t"), (X, "x")] {
        view.press(code, Flags::empty(), typed, typed);
    }
    app.settle().await;

    let preedit = |text: &str, cursor| SurfaceInput::Preedit { text: text.to_owned(), cursor };
    let text: Vec<SurfaceInput> = log
        .borrow()
        .iter()
        .filter(|input| matches!(input, SurfaceInput::Text(_) | SurfaceInput::Preedit { .. }))
        .cloned()
        .collect();
    let expected =
        [preedit("k", Some(1)), preedit("か", Some(3)), preedit("", None), SurfaceInput::Text("か".to_owned())];
    assert_eq!(text, expected, "{log:?}");
    let expected = [(KeyCode::Tab, None, None), (KeyCode::KeyX, Some("x".to_owned()), Some('x'))];
    assert_eq!(typed(&log), expected, "{log:?}");
    assert_eq!(
        keys(&log),
        [(KeyCode::Tab, true), (KeyCode::Tab, false), (KeyCode::KeyX, true), (KeyCode::KeyX, false)]
    );
}

/// An input method can replace what keys typed: macOS's accents, offered
/// as a key is held, mark the letter it typed (the caret is after what
/// was typed, or the accents aren't offered), and replace it with the one
/// chosen. The app is told to take the letter back; what was typed before
/// a key that isn't text (an arrow) is out of reach.
#[cfg(target_os = "macos")]
#[mitsuami_test::test]
async fn takes_back_what_an_input_method_replaces(app: TestApp) {
    use appkit_view::{NOT_FOUND, NSEventModifierFlags as Flags, View};
    use mitsuami::appkit::objc2_foundation::NSRange;

    let seen = Rc::new(RefCell::new(Vec::new()));
    let (log, on_input) = input_log();
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(200.px(), 100.px()).on_input(on_input).text_input(true)));
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.get(screen()).click_at(10.0, 10.0).await;
    let view = View::of(&handle(&seen));

    // `kVK_ANSI_X`, `kVK_LeftArrow`, `kVK_ANSI_I`.
    view.press(0x07, Flags::empty(), "x", "x");
    view.press(0x7B, Flags::Function | Flags::NumericPad, "\u{F702}", "\u{F702}");
    assert_eq!(view.selected_range(), NSRange::new(0, 0));
    view.press(0x22, Flags::empty(), "i", "i");
    assert_eq!(view.selected_range(), NSRange::new(1, 0));
    app.settle().await;
    log.borrow_mut().clear();
    // Held: the letter is marked; one is chosen.
    view.set_marked_text("i", NSRange::new(1, 0), NSRange::new(0, 1));
    view.insert_text("í", NSRange::new(NOT_FOUND, 0));
    // Or chosen right away, replacing it.
    view.insert_text("ì", NSRange::new(0, 1));
    app.settle().await;

    let preedit = |text: &str, cursor| SurfaceInput::Preedit { text: text.to_owned(), cursor };
    let expected = [
        SurfaceInput::DeleteSurrounding { before: 1, after: 0 },
        preedit("i", Some(1)),
        preedit("", None),
        SurfaceInput::Text("í".to_owned()),
        SurfaceInput::DeleteSurrounding { before: 1, after: 0 },
        SurfaceInput::Text("ì".to_owned()),
    ];
    assert_eq!(*log.borrow(), expected);
    assert_eq!(view.selected_range(), NSRange::new(1, 0));
}

/// With input methods off, keys still type what they type, as they do
/// with them on, and it reads back as the app set it, as it's turned on
/// and off: a modal editor's keys outside insert mode are commands.
#[mitsuami_test::test]
async fn types_text_with_input_methods_off(app: TestApp) {
    let input_method = signal(false);
    let (log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).text_input(true).input_method(input_method)));
    app.settle().await;
    app.get(screen()).click_at(10.0, 10.0).await;

    app.get(screen()).type_text("aB").await;

    let expected = [(KeyCode::KeyA, Some("a".to_owned()), Some('a')), (KeyCode::KeyB, Some("B".to_owned()), Some('b'))];
    assert_eq!(typed(&log), expected, "{log:?}");
    let props = app.get(screen()).native_state().props;
    assert!(props.contains(&Prop::InputMethod(false)), "{props:?}");

    input_method.set(true);
    app.settle().await;

    let props = app.get(screen()).native_state().props;
    assert!(props.contains(&Prop::InputMethod(true)), "{props:?}");
    app.get(screen()).commit_text("日本").await;
    assert!(log.borrow().contains(&SurfaceInput::Text("日本".to_owned())), "{log:?}");
}

/// Whether it takes text, and where its caret is, read back as the app
/// set them, as the caret moves.
#[mitsuami_test::test]
async fn keeps_its_caret(app: TestApp) {
    let caret = signal(Some(Rect::new(10.0, 20.0, 8.0, 16.0)));
    let (_log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).text_input(true).text_cursor(caret)));
    app.settle().await;
    let props = app.get(screen()).native_state().props;
    assert!(props.contains(&Prop::TextInput(true)), "{props:?}");
    assert!(props.contains(&Prop::TextCursor(Some(Rect::new(10.0, 20.0, 8.0, 16.0)))), "{props:?}");

    caret.set(None);
    app.settle().await;

    let props = app.get(screen()).native_state().props;
    assert!(props.contains(&Prop::TextCursor(None)), "{props:?}");
}

/// On Windows, keys typed through the platform's own path: `WM_KEYDOWN`s
/// posted to the window with focus, which the run loop translates with
/// the thread's keyboard state and layout, as it does a real key. Shift
/// folds in, Control types nothing, a dead key composes with the next
/// key, AltGr types what the layout gives it (reported with Control and
/// Alt held, as Windows has it), and characters no key typed are an input
/// method's.
#[cfg(windows)]
#[mitsuami_test::test]
async fn types_through_windows_text_input(app: TestApp) {
    use mitsuami::raw_window_handle::RawWindowHandle;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetFocus() -> isize;
        fn GetAncestor(hwnd: isize, flags: u32) -> isize;
        fn FindWindowExW(parent: isize, after: isize, class: *const u16, title: *const u16) -> isize;
        fn PostMessageW(hwnd: isize, msg: u32, w: usize, l: isize) -> i32;
        fn SetKeyboardState(state: *const u8) -> i32;
        fn MapVirtualKeyW(code: u32, map: u32) -> u32;
        fn GetKeyboardLayout(thread: u32) -> isize;
        fn GetKeyboardLayoutList(count: i32, list: *mut isize) -> i32;
        fn LoadKeyboardLayoutW(id: *const u16, flags: u32) -> isize;
        fn ActivateKeyboardLayout(layout: isize, flags: u32) -> isize;
        fn UnloadKeyboardLayout(layout: isize) -> i32;
    }
    const SHIFT: u8 = 0x10;
    const CONTROL: u8 = 0x11;
    const MENU: u8 = 0x12;
    const LCONTROL: u8 = 0xA2;
    const RMENU: u8 = 0xA5;
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();

    let seen = Rc::new(RefCell::new(Vec::new()));
    let (log, on_input) = input_log();
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(200.px(), 100.px()).on_input(on_input).text_input(true)));
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.get(screen()).click_at(10.0, 10.0).await;
    let RawWindowHandle::Win32(child) = handle(&seen).window_handle().unwrap().as_raw() else { panic!() };
    // XAML's content island has Win32 focus; it may not in a window that
    // isn't the active one, but still gets the keys posted to it.
    let focus = match unsafe { GetFocus() } {
        0 => unsafe {
            let window = GetAncestor(child.hwnd.get(), 2);
            let bridge = wide("Microsoft.UI.Content.DesktopChildSiteBridge");
            let island = FindWindowExW(window, 0, bridge.as_ptr(), std::ptr::null());
            FindWindowExW(island, 0, wide("InputSiteWindowClass").as_ptr(), std::ptr::null())
        },
        focus => focus,
    };
    assert_ne!(focus, 0, "no window to post keys to");
    let press = async |key: u32, held: &[u8]| {
        let mut state = [0u8; 256];
        for k in held {
            state[*k as usize] = 0x80;
        }
        let scan = (unsafe { MapVirtualKeyW(key, 0) } as isize) << 16;
        // The release once the press is handled: posted with it, it would
        // be queued before the characters the press is translated to, which
        // a real key's release, coming as input, never is.
        unsafe {
            SetKeyboardState(state.as_ptr());
            PostMessageW(focus, 0x100, key as usize, 1 | scan);
        }
        app.settle().await;
        unsafe { PostMessageW(focus, 0x101, key as usize, 1 | scan | 3 << 30) };
        app.settle().await;
        unsafe { SetKeyboardState([0u8; 256].as_ptr()) };
    };

    press(0x41, &[SHIFT]).await;
    press(0x41, &[CONTROL]).await;
    // US-International for this thread, unloaded after if it wasn't there.
    let mut had = [0isize; 64];
    let count = unsafe { GetKeyboardLayoutList(had.len() as i32, had.as_mut_ptr()) }.max(0) as usize;
    let previous = unsafe { GetKeyboardLayout(0) };
    let intl = unsafe { LoadKeyboardLayoutW(wide("00020409").as_ptr(), 0) };
    unsafe { ActivateKeyboardLayout(intl, 0) };
    press(0xDE, &[]).await;
    press(0x45, &[]).await;
    press(0x45, &[CONTROL, MENU, LCONTROL, RMENU]).await;
    unsafe { ActivateKeyboardLayout(previous, 0) };
    if !had[..count].contains(&intl) {
        unsafe { UnloadKeyboardLayout(intl) };
    }
    unsafe { PostMessageW(focus, 0x102, 0x65E5, 1) };
    app.settle().await;

    let expected = [
        (KeyCode::KeyA, Some("A".to_owned()), Some('a')),
        (KeyCode::KeyA, None, Some('a')),
        (KeyCode::Quote, None, Some('\'')),
        (KeyCode::KeyE, Some("é".to_owned()), Some('e')),
        (KeyCode::KeyE, Some("é".to_owned()), Some('e')),
    ];
    assert_eq!(typed(&log), expected, "{log:?}");
    // The dead acute shows as a composition until the E composes it in, as
    // in a text field: it ends before the E's text.
    let around_dead: Vec<SurfaceInput> = log
        .borrow()
        .iter()
        .filter(|input| matches!(input, SurfaceInput::Key { pressed: true, .. } | SurfaceInput::Preedit { .. }))
        .skip_while(|input| !matches!(input, SurfaceInput::Key { code: KeyCode::Quote, .. }))
        .skip(1)
        .take(3)
        .map(|input| match input {
            SurfaceInput::Key { code, text, .. } => SurfaceInput::Text(format!("{code:?} {text:?}")),
            other => other.clone(),
        })
        .collect();
    let preedit = |text: &str, cursor| SurfaceInput::Preedit { text: text.to_owned(), cursor };
    assert_eq!(
        around_dead,
        [preedit("'", Some(1)), preedit("", None), SurfaceInput::Text("KeyE Some(\"é\")".to_owned())],
        "{log:?}"
    );
    let altgr = log.borrow().iter().rev().find_map(|input| match input {
        SurfaceInput::Key { pressed: true, modifiers, .. } => Some(*modifiers),
        _ => None,
    });
    assert_eq!(altgr, Some(Modifiers { control: true, alt: true, ..Modifiers::default() }), "{log:?}");
    assert!(log.borrow().contains(&SurfaceInput::Text("日".to_owned())), "{log:?}");
}

/// Whether Windows' text services (TSF) have a document with a context
/// focused on this thread: somewhere an input method can compose, as in a
/// text field. The thread's `ITfThreadMgr` (one per thread), its focused
/// `ITfDocumentMgr` (`GetFocus`, slot 7) and that one's top context
/// (`GetTop`, slot 6). XAML leaves the last one focused as focus moves on
/// to an element that isn't a text control, so this can't tell that it
/// did: [`text_box_has_focus`] can.
#[cfg(windows)]
fn input_methods_have_a_document() -> bool {
    use std::ffi::c_void;

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoCreateInstance(
            clsid: *const [u8; 16],
            outer: *mut c_void,
            context: u32,
            iid: *const [u8; 16],
            out: *mut *mut c_void,
        ) -> i32;
    }
    /// A COM method `index` of `object`'s vtable, taking one out-pointer.
    unsafe fn call(object: *mut c_void, index: usize) -> *mut c_void {
        type Method = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
        let vtable = unsafe { *(object as *const *const usize) };
        let method: Method = unsafe { std::mem::transmute(*vtable.add(index)) };
        let mut out = std::ptr::null_mut();
        unsafe { method(object, &mut out) };
        out
    }
    /// A GUID's bytes, as `GUID` lays them out.
    fn guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..4].copy_from_slice(&data1.to_le_bytes());
        bytes[4..6].copy_from_slice(&data2.to_le_bytes());
        bytes[6..8].copy_from_slice(&data3.to_le_bytes());
        bytes[8..].copy_from_slice(&data4);
        bytes
    }
    unsafe {
        let clsid = guid(0x529a9e6b, 0x6587, 0x4f23, [0xab, 0x9e, 0x9c, 0x7d, 0x68, 0x3e, 0x3c, 0x50]);
        let iid = guid(0xaa80e801, 0x2021, 0x11d2, [0x93, 0xe0, 0x00, 0x60, 0xb0, 0x67, 0xb8, 0x6e]);
        let mut manager = std::ptr::null_mut();
        if CoCreateInstance(&clsid, std::ptr::null_mut(), 1, &iid, &mut manager) < 0 || manager.is_null() {
            return false;
        }
        let document = call(manager, 7);
        !document.is_null() && !call(document, 6).is_null()
    }
}

/// Whether the text box in `window` (its one edit control, as UI
/// Automation sees the window) has XAML's focus, where input methods
/// compose, rather than the canvas: `IUIAutomation::ElementFromHandle`
/// (slot 6), `CreatePropertyCondition` (23) on the control type,
/// `FindFirst` (5) among its descendants, and that element's
/// `HasKeyboardFocus` (`GetCurrentPropertyValue`, 10). The window needn't
/// be the active one, as `GetFocusedElement` needs.
#[cfg(windows)]
fn text_box_has_focus(window: isize) -> bool {
    use std::ffi::c_void;

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoCreateInstance(
            clsid: *const [u8; 16],
            outer: *mut c_void,
            context: u32,
            iid: *const [u8; 16],
            out: *mut *mut c_void,
        ) -> i32;
    }
    /// A `VARIANT`: its type, then its value (`VT_I4`'s, `VT_BOOL`'s).
    #[repr(C)]
    #[derive(Default)]
    struct Variant {
        kind: u16,
        _reserved: [u16; 3],
        value: u64,
        _rest: u64,
    }
    const VT_I4: u16 = 3;
    const CONTROL_TYPE: i32 = 30003;
    const HAS_KEYBOARD_FOCUS: i32 = 30008;
    const EDIT: u64 = 50004;
    const DESCENDANTS: i32 = 4;
    unsafe fn method<F>(object: *mut c_void, index: usize) -> F {
        let vtable = unsafe { *(object as *const *const usize) };
        unsafe { std::mem::transmute_copy(&*vtable.add(index)) }
    }
    fn guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..4].copy_from_slice(&data1.to_le_bytes());
        bytes[4..6].copy_from_slice(&data2.to_le_bytes());
        bytes[6..8].copy_from_slice(&data3.to_le_bytes());
        bytes[8..].copy_from_slice(&data4);
        bytes
    }
    type Out = *mut *mut c_void;
    unsafe {
        let clsid = guid(0xff48dba4, 0x60ef, 0x4201, [0xaa, 0x87, 0x54, 0x10, 0x3e, 0xef, 0x59, 0x4e]);
        let iid = guid(0x30cbe57d, 0xd9d0, 0x452a, [0xab, 0x13, 0x7a, 0xc5, 0xac, 0x48, 0x25, 0xee]);
        let mut automation = std::ptr::null_mut();
        if CoCreateInstance(&clsid, std::ptr::null_mut(), 1, &iid, &mut automation) < 0 || automation.is_null() {
            return false;
        }
        let mut root = std::ptr::null_mut();
        let from_handle: unsafe extern "system" fn(*mut c_void, isize, Out) -> i32 = method(automation, 6);
        if from_handle(automation, window, &mut root) < 0 || root.is_null() {
            return false;
        }
        let edit = Variant { kind: VT_I4, value: EDIT, ..Variant::default() };
        let mut condition = std::ptr::null_mut();
        let property_condition: unsafe extern "system" fn(*mut c_void, i32, Variant, Out) -> i32 =
            method(automation, 23);
        if property_condition(automation, CONTROL_TYPE, edit, &mut condition) < 0 || condition.is_null() {
            return false;
        }
        let mut found = std::ptr::null_mut();
        let find_first: unsafe extern "system" fn(*mut c_void, i32, *mut c_void, Out) -> i32 = method(root, 5);
        if find_first(root, DESCENDANTS, condition, &mut found) < 0 || found.is_null() {
            return false;
        }
        let mut focused = Variant::default();
        let value: unsafe extern "system" fn(*mut c_void, i32, *mut Variant) -> i32 = method(found, 10);
        value(found, HAS_KEYBOARD_FOCUS, &mut focused) >= 0 && focused.value as u16 != 0
    }
}

/// On Windows, taking text with focus from a click, an input method has
/// somewhere to compose. On the canvas alone XAML gave them no document,
/// and Japanese typed as Latin letters.
#[cfg(windows)]
#[mitsuami_test::test]
async fn gives_input_methods_a_document_on_windows(app: TestApp) {
    let (_log, on_input) = input_log();
    app.mount(move || Column::new().child(taking_input(on_input).text_input(true)));
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.get(screen()).click_at(10.0, 10.0).await;
    app.settle().await;

    assert!(input_methods_have_a_document(), "no text services document to compose in");
}

/// The same with focus from code as the surface is built, as an editor
/// that is all surface focuses it: the focus may come before the surface
/// takes text.
#[cfg(windows)]
#[mitsuami_test::test]
async fn gives_input_methods_a_document_when_focused_from_code(app: TestApp) {
    let (_log, on_input) = input_log();
    app.mount(move || {
        let surface = node_ref();
        surface.focus();
        Column::new().child(taking_input(on_input).node_ref(surface).text_input(true))
    });
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.expect(screen()).to_be_focused().await;

    assert!(input_methods_have_a_document(), "no text services document to compose in");
}

/// The XAML window a surface is in, from its child window.
#[cfg(windows)]
fn xaml_window(seen: &Rc<RefCell<Vec<Seen>>>) -> isize {
    use mitsuami::raw_window_handle::RawWindowHandle;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetAncestor(hwnd: isize, flags: u32) -> isize;
    }
    const ROOT: u32 = 2;
    let RawWindowHandle::Win32(child) = handle(seen).window_handle().unwrap().as_raw() else { panic!() };
    unsafe { GetAncestor(child.hwnd.get(), ROOT) }
}

/// On Windows, a click on the surface itself (its child window, over the
/// canvas, which the pointer clicks) leaves focus in its text box, where
/// input methods compose. It focused the canvas, and Japanese typed as
/// Latin letters after it.
#[cfg(windows)]
#[mitsuami_test::test]
async fn keeps_input_methods_through_a_click_on_windows(app: TestApp) {
    use mitsuami::raw_window_handle::RawWindowHandle;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageW(hwnd: isize, msg: u32, w: usize, l: isize) -> isize;
    }
    const LBUTTONDOWN: u32 = 0x201;
    const LBUTTONUP: u32 = 0x202;
    const LBUTTON: usize = 1;

    let seen = Rc::new(RefCell::new(Vec::new()));
    let (_log, on_input) = input_log();
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(200.px(), 100.px()).on_input(on_input).text_input(true)));
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.get(screen()).click_at(10.0, 10.0).await;
    app.settle().await;
    let window = xaml_window(&seen);
    assert!(text_box_has_focus(window), "the text box hasn't focus before the click");
    let RawWindowHandle::Win32(child) = handle(&seen).window_handle().unwrap().as_raw() else { panic!() };

    let at = 10 | 10 << 16;
    unsafe {
        SendMessageW(child.hwnd.get(), LBUTTONDOWN, LBUTTON, at);
        SendMessageW(child.hwnd.get(), LBUTTONUP, 0, at);
    }
    app.settle().await;

    app.expect(screen()).to_be_focused().await;
    assert!(text_box_has_focus(window), "the click took focus from the text box");
}

/// On Windows, with input methods off the canvas has focus, not the text
/// box, so keys type as letters; turned back on, focus goes to the box,
/// where they compose.
#[cfg(windows)]
#[mitsuami_test::test]
async fn takes_focus_from_the_text_box_with_input_methods_off_on_windows(app: TestApp) {
    let input_method = signal(true);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (_log, on_input) = input_log();
    let s = seen.clone();
    app.mount(move || {
        Column::new()
            .child(surface(&s).size(200.px(), 100.px()).on_input(on_input).text_input(true).input_method(input_method))
    });
    app.settle().await;
    if app.is_headless() {
        return;
    }
    app.get(screen()).click_at(10.0, 10.0).await;
    app.settle().await;
    let window = xaml_window(&seen);
    assert!(text_box_has_focus(window), "the text box hasn't focus");

    input_method.set(false);
    app.settle().await;
    app.expect(screen()).to_be_focused().await;
    assert!(!text_box_has_focus(window), "the text box has focus with input methods off");

    input_method.set(true);
    app.settle().await;
    app.expect(screen()).to_be_focused().await;
    assert!(text_box_has_focus(window), "the text box hasn't focus with input methods back on");
}

mitsuami_test::main!();
