//! `GpuSurface`: a native surface the app presents to with its own GPU API.
//! It's reported once it exists, then its size in pixels whenever that or
//! its scale changes; its handle keeps the native surface after the widget
//! is gone. It has no natural size, and reads as an image named by its
//! label. With `on_input` it takes focus, keys and the pointer; its
//! pointer lock and keyboard grab end as the platform ends them.

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

/// A native backend hands out its platform's handles; headless has none
/// to give.
#[mitsuami_test::test]
async fn hands_out_raw_window_handles(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    app.mount(move || Column::new().child(surface(&s).size(64.px(), 64.px())));
    app.settle().await;

    let handle = handle(&seen);
    if app.is_headless() {
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
    if !app.is_headless() {
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

/// The app's cursor over it: the platform's, none, or an image with its
/// hotspot. The native surface carries it (the mirror check compares).
#[mitsuami_test::test]
async fn shows_the_app_s_cursor(app: TestApp) {
    let arrow = Pixels::new(2, 2, vec![255; 16]).scale(2.0);
    let cursor = signal(Cursor::Default);
    app.mount(move || Column::new().child(surface_only().cursor(cursor)));
    app.settle().await;

    for next in
        [Cursor::Image { pixels: arrow.clone(), hotspot: Point::new(0.5, 0.5) }, Cursor::Hidden, Cursor::Default]
    {
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
        .copied()
        .collect();
    assert_eq!(moves, [SurfaceInput::Motion { dx: 3.0, dy: -2.0 }, SurfaceInput::RawMotion { dx: 3.0, dy: -2.0 }]);
}

mitsuami_test::main!();
