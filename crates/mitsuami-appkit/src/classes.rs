//! Objective-C classes bridging AppKit callbacks into mitsuami events.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use mitsuami_core::{
    DisplayList, EventSink, EventValue, NodeId, Point, PointerEvent, PointerKind, Size, UiEvent, WidgetKind,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControl, NSControlStateValueOn, NSControlTextEditingDelegate, NSEvent, NSPopUpButton,
    NSRectFill, NSSwitch, NSTextField, NSTextFieldDelegate, NSTextView, NSView, NSWindow, NSWindowDelegate,
};
use objc2_foundation::{
    NSKeyValueObservingOptions, NSNotification, NSObjectNSKeyValueObserverRegistration, NSPoint, NSRect, NSSize,
    NSString,
};

pub(crate) fn zero_rect() -> NSRect {
    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
}

pub(crate) struct HostIvars {
    /// Paint the window background (window content views only), so
    /// offscreen captures look like the real window.
    fill: Cell<bool>,
}

define_class!(
    /// A layout host: a flipped view (origin top-left, like the core) that
    /// never lays out its children itself.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = HostIvars]
    pub(crate) struct HostView;

    impl HostView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            if self.ivars().fill.get() {
                NSColor::windowBackgroundColor().setFill();
                NSRectFill(dirty);
            }
        }
    }
);

impl HostView {
    pub(crate) fn new(mtm: MainThreadMarker, fill: bool) -> Retained<HostView> {
        let this = HostView::alloc(mtm).set_ivars(HostIvars { fill: Cell::new(fill) });
        unsafe { msg_send![super(this), initWithFrame: zero_rect()] }
    }
}

pub(crate) struct TargetIvars {
    id: NodeId,
    kind: WidgetKind,
    events: EventSink,
}

define_class!(
    /// Target of control actions and delegate of text fields for one node.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetIvars]
    pub(crate) struct ActionTarget;

    impl ActionTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, sender: &AnyObject) {
            let TargetIvars { id, kind, events } = self.ivars();
            let event = match kind {
                WidgetKind::Button => UiEvent::Click,
                WidgetKind::Checkbox => match sender.downcast_ref::<NSButton>() {
                    Some(b) => UiEvent::Changed(EventValue::Bool(b.state() == NSControlStateValueOn)),
                    None => return,
                },
                WidgetKind::Switch => match sender.downcast_ref::<NSSwitch>() {
                    Some(s) => UiEvent::Changed(EventValue::Bool(s.state() == NSControlStateValueOn)),
                    None => return,
                },
                WidgetKind::Select => match sender.downcast_ref::<NSPopUpButton>().map(|p| p.indexOfSelectedItem()) {
                    Some(index) if index >= 0 => UiEvent::Changed(EventValue::Index(index as usize)),
                    _ => return,
                },
                _ => return,
            };
            events.emit(*id, event);
        }
    }

    impl ActionTarget {
        /// A scroll view's clip view moved (`NSViewBoundsDidChangeNotification`).
        #[unsafe(method(scrolled:))]
        fn scrolled(&self, notification: &NSNotification) {
            let Some(object) = notification.object() else { return };
            let Some(clip) = object.downcast_ref::<NSView>() else { return };
            let origin = clip.bounds().origin;
            let offset = Point::new(origin.x as f32, origin.y as f32);
            self.ivars().events.emit(self.ivars().id, UiEvent::Scrolled(offset));
        }
    }

    unsafe impl NSObjectProtocol for ActionTarget {}

    unsafe impl NSControlTextEditingDelegate for ActionTarget {
        /// Return / Enter submits. Deliberately not the field's action: that
        /// also fires when editing ends by Tab or a click elsewhere.
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn control_text_view_do_command_by_selector(
            &self,
            _control: &NSControl,
            _text_view: &NSTextView,
            command: Sel,
        ) -> bool {
            if command == sel!(insertNewline:) {
                self.ivars().events.emit(self.ivars().id, UiEvent::Submit);
            }
            // Not handled: AppKit carries on with its default behaviour.
            false
        }

        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, notification: &NSNotification) {
            let Some(object) = notification.object() else { return };
            if let Some(field) = object.downcast_ref::<NSTextField>() {
                let text = field.stringValue().to_string();
                self.ivars().events.emit(self.ivars().id, UiEvent::Changed(EventValue::Text(text)));
            }
        }
    }

    unsafe impl NSTextFieldDelegate for ActionTarget {}
);

impl ActionTarget {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        id: NodeId,
        kind: WidgetKind,
        events: EventSink,
    ) -> Retained<ActionTarget> {
        let this = ActionTarget::alloc(mtm).set_ivars(TargetIvars { id, kind, events });
        unsafe { msg_send![super(this), init] }
    }
}

/// Native view address → node, shared between the backend and delegates.
pub(crate) type ViewMap = Rc<RefCell<HashMap<usize, NodeId>>>;

pub(crate) struct WindowIvars {
    id: NodeId,
    events: EventSink,
    views: ViewMap,
    /// The node we last reported as focused.
    focused: Cell<Option<NodeId>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = WindowIvars]
    pub(crate) struct WindowDelegate;

    impl WindowDelegate {
        /// KVO on the window's `firstResponder`: every focus change,
        /// whether it came from a click, Tab or code.
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(
            &self,
            _key_path: Option<&NSString>,
            object: Option<&AnyObject>,
            _change: Option<&AnyObject>,
            _context: *mut c_void,
        ) {
            let Some(window) = object.and_then(|o| o.downcast_ref::<NSWindow>()) else { return };
            let now = self.focused_node(window);
            let WindowIvars { events, focused, .. } = self.ivars();
            let before = focused.replace(now);
            if before != now {
                if let Some(old) = before {
                    events.emit(old, UiEvent::FocusOut);
                }
                if let Some(new) = now {
                    events.emit(new, UiEvent::FocusIn);
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        /// The app decides whether a window closes (e.g. to ask about
        /// unsaved changes); the core destroys it if so.
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            self.ivars().events.emit(self.ivars().id, UiEvent::WindowCloseRequested);
            false
        }

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, notification: &NSNotification) {
            let Some(object) = notification.object() else { return };
            let Some(window) = object.downcast_ref::<NSWindow>() else { return };
            let content = window.contentRectForFrameRect(window.frame()).size;
            let size = Size::new(content.width as f32, content.height as f32);
            self.ivars().events.emit(self.ivars().id, UiEvent::WindowResized(size));
        }

        #[unsafe(method(windowDidChangeBackingProperties:))]
        fn window_did_change_backing_properties(&self, _notification: &NSNotification) {
            self.ivars().events.emit(self.ivars().id, UiEvent::MetricsChanged);
        }
    }
);

const FIRST_RESPONDER: &str = "firstResponder";

impl WindowDelegate {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        id: NodeId,
        events: EventSink,
        views: ViewMap,
    ) -> Retained<WindowDelegate> {
        let this = WindowDelegate::alloc(mtm).set_ivars(WindowIvars { id, events, views, focused: Cell::new(None) });
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn observe_focus(&self, window: &NSWindow) {
        unsafe {
            window.addObserver_forKeyPath_options_context(
                self,
                &NSString::from_str(FIRST_RESPONDER),
                NSKeyValueObservingOptions::New,
                std::ptr::null_mut(),
            );
        }
    }

    pub(crate) fn stop_observing_focus(&self, window: &NSWindow) {
        unsafe { window.removeObserver_forKeyPath(self, &NSString::from_str(FIRST_RESPONDER)) };
    }

    /// The node owning the first responder: the nearest known view among
    /// the responder and its superviews. While a text field is edited, the
    /// first responder is the window's field editor, which AppKit places
    /// inside the field (its delegate isn't set yet when focus moves).
    fn focused_node(&self, window: &NSWindow) -> Option<NodeId> {
        let responder = window.firstResponder()?;
        let views = self.ivars().views.borrow();
        let mut view: Option<Retained<NSView>> = responder.downcast::<NSView>().ok();
        while let Some(current) = view {
            if let Some(id) = views.get(&(&*current as *const NSView as usize)) {
                return Some(*id);
            }
            view = unsafe { current.superview() };
        }
        None
    }
}

pub(crate) struct ClosureIvars {
    handler: Box<dyn Fn(&AnyObject)>,
}

define_class!(
    /// A control target that runs a closure: how native renders and native
    /// views hear about their controls' actions.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ClosureIvars]
    pub(crate) struct ClosureTarget;

    impl ClosureTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, sender: &AnyObject) {
            (self.ivars().handler)(sender);
        }
    }

    unsafe impl NSObjectProtocol for ClosureTarget {}
);

impl ClosureTarget {
    pub(crate) fn new(mtm: MainThreadMarker, handler: impl Fn(&AnyObject) + 'static) -> Retained<ClosureTarget> {
        let this = ClosureTarget::alloc(mtm).set_ivars(ClosureIvars { handler: Box::new(handler) });
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct DrawnIvars {
    id: NodeId,
    events: EventSink,
    drawing: RefCell<DisplayList>,
}

define_class!(
    /// A drawn custom widget: rasterizes the display list the core sends,
    /// and reports pointer events for the core to interpret.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DrawnIvars]
    pub(crate) struct DrawnView;

    impl DrawnView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            crate::custom::rasterize(&self.ivars().drawing.borrow());
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.pointer(PointerKind::Down, event);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.pointer(PointerKind::Up, event);
        }
    }
);

impl DrawnView {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> Retained<DrawnView> {
        let this =
            DrawnView::alloc(mtm).set_ivars(DrawnIvars { id, events, drawing: RefCell::new(DisplayList::default()) });
        unsafe { msg_send![super(this), initWithFrame: zero_rect()] }
    }

    pub(crate) fn drawing(&self) -> DisplayList {
        self.ivars().drawing.borrow().clone()
    }

    pub(crate) fn set_drawing(&self, drawing: DisplayList) {
        *self.ivars().drawing.borrow_mut() = drawing;
        self.setNeedsDisplay(true);
    }

    fn pointer(&self, kind: PointerKind, event: &NSEvent) {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        let position = Point::new(p.x as f32, p.y as f32);
        self.ivars().events.emit(self.ivars().id, UiEvent::Pointer(PointerEvent { kind, position }));
    }
}
