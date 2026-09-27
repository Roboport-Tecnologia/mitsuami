//! `GpuSurface`: a view whose backing layer is a `CAMetalLayer`, which the
//! app presents to from its own thread (wgpu takes the view's layer as it
//! is).

use std::cell::RefCell;
use std::ptr::NonNull;

use mitsuami_core::raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HandleError, RawDisplayHandle, RawWindowHandle,
};
use mitsuami_core::{EventSink, NativeSurface, NodeId, SurfaceHandle, SurfaceSize, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAccessibility, NSAccessibilityImageRole, NSView, NSViewLayerContentsPlacement};
use objc2_foundation::NSSize;

use crate::classes::zero_rect;

pub(crate) struct SurfaceIvars {
    id: NodeId,
    events: EventSink,
    /// Taken when the node is destroyed: the app's handle may keep the view
    /// alive, but it no longer reports (and holds no handle to itself).
    handle: RefCell<Option<SurfaceHandle>>,
}

define_class!(
    /// A flipped, layer-backed view whose backing layer is a
    /// `CAMetalLayer`, sized with the view by AppKit.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SurfaceIvars]
    pub(crate) struct SurfaceView;

    impl SurfaceView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method_id(makeBackingLayer))]
        fn make_backing_layer(&self) -> Retained<AnyObject> {
            let class = AnyClass::get(c"CAMetalLayer").expect("QuartzCore's CAMetalLayer");
            unsafe { msg_send![class, layer] }
        }

        /// The app draws the layer; AppKit never does.
        #[unsafe(method(wantsUpdateLayer))]
        fn wants_update_layer(&self) -> bool {
            true
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.report();
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn did_change_backing_properties(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidChangeBackingProperties] };
            if let Some(window) = self.window() {
                let layer: Option<Retained<AnyObject>> = unsafe { msg_send![self, layer] };
                if let Some(layer) = layer {
                    let _: () = unsafe { msg_send![&*layer, setContentsScale: window.backingScaleFactor()] };
                }
            }
            self.report();
        }
    }
);

impl SurfaceView {
    /// The view, and the handle the app gets, whose size it keeps.
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> (Retained<SurfaceView>, SurfaceHandle) {
        let this = SurfaceView::alloc(mtm).set_ivars(SurfaceIvars { id, events, handle: RefCell::new(None) });
        let view: Retained<SurfaceView> = unsafe { msg_send![super(this), initWithFrame: zero_rect()] };
        view.setWantsLayer(true);
        // Until the app presents at a new size, its last frame stays as it
        // was, at the top left, rather than stretched to the new bounds
        // (AppKit's default for layer-backed views).
        view.setLayerContentsPlacement(NSViewLayerContentsPlacement::TopLeft);
        view.setAccessibilityElement(true);
        view.setAccessibilityRole(Some(unsafe { NSAccessibilityImageRole }));
        let handle = SurfaceHandle::new(AppKitSurface(Some(Retained::into_super(view.clone()))));
        *view.ivars().handle.borrow_mut() = Some(handle.clone());
        (view, handle)
    }

    /// The node is gone; the app may still hold the view.
    pub(crate) fn detach(&self) {
        self.ivars().handle.borrow_mut().take();
    }

    /// Its size in pixels, as the layer's drawable should be, if it changed.
    fn report(&self) {
        let ivars = self.ivars();
        let Some(handle) = ivars.handle.borrow().clone() else { return };
        let pixels = self.convertSizeToBacking(self.bounds().size);
        let scale = self.window().map_or(1.0, |w| w.backingScaleFactor());
        let size = SurfaceSize {
            width: pixels.width.round() as u32,
            height: pixels.height.round() as u32,
            scale: scale as f32,
        };
        if handle.set_size(size) {
            ivars.events.emit(ivars.id, UiEvent::SurfaceResized(size));
        }
    }
}

/// The view, for `raw-window-handle`. Views belong to the main thread:
/// the last handle dropped elsewhere sends it there to be released.
struct AppKitSurface(Option<Retained<NSView>>);

// SAFETY: the view is only messaged on the main thread; other threads only
// read its address, and send it back to the main thread to be released.
unsafe impl Send for AppKitSurface {}
unsafe impl Sync for AppKitSurface {}

impl NativeSurface for AppKitSurface {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError> {
        let view = self.0.as_ref().ok_or(HandleError::Unavailable)?;
        Ok(RawWindowHandle::AppKit(AppKitWindowHandle::new(NonNull::from(&**view).cast())))
    }

    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError> {
        Ok(RawDisplayHandle::AppKit(AppKitDisplayHandle::new()))
    }
}

impl Drop for AppKitSurface {
    fn drop(&mut self) {
        if MainThreadMarker::new().is_some() {
            return;
        }
        struct Release(Retained<NSView>);
        // SAFETY: released on the main thread, never touched on the way.
        unsafe impl Send for Release {}
        if let Some(view) = self.0.take() {
            let release = Release(view);
            dispatch2::DispatchQueue::main().exec_async(move || {
                let release = release;
                drop(release.0);
            });
        }
    }
}
