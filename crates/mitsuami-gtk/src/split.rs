//! A split: a `gtk::Paned`, as GNOME apps split a window, with Adwaita's
//! thin handle. Each pane host is in a host of its own, the paned's start
//! or end child, at the top left and at the size the core gave it: the
//! paned sizes its children, and the core already laid the pane out.
//!
//! The wrappers ask for nothing, so the paned lets the divider go anywhere;
//! the panes' least sizes are kept here, by setting the position back
//! (GNOME apps get the same from `shrink-start` and `shrink-end` off and
//! their children's minimums, but a host asks for exactly its frame, which
//! would pin the divider where it is). The first pane keeps its length as
//! the paned grows (`resize-start` off), as the core lays it out.
//!
//! `notify::position` fires for the backend's sets and for allocations
//! too, so only what moves while the primary button is down on the paned
//! (its handle's drag), the handle's keys, or assistive technology's move
//! is reported.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::graphene;
use gtk::prelude::*;
use mitsuami_core::{EventValue, NodeId, Point, Rect, Size, UiEvent};

use crate::host::{Events, Frames, Host};

/// What the paned's handlers share.
#[derive(Default)]
struct State {
    /// The user is dragging the handle, moving it with its keys, or
    /// assistive technology is moving it.
    moving: Cell<bool>,
    /// The panes' least lengths along the split.
    minimums: Cell<[f32; 2]>,
    /// The first pane's length last reported or set.
    position: Cell<Option<f64>>,
}

pub(crate) struct Split {
    pub(crate) paned: gtk::Paned,
    wrappers: [Host; 2],
    state: Rc<State>,
    frames: Frames,
    /// Where the core put each pane, until the paned has placed it.
    origins: RefCell<HashMap<gtk::Widget, Point>>,
}

impl Split {
    pub(crate) fn new(id: NodeId, events: Events, frames: Frames) -> Split {
        let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        paned.set_wide_handle(false);
        paned.set_resize_start_child(false);
        paned.set_resize_end_child(true);
        paned.set_shrink_start_child(true);
        paned.set_shrink_end_child(true);
        let wrappers = [Host::new(frames.clone(), None), Host::new(frames.clone(), None)];
        paned.set_start_child(Some(&wrappers[0]));
        paned.set_end_child(Some(&wrappers[1]));
        let state = Rc::new(State::default());

        // The primary button down on the paned: its handle is being dragged
        // (presses in the panes move nothing). Raw events, since the
        // handle's drag claims the sequence from any other gesture.
        let legacy = gtk::EventControllerLegacy::new();
        legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
        let moving = state.clone();
        legacy.connect_event(move |_, event| {
            match event.event_type() {
                gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::TouchBegin => moving.moving.set(true),
                gtk::gdk::EventType::ButtonRelease
                | gtk::gdk::EventType::TouchEnd
                | gtk::gdk::EventType::TouchCancel => moving.moving.set(false),
                _ => {}
            }
            gtk::glib::Propagation::Proceed
        });
        paned.add_controller(legacy);
        // The handle's keys (F8 focuses it, then the arrows move it): the
        // move is the default handler's, after this one, and done at once.
        let moving = state.clone();
        paned.connect_move_handle(move |_, _| {
            moving.moving.set(true);
            let moving = moving.clone();
            gtk::glib::idle_add_local_once(move || moving.moving.set(false));
            false
        });

        let reported = state.clone();
        paned.connect_position_notify(move |paned| {
            let state = &reported;
            let [first, second] = state.minimums.get();
            let position = paned.position();
            let kept = keep(paned, position, first, second);
            if kept != position {
                // Notified again, at the kept position.
                paned.set_position(kept);
                return;
            }
            if !state.moving.get() || state.position.get() == Some(f64::from(position)) {
                return;
            }
            state.position.set(Some(f64::from(position)));
            events.emit(id, UiEvent::Changed(EventValue::Number(f64::from(position))));
        });
        Split { paned, wrappers, state, frames, origins: RefCell::default() }
    }

    /// Puts a pane host in its wrapper: 0 first, 1 second.
    pub(crate) fn insert(&self, index: usize, host: &gtk::Widget) {
        host.set_parent(&self.wrappers[index]);
        self.place(host);
    }

    pub(crate) fn remove(&self, host: &gtk::Widget) {
        if !self.wrappers.iter().any(|w| host.parent().as_ref() == Some(w.upcast_ref())) {
            return;
        }
        host.unparent();
        // It goes back to its place in its next parent.
        if let Some(origin) = self.origins.borrow_mut().remove(host)
            && let Some(frame) = self.frames.borrow_mut().get_mut(host)
        {
            frame.origin = origin;
        }
    }

    /// Keeps a pane's size and puts it at the top left of its wrapper: the
    /// paned places the wrapper. The first pane's length is where the
    /// paned's divider goes. Called when a pane's frame changes.
    pub(crate) fn place(&self, host: &gtk::Widget) {
        let first = host.parent().as_ref() == Some(self.wrappers[0].upcast_ref());
        let mut frames = self.frames.borrow_mut();
        let Some(frame) = frames.get_mut(host) else { return };
        self.origins.borrow_mut().insert(host.clone(), frame.origin);
        let length = if self.vertical() { frame.height() } else { frame.width() };
        frame.origin = Point::ZERO;
        drop(frames);
        if first {
            self.paned.set_position(length.round() as i32);
        }
        for wrapper in &self.wrappers {
            wrapper.queue_allocate();
        }
    }

    pub(crate) fn set_vertical(&self, vertical: bool) {
        self.paned.set_orientation(if vertical { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal });
    }

    pub(crate) fn vertical(&self) -> bool {
        self.paned.orientation() == gtk::Orientation::Vertical
    }

    pub(crate) fn set_minimums(&self, minimums: [f32; 2]) {
        self.state.minimums.set(minimums);
    }

    pub(crate) fn minimums(&self) -> [f32; 2] {
        self.state.minimums.get()
    }

    /// Shows or hides the panes' wrappers: the paned hides its handle
    /// while either child is hidden, and gives the other the whole split.
    /// The wrappers are there from the start, so a pane inserted later
    /// lands in the right one.
    pub(crate) fn set_shown(&self, shown: [bool; 2]) {
        for (wrapper, shown) in self.wrappers.iter().zip(shown) {
            wrapper.set_visible(shown);
        }
    }

    /// Which panes show, from their wrappers.
    pub(crate) fn shown(&self) -> [bool; 2] {
        [self.wrappers[0].is_visible(), self.wrappers[1].is_visible()]
    }

    /// The app's position: the core lays the panes out at it.
    pub(crate) fn set_position(&self, position: f64) {
        self.state.position.set(Some(position));
    }

    pub(crate) fn position(&self) -> Option<f64> {
        self.state.position.get()
    }

    /// Moves the divider as the user would (assistive technology sets a
    /// splitter's value), within the panes' least sizes: it's reported.
    pub(crate) fn move_divider(&self, position: f64) {
        let [first, second] = self.state.minimums.get();
        let position = keep(&self.paned, position.round() as i32, first, second);
        let was = self.state.moving.replace(true);
        self.paned.set_position(position);
        self.state.moving.set(was);
    }

    /// The pane hosts, first then second.
    pub(crate) fn panes(&self) -> Vec<gtk::Widget> {
        self.wrappers.iter().filter_map(WidgetExt::first_child).collect()
    }

    /// Where the paned put a pane, in the split's coordinates.
    pub(crate) fn pane_frame(&self, host: &gtk::Widget, size: Size) -> Rect {
        let at = host
            .is_mapped()
            .then(|| host.compute_point(&self.paned, &graphene::Point::new(0.0, 0.0)))
            .flatten()
            .map(|p| Point::new(p.x(), p.y()));
        let origin = at.or_else(|| self.origins.borrow().get(host).copied()).unwrap_or(Point::ZERO);
        Rect { origin, size }
    }

    /// The node is gone: the shared frames let go of the wrappers.
    pub(crate) fn forget(&self) {
        let mut frames = self.frames.borrow_mut();
        for wrapper in &self.wrappers {
            frames.remove(wrapper.upcast_ref::<gtk::Widget>());
        }
    }
}

/// A position within the panes' least lengths, once the paned has a size.
fn keep(paned: &gtk::Paned, position: i32, first: f32, second: f32) -> i32 {
    let length = if paned.orientation() == gtk::Orientation::Vertical { paned.height() } else { paned.width() };
    let most = length - divider() - second.round() as i32;
    let position = if length > 0 { position.min(most) } else { position };
    position.max(first.round() as i32)
}

thread_local! {
    /// The handle's thickness, as the metrics last measured it.
    static DIVIDER: Cell<i32> = const { Cell::new(1) };
}

fn divider() -> i32 {
    DIVIDER.get()
}

/// Measures the handle's thickness from a probe (Adwaita's is a 1px line),
/// for the metrics: the theme decides it.
pub(crate) fn measure_divider() -> f32 {
    let probe = gtk::Paned::new(gtk::Orientation::Horizontal);
    probe.set_wide_handle(false);
    probe.set_start_child(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
    probe.set_end_child(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
    let thickness = probe.measure(gtk::Orientation::Horizontal, -1).0;
    DIVIDER.set(thickness);
    thickness as f32
}
