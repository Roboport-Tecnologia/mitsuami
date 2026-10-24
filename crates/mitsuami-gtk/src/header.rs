//! A window's header (`Prop::Header`): a container at the top of the
//! window's content that goes in the window's top bars, under the header
//! bar, as libadwaita apps put a search bar or a tab bar there. The window
//! becomes an `AdwToolbarView` with raised top bars, which share one
//! background, the header bar's, and one shade under the last.
//!
//! As on AppKit, the core lays the header out at the content's top, where
//! its own host stays, empty, holding its place; its children are in a
//! host of their own, which is in that placeholder until the header goes
//! in the top bars, and back in it when it leaves. The window's host runs
//! up under the header (the toolbar view extends its content to its top
//! edge, and the host's top margin is the header bar's height), so the
//! core's layout stands as it was.

use std::cell::Cell;

use gtk::prelude::*;
use mitsuami_core::{NodeId, Rect};

use crate::host::{Frames, Host};

pub(crate) struct Header {
    /// The header's children.
    pub(crate) content: Host,
    /// The app made it a header (it can unmake it).
    pub(crate) on: Cell<bool>,
    /// The window whose top bars it's in.
    pub(crate) window: Cell<Option<NodeId>>,
}

impl Header {
    /// Makes `placeholder` a header: its children (none yet, as a rule)
    /// move into the header's own host, which fills it.
    pub(crate) fn new(frames: Frames, placeholder: &Host) -> Header {
        let content = Host::new(frames, None);
        let mut next = placeholder.first_child();
        while let Some(child) = next {
            next = child.next_sibling();
            // A context menu's popover stays by the placeholder.
            if child.is::<gtk::Popover>() {
                continue;
            }
            child.unparent();
            child.set_parent(&content);
        }
        content.set_parent(placeholder);
        let header = Header { content, on: Cell::new(true), window: Cell::new(None) };
        header.place(placeholder);
        header
    }

    /// Sizes its host to the placeholder's frame, wherever it is.
    pub(crate) fn place(&self, placeholder: &Host) {
        let frames = self.content.frames();
        let size = frames.borrow().get(placeholder.upcast_ref::<gtk::Widget>()).map(|f| f.size).unwrap_or_default();
        frames.borrow_mut().insert(self.content.clone().upcast(), Rect::new(0.0, 0.0, size.width, size.height));
        self.content.queue_resize();
    }

    /// The node is gone: the shared frames let go of its host.
    pub(crate) fn forget(&self) {
        self.content.frames().borrow_mut().remove(self.content.upcast_ref::<gtk::Widget>());
    }

    /// Puts it under `window`'s header bar, the window's content running up
    /// under both. Returns the toolbar view the window is now.
    pub(crate) fn attach(
        &self,
        id: NodeId,
        window: &gtk::Window,
        header_bar: &adw::HeaderBar,
        host: &Host,
    ) -> adw::ToolbarView {
        let view = adw::ToolbarView::new();
        view.set_top_bar_style(adw::ToolbarStyle::Raised);
        view.set_extend_content_to_top_edge(true);
        window.set_child(None::<&gtk::Widget>);
        // As `AdwWindow` does: a title bar that isn't shown, so GTK adds
        // none of its own.
        let hidden = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hidden.set_visible(false);
        window.set_titlebar(Some(&hidden));
        view.add_top_bar(header_bar);
        self.content.unparent();
        view.add_top_bar(&self.content);
        view.set_content(Some(host));
        window.set_child(Some(&view));
        self.window.set(Some(id));
        view
    }

    /// Out of the top bars, back in its placeholder, if it's still there.
    pub(crate) fn detach(&self, placeholder: Option<&Host>) {
        self.window.set(None);
        if self.content.parent().is_some() {
            self.content.unparent();
        }
        if let Some(placeholder) = placeholder {
            self.content.set_parent(placeholder);
        }
    }
}

/// What's above a window's content under a header: its top bars (the
/// header bar and the header, and the spacing libadwaita gives two bars),
/// less the header, which the content runs up under.
pub(crate) fn above_content(view: &adw::ToolbarView, header: &Host) -> i32 {
    let mut next = view.first_child();
    while let Some(child) = next {
        if child.has_css_class("top-bar") {
            let bars = child.measure(gtk::Orientation::Vertical, -1).1;
            return (bars - header.measure(gtk::Orientation::Vertical, -1).1).max(0);
        }
        next = child.next_sibling();
    }
    0
}

/// Gives a window whose header left its header bar and content back, as
/// `create_window` made them.
pub(crate) fn restore_window(view: &adw::ToolbarView, window: &gtk::Window, header_bar: &adw::HeaderBar, host: &Host) {
    window.set_child(None::<&gtk::Widget>);
    view.set_content(None::<&gtk::Widget>);
    view.remove(header_bar);
    host.set_margin_top(0);
    window.set_titlebar(Some(header_bar));
    window.set_child(Some(host));
}
