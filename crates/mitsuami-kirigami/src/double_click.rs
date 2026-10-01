//! Double clicks (`Prop::DoubleClick`): a Qt Quick `TapHandler` on the
//! node's item, whose `doubleTapped` is Qt Quick's double click. A control
//! inside takes its own taps, so only the item's and its labels' reach it.

use std::rc::Rc;

use mitsuami_core::{NodeId, UiEvent};

use crate::events::Events;
use crate::ffi::QmlObject;

/// An item's tap handler, made on it (its `parent` registers it there).
pub(crate) struct DoubleClick {
    handler: QmlObject,
    report: Rc<dyn Fn()>,
}

impl DoubleClick {
    pub(crate) fn new(item: QmlObject, id: NodeId, events: Events) -> DoubleClick {
        // `doubleTapped` carries an event point and a button, which the
        // connection can't take: a signal of its own passes it on.
        let handler = QmlObject::load_in(
            "TapHandler { signal mitsuamiDoubleTapped(); onDoubleTapped: mitsuamiDoubleTapped() }",
            item,
        );
        let report: Rc<dyn Fn()> = Rc::new(move || events.emit(id, UiEvent::DoubleClick));
        let on = report.clone();
        handler.connect("mitsuamiDoubleTapped()", move || on());
        DoubleClick { handler, report }
    }

    /// Whether it's still on `item`.
    pub(crate) fn is_on(&self, item: QmlObject) -> bool {
        self.handler.object("parent") == Some(item)
    }

    /// What its signal runs, for `synthesize` to run with the backend's
    /// state let go: Qt can't be sent clicks without a window on screen.
    pub(crate) fn report(&self) -> Rc<dyn Fn()> {
        self.report.clone()
    }

    /// Deletes it now: its item may go right after.
    pub(crate) fn remove(self) {
        self.handler.destroy();
    }
}
