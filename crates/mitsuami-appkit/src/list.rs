//! `List`: a view-based `NSTableView` with one column and no header, in an
//! `NSScrollView`.
//!
//! The table asks for rows whenever it likes, including in the middle of our
//! own `apply` (a reload, a scroll, a resize), so its data source reads only
//! the list's own [`ListData`], never the backend's state, and only emits.
//! Each row's cell view is the row's host, once the core has mounted it, or
//! an empty view until then.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::{EventSink, EventValue, ListRow, NodeId, Rect, RowKey, SelectionMode, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimationContext, NSControlTextEditingDelegate, NSEvent, NSScrollView, NSTableColumn,
    NSTableColumnResizingOptions, NSTableView, NSTableViewColumnAutoresizingStyle, NSTableViewDataSource,
    NSTableViewDelegate, NSTableViewStyle, NSView,
};
use objc2_foundation::{NSIndexSet, NSInteger, NSMutableIndexSet, NSNotFound, NSNotification, NSSize, NSString};

use crate::classes::{HostView, zero_rect};

/// What the table shows, shared by the backend and the table's data source.
#[derive(Default)]
pub(crate) struct ListData {
    rows: Vec<ListRow>,
    index: HashMap<RowKey, usize>,
    /// The mounted rows' hosts.
    hosts: HashMap<RowKey, (NodeId, Retained<NSView>)>,
    mode: SelectionMode,
    /// Set while the backend changes the selection itself.
    muted: bool,
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
    }

    unsafe impl NSTableViewDelegate for ListSource {
        #[unsafe(method(tableView:heightOfRow:))]
        fn height_of_row(&self, _table: &NSTableView, row: NSInteger) -> f64 {
            // AppKit wants rows at least a point high.
            let data = self.ivars().data.borrow();
            data.rows.get(row as usize).map_or(1.0, |r| (r.height as f64).max(1.0))
        }

        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            _column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<NSView>> {
            let host = {
                let data = self.ivars().data.borrow();
                data.rows.get(row as usize).and_then(|r| data.hosts.get(&r.key)).map(|(_, view)| view.clone())
            };
            // Not mounted yet: an empty cell, until the core sends the host.
            Some(host.unwrap_or_else(|| Retained::into_super(HostView::new(self.mtm(), false))))
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
    fn activate(&self, row: NSInteger) {
        let key = self.data.borrow().rows.get(usize::try_from(row).ok().unwrap_or(usize::MAX)).map(|r| r.key);
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
            // Return, and Enter on the keypad.
            if matches!(event.keyCode(), 36 | 76) && self.selectedRow() >= 0 {
                self.ivars().activate(self.selectedRow());
                return;
            }
            unsafe { msg_send![super(self), keyDown: event] }
        }
    }
);

impl ListData {
    /// The keys of the rows at these indexes.
    fn keys(&self, indexes: &NSIndexSet) -> Vec<RowKey> {
        indices(indexes).into_iter().filter_map(|i| self.rows.get(i).map(|r| r.key)).collect()
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
    column: Retained<NSTableColumn>,
    _source: Retained<ListSource>,
    pub data: SharedList,
    /// The row width last reported.
    reported_width: Cell<Option<f32>>,
}

impl List {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> List {
        let data = SharedList::default();
        let ivars = || ListIvars { id, events: events.clone(), data: data.clone() };
        let source: Retained<ListSource> = unsafe { msg_send![super(ListSource::alloc(mtm).set_ivars(ivars())), init] };
        let table: Retained<ListTable> =
            unsafe { msg_send![super(ListTable::alloc(mtm).set_ivars(ivars())), initWithFrame: zero_rect()] };
        table.setHeaderView(None);
        // Rows exactly as high as the core says, one right below the other.
        table.setStyle(NSTableViewStyle::Plain);
        table.setIntercellSpacing(NSSize::new(0.0, 0.0));
        table.setUsesAutomaticRowHeights(false);
        table.setAllowsEmptySelection(true);
        table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &NSString::from_str("row"));
        column.setResizingMask(NSTableColumnResizingOptions::AutoresizingMask);
        table.addTableColumn(&column);
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            table.setDelegate(Some(ProtocolObject::from_ref(&*source)));
            table.setTarget(Some(&source));
            table.setDoubleAction(Some(sel!(activated:)));
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), zero_rect());
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        // The core places the list; AppKit mustn't inset its content for
        // the title bar on top of that.
        scroll.setAutomaticallyAdjustsContentInsets(false);
        scroll.setDocumentView(Some(&table));
        List { scroll, table, column, _source: source, data, reported_width: Cell::new(None) }
    }

    /// New rows. The same keys with new heights: the table re-reads the
    /// heights. Otherwise it reloads, and keeps the selected rows that
    /// stayed selected (reporting it if some went).
    pub(crate) fn set_rows(&self, rows: Vec<ListRow>) {
        let (same_keys, changed_heights, selected) = {
            let data = self.data.borrow();
            let same = data.rows.len() == rows.len() && data.rows.iter().zip(&rows).all(|(a, b)| a.key == b.key);
            let changed: Vec<usize> =
                data.rows.iter().zip(&rows).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
            (same, changed, data.keys(&self.table.selectedRowIndexes()))
        };
        {
            let mut data = self.data.borrow_mut();
            data.index = rows.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
            data.rows = rows;
        }
        if same_keys {
            if !changed_heights.is_empty() {
                without_animation(|| self.table.noteHeightOfRowsWithIndexesChanged(&index_set(changed_heights)));
            }
            return;
        }
        self.data.borrow_mut().muted = true;
        self.table.reloadData();
        let kept: Vec<RowKey> = {
            let data = self.data.borrow();
            selected.iter().copied().filter(|k| data.index.contains_key(k)).collect()
        };
        self.select(&kept);
        self.data.borrow_mut().muted = false;
        if kept != selected {
            self.report_selection();
        }
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
        let ListIvars { id, events, data } = self.table.ivars();
        events.emit(*id, UiEvent::Changed(EventValue::Rows(data.borrow().keys(&self.table.selectedRowIndexes()))));
    }

    pub(crate) fn set_mode(&self, mode: SelectionMode) {
        self.data.borrow_mut().mode = mode;
        self.table.setAllowsMultipleSelection(mode == SelectionMode::Multiple);
    }

    pub(crate) fn mode(&self) -> SelectionMode {
        self.data.borrow().mode
    }

    /// Hosts a mounted row, and shows it if its row is on screen.
    pub(crate) fn insert(&self, key: RowKey, id: NodeId, view: Retained<NSView>) {
        self.data.borrow_mut().hosts.insert(key, (id, view));
        self.reload_row(key);
    }

    /// Unhosts a row; an empty cell takes its place.
    pub(crate) fn remove(&self, key: RowKey) {
        let removed = self.data.borrow_mut().hosts.remove(&key);
        if removed.is_some() {
            self.reload_row(key);
        }
    }

    fn reload_row(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let Some(index) = index {
            self.table.reloadDataForRowIndexes_columnIndexes(&index_set([index]), &index_set([0]));
        }
    }

    /// Sizes the list, and its one column to the width rows get, which it
    /// reports when it changes: legacy scroll bars take room from the rows.
    pub(crate) fn set_frame(&self, frame: objc2_foundation::NSRect) {
        self.scroll.setFrame(frame);
        let width = self.scroll.contentSize().width;
        self.column.setWidth(width);
        let width = width as f32;
        if self.reported_width.replace(Some(width)) != Some(width) {
            let ListIvars { id, events, .. } = self.table.ivars();
            events.emit(*id, UiEvent::RowWidth(width));
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

    /// The rows as the table has them: its row count, and each row's height.
    pub(crate) fn native_rows(&self) -> Vec<ListRow> {
        let data = self.data.borrow();
        (0..self.table.numberOfRows())
            .map(|i| ListRow {
                key: data.rows.get(i as usize).map_or(RowKey(0), |r| r.key),
                height: self.table.rectOfRow(i).size.height as f32,
            })
            .collect()
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        self.data.borrow().keys(&self.table.selectedRowIndexes())
    }

    /// The hosted rows, in row order.
    pub(crate) fn children(&self) -> Vec<NodeId> {
        let data = self.data.borrow();
        let mut hosts: Vec<(usize, NodeId)> =
            data.hosts.iter().filter_map(|(key, (id, _))| Some((*data.index.get(key)?, *id))).collect();
        hosts.sort();
        hosts.into_iter().map(|(_, id)| id).collect()
    }
}
