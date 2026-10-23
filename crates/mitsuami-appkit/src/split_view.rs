//! A split: an `NSSplitView` with the thin divider macOS's own apps use
//! (Finder's, Mail's, Xcode's), its panes the hosts the core lays out.
//!
//! The core places the panes; the split view draws the divider between
//! them and moves them as the user drags it. It stops the divider at the
//! panes' least sizes, and leaves the first pane as it is when the split
//! grows, as the core does.
//!
//! The split view resizes its panes whenever it adjusts itself (a pane
//! added, its orientation set), and tells its delegate each time; only
//! what happens during the user's drag (its `mouseDown:` tracking) or
//! assistive technology's move is reported.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami_core::{EventSink, EventValue, NodeId, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSResponder, NSSplitView, NSSplitViewDelegate, NSSplitViewDividerStyle, NSView};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{NSInteger, NSNotification, NSPoint, NSRect, NSSize};

use crate::classes::zero_rect;

/// What the split view and its delegate share.
#[derive(Default)]
pub(crate) struct State {
    /// Set while the user drags the divider, or assistive technology
    /// moves it: what moves then is reported.
    moving: Cell<bool>,
    /// The panes' least lengths along the split.
    minimums: Cell<[f32; 2]>,
    /// The first pane's length last reported or set.
    position: Cell<Option<f64>>,
}

impl State {
    pub(crate) fn position(&self) -> Option<f64> {
        self.position.get()
    }

    pub(crate) fn minimums(&self) -> [f32; 2] {
        self.minimums.get()
    }

    /// Runs `f` as a move of the user's.
    fn moving(&self, f: impl FnOnce()) {
        let was = self.moving.replace(true);
        f();
        self.moving.set(was);
    }
}

define_class!(
    /// An `NSSplitView` that knows when the user is dragging its divider.
    #[unsafe(super(NSSplitView, NSView, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Rc<State>]
    pub(crate) struct DraggedSplitView;

    impl DraggedSplitView {
        /// AppKit lays the panes out on every resize, and in a split with
        /// no room for its divider (as it is before the core sizes it) it
        /// gives the second pane a negative length, then logs that its own
        /// layout is inconsistent. The core places the panes: until there's
        /// room, they're flat at the split's start.
        #[unsafe(method(resizeSubviewsWithOldSize:))]
        fn resize_subviews(&self, old: NSSize) {
            let bounds = self.bounds().size;
            if along(self, bounds.width, bounds.height) >= self.dividerThickness() {
                unsafe {
                    let _: () = msg_send![super(self), resizeSubviewsWithOldSize: old];
                }
                return;
            }
            let flat = match self.isVertical() {
                true => NSSize::new(0.0, bounds.height),
                false => NSSize::new(bounds.width, 0.0),
            };
            for pane in self.subviews() {
                pane.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), flat));
            }
        }

        /// AppKit tracks the drag inside this call.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.ivars().clone().moving(|| unsafe {
                let _: () = msg_send![super(self), mouseDown: event];
            });
        }
    }
);

pub(crate) struct DelegateIvars {
    id: NodeId,
    events: EventSink,
    state: Rc<State>,
}

define_class!(
    /// Reports the user's moves, and keeps the divider within the panes'
    /// least sizes.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    pub(crate) struct SplitDelegate;

    unsafe impl NSObjectProtocol for SplitDelegate {}

    unsafe impl NSSplitViewDelegate for SplitDelegate {
        #[unsafe(method(splitView:constrainMinCoordinate:ofSubviewAt:))]
        fn constrain_min(&self, _split: &NSSplitView, proposed: CGFloat, _index: NSInteger) -> CGFloat {
            proposed.max(f64::from(self.ivars().state.minimums.get()[0]))
        }

        #[unsafe(method(splitView:constrainMaxCoordinate:ofSubviewAt:))]
        fn constrain_max(&self, split: &NSSplitView, proposed: CGFloat, _index: NSInteger) -> CGFloat {
            proposed.min(most(split, self.ivars().state.minimums.get()[1]))
        }

        /// The first pane keeps its length as the split grows or shrinks;
        /// the second takes the difference.
        #[unsafe(method(splitView:shouldAdjustSizeOfSubview:))]
        fn should_adjust(&self, split: &NSSplitView, subview: &NSView) -> bool {
            split.subviews().firstObject().is_none_or(|first| *first != *subview)
        }

        #[unsafe(method(splitViewDidResizeSubviews:))]
        fn did_resize(&self, notification: &NSNotification) {
            let DelegateIvars { id, events, state } = self.ivars();
            if !state.moving.get() {
                return;
            }
            let Some(split) = notification.object().and_then(|o| o.downcast::<NSSplitView>().ok()) else { return };
            let Some(first) = split.subviews().firstObject() else { return };
            let length = along(&split, first.frame().size.width, first.frame().size.height);
            if state.position.get() != Some(length) {
                state.position.set(Some(length));
                events.emit(*id, UiEvent::Changed(EventValue::Number(length)));
            }
        }
    }
);

/// A length along the split: a width side by side, a height stacked.
fn along(split: &NSSplitView, width: CGFloat, height: CGFloat) -> CGFloat {
    if split.isVertical() { width } else { height }
}

/// The furthest the divider goes: the second pane at its least length.
fn most(split: &NSSplitView, second: f32) -> CGFloat {
    let bounds = split.bounds().size;
    along(split, bounds.width, bounds.height) - split.dividerThickness() - f64::from(second)
}

pub(crate) struct SplitView {
    pub(crate) view: Retained<DraggedSplitView>,
    _delegate: Retained<SplitDelegate>,
    pub(crate) state: Rc<State>,
}

impl SplitView {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> SplitView {
        let state = Rc::new(State::default());
        let view = DraggedSplitView::alloc(mtm).set_ivars(state.clone());
        let view: Retained<DraggedSplitView> = unsafe { msg_send![super(view), initWithFrame: zero_rect()] };
        view.setDividerStyle(NSSplitViewDividerStyle::Thin);
        // Side by side until the app says otherwise.
        view.setVertical(true);
        let delegate = SplitDelegate::alloc(mtm).set_ivars(DelegateIvars { id, events, state: state.clone() });
        let delegate: Retained<SplitDelegate> = unsafe { msg_send![super(delegate), init] };
        view.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        SplitView { view, _delegate: delegate, state }
    }

    pub(crate) fn set_minimums(&self, minimums: [f32; 2]) {
        self.state.minimums.set(minimums);
    }

    /// The app's position: the core lays the panes out at it.
    pub(crate) fn set_position(&self, position: f64) {
        self.state.position.set(Some(position));
    }

    /// The core placed the split or a pane. The split view works out
    /// where to draw its divider in its layout pass, and keeps that until
    /// the next one: a window resize marks it for one, panes set from
    /// outside don't, so the divider stayed where the last pass put it.
    pub(crate) fn panes_moved(&self) {
        self.view.setNeedsLayout(true);
        self.view.setNeedsDisplay(true);
    }

    /// Moves the divider as the user would (assistive technology sets a
    /// splitter's value), within the panes' least sizes: it's reported.
    pub(crate) fn move_divider(&self, position: f64) {
        let [first, second] = self.state.minimums.get();
        let position = position.min(most(&self.view, second)).max(f64::from(first));
        self.state.moving(|| self.view.setPosition_ofDividerAtIndex(position, 0));
    }

    /// The thin divider's thickness, for the core's layout.
    pub(crate) fn divider(mtm: MainThreadMarker) -> f32 {
        let probe = NSSplitView::initWithFrame(NSSplitView::alloc(mtm), zero_rect());
        probe.setDividerStyle(NSSplitViewDividerStyle::Thin);
        probe.dividerThickness() as f32
    }
}
