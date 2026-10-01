//! Hover (`Prop::Hover`): a Qt Quick `HoverHandler` on the node's item, as
//! Qt Quick apps watch the pointer. It's passive, so it stays hovered over
//! the item's children, and they still get the pointer.

use std::rc::Rc;

use mitsuami_core::{NodeId, UiEvent};

use crate::events::Events;
use crate::ffi::QmlObject;

/// An item's hover handler, made on it (its `parent` registers it there).
pub(crate) struct Hover {
    handler: QmlObject,
    report: Rc<dyn Fn(bool)>,
}

impl Hover {
    pub(crate) fn new(item: QmlObject, id: NodeId, events: Events) -> Hover {
        let handler = QmlObject::load_in("HoverHandler {}", item);
        let report: Rc<dyn Fn(bool)> = Rc::new(move |over| events.emit(id, UiEvent::Hover(over)));
        let on = report.clone();
        handler.connect("hoveredChanged()", move || on(handler.bool("hovered")));
        Hover { handler, report }
    }

    /// Whether it's still on `item`.
    pub(crate) fn is_on(&self, item: QmlObject) -> bool {
        self.handler.object("parent") == Some(item)
    }

    /// What its `hoveredChanged` runs, for `synthesize` to run with the
    /// backend's state let go: Qt can't be told the pointer moved without
    /// moving it over the window.
    pub(crate) fn report(&self) -> Rc<dyn Fn(bool)> {
        self.report.clone()
    }

    /// Deletes it now: its item may go right after.
    pub(crate) fn remove(self) {
        self.handler.destroy();
    }
}
