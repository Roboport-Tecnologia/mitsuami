//! A window's header (`Prop::Header`): a container at the top of the
//! window's content that goes in the title bar, as a bottom
//! `NSTitlebarAccessoryViewController`, so it's in the bar's glass under
//! the title and toolbar, as Safari's and Mail's bars are. The content
//! view is full size then, as with a toolbar (`content_under_bar`), and the
//! scroll view right under the header runs up under it and the bar
//! (`extend_under_bars`), which blurs what scrolls there.
//!
//! The core lays the header out at the content's top, where its own view
//! stays, empty, holding its place; its children are in a view of their
//! own, which is in that placeholder until the header goes in the title
//! bar, and back in it when it leaves. The window's host runs up under the
//! accessory by its height, so the core's layout stands as it was: the
//! placeholder is under the accessory, and what's under the header where
//! the core put it.

use std::cell::Cell;

use mitsuami_core::{HeaderEdge, NodeId};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, NSObjectProtocol};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSLayoutAttribute, NSLayoutRelation, NSTitlebarAccessoryViewController, NSView, NSWindow,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::classes::HostView;

pub(crate) struct Header {
    /// The header's children.
    pub(crate) content: Retained<HostView>,
    accessory: Retained<NSTitlebarAccessoryViewController>,
    /// The app made it a header (it can unmake it).
    pub(crate) on: Cell<bool>,
    /// The window whose title bar it's in.
    pub(crate) window: Cell<Option<NodeId>>,
}

impl Header {
    /// Makes `placeholder` a header: its children (none yet, as a rule)
    /// move into the header's own view, which fills it.
    pub(crate) fn new(mtm: MainThreadMarker, placeholder: &NSView) -> Header {
        let content = HostView::new(mtm);
        for child in placeholder.subviews().iter() {
            child.removeFromSuperview();
            content.addSubview(&child);
        }
        let accessory = NSTitlebarAccessoryViewController::new(mtm);
        accessory.setView(&content);
        accessory.setLayoutAttribute(NSLayoutAttribute::Bottom);
        let header = Header { content, accessory, on: Cell::new(true), window: Cell::new(None) };
        header.fill(placeholder);
        header
    }

    /// Back in its placeholder, filling it.
    fn fill(&self, placeholder: &NSView) {
        self.content.removeFromSuperview();
        self.content.setFrame(placeholder.bounds());
        self.content.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        placeholder.addSubview(&self.content);
    }

    /// Puts it in `window`'s title bar, as high as its placeholder. The
    /// window's content must be full size already.
    pub(crate) fn attach(&self, id: NodeId, window: &NSWindow, host: &NSView, height: f64) {
        self.content.removeFromSuperview();
        self.content.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        self.content.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(host.frame().size.width, height)));
        window.addTitlebarAccessoryViewController(&self.accessory);
        self.window.set(Some(id));
        set_host_top(host, -height);
    }

    /// How it meets what scrolls under it: the accessory's scroll edge
    /// effect style, macOS 26.1's (`preferredScrollEdgeEffectStyle`, which
    /// objc2 doesn't bind yet). Before, AppKit's own.
    pub(crate) fn set_edge(&self, edge: HeaderEdge) {
        let Some(class) = AnyClass::get(c"NSScrollEdgeEffectStyle") else { return };
        if !self.accessory.respondsToSelector(sel!(setPreferredScrollEdgeEffectStyle:)) {
            return;
        }
        let style: Option<Retained<AnyObject>> = unsafe {
            match edge {
                HeaderEdge::Automatic => msg_send![class, automaticStyle],
                HeaderEdge::Soft => msg_send![class, softStyle],
                HeaderEdge::Hard => msg_send![class, hardStyle],
            }
        };
        if let Some(style) = style {
            let _: () = unsafe { msg_send![&*self.accessory, setPreferredScrollEdgeEffectStyle: &*style] };
        }
    }

    /// Follows its placeholder's height in the title bar.
    pub(crate) fn set_height(&self, host: &NSView, height: f64) {
        let size = self.content.frame().size;
        if size.height != height {
            self.content.setFrameSize(NSSize::new(size.width, height));
            set_host_top(host, -height);
        }
    }

    /// Out of the title bar, back in its placeholder. `host`: the window's,
    /// if the window is still there.
    pub(crate) fn detach(&self, placeholder: &NSView, host: Option<&NSView>) {
        if self.window.take().is_none() {
            return;
        }
        self.accessory.removeFromParentViewController();
        if let Some(host) = host {
            set_host_top(host, 0.0);
        }
        self.fill(placeholder);
    }
}

/// Moves the window's host from the bottom of its title bar and
/// accessories by `offset` (`content_under_bar`'s constraint).
fn set_host_top(host: &NSView, offset: f64) {
    let Some(frame) = (unsafe { host.superview() }) else { return };
    for constraint in frame.constraints().iter() {
        let first = unsafe { constraint.firstItem() };
        if first.is_some_and(|item| std::ptr::eq(&*item as *const _ as *const NSView, host))
            && constraint.firstAttribute() == NSLayoutAttribute::Top
            && constraint.relation() == NSLayoutRelation::Equal
        {
            constraint.setConstant(offset);
            return;
        }
    }
}
