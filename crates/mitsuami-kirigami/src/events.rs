//! Where native events go.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{EventSink, NodeId, UiEvent};

/// The event queue, shared by every closure Qt calls back. Cheap to clone.
#[derive(Clone, Default)]
pub(crate) struct Events(Rc<EventsInner>);

#[derive(Default)]
struct EventsInner {
    sink: RefCell<EventSink>,
    /// Set while the backend applies commands. Qt's controls report user
    /// changes with signals of their own (`toggled`, `textEdited`), but a
    /// native render's signals may fire for the backend's own updates, and
    /// those aren't user events. Focus and scroll changes are still
    /// reported.
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

/// The key a node's items carry in Qt (0 means none).
pub(crate) fn node_key(id: NodeId) -> u64 {
    id.raw() as u64 + 1
}

pub(crate) fn node_from_key(key: u64) -> NodeId {
    NodeId::from_raw((key - 1) as u32)
}
