//! A window's sidebar: a view-based `NSTableView` in the source list
//! style, in the sidebar item of an `NSSplitViewController` whose other
//! item holds the window's content, as in System Settings and Finder.
//!
//! The sidebar node owns the table ([`Sidebar`]); the window makes the
//! split when the sidebar is inserted ([`Split`]), with the window's
//! content under the title bar, as the sidebar is full height.
//!
//! Like a list's, the table's data source reads only the sidebar's own
//! [`SidebarData`], and only emits. It's also the table's menu's delegate,
//! which fills the menu with the right-clicked item's (Finder's sidebar
//! rings that row while it's up), and its double-click target.

use std::cell::{Cell, OnceCell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

use mitsuami_core::a11y::ActionError;
use mitsuami_core::services::menu_item_by_id;
use mitsuami_core::{EventSink, EventValue, NodeId, SidebarItemData, SidebarSectionData, UiEvent};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSControlTextEditingDelegate, NSFont, NSFontTextStyleSubheadline, NSImage, NSImageScaling, NSImageView,
    NSLayoutConstraint, NSLineBreakMode, NSMenu, NSMenuDelegate, NSMenuItem, NSScrollView, NSSplitViewController,
    NSSplitViewItem, NSTableCellView, NSTableColumn, NSTableColumnResizingOptions, NSTableView,
    NSTableViewColumnAutoresizingStyle, NSTableViewDataSource, NSTableViewDelegate, NSTableViewRowSizeStyle,
    NSTableViewStyle, NSTextField, NSView, NSViewController, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSArray, NSIndexSet, NSInteger, NSKeyValueObservingOptions, NSNotification, NSObjectNSKeyValueObserverRegistration,
    NSString,
};

use crate::classes::zero_rect;
use crate::services::{ItemTarget, app_items, context_menu};

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
    /// The table, for the row a right-click or a double-click was on.
    table: OnceCell<Weak<NSTableView>>,
}

impl SourceIvars {
    /// The item of the row the user last clicked (a right-click, a
    /// double-click), if it's an item.
    fn clicked(&self) -> Option<usize> {
        let table = self.table.get()?.load()?;
        self.data.borrow().item_at(table.clickedRow())
    }
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
                Row::Heading(section) => {
                    cell(self.mtm(), data.sections[section].title.as_deref().unwrap_or(""), None, None)
                }
                Row::Item(item) => {
                    let item = &data.items[item];
                    cell(self.mtm(), &item.title, item.icon.as_deref(), item.subtitle.as_deref())
                }
            });
            cell.map(Retained::into_super)
        }

        /// An item with a subtitle is as tall as its two lines, the system's
        /// row height (which follows the user's sidebar icon size) with the
        /// subtitle's line added; the rest are the system's.
        #[unsafe(method(tableView:heightOfRow:))]
        fn height_of_row(&self, table: &NSTableView, row: NSInteger) -> f64 {
            let data = self.ivars().data.borrow();
            let two_lines = data.item_at(row).is_some_and(|i| data.items[i].subtitle.is_some());
            let base = table.rowHeight();
            if two_lines { base + line_height(&subtitle_font()) } else { base }
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
            let SourceIvars { id, events, data, .. } = self.ivars();
            let data = data.borrow();
            if let (false, Some(item)) = (data.muted, data.item_at(table.selectedRow())) {
                events.emit(*id, UiEvent::Changed(EventValue::Index(item)));
            }
        }
    }

    unsafe impl NSMenuDelegate for SidebarSource {
        /// The right-clicked item's menu, built as AppKit opens it. A row
        /// without one (a heading, an item without a menu) gets none, and
        /// AppKit shows nothing.
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            menu.removeAllItems();
            let Some(item) = self.ivars().clicked() else { return };
            let entries = self.ivars().data.borrow().items[item].menu.clone();
            let target = ItemTarget { object: self, action: sel!(chose:) };
            for entry in app_items(self.mtm(), &entries, target) {
                menu.addItem(&entry);
            }
        }
    }

    impl SidebarSource {
        /// An item of an item's menu was chosen.
        #[unsafe(method(chose:))]
        fn chose(&self, sender: &NSMenuItem) {
            let SourceIvars { id, events, .. } = self.ivars();
            events.emit(*id, UiEvent::ContextMenuItem(sender.tag() as u32));
        }

        /// The table's double action: a double-click on an item activates
        /// it (it was chosen by the first click).
        #[unsafe(method(activated:))]
        fn activated(&self, _sender: &NSTableView) {
            if let Some(item) = self.ivars().clicked() {
                let SourceIvars { id, events, .. } = self.ivars();
                events.emit(*id, UiEvent::SidebarItemActivated(item));
            }
        }
    }
);

/// How wide a row's icon is: a slot of its own, so titles line up whatever
/// their symbols' widths, as in Finder's sidebar.
const ICON_WIDTH: f64 = 20.0;

/// A subtitle's font: the subheadline style, smaller than the title, as
/// Mail's and Messages' second lines are.
fn subtitle_font() -> Retained<NSFont> {
    unsafe {
        NSFont::preferredFontForTextStyle_options(NSFontTextStyleSubheadline, &objc2_foundation::NSDictionary::new())
    }
}

fn line_height(font: &NSFont) -> f64 {
    (font.ascender() - font.descender() + font.leading()).ceil()
}

/// A row's view: its icon and title, which the source list sizes and
/// styles for the row size the user chose (System Settings' "Sidebar icon
/// size"), as Interface Builder's image and text cell; an item's subtitle
/// under the title, in the secondary label colour.
fn cell(mtm: MainThreadMarker, title: &str, icon: Option<&str>, subtitle: Option<&str>) -> Retained<NSTableCellView> {
    let cell = NSTableCellView::new(mtm);
    let label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setTranslatesAutoresizingMaskIntoConstraints(false);
    cell.addSubview(&label);
    unsafe { cell.setTextField(Some(&label)) };
    let second = subtitle.map(|text| {
        let line = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        line.setFont(Some(&subtitle_font()));
        line.setTextColor(Some(&NSColor::secondaryLabelColor()));
        line.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        line.setTranslatesAutoresizingMaskIntoConstraints(false);
        cell.addSubview(&line);
        line
    });
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
    let mut constraints =
        vec![leading, label.trailingAnchor().constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -2.0)];
    match &second {
        // The two lines centred together, the subtitle under the title.
        Some(line) => constraints.extend([
            label.bottomAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
            line.topAnchor().constraintEqualToAnchor(&label.bottomAnchor()),
            line.leadingAnchor().constraintEqualToAnchor(&label.leadingAnchor()),
            line.trailingAnchor().constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -2.0),
        ]),
        None => constraints.push(label.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor())),
    }
    NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&constraints));
    cell
}

/// The sidebar node's native parts: the table in its scroll view, which
/// becomes the split's sidebar when the window takes it.
pub(crate) struct Sidebar {
    pub scroll: Retained<NSScrollView>,
    pub table: Retained<NSTableView>,
    _source: Retained<SidebarSource>,
    data: Shared,
    /// Shown as the app wants it, if it said: the split shows it.
    pub shown: Cell<Option<bool>>,
}

impl Sidebar {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> Sidebar {
        let data = Shared::default();
        let source: Retained<SidebarSource> = unsafe {
            let ivars = SourceIvars { id, events, data: data.clone(), table: OnceCell::new() };
            msg_send![super(SidebarSource::alloc(mtm).set_ivars(ivars)), init]
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
            table.setTarget(Some(&source));
            table.setDoubleAction(Some(sel!(activated:)));
        }
        let _ = source.ivars().table.set(Weak::from_retained(&table));
        // Filled with the right-clicked item's menu as it opens.
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        unsafe { table.setMenu(Some(&menu)) };
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), zero_rect());
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(&table));
        Sidebar { scroll, table, _source: source, data, shown: Cell::new(None) }
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

    /// Stops the table from calling its data source, which goes with the
    /// sidebar, while the split may still hold the table.
    pub(crate) fn detach(&self) {
        unsafe {
            self.table.setDataSource(None);
            self.table.setDelegate(None);
            self.table.setTarget(None);
            if let Some(menu) = self.table.menu() {
                menu.setDelegate(None);
            }
        }
    }

    /// Chooses the item with this id in an item's menu, as the menu does:
    /// what a screen reader does once it has opened it.
    pub(crate) fn choose_menu_item(&self, id: u32) -> Result<(), ActionError> {
        let entries = {
            let data = self.data.borrow();
            let menu = data.items.iter().map(|i| &i.menu).find(|m| menu_item_by_id(m, id).is_some());
            menu.cloned().ok_or(ActionError::Unsupported)?
        };
        let target = ItemTarget { object: &self._source, action: sel!(chose:) };
        let menu = context_menu(self.table.mtm(), &entries, target);
        let (item, menu) = crate::services::find_tagged(&menu, id).ok_or(ActionError::Unsupported)?;
        if !item.isEnabled() {
            return Err(ActionError::Disabled);
        }
        menu.performActionForItemAtIndex(menu.indexOfItem(&item));
        Ok(())
    }

    /// Activates the chosen item, as a double-click on it does. `false` if
    /// none is chosen.
    pub(crate) fn activate(&self) -> bool {
        let Some(item) = self.selected() else { return false };
        let SourceIvars { id, events, .. } = self._source.ivars();
        events.emit(*id, UiEvent::SidebarItemActivated(item));
        true
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

pub(crate) struct WatchIvars {
    sidebar: NodeId,
    events: EventSink,
    /// Collapsed as last reported or set, to tell the user's changes from
    /// the backend's.
    collapsed: Cell<bool>,
}

define_class!(
    /// Watches the sidebar item's `collapsed` (KVO): the user collapsed or
    /// expanded it (its divider, the toolbar's toggle, View ▸ Hide
    /// Sidebar), or AppKit did for a narrow window.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = WatchIvars]
    pub(crate) struct CollapseWatch;

    impl CollapseWatch {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(
            &self,
            _key_path: Option<&NSString>,
            object: Option<&AnyObject>,
            _change: Option<&AnyObject>,
            _context: *mut c_void,
        ) {
            let Some(item) = object.and_then(|o| o.downcast_ref::<NSSplitViewItem>()) else { return };
            let WatchIvars { sidebar, events, collapsed } = self.ivars();
            let now = item.isCollapsed();
            if collapsed.replace(now) != now {
                events.emit(*sidebar, UiEvent::SidebarShownChanged(!now));
            }
        }
    }

    unsafe impl NSObjectProtocol for CollapseWatch {}
);

const COLLAPSED: &str = "collapsed";

/// A window split in two: the sidebar, and a view holding the window's
/// content (its host), below the title bar and toolbar.
pub(crate) struct Split {
    pub sidebar: NodeId,
    controller: Retained<NSSplitViewController>,
    item: Retained<NSSplitViewItem>,
    watch: Retained<CollapseWatch>,
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
        events: EventSink,
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
        let watch = CollapseWatch::alloc(mtm).set_ivars(WatchIvars {
            sidebar,
            events,
            collapsed: Cell::new(item.isCollapsed()),
        });
        let watch: Retained<CollapseWatch> = unsafe { msg_send![super(watch), init] };
        unsafe {
            item.addObserver_forKeyPath_options_context(
                &watch,
                &NSString::from_str(COLLAPSED),
                NSKeyValueObservingOptions::New,
                std::ptr::null_mut(),
            );
        }
        Split { sidebar, controller, item, watch }
    }

    /// Shows or hides the sidebar without reporting it, as the toolbar's
    /// toggle does (`toggleSidebar:`), animated in a shown window.
    pub(crate) fn set_shown(&self, shown: bool) {
        self.watch.ivars().collapsed.set(!shown);
        if self.item.isCollapsed() == shown {
            self.item.setCollapsed(!shown);
        }
    }

    /// Stops watching the item: KVO doesn't retain the watch, which goes
    /// with the split while the item may live on in the window.
    pub(crate) fn detach(&self) {
        unsafe { self.item.removeObserver_forKeyPath(&self.watch, &NSString::from_str(COLLAPSED)) };
    }

    /// Gives the window its content back, as it was before the split.
    pub(crate) fn remove(self, window: &NSWindow, host: &NSView) {
        self.detach();
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
