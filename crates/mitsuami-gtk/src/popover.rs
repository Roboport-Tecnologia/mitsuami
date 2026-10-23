//! `Popover`: a `gtk::Popover` whose child is the node's host, shown under
//! its parent while the core says so. It hides on a click outside and on
//! Escape, as GNOME's do (`autohide`), which is reported.

use gtk::prelude::*;
use mitsuami_core::{NodeId, Rect, UiEvent};

use crate::host::{Events, Frames, Host};

pub(crate) struct Popover {
    pub(crate) popover: gtk::Popover,
    pub(crate) host: Host,
}

impl Popover {
    pub(crate) fn new(id: NodeId, events: Events, frames: Frames) -> Popover {
        let host = Host::new(frames, None);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&host));
        popover.set_autohide(true);
        popover.set_position(gtk::PositionType::Bottom);
        // The user's dismissal; the backend's own `popdown` is muted.
        popover.connect_closed(move |_| {
            if !events.is_muted() {
                events.emit(id, UiEvent::PopoverShownChanged(false));
            }
        });
        Popover { popover, host }
    }

    pub(crate) fn set_shown(&self, shown: bool) {
        match shown {
            true => self.popover.popup(),
            false => self.popover.popdown(),
        }
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
