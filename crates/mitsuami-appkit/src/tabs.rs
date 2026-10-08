//! A tab view: an `NSTabView` with its tabs on top, each page host in a
//! plain view of its own as its `NSTabViewItem`'s view.
//!
//! The tab view sizes its items' views to its page area; the wrapper takes
//! that size, and the host inside it keeps the one the core gives it, at
//! the wrapper's top-left. Only the shown item's view is in the tab view:
//! AppKit takes the others out, and keeps them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{EventSink, EventValue, Insets, NodeId, Rect, Size, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSTabView, NSTabViewDelegate, NSTabViewItem, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::classes::{HostView, zero_rect};

pub(crate) struct DelegateIvars {
    id: NodeId,
    events: EventSink,
    /// Set while the backend changes the tab view itself.
    muted: Rc<Cell<bool>>,
}

define_class!(
    /// Reports the tab the user picks.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    pub(crate) struct TabsDelegate;

    unsafe impl NSObjectProtocol for TabsDelegate {}

    unsafe impl NSTabViewDelegate for TabsDelegate {
        /// Also called when the backend selects an item, or adds or takes
        /// one away: only the user's choice is reported.
        #[unsafe(method(tabView:didSelectTabViewItem:))]
        fn did_select(&self, tab_view: &NSTabView, item: Option<&NSTabViewItem>) {
            let DelegateIvars { id, events, muted } = self.ivars();
            if muted.get() {
                return;
            }
            if let Some(item) = item {
                let index = tab_view.indexOfTabViewItem(item);
                if index >= 0 {
                    events.emit(*id, UiEvent::Changed(EventValue::Index(index as usize)));
                }
            }
        }
    }
);

/// One page: its host, and the view the tab view sizes around it.
struct Page {
    id: NodeId,
    host: Retained<NSView>,
    wrapper: Retained<HostView>,
    item: Retained<NSTabViewItem>,
}

pub(crate) struct Tabs {
    pub(crate) view: Retained<NSTabView>,
    _delegate: Retained<TabsDelegate>,
    muted: Rc<Cell<bool>>,
    pages: RefCell<Vec<Page>>,
    titles: RefCell<Vec<String>>,
    /// The page the core shows, which AppKit may change as pages come and
    /// go: it's shown again after.
    shown: Cell<Option<usize>>,
}

impl Tabs {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> Tabs {
        let view = NSTabView::initWithFrame(NSTabView::alloc(mtm), zero_rect());
        let muted = Rc::new(Cell::new(false));
        let delegate = TabsDelegate::alloc(mtm).set_ivars(DelegateIvars { id, events, muted: muted.clone() });
        let delegate: Retained<TabsDelegate> = unsafe { msg_send![super(delegate), init] };
        // The node keeps the delegate alive as long as the view.
        view.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Tabs {
            view,
            _delegate: delegate,
            muted,
            pages: RefCell::default(),
            titles: RefCell::default(),
            shown: Cell::new(None),
        }
    }

    /// Runs a change the user didn't make, without reporting it.
    fn quietly(&self, change: impl FnOnce()) {
        let was = self.muted.replace(true);
        change();
        self.muted.set(was);
    }

    pub(crate) fn insert(&self, mtm: MainThreadMarker, id: NodeId, host: Retained<NSView>, index: usize) {
        let wrapper = HostView::new(mtm);
        wrapper.addSubview(&host);
        let item = NSTabViewItem::new();
        item.setView(Some(&wrapper));
        item.setLabel(&NSString::from_str(self.titles.borrow().get(index).map_or("", |t| t.as_str())));
        self.quietly(|| self.view.insertTabViewItem_atIndex(&item, index as isize));
        self.pages.borrow_mut().insert(index, Page { id, host, wrapper, item });
        self.show();
    }

    pub(crate) fn remove(&self, id: NodeId) {
        let Some(index) = self.pages.borrow().iter().position(|p| p.id == id) else { return };
        let page = self.pages.borrow_mut().remove(index);
        self.quietly(|| self.view.removeTabViewItem(&page.item));
        page.host.removeFromSuperview();
        self.show();
    }

    /// A page's host gets the core's size, at its wrapper's top-left.
    pub(crate) fn set_page_size(&self, id: NodeId, size: NSSize) {
        if let Some(page) = self.pages.borrow().iter().find(|p| p.id == id) {
            page.host.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), size));
        }
    }

    pub(crate) fn set_titles(&self, titles: Vec<String>) {
        for (page, title) in self.pages.borrow().iter().zip(&titles) {
            page.item.setLabel(&NSString::from_str(title));
        }
        *self.titles.borrow_mut() = titles;
    }

    pub(crate) fn set_shown(&self, index: Option<usize>) {
        self.shown.set(index);
        self.show();
    }

    fn show(&self) {
        let count = self.pages.borrow().len();
        if let Some(index) = self.shown.get().filter(|i| *i < count) {
            self.quietly(|| self.view.selectTabViewItemAtIndex(index as isize));
        }
    }

    pub(crate) fn titles(&self) -> Vec<String> {
        self.pages.borrow().iter().map(|p| p.item.label().to_string()).collect()
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        let item = self.view.selectedTabViewItem()?;
        usize::try_from(self.view.indexOfTabViewItem(&item)).ok()
    }

    pub(crate) fn ids(&self) -> Vec<NodeId> {
        self.pages.borrow().iter().map(|p| p.id).collect()
    }

    /// Where the tab view shows a page, from its alignment rect's top-left
    /// (where the core placed it); nowhere if it isn't the one shown.
    pub(crate) fn page_frame(&self, id: NodeId) -> Option<Rect> {
        let pages = self.pages.borrow();
        let page = pages.iter().find(|p| p.id == id)?;
        let shown = unsafe { page.wrapper.superview() }.is_some_and(|s| std::ptr::eq(&*s, &**self.view as &NSView));
        if !shown {
            return Some(Rect::ZERO);
        }
        let align = self.view.alignmentRectInsets();
        let origin = page.wrapper.frame().origin;
        let size = page.host.frame().size;
        Some(Rect::new(
            (origin.x - align.left) as f32,
            (origin.y - align.top) as f32,
            size.width as f32,
            size.height as f32,
        ))
    }

    /// Picks a tab by its title, as a click on it would: the delegate
    /// reports it. Whether there's one.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let index = self.pages.borrow().iter().position(|p| p.item.label().to_string() == title);
        match index {
            Some(index) => {
                self.view.selectTabViewItemAtIndex(index as isize);
                true
            }
            None => false,
        }
    }

    /// Its tabs and border, with no page: as narrow as AppKit lets it be,
    /// which truncates the tabs' titles to fit.
    pub(crate) fn natural_size(&self, insets: Insets) -> Size {
        let min = self.view.minimumSize();
        let align = self.view.alignmentRectInsets();
        let width = (min.width - align.left - align.right).max((insets.left + insets.right) as f64);
        Size::new(width.ceil() as f32, insets.top + insets.bottom)
    }
}

/// Where a tab view with tabs on top shows its pages, from its alignment
/// rect's edges: measured on one, as AppKit doesn't say.
pub(crate) fn insets(mtm: MainThreadMarker) -> Insets {
    let view =
        NSTabView::initWithFrame(NSTabView::alloc(mtm), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 300.0)));
    let item = NSTabViewItem::new();
    item.setLabel(&NSString::from_str("Tab"));
    view.addTabViewItem(&item);
    let (bounds, content, align) = (view.bounds(), view.contentRect(), view.alignmentRectInsets());
    // Flipped: y grows down.
    Insets::new(
        (content.origin.y - bounds.origin.y - align.top) as f32,
        (bounds.origin.x + bounds.size.width - content.origin.x - content.size.width - align.right) as f32,
        (bounds.origin.y + bounds.size.height - content.origin.y - content.size.height - align.bottom) as f32,
        (content.origin.x - bounds.origin.x - align.left) as f32,
    )
}
