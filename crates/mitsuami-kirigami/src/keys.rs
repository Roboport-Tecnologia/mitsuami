//! A node's keys (`Prop::Keys`): Qt Quick sends a key press to the focused
//! item, then up its parent items until one accepts it, so a filter on the
//! node's item takes the keys it was given as they come up, after the
//! items inside it (a list view's arrows, a text field's typing).

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::services::Shortcut;
use mitsuami_core::{NodeId, UiEvent};

use crate::backend::qt_key;
use crate::events::Events;
use crate::ffi::QmlObject;

/// A node's key filter, and the keys it takes.
pub(crate) struct NodeKeys {
    filter: QmlObject,
    keys: Rc<RefCell<Vec<Shortcut>>>,
}

impl NodeKeys {
    pub(crate) fn new(item: QmlObject, id: NodeId, events: Events) -> NodeKeys {
        let keys = Rc::new(RefCell::new(Vec::new()));
        let taken = keys.clone();
        let filter = item.key_filter(move |index| {
            let shortcut = taken.borrow().get(index).copied();
            if let Some(shortcut) = shortcut {
                events.emit(id, UiEvent::Key(shortcut));
            }
        });
        NodeKeys { filter, keys }
    }

    pub(crate) fn set(&self, keys: &[Shortcut]) {
        let qt: Vec<(i32, i32)> = keys.iter().map(|s| (qt_key(s.key).0, modifiers(s))).collect();
        self.filter.set_key_filter(&qt);
        *self.keys.borrow_mut() = keys.to_vec();
    }

    /// The keys, as the app gave them: Qt's codes don't say which
    /// character a key is.
    pub(crate) fn keys(&self) -> Vec<Shortcut> {
        self.keys.borrow().clone()
    }

    pub(crate) fn takes(&self, shortcut: &Shortcut) -> bool {
        self.keys.borrow().contains(shortcut)
    }
}

/// A shortcut's modifiers as the shim's input flags (`MQ_SHIFT`…). The
/// primary modifier is Ctrl.
pub(crate) fn modifiers(shortcut: &Shortcut) -> i32 {
    (shortcut.shift as i32) | (shortcut.primary as i32) << 1 | (shortcut.alt as i32) << 2
}
