//! A window's toolbar: an `NSToolbar` in its title bar, which shows the
//! window's title, with one `NSToolbarItem` per toolbar item node. Each
//! item's view holds the node's host, as big as the core sized it; a
//! flexible space ahead of them puts them at the trailing end.

use std::cell::RefCell;

use mitsuami_core::NodeId;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSAnimationContext, NSLayoutConstraint, NSToolbar, NSToolbarDelegate, NSToolbarDisplayMode,
    NSToolbarFlexibleSpaceItemIdentifier, NSToolbarItem, NSToolbarItemIdentifier, NSView, NSWindow,
};
use objc2_foundation::{NSArray, NSOperatingSystemVersion, NSProcessInfo, NSRect, NSString};

pub(crate) struct DelegateIvars {
    /// The items the toolbar shows after its flexible space, in order.
    shown: RefCell<Vec<Retained<NSToolbarItem>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    pub(crate) struct ToolbarDelegate;

    unsafe impl NSObjectProtocol for ToolbarDelegate {}

    unsafe impl NSToolbarDelegate for ToolbarDelegate {
        #[unsafe(method_id(toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:))]
        fn item(
            &self,
            _toolbar: &NSToolbar,
            identifier: &NSToolbarItemIdentifier,
            _inserted: bool,
        ) -> Option<Retained<NSToolbarItem>> {
            self.ivars().shown.borrow().iter().find(|item| &*item.itemIdentifier() == identifier).cloned()
        }

        #[unsafe(method_id(toolbarDefaultItemIdentifiers:))]
        fn default_items(&self, _toolbar: &NSToolbar) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            self.identifiers()
        }

        #[unsafe(method_id(toolbarAllowedItemIdentifiers:))]
        fn allowed_items(&self, _toolbar: &NSToolbar) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            self.identifiers()
        }
    }
);

impl ToolbarDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<ToolbarDelegate> {
        let this = ToolbarDelegate::alloc(mtm).set_ivars(DelegateIvars { shown: RefCell::new(Vec::new()) });
        unsafe { msg_send![super(this), init] }
    }

    fn identifiers(&self) -> Retained<NSArray<NSToolbarItemIdentifier>> {
        let mut identifiers = vec![unsafe { NSToolbarFlexibleSpaceItemIdentifier }.retain()];
        identifiers.extend(self.ivars().shown.borrow().iter().map(|item| item.itemIdentifier()));
        NSArray::from_retained_slice(&identifiers)
    }
}

/// How far an item's content is from the sides of its glass capsule on
/// macOS 26, where each item has one: as far as the system's own image
/// items have their image. The capsule is drawn around the item's view,
/// which the system's views pad themselves.
const GLASS_INSET: f64 = 10.0;

fn glass() -> bool {
    let version = NSOperatingSystemVersion { majorVersion: 26, minorVersion: 0, patchVersion: 0 };
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(version)
}

struct Item {
    id: NodeId,
    item: Retained<NSToolbarItem>,
    host: Retained<NSView>,
    width: Retained<NSLayoutConstraint>,
    height: Retained<NSLayoutConstraint>,
    /// Empty items are out of the toolbar: AppKit can hide an item only
    /// from macOS 15.
    empty: bool,
}

/// A window's toolbar, made when its first item arrives.
pub(crate) struct Toolbar {
    toolbar: Retained<NSToolbar>,
    delegate: Retained<ToolbarDelegate>,
    items: Vec<Item>,
    /// Animate items coming and going, as toolbars do. Tests read where
    /// items end up, not where they are on the way.
    animate: bool,
    /// Space between an item's view and its host, on each side.
    inset: f64,
}

impl Toolbar {
    pub(crate) fn new(mtm: MainThreadMarker, window: &NSWindow, id: NodeId, animate: bool) -> Toolbar {
        // Toolbars with the same identifier keep each other in sync.
        let identifier = NSString::from_str(&format!("mitsuami.window.{}", id.raw()));
        let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &identifier);
        let delegate = ToolbarDelegate::new(mtm);
        toolbar.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The items are views; there are no labels to show under them.
        toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
        toolbar.setAllowsUserCustomization(false);
        // The content keeps its size: the window grows by the bar.
        let content = window.contentRectForFrameRect(window.frame()).size;
        window.setToolbar(Some(&toolbar));
        window.setContentSize(content);
        let inset = if glass() { GLASS_INSET } else { 0.0 };
        Toolbar { toolbar, delegate, items: Vec::new(), animate, inset }
    }

    /// Adds the host of an item node at `index` among the items.
    pub(crate) fn insert(&mut self, mtm: MainThreadMarker, id: NodeId, host: &NSView, index: usize) {
        let identifier = NSString::from_str(&format!("mitsuami.item.{}", id.raw()));
        let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &identifier);
        // Toolbars size item views with Auto Layout. The host sits in a
        // view of its own, inset from the capsule's sides.
        let view = NSView::new(mtm);
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
        host.setTranslatesAutoresizingMaskIntoConstraints(false);
        view.addSubview(host);
        let width = host.widthAnchor().constraintEqualToConstant(0.0);
        let height = host.heightAnchor().constraintEqualToConstant(0.0);
        for constraint in [
            &width,
            &height,
            &host.leadingAnchor().constraintEqualToAnchor_constant(&view.leadingAnchor(), self.inset),
            &view.trailingAnchor().constraintEqualToAnchor_constant(&host.trailingAnchor(), self.inset),
            &host.topAnchor().constraintEqualToAnchor(&view.topAnchor()),
            &host.bottomAnchor().constraintEqualToAnchor(&view.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        item.setView(Some(&view));
        let host = host.retain();
        self.items.insert(index.min(self.items.len()), Item { id, item, host, width, height, empty: true });
        self.sync();
    }

    pub(crate) fn remove(&mut self, id: NodeId) {
        let Some(index) = self.items.iter().position(|i| i.id == id) else { return };
        let item = self.items.remove(index);
        item.host.removeFromSuperview();
        item.width.setActive(false);
        item.height.setActive(false);
        item.item.setView(None);
        self.sync();
    }

    /// Sizes an item's host; an empty one leaves the toolbar.
    pub(crate) fn set_size(&mut self, id: NodeId, width: f64, height: f64) {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
        item.width.setConstant(width);
        item.height.setConstant(height);
        item.empty = width <= 0.0 || height <= 0.0;
        self.sync();
    }

    pub(crate) fn contains(&self, id: NodeId) -> bool {
        self.items.iter().any(|i| i.id == id)
    }

    /// The item nodes, in order.
    pub(crate) fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.items.iter().map(|i| i.id)
    }

    /// Where the toolbar shows an item, in the coordinates of the window's
    /// content view (above it, so at negative y); `None` while it's empty.
    pub(crate) fn frame(&self, id: NodeId, content: &NSView) -> Option<NSRect> {
        let item = self.items.iter().find(|i| i.id == id).filter(|i| !i.empty)?;
        item.host.window()?;
        Some(item.host.convertRect_toView(item.host.bounds(), Some(content)))
    }

    /// Makes the toolbar show the non-empty items, in order: after the
    /// flexible space, which is always first.
    fn sync(&self) {
        let wanted: Vec<Retained<NSToolbarItem>> =
            self.items.iter().filter(|i| !i.empty).map(|i| i.item.clone()).collect();
        let shown = self.toolbar.items();
        let same =
            shown.len() == wanted.len() + 1 && shown.iter().skip(1).zip(&wanted).all(|(a, b)| std::ptr::eq(&*a, &**b));
        if same {
            return;
        }
        *self.delegate.ivars().shown.borrow_mut() = wanted.clone();
        if !self.animate {
            NSAnimationContext::beginGrouping();
            NSAnimationContext::currentContext().setDuration(0.0);
        }
        for index in (0..shown.len()).rev() {
            self.toolbar.removeItemAtIndex(index as isize);
        }
        for (index, identifier) in self.delegate.identifiers().iter().enumerate() {
            self.toolbar.insertItemWithItemIdentifier_atIndex(&identifier, index as isize);
        }
        if !self.animate {
            NSAnimationContext::endGrouping();
        }
    }
}
