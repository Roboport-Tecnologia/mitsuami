//! Clicks and double clicks (`Prop::Click`, `Prop::DoubleClick`): a Qt
//! Quick `TapHandler` on the node's item, whose `tapped` is a click and
//! `doubleTapped` Qt Quick's double click. A control inside takes its own
//! taps, so only the item's and its labels' reach it.

use std::rc::Rc;

use mitsuami_core::{NodeId, UiEvent};

use crate::events::Events;
use crate::ffi::QmlObject;

/// An item's tap handler, made on it (its `parent` registers it there).
pub(crate) struct Tap {
    handler: QmlObject,
    report: Rc<dyn Fn()>,
}

impl Tap {
    /// Reports `UiEvent::DoubleClick` on a double tap if `double`, else
    /// `UiEvent::Click` on a tap.
    pub(crate) fn new(item: QmlObject, id: NodeId, events: Events, double: bool) -> Tap {
        // `tapped` and `doubleTapped` carry an event point and a button,
        // which the connection can't take: a signal of its own passes it on.
        let (qml, event) = match double {
            true => ("TapHandler { signal mitsuamiTapped(); onDoubleTapped: mitsuamiTapped() }", UiEvent::DoubleClick),
            false => ("TapHandler { signal mitsuamiTapped(); onTapped: mitsuamiTapped() }", UiEvent::Click),
        };
        let handler = QmlObject::load_in(qml, item);
        let report: Rc<dyn Fn()> = Rc::new(move || events.emit(id, event.clone()));
        let on = report.clone();
        handler.connect("mitsuamiTapped()", move || on());
        Tap { handler, report }
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
