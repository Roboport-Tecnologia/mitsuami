//! `Popover`: a `gtk::Popover` whose child is the node's host, shown under
//! its parent while the core says so. It hides on a click outside and on
//! Escape, as GNOME's do (`autohide`), which is reported. Shown before
//! its parent is in a window, it pops up once the parent maps: GTK can't
//! place a popover without its parent's surface.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use mitsuami_core::{NodeId, Rect, UiEvent};

use crate::host::{Events, Frames, Host};

pub(crate) struct Popover {
    pub(crate) popover: gtk::Popover,
    pub(crate) host: Host,
    /// Whether the core shows it.
    wanted: Rc<Cell<bool>>,
}

impl Popover {
    pub(crate) fn new(id: NodeId, events: Events, frames: Frames) -> Popover {
        let host = Host::new(frames, None);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&host));
        popover.set_autohide(true);
        popover.set_position(gtk::PositionType::Bottom);
        let wanted = Rc::new(Cell::new(false));
        // The user's dismissal; the backend's own `popdown` is muted.
        let dismissed = wanted.clone();
        popover.connect_closed(move |_| {
            if !events.is_muted() {
                dismissed.set(false);
                events.emit(id, UiEvent::PopoverShownChanged(false));
            }
        });
        // Its parent: shown as it maps, if it's wanted by then.
        let (weak, shown) = (popover.downgrade(), wanted.clone());
        popover.connect_notify_local(Some("parent"), move |popover, _| {
            let Some(parent) = popover.parent() else { return };
            let (weak, shown) = (weak.clone(), shown.clone());
            parent.connect_map(move |_| {
                if let Some(popover) = weak.upgrade() {
                    sync(&popover, shown.get());
                }
            });
        });
        Popover { popover, host, wanted }
    }

    pub(crate) fn set_shown(&self, shown: bool) {
        self.wanted.set(shown);
        sync(&self.popover, shown);
    }

    pub(crate) fn shown(&self) -> bool {
        self.popover.is_visible()
    }

    /// Its content's size, as the core laid it out: the host takes it,
    /// and the popover fits it.
    pub(crate) fn set_size(&self, frames: &Frames, width: f32, height: f32) {
        frames.borrow_mut().insert(self.host.clone().upcast(), Rect::new(0.0, 0.0, width, height));
        self.host.queue_resize();
    }

    /// Where its content shows, in its parent's coordinates; empty while
    /// it's hidden.
    pub(crate) fn frame(&self) -> Rect {
        let Some(parent) = self.popover.parent().filter(|_| self.shown() && self.host.is_mapped()) else {
            return Rect::ZERO;
        };
        match self.host.compute_bounds(&parent) {
            Some(b) => Rect::new(b.x(), b.y(), b.width(), b.height()),
            None => Rect::ZERO,
        }
    }
}

/// Pops the popover up or down as wanted, up only once its parent is
/// mapped.
fn sync(popover: &gtk::Popover, wanted: bool) {
    let mapped = popover.parent().is_some_and(|p| p.is_mapped());
    match wanted {
        true if mapped && !popover.is_visible() => popover.popup(),
        false if popover.is_visible() => popover.popdown(),
        _ => {}
    }
}
