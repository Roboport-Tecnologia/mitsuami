//! `GpuSurface`: a view whose backing layer is a `CAMetalLayer`, which the
//! app presents to from its own thread (wgpu takes the view's layer as it
//! is).
//!
//! Taking input, it's a responder like a game's view: it takes the first
//! responder from a click and Tab, keys come to `keyDown:` and
//! `flagsChanged:` after the menus' key equivalents, and a local event
//! monitor catches their `keyUp:`s (AppKit sends none for keys released
//! while Command is held). Its pointer lock is what games do: the cursor
//! hidden and dissociated from the mouse, whose moves still come as
//! events with deltas, and GameController's `GCMouse` gives the same
//! moves before the system's acceleration. Its keyboard grab takes every key in that monitor
//! before AppKit dispatches it, menus' key equivalents too, and turns off
//! Command-Tab with the app's presentation options.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ptr::NonNull;

use block2::RcBlock;
use mitsuami_core::backend::{Key, SyntheticInput};
use mitsuami_core::raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HandleError, RawDisplayHandle, RawWindowHandle,
};
use mitsuami_core::{
    ActionError, Cursor, EventSink, ImageSource, KeyCode, Modifiers, MouseButton, NativeSurface, NodeId, Pixels, Point,
    ScrollDelta, SurfaceHandle, SurfaceInput, SurfaceSize, UiEvent,
};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyClass, AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    AllocAnyThread, ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityImageRole, NSApplication, NSApplicationPresentationOptions, NSCursor, NSEvent,
    NSEventMask, NSEventModifierFlags, NSEventType, NSResponder, NSScreen, NSTrackingArea, NSTrackingAreaOptions,
    NSView, NSViewLayerContentsPlacement, NSWindow, NSWindowDidResignKeyNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue, NSPoint, NSSize, NSString};

use objc2_game_controller::{GCMouse, GCMouseDidConnectNotification, GCMouseInput};

use crate::classes::zero_rect;

#[repr(C)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGAssociateMouseAndMouseCursorPosition(connected: u32) -> i32;
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
}

type Observer = Retained<ProtocolObject<dyn NSObjectProtocol>>;

pub(crate) struct SurfaceIvars {
    id: NodeId,
    events: EventSink,
    /// Taken when the node is destroyed: the app's handle may keep the view
    /// alive, but it no longer reports (and holds no handle to itself).
    handle: RefCell<Option<SurfaceHandle>>,
    takes_input: Cell<bool>,
    /// Keys down, by their virtual key codes: for modifier keys, which
    /// only report their flags, and so a release is never reported twice.
    down: RefCell<HashSet<u16>>,
    tracking: RefCell<Option<Retained<NSTrackingArea>>>,
    /// The monitor for key events, while it's the first responder.
    monitor: RefCell<Option<Retained<AnyObject>>>,
    /// What the app asked for, and what's in effect: a lock or a grab
    /// asked for before the view is in a window waits for one.
    lock_wanted: Cell<bool>,
    locked: Cell<bool>,
    grab_wanted: Cell<bool>,
    /// The presentation options before the grab, while it's in effect.
    grab: Cell<Option<NSApplicationPresentationOptions>>,
    /// Ends the lock and the grab when the window stops being the key one.
    resign_observer: RefCell<Option<Observer>>,
    /// While locked: mice that connect get the raw motion handler too.
    mouse_observer: RefCell<Option<Observer>>,
    /// Lets go of the keys down when the window stops being the key one,
    /// while it's the first responder.
    key_observer: RefCell<Option<Observer>>,
    /// The cursor over it (none: the arrow), and what the app asked for,
    /// which an `NSCursor` can't give back.
    cursor: RefCell<Option<Retained<NSCursor>>>,
    cursor_prop: RefCell<Cursor>,
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

        /// A lock or grab asked for before it had a window.
        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            if self.window().is_some() {
                self.apply_lock();
                self.apply_grab();
            }
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            self.ivars().takes_input.get()
        }

        #[unsafe(method(becomeFirstResponder))]
        fn become_first_responder(&self) -> bool {
            let became: bool = unsafe { msg_send![super(self), becomeFirstResponder] };
            if became {
                self.watch_keys();
            }
            became
        }

        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            let resigned: bool = unsafe { msg_send![super(self), resignFirstResponder] };
            if resigned {
                self.stop_watching_keys();
                self.release_keys();
                // A grab lasts while the surface has focus.
                if self.ungrab() {
                    self.end_grab();
                }
            }
            resigned
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            // Control-Tab leaves a view that takes Tab, as it leaves a
            // text view: the window moves focus on.
            let flags = event.modifierFlags();
            if !self.ivars().takes_input.get()
                || (event.keyCode() == TAB && flags.contains(NSEventModifierFlags::Control))
            {
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
                return;
            }
            self.key(event, true);
        }

        /// Only synthesized: the monitor takes real ones.
        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            if self.ivars().takes_input.get() {
                self.key(event, false);
            } else {
                let _: () = unsafe { msg_send![super(self), keyUp: event] };
            }
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            if self.ivars().takes_input.get() {
                self.modifier_key(event);
            } else {
                let _: () = unsafe { msg_send![super(self), flagsChanged: event] };
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if !self.pointer_event(event, Some(true)) {
                let _: () = unsafe { msg_send![super(self), mouseDown: event] };
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            if !self.pointer_event(event, Some(false)) {
                let _: () = unsafe { msg_send![super(self), mouseUp: event] };
            }
        }

        /// A context menu, if the app gave it one, still opens.
        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.pointer_event(event, Some(true));
            let _: () = unsafe { msg_send![super(self), rightMouseDown: event] };
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            if !self.pointer_event(event, Some(false)) {
                let _: () = unsafe { msg_send![super(self), rightMouseUp: event] };
            }
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            if !self.pointer_event(event, Some(true)) {
                let _: () = unsafe { msg_send![super(self), otherMouseDown: event] };
            }
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            if !self.pointer_event(event, Some(false)) {
                let _: () = unsafe { msg_send![super(self), otherMouseUp: event] };
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            if !self.pointer_event(event, None) {
                let _: () = unsafe { msg_send![super(self), mouseMoved: event] };
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            if !self.pointer_event(event, None) {
                let _: () = unsafe { msg_send![super(self), mouseDragged: event] };
            }
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            if !self.pointer_event(event, None) {
                let _: () = unsafe { msg_send![super(self), rightMouseDragged: event] };
            }
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            if !self.pointer_event(event, None) {
                let _: () = unsafe { msg_send![super(self), otherMouseDragged: event] };
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, event: &NSEvent) {
            if self.ivars().takes_input.get() && !self.ivars().locked.get() {
                self.emit(SurfaceInput::PointerLeft);
            } else {
                let _: () = unsafe { msg_send![super(self), mouseExited: event] };
            }
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            if !self.ivars().takes_input.get() {
                let _: () = unsafe { msg_send![super(self), scrollWheel: event] };
                return;
            }
            // AppKit's deltas are how far the content moves, with the
            // user's scrolling direction applied: towards the end is less.
            let (x, y) = (-event.scrollingDeltaX() as f32, -event.scrollingDeltaY() as f32);
            let delta =
                if event.hasPreciseScrollingDeltas() { ScrollDelta::Points { x, y } } else { ScrollDelta::Lines { x, y } };
            self.emit(SurfaceInput::Scroll { delta, modifiers: modifiers(event.modifierFlags()) });
        }

        /// The app's cursor, over the whole view; AppKit shows it while
        /// the window is the key one.
        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            if let Some(cursor) = &*self.ivars().cursor.borrow() {
                self.addCursorRect_cursor(self.bounds(), cursor);
            }
        }

        /// Moves, entering and leaving, while it takes input.
        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
            self.track();
        }
    }
);

/// `kVK_Tab`.
const TAB: u16 = 0x30;

impl SurfaceView {
    /// The view, and the handle the app gets, whose size it keeps.
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> (Retained<SurfaceView>, SurfaceHandle) {
        let this = SurfaceView::alloc(mtm).set_ivars(SurfaceIvars {
            id,
            events,
            handle: RefCell::new(None),
            takes_input: Cell::new(false),
            down: RefCell::default(),
            tracking: RefCell::new(None),
            monitor: RefCell::new(None),
            lock_wanted: Cell::new(false),
            locked: Cell::new(false),
            grab_wanted: Cell::new(false),
            grab: Cell::new(None),
            resign_observer: RefCell::new(None),
            mouse_observer: RefCell::new(None),
            key_observer: RefCell::new(None),
            cursor: RefCell::new(None),
            cursor_prop: RefCell::new(Cursor::Default),
        });
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

    /// The node is gone; the app may still hold the view. It lets go of
    /// the pointer and keyboard without reporting it.
    pub(crate) fn detach(&self) {
        self.ivars().handle.borrow_mut().take();
        self.ivars().lock_wanted.set(false);
        self.ivars().grab_wanted.set(false);
        self.unlock();
        self.ungrab();
        self.stop_watching_keys();
        self.set_takes_input(false);
    }

    pub(crate) fn takes_input(&self) -> bool {
        self.ivars().takes_input.get()
    }

    pub(crate) fn pointer_locked(&self) -> bool {
        self.ivars().locked.get()
    }

    pub(crate) fn keyboard_grabbed(&self) -> bool {
        self.ivars().grab.get().is_some()
    }

    pub(crate) fn set_takes_input(&self, takes: bool) {
        if self.ivars().takes_input.replace(takes) == takes {
            return;
        }
        self.track();
        if !takes && let Some(window) = self.window() {
            let first: Option<Retained<NSResponder>> = window.firstResponder();
            if first.is_some_and(|r| std::ptr::eq(&*r, self.as_super().as_super())) {
                window.makeFirstResponder(None);
            }
        }
    }

    pub(crate) fn cursor(&self) -> Cursor {
        self.ivars().cursor_prop.borrow().clone()
    }

    /// An image, or a clear one for none: hiding the cursor with
    /// `NSCursor.hide` would hide it everywhere until it moved out.
    pub(crate) fn set_cursor(&self, cursor: &Cursor) {
        let ns_cursor = match cursor {
            Cursor::Default => None,
            Cursor::Hidden => cursor_image(&Pixels::new(1, 1, vec![0; 4]), Point::ZERO),
            Cursor::Image { pixels, hotspot } => cursor_image(pixels, *hotspot),
        };
        *self.ivars().cursor.borrow_mut() = ns_cursor;
        *self.ivars().cursor_prop.borrow_mut() = cursor.clone();
        if let Some(window) = self.window() {
            window.invalidateCursorRectsForView(self);
        }
    }

    pub(crate) fn set_pointer_lock(&self, on: bool) {
        self.ivars().lock_wanted.set(on);
        if on {
            self.apply_lock();
        } else {
            self.unlock();
        }
    }

    pub(crate) fn set_keyboard_grab(&self, on: bool) {
        self.ivars().grab_wanted.set(on);
        if on {
            self.apply_grab();
        } else {
            self.ungrab();
        }
    }

    fn emit(&self, input: SurfaceInput) {
        let ivars = self.ivars();
        if ivars.handle.borrow().is_some() {
            ivars.events.emit(ivars.id, UiEvent::SurfaceInput(input));
        }
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

    fn track(&self) {
        let ivars = self.ivars();
        if let Some(area) = ivars.tracking.borrow_mut().take() {
            self.removeTrackingArea(&area);
        }
        if !ivars.takes_input.get() {
            return;
        }
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::ActiveInActiveApp
            | NSTrackingAreaOptions::InVisibleRect;
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                zero_rect(),
                options,
                Some(self),
                None,
            )
        };
        self.addTrackingArea(&area);
        *ivars.tracking.borrow_mut() = Some(area);
    }

    // ------------------------------------------------------------- keys

    fn key(&self, event: &NSEvent, pressed: bool) {
        let code = event.keyCode();
        // Only key downs and ups know whether they repeat; asking a
        // modifier key's `flagsChanged:` event raises an exception.
        let repeat = pressed && event.r#type() == NSEventType::KeyDown && event.isARepeat();
        let was_down = if pressed {
            !self.ivars().down.borrow_mut().insert(code)
        } else {
            self.ivars().down.borrow_mut().remove(&code)
        };
        // A release for a key it never saw go down (it went down elsewhere).
        if !pressed && !was_down {
            return;
        }
        let modifiers = modifiers(event.modifierFlags());
        self.emit(SurfaceInput::Key { code: KeyCode::from_mac(code), native: code as u32, pressed, repeat, modifiers });
    }

    /// Modifier keys only change the flags: each side's device flag says
    /// whether it's down. Caps Lock's flag is the lock's, and it comes
    /// once for each press, so it's reported down and up.
    fn modifier_key(&self, event: &NSEvent) {
        let code = event.keyCode();
        let flags = event.modifierFlags().0;
        let side = match code {
            0x38 => 0x02,   // left Shift
            0x3C => 0x04,   // right Shift
            0x3B => 0x01,   // left Control
            0x3E => 0x2000, // right Control
            0x3A => 0x20,   // left Option
            0x3D => 0x40,   // right Option
            0x37 => 0x08,   // left Command
            0x36 => 0x10,   // right Command
            0x39 => {
                let modifiers = modifiers(event.modifierFlags());
                for pressed in [true, false] {
                    let native = code as u32;
                    self.emit(SurfaceInput::Key { code: KeyCode::CapsLock, native, pressed, repeat: false, modifiers });
                }
                return;
            }
            _ => return,
        };
        let pressed = flags & side != 0;
        if pressed == self.ivars().down.borrow().contains(&code) {
            return;
        }
        self.key(event, pressed);
    }

    /// Releases every key it has down: focus left, or the window stopped
    /// being the key one, so their releases go elsewhere.
    fn release_keys(&self) {
        let down: Vec<u16> = self.ivars().down.borrow_mut().drain().collect();
        for code in down {
            let (native, modifiers) = (code as u32, Modifiers::default());
            let code = KeyCode::from_mac(code);
            self.emit(SurfaceInput::Key { code, native, pressed: false, repeat: false, modifiers });
        }
    }

    /// While it's the first responder: every key release (AppKit sends
    /// none for keys let go while Command is held), and while the keyboard
    /// is grabbed, every key, before menus' key equivalents and the
    /// window see it.
    fn watch_keys(&self) {
        if self.ivars().monitor.borrow().is_some() {
            return;
        }
        let weak = Weak::from_retained(&self.retain());
        let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            let Some(view) = weak.load() else { return event.as_ptr() };
            // SAFETY: AppKit hands the monitor a live event.
            let e = unsafe { event.as_ref() };
            let mtm = MainThreadMarker::from(&*view);
            let ours = e.window(mtm).zip(view.window()).is_some_and(|(a, b)| std::ptr::eq(&*a, &*b));
            if !ours {
                return event.as_ptr();
            }
            match e.r#type() {
                NSEventType::KeyUp => view.key(e, false),
                NSEventType::KeyDown if view.keyboard_grabbed() => view.key(e, true),
                NSEventType::FlagsChanged if view.keyboard_grabbed() => view.modifier_key(e),
                _ => return event.as_ptr(),
            }
            std::ptr::null_mut()
        });
        let mask = NSEventMask::KeyDown | NSEventMask::KeyUp | NSEventMask::FlagsChanged;
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &handler) };
        *self.ivars().monitor.borrow_mut() = monitor;
        // The window stays the first responder's when it stops being the
        // key one, but the keys' releases go to the new key window.
        let Some(window) = self.window() else { return };
        let weak = Weak::from_retained(&self.retain());
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            if let Some(view) = weak.load() {
                view.release_keys();
            }
        });
        let observer = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidResignKeyNotification),
                Some(&window),
                None,
                &block,
            )
        };
        *self.ivars().key_observer.borrow_mut() = Some(observer);
    }

    fn stop_watching_keys(&self) {
        if let Some(monitor) = self.ivars().monitor.borrow_mut().take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        if let Some(observer) = self.ivars().key_observer.borrow_mut().take() {
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref()) };
        }
    }

    // ---------------------------------------------------------- pointer

    /// A button (`Some(pressed)`) or a move; returns whether it took it.
    fn pointer_event(&self, event: &NSEvent, pressed: Option<bool>) -> bool {
        let ivars = self.ivars();
        if !ivars.takes_input.get() {
            return false;
        }
        let modifiers = modifiers(event.modifierFlags());
        let Some(pressed) = pressed else {
            if ivars.locked.get() {
                self.emit(SurfaceInput::Motion { dx: event.deltaX() as f32, dy: event.deltaY() as f32 });
            } else {
                self.emit(SurfaceInput::PointerMoved { position: self.position(event), modifiers });
            }
            return true;
        };
        if pressed && let Some(window) = self.window() {
            window.makeFirstResponder(Some(self));
        }
        let button = match event.r#type() {
            NSEventType::LeftMouseDown | NSEventType::LeftMouseUp => MouseButton::Primary,
            NSEventType::RightMouseDown | NSEventType::RightMouseUp => MouseButton::Secondary,
            _ => match event.buttonNumber() {
                2 => MouseButton::Middle,
                3 => MouseButton::Back,
                4 => MouseButton::Forward,
                n => MouseButton::Other(n as u16),
            },
        };
        let position = self.position(event);
        self.emit(SurfaceInput::Button { button, pressed, position, modifiers });
        true
    }

    fn position(&self, event: &NSEvent) -> Point {
        let at = self.convertPoint_fromView(event.locationInWindow(), None);
        Point::new(at.x as f32, at.y as f32)
    }

    /// Locks the pointer if the app asked and the window is the key one;
    /// in a window that isn't, it can't.
    fn apply_lock(&self) {
        let ivars = self.ivars();
        if !ivars.lock_wanted.get() || ivars.locked.get() {
            return;
        }
        let Some(window) = self.window() else { return };
        if !active(&window) {
            return self.end_lock();
        }
        // The cursor stays put, hidden, and the mouse's moves still come
        // as events, with their deltas.
        unsafe { CGAssociateMouseAndMouseCursorPosition(0) };
        NSCursor::hide();
        ivars.locked.set(true);
        self.warp_to_middle(&window);
        self.watch_window(&window);
        self.watch_mice();
    }

    /// Returns whether it was locked.
    fn unlock(&self) -> bool {
        if !self.ivars().locked.replace(false) {
            return false;
        }
        unsafe { CGAssociateMouseAndMouseCursorPosition(1) };
        NSCursor::unhide();
        self.stop_watching_window();
        self.stop_watching_mice();
        true
    }

    /// Raw motion while locked: `GCMouse`'s deltas are the device's, not
    /// affected by the pointer's speed setting (a small program showed the
    /// ratio to `NSEvent`'s changing with speed). Up is positive there.
    /// Its handlers run on the main queue, GameController's default. A
    /// mouse has one handler, so the last surface to lock has it.
    fn watch_mice(&self) {
        let mouse_handler = |view: &SurfaceView| {
            let weak = Weak::from_retained(&view.retain());
            RcBlock::new(move |_: NonNull<GCMouseInput>, dx: f32, dy: f32| {
                if MainThreadMarker::new().is_none() {
                    return;
                }
                if let Some(view) = weak.load()
                    && view.ivars().locked.get()
                {
                    view.emit(SurfaceInput::RawMotion { dx, dy: -dy });
                }
            })
        };
        let watch = move |mouse: &GCMouse, view: &SurfaceView| {
            if let Some(input) = unsafe { mouse.mouseInput() } {
                let handler = mouse_handler(view);
                unsafe { input.setMouseMovedHandler(RcBlock::as_ptr(&handler)) };
            }
        };
        for mouse in unsafe { GCMouse::mice() } {
            watch(&mouse, self);
        }
        let weak = Weak::from_retained(&self.retain());
        let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
            let Some(view) = weak.load() else { return };
            // SAFETY: the notification center hands the block a live one.
            let object = unsafe { notification.as_ref() }.object();
            if let Some(mouse) = object.and_then(|o| o.downcast::<GCMouse>().ok()) {
                watch(&mouse, &view);
            }
        });
        let observer = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(GCMouseDidConnectNotification),
                None,
                Some(&NSOperationQueue::mainQueue()),
                &block,
            )
        };
        *self.ivars().mouse_observer.borrow_mut() = Some(observer);
    }

    fn stop_watching_mice(&self) {
        if let Some(observer) = self.ivars().mouse_observer.borrow_mut().take() {
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref()) };
        }
        for mouse in unsafe { GCMouse::mice() } {
            if let Some(input) = unsafe { mouse.mouseInput() } {
                unsafe { input.setMouseMovedHandler(std::ptr::null_mut()) };
            }
        }
    }

    fn end_lock(&self) {
        self.unlock();
        self.ivars().lock_wanted.set(false);
        let ivars = self.ivars();
        if ivars.handle.borrow().is_some() {
            ivars.events.emit(ivars.id, UiEvent::PointerLockEnded);
        }
    }

    /// Puts the (hidden) cursor over the middle of the surface, so a
    /// click lands on it.
    fn warp_to_middle(&self, window: &NSWindow) {
        let bounds = self.bounds();
        let middle =
            NSPoint::new(bounds.origin.x + bounds.size.width / 2.0, bounds.origin.y + bounds.size.height / 2.0);
        let on_screen = window.convertPointToScreen(self.convertPoint_toView(middle, None));
        // Quartz's global coordinates start at the top left of the main
        // display; AppKit's at its bottom left.
        let mtm = MainThreadMarker::from(self);
        let Some(main) = NSScreen::screens(mtm).firstObject() else { return };
        let point = CGPoint { x: on_screen.x, y: main.frame().size.height - on_screen.y };
        unsafe { CGWarpMouseCursorPosition(point) };
    }

    // --------------------------------------------------------- keyboard

    /// Grabs the keyboard if the app asked and the window is the key one,
    /// focusing the surface.
    fn apply_grab(&self) {
        let ivars = self.ivars();
        if !ivars.grab_wanted.get() || ivars.grab.get().is_some() {
            return;
        }
        let Some(window) = self.window() else { return };
        if !active(&window) || !ivars.takes_input.get() || !window.makeFirstResponder(Some(self)) {
            return self.end_grab();
        }
        // Command-Tab and Command-H stay with the app; AppKit allows
        // turning off process switching only with the Dock hidden or
        // hiding itself.
        let app = NSApplication::sharedApplication(MainThreadMarker::from(self));
        let before = app.presentationOptions();
        let mut options = before
            | NSApplicationPresentationOptions::DisableProcessSwitching
            | NSApplicationPresentationOptions::DisableHideApplication;
        if !before.contains(NSApplicationPresentationOptions::HideDock) {
            options |= NSApplicationPresentationOptions::AutoHideDock;
        }
        app.setPresentationOptions(options);
        ivars.grab.set(Some(before));
        self.watch_window(&window);
    }

    /// Returns whether it was grabbed.
    fn ungrab(&self) -> bool {
        let Some(before) = self.ivars().grab.take() else { return false };
        NSApplication::sharedApplication(MainThreadMarker::from(self)).setPresentationOptions(before);
        self.stop_watching_window();
        true
    }

    fn end_grab(&self) {
        self.ungrab();
        self.ivars().grab_wanted.set(false);
        let ivars = self.ivars();
        if ivars.handle.borrow().is_some() {
            ivars.events.emit(ivars.id, UiEvent::KeyboardGrabEnded);
        }
    }

    /// While locked or grabbed: the window stopping being the key one
    /// (another app, another window) ends both.
    fn watch_window(&self, window: &NSWindow) {
        if self.ivars().resign_observer.borrow().is_some() {
            return;
        }
        let weak = Weak::from_retained(&self.retain());
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            let Some(view) = weak.load() else { return };
            if view.unlock() {
                view.end_lock();
            }
            if view.ungrab() {
                view.end_grab();
            }
        });
        let center = NSNotificationCenter::defaultCenter();
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidResignKeyNotification),
                Some(window),
                None,
                &block,
            )
        };
        *self.ivars().resign_observer.borrow_mut() = Some(observer);
    }

    fn stop_watching_window(&self) {
        let ivars = self.ivars();
        if ivars.locked.get() || ivars.grab.get().is_some() {
            return;
        }
        if let Some(observer) = ivars.resign_observer.borrow_mut().take() {
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref()) };
        }
    }
}

fn cursor_image(pixels: &Pixels, hotspot: Point) -> Option<Retained<NSCursor>> {
    let image = crate::backend::ns_image(&ImageSource::Pixels(pixels.clone()))?;
    let hotspot = NSPoint::new(hotspot.x as f64, hotspot.y as f64);
    Some(NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, hotspot))
}

/// Whether the window takes the keyboard: the key window of the active
/// app.
fn active(window: &NSWindow) -> bool {
    window.isKeyWindow() && NSApplication::sharedApplication(MainThreadMarker::from(window)).isActive()
}

fn modifiers(flags: NSEventModifierFlags) -> Modifiers {
    Modifiers {
        shift: flags.contains(NSEventModifierFlags::Shift),
        control: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        meta: flags.contains(NSEventModifierFlags::Command),
    }
}

/// Input for tests, through the view's own event methods: a click as
/// real mouse events (which focus it), keys as real key events to the
/// first responder. Scrolls are reported as they'd come: AppKit makes no
/// scroll events but from Quartz ones.
pub(crate) fn synthesize(view: &SurfaceView, input: &SyntheticInput) -> Result<(), ActionError> {
    if !view.takes_input() {
        return Err(ActionError::Unsupported);
    }
    let window = view.window().ok_or(ActionError::Unsupported)?;
    match input {
        SyntheticInput::Click(point) => {
            let location = view.convertPoint_toView(NSPoint::new(point.x as f64, point.y as f64), None);
            for (kind, up) in [(NSEventType::LeftMouseDown, false), (NSEventType::LeftMouseUp, true)] {
                let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                    kind,
                    location,
                    NSEventModifierFlags::empty(),
                    0.0,
                    window.windowNumber(),
                    None,
                    0,
                    1,
                    if up { 0.0 } else { 1.0 },
                )
                .ok_or(ActionError::Unsupported)?;
                if up { view.mouseUp(&event) } else { view.mouseDown(&event) }
            }
        }
        SyntheticInput::Key(key) => {
            let first: Option<Retained<NSResponder>> = window.firstResponder();
            if !first.is_some_and(|r| std::ptr::eq(&*r, view.as_super().as_super())) {
                return Err(ActionError::Unsupported);
            }
            let (code, character) = match key {
                Key::Char(c) => (KeyCode::from_us_char(*c), *c),
                Key::Enter => (KeyCode::Enter, '\r'),
                Key::Escape => (KeyCode::Escape, '\u{1b}'),
                Key::Tab => (KeyCode::Tab, '\t'),
                Key::Backspace => (KeyCode::Backspace, '\u{7f}'),
                Key::Up => (KeyCode::ArrowUp, '\u{f700}'),
                Key::Down => (KeyCode::ArrowDown, '\u{f701}'),
                Key::Home => (KeyCode::Home, '\u{f729}'),
                Key::End => (KeyCode::End, '\u{f72b}'),
            };
            let code = (0..0x7F).find(|c| KeyCode::from_mac(*c) == code).ok_or(ActionError::Unsupported)?;
            let characters = NSString::from_str(&character.to_string());
            for (kind, up) in [(NSEventType::KeyDown, false), (NSEventType::KeyUp, true)] {
                let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                    kind,
                    NSPoint::new(0.0, 0.0),
                    NSEventModifierFlags::empty(),
                    0.0,
                    window.windowNumber(),
                    None,
                    &characters,
                    &characters,
                    false,
                    code,
                )
                .ok_or(ActionError::Unsupported)?;
                if up { view.keyUp(&event) } else { view.keyDown(&event) }
            }
        }
        SyntheticInput::Scroll { dx, dy } => {
            let delta = ScrollDelta::Points { x: *dx, y: *dy };
            view.emit(SurfaceInput::Scroll { delta, modifiers: Modifiers::default() });
        }
    }
    Ok(())
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
