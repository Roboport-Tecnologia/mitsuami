//! A window's sidebar: a view-based `NSTableView` in the source list
//! style, in the sidebar item of an `NSSplitViewController` whose other
//! item holds the window's content, as in System Settings and Finder.
//!
//! The sidebar node owns the table ([`Sidebar`]); the window makes the
//! split when the sidebar is inserted ([`Split`]), with the window's
//! content under the title bar, as the sidebar is full height.
//!
//! Like a list's, the table's data source reads only the sidebar's own
//! [`SidebarData`], and only emits.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::{EventSink, EventValue, NodeId, SidebarItemData, SidebarSectionData, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSControlTextEditingDelegate, NSImage, NSImageScaling, NSImageView, NSLayoutConstraint, NSLineBreakMode,
    NSScrollView, NSSplitViewController, NSSplitViewItem, NSTableCellView, NSTableColumn, NSTableColumnResizingOptions,
    NSTableView, NSTableViewColumnAutoresizingStyle, NSTableViewDataSource, NSTableViewDelegate,
    NSTableViewRowSizeStyle, NSTableViewStyle, NSTextField, NSView, NSViewController, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSIndexSet, NSInteger, NSNotification, NSString};

use crate::classes::zero_rect;

/// A row of the table: a section's heading, or an item (its index across
/// the sections).
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Heading(usize),
    Item(usize),
}

/// What the table shows, shared by the backend and the table's data source.
#[derive(Default)]
pub(crate) struct SidebarData {
    sections: Vec<SidebarSectionData>,
    items: Vec<SidebarItemData>,
    rows: Vec<Row>,
    /// Set while the backend changes the selection itself.
    muted: bool,
}

impl SidebarData {
    fn row_of(&self, item: usize) -> Option<usize> {
        self.rows.iter().position(|r| *r == Row::Item(item))
    }

    fn item_at(&self, row: NSInteger) -> Option<usize> {
        match self.rows.get(usize::try_from(row).ok()?)? {
            Row::Item(item) => Some(*item),
            Row::Heading(_) => None,
        }
    }
}

type Shared = Rc<RefCell<SidebarData>>;

pub(crate) struct SourceIvars {
    id: NodeId,
    events: EventSink,
    data: Shared,
}

define_class!(
    /// The table's data source and delegate.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SourceIvars]
    pub(crate) struct SidebarSource;

    unsafe impl NSObjectProtocol for SidebarSource {}

    unsafe impl NSControlTextEditingDelegate for SidebarSource {}

    unsafe impl NSTableViewDataSource for SidebarSource {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn number_of_rows(&self, _table: &NSTableView) -> NSInteger {
            self.ivars().data.borrow().rows.len() as NSInteger
        }
    }

    unsafe impl NSTableViewDelegate for SidebarSource {
        /// A section's heading, which a source list draws as one.
        #[unsafe(method(tableView:isGroupRow:))]
        fn is_group_row(&self, _table: &NSTableView, row: NSInteger) -> bool {
            let data = self.ivars().data.borrow();
            matches!(data.rows.get(row as usize), Some(Row::Heading(_)))
        }

        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            _column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<NSView>> {
            let data = self.ivars().data.borrow();
            let cell = data.rows.get(row as usize).map(|row| match *row {
                Row::Heading(section) => cell(self.mtm(), data.sections[section].title.as_deref().unwrap_or(""), None),
                Row::Item(item) => cell(self.mtm(), &data.items[item].title, data.items[item].icon.as_deref()),
            });
            cell.map(Retained::into_super)
        }

        #[unsafe(method(tableView:shouldSelectRow:))]
        fn should_select_row(&self, _table: &NSTableView, row: NSInteger) -> bool {
            self.ivars().data.borrow().item_at(row).is_some()
        }

        /// A sidebar keeps its selection: a click on empty space doesn't
        /// take it away, as none does in Finder's or System Settings'.
        #[unsafe(method_id(tableView:selectionIndexesForProposedSelection:))]
        fn selection_for_proposed(&self, table: &NSTableView, proposed: &NSIndexSet) -> Retained<NSIndexSet> {
            if proposed.count() == 0 && !self.ivars().data.borrow().muted {
                table.selectedRowIndexes()
            } else {
                proposed.retain()
            }
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_did_change(&self, notification: &NSNotification) {
            let Some(table) = notification.object().and_then(|o| o.downcast::<NSTableView>().ok()) else { return };
            let SourceIvars { id, events, data } = self.ivars();
            let data = data.borrow();
            if let (false, Some(item)) = (data.muted, data.item_at(table.selectedRow())) {
                events.emit(*id, UiEvent::Changed(EventValue::Index(item)));
            }
        }
    }
);

/// How wide a row's icon is: a slot of its own, so titles line up whatever
/// their symbols' widths, as in Finder's sidebar.
const ICON_WIDTH: f64 = 20.0;

/// A row's view: its icon and title, which the source list sizes and
/// styles for the row size the user chose (System Settings' "Sidebar icon
/// size"), as Interface Builder's image and text cell.
fn cell(mtm: MainThreadMarker, title: &str, icon: Option<&str>) -> Retained<NSTableCellView> {
    let cell = NSTableCellView::new(mtm);
    let label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setTranslatesAutoresizingMaskIntoConstraints(false);
    cell.addSubview(&label);
    unsafe { cell.setTextField(Some(&label)) };
    let image = icon
        .and_then(|name| NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None));
    let leading = match image {
        Some(image) => {
            let view = NSImageView::imageViewWithImage(&image, mtm);
            // Centred in its slot at its own size, not scaled to it.
            view.setImageScaling(NSImageScaling::ScaleNone);
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
            cell.addSubview(&view);
            unsafe { cell.setImageView(Some(&view)) };
            NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&[
                view.leadingAnchor().constraintEqualToAnchor_constant(&cell.leadingAnchor(), 2.0),
                view.widthAnchor().constraintEqualToConstant(ICON_WIDTH),
                view.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
            ]));
            view.trailingAnchor().constraintEqualToAnchor_constant(&label.leadingAnchor(), -6.0)
        }
        None => label.leadingAnchor().constraintEqualToAnchor_constant(&cell.leadingAnchor(), 2.0),
    };
    NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&[
        leading,
        label.trailingAnchor().constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -2.0),
        label.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ]));
    cell
}

/// The sidebar node's native parts: the table in its scroll view, which
/// becomes the split's sidebar when the window takes it.
pub(crate) struct Sidebar {
    pub scroll: Retained<NSScrollView>,
    pub table: Retained<NSTableView>,
    _source: Retained<SidebarSource>,
    data: Shared,
}

impl Sidebar {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> Sidebar {
        let data = Shared::default();
        let source: Retained<SidebarSource> = unsafe {
            msg_send![super(SidebarSource::alloc(mtm).set_ivars(SourceIvars { id, events, data: data.clone() })), init]
        };
        let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), zero_rect());
        table.setHeaderView(None);
        table.setStyle(NSTableViewStyle::SourceList);
        // The system's row size, which follows the user's sidebar icon size.
        table.setRowSizeStyle(NSTableViewRowSizeStyle::Default);
        table.setFloatsGroupRows(false);
        table.setAllowsEmptySelection(true);
        table.setAllowsMultipleSelection(false);
        table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &NSString::from_str("item"));
        column.setResizingMask(NSTableColumnResizingOptions::AutoresizingMask);
        table.addTableColumn(&column);
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            table.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), zero_rect());
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(&table));
        Sidebar { scroll, table, _source: source, data }
    }

    /// New items: the table reloads, and keeps the selected item, which
    /// the core sends again if it moved.
    pub(crate) fn set_sections(&self, sections: Vec<SidebarSectionData>) {
        let selected = self.selected();
        {
            let mut data = self.data.borrow_mut();
            data.items = sections.iter().flat_map(|s| s.items.iter().cloned()).collect();
            data.rows.clear();
            let mut item = 0;
            for (index, section) in sections.iter().enumerate() {
                if section.title.is_some() {
                    data.rows.push(Row::Heading(index));
                }
                for _ in &section.items {
                    data.rows.push(Row::Item(item));
                    item += 1;
                }
            }
            data.sections = sections;
            data.muted = true;
        }
        self.table.reloadData();
        self.show_selected(selected.filter(|i| *i < self.data.borrow().items.len()));
        self.data.borrow_mut().muted = false;
    }

    pub(crate) fn sections(&self) -> Vec<SidebarSectionData> {
        self.data.borrow().sections.clone()
    }

    /// Selects an item without reporting it.
    pub(crate) fn set_selected(&self, item: Option<usize>) {
        self.data.borrow_mut().muted = true;
        self.show_selected(item);
        self.data.borrow_mut().muted = false;
    }

    fn show_selected(&self, item: Option<usize>) {
        let row = item.and_then(|i| self.data.borrow().row_of(i));
        match row {
            Some(row) => {
                self.table.selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row), false);
                self.table.scrollRowToVisible(row as NSInteger);
            }
            None => unsafe { self.table.deselectAll(None) },
        }
    }

    /// The item the table shows selected.
    pub(crate) fn selected(&self) -> Option<usize> {
        self.data.borrow().item_at(self.table.selectedRow())
    }

    /// Selects the first item with this title as the user would, which the
    /// delegate reports: what a screen reader's select does. `false` if
    /// there's none.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let row = {
            let data = self.data.borrow();
            let item = data.items.iter().position(|i| i.title == title);
            item.and_then(|i| data.row_of(i))
        };
        let Some(row) = row else { return false };
        self.table.selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row), false);
        true
    }
}

/// A window split in two: the sidebar, and a view holding the window's
/// content (its host), below the title bar and toolbar.
pub(crate) struct Split {
    pub sidebar: NodeId,
    controller: Retained<NSSplitViewController>,
    item: Retained<NSSplitViewItem>,
}

impl Split {
    /// Puts the window's content beside the sidebar. The sidebar is full
    /// height, under the title bar, as on macOS 11 and later; the content
    /// stays below it.
    pub(crate) fn new(
        mtm: MainThreadMarker,
        window: &NSWindow,
        host: &NSView,
        sidebar: NodeId,
        scroll: &NSScrollView,
    ) -> Split {
        let side = NSViewController::new(mtm);
        side.setView(scroll);
        let item = NSSplitViewItem::sidebarWithViewController(&side);
        let detail = NSView::new(mtm);
        let content = NSViewController::new(mtm);
        content.setView(&detail);
        let controller = NSSplitViewController::new(mtm);
        controller.addSplitViewItem(&item);
        controller.addSplitViewItem(&NSSplitViewItem::splitViewItemWithViewController(&content));
        let frame = window.contentView().map_or(zero_rect(), |v| v.frame());
        controller.view().setFrame(frame);
        window.setStyleMask(window.styleMask() | NSWindowStyleMask::FullSizeContentView);
        window.setContentViewController(Some(&controller));
        host.removeFromSuperview();
        host.setTranslatesAutoresizingMaskIntoConstraints(false);
        detail.addSubview(host);
        NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&[
            host.topAnchor().constraintEqualToAnchor(&detail.safeAreaLayoutGuide().topAnchor()),
            host.leadingAnchor().constraintEqualToAnchor(&detail.leadingAnchor()),
            host.trailingAnchor().constraintEqualToAnchor(&detail.trailingAnchor()),
            host.bottomAnchor().constraintEqualToAnchor(&detail.bottomAnchor()),
        ]));
        Split { sidebar, controller, item }
    }

    /// Gives the window its content back, as it was before the split.
    pub(crate) fn remove(self, window: &NSWindow, host: &NSView) {
        window.setContentViewController(None);
        host.removeFromSuperview();
        host.setTranslatesAutoresizingMaskIntoConstraints(true);
        window.setStyleMask(window.styleMask() & !NSWindowStyleMask::FullSizeContentView);
        window.setContentView(Some(host));
        drop(self.controller);
    }

    /// Collapsed: the user dragged it away, or the window is too narrow.
    pub(crate) fn collapsed(&self) -> bool {
        self.item.isCollapsed()
    }
}
