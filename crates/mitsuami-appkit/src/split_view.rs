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

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{EventSink, EventValue, NodeId, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSEvent, NSResponder, NSSplitView, NSSplitViewDelegate, NSSplitViewDividerStyle, NSView, NSWindowOrderingMode,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{NSEdgeInsets, NSInteger, NSNotification, NSPoint, NSRect, NSSize};

use crate::classes::{HostView, zero_rect};

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
    /// How far it runs up under its window's bars (see
    /// `SplitView::set_under_bar`).
    under_bar: Cell<f64>,
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
        /// The part under the window's bars is outside where the core
        /// placed it.
        #[unsafe(method(alignmentRectInsets))]
        fn alignment_rect_insets(&self) -> NSEdgeInsets {
            NSEdgeInsets { top: self.ivars().under_bar.get(), left: 0.0, bottom: 0.0, right: 0.0 }
        }

        /// AppKit lays the panes out on every resize of the split; the core
        /// places them, so they stay where it put them. AppKit's own layout
        /// set a pane's frame to its whole share, which is wrong for a
        /// control placed by its alignment rect (a tab view as a pane got
        /// its bezel's insets wrong, and kept them, since the core's frame
        /// for it hadn't changed: SourceGit's details' tab strip came back
        /// laid out too narrow, 2026-10-23); and in a split with no room
        /// for its divider it gave the second pane a negative length, then
        /// logged that its layout was inconsistent. Moving the divider
        /// moves the panes, as AppKit does.
        #[unsafe(method(resizeSubviewsWithOldSize:))]
        fn resize_subviews(&self, _old: NSSize) {}

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
    /// Its panes, each in a host of its own: the split view sizes the
    /// hosts, as its arranged views, and a pane is placed in its host by
    /// its alignment rect, as everything the core places is. A control as
    /// a pane (a tab view) has a frame past its alignment rect, which the
    /// split view, laying out its arranged views, got wrong.
    panes: RefCell<Vec<(NodeId, Retained<HostView>)>>,
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
        SplitView { view, _delegate: delegate, state, panes: RefCell::default() }
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

    /// Turns it: panes side by side, or stacked. Under the bars, both
    /// panes reach up under them side by side, only the first stacked, so
    /// their hosts change with it. AppKit checks, as it turns, that the
    /// panes tile it along the new axis, and logs that they're "in an
    /// inconsistent state" if not: they're tiled halfway first, and the
    /// core places them where they go on its next pass.
    pub(crate) fn turn(&self, side_by_side: bool) {
        if self.view.isVertical() == side_by_side {
            return;
        }
        let bounds = self.view.bounds();
        let divider = self.view.dividerThickness();
        let under_bar = self.state.under_bar.get();
        let panes = self.panes.borrow();
        let along = if side_by_side { bounds.size.width } else { bounds.size.height };
        let half = ((along - divider) / 2.0).max(0.0).floor();
        for (index, (_, host)) in panes.iter().enumerate() {
            host.set_under_bar(if side_by_side || index == 0 { under_bar } else { 0.0 });
            let (start, length) = match index {
                0 => (0.0, half),
                _ => (half + divider, (along - divider - half).max(0.0)),
            };
            host.setFrame(match side_by_side {
                true => NSRect::new(
                    NSPoint::new(bounds.origin.x + start, bounds.origin.y),
                    NSSize::new(length, bounds.size.height),
                ),
                false => NSRect::new(
                    NSPoint::new(bounds.origin.x, bounds.origin.y + start),
                    NSSize::new(bounds.size.width, length),
                ),
            });
            host.keep_under_bar();
        }
        drop(panes);
        self.view.setVertical(side_by_side);
    }

    /// Puts a pane's view in a host of its own, at `index` among the panes.
    pub(crate) fn insert(&self, mtm: MainThreadMarker, id: NodeId, pane: &NSView, index: usize) {
        let host = HostView::new(mtm);
        host.addSubview(pane);
        let mut panes = self.panes.borrow_mut();
        match panes.get(index) {
            Some((_, after)) => {
                self.view.addSubview_positioned_relativeTo(&host, NSWindowOrderingMode::Below, Some(after))
            }
            None => self.view.addSubview(&host),
        }
        let index = index.min(panes.len());
        panes.insert(index, (id, host));
    }

    /// Takes a pane out, with its host.
    pub(crate) fn remove(&self, id: NodeId, pane: &NSView) {
        let mut panes = self.panes.borrow_mut();
        if let Some(index) = panes.iter().position(|(p, _)| *p == id) {
            let (_, host) = panes.remove(index);
            pane.removeFromSuperview();
            host.removeFromSuperview();
        }
    }

    /// Places a pane where the core put it in the split: its host there
    /// (lower by how far the split runs up under the bars), and the pane in
    /// its host by its alignment rect.
    pub(crate) fn place(&self, id: NodeId, pane: &NSView, rect: NSRect) {
        self.place_host(id, rect);
        pane.setFrame(pane.frameForAlignmentRect(NSRect::new(NSPoint::new(0.0, 0.0), rect.size)));
    }

    /// Places only a pane's host, for a pane that keeps its own frame (a
    /// tab view with no room).
    pub(crate) fn place_host(&self, id: NodeId, rect: NSRect) {
        let Some(host) = self.panes.borrow().iter().find(|(p, _)| *p == id).map(|(_, h)| h.clone()) else {
            return;
        };
        let mut placed = rect;
        placed.origin.y += self.state.under_bar.get();
        host.setFrame(host.frameForAlignmentRect(placed));
        host.keep_under_bar();
    }

    /// Where a pane is in the split, as the core placed it.
    pub(crate) fn placed(&self, id: NodeId, pane: &NSView) -> Option<NSRect> {
        let panes = self.panes.borrow();
        let (_, host) = panes.iter().find(|(p, _)| *p == id)?;
        let mut at = host.alignmentRectForFrame(host.frame());
        at.origin.y -= self.state.under_bar.get();
        let inside = pane.alignmentRectForFrame(pane.frame());
        Some(NSRect::new(NSPoint::new(at.origin.x + inside.origin.x, at.origin.y + inside.origin.y), inside.size))
    }

    /// Runs this far up under its window's bars (zero: not under them),
    /// with its panes, keeping where the core placed them, for a scroll
    /// view that runs up under the bars from a pane: macOS shows what
    /// scrolls under its bars only from a scroll view whose split view and
    /// pane reach them too (one reaching up out of its pane showed
    /// nothing; plain views around it don't matter). What's in the panes
    /// stays where the core put it, as their bounds start above their
    /// tops. Whether it changed.
    pub(crate) fn set_under_bar(&self, height: f64) -> bool {
        let old = self.state.under_bar.get();
        if old == height {
            return false;
        }
        let view = &self.view;
        let placed = view.alignmentRectForFrame(view.frame());
        let panes: Vec<_> = view
            .subviews()
            .iter()
            .map(|pane| {
                let mut placed = pane.alignmentRectForFrame(pane.frame());
                placed.origin.y -= old;
                (pane.clone(), placed)
            })
            .collect();
        self.state.under_bar.set(height);
        // Side by side, both panes reach the top; stacked, the first. The
        // panes move before the split: AppKit checks, as the split's frame
        // changes, that they line up with its edges.
        let vertical = view.isVertical();
        for (index, (pane, mut placed)) in panes.into_iter().enumerate() {
            placed.origin.y += height;
            if let Some(host) = pane.downcast_ref::<HostView>() {
                host.set_under_bar(if vertical || index == 0 { height } else { 0.0 });
            }
            pane.setFrame(pane.frameForAlignmentRect(placed));
            if let Some(host) = pane.downcast_ref::<HostView>() {
                host.keep_under_bar();
            }
        }
        view.setFrame(view.frameForAlignmentRect(placed));
        true
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
