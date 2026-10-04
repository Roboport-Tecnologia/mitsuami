//! A group: a layout host with, behind its children, a heading and a
//! libadwaita card under it, as `AdwPreferencesGroup` lays one out. The
//! core places the children inside the card (`insets`); the heading and
//! card are the host's first two children, sized with it.

use std::cell::RefCell;

use gtk::glib;
use gtk::prelude::*;
use mitsuami_core::{Insets, Rect};

use crate::host::{Frames, Host};

/// What's between the heading and the card, as in a preferences group.
const HEADING_GAP: f32 = 12.0;
/// Content inside the card, as far in as GNOME apps put it.
const MARGIN: f32 = 12.0;

/// The group node's native parts.
pub(crate) struct Group {
    pub host: Host,
    pub heading: gtk::Label,
    pub card: gtk::Box,
    frames: Frames,
}

impl Group {
    pub(crate) fn new(frames: Frames) -> Group {
        let host = Host::new(frames.clone(), None);
        let heading = gtk::Label::new(None);
        heading.add_css_class("heading");
        heading.set_xalign(0.0);
        heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
        // The group is named by it: the heading isn't read again.
        heading.set_accessible_role(gtk::AccessibleRole::Presentation);
        heading.set_visible(false);
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("card");
        card.set_accessible_role(gtk::AccessibleRole::Presentation);
        // Behind the children the core inserts after them.
        heading.set_parent(&host);
        card.set_parent(&host);
        Group { host, heading, card, frames }
    }

    /// How many of the host's children are the group's own, before the
    /// core's.
    pub(crate) const OWN_CHILDREN: usize = 2;

    pub(crate) fn set_title(&self, title: &str) {
        self.heading.set_label(title);
        self.heading.set_visible(!title.is_empty());
        self.place();
    }

    pub(crate) fn title(&self) -> String {
        self.heading.label().to_string()
    }

    /// Sizes the heading and card to the group's frame.
    pub(crate) fn place(&self) {
        let size = self.frames.borrow().get(self.host.upcast_ref::<gtk::Widget>()).map(|f| f.size).unwrap_or_default();
        let top = if self.heading.is_visible() { heading_height() + HEADING_GAP } else { 0.0 };
        let mut frames = self.frames.borrow_mut();
        frames.insert(self.heading.clone().upcast(), Rect::new(0.0, 0.0, size.width, heading_height()));
        frames.insert(self.card.clone().upcast(), Rect::new(0.0, top, size.width, (size.height - top).max(0.0)));
        drop(frames);
        self.host.queue_allocate();
    }

    /// The node is gone: the shared frames let go of the heading and
    /// card, which only the core's nodes' frames are removed with.
    pub(crate) fn forget(&self) {
        let mut frames = self.frames.borrow_mut();
        frames.remove(self.heading.upcast_ref::<gtk::Widget>());
        frames.remove(self.card.upcast_ref::<gtk::Widget>());
    }
}

thread_local! {
    /// The heading's height, and the font, theme and text scale it was
    /// measured with.
    static HEADING: RefCell<Option<(HeadingKey, f32)>> = const { RefCell::new(None) };
}

type HeadingKey = (Option<glib::GString>, Option<glib::GString>, i32);

/// The heading's line height, from a throwaway label, as a preferences
/// group's heading is: measured once for each font, theme and text scale.
fn heading_height() -> f32 {
    let settings = gtk::Settings::default();
    let key = settings.as_ref().map_or((None, None, 0), |s| (s.gtk_font_name(), s.gtk_theme_name(), s.gtk_xft_dpi()));
    if let Some(height) = HEADING.with_borrow(|h| h.as_ref().filter(|(k, _)| *k == key).map(|(_, h)| *h)) {
        return height;
    }
    let label = gtk::Label::new(Some("Heading"));
    label.add_css_class("heading");
    let height = label.measure(gtk::Orientation::Vertical, -1).1 as f32;
    HEADING.set(Some((key, height)));
    height
}

/// The metrics changed: the heading is measured again.
pub(crate) fn forget_heading_height() {
    HEADING.set(None);
}

/// Where the content goes: inside the card's margins, and below the
/// heading and its gap when there's one.
pub(crate) fn insets(titled: bool) -> Insets {
    let top = if titled { heading_height() + HEADING_GAP + MARGIN } else { MARGIN };
    Insets::new(top, MARGIN, MARGIN, MARGIN)
}
