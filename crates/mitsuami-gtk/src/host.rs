//! The layout host: a widget that places its children at the frames the
//! core computed, and never lays anything out itself.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{graphene, gsk};
use mitsuami_core::{EventSink, NodeId, Rect, Size, UiEvent};

/// Every node's frame, parent-relative, as last sent by the core. Shared
/// by all hosts: a child keeps its frame when it moves to another parent.
pub(crate) type Frames = Rc<RefCell<HashMap<gtk::Widget, Rect>>>;

/// Where native events go. Signal handlers hold a clone.
#[derive(Clone, Default)]
pub(crate) struct Events(Rc<EventsInner>);

#[derive(Default)]
struct EventsInner {
    sink: RefCell<EventSink>,
    /// Set while the backend applies commands: GTK reports programmatic
    /// value changes (`set_active`, `set_text`, a native render's update)
    /// like user ones, and those aren't user events. Focus and scroll
    /// changes are still reported.
    muted: Cell<bool>,
    /// Called after each event, so the run loop ticks.
    wake: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Events {
    pub(crate) fn emit(&self, id: NodeId, event: UiEvent) {
        if self.0.muted.get() && matches!(event, UiEvent::Changed(_) | UiEvent::Custom(_)) {
            return;
        }
        self.0.sink.borrow().emit(id, event);
        let wake = self.0.wake.borrow().clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    /// Reports a change the user didn't make but must hear about, even
    /// while the backend applies commands: selected rows a data change
    /// removed.
    pub(crate) fn emit_always(&self, id: NodeId, event: UiEvent) {
        let was = self.0.muted.replace(false);
        self.emit(id, event);
        self.0.muted.set(was);
    }

    /// Whether the backend is applying commands: signals are ours, not the
    /// user's.
    pub(crate) fn is_muted(&self) -> bool {
        self.0.muted.get()
    }

    pub(crate) fn muted<R>(&self, f: impl FnOnce() -> R) -> R {
        let was = self.0.muted.replace(true);
        let result = f();
        self.0.muted.set(was);
        result
    }

    pub(crate) fn set_sink(&self, sink: EventSink) {
        *self.0.sink.borrow_mut() = sink;
    }

    pub(crate) fn set_wake(&self, wake: impl Fn() + 'static) {
        *self.0.wake.borrow_mut() = Some(Rc::new(wake));
    }
}

/// What the content host of a window does on top of placing children.
pub(crate) struct WindowRoot {
    pub(crate) id: NodeId,
    pub(crate) events: Events,
    /// Content size the core knows about (requested or last reported).
    pub(crate) size: Cell<Size>,
    /// The core's Tab order.
    pub(crate) focus_order: RefCell<Vec<gtk::Widget>>,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub(crate) struct Host {
        pub(super) frames: std::cell::OnceCell<Frames>,
        pub(super) root: std::cell::OnceCell<WindowRoot>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Host {
        const NAME: &'static str = "MitsuamiHost";
        type Type = super::Host;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Host {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Host {
        /// Hosts ask for exactly their frame: the core lays them out, and
        /// a scroll view's viewport learns the content size from this.
        /// Window content hosts have no frame and ask for nothing, so
        /// windows can shrink freely.
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let obj = self.obj();
            let frame = self.frames.get().and_then(|f| f.borrow().get(obj.upcast_ref::<gtk::Widget>()).copied());
            let size = frame.map_or(0, |f| {
                (if orientation == gtk::Orientation::Horizontal { f.width() } else { f.height() }).round() as i32
            });
            (size, size, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let frames = self.frames.get().expect("hosts are built with frames").borrow();
            let mut next = self.obj().first_child();
            while let Some(child) = next {
                next = child.next_sibling();
                // Hidden by the backend (an empty frame): nothing to place.
                if !child.is_child_visible() {
                    continue;
                }
                let frame = frames.get(&child).copied().unwrap_or_default();
                // GTK wants every widget measured before it's allocated.
                child.measure(gtk::Orientation::Horizontal, -1);
                let transform = gsk::Transform::new().translate(&graphene::Point::new(frame.x(), frame.y()));
                child.allocate(frame.width().round() as i32, frame.height().round() as i32, -1, Some(transform));
            }
            if let Some(root) = self.root.get() {
                let size = Size::new(width as f32, height as f32);
                if root.size.replace(size) != size {
                    root.events.emit(root.id, UiEvent::WindowResized(size));
                }
            }
        }

        /// Tab and Shift+Tab follow the core's order in window content;
        /// other directions (arrow keys) keep GTK's geometric behaviour.
        fn focus(&self, direction: gtk::DirectionType) -> bool {
            let forward = match direction {
                gtk::DirectionType::TabForward => true,
                gtk::DirectionType::TabBackward => false,
                _ => return self.parent_focus(direction),
            };
            let Some(root) = self.root.get() else { return self.parent_focus(direction) };
            let order = root.focus_order.borrow().clone();
            if order.is_empty() {
                return false;
            }
            let focus = self.obj().root().and_then(|r| r.focus());
            // Entries put focus on their inner text widget.
            let at = focus.and_then(|f| order.iter().position(|w| f == *w || f.is_ancestor(w)));
            let n = order.len();
            let steps: Vec<usize> = match at {
                Some(at) => (1..=n).map(|s| if forward { (at + s) % n } else { (at + n - s) % n }).collect(),
                None if forward => (0..n).collect(),
                None => (0..n).rev().collect(),
            };
            // Wraps around on its own, so focus stays in the content.
            steps.into_iter().any(|i| order[i].grab_focus())
        }
    }
}

glib::wrapper! {
    pub(crate) struct Host(ObjectSubclass<imp::Host>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Host {
    pub(crate) fn new(frames: Frames, root: Option<WindowRoot>) -> Host {
        let host: Host = glib::Object::new();
        let _ = host.imp().frames.set(frames);
        if let Some(root) = root {
            // Paints the window background, so captures of the content
            // look like the window.
            host.add_css_class("background");
            let _ = host.imp().root.set(root);
        }
        host
    }

    pub(crate) fn window_root(&self) -> Option<&WindowRoot> {
        self.imp().root.get()
    }
}
