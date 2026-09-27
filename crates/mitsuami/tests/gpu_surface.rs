//! `GpuSurface`: a native surface the app presents to with its own GPU API.
//! It's reported once it exists, then its size in pixels whenever that or
//! its scale changes; its handle keeps the native surface after the widget
//! is gone. It has no natural size, and reads as an image named by its
//! label.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::WidgetKind;
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

#[mitsuami_test::test]
async fn reads_as_an_image_named_by_its_label(app: TestApp) {
    app.mount(|| Column::new().child(GpuSurface::new().label("Screen").size(64.px(), 64.px())));

    app.expect(screen()).to_be_visible().await;
}

mitsuami_test::main!();
