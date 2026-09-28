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
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControl, NSControlStateValueOn, NSControlTextEditingDelegate, NSEvent, NSPopUpButton,
    NSRectFill, NSScreen, NSSlider, NSSwitch, NSTextField, NSTextFieldDelegate, NSTextView, NSView,
    NSViewFrameDidChangeNotification, NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    NSKeyValueObservingOptions, NSNotification, NSNotificationCenter, NSObjectNSKeyValueObserverRegistration, NSPoint,
    NSRect, NSSize, NSString,
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
                    Some(b) => {
                        // A click leaves the mixed state; later clicks
                        // mustn't cycle back into it.
                        b.setAllowsMixedState(false);
                        UiEvent::Changed(EventValue::Bool(b.state() == NSControlStateValueOn))
                    }
                    None => return,
                },
                WidgetKind::Switch => match sender.downcast_ref::<NSSwitch>() {
                    Some(s) => UiEvent::Changed(EventValue::Bool(s.state() == NSControlStateValueOn)),
                    None => return,
                },
                WidgetKind::Slider => match sender.downcast_ref::<NSSlider>() {
                    Some(s) => UiEvent::Changed(EventValue::Number(s.doubleValue())),
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
    /// A dialog: Escape asks it to close.
    modal: Cell<bool>,
    /// Full screen as AppKit has it, or is moving into.
    full_screen: Cell<bool>,
    /// Full screen as the app wants it (and the user, who changes it too).
    full_screen_wanted: Cell<bool>,
    /// A transition is under way: AppKit ignores a toggle until it ends.
    full_screen_moving: Cell<bool>,
    /// The transition starting is the app's own, so it isn't reported.
    full_screen_ours: Cell<bool>,
    /// The app's minimum content size, if it set one.
    min_size: Cell<Option<Size>>,
    /// The content sets the height, not the user.
    height_locked: Cell<bool>,
    /// The window's content (its host), when it's beside a sidebar: the
    /// window's content area is larger by the sidebar and the title bar.
    detail: RefCell<Option<Retained<NSView>>>,
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

    impl WindowDelegate {
        /// The content beside a sidebar changed size: the window's did, or
        /// the user moved the divider (`NSViewFrameDidChangeNotification`).
        #[unsafe(method(detailDidResize:))]
        fn detail_did_resize(&self, notification: &NSNotification) {
            let Some(view) = notification.object().and_then(|o| o.downcast::<NSView>().ok()) else { return };
            let size = view.frame().size;
            let size = Size::new(size.width as f32, size.height as f32);
            self.ivars().events.emit(self.ivars().id, UiEvent::WindowResized(size));
        }
    }

    impl WindowDelegate {
        /// Escape, from whatever has the focus and didn't use it: the
        /// window hands action messages nobody took to its delegate.
        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            if self.ivars().modal.get() {
                self.ivars().events.emit(self.ivars().id, UiEvent::WindowCloseRequested);
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
            // A locked height is locked at the new one.
            if self.ivars().height_locked.get() && !window.styleMask().contains(NSWindowStyleMask::FullScreen) {
                self.apply_min_size(window);
            }
            // Beside a sidebar, the content's own size is reported, once
            // the split lays it out (`detailDidResize:`).
            if self.ivars().detail.borrow().is_none() {
                self.ivars().events.emit(self.ivars().id, UiEvent::WindowResized(size));
            }
        }

        /// Another screen, another cap on the minimum.
        #[unsafe(method(windowDidChangeScreen:))]
        fn window_did_change_screen(&self, notification: &NSNotification) {
            if let Some(window) = notification.object().and_then(|o| o.downcast::<NSWindow>().ok()) {
                self.apply_min_size(&window);
            }
        }

        #[unsafe(method(windowDidChangeBackingProperties:))]
        fn window_did_change_backing_properties(&self, _notification: &NSNotification) {
            self.ivars().events.emit(self.ivars().id, UiEvent::MetricsChanged);
        }

        // Full screen's transitions. `willEnter` and `willExit` come
        // inside `toggleFullScreen:`, from the app or from the title
        // bar's button; `did` ends the animation.

        #[unsafe(method(windowWillEnterFullScreen:))]
        fn window_will_enter_full_screen(&self, _notification: &NSNotification) {
            self.full_screen_starts(true);
        }

        #[unsafe(method(windowWillExitFullScreen:))]
        fn window_will_exit_full_screen(&self, _notification: &NSNotification) {
            self.full_screen_starts(false);
        }

        #[unsafe(method(windowDidEnterFullScreen:))]
        fn window_did_enter_full_screen(&self, notification: &NSNotification) {
            self.full_screen_ends(notification);
        }

        #[unsafe(method(windowDidExitFullScreen:))]
        fn window_did_exit_full_screen(&self, notification: &NSNotification) {
            self.full_screen_ends(notification);
        }

        #[unsafe(method(windowDidFailToEnterFullScreen:))]
        fn window_did_fail_to_enter_full_screen(&self, _window: &NSWindow) {
            self.full_screen_fails(false);
        }

        #[unsafe(method(windowDidFailToExitFullScreen:))]
        fn window_did_fail_to_exit_full_screen(&self, _window: &NSWindow) {
            self.full_screen_fails(true);
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
        let this = WindowDelegate::alloc(mtm).set_ivars(WindowIvars {
            id,
            events,
            views,
            focused: Cell::new(None),
            modal: Cell::new(false),
            full_screen: Cell::new(false),
            full_screen_wanted: Cell::new(false),
            full_screen_moving: Cell::new(false),
            full_screen_ours: Cell::new(false),
            min_size: Cell::new(None),
            height_locked: Cell::new(false),
            detail: RefCell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn set_modal(&self, modal: bool) {
        self.ivars().modal.set(modal);
    }

    /// The app's minimum content size. AppKit keeps the user from resizing
    /// below it, but a window already smaller stays so: it grows here, as
    /// on the other platforms.
    pub(crate) fn set_min_size(&self, window: &NSWindow, min: Size) {
        self.ivars().min_size.set(Some(min));
        self.apply_min_size(window);
    }

    /// The content sets the height: a minimum and a maximum at the height
    /// it has, so the user resizes only the width, and AppKit's resize
    /// cursors say so.
    pub(crate) fn set_height_locked(&self, window: &NSWindow, locked: bool) {
        self.ivars().height_locked.set(locked);
        self.apply_min_size(window);
    }

    pub(crate) fn height_locked(&self, window: &NSWindow) -> bool {
        window.contentMaxSize().height < f32::MAX as f64
    }

    /// Puts the window's content beside a sidebar (`Some`), or back as
    /// the window's whole content area. Its size is reported from then on.
    pub(crate) fn set_detail(&self, detail: Option<&NSView>) {
        let center = NSNotificationCenter::defaultCenter();
        if let Some(old) = self.ivars().detail.replace(detail.map(|d| d.retain())) {
            unsafe { center.removeObserver_name_object(self, Some(NSViewFrameDidChangeNotification), Some(&old)) };
        }
        if let Some(detail) = detail {
            detail.setPostsFrameChangedNotifications(true);
            unsafe {
                center.addObserver_selector_name_object(
                    self,
                    sel!(detailDidResize:),
                    Some(NSViewFrameDidChangeNotification),
                    Some(detail),
                )
            };
        }
    }

    /// How much larger the window's content area is than its content: by
    /// the sidebar and the title bar, beside a sidebar; else not at all.
    pub(crate) fn extra(&self, window: &NSWindow) -> NSSize {
        let Some(detail) = self.ivars().detail.borrow().clone() else { return NSSize::ZERO };
        window.layoutIfNeeded();
        let area = window.contentRectForFrameRect(window.frame()).size;
        let content = detail.frame().size;
        NSSize::new((area.width - content.width).max(0.0), (area.height - content.height).max(0.0))
    }

    /// The minimum, no larger than the content of a window filling its
    /// screen's visible area (a machine's mode can be larger than a laptop's
    /// screen; AppKit would make a window as large as asked), and a locked
    /// height.
    fn apply_min_size(&self, window: &NSWindow) {
        let extra = self.extra(window);
        let min = self.min_size_on_screen(window).map(|m| NSSize::new(m.width + extra.width, m.height + extra.height));
        let content = window.contentRectForFrameRect(window.frame()).size;
        let (mut least, mut most) = (min.unwrap_or(NSSize::ZERO), NSSize::new(f32::MAX as f64, f32::MAX as f64));
        if self.ivars().height_locked.get() {
            least.height = content.height;
            most.height = content.height;
        }
        window.setContentMinSize(least);
        window.setContentMaxSize(most);
        let Some(min) = min else { return };
        let grown = NSSize::new(content.width.max(min.width), content.height.max(min.height));
        if grown != content && !window.styleMask().contains(NSWindowStyleMask::FullScreen) {
            window.setContentSize(grown);
        }
    }

    /// The content area for a content size the app asks for, no smaller
    /// than its minimum: AppKit's `setContentSize:` would go below it.
    pub(crate) fn at_least_min(&self, window: &NSWindow, size: Size) -> NSSize {
        let min = self.min_size_on_screen(window).unwrap_or(NSSize::ZERO);
        let extra = self.extra(window);
        NSSize::new(
            (size.width as f64).max(min.width) + extra.width,
            (size.height as f64).max(min.height) + extra.height,
        )
    }

    /// The app's minimum content size, no larger than a window filling its
    /// screen could give its content.
    fn min_size_on_screen(&self, window: &NSWindow) -> Option<NSSize> {
        let min = self.ivars().min_size.get()?;
        let screen = window.screen().or_else(|| NSScreen::mainScreen(MainThreadMarker::from(self)));
        let extra = self.extra(window);
        let most = screen.map_or(NSSize::new(f64::MAX, f64::MAX), |s| {
            let area = window.contentRectForFrameRect(s.visibleFrame()).size;
            NSSize::new(area.width - extra.width, area.height - extra.height)
        });
        Some(NSSize::new((min.width as f64).min(most.width), (min.height as f64).min(most.height)))
    }

    /// The minimum as AppKit has it: the app's, if AppKit holds it as
    /// capped by the screen. A locked height hides the minimum's.
    pub(crate) fn min_size(&self, window: &NSWindow) -> Size {
        let extra = self.extra(window);
        let now = window.contentMinSize();
        let now = NSSize::new((now.width - extra.width).max(0.0), (now.height - extra.height).max(0.0));
        let locked = self.height_locked(window);
        match (self.ivars().min_size.get(), self.min_size_on_screen(window)) {
            (Some(min), Some(capped)) if capped.width == now.width && (locked || capped.height == now.height) => min,
            _ => Size::new(now.width as f32, now.height as f32),
        }
    }

    /// Full screen as the app wants it: applied once the window is shown
    /// (a hidden window would go into full screen unseen) and no
    /// transition is under way.
    pub(crate) fn set_full_screen(&self, window: &NSWindow, on: bool) {
        self.ivars().full_screen_wanted.set(on);
        self.apply_full_screen(window);
    }

    pub(crate) fn apply_full_screen(&self, window: &NSWindow) {
        let ivars = self.ivars();
        if ivars.full_screen_moving.get()
            || !window.isVisible()
            || ivars.full_screen.get() == ivars.full_screen_wanted.get()
        {
            return;
        }
        ivars.full_screen_ours.set(true);
        window.toggleFullScreen(None);
        // No transition started: AppKit refused (a sheet can't have full
        // screen). The window stays as it is, and the app hears so.
        if ivars.full_screen_ours.replace(false) {
            ivars.full_screen_wanted.set(ivars.full_screen.get());
            ivars.events.emit(ivars.id, UiEvent::FullScreenChanged(ivars.full_screen.get()));
        }
    }

    /// What the window shows: while it's hidden or moving, what it will.
    pub(crate) fn full_screen(&self, window: &NSWindow) -> bool {
        let ivars = self.ivars();
        if ivars.full_screen_moving.get() || !window.isVisible() {
            ivars.full_screen_wanted.get()
        } else {
            ivars.full_screen.get()
        }
    }

    fn full_screen_starts(&self, on: bool) {
        let ivars = self.ivars();
        ivars.full_screen.set(on);
        ivars.full_screen_moving.set(true);
        if !ivars.full_screen_ours.replace(false) {
            ivars.full_screen_wanted.set(on);
            ivars.events.emit(ivars.id, UiEvent::FullScreenChanged(on));
        }
    }

    /// The app may have changed its mind while it moved.
    fn full_screen_ends(&self, notification: &NSNotification) {
        self.ivars().full_screen_moving.set(false);
        if let Some(window) = notification.object().and_then(|o| o.downcast::<NSWindow>().ok()) {
            self.apply_full_screen(&window);
        }
    }

    /// The window is back as it was: the app hears so.
    fn full_screen_fails(&self, was: bool) {
        let ivars = self.ivars();
        ivars.full_screen.set(was);
        ivars.full_screen_moving.set(false);
        if ivars.full_screen_wanted.replace(was) != was {
            ivars.events.emit(ivars.id, UiEvent::FullScreenChanged(was));
        }
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
        self.set_detail(None);
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
