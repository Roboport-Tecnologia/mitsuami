//! A window's toolbar: an `NSToolbar` in its title bar, which shows the
//! window's title, with one `NSToolbarItem` per toolbar item node. Each
//! item's view holds the node's host, as big as the core sized it; a
//! flexible space ahead of them puts them at the trailing end. Beside a
//! sidebar, the sidebar's tracking separator comes first, so the items and
//! the title are over the content.
//!
//! An item that is one button is that button, in the toolbar's bezel
//! style: AppKit draws a button that is an item's view as it draws its own
//! items (tinted, lit under the pointer, the glass capsule its own on
//! macOS 26), and doesn't one inside another view. An item that is one
//! search field is an `NSSearchToolbarItem` with that field, which gets
//! its own glass; in another view, macOS 26 put it in the glass of the
//! button before it, and as the item's view in none.
//!
//! An item that is a row of buttons is one segmented control, as AppKit's
//! own item groups are: macOS 26 puts adjacent buttons with a title each
//! in a capsule of their own, and only image buttons in a shared one. An
//! item with no control in it (a progress bar, a label) has no glass: it
//! sits on the bar, as a status does.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use mitsuami_core::NodeId;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimationContext, NSBezelStyle, NSButton, NSLayoutConstraint, NSSearchField, NSSearchToolbarItem,
    NSSegmentDistribution, NSSegmentStyle, NSSegmentSwitchTracking, NSSegmentedControl, NSToolbar, NSToolbarDelegate,
    NSToolbarDisplayMode, NSToolbarFlexibleSpaceItemIdentifier, NSToolbarItem, NSToolbarItemIdentifier,
    NSToolbarSidebarTrackingSeparatorItemIdentifier, NSView, NSWindow,
};

use crate::classes::ClosureTarget;
use objc2_foundation::{NSArray, NSOperatingSystemVersion, NSProcessInfo, NSRect, NSSize, NSString};

pub(crate) struct DelegateIvars {
    /// The items the toolbar shows after its flexible space, in order.
    shown: RefCell<Vec<Retained<NSToolbarItem>>>,
    /// The window has a sidebar.
    sidebar: Cell<bool>,
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
        let this = ToolbarDelegate::alloc(mtm)
            .set_ivars(DelegateIvars { shown: RefCell::new(Vec::new()), sidebar: Cell::new(false) });
        unsafe { msg_send![super(this), init] }
    }

    /// The system's items ahead of ours: the sidebar's tracking separator,
    /// if there's a sidebar, then the flexible space.
    fn system_items(&self) -> Vec<Retained<NSToolbarItemIdentifier>> {
        let mut identifiers = Vec::new();
        if self.ivars().sidebar.get() {
            identifiers.push(unsafe { NSToolbarSidebarTrackingSeparatorItemIdentifier }.retain());
        }
        identifiers.push(unsafe { NSToolbarFlexibleSpaceItemIdentifier }.retain());
        identifiers
    }

    fn identifiers(&self) -> Retained<NSArray<NSToolbarItemIdentifier>> {
        let mut identifiers = self.system_items();
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

/// The buttons a group's segments click, in order.
type Segments = Rc<RefCell<Vec<Retained<NSButton>>>>;

/// A control shown as its item's view.
struct Adopted {
    view: Retained<NSView>,
    /// A button, and what it was like in its host.
    button: Option<(Retained<NSButton>, NSBezelStyle, bool)>,
    /// A search field is as wide as the core made it.
    width: Option<Retained<NSLayoutConstraint>>,
    /// The item a search item stands in for.
    replaced: Option<Retained<NSToolbarItem>>,
    /// A row of buttons, shown as the segments of `view`: the buttons a
    /// segment clicks, which stay in their host, and the control's target.
    group: Option<(Segments, Retained<ClosureTarget>)>,
}

struct Item {
    id: NodeId,
    item: Retained<NSToolbarItem>,
    /// The item's view while it shows its host.
    view: Retained<NSView>,
    host: Retained<NSView>,
    adopted: Option<Adopted>,
    width: Retained<NSLayoutConstraint>,
    height: Retained<NSLayoutConstraint>,
    /// The host's distance from the view's sides.
    insets: [Retained<NSLayoutConstraint>; 2],
    /// The host is in a capsule, as a view item is on macOS 26 until it's
    /// set borderless (though `bordered` reads false until then).
    glass: bool,
    /// Empty items are out of the toolbar: AppKit can hide an item only
    /// from macOS 15.
    empty: bool,
}

impl Item {
    /// Puts the item in a capsule or takes it out, and says if that
    /// changed. Before macOS 26 (`inset` 0) there's no capsule.
    fn set_glass(&mut self, glass: bool, inset: f64) -> bool {
        if self.glass == glass || inset == 0.0 {
            return false;
        }
        self.glass = glass;
        self.item.setBordered(glass);
        for constraint in &self.insets {
            constraint.setConstant(if glass { inset } else { 0.0 });
        }
        true
    }
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
        // Toolbars with the same identifier keep each other in sync, even
        // one a window let go of that isn't freed yet: one of its own.
        static MADE: AtomicU32 = AtomicU32::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let identifier = NSString::from_str(&format!("mitsuami.window.{}.{made}", id.raw()));
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
        let insets = [
            host.leadingAnchor().constraintEqualToAnchor_constant(&view.leadingAnchor(), self.inset),
            view.trailingAnchor().constraintEqualToAnchor_constant(&host.trailingAnchor(), self.inset),
        ];
        for constraint in [
            &width,
            &height,
            &insets[0],
            &insets[1],
            &host.topAnchor().constraintEqualToAnchor(&view.topAnchor()),
            &host.bottomAnchor().constraintEqualToAnchor(&view.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        item.setView(Some(&view));
        let host = host.retain();
        let at = index.min(self.items.len());
        self.items
            .insert(at, Item { id, item, view, host, adopted: None, width, height, insets, glass: true, empty: true });
        self.sync();
    }

    /// Shows an item as its only child, a button, in the toolbar's bezel
    /// style. `bordered` is how its own style has it, for when it's back
    /// in its host.
    pub(crate) fn adopt_button(&mut self, id: NodeId, button: &NSButton, bordered: bool) {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
        if let Some(Adopted { view, button: Some(kept), .. }) = &mut item.adopted
            && std::ptr::eq(&**view, &***button)
        {
            // Its props may have been set again since.
            kept.2 = bordered;
        } else {
            let bezel = button.bezelStyle();
            self.adopt(id, button, Some((button.retain(), bezel, bordered)));
        }
        button.setBordered(true);
        button.setBezelStyle(NSBezelStyle::Toolbar);
    }

    /// Shows an item's host, in a capsule only if it has a control in it.
    pub(crate) fn show_host(&mut self, id: NodeId, glass: bool) {
        self.release(id);
        let inset = self.inset;
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
        if item.set_glass(glass, inset) {
            self.resync();
        }
    }

    /// Shows an item whose only child is a row of buttons as one
    /// segmented control, a segment per button, in one capsule. A segment
    /// clicks its button, which stays in its host, out of the window.
    pub(crate) fn adopt_group(&mut self, mtm: MainThreadMarker, id: NodeId, buttons: Vec<Retained<NSButton>>) {
        let shown = self.items.iter().find(|i| i.id == id).and_then(|i| i.adopted.as_ref());
        let (control, kept, made) = match shown {
            Some(Adopted { view, group: Some((kept, _)), .. }) => {
                (view.downcast_ref::<NSSegmentedControl>().expect("a group's view").retain(), kept.clone(), false)
            }
            _ => {
                self.release(id);
                let control = NSSegmentedControl::new(mtm);
                control.setTrackingMode(NSSegmentSwitchTracking::Momentary);
                control.setSegmentStyle(NSSegmentStyle::Separated);
                control.setSegmentDistribution(NSSegmentDistribution::Fill);
                // As wide as the core lays the row out, its buttons side by
                // side; as tall as the toolbar makes it.
                control.setTranslatesAutoresizingMaskIntoConstraints(false);
                let kept: Segments = Rc::default();
                let clicked = kept.clone();
                let target = ClosureTarget::new(mtm, move |sender: &AnyObject| {
                    let Some(control) = sender.downcast_ref::<NSSegmentedControl>() else { return };
                    let button =
                        usize::try_from(control.selectedSegment()).ok().and_then(|i| clicked.borrow().get(i).cloned());
                    if let Some(button) = button {
                        unsafe { button.performClick(None) };
                    }
                });
                unsafe {
                    control.setTarget(Some(&target));
                    control.setAction(Some(sel!(fire:)));
                }
                let inset = self.inset;
                let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
                item.set_glass(true, inset);
                let sized = item.width.constant();
                let sized = if sized > 0.0 { sized } else { control.fittingSize().width.max(1.0) };
                let width = control.widthAnchor().constraintEqualToConstant(sized);
                width.setActive(true);
                item.item.setView(Some(&control));
                item.adopted = Some(Adopted {
                    view: control.clone().into_super().into_super(),
                    button: None,
                    width: Some(width),
                    replaced: None,
                    group: Some((kept.clone(), target)),
                });
                item.empty = false;
                (control, kept, true)
            }
        };
        let resized = control.segmentCount() != buttons.len() as isize;
        control.setSegmentCount(buttons.len() as isize);
        for (index, button) in buttons.iter().enumerate() {
            let segment = index as isize;
            control.setLabel_forSegment(&button.title(), segment);
            control.setImage_forSegment(button.image().as_deref(), segment);
            control.setEnabled_forSegment(button.isEnabled(), segment);
            control.setToolTip_forSegment(button.toolTip().as_deref(), segment);
        }
        *kept.borrow_mut() = buttons;
        // Only a new or resized control needs the toolbar to take it again:
        // placing takes every item out and back in, and the buttons' props
        // change often (a back button's enabled state).
        if made || resized {
            self.place(true);
        }
    }

    /// Shows an item as its only child, a search field, in a search item.
    pub(crate) fn adopt_field(&mut self, id: NodeId, field: &NSSearchField) {
        let shown = self.items.iter().find(|i| i.id == id).and_then(|i| i.adopted.as_ref());
        if shown.is_none_or(|a| !std::ptr::eq(&*a.view, &****field)) {
            self.adopt(id, field, None);
        }
    }

    fn adopt(&mut self, id: NodeId, view: &NSView, button: Option<(Retained<NSButton>, NSBezelStyle, bool)>) {
        self.release(id);
        let inset = self.inset;
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
        item.set_glass(true, inset);
        view.removeFromSuperview();
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
        // The toolbar sizes a button; a field has no width of its own.
        let width = button.is_none().then(|| {
            // Not empty until the core sizes it: the toolbar measures
            // the view as it takes it, and warns about an empty one.
            let sized = item.width.constant();
            let sized = if sized > 0.0 { sized } else { view.fittingSize().width.max(1.0) };
            let width = view.widthAnchor().constraintEqualToConstant(sized);
            width.setActive(true);
            width
        });
        let replaced = match view.downcast_ref::<NSSearchField>() {
            Some(field) => {
                let mtm = MainThreadMarker::from(field);
                let search = NSSearchToolbarItem::initWithItemIdentifier(
                    NSSearchToolbarItem::alloc(mtm),
                    &item.item.itemIdentifier(),
                );
                search.setSearchField(field);
                Some(std::mem::replace(&mut item.item, search.into_super()))
            }
            None => {
                item.item.setView(Some(view));
                None
            }
        };
        item.adopted = Some(Adopted { view: view.retain(), button, width, replaced, group: None });
        // Shown before the core sizes it: the toolbar says how big it is.
        item.empty = false;
        self.place(true);
    }

    /// Puts an item's control back in its host, as it was.
    pub(crate) fn release(&mut self, id: NodeId) {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
        let Some(Adopted { view, button, width, replaced, group }) = item.adopted.take() else { return };
        if group.is_some() {
            // The buttons never left their host.
            if let Some(width) = width {
                width.setActive(false);
            }
            item.item.setView(Some(&item.view));
            self.resync();
            return;
        }
        if let Some(replaced) = replaced {
            item.item = replaced;
        }
        item.item.setView(Some(&item.view));
        if let Some(width) = width {
            width.setActive(false);
        }
        view.setTranslatesAutoresizingMaskIntoConstraints(true);
        if let Some((button, bezel, bordered)) = button {
            button.setBezelStyle(bezel);
            button.setBordered(bordered);
        }
        item.host.addSubview(&view);
        self.resync();
    }

    /// The item shows its control as its view.
    pub(crate) fn adopted(&self, id: NodeId) -> bool {
        self.adopted_view(id).is_some()
    }

    /// The control an item shows as its view.
    pub(crate) fn adopted_view(&self, id: NodeId) -> Option<&NSView> {
        Some(&self.items.iter().find(|i| i.id == id)?.adopted.as_ref()?.view)
    }

    /// The toolbar makes it as wide as it likes: a button, not a field or
    /// a group.
    pub(crate) fn sizes_width(&self, id: NodeId) -> bool {
        self.items.iter().any(|i| i.id == id && i.adopted.as_ref().is_some_and(|a| a.button.is_some()))
    }

    /// The item shows a row of buttons as one control.
    pub(crate) fn is_group(&self, id: NodeId) -> bool {
        self.items.iter().any(|i| i.id == id && i.adopted.as_ref().is_some_and(|a| a.group.is_some()))
    }

    /// How big the toolbar made an item's control, once it's shown: in
    /// whole points, as the core lays out (the toolbar gave 39.5).
    pub(crate) fn adopted_size(&self, id: NodeId) -> Option<NSSize> {
        let view = self.adopted_view(id).filter(|v| v.window().is_some())?;
        let size = view.alignmentRectForFrame(view.bounds()).size;
        Some(NSSize::new(size.width.ceil(), size.height.ceil()))
    }

    pub(crate) fn remove(&mut self, id: NodeId) {
        let Some(index) = self.items.iter().position(|i| i.id == id) else { return };
        self.release(id);
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
        if let Some(Adopted { width: Some(adopted), .. }) = &item.adopted
            && width > 0.0
        {
            adopted.setConstant(width);
        }
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

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The window has a sidebar, or no longer has.
    pub(crate) fn set_sidebar(&self, sidebar: bool) {
        self.delegate.ivars().sidebar.set(sidebar);
        self.sync();
    }

    /// Where the toolbar shows an item, in the coordinates of the window's
    /// content view (above it, so at negative y); `None` while it's empty.
    pub(crate) fn frame(&self, id: NodeId, content: &NSView) -> Option<NSRect> {
        let item = self.items.iter().find(|i| i.id == id).filter(|i| !i.empty)?;
        let shown: &NSView = match &item.adopted {
            Some(adopted) => &adopted.view,
            None => &item.host,
        };
        shown.window()?;
        let mut frame = shown.convertRect_toView(shown.alignmentRectForFrame(shown.bounds()), Some(content));
        if let Some(size) = self.adopted_size(id) {
            frame.size = size;
        }
        Some(frame)
    }

    /// Has the toolbar take a shown item's new view: it keeps the one the
    /// item had when it was inserted.
    fn resync(&self) {
        if self.toolbar.items().len() > self.delegate.system_items().len() {
            self.place(true);
        }
    }

    /// Makes the toolbar show the non-empty items, in order: after the
    /// system's items, which are always first.
    fn sync(&self) {
        self.place(false);
    }

    fn place(&self, again: bool) {
        let wanted: Vec<Retained<NSToolbarItem>> =
            self.items.iter().filter(|i| !i.empty).map(|i| i.item.clone()).collect();
        let shown = self.toolbar.items();
        let system = self.delegate.system_items();
        let same = shown.len() == wanted.len() + system.len()
            && shown.iter().zip(&system).all(|(a, b)| a.itemIdentifier().isEqualToString(b))
            && shown.iter().skip(system.len()).zip(&wanted).all(|(a, b)| std::ptr::eq(&*a, &**b));
        if same && !again {
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
