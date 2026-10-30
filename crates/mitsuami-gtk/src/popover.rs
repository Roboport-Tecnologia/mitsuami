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
        if let Some(b) = self.host.compute_bounds(&parent) {
            return Rect::new(b.x(), b.y(), b.width(), b.height());
        }
        // GTK 4.14 relates no widgets across surfaces, and a popover is one
        // of its own: through its popup's place on its parent's surface.
        self.across_surfaces(&parent).unwrap_or(Rect::ZERO)
    }

    /// The popup's place is in its parent window's coordinates (past the
    /// window's shadow): what 4.24's `compute_bounds` gives.
    fn across_surfaces(&self, parent: &gtk::Widget) -> Option<Rect> {
        let popup = self.popover.surface()?.downcast::<gtk::gdk::Popup>().ok()?;
        let at = parent.compute_bounds(&parent.native()?)?;
        let (inset_x, inset_y) = self.popover.surface_transform();
        let host = self.host.compute_bounds(&self.popover)?;
        let x = popup.position_x() as f64 + inset_x + host.x() as f64 - at.x() as f64;
        let y = popup.position_y() as f64 + inset_y + host.y() as f64 - at.y() as f64;
        Some(Rect::new(x as f32, y as f32, host.width(), host.height()))
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
