//! `List`, `Table` and `Tree`: a view-based `NSTableView` in an
//! `NSScrollView`, with one column and no header for a list, and the app's
//! columns under a header for a table; a tree's is an `NSOutlineView`, an
//! `NSTableView` too, with one column that's its outline column.
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
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::{
    ColumnData, ColumnSort, EventSink, EventValue, ListStyle, NodeId, Rect, RowDrop, RowKey, RowMoves, SelectionMode,
    SortOrder, TreeIndent, TreeRow, UiEvent,
};

use mitsuami_core::services::Shortcut;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimationContext, NSBorderType, NSControlSize, NSControlTextEditingDelegate, NSDragOperation, NSDraggingInfo,
    NSEvent, NSEventModifierFlags, NSImage, NSMenu, NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate,
    NSOutlineViewDisclosureButtonKey, NSPasteboardItem, NSPasteboardTypeFileURL, NSPasteboardWriting,
    NSProgressIndicator, NSProgressIndicatorStyle, NSResponder, NSScrollView, NSTableColumn,
    NSTableColumnResizingOptions, NSTableRowView, NSTableView, NSTableViewAnimationOptions,
    NSTableViewColumnAutoresizingStyle, NSTableViewDataSource, NSTableViewDelegate, NSTableViewDropOperation,
    NSTableViewStyle, NSUserInterfaceItemIdentification, NSView, NSViewBoundsDidChangeNotification,
    NSViewFrameDidChangeNotification,
};
use objc2_foundation::{
    NSArray, NSIndexSet, NSInteger, NSKeyValueObservingOptions, NSMutableIndexSet, NSNotFound, NSNotification,
    NSNotificationCenter, NSObjectNSKeyValueObserverRegistration, NSPoint, NSRect, NSSize, NSSortDescriptor, NSString,
    NSURL,
};

use crate::classes::{HostView, zero_rect};

/// The pasteboard type a row dragged within its list carries: its key.
const ROW_TYPE: &str = "br.com.roboport.mitsuami.row";

/// The key path of a table column's width, which the list watches.
const WIDTH: &str = "width";

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
    /// The heights the table was given, which it keeps until told a row's
    /// changed.
    answered: HashMap<RowKey, f64>,
    /// Tables only: their cells' hosts' heights.
    cell_heights: HashMap<Slot, f64>,
    /// Tables with a column that fills its rows only: the rows' heights
    /// without those cells, as reported.
    bare_heights: HashMap<RowKey, f64>,
    estimate: Option<f64>,
    /// Without the app's estimate: the first row measured.
    learned: Option<f64>,
    /// The mounted rows' hosts: a list's row hosts, a table's cell hosts.
    hosts: HashMap<Slot, (NodeId, Retained<NSView>)>,
    /// The cells of the rows the table shows, by row and then column, so
    /// a row view's going lets go of its cells without a walk.
    cells: HashMap<RowKey, HashMap<usize, Retained<HostView>>>,
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
    /// How deep the table is in its own layout, where it adds and removes
    /// row views in a run: they're reported once, as it finishes.
    laying_out: usize,
    /// A row view came or went while laying out.
    shown_stale: bool,
    /// The keys the app gave it (`Prop::Keys`), if any.
    keys: Option<Vec<Shortcut>>,
    /// The rows' files, as the app gave them (`Prop::RowFiles`), and by row.
    files: Option<Vec<(RowKey, PathBuf)>>,
    file_of: HashMap<RowKey, PathBuf>,
    /// Lists only: the row width last reported.
    row_width: Option<f32>,
    /// Where rows dragged within it can go (`Prop::RowMoves`).
    moves: Option<RowMoves>,
    /// Trees only: each row that shows, as sent; the children of each open
    /// row (and the top rows, under `None`), which the data source gives;
    /// an object per row, the item the outline view knows it by, kept while
    /// the row is there, as the outline view tells items apart by identity;
    /// whether the backend is opening and closing rows itself; and the
    /// indent last reported.
    tree: HashMap<RowKey, TreeRow>,
    children: HashMap<Option<RowKey>, Vec<RowKey>>,
    items: HashMap<RowKey, Retained<NSString>>,
    expanding: bool,
    indent: Option<TreeIndent>,
    /// The rows whose children are being read, as sent (`TreeLoading`).
    loading: Option<HashSet<RowKey>>,
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

    /// What dragging a row gives the pasteboard: its file's URL, which
    /// Finder and other apps take as the file, and, where rows move within
    /// the list, its key. The table asks for each row it drags: the
    /// selected rows when a selected row is dragged.
    fn pasteboard_writer(&self, key: RowKey) -> Option<Retained<ProtocolObject<dyn NSPasteboardWriting>>> {
        let url =
            self.file_of.get(&key).map(|path| NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy())));
        if self.moves.is_none() {
            return url.map(ProtocolObject::from_retained);
        }
        let item = NSPasteboardItem::new();
        item.setString_forType(&NSString::from_str(&key.0.to_string()), &NSString::from_str(ROW_TYPE));
        if let Some(url) = url.and_then(|url| url.absoluteString()) {
            item.setString_forType(&url, unsafe { NSPasteboardTypeFileURL });
        }
        Some(ProtocolObject::from_retained(item))
    }

    /// Where a drop the table proposes (a row, and on it or above it; the
    /// row past the last for the end) lands, by `RowMoves::target`, with
    /// the row and operation the table shows it at. `None`: refused.
    fn drop_target(&self, dragged: &[RowKey], row: isize, on: bool) -> Option<(RowDrop<RowKey>, isize, bool)> {
        let moves = self.moves?;
        let proposed = match (usize::try_from(row).ok().and_then(|i| self.rows.get(i)), on) {
            (Some(key), true) => RowDrop::On(*key),
            (Some(key), false) => RowDrop::Before(*key),
            (None, _) => RowDrop::End,
        };
        let target = moves.target(dragged, proposed)?;
        let (row, on) = match target {
            RowDrop::On(key) => (*self.index.get(&key)? as isize, true),
            RowDrop::Before(key) => (*self.index.get(&key)? as isize, false),
            RowDrop::End => (self.rows.len() as isize, false),
        };
        Some((target, row, on))
    }

    /// What an outline view's proposal (an item, or the top, and a child
    /// index; -1 for onto the item) is as a drop: onto the item, before
    /// the child at the index, or, past a folder's last child, before the
    /// row after everything in the folder (or the end).
    fn tree_proposal(&self, item: Option<RowKey>, index: isize) -> RowDrop<RowKey> {
        let Ok(index) = usize::try_from(index) else {
            return item.map_or(RowDrop::End, RowDrop::On);
        };
        if let Some(child) = self.children.get(&item).and_then(|c| c.get(index)) {
            return RowDrop::Before(*child);
        }
        let Some((key, at)) = item.and_then(|key| Some((key, *self.index.get(&key)?))) else { return RowDrop::End };
        let depth = self.tree.get(&key).map_or(0, |r| r.depth);
        let after = self.rows[at + 1..].iter().find(|r| self.tree.get(r).is_none_or(|r| r.depth <= depth));
        after.map_or(RowDrop::End, |row| RowDrop::Before(*row))
    }

    /// The row under a drag's pointer, and what the pointer proposes there,
    /// as every tree proposes (`RowMoves::tree_proposal`): below the rows,
    /// the end. The outline's own proposal (`tree_proposal`) stands in when
    /// there's no row to read.
    fn pointer_proposal(
        &self,
        outline: &NSOutlineView,
        info: &ProtocolObject<dyn NSDraggingInfo>,
        fallback: RowDrop<RowKey>,
    ) -> (Option<RowKey>, RowDrop<RowKey>) {
        let point = outline.convertPoint_fromView(info.draggingLocation(), None);
        let row = outline.rowAtPoint(point);
        let Some(key) = usize::try_from(row).ok().and_then(|i| self.rows.get(i)).copied() else {
            return (None, if row < 0 { RowDrop::End } else { fallback });
        };
        let rect = outline.rectOfRow(row);
        // The outline is flipped: y grows down the row.
        let y = (point.y - rect.origin.y) as f32;
        let next = self.index.get(&key).and_then(|i| self.rows.get(i + 1)).copied();
        // Across the outline, whose rows start at its edge: it picks the
        // level, as AppKit's outlines pick it.
        let x = self.indent.map(|_| point.x as f32);
        let indent = self.indent.unwrap_or_default();
        let proposed =
            RowMoves::tree_proposal(key, x, y, rect.size.height as f32, indent, next, |r| self.tree.get(&r).copied());
        (Some(key), proposed)
    }

    /// Where a drop in a tree lands, by `RowMoves::tree_target` (only
    /// folders take rows onto them, nothing lands over a dragged row,
    /// `over`), with the item and child index the outline view shows it at
    /// (-1: onto the item). A drop onto a folder that isn't the row under
    /// the pointer is at the end of it, below its last row, so the outline
    /// shows it there, past its last child. `None`: refused.
    fn tree_drop_target(
        &self,
        dragged: &[RowKey],
        over: Option<RowKey>,
        proposed: RowDrop<RowKey>,
    ) -> Option<(RowDrop<RowKey>, Option<RowKey>, isize)> {
        let moves = self.moves?;
        let target = moves.tree_target(dragged, over, proposed, |row| self.tree.get(&row).copied())?;
        let (item, index) = match target {
            RowDrop::On(key) if over.is_some_and(|o| o != key) => {
                (Some(key), self.children.get(&Some(key)).map_or(0, |c| c.len()) as isize)
            }
            RowDrop::On(key) => (Some(key), -1),
            RowDrop::Before(key) => {
                let parent = self.tree.get(&key)?.parent;
                (parent, self.children.get(&parent)?.iter().position(|c| *c == key)? as isize)
            }
            RowDrop::End => (None, self.children.get(&None).map_or(0, |c| c.len()) as isize),
        };
        Some((target, item, index))
    }

    /// The rows a drag carries, in the list's order: the keys on its
    /// pasteboard, from this list only.
    fn dragged(&self, table: &NSTableView, info: &ProtocolObject<dyn NSDraggingInfo>) -> Vec<RowKey> {
        let from_here = info
            .draggingSource()
            .is_some_and(|source| std::ptr::eq(&*source, table as &NSTableView as *const _ as *const AnyObject));
        if !from_here {
            return Vec::new();
        }
        let items = info.draggingPasteboard().pasteboardItems().unwrap_or_default();
        let mut rows: Vec<RowKey> = items
            .iter()
            .filter_map(|item| item.stringForType(&NSString::from_str(ROW_TYPE)))
            .filter_map(|key| key.to_string().parse().ok().map(RowKey))
            .filter(|row| self.index.contains_key(row))
            .collect();
        rows.sort_by_key(|row| self.index[row]);
        rows
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

    /// A row's height without its cells that fill it, if it has any: its
    /// other cells' highest, and at least the row height.
    fn bare_height(&self, table: &NSTableView, key: RowKey) -> Option<f64> {
        if !self.columns.iter().any(|c| c.fills_row) {
            return None;
        }
        let others = self.columns.iter().enumerate().filter(|(_, c)| !c.fills_row);
        let cells = others.filter_map(|(column, _)| self.cell_heights.get(&(key, column)).copied());
        Some(cells.fold(table.rowHeight(), f64::max))
    }
}

/// A tree's row, by the item the outline view has for it.
fn item_key(item: &AnyObject) -> Option<RowKey> {
    item.downcast_ref::<NSString>()?.to_string().parse().ok().map(RowKey)
}

/// The keys of an outline view's rows at these indexes, as it shows them.
fn outline_keys(outline: &NSOutlineView, indexes: &NSIndexSet) -> Vec<RowKey> {
    indices(indexes)
        .into_iter()
        .filter_map(|i| outline.itemAtRow(i as NSInteger).and_then(|item| item_key(&item)))
        .collect()
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
        /// The table scrolled: rows coming near are built before they're
        /// in view.
        #[unsafe(method(clipBoundsChanged:))]
        fn clip_bounds_changed(&self, notification: &NSNotification) {
            if let Some((_, table)) = clip_parts(notification) {
                self.ivars().report_shown(&table);
            }
        }

        /// The table's clip view was resized: as the list was, or as legacy
        /// scrollers came or went, which the list's frame doesn't show
        /// (its vertical scroller appears once the rows are taller than
        /// it).
        #[unsafe(method(clipFrameChanged:))]
        fn clip_frame_changed(&self, notification: &NSNotification) {
            if let Some((Some(scroll), table)) = clip_parts(notification) {
                self.ivars().fit_rows(&scroll, &table);
            }
        }

        /// A column's width changed (KVO). The table posts its resize
        /// notification only once the user lets go of the divider, and the
        /// cells should follow it on the way.
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(
            &self,
            _key_path: Option<&NSString>,
            object: Option<&AnyObject>,
            _change: Option<&AnyObject>,
            _context: *mut std::ffi::c_void,
        ) {
            let Some(table) = object.and_then(|o| o.downcast_ref::<NSTableColumn>()).and_then(|c| c.tableView())
            else {
                return;
            };
            if !self.ivars().data.borrow().muted {
                self.ivars().report_widths(&table);
            }
        }

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

        #[unsafe(method_id(tableView:pasteboardWriterForRow:))]
        fn pasteboard_writer_for_row(
            &self,
            _table: &NSTableView,
            row: NSInteger,
        ) -> Option<Retained<ProtocolObject<dyn NSPasteboardWriting>>> {
            let data = self.ivars().data.borrow();
            data.rows.get(row as usize).and_then(|key| data.pasteboard_writer(*key))
        }

        /// A drag over the table: rows from this list, shown where they'd
        /// land (`RowMoves::target`), as a move.
        #[unsafe(method(tableView:validateDrop:proposedRow:proposedDropOperation:))]
        fn validate_drop(
            &self,
            table: &NSTableView,
            info: &ProtocolObject<dyn NSDraggingInfo>,
            row: NSInteger,
            operation: NSTableViewDropOperation,
        ) -> NSDragOperation {
            let data = self.ivars().data.borrow();
            let dragged = data.dragged(table, info);
            if dragged.is_empty() {
                return NSDragOperation::None;
            }
            match data.drop_target(&dragged, row, operation == NSTableViewDropOperation::On) {
                Some((_, row, on)) => {
                    let operation = if on { NSTableViewDropOperation::On } else { NSTableViewDropOperation::Above };
                    table.setDropRow_dropOperation(row, operation);
                    NSDragOperation::Move
                }
                None => NSDragOperation::None,
            }
        }

        #[unsafe(method(tableView:acceptDrop:row:dropOperation:))]
        fn accept_drop(
            &self,
            table: &NSTableView,
            info: &ProtocolObject<dyn NSDraggingInfo>,
            row: NSInteger,
            operation: NSTableViewDropOperation,
        ) -> bool {
            let ListIvars { id, events, data } = self.ivars();
            let moved = {
                let data = data.borrow();
                let dragged = data.dragged(table, info);
                data.drop_target(&dragged, row, operation == NSTableViewDropOperation::On).map(|(to, ..)| (dragged, to))
            };
            match moved {
                Some((rows, to)) if !rows.is_empty() => {
                    events.emit(*id, UiEvent::RowsMoved { rows, to });
                    true
                }
                _ => false,
            }
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
            let mut data = self.ivars().data.borrow_mut();
            let key = data.rows.get(row as usize).copied();
            let height = key.and_then(|key| data.heights.get(&key).copied());
            let estimate = if data.table { data.estimate().max(table.rowHeight()) } else { data.estimate() };
            // AppKit wants rows at least a point high.
            let height = height.unwrap_or(estimate).max(1.0);
            if let Some(key) = key {
                data.answered.insert(key, height);
            }
            height
        }

        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<NSView>> {
            let table = self.ivars().data.borrow().table;
            let cell = if table { HostView::new_cell(self.mtm()) } else { HostView::new(self.mtm()) };
            // A table's columns are named by their index; a list's one is 0.
            let column = column.and_then(|c| c.identifier().to_string().parse().ok()).unwrap_or(0);
            let mut data = self.ivars().data.borrow_mut();
            if let Some(key) = data.rows.get(row as usize).copied() {
                if let Some((_, host)) = data.hosts.get(&(key, column)) {
                    cell.addSubview(host);
                }
                data.cells.entry(key).or_default().insert(column, cell.clone());
            }
            Some(Retained::into_super(cell))
        }

        #[unsafe(method(tableViewColumnDidResize:))]
        fn column_did_resize(&self, notification: &NSNotification) {
            let Some(table) = notification.object().and_then(|o| o.downcast::<NSTableView>().ok()) else { return };
            self.ivars().report_widths(&table);
        }

        #[unsafe(method(tableView:didAddRowView:forRow:))]
        fn did_add_row_view(&self, table: &NSTableView, row_view: &NSTableRowView, row: NSInteger) {
            {
                let mut data = self.ivars().data.borrow_mut();
                let Some(key) = data.rows.get(row as usize).copied() else { return };
                data.row_views.insert(row_view as *const _ as usize, key);
            }
            self.ivars().row_views_changed(table);
        }

        #[unsafe(method(tableView:didRemoveRowView:forRow:))]
        fn did_remove_row_view(&self, table: &NSTableView, row_view: &NSTableRowView, _row: NSInteger) {
            {
                let mut data = self.ivars().data.borrow_mut();
                let Some(key) = data.row_views.remove(&(row_view as *const _ as usize)) else { return };
                data.cells.remove(&key);
            }
            self.ivars().row_views_changed(table);
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

/// The scroll view and table of a clip view's notification.
fn clip_parts(notification: &NSNotification) -> Option<(Option<Retained<NSScrollView>>, Retained<NSTableView>)> {
    let clip = notification.object().and_then(|o| o.downcast::<NSView>().ok())?;
    // SAFETY: the clip view is in its scroll view, on the main thread.
    let scroll = unsafe { clip.superview() }.and_then(|s| s.downcast::<NSScrollView>().ok());
    let table = clip.subviews().firstObject()?.downcast::<NSTableView>().ok()?;
    Some((scroll, table))
}

define_class!(
    /// A tree's data source and delegate, and the target of its double
    /// click: the list's, answering by item. Each item is its row's key, so
    /// what it reads is the tree's rows as the core sent them.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ListIvars]
    pub(crate) struct TreeSource;

    impl TreeSource {
        #[unsafe(method(clipBoundsChanged:))]
        fn clip_bounds_changed(&self, notification: &NSNotification) {
            if let Some((_, table)) = clip_parts(notification) {
                self.ivars().report_shown(&table);
            }
        }

        #[unsafe(method(clipFrameChanged:))]
        fn clip_frame_changed(&self, notification: &NSNotification) {
            if let Some((Some(scroll), table)) = clip_parts(notification) {
                self.ivars().fit_rows(&scroll, &table);
            }
        }

        #[unsafe(method(activated:))]
        fn activated(&self, sender: &AnyObject) {
            let Some(table) = sender.downcast_ref::<NSTableView>() else { return };
            self.ivars().activate(table.clickedRow());
        }
    }

    unsafe impl NSObjectProtocol for TreeSource {}

    unsafe impl NSControlTextEditingDelegate for TreeSource {}

    unsafe impl NSOutlineViewDataSource for TreeSource {
        /// The children of an open row, or the top rows: those sent. A
        /// closed row has none until it opens and the core sends them.
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn number_of_children(&self, _outline: &NSOutlineView, item: Option<&AnyObject>) -> NSInteger {
            let parent = match item {
                Some(item) => match item_key(item) {
                    Some(key) => Some(key),
                    None => return 0,
                },
                None => None,
            };
            self.ivars().data.borrow().children.get(&parent).map_or(0, |c| c.len() as NSInteger)
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _outline: &NSOutlineView, index: NSInteger, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let data = self.ivars().data.borrow();
            let parent = item.and_then(item_key);
            let child = data.children.get(&parent).and_then(|c| c.get(index as usize)).and_then(|k| data.items.get(k));
            // It only asks for the children it was told of.
            let child = child.cloned().unwrap_or_else(NSString::new);
            Retained::into_super(Retained::into_super(child))
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn is_item_expandable(&self, _outline: &NSOutlineView, item: &AnyObject) -> bool {
            let data = self.ivars().data.borrow();
            item_key(item).and_then(|key| data.tree.get(&key)).is_some_and(|r| r.expandable)
        }

        #[unsafe(method_id(outlineView:pasteboardWriterForItem:))]
        fn pasteboard_writer_for_item(
            &self,
            _outline: &NSOutlineView,
            item: &AnyObject,
        ) -> Option<Retained<ProtocolObject<dyn NSPasteboardWriting>>> {
            let data = self.ivars().data.borrow();
            item_key(item).and_then(|key| data.pasteboard_writer(key))
        }

        /// A drag over the tree: rows from this tree, shown where they'd
        /// land (`RowMoves::tree_target`), as a move.
        #[unsafe(method(outlineView:validateDrop:proposedItem:proposedChildIndex:))]
        fn validate_drop(
            &self,
            outline: &NSOutlineView,
            info: &ProtocolObject<dyn NSDraggingInfo>,
            item: Option<&AnyObject>,
            index: NSInteger,
        ) -> NSDragOperation {
            let data = self.ivars().data.borrow();
            let dragged = data.dragged(outline, info);
            if dragged.is_empty() {
                return NSDragOperation::None;
            }
            let fallback = data.tree_proposal(item.and_then(item_key), index);
            let (over, proposed) = data.pointer_proposal(outline, info, fallback);
            match data.tree_drop_target(&dragged, over, proposed) {
                Some((_, item, index)) => {
                    let item = item.and_then(|key| data.items.get(&key));
                    unsafe { outline.setDropItem_dropChildIndex(item.map(|i| i as &AnyObject), index) };
                    NSDragOperation::Move
                }
                None => NSDragOperation::None,
            }
        }

        #[unsafe(method(outlineView:acceptDrop:item:childIndex:))]
        fn accept_drop(
            &self,
            outline: &NSOutlineView,
            info: &ProtocolObject<dyn NSDraggingInfo>,
            item: Option<&AnyObject>,
            index: NSInteger,
        ) -> bool {
            let ListIvars { id, events, data } = self.ivars();
            let moved = {
                let data = data.borrow();
                let dragged = data.dragged(outline, info);
                let fallback = data.tree_proposal(item.and_then(item_key), index);
                let (over, proposed) = data.pointer_proposal(outline, info, fallback);
                data.tree_drop_target(&dragged, over, proposed).map(|(to, ..)| (dragged, to))
            };
            match moved {
                Some((rows, to)) if !rows.is_empty() => {
                    events.emit(*id, UiEvent::RowsMoved { rows, to });
                    true
                }
                _ => false,
            }
        }
    }

    unsafe impl NSOutlineViewDelegate for TreeSource {
        #[unsafe(method(outlineView:heightOfRowByItem:))]
        fn height_of_row(&self, _outline: &NSOutlineView, item: &AnyObject) -> f64 {
            let mut data = self.ivars().data.borrow_mut();
            let key = item_key(item);
            // AppKit wants rows at least a point high.
            let height = key.and_then(|key| data.heights.get(&key).copied()).unwrap_or(data.estimate()).max(1.0);
            if let Some(key) = key {
                data.answered.insert(key, height);
            }
            height
        }

        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for_item(
            &self,
            _outline: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            let cell = HostView::new(self.mtm());
            let mut data = self.ivars().data.borrow_mut();
            if let Some(key) = item_key(item) {
                if let Some((_, host)) = data.hosts.get(&(key, 0)) {
                    cell.addSubview(host);
                }
                data.cells.entry(key).or_default().insert(0, cell.clone());
            }
            Some(Retained::into_super(cell))
        }

        /// By its item, not its index: while rows open and close, the
        /// outline view's indexes run ahead of the rows the list has.
        #[unsafe(method(outlineView:didAddRowView:forRow:))]
        fn did_add_row_view(&self, outline: &NSOutlineView, row_view: &NSTableRowView, row: NSInteger) {
            let Some(key) = outline.itemAtRow(row).and_then(|item| item_key(&item)) else { return };
            let loading = {
                let mut data = self.ivars().data.borrow_mut();
                data.row_views.insert(row_view as *const _ as usize, key);
                data.loading.as_ref().is_some_and(|loading| loading.contains(&key))
            };
            // A row view the table reuses has the spinner of the row it had.
            if let Some(outline) = outline.downcast_ref::<TreeTable>() {
                show_spinner(outline, row_view, row, loading);
            }
            self.ivars().row_views_changed(outline);
        }

        #[unsafe(method(outlineView:didRemoveRowView:forRow:))]
        fn did_remove_row_view(&self, outline: &NSOutlineView, row_view: &NSTableRowView, _row: NSInteger) {
            {
                let mut data = self.ivars().data.borrow_mut();
                let Some(key) = data.row_views.remove(&(row_view as *const _ as usize)) else { return };
                data.cells.remove(&key);
            }
            self.ivars().row_views_changed(outline);
        }

        #[unsafe(method(outlineView:shouldSelectItem:))]
        fn should_select_item(&self, _outline: &NSOutlineView, _item: &AnyObject) -> bool {
            self.ivars().data.borrow().mode != SelectionMode::None
        }

        /// By the items: closing a row deselects its children's rows
        /// before the list's rows follow.
        #[unsafe(method(outlineViewSelectionDidChange:))]
        fn selection_did_change(&self, notification: &NSNotification) {
            let Some(outline) = notification.object().and_then(|o| o.downcast::<NSOutlineView>().ok()) else {
                return;
            };
            let ListIvars { id, events, data } = self.ivars();
            if !data.borrow().muted {
                let keys = outline_keys(&outline, &outline.selectedRowIndexes());
                events.emit(*id, UiEvent::Changed(EventValue::Rows(keys)));
            }
        }

        /// Row views come and go as a row opens or closes: they're
        /// reported once it's done, with the rows it shows then.
        #[unsafe(method(outlineViewItemWillExpand:))]
        fn item_will_expand(&self, _notification: &NSNotification) {
            self.ivars().data.borrow_mut().laying_out += 1;
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn item_did_expand(&self, notification: &NSNotification) {
            self.ivars().expanded(notification, true);
        }

        #[unsafe(method(outlineViewItemWillCollapse:))]
        fn item_will_collapse(&self, _notification: &NSNotification) {
            self.ivars().data.borrow_mut().laying_out += 1;
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn item_did_collapse(&self, notification: &NSNotification) {
            self.ivars().expanded(notification, false);
        }
    }
);

/// How far past the visible rows a list builds rows, in screenfuls on
/// each side, and how far it keeps rows it built. AppKit's table makes row
/// views only for what's in view, which a fast scroll replaces all at
/// once, so they'd show empty until built; GTK's, Qt's and WinUI's lists
/// keep rows past the view themselves.
const AHEAD: usize = 2;
const KEPT: usize = 4;

impl ListIvars {
    /// Sizes a list's one column to the width its rows get, and reports it
    /// when it changes: legacy scroll bars and a border take room from the
    /// rows. A table sizes its columns itself, and reports their widths.
    fn fit_rows(&self, scroll: &NSScrollView, table: &NSTableView) {
        if self.data.borrow().table {
            self.report_widths(table);
            return;
        }
        let Some(column) = table.tableColumns().firstObject() else { return };
        let width = scroll.contentSize().width;
        column.setWidth(width);
        let width = width as f32;
        if self.data.borrow_mut().row_width.replace(width) != Some(width) {
            self.events.emit(self.id, UiEvent::RowWidth(width));
        }
    }

    /// A row view came or went: reported now, or once the table's layout
    /// that adds and removes them is done. A scroll jump replaces every
    /// row view in one pass, and reporting after each made it quadratic.
    fn row_views_changed(&self, table: &NSTableView) {
        {
            let mut data = self.data.borrow_mut();
            if data.laying_out > 0 {
                data.shown_stale = true;
                return;
            }
        }
        self.report_shown(table);
    }

    /// Runs the table's own layout (or preparing content past its view),
    /// reporting the rows it showed or let go once, at the end.
    fn batching_row_views(&self, table: &NSTableView, layout: impl FnOnce()) {
        self.data.borrow_mut().laying_out += 1;
        layout();
        let stale = {
            let mut data = self.data.borrow_mut();
            data.laying_out -= 1;
            data.laying_out == 0 && std::mem::take(&mut data.shown_stale)
        };
        if stale {
            self.report_shown(table);
        }
    }

    /// Reports the rows shown and let go since the last report: the rows
    /// the table has row views for, the rows within `AHEAD` screenfuls of
    /// the view, and the rows shown before that are still within `KEPT`.
    fn report_shown(&self, table: &NSTableView) {
        if self.data.borrow().reloading {
            return;
        }
        // Counted in rows, not points: asking where rows out of view are
        // has the table measure them, which moves a scroll under way.
        let visible = table.rowsInRect(table.visibleRect());
        let around = |pages: usize| {
            let margin = visible.length * pages;
            visible.location.saturating_sub(margin)..visible.location + visible.length + margin
        };
        let (ahead, kept) = (around(AHEAD), around(KEPT));
        let ListIvars { id, events, data } = self;
        let mut data = data.borrow_mut();
        let index = |key: &RowKey| data.index.get(key).copied();
        let mut now: HashSet<RowKey> = data.row_views.values().copied().collect();
        now.extend(data.rows.get(ahead.start.min(data.rows.len())..ahead.end.min(data.rows.len())).unwrap_or(&[]));
        now.extend(data.shown.iter().filter(|k| index(k).is_some_and(|i| kept.contains(&i))));
        let mut hidden: Vec<RowKey> = data.shown.difference(&now).copied().collect();
        let mut shown: Vec<RowKey> = now.difference(&data.shown).copied().collect();
        hidden.sort_by_key(|k| data.index.get(k).copied());
        shown.sort_by_key(|k| data.index.get(k).copied());
        for row in hidden {
            // Shown again, its height is reported again.
            data.bare_heights.remove(&row);
            events.emit(*id, UiEvent::RowHidden(row));
        }
        for row in shown {
            events.emit(*id, UiEvent::RowShown(row));
        }
        data.shown = now;
    }

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

    /// A tree's row opened or closed: the user's (its disclosure, the arrow
    /// keys; ⌥ opens its folders too, each reported) or the backend's. The
    /// rows follow the outline view's, the row views it added and removed
    /// meanwhile are reported once, and the user's is reported.
    fn expanded(&self, notification: &NSNotification, open: bool) {
        let Some(outline) = notification.object().and_then(|o| o.downcast::<NSOutlineView>().ok()) else { return };
        let item = notification.userInfo().and_then(|info| info.objectForKey(&NSString::from_str("NSObject")));
        self.sync_rows(&outline);
        let stale = {
            let mut data = self.data.borrow_mut();
            data.laying_out = data.laying_out.saturating_sub(1);
            data.laying_out == 0 && std::mem::take(&mut data.shown_stale)
        };
        if stale {
            self.report_shown(&outline);
        }
        let expanding = self.data.borrow().expanding;
        if let (false, Some(key)) = (expanding, item.and_then(|item| item_key(&item))) {
            self.events.emit(self.id, UiEvent::RowExpanded(key, open));
        }
    }

    /// A tree's rows as its outline view shows them: the user opening and
    /// closing rows changes them before the core sends them.
    fn sync_rows(&self, outline: &NSOutlineView) {
        let rows: Vec<RowKey> =
            (0..outline.numberOfRows()).filter_map(|i| outline.itemAtRow(i).and_then(|item| item_key(&item))).collect();
        let mut data = self.data.borrow_mut();
        data.index = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
        data.rows = rows;
    }

    /// Reports how far a tree indents its rows' content, when it changes:
    /// where the outline column's cell of a top row starts (the
    /// disclosure's room), and the outline view's indentation per level.
    fn report_indent(&self, outline: &NSOutlineView) {
        if outline.numberOfRows() == 0 {
            return;
        }
        let row = outline.rectOfRow(0);
        let cell = outline.frameOfCellAtColumn_row(0, 0);
        let start = (cell.origin.x - row.origin.x) as f32;
        let indent = TreeIndent { start, per_level: outline.indentationPerLevel() as f32 };
        let mut data = self.data.borrow_mut();
        if data.indent != Some(indent) {
            data.indent = Some(indent);
            drop(data);
            self.events.emit(self.id, UiEvent::TreeIndent(indent));
        }
    }

    /// Whether the app gave the list this key (`Prop::Keys`).
    fn takes(&self, shortcut: Shortcut) -> bool {
        self.data.borrow().keys.as_ref().is_some_and(|keys| keys.contains(&shortcut))
    }

    /// Reports the shortcut if the app gave the list it.
    fn take(&self, shortcut: Shortcut) -> bool {
        let taken = self.takes(shortcut);
        if taken {
            self.events.emit(self.id, UiEvent::Key(shortcut));
        }
        taken
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
        #[unsafe(method(layout))]
        fn layout(&self) {
            self.ivars().batching_row_views(self, || unsafe { msg_send![super(self), layout] });
        }

        /// Where responsive scrolling adds rows past the view.
        #[unsafe(method(prepareContentInRect:))]
        fn prepare_content_in_rect(&self, rect: NSRect) {
            self.ivars().batching_row_views(self, || unsafe { msg_send![super(self), prepareContentInRect: rect] });
        }

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
            row_menu(self, event).or_else(|| unsafe { msg_send![super(self), menuForEvent: event] })
        }

    }
);

define_class!(
    /// A tree's outline view, with the list table's own handling: rows
    /// built past the view, Return, and its rows' menus. Its arrows open
    /// and close rows, as any outline view's.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ListIvars]
    pub(crate) struct TreeTable;

    impl TreeTable {
        #[unsafe(method(layout))]
        fn layout(&self) {
            self.ivars().batching_row_views(self, || unsafe { msg_send![super(self), layout] });
        }

        #[unsafe(method(prepareContentInRect:))]
        fn prepare_content_in_rect(&self, rect: NSRect) {
            self.ivars().batching_row_views(self, || unsafe { msg_send![super(self), prepareContentInRect: rect] });
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let held = NSEventModifierFlags::Command | NSEventModifierFlags::Option | NSEventModifierFlags::Control;
            if matches!(event.keyCode(), 36 | 76) && !event.modifierFlags().intersects(held) && self.selectedRow() >= 0 {
                self.ivars().activate(self.selectedRow());
                return;
            }
            unsafe { msg_send![super(self), keyDown: event] }
        }

        #[unsafe(method_id(menuForEvent:))]
        fn menu_for_event(&self, event: &NSEvent) -> Option<Retained<NSMenu>> {
            row_menu(self, event).or_else(|| unsafe { msg_send![super(self), menuForEvent: event] })
        }
    }
);

/// A row view's disclosure button, which the outline view adds to the
/// rows that open.
fn disclosure_button(row_view: &NSTableRowView) -> Option<Retained<NSView>> {
    let key = unsafe { NSOutlineViewDisclosureButtonKey };
    row_view.subviews().iter().find(|view| view.identifier().is_some_and(|id| &*id == key))
}

/// A tree's row's spinner, in place of its disclosure while its children
/// are being read, as Finder shows one in a folder it opens. The outline
/// view has none of its own. The disclosure is hidden under it, not given
/// no frame: the outline view keeps a disclosure's frame from its last
/// layout of the row, and dropped the button laid out with none, so the
/// row couldn't close until another opened.
fn show_spinner(outline: &TreeTable, row_view: &NSTableRowView, row: NSInteger, loading: bool) {
    let spinner = row_view.subviews().iter().find_map(|view| view.downcast::<NSProgressIndicator>().ok());
    match (spinner, loading) {
        (Some(spinner), false) => spinner.removeFromSuperview(),
        (None, true) => {
            let frame = outline.frameOfOutlineCellAtRow(row);
            if frame.size.width <= 0.0 {
                return;
            }
            let spinner = NSProgressIndicator::new(outline.mtm());
            spinner.setStyle(NSProgressIndicatorStyle::Spinning);
            spinner.setControlSize(NSControlSize::Small);
            spinner.setIndeterminate(true);
            spinner.setDisplayedWhenStopped(false);
            spinner.sizeToFit();
            let size = spinner.frame().size;
            let room = row_view.convertRect_fromView(frame, Some(outline));
            spinner.setFrameOrigin(NSPoint::new(
                (room.origin.x + (room.size.width - size.width) / 2.0).round(),
                (room.origin.y + (room.size.height - size.height) / 2.0).round(),
            ));
            row_view.addSubview(&spinner);
            unsafe { spinner.startAnimation(None) };
        }
        _ => {}
    }
    let spinning = row_view.subviews().iter().any(|view| view.downcast_ref::<NSProgressIndicator>().is_some());
    if let Some(button) = disclosure_button(row_view) {
        button.setHidden(spinning);
    }
}

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

/// The list's state of a list's or tree's table view, if `view` is one.
fn ivars_of(view: &NSView) -> Option<&ListIvars> {
    if let Some(table) = view.downcast_ref::<ListTable>() {
        return Some(table.ivars());
    }
    view.downcast_ref::<TreeTable>().map(|tree| tree.ivars())
}

/// Whether `view` is a list's table that the app gave this key
/// (`Prop::Keys`).
pub(crate) fn takes(view: &NSView, shortcut: Shortcut) -> bool {
    ivars_of(view).is_some_and(|ivars| ivars.takes(shortcut))
}

/// Reports the shortcut if `view` is a list's table that the app gave it.
pub(crate) fn take(view: &NSView, shortcut: Shortcut) -> bool {
    ivars_of(view).is_some_and(|ivars| ivars.take(shortcut))
}

/// The menu of the view under the pointer in a row, or of the nearest
/// view around it. The table's `hitTest:` answers itself over its rows'
/// labels, so the search starts from the cell.
fn row_menu(table: &NSTableView, event: &NSEvent) -> Option<Retained<NSMenu>> {
    let point = table.convertPoint_fromView(event.locationInWindow(), None);
    let (row, column) = (table.rowAtPoint(point), table.columnAtPoint(point));
    if row < 0 || column < 0 {
        return None;
    }
    let cell = table.viewAtColumn_row_makeIfNecessary(column, row, false)?;
    // `hitTest:` takes a point in the superview's coordinates.
    let around = unsafe { cell.superview() }?;
    let mut view = cell.hitTest(around.convertPoint_fromView(point, Some(table)));
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

/// A change of rows as the table takes it, in order: rows removed (by
/// their old index), rows moved (each by where it is at that point, to
/// where it goes), then rows inserted (by their new index). The table
/// keeps the row views, heights and selection of the rows that stay, where
/// a reload asks every row's height again and makes every row view anew.
struct RowChange {
    removed: Vec<usize>,
    moved: Vec<(usize, usize)>,
    inserted: Vec<usize>,
    /// The moved rows' new indexes, whose heights the table is told of.
    moved_to: Vec<usize>,
}

/// Past this many rows changed, and half the list, the table reloads, as
/// WinUI's lists replace their items: one pass beats many small ones.
const RELOAD_PAST: usize = 64;
/// Past this many moves (each a call, and a walk here) the table reloads.
const MOVES_PAST: usize = 64;

impl RowChange {
    /// The change from `old` to `new`, or `None` for a reload: a first
    /// fill, or a change too large. Keys are unique in each.
    fn between(
        old: &[RowKey],
        old_index: &HashMap<RowKey, usize>,
        new: &[RowKey],
        new_index: &HashMap<RowKey, usize>,
    ) -> Option<RowChange> {
        if old.is_empty() || new.is_empty() || old_index.len() != old.len() || new_index.len() != new.len() {
            return None;
        }
        let removed: Vec<usize> = (0..old.len()).filter(|i| !new_index.contains_key(&old[*i])).collect();
        let inserted: Vec<usize> = (0..new.len()).filter(|i| !old_index.contains_key(&new[*i])).collect();
        let changed = removed.len() + inserted.len();
        if changed > RELOAD_PAST && changed * 2 > new.len() {
            return None;
        }
        // The rows that stay, as they were and as they'll be. The longest
        // run of them already in order stays put; the others move, each
        // just after the row before it in the new order.
        let mut working: Vec<RowKey> = old.iter().copied().filter(|k| new_index.contains_key(k)).collect();
        let kept: Vec<RowKey> = new.iter().copied().filter(|k| old_index.contains_key(k)).collect();
        let still = in_order(&kept.iter().map(|k| old_index[k]).collect::<Vec<_>>());
        if kept.len() - still.len() > MOVES_PAST {
            return None;
        }
        let still: HashSet<RowKey> = still.into_iter().map(|i| kept[i]).collect();
        let (mut moved, mut moved_to) = (Vec::new(), Vec::new());
        for (i, key) in kept.iter().enumerate() {
            if still.contains(key) {
                continue;
            }
            let from = working.iter().position(|k| k == key)?;
            let after = match i {
                0 => 0,
                _ => working.iter().position(|k| *k == kept[i - 1])? + 1,
            };
            let to = if from < after { after - 1 } else { after };
            if from != to {
                working.remove(from);
                working.insert(to, *key);
                moved.push((from, to));
                moved_to.push(new_index[key]);
            }
        }
        debug_assert_eq!(working, kept);
        Some(RowChange { removed, moved, inserted, moved_to })
    }

    /// Tells the table, which asks for the new rows' heights and views.
    fn apply(&self, table: &NSTableView) {
        let none = NSTableViewAnimationOptions::EffectNone;
        table.beginUpdates();
        if !self.removed.is_empty() {
            table.removeRowsAtIndexes_withAnimation(&index_set(self.removed.iter().copied()), none);
        }
        for (from, to) in &self.moved {
            table.moveRowAtIndex_toIndex(*from as NSInteger, *to as NSInteger);
        }
        if !self.inserted.is_empty() {
            table.insertRowsAtIndexes_withAnimation(&index_set(self.inserted.iter().copied()), none);
        }
        table.endUpdates();
        if !self.moved_to.is_empty() {
            table.noteHeightOfRowsWithIndexesChanged(&index_set(self.moved_to.iter().copied()));
        }
    }
}

/// The positions in `values` of a longest increasing run of them
/// (patience sorting, O(n log n)).
fn in_order(values: &[usize]) -> Vec<usize> {
    // `tails[l]`: the position of the smallest value ending a run of l + 1.
    let mut tails: Vec<usize> = Vec::new();
    let mut before: Vec<Option<usize>> = vec![None; values.len()];
    for (i, value) in values.iter().enumerate() {
        let l = tails.partition_point(|t| values[*t] < *value);
        before[i] = l.checked_sub(1).map(|l| tails[l]);
        if l == tails.len() {
            tails.push(i);
        } else {
            tails[l] = i;
        }
    }
    let mut run = Vec::with_capacity(tails.len());
    let mut at = tails.last().copied();
    while let Some(i) = at {
        run.push(i);
        at = before[i];
    }
    run.reverse();
    run
}

/// A list's native parts.
pub(crate) struct List {
    pub scroll: Retained<crate::classes::EdgeScrollView>,
    /// The table view: a list's or table's `ListTable`, or a tree's
    /// `TreeTable`, an outline view.
    pub table: Retained<NSTableView>,
    /// A tree's outline view: the table, as one.
    outline: Option<Retained<TreeTable>>,
    /// The table's data source and delegate: a `ListSource`, or a tree's
    /// `TreeSource`.
    source: Retained<NSObject>,
    /// The table's next responder, which the table doesn't retain.
    _keys: Retained<ListKeys>,
    ivars: ListIvars,
    data: SharedList,
    /// AppKit can't tell `Automatic` from `Plain`.
    style: Cell<Option<ListStyle>>,
    /// The natural height last measured.
    natural: Cell<f64>,
}

/// A list's one column, with no header: rows exactly as high as their
/// hosts, one right below the other.
fn one_column(mtm: MainThreadMarker, table: &NSTableView) -> Retained<NSTableColumn> {
    table.setHeaderView(None);
    table.setStyle(NSTableViewStyle::Plain);
    table.setIntercellSpacing(NSSize::new(0.0, 0.0));
    table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &NSString::from_str("row"));
    column.setResizingMask(NSTableColumnResizingOptions::AutoresizingMask);
    table.addTableColumn(&column);
    column
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
        if columns {
            // The table's own header, style and spacing; the columns that
            // expand share its width.
            if table.headerView().is_none() {
                table.setHeaderView(Some(&objc2_app_kit::NSTableHeaderView::new(mtm)));
            }
            table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::UniformColumnAutoresizingStyle);
        } else {
            one_column(mtm, &table);
        }
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            table.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        }
        let (table, source) = (Retained::into_super(table), Retained::into_super(source));
        List::finish(mtm, table, None, source, ivars(), columns)
    }

    /// A tree: an outline view whose one column is its outline column, which
    /// it indents and puts the disclosures in.
    pub(crate) fn new_tree(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> List {
        let data = SharedList::default();
        let ivars = || ListIvars { id, events: events.clone(), data: data.clone() };
        let source: Retained<TreeSource> = unsafe { msg_send![super(TreeSource::alloc(mtm).set_ivars(ivars())), init] };
        let outline: Retained<TreeTable> =
            unsafe { msg_send![super(TreeTable::alloc(mtm).set_ivars(ivars())), initWithFrame: zero_rect()] };
        let column = one_column(mtm, &outline);
        unsafe { outline.setOutlineTableColumn(Some(&column)) };
        // The column is as wide as the rows, which `fit_rows` keeps; the
        // outline view would widen it to its deepest row.
        outline.setAutoresizesOutlineColumn(false);
        unsafe {
            outline.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            outline.setDelegate(Some(ProtocolObject::from_ref(&*source)));
        }
        let table = Retained::into_super(Retained::into_super(outline.clone()));
        List::finish(mtm, table, Some(outline), Retained::into_super(source), ivars(), false)
    }

    /// What lists, tables and trees share: selection, dragging, activation,
    /// the scroll view, and the keys.
    fn finish(
        mtm: MainThreadMarker,
        table: Retained<NSTableView>,
        outline: Option<Retained<TreeTable>>,
        source: Retained<NSObject>,
        ivars: ListIvars,
        columns: bool,
    ) -> List {
        table.setUsesAutomaticRowHeights(false);
        table.setAllowsEmptySelection(true);
        // Rows dragged out (`Prop::RowFiles`) are copied, as from apps that
        // aren't the file manager; the table offers nothing outside the app
        // by default.
        table.setDraggingSourceOperationMask_forLocal(NSDragOperation::Copy, false);
        table.setDraggingSourceOperationMask_forLocal(NSDragOperation::Copy, true);
        unsafe {
            table.setTarget(Some(&source));
            table.setDoubleAction(Some(sel!(activated:)));
        }
        let scroll = crate::classes::EdgeScrollView::new(mtm);
        scroll.setHasVerticalScroller(true);
        // A table's columns can be wider than it.
        scroll.setHasHorizontalScroller(columns);
        scroll.setAutohidesScrollers(true);
        // The core places the list; AppKit mustn't inset its content for
        // the title bar on top of that.
        scroll.setAutomaticallyAdjustsContentInsets(false);
        scroll.setDocumentView(Some(&table));
        let clip = scroll.contentView();
        clip.setPostsBoundsChangedNotifications(true);
        clip.setPostsFrameChangedNotifications(true);
        unsafe {
            let center = NSNotificationCenter::defaultCenter();
            center.addObserver_selector_name_object(
                &source,
                sel!(clipBoundsChanged:),
                Some(NSViewBoundsDidChangeNotification),
                Some(&clip),
            );
            center.addObserver_selector_name_object(
                &source,
                sel!(clipFrameChanged:),
                Some(NSViewFrameDidChangeNotification),
                Some(&clip),
            );
        }
        // Keys the table passes on reach the list's own keys first. The
        // table's superview (its next responder) stays put from here.
        let keys_ivars = ListIvars { id: ivars.id, events: ivars.events.clone(), data: ivars.data.clone() };
        let keys: Retained<ListKeys> = unsafe { msg_send![super(ListKeys::alloc(mtm).set_ivars(keys_ivars)), init] };
        // SAFETY: both responders live as long as the list, which keeps
        // `keys` (a next responder isn't retained).
        unsafe {
            keys.setNextResponder(table.nextResponder().as_deref());
            table.setNextResponder(Some(&keys));
        }
        let data = ivars.data.clone();
        List {
            scroll,
            table,
            outline,
            source,
            _keys: keys,
            ivars,
            data,
            style: Cell::new(None),
            natural: Cell::new(0.0),
        }
    }

    /// A tree's rows that show, each with its place in the tree. The data
    /// source gives each open row's children as sent, and none for a closed
    /// row; the outline view reloads, then, from the top, each row is opened
    /// or closed as sent, which shows or drops its children's rows, so the
    /// outline view shows the rows in their order. Rows that stay keep their
    /// hosts, and only the difference in rows shown is reported.
    pub(crate) fn set_tree_rows(&self, rows: Vec<TreeRow>) {
        let Some(outline) = &self.outline else { return };
        let selected = self.selected();
        {
            let mut data = self.data.borrow_mut();
            let index: HashMap<RowKey, usize> = rows.iter().enumerate().map(|(i, r)| (r.row, i)).collect();
            data.heights.retain(|key, _| index.contains_key(key));
            // The outline view asks again as it reloads.
            data.answered.clear();
            data.children.clear();
            for row in &rows {
                data.children.entry(row.parent).or_default().push(row.row);
            }
            data.items.retain(|key, _| index.contains_key(key));
            for row in &rows {
                data.items.entry(row.row).or_insert_with(|| NSString::from_str(&row.row.0.to_string()));
            }
            data.tree = rows.iter().map(|r| (r.row, *r)).collect();
            data.index = index;
            data.rows = rows.iter().map(|r| r.row).collect();
            data.muted = true;
            data.reloading = true;
            data.expanding = true;
        }
        without_animation(|| {
            outline.reloadData();
            for (position, row) in rows.iter().enumerate() {
                let Some(item) = outline.itemAtRow(position as NSInteger) else { break };
                if item_key(&item) != Some(row.row) {
                    break;
                }
                // SAFETY: the item is one of the outline view's own.
                unsafe {
                    match (outline.isItemExpanded(Some(&item)), row.expanded) {
                        (false, true) => outline.expandItem(Some(&item)),
                        (true, false) => outline.collapseItem(Some(&item)),
                        _ => {}
                    }
                }
            }
        });
        self.ivars.sync_rows(outline);
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
            data.expanding = false;
        }
        if kept != selected {
            self.report_selection();
        }
        self.ivars.report_shown(&self.table);
        self.ivars.report_indent(outline);
    }

    /// Opens or closes a tree's row as assistive technology does (the
    /// outline view's `AXDisclosing`), reporting it. Closing it drops its
    /// children's rows; opening it shows them once the core sends them.
    /// False for a row that doesn't open.
    pub(crate) fn expand(&self, key: RowKey, open: bool) -> bool {
        let Some(outline) = &self.outline else { return false };
        let Some(item) = self.data.borrow().items.get(&key).cloned() else { return false };
        // SAFETY: the item is one of the outline view's own.
        unsafe {
            if !outline.isExpandable(Some(&item)) {
                return false;
            }
            if outline.isItemExpanded(Some(&item)) != open {
                self.data.borrow_mut().expanding = true;
                without_animation(|| match open {
                    true => outline.expandItem(Some(&item)),
                    false => outline.collapseItem(Some(&item)),
                });
                self.data.borrow_mut().expanding = false;
                self.ivars.events.emit(self.ivars.id, UiEvent::RowExpanded(key, open));
            }
        }
        true
    }

    /// The rows whose children are being read: their spinners show in
    /// place of their disclosures, in the row views there now and in those
    /// added later.
    pub(crate) fn set_tree_loading(&self, rows: &[RowKey]) {
        let Some(outline) = &self.outline else { return };
        let loading: HashSet<RowKey> = rows.iter().copied().collect();
        self.data.borrow_mut().loading = Some(loading.clone());
        for row in 0..outline.numberOfRows() {
            let (Some(key), Some(row_view)) = (
                outline.itemAtRow(row).and_then(|item| item_key(&item)),
                outline.rowViewAtRow_makeIfNecessary(row, false),
            ) else {
                continue;
            };
            show_spinner(outline, &row_view, row, loading.contains(&key));
        }
    }

    /// The rows whose spinners show: of those sent, the ones whose row view
    /// has its spinner, and those without a row view, which get it when
    /// they're shown.
    pub(crate) fn tree_loading(&self) -> Option<Vec<RowKey>> {
        let outline = self.outline.as_ref()?;
        let loading = self.data.borrow().loading.clone()?;
        let shows = |row: NSInteger| match outline.rowViewAtRow_makeIfNecessary(row, false) {
            Some(row_view) => {
                row_view.subviews().iter().any(|view| view.downcast_ref::<NSProgressIndicator>().is_some())
            }
            None => true,
        };
        Some(
            (0..outline.numberOfRows())
                .filter_map(|row| Some((row, outline.itemAtRow(row).and_then(|item| item_key(&item))?)))
                .filter(|(row, key)| loading.contains(key) && shows(*row))
                .map(|(_, key)| key)
                .collect(),
        )
    }

    /// A tree's rows as its outline view shows them: each with its parent,
    /// depth, and whether it opens and is open.
    pub(crate) fn tree_rows(&self) -> Vec<TreeRow> {
        let Some(outline) = &self.outline else { return Vec::new() };
        (0..outline.numberOfRows())
            .filter_map(|i| {
                let item = outline.itemAtRow(i)?;
                // SAFETY: the item is one of the outline view's own.
                unsafe {
                    Some(TreeRow {
                        row: item_key(&item)?,
                        parent: outline.parentForItem(Some(&item)).and_then(|p| item_key(&p)),
                        depth: outline.levelForRow(i).max(0) as u32,
                        expandable: outline.isExpandable(Some(&item)),
                        expanded: outline.isItemExpanded(Some(&item)),
                    })
                }
            })
            .collect()
    }

    /// New rows: the table takes the change (rows removed, moved and
    /// inserted, or a reload for a large one), keeps the selected rows that
    /// stayed (reporting it if some went), and reports the rows it shows
    /// now.
    pub(crate) fn set_rows(&self, rows: Vec<RowKey>) {
        let selected = self.selected();
        let change = {
            let mut data = self.data.borrow_mut();
            let index: HashMap<RowKey, usize> = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
            let change = RowChange::between(&data.rows, &data.index, &rows, &index);
            data.heights.retain(|key, _| index.contains_key(key));
            data.cell_heights.retain(|(key, _), _| index.contains_key(key));
            data.bare_heights.retain(|key, _| index.contains_key(key));
            match &change {
                // The table asks again as it reloads.
                None => data.answered.clear(),
                // It keeps the heights of the rows that stay.
                Some(_) => data.answered.retain(|key, _| index.contains_key(key)),
            }
            data.index = index;
            data.rows = rows;
            data.muted = true;
            data.reloading = true;
            change
        };
        match change {
            Some(change) => without_animation(|| change.apply(&self.table)),
            None => self.table.reloadData(),
        }
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
        self.ivars.report_shown(&self.table);
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
        let ListIvars { id, events, .. } = &self.ivars;
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
        self.set_frame(self.scroll.alignmentRectForFrame(self.scroll.frame()));
    }

    pub(crate) fn style(&self) -> Option<ListStyle> {
        self.style.get()
    }

    /// Hosts a mounted row, or a table's cell, in its cell if the table
    /// shows it.
    pub(crate) fn insert(&self, key: RowKey, column: usize, id: NodeId, view: Retained<NSView>) {
        let mut data = self.data.borrow_mut();
        if let Some(cell) = data.cells.get(&key).and_then(|cells| cells.get(&column)) {
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
                if let Some(cell) = data.cells.get(&key).and_then(|cells| cells.get(&column)) {
                    cell.center_subviews();
                }
                // The cells that fill the row follow the others.
                if let Some(bare) = data.bare_height(&self.table, key)
                    && data.bare_heights.insert(key, bare) != Some(bare)
                {
                    let ListIvars { id, events, .. } = &self.ivars;
                    events.emit(*id, UiEvent::RowHeight(key, bare as f32));
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
            } else if !data.row_views.values().any(|k| *k == key)
                && data.answered.get(&key).is_none_or(|answered| *answered == height.max(1.0))
            {
                // Out of view (see `AHEAD`), and the table has it right or
                // will ask when it needs it: told about it, it places the
                // rows again, which moves what's in view.
                return;
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
            clip.scrollToPoint(NSPoint::new(clip.bounds().origin.x, end));
            self.scroll.reflectScrolledClipView(&clip);
        }
    }

    /// How high the list is with all its rows: each as high as the table
    /// is told it is (its host's, or the estimate), the spacing between
    /// them, a table's header, and a framed list's border. Counted from the
    /// heights, not asked of the table: asking where rows out of view are
    /// has it measure them, which moves a scroll under way.
    pub(crate) fn natural_height(&self) -> f32 {
        let content = {
            let data = self.data.borrow();
            let least = if data.table { self.table.rowHeight() } else { 0.0 };
            let estimate = data.estimate().max(least);
            let rows: f64 =
                data.rows.iter().map(|key| data.heights.get(key).copied().unwrap_or(estimate).max(1.0)).sum();
            rows + self.table.intercellSpacing().height * data.rows.len() as f64
        };
        let header = self.table.headerView().map_or(0.0, |header| header.frame().size.height);
        // Without scroller classes: rows that fit need no scroll bar.
        let size = unsafe {
            NSScrollView::frameSizeForContentSize_horizontalScrollerClass_verticalScrollerClass_borderType_controlSize_scrollerStyle(
                NSSize::new(0.0, content + header),
                None,
                None,
                self.scroll.borderType(),
                objc2_app_kit::NSControlSize::Regular,
                self.scroll.scrollerStyle(),
                MainThreadMarker::from(&*self.table),
            )
        };
        // The core's frames are alignment rects.
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
        self.scroll.alignmentRectForFrame(frame).size.height.ceil() as f32
    }

    /// Reports `Remeasure` when the list's natural height changed.
    pub(crate) fn remeasure(&self) {
        let natural = self.natural_height() as f64;
        if self.natural.replace(natural) != natural {
            let ListIvars { id, events, .. } = &self.ivars;
            events.emit(*id, UiEvent::Remeasure);
        }
    }

    /// Sizes the list, and its rows (`ListIvars::fit_rows`). Its frame
    /// may run up under the window's bar, outside where the core put it.
    pub(crate) fn set_frame(&self, frame: objc2_foundation::NSRect) {
        self.scroll.setFrame(self.scroll.frameForAlignmentRect(frame));
        self.ivars.fit_rows(&self.scroll, &self.table);
    }

    /// A table's columns: their titles, the widths they start at, the ones
    /// that take the room left (the uniform style shares it among those
    /// that autoresize), and a sort descriptor for the sortable ones,
    /// which makes their headers sort. Each is named by its index.
    pub(crate) fn set_columns(&self, mtm: MainThreadMarker, columns: &[ColumnData]) {
        let sort = sort_of(&self.table);
        self.data.borrow_mut().muted = true;
        for column in self.table.tableColumns().iter() {
            self.unwatch(&column);
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
            unsafe {
                column.addObserver_forKeyPath_options_context(
                    &self.source,
                    &NSString::from_str(WIDTH),
                    NSKeyValueObservingOptions::New,
                    std::ptr::null_mut(),
                );
            }
        }
        self.data.borrow_mut().columns = columns.to_vec();
        self.table.sizeToFit();
        self.set_sort(sort.filter(|s| columns.get(s.column).is_some_and(|c| c.sortable)));
        self.data.borrow_mut().muted = false;
        self.ivars.report_widths(&self.table);
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
            self.ivars.activate(index as NSInteger);
        }
    }

    fn unwatch(&self, column: &NSTableColumn) {
        if self.data.borrow().table {
            unsafe { column.removeObserver_forKeyPath(&self.source, &NSString::from_str(WIDTH)) };
        }
    }

    /// Stops the table from calling its data source, which goes with the list.
    pub(crate) fn detach(&self) {
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(&self.source) };
        for column in self.table.tableColumns().iter() {
            self.unwatch(&column);
        }
        unsafe {
            self.table.setDataSource(None);
            self.table.setDelegate(None);
            self.table.setTarget(None);
            // The table doesn't retain `_keys`, which goes with the list:
            // it gets its own next responder back.
            self.table.setNextResponder(self._keys.nextResponder().as_deref());
        }
    }

    /// Where the table put a row, in its content's coordinates.
    pub(crate) fn row_rect(&self, key: RowKey) -> Option<Rect> {
        let index = *self.data.borrow().index.get(&key)?;
        let mut r = self.table.rectOfRow(index as NSInteger);
        // A tree's row is its outline cell, in by its indent.
        if self.outline.is_some() {
            let cell = self.table.frameOfCellAtColumn_row(0, index as NSInteger);
            (r.origin.x, r.size.width) = (cell.origin.x, cell.size.width);
        }
        Some(Rect::new(r.origin.x as f32, r.origin.y as f32, r.size.width as f32, r.size.height as f32))
    }

    /// Where the table put a cell's host, in the scroll view's content
    /// (below the header, unscrolled): its cell's place, and its own in
    /// the cell. The part of the list under the window's bar isn't the
    /// core's.
    pub(crate) fn cell_rect(&self, key: RowKey, column: usize, host: &NSView) -> Option<Rect> {
        let index = *self.data.borrow().index.get(&key)?;
        let cell = self.table.frameOfCellAtColumn_row(column as NSInteger, index as NSInteger);
        let origin = self.table.convertPoint_toView(cell.origin, Some(&self.scroll));
        let scrolled = crate::classes::scrolled(&self.scroll.contentView());
        let f = host.frame();
        Some(Rect::new(
            (origin.x + scrolled.x + f.origin.x) as f32,
            (origin.y + scrolled.y + f.origin.y - self.scroll.under_bar()) as f32,
            f.size.width as f32,
            f.size.height as f32,
        ))
    }

    /// Rows move within the list: the table takes its own rows' drags,
    /// as a move.
    pub(crate) fn set_moves(&self, moves: RowMoves) {
        self.data.borrow_mut().moves = Some(moves);
        self.table.registerForDraggedTypes(&NSArray::from_retained_slice(&[NSString::from_str(ROW_TYPE)]));
        self.table.setDraggingSourceOperationMask_forLocal(NSDragOperation::Copy | NSDragOperation::Move, true);
    }

    /// A drag of `rows` dropped where the platform proposed `to`, through
    /// the same targeting as a real drop: for tests, which can't start a
    /// drag.
    pub(crate) fn move_rows(&self, rows: &[RowKey], to: RowDrop<RowKey>) {
        let ListIvars { id, events, data } = &self.ivars;
        let moved = {
            let data = data.borrow();
            let (row, on) = match to {
                RowDrop::On(key) => (data.index.get(&key).map_or(-1, |i| *i as isize), true),
                RowDrop::Before(key) => (data.index.get(&key).map_or(-1, |i| *i as isize), false),
                RowDrop::End => (data.rows.len() as isize, false),
            };
            let mut rows: Vec<RowKey> = rows.iter().filter(|row| data.index.contains_key(row)).copied().collect();
            rows.sort_by_key(|row| data.index[row]);
            // A tree's rows nest: the outline's targeting, the pointer over
            // the row dropped onto or before, as a drag's.
            let over = match to {
                RowDrop::On(key) | RowDrop::Before(key) => Some(key),
                RowDrop::End => None,
            };
            match self.outline {
                Some(_) => data.tree_drop_target(&rows, over, to).map(|(to, ..)| (rows, to)),
                None => data.drop_target(&rows, row, on).map(|(to, ..)| (rows, to)),
            }
        };
        if let Some((rows, to)) = moved.filter(|(rows, _)| !rows.is_empty()) {
            events.emit(*id, UiEvent::RowsMoved { rows, to });
        }
    }

    /// What the app gave, which the table can't give back.
    pub(crate) fn moves(&self) -> Option<RowMoves> {
        self.data.borrow().moves
    }

    pub(crate) fn set_row_files(&self, files: Vec<(RowKey, PathBuf)>) {
        let mut data = self.data.borrow_mut();
        data.file_of = files.iter().cloned().collect();
        data.files = Some(files);
    }

    pub(crate) fn row_files(&self) -> Option<Vec<(RowKey, PathBuf)>> {
        self.data.borrow().files.clone()
    }

    /// What dragging `row` puts on the pasteboard, as the table drags: every
    /// selected row's writer when it's selected, else its own, asked of the
    /// table's data source as the table asks it.
    pub(crate) fn dragged_files(&self, row: RowKey) -> Option<Vec<PathBuf>> {
        let selected = self.selected();
        let rows = if selected.contains(&row) { selected } else { vec![row] };
        // A tree's rows don't move, so each one's writer is its file's URL.
        if self.outline.is_some() {
            let data = self.data.borrow();
            let paths: Vec<PathBuf> = rows.iter().filter_map(|row| data.file_of.get(row).cloned()).collect();
            return (!paths.is_empty()).then_some(paths);
        }
        // SAFETY: the data source is the list's own `ListSource`, alive as
        // long as the list.
        let source = unsafe { self.table.dataSource() }?;
        let indexes: Vec<NSInteger> = {
            let data = self.data.borrow();
            rows.iter().filter_map(|row| data.index.get(row).map(|i| *i as NSInteger)).collect()
        };
        let paths: Vec<PathBuf> = indexes
            .into_iter()
            .filter_map(|index| {
                let writer: Option<Retained<AnyObject>> =
                    unsafe { msg_send![&*source, tableView: &*self.table, pasteboardWriterForRow: index] };
                let url = match writer?.downcast::<NSPasteboardItem>() {
                    // Rows that move carry their key too, in an item.
                    Ok(item) => {
                        let url = item.stringForType(unsafe { NSPasteboardTypeFileURL })?;
                        NSURL::URLWithString(&url)?
                    }
                    Err(writer) => writer.downcast::<NSURL>().ok()?,
                };
                url.path().map(|p| PathBuf::from(p.to_string()))
            })
            .collect();
        (!paths.is_empty()).then_some(paths)
    }

    /// The rows as the table has them.
    pub(crate) fn rows(&self) -> Vec<RowKey> {
        let data = self.data.borrow();
        (0..self.table.numberOfRows()).filter_map(|i| data.rows.get(i as usize).copied()).collect()
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        match &self.outline {
            Some(outline) => outline_keys(outline, &outline.selectedRowIndexes()),
            None => self.data.borrow().keys(&self.table.selectedRowIndexes()),
        }
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
