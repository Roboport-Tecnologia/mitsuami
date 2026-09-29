//! `List` and `Table`: a view-based `NSTableView` in an `NSScrollView`,
//! with one column and no header for a list, and the app's columns under
//! a header for a table.
//!
//! The table virtualises: it adds row views for the rows it shows and
//! removes them when they scroll away, and we report both (`RowShown`,
//! `RowHidden`) so the core mounts and disposes those rows. Each cell is a
//! plain view that holds its host once the core sends it: a list's row
//! host, or a table's cell host, centred in its row's height.
//!
//! The table calls its data source and delegate whenever it likes,
//! including in the middle of our own `apply` (a reload, a scroll, a
//! resize), so they read only the list's own [`ListData`], never the
//! backend's state, and only emit.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mitsuami_core::{
    ColumnData, ColumnSort, EventSink, EventValue, ListStyle, NodeId, Rect, RowKey, SelectionMode, SortOrder, UiEvent,
};

use mitsuami_core::services::Shortcut;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimationContext, NSBorderType, NSControlTextEditingDelegate, NSEvent, NSEventModifierFlags, NSImage, NSMenu,
    NSResponder, NSScrollView, NSTableColumn, NSTableColumnResizingOptions, NSTableRowView, NSTableView,
    NSTableViewColumnAutoresizingStyle, NSTableViewDataSource, NSTableViewDelegate, NSTableViewStyle, NSView,
};
use objc2_foundation::{
    NSArray, NSIndexSet, NSInteger, NSMutableIndexSet, NSNotFound, NSNotification, NSSize, NSSortDescriptor, NSString,
};

use crate::classes::{HostView, zero_rect};

/// A cell: its row, and its column (a list's is 0).
type Slot = (RowKey, usize);

/// What the table shows, shared by the backend and the table's data source.
#[derive(Default)]
pub(crate) struct ListData {
    /// A table: columns under a header, and cells centred in rows at least
    /// the table's row height.
    table: bool,
    rows: Vec<RowKey>,
    index: HashMap<RowKey, usize>,
    /// The rows' heights: a list's from its hosts' sizes, a table's from
    /// its highest cell's and its row height.
    heights: HashMap<RowKey, f64>,
    /// Tables only: their cells' hosts' heights.
    cell_heights: HashMap<Slot, f64>,
    estimate: Option<f64>,
    /// Without the app's estimate: the first row measured.
    learned: Option<f64>,
    /// The mounted rows' hosts: a list's row hosts, a table's cell hosts.
    hosts: HashMap<Slot, (NodeId, Retained<NSView>)>,
    /// The cells of the rows the table shows.
    cells: HashMap<Slot, Retained<HostView>>,
    /// Tables only: the columns as the app sent them, and the widths last
    /// reported.
    columns: Vec<ColumnData>,
    widths: Vec<f32>,
    /// The table's row views (by address), and the rows they show.
    row_views: HashMap<usize, RowKey>,
    /// The rows reported shown.
    shown: HashSet<RowKey>,
    mode: SelectionMode,
    /// Set while the backend changes the selection itself.
    muted: bool,
    /// Set while the table reloads: rows come and go, and only the
    /// difference is reported at the end.
    reloading: bool,
    /// The keys the app gave it (`Prop::Keys`), if any.
    keys: Option<Vec<Shortcut>>,
}

impl ListData {
    /// The keys of the rows at these indexes.
    fn keys(&self, indexes: &NSIndexSet) -> Vec<RowKey> {
        indices(indexes).into_iter().filter_map(|i| self.rows.get(i).copied()).collect()
    }

    /// How high rows not shown yet are: the app's estimate, or else the
    /// first row measured, or else a line of text and padding. It stays
    /// put once known: the table keeps the heights it read.
    fn estimate(&self) -> f64 {
        self.estimate.or(self.learned).unwrap_or(24.0)
    }

    /// The rows' heights, as sent: a list's host's, or a table's highest
    /// cell's, but at least its row height.
    fn row_height(&self, table: &NSTableView, key: RowKey) -> Option<f64> {
        if !self.table {
            return self.heights.get(&key).copied();
        }
        let cells = (0..self.columns.len()).filter_map(|column| self.cell_heights.get(&(key, column)).copied());
        cells.reduce(f64::max).map(|h| h.max(table.rowHeight()))
    }
}

pub(crate) type SharedList = Rc<RefCell<ListData>>;

pub(crate) struct ListIvars {
    id: NodeId,
    events: EventSink,
    data: SharedList,
}

define_class!(
    /// The table's data source and delegate, and the target of its double
    /// click.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ListIvars]
    pub(crate) struct ListSource;

    impl ListSource {
        #[unsafe(method(activated:))]
        fn activated(&self, sender: &AnyObject) {
            let Some(table) = sender.downcast_ref::<NSTableView>() else { return };
            self.ivars().activate(table.clickedRow());
        }
    }

    unsafe impl NSObjectProtocol for ListSource {}

    unsafe impl NSControlTextEditingDelegate for ListSource {}

    unsafe impl NSTableViewDataSource for ListSource {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn number_of_rows(&self, _table: &NSTableView) -> NSInteger {
            self.ivars().data.borrow().rows.len() as NSInteger
        }

        #[unsafe(method(tableView:sortDescriptorsDidChange:))]
        fn sort_descriptors_did_change(&self, table: &NSTableView, _old: &NSArray<NSSortDescriptor>) {
            let ListIvars { id, events, data } = self.ivars();
            if data.borrow().muted {
                return;
            }
            if let Some(sort) = sort_of(table) {
                events.emit(*id, UiEvent::Changed(EventValue::Sort(sort)));
            }
            show_sort(table, sort_of(table));
        }
    }

    unsafe impl NSTableViewDelegate for ListSource {
        #[unsafe(method(tableView:heightOfRow:))]
        fn height_of_row(&self, table: &NSTableView, row: NSInteger) -> f64 {
            let data = self.ivars().data.borrow();
            let height = data.rows.get(row as usize).and_then(|key| data.heights.get(key).copied());
            let estimate = || if data.table { data.estimate().max(table.rowHeight()) } else { data.estimate() };
            // AppKit wants rows at least a point high.
            height.unwrap_or_else(estimate).max(1.0)
        }

        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<NSView>> {
            let table = self.ivars().data.borrow().table;
            let cell = if table { HostView::new_cell(self.mtm()) } else { HostView::new(self.mtm(), false) };
            // A table's columns are named by their index; a list's one is 0.
            let column = column.and_then(|c| c.identifier().to_string().parse().ok()).unwrap_or(0);
            let mut data = self.ivars().data.borrow_mut();
            if let Some(key) = data.rows.get(row as usize).copied() {
                if let Some((_, host)) = data.hosts.get(&(key, column)) {
                    cell.addSubview(host);
                }
                data.cells.insert((key, column), cell.clone());
            }
            Some(Retained::into_super(cell))
        }

        #[unsafe(method(tableViewColumnDidResize:))]
        fn column_did_resize(&self, notification: &NSNotification) {
            let Some(table) = notification.object().and_then(|o| o.downcast::<NSTableView>().ok()) else { return };
            self.ivars().report_widths(&table);
        }

        #[unsafe(method(tableView:didAddRowView:forRow:))]
        fn did_add_row_view(&self, _table: &NSTableView, row_view: &NSTableRowView, row: NSInteger) {
            let ListIvars { id, events, data } = self.ivars();
            let mut data = data.borrow_mut();
            let Some(key) = data.rows.get(row as usize).copied() else { return };
            data.row_views.insert(row_view as *const _ as usize, key);
            if !data.reloading && data.shown.insert(key) {
                events.emit(*id, UiEvent::RowShown(key));
            }
        }

        #[unsafe(method(tableView:didRemoveRowView:forRow:))]
        fn did_remove_row_view(&self, _table: &NSTableView, row_view: &NSTableRowView, _row: NSInteger) {
            let ListIvars { id, events, data } = self.ivars();
            let mut data = data.borrow_mut();
            let Some(key) = data.row_views.remove(&(row_view as *const _ as usize)) else { return };
            data.cells.retain(|(row, _), _| *row != key);
            if !data.reloading && data.shown.remove(&key) {
                events.emit(*id, UiEvent::RowHidden(key));
            }
        }

        #[unsafe(method(tableView:shouldSelectRow:))]
        fn should_select_row(&self, _table: &NSTableView, _row: NSInteger) -> bool {
            self.ivars().data.borrow().mode != SelectionMode::None
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_did_change(&self, notification: &NSNotification) {
            let Some(table) = notification.object().and_then(|o| o.downcast::<NSTableView>().ok()) else { return };
            let ListIvars { id, events, data } = self.ivars();
            let data = data.borrow();
            if !data.muted {
                events.emit(*id, UiEvent::Changed(EventValue::Rows(data.keys(&table.selectedRowIndexes()))));
            }
        }
    }
);

impl ListIvars {
    /// Reports the widths a table's columns give their cells, when they
    /// change: a column's less the space between cells.
    fn report_widths(&self, table: &NSTableView) {
        let mut data = self.data.borrow_mut();
        if !data.table {
            return;
        }
        let spacing = table.intercellSpacing().width;
        let widths: Vec<f32> = table.tableColumns().iter().map(|c| (c.width() - spacing).max(0.0) as f32).collect();
        if widths != data.widths {
            data.widths = widths.clone();
            self.events.emit(self.id, UiEvent::ColumnWidths(widths));
        }
    }

    fn activate(&self, row: NSInteger) {
        let key = usize::try_from(row).ok().and_then(|row| self.data.borrow().rows.get(row).copied());
        if let Some(key) = key {
            self.events.emit(self.id, UiEvent::RowActivated(key));
        }
    }
}

define_class!(
    /// The table: Return activates the selected row, as it opens the
    /// selected item in Finder's and Mail's lists.
    #[unsafe(super(NSTableView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ListIvars]
    pub(crate) struct ListTable;

    impl ListTable {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            // Return, and Enter on the keypad, without ⌘, ⌥ or ⌃.
            let held = NSEventModifierFlags::Command | NSEventModifierFlags::Option | NSEventModifierFlags::Control;
            if matches!(event.keyCode(), 36 | 76) && !event.modifierFlags().intersects(held) && self.selectedRow() >= 0 {
                self.ivars().activate(self.selectedRow());
                return;
            }
            unsafe { msg_send![super(self), keyDown: event] }
        }

        /// A right-click in a row: the menu of the view under the pointer,
        /// or of the nearest one around it, as outside a table. The table
        /// takes right-clicks on its rows' labels to itself, and would
        /// show only its own menu (the `List`'s).
        #[unsafe(method_id(menuForEvent:))]
        fn menu_for_event(&self, event: &NSEvent) -> Option<Retained<NSMenu>> {
            self.row_menu(event).or_else(|| unsafe { msg_send![super(self), menuForEvent: event] })
        }

    }
);

define_class!(
    /// The table's next responder, ahead of its clip view: the keys the
    /// table passes on (those it doesn't use) come here first, and go to
    /// the list's own keys (`Prop::Keys`), then on up.
    #[unsafe(super(NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ListIvars]
    pub(crate) struct ListKeys;

    impl ListKeys {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let ListIvars { id, events, data } = self.ivars();
            let keys = data.borrow().keys.clone().unwrap_or_default();
            match crate::keys::shortcut_of(event).filter(|s| keys.contains(s)) {
                Some(shortcut) => events.emit(*id, UiEvent::Key(shortcut)),
                None => unsafe { msg_send![super(self), keyDown: event] },
            }
        }
    }
);

impl ListTable {
    /// Whether the app gave the list this key (`Prop::Keys`).
    pub(crate) fn takes(&self, shortcut: Shortcut) -> bool {
        self.ivars().data.borrow().keys.as_ref().is_some_and(|keys| keys.contains(&shortcut))
    }

    /// The menu of the view under the pointer in a row, or of the nearest
    /// view around it. The table's `hitTest:` answers itself over its
    /// rows' labels, so the search starts from the cell.
    fn row_menu(&self, event: &NSEvent) -> Option<Retained<NSMenu>> {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let (row, column) = (self.rowAtPoint(point), self.columnAtPoint(point));
        if row < 0 || column < 0 {
            return None;
        }
        let cell = self.viewAtColumn_row_makeIfNecessary(column, row, false)?;
        // `hitTest:` takes a point in the superview's coordinates.
        let around = unsafe { cell.superview() }?;
        let mut view = cell.hitTest(around.convertPoint_fromView(point, Some(self)));
        while let Some(v) = view {
            if let Some(menu) = v.menu() {
                return Some(menu);
            }
            if std::ptr::eq(&*v, &*cell) {
                break;
            }
            view = unsafe { v.superview() };
        }
        None
    }
}

fn indices(set: &NSIndexSet) -> Vec<usize> {
    let mut out = Vec::with_capacity(set.count());
    let mut index = set.firstIndex();
    while index != NSNotFound as usize {
        out.push(index);
        index = set.indexGreaterThanIndex(index);
    }
    out
}

fn index_set(indices: impl IntoIterator<Item = usize>) -> Retained<NSIndexSet> {
    let set = NSMutableIndexSet::new();
    for index in indices {
        set.addIndex(index);
    }
    Retained::into_super(set)
}

/// A table's sort, from its first sort descriptor: its column is named by
/// its index.
fn sort_of(table: &NSTableView) -> Option<ColumnSort> {
    let first = table.sortDescriptors().firstObject()?;
    let column = first.key()?.to_string().parse().ok()?;
    let order = if first.ascending() { SortOrder::Ascending } else { SortOrder::Descending };
    Some(ColumnSort { column, order })
}

/// Shows the sort in the header: the sorted column's indicator, pointing
/// its way.
fn show_sort(table: &NSTableView, sort: Option<ColumnSort>) {
    for (index, column) in table.tableColumns().iter().enumerate() {
        let image = sort.filter(|s| s.column == index).and_then(|s| {
            NSImage::imageNamed(&NSString::from_str(match s.order {
                SortOrder::Ascending => "NSAscendingSortIndicator",
                SortOrder::Descending => "NSDescendingSortIndicator",
            }))
        });
        table.setIndicatorImage_inTableColumn(image.as_deref(), &column);
    }
}

/// Runs `f` with AppKit's animations off: row changes happen at once.
fn without_animation(f: impl FnOnce()) {
    NSAnimationContext::beginGrouping();
    NSAnimationContext::currentContext().setDuration(0.0);
    f();
    NSAnimationContext::endGrouping();
}

/// A list's native parts.
pub(crate) struct List {
    pub scroll: Retained<NSScrollView>,
    pub table: Retained<ListTable>,
    /// A list's one column; a table has the app's.
    column: Option<Retained<NSTableColumn>>,
    _source: Retained<ListSource>,
    /// The table's next responder, which the table doesn't retain.
    _keys: Retained<ListKeys>,
    data: SharedList,
    /// The row width last reported.
    reported_width: Cell<Option<f32>>,
    /// AppKit can't tell `Automatic` from `Plain`.
    style: Cell<Option<ListStyle>>,
}

impl List {
    /// A list, or with `columns` a table: its columns come with
    /// `set_columns`.
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink, columns: bool) -> List {
        let data = SharedList::default();
        data.borrow_mut().table = columns;
        let ivars = || ListIvars { id, events: events.clone(), data: data.clone() };
        let source: Retained<ListSource> = unsafe { msg_send![super(ListSource::alloc(mtm).set_ivars(ivars())), init] };
        let table: Retained<ListTable> =
            unsafe { msg_send![super(ListTable::alloc(mtm).set_ivars(ivars())), initWithFrame: zero_rect()] };
        table.setUsesAutomaticRowHeights(false);
        table.setAllowsEmptySelection(true);
        let column = if columns {
            // The table's own header, style and spacing; the columns that
            // expand share its width.
            if table.headerView().is_none() {
                table.setHeaderView(Some(&objc2_app_kit::NSTableHeaderView::new(mtm)));
            }
            table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::UniformColumnAutoresizingStyle);
            None
        } else {
            table.setHeaderView(None);
            // Rows exactly as high as their hosts, one right below the other.
            table.setStyle(NSTableViewStyle::Plain);
            table.setIntercellSpacing(NSSize::new(0.0, 0.0));
            table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
            let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &NSString::from_str("row"));
            column.setResizingMask(NSTableColumnResizingOptions::AutoresizingMask);
            table.addTableColumn(&column);
            Some(column)
        };
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            table.setDelegate(Some(ProtocolObject::from_ref(&*source)));
            table.setTarget(Some(&source));
            table.setDoubleAction(Some(sel!(activated:)));
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), zero_rect());
        scroll.setHasVerticalScroller(true);
        // A table's columns can be wider than it.
        scroll.setHasHorizontalScroller(columns);
        scroll.setAutohidesScrollers(true);
        // The core places the list; AppKit mustn't inset its content for
        // the title bar on top of that.
        scroll.setAutomaticallyAdjustsContentInsets(false);
        scroll.setDocumentView(Some(&table));
        // Keys the table passes on reach the list's own keys first. The
        // table's superview (its next responder) stays put from here.
        let keys: Retained<ListKeys> = unsafe { msg_send![super(ListKeys::alloc(mtm).set_ivars(ivars())), init] };
        // SAFETY: both responders live as long as the list, which keeps
        // `keys` (a next responder isn't retained).
        unsafe {
            keys.setNextResponder(table.nextResponder().as_deref());
            table.setNextResponder(Some(&keys));
        }
        List {
            scroll,
            table,
            column,
            _source: source,
            _keys: keys,
            data,
            reported_width: Cell::new(None),
            style: Cell::new(None),
        }
    }

    /// New rows: the table reloads, keeps the selected rows that stayed
    /// (reporting it if some went), and reports the rows it shows now.
    pub(crate) fn set_rows(&self, rows: Vec<RowKey>) {
        let selected = self.selected();
        {
            let mut data = self.data.borrow_mut();
            let index: HashMap<RowKey, usize> = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
            data.heights.retain(|key, _| index.contains_key(key));
            data.cell_heights.retain(|(key, _), _| index.contains_key(key));
            data.index = index;
            data.rows = rows;
            data.muted = true;
            data.reloading = true;
        }
        self.table.reloadData();
        // The table adds its row views back at its next layout: have it
        // now, so rows that stay are seen to stay (and keep their state).
        self.table.setNeedsLayout(true);
        self.table.layoutSubtreeIfNeeded();
        let kept: Vec<RowKey> = {
            let data = self.data.borrow();
            selected.iter().copied().filter(|k| data.index.contains_key(k)).collect()
        };
        self.select(&kept);
        {
            let mut data = self.data.borrow_mut();
            data.muted = false;
            data.reloading = false;
        }
        if kept != selected {
            self.report_selection();
        }
        self.report_shown();
    }

    /// Reports the rows shown and let go since the last report.
    fn report_shown(&self) {
        let ListIvars { id, events, data } = self.table.ivars();
        let mut data = data.borrow_mut();
        let now: HashSet<RowKey> = data.row_views.values().copied().collect();
        let mut hidden: Vec<RowKey> = data.shown.difference(&now).copied().collect();
        let mut shown: Vec<RowKey> = now.difference(&data.shown).copied().collect();
        hidden.sort_by_key(|k| data.index.get(k).copied());
        shown.sort_by_key(|k| data.index.get(k).copied());
        for row in hidden {
            events.emit(*id, UiEvent::RowHidden(row));
        }
        for row in shown {
            events.emit(*id, UiEvent::RowShown(row));
        }
        data.shown = now;
    }

    pub(crate) fn set_keys(&self, keys: Vec<Shortcut>) {
        self.data.borrow_mut().keys = Some(keys);
    }

    pub(crate) fn keys(&self) -> Option<Vec<Shortcut>> {
        self.data.borrow().keys.clone()
    }

    pub(crate) fn set_estimate(&self, height: f32) {
        self.data.borrow_mut().estimate = Some(height as f64);
    }

    pub(crate) fn estimate(&self) -> Option<f32> {
        self.data.borrow().estimate.map(|h| h as f32)
    }

    /// Selects rows without reporting it.
    pub(crate) fn set_selected(&self, keys: &[RowKey]) {
        self.data.borrow_mut().muted = true;
        self.select(keys);
        self.data.borrow_mut().muted = false;
    }

    fn select(&self, keys: &[RowKey]) {
        let indexes = {
            let data = self.data.borrow();
            index_set(keys.iter().filter_map(|k| data.index.get(k).copied()))
        };
        self.table.selectRowIndexes_byExtendingSelection(&indexes, false);
    }

    /// Reports the selection as the user's: what a screen reader's select
    /// does.
    pub(crate) fn report_selection(&self) {
        let ListIvars { id, events, .. } = self.table.ivars();
        events.emit(*id, UiEvent::Changed(EventValue::Rows(self.selected())));
    }

    /// The table keeps its selection when it stops allowing several rows,
    /// or any: keep what the new mode holds, `selectedRow` (the row
    /// selected last) for one, and report the rows let go.
    pub(crate) fn set_mode(&self, mode: SelectionMode) {
        let selected = self.selected();
        self.data.borrow_mut().mode = mode;
        self.table.setAllowsMultipleSelection(mode == SelectionMode::Multiple);
        let kept: Vec<RowKey> = match mode {
            SelectionMode::None => Vec::new(),
            SelectionMode::Single if selected.len() > 1 => {
                let last = usize::try_from(self.table.selectedRow()).ok();
                last.and_then(|row| self.data.borrow().rows.get(row).copied()).into_iter().collect()
            }
            _ => selected.clone(),
        };
        if kept != selected {
            self.set_selected(&kept);
            self.report_selection();
        }
    }

    pub(crate) fn mode(&self) -> SelectionMode {
        self.data.borrow().mode
    }

    /// A framed list has the bezel border, which takes room from the rows.
    pub(crate) fn set_style(&self, style: ListStyle) {
        self.style.set(Some(style));
        self.scroll.setBorderType(if style.framed() { NSBorderType::BezelBorder } else { NSBorderType::NoBorder });
        self.set_frame(self.scroll.frame());
    }

    pub(crate) fn style(&self) -> Option<ListStyle> {
        self.style.get()
    }

    /// Hosts a mounted row, or a table's cell, in its cell if the table
    /// shows it.
    pub(crate) fn insert(&self, key: RowKey, column: usize, id: NodeId, view: Retained<NSView>) {
        let mut data = self.data.borrow_mut();
        if let Some(cell) = data.cells.get(&(key, column)) {
            cell.addSubview(&view);
            if data.table {
                cell.center_subviews();
            }
        }
        data.hosts.insert((key, column), (id, view));
    }

    pub(crate) fn remove(&self, key: RowKey, column: usize) {
        if let Some((_, view)) = self.data.borrow_mut().hosts.remove(&(key, column)) {
            view.removeFromSuperview();
        }
    }

    /// A host has a new size: a table centres a cell's in its cell. The
    /// table re-reads the row's height, a list's host's or a table's
    /// highest cell's. The first one measured, without the app's estimate,
    /// becomes the estimate, and the table re-reads every row's height,
    /// once. A list scrolled to its end stays there.
    pub(crate) fn set_host_height(&self, key: RowKey, column: usize, height: f32) {
        let changed = {
            let mut data = self.data.borrow_mut();
            let height = height as f64;
            if data.table {
                data.cell_heights.insert((key, column), height);
                if let Some(cell) = data.cells.get(&(key, column)) {
                    cell.center_subviews();
                }
            }
            let Some(height) = (if data.table { data.row_height(&self.table, key) } else { Some(height) }) else {
                return;
            };
            if data.heights.insert(key, height) == Some(height) {
                return;
            }
            if data.estimate.is_none() && data.learned.is_none() {
                data.learned = Some(height);
                index_set(0..data.rows.len())
            } else {
                match data.index.get(&key) {
                    Some(index) => index_set([*index]),
                    None => return,
                }
            }
        };
        let clip = self.scroll.contentView();
        let at_end = clip.bounds().origin.y > 0.0
            && clip.bounds().origin.y + clip.bounds().size.height >= self.table.frame().size.height - 0.5;
        without_animation(|| self.table.noteHeightOfRowsWithIndexesChanged(&changed));
        if at_end {
            let end = (self.table.frame().size.height - clip.bounds().size.height).max(0.0);
            clip.scrollToPoint(objc2_foundation::NSPoint::new(clip.bounds().origin.x, end));
            self.scroll.reflectScrolledClipView(&clip);
        }
    }

    /// Sizes the list, and its one column to the width rows get, which it
    /// reports when it changes: legacy scroll bars and a border take room
    /// from the rows. A table sizes its columns itself, and reports their
    /// widths.
    pub(crate) fn set_frame(&self, frame: objc2_foundation::NSRect) {
        self.scroll.setFrame(frame);
        let ListIvars { id, events, .. } = self.table.ivars();
        let Some(column) = &self.column else {
            self.table.ivars().report_widths(&self.table);
            return;
        };
        let width = self.scroll.contentSize().width;
        column.setWidth(width);
        let width = width as f32;
        if self.reported_width.replace(Some(width)) != Some(width) {
            events.emit(*id, UiEvent::RowWidth(width));
        }
    }

    /// A table's columns: their titles, the widths they start at, the ones
    /// that take the room left (the uniform style shares it among those
    /// that autoresize), and a sort descriptor for the sortable ones,
    /// which makes their headers sort. Each is named by its index.
    pub(crate) fn set_columns(&self, mtm: MainThreadMarker, columns: &[ColumnData]) {
        let sort = sort_of(&self.table);
        self.data.borrow_mut().muted = true;
        for column in self.table.tableColumns().iter() {
            self.table.removeTableColumn(&column);
        }
        for (index, data) in columns.iter().enumerate() {
            let name = NSString::from_str(&index.to_string());
            let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &name);
            column.setTitle(&NSString::from_str(&data.title));
            if let Some(width) = data.width {
                column.setWidth(width as f64);
            }
            column.setResizingMask(if data.expand {
                NSTableColumnResizingOptions::AutoresizingMask | NSTableColumnResizingOptions::UserResizingMask
            } else {
                NSTableColumnResizingOptions::UserResizingMask
            });
            if data.sortable {
                let prototype = NSSortDescriptor::sortDescriptorWithKey_ascending(Some(&name), true);
                column.setSortDescriptorPrototype(Some(&prototype));
            }
            self.table.addTableColumn(&column);
        }
        self.data.borrow_mut().columns = columns.to_vec();
        self.table.sizeToFit();
        self.set_sort(sort.filter(|s| columns.get(s.column).is_some_and(|c| c.sortable)));
        self.data.borrow_mut().muted = false;
        self.table.ivars().report_widths(&self.table);
    }

    pub(crate) fn columns(&self) -> Vec<ColumnData> {
        self.data.borrow().columns.clone()
    }

    /// Shows the sort in the header, without reporting it.
    pub(crate) fn set_sort(&self, sort: Option<ColumnSort>) {
        let muted = std::mem::replace(&mut self.data.borrow_mut().muted, true);
        let descriptors: Vec<Retained<NSSortDescriptor>> = sort
            .map(|s| {
                let key = NSString::from_str(&s.column.to_string());
                NSSortDescriptor::sortDescriptorWithKey_ascending(Some(&key), s.order == SortOrder::Ascending)
            })
            .into_iter()
            .collect();
        self.table.setSortDescriptors(&NSArray::from_retained_slice(&descriptors));
        show_sort(&self.table, sort);
        self.data.borrow_mut().muted = muted;
    }

    pub(crate) fn sort(&self) -> Option<ColumnSort> {
        sort_of(&self.table)
    }

    /// Presses a sortable column's header, as a click on it does: the same
    /// column the other way round, another one by its prototype
    /// (ascending). The table reports it.
    pub(crate) fn press_header(&self, column: usize) -> bool {
        let columns = self.table.tableColumns();
        if column >= columns.len() {
            return false;
        }
        let Some(prototype) = columns.objectAtIndex(column).sortDescriptorPrototype() else { return false };
        let descriptor = match self.table.sortDescriptors().firstObject() {
            // `reversedSortDescriptor` returns an `NSSortDescriptor`.
            Some(first) if first.key() == prototype.key() => unsafe {
                Retained::cast_unchecked::<NSSortDescriptor>(first.reversedSortDescriptor())
            },
            _ => prototype,
        };
        self.table.setSortDescriptors(&NSArray::from_retained_slice(&[descriptor]));
        true
    }

    pub(crate) fn scroll_to_row(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let Some(index) = index {
            self.table.scrollRowToVisible(index as NSInteger);
        }
    }

    /// Activates a row, as double-clicking it does.
    pub(crate) fn activate(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let Some(index) = index {
            self.table.ivars().activate(index as NSInteger);
        }
    }

    /// Stops the table from calling its data source, which goes with the list.
    pub(crate) fn detach(&self) {
        unsafe {
            self.table.setDataSource(None);
            self.table.setDelegate(None);
            self.table.setTarget(None);
        }
    }

    /// Where the table put a row, in its content's coordinates.
    pub(crate) fn row_rect(&self, key: RowKey) -> Option<Rect> {
        let index = *self.data.borrow().index.get(&key)?;
        let r = self.table.rectOfRow(index as NSInteger);
        Some(Rect::new(r.origin.x as f32, r.origin.y as f32, r.size.width as f32, r.size.height as f32))
    }

    /// Where the table put a cell's host, in the scroll view's content
    /// (below the header, unscrolled): its cell's place, and its own in
    /// the cell.
    pub(crate) fn cell_rect(&self, key: RowKey, column: usize, host: &NSView) -> Option<Rect> {
        let index = *self.data.borrow().index.get(&key)?;
        let cell = self.table.frameOfCellAtColumn_row(column as NSInteger, index as NSInteger);
        let origin = self.table.convertPoint_toView(cell.origin, Some(&self.scroll));
        let scrolled = crate::classes::scrolled(&self.scroll.contentView());
        let f = host.frame();
        Some(Rect::new(
            (origin.x + scrolled.x + f.origin.x) as f32,
            (origin.y + scrolled.y + f.origin.y) as f32,
            f.size.width as f32,
            f.size.height as f32,
        ))
    }

    /// The rows as the table has them.
    pub(crate) fn rows(&self) -> Vec<RowKey> {
        let data = self.data.borrow();
        (0..self.table.numberOfRows()).filter_map(|i| data.rows.get(i as usize).copied()).collect()
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        self.data.borrow().keys(&self.table.selectedRowIndexes())
    }

    /// The hosted rows, in row order.
    pub(crate) fn children(&self) -> Vec<NodeId> {
        let data = self.data.borrow();
        let mut hosts: Vec<((usize, usize), NodeId)> = data
            .hosts
            .iter()
            .filter_map(|((key, column), (id, _))| Some(((*data.index.get(key)?, *column), *id)))
            .collect();
        hosts.sort();
        hosts.into_iter().map(|(_, id)| id).collect()
    }
}
