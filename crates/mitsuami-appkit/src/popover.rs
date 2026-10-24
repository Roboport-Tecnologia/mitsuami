//! `Popover`: an `NSPopover` whose content is the node's host, shown by its
//! parent's view while the core says so. It's transient, as macOS's
//! popovers are: a click outside or Escape closes it, which is reported.
//! An `NSPopover` puts its content view edge to edge, and apps inset what's
//! in it: the host is `INSET` in from its content view's edges, AppKit's
//! standard spacing between content and its container's edge (what
//! `|-[view]-|` constraints and Interface Builder's popover use), as GTK's,
//! WinUI's and Qt's popovers inset their content themselves.
//!
//! AppKit shows a popover only from a window on screen, and tests never
//! show windows: there, it keeps what the core asked and says it shows
//! under its parent, where it would, and Escape in it closes it as the
//! popover's own window does (`dismiss`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{EventSink, NodeId, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{NSPopover, NSPopoverBehavior, NSPopoverDelegate, NSView, NSViewController};
use objc2_foundation::{NSNotification, NSRect, NSRectEdge, NSSize};

use crate::classes::HostView;

/// How far under its parent a popover's content is in tests: about where
/// its arrow and inset put it.
const GAP: f64 = 33.0;

/// Between the popover's edges and its content.
const INSET: f64 = 20.0;

pub(crate) struct State {
    id: NodeId,
    events: EventSink,
    /// What the core asked for.
    wanted: Cell<bool>,
    /// The popover is closing because the core asked: not reported. It
    /// animates out, so AppKit says it closed later.
    closing: Cell<bool>,
    anchor: RefCell<Option<Retained<NSView>>>,
    /// Windows are shown: off in tests (see above).
    on_screen: bool,
}

define_class!(
    /// Hears the popover close, and reports it unless the core closed it.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Rc<State>]
    pub(crate) struct PopoverDelegate;

    unsafe impl NSObjectProtocol for PopoverDelegate {}

    unsafe impl NSPopoverDelegate for PopoverDelegate {
        #[unsafe(method(popoverDidClose:))]
        fn did_close(&self, _notification: &NSNotification) {
            let state = self.ivars();
            if state.closing.replace(false) {
                return;
            }
            if state.wanted.get() {
                state.wanted.set(false);
                state.events.emit(state.id, UiEvent::PopoverShownChanged(false));
            }
        }
    }
);

pub(crate) struct Popover {
    pub(crate) host: Retained<HostView>,
    popover: Retained<NSPopover>,
    _delegate: Retained<PopoverDelegate>,
    state: Rc<State>,
}

impl Popover {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink, on_screen: bool) -> Popover {
        let host = HostView::new(mtm);
        let content = HostView::new(mtm);
        content.addSubview(&host);
        let controller = NSViewController::new(mtm);
        controller.setView(&content);
        let popover = NSPopover::new(mtm);
        popover.setContentViewController(Some(&controller));
        popover.setBehavior(NSPopoverBehavior::Transient);
        let state = Rc::new(State {
            id,
            events,
            wanted: Cell::new(false),
            closing: Cell::new(false),
            anchor: RefCell::new(None),
            on_screen,
        });
        let delegate = PopoverDelegate::alloc(mtm).set_ivars(state.clone());
        let delegate: Retained<PopoverDelegate> = unsafe { msg_send![super(delegate), init] };
        popover.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Popover { host, popover, _delegate: delegate, state }
    }

    /// The view it points at: its parent's. None: it's out of the tree.
    pub(crate) fn set_anchor(&self, anchor: Option<&NSView>) {
        *self.state.anchor.borrow_mut() = anchor.map(|a| a.retain());
        if anchor.is_none() {
            self.close();
        }
        self.sync();
    }

    pub(crate) fn set_shown(&self, shown: bool) {
        self.state.wanted.set(shown);
        self.sync();
    }

    pub(crate) fn wanted(&self) -> bool {
        self.state.wanted.get()
    }

    /// Its content's size, as the core laid it out.
    pub(crate) fn set_size(&self, size: NSSize) {
        // Its whole frame: the backend gives nodes their frames as they're
        // made, so its place in the content view is set with its size.
        self.host.setFrame(NSRect::new(objc2_foundation::NSPoint::new(INSET, INSET), size));
        self.popover.setContentSize(NSSize::new(size.width + 2.0 * INSET, size.height + 2.0 * INSET));
        self.sync();
    }

    /// Shows it once it's wanted, has a size and its parent is in a
    /// window; closes it when it's no longer wanted.
    fn sync(&self) {
        let shown = self.popover.isShown();
        if !self.state.wanted.get() {
            if shown {
                self.close();
            }
            return;
        }
        let size = self.host.frame().size;
        if shown || size.width <= 0.0 || size.height <= 0.0 {
            return;
        }
        let Some(anchor) = self.state.anchor.borrow().clone() else { return };
        if !self.state.on_screen || anchor.window().is_none() {
            return;
        }
        // Under its parent where it fits, as menus and pickers open.
        let edge = if anchor.isFlipped() { NSRectEdge::MaxY } else { NSRectEdge::MinY };
        self.popover.showRelativeToRect_ofView_preferredEdge(anchor.bounds(), &anchor, edge);
    }

    fn close(&self) {
        if self.popover.isShown() {
            self.state.closing.set(true);
            self.popover.close();
        }
    }

    /// Whether it shows: as AppKit shows it, or in tests as the core
    /// asked while it has a parent.
    pub(crate) fn shown(&self) -> bool {
        match self.state.on_screen {
            true => self.popover.isShown(),
            false => self.state.wanted.get() && self.state.anchor.borrow().is_some(),
        }
    }

    /// The user's Escape in it, in tests: it closes, and says so, as
    /// AppKit's transient popover does.
    pub(crate) fn dismiss(&self) -> bool {
        if self.state.on_screen || !self.shown() {
            return false;
        }
        self.state.wanted.set(false);
        self.state.events.emit(self.state.id, UiEvent::PopoverShownChanged(false));
        true
    }

    /// Where it shows, in its parent's coordinates; None while it's
    /// hidden. In tests, under its parent, as it would.
    pub(crate) fn frame(&self) -> Option<NSRect> {
        if !self.shown() {
            return None;
        }
        if !self.state.on_screen {
            let anchor = self.state.anchor.borrow().clone()?;
            let below = anchor.bounds().size.height + GAP;
            return Some(NSRect::new(objc2_foundation::NSPoint::new(0.0, below), self.host.frame().size));
        }
        let anchor = self.state.anchor.borrow().clone()?;
        let (window, anchor_window) = (self.host.window()?, anchor.window()?);
        let on_screen = window.convertRectToScreen(self.host.convertRect_toView(self.host.bounds(), None));
        Some(anchor.convertRect_fromView(anchor_window.convertRectFromScreen(on_screen), None))
    }
}
