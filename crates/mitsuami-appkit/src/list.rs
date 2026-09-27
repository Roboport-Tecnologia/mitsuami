//! `List`: a view-based `NSTableView` with one column and no header, in an
//! `NSScrollView`.
//!
//! The table virtualises: it adds row views for the rows it shows and
//! removes them when they scroll away, and we report both (`RowShown`,
//! `RowHidden`) so the core mounts and disposes those rows. Each row's cell
//! is a plain view that holds the row's host once the core sends it.
//!
//! The table calls its data source and delegate whenever it likes,
//! including in the middle of our own `apply` (a reload, a scroll, a
//! resize), so they read only the list's own [`ListData`], never the
//! backend's state, and only emit.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mitsuami_core::{EventSink, EventValue, ListStyle, NodeId, Rect, RowKey, SelectionMode, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimationContext, NSBorderType, NSControlTextEditingDelegate, NSEvent, NSScrollView, NSTableColumn,
    NSTableColumnResizingOptions, NSTableRowView, NSTableView, NSTableViewColumnAutoresizingStyle,
    NSTableViewDataSource, NSTableViewDelegate, NSTableViewStyle, NSView,
};
use objc2_foundation::{NSIndexSet, NSInteger, NSMutableIndexSet, NSNotFound, NSNotification, NSSize, NSString};

use crate::classes::{HostView, zero_rect};

/// What the table shows, shared by the backend and the table's data source.
#[derive(Default)]
pub(crate) struct ListData {
    rows: Vec<RowKey>,
    index: HashMap<RowKey, usize>,
    /// The rows' heights, from their hosts' sizes.
    heights: HashMap<RowKey, f64>,
    estimate: Option<f64>,
    /// Without the app's estimate: the first row measured.
    learned: Option<f64>,
    /// The mounted rows' hosts.
    hosts: HashMap<RowKey, (NodeId, Retained<NSView>)>,
    /// The cells of the rows the table shows.
    cells: HashMap<RowKey, Retained<HostView>>,
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
            let data = self.ivars().data.borrow();
            let height = data.rows.get(row as usize).and_then(|key| data.heights.get(key).copied());
            // AppKit wants rows at least a point high.
            height.unwrap_or_else(|| data.estimate()).max(1.0)
        }

        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            _column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<NSView>> {
            let cell = HostView::new(self.mtm(), false);
            let mut data = self.ivars().data.borrow_mut();
            if let Some(key) = data.rows.get(row as usize).copied() {
                if let Some((_, host)) = data.hosts.get(&key) {
                    cell.addSubview(host);
                }
                data.cells.insert(key, cell.clone());
            }
            Some(Retained::into_super(cell))
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
            data.cells.remove(&key);
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
            // Return, and Enter on the keypad.
            if matches!(event.keyCode(), 36 | 76) && self.selectedRow() >= 0 {
                self.ivars().activate(self.selectedRow());
                return;
            }
            unsafe { msg_send![super(self), keyDown: event] }
        }
    }
);

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
    data: SharedList,
    /// The row width last reported.
    reported_width: Cell<Option<f32>>,
    /// AppKit can't tell `Automatic` from `Plain`.
    style: Cell<Option<ListStyle>>,
}

impl List {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> List {
        let data = SharedList::default();
        let ivars = || ListIvars { id, events: events.clone(), data: data.clone() };
        let source: Retained<ListSource> = unsafe { msg_send![super(ListSource::alloc(mtm).set_ivars(ivars())), init] };
        let table: Retained<ListTable> =
            unsafe { msg_send![super(ListTable::alloc(mtm).set_ivars(ivars())), initWithFrame: zero_rect()] };
        table.setHeaderView(None);
        // Rows exactly as high as their hosts, one right below the other.
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
        List { scroll, table, column, _source: source, data, reported_width: Cell::new(None), style: Cell::new(None) }
    }

    /// New rows: the table reloads, keeps the selected rows that stayed
    /// (reporting it if some went), and reports the rows it shows now.
    pub(crate) fn set_rows(&self, rows: Vec<RowKey>) {
        let selected = self.selected();
        {
            let mut data = self.data.borrow_mut();
            let index: HashMap<RowKey, usize> = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
            data.heights.retain(|key, _| index.contains_key(key));
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

    pub(crate) fn set_mode(&self, mode: SelectionMode) {
        self.data.borrow_mut().mode = mode;
        self.table.setAllowsMultipleSelection(mode == SelectionMode::Multiple);
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

    /// Hosts a mounted row, in its cell if the table shows it.
    pub(crate) fn insert(&self, key: RowKey, id: NodeId, view: Retained<NSView>) {
        let mut data = self.data.borrow_mut();
        if let Some(cell) = data.cells.get(&key) {
            cell.addSubview(&view);
        }
        data.hosts.insert(key, (id, view));
    }

    pub(crate) fn remove(&self, key: RowKey) {
        if let Some((_, view)) = self.data.borrow_mut().hosts.remove(&key) {
            view.removeFromSuperview();
        }
    }

    /// A row's host has a new height: the table re-reads it. The first
    /// one measured, without the app's estimate, becomes the estimate, and
    /// the table re-reads every row's height, once. A list scrolled to its
    /// end stays there.
    pub(crate) fn set_row_height(&self, key: RowKey, height: f32) {
        let changed = {
            let mut data = self.data.borrow_mut();
            let height = height as f64;
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
    /// from the rows.
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
        let mut hosts: Vec<(usize, NodeId)> =
            data.hosts.iter().filter_map(|(key, (id, _))| Some((*data.index.get(key)?, *id))).collect();
        hosts.sort();
        hosts.into_iter().map(|(_, id)| id).collect()
    }
}
