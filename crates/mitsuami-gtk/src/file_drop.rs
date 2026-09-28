//! Files dropped on a host: a `gtk::DropTarget` for `gdk::FileList`.
//!
//! GTK only hands a drop's files over once they're read, so the target
//! preloads them while they hover: until they're in, it offers to copy;
//! once they are, it rejects a drag with nothing it takes, as a drag
//! whose files the app can't open is refused elsewhere. Synthetic drags
//! run the same functions with the paths given.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use mitsuami_core::{FileDrop, NodeId, UiEvent};

use crate::host::Events;

pub(crate) struct FileDropTarget {
    widget: gtk::Widget,
    controller: gtk::DropTarget,
    state: Rc<DropState>,
}

struct DropState {
    id: NodeId,
    events: Events,
    drop: RefCell<FileDrop>,
    /// Whether it reported `DropHover(true)` without its `false` yet.
    hover: Cell<bool>,
}

impl DropState {
    /// Files entered or moved over it: whether it takes any, and the
    /// hover reported once.
    fn over(&self, paths: &[PathBuf]) -> bool {
        let takes = !self.drop.borrow().accepted(paths).is_empty();
        if takes && !self.hover.replace(true) {
            self.events.emit(self.id, UiEvent::DropHover(true));
        }
        takes
    }

    fn leave(&self) {
        if self.hover.replace(false) {
            self.events.emit(self.id, UiEvent::DropHover(false));
        }
    }

    /// Dropped: the files it takes, if any; whether it took the drop.
    fn dropped(&self, paths: &[PathBuf]) -> bool {
        self.over(paths);
        let accepted = self.drop.borrow().accepted(paths);
        if !accepted.is_empty() {
            self.events.emit(self.id, UiEvent::FilesDropped(accepted.clone()));
        }
        self.leave();
        !accepted.is_empty()
    }
}

/// A file list's local paths; files without one (remote URIs) aren't
/// taken, as the app gets paths.
fn paths(value: &glib::Value) -> Vec<PathBuf> {
    value
        .get::<gdk::FileList>()
        .map(|list| list.files().iter().filter_map(gio::File::path).collect())
        .unwrap_or_default()
}

impl FileDropTarget {
    pub(crate) fn new(id: NodeId, events: Events, widget: &gtk::Widget, drop: FileDrop) -> FileDropTarget {
        let controller = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        controller.set_preload(true);
        let state = Rc::new(DropState { id, events, drop: RefCell::new(drop), hover: Cell::new(false) });
        // The files, once read: a drag with nothing it takes is refused.
        controller.connect_value_notify({
            let state = state.clone();
            move |target| {
                let Some(value) = target.value() else { return };
                if !state.over(&paths(&value)) {
                    target.reject();
                }
            }
        });
        let offer = {
            let state = state.clone();
            move |target: &gtk::DropTarget, _: f64, _: f64| match target.value() {
                Some(value) if !state.over(&paths(&value)) => gdk::DragAction::empty(),
                _ => gdk::DragAction::COPY,
            }
        };
        controller.connect_enter(offer.clone());
        controller.connect_motion(offer);
        controller.connect_leave({
            let state = state.clone();
            move |_| state.leave()
        });
        controller.connect_drop({
            let state = state.clone();
            move |_, value, _, _| state.dropped(&paths(value))
        });
        widget.add_controller(controller.clone());
        FileDropTarget { widget: widget.clone(), controller, state }
    }

    pub(crate) fn set(&self, drop: FileDrop) {
        *self.state.drop.borrow_mut() = drop;
    }

    pub(crate) fn file_drop(&self) -> FileDrop {
        self.state.drop.borrow().clone()
    }

    /// Synthetic input: what the controller's handlers do, with these
    /// paths.
    pub(crate) fn drag(&self, paths: &[PathBuf]) {
        self.state.over(paths);
    }

    pub(crate) fn drag_leave(&self) {
        self.state.leave();
    }

    pub(crate) fn drop_files(&self, paths: &[PathBuf]) {
        self.state.dropped(paths);
    }

    /// Takes the controller off the widget; a hover in progress ends.
    pub(crate) fn remove(self) {
        self.state.leave();
        self.widget.remove_controller(&self.controller);
    }
}
