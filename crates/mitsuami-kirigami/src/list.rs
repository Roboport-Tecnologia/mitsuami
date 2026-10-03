//! `List` and `Table`: a QML `ListView` over the row keys, with Qt Quick
//! Controls' item delegates, in the style's scroll view (see
//! [`qml::list`](crate::qml::list)); or for a table, a QML `TableView` over
//! them with a delegate per cell, under a header (see
//! [`qml::table`](crate::qml::table)).
//!
//! The list view virtualises: it creates delegates for the rows in view
//! (and its cache buffer) and destroys them when they scroll away. The
//! delegates say so, coalesced to once per event loop turn, and the rows
//! with a delegate are compared with the rows reported: only the difference
//! is reported (`RowShown`, `RowHidden`), so a row whose delegate is
//! recreated (a data change resets the view) keeps its state. Each row's
//! host goes in its delegate; a table's cells' hosts in their delegates,
//! and a row is shown while any of its cells has one.
//!
//! The list view's signals come from Qt's event processing, never from our
//! own `apply`; they still only touch the list's own data and emit.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mitsuami_core::{
    ColumnData, ColumnSort, EventValue, ListStyle, NodeId, Point, Rect, RowKey, SelectionMode, SortOrder, UiEvent,
};

use crate::events::Events;
use crate::ffi::QmlObject;
use crate::qml;

/// A cell: its row, and its column (a list's is 0).
type Slot = (RowKey, usize);

#[derive(Default)]
struct Data {
    /// A table: a `TableView` of cells under a header.
    table: bool,
    rows: Vec<RowKey>,
    index: HashMap<RowKey, usize>,
    /// The mounted rows' hosts: a list's row hosts, a table's cell hosts.
    hosts: HashMap<Slot, (NodeId, QmlObject)>,
    /// Tables only: the columns as the app sent them, and the widths their
    /// cells get, as last reported.
    columns: Vec<ColumnData>,
    widths: Vec<f32>,
    /// The rows reported shown.
    shown: HashSet<RowKey>,
    estimate: Option<f64>,
    /// Without the app's estimate: the first row measured.
    learned: Option<f64>,
    mode: SelectionMode,
    /// Qt can't tell `Automatic` from `Plain`.
    style: Option<ListStyle>,
    /// The width last reported for rows.
    row_width: Option<f64>,
    /// The rows' files, as the app gave them (`Prop::RowFiles`).
    files: Option<Vec<(RowKey, PathBuf)>>,
}

pub(crate) struct List {
    /// The scroll view: the item that stands for the list; a table's item
    /// holding its header and scroll view.
    pub root: QmlObject,
    /// The list or table view in it.
    pub view: QmlObject,
    id: NodeId,
    events: Events,
    data: Rc<RefCell<Data>>,
}

/// A path as a `file://` URL, as `text/uri-list` holds them: bytes other
/// than unreserved ones and `/` percent-encoded.
fn file_url(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut url = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => url.push(byte as char),
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

/// The path of a `file://` URL.
fn local_path(url: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let encoded = url.strip_prefix("file://")?.as_bytes();
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut i = 0;
    while i < encoded.len() {
        let hex = encoded.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match (encoded[i], hex) {
            (b'%', Some(byte)) => {
                bytes.push(byte);
                i += 3;
            }
            (byte, _) => {
                bytes.push(byte);
                i += 1;
            }
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

fn parse(key: &str) -> Option<RowKey> {
    key.parse().ok().map(RowKey)
}

impl List {
    /// A list, or with `table` a table: its columns come with
    /// `set_columns`.
    pub(crate) fn new(id: NodeId, events: Events, table: bool) -> List {
        let (root, view) = if table {
            let root = QmlObject::load(&qml::table());
            (root, root.child("mitsuamiTableView").expect("tables have a table view"))
        } else {
            let root = QmlObject::load(&qml::list());
            (root, root.child("mitsuamiListView").expect("lists have a list view"))
        };
        let data = Rc::new(RefCell::new(Data { table, ..Data::default() }));
        if table {
            {
                // The widths columns give their cells, each time the view
                // lays out.
                let (data, events) = (data.clone(), events.clone());
                view.connect("mitsuamiCellWidthsChanged()", move || {
                    let text = view.str("mitsuamiCellWidths");
                    let widths: Vec<f32> = text.split(',').filter_map(|w| w.parse().ok()).collect();
                    let mut d = data.borrow_mut();
                    if !widths.is_empty() && widths.len() == d.columns.len() && widths != d.widths {
                        d.widths = widths.clone();
                        drop(d);
                        events.emit(id, UiEvent::ColumnWidths(widths));
                    }
                });
            }
            // A header pressed sorts, as the view does: reported as the
            // user's.
            let events = events.clone();
            view.connect("mitsuamiSortPressed()", move || {
                if let Some(sort) = sort_of(view) {
                    events.emit(id, UiEvent::Changed(EventValue::Sort(sort)));
                }
            });
        } else {
            // The scroll bar's column, and a frame, take room from the rows.
            let (data, events) = (data.clone(), events.clone());
            view.connect("widthChanged()", move || {
                let width = view.real("width");
                if data.borrow_mut().row_width.replace(width) != Some(width) {
                    events.emit(id, UiEvent::RowWidth(width as f32));
                }
            });
        }
        {
            let (data, events) = (data.clone(), events.clone());
            view.connect("mitsuamiRowsChanged()", move || rows_changed(view, &data, &events, id));
        }
        {
            let events = events.clone();
            view.connect("mitsuamiSelectionChanged()", move || {
                let selected = selected(view);
                events.emit(id, UiEvent::Changed(EventValue::Rows(selected)));
            });
        }
        {
            let events = events.clone();
            view.connect("mitsuamiActivate()", move || {
                if let Some(key) = parse(&view.str("mitsuamiActivated")) {
                    events.emit(id, UiEvent::RowActivated(key));
                }
            });
        }
        for signal in ["contentXChanged()", "contentYChanged()", "originXChanged()", "originYChanged()"] {
            let events = events.clone();
            view.connect(signal, move || events.emit(id, UiEvent::Scrolled(scroll_offset(view))));
        }
        List { root, view, id, events, data }
    }

    /// New rows. The view resets, so its scroll position is put back, and
    /// the selected rows that stayed stay selected (reporting it if some
    /// went).
    pub(crate) fn set_rows(&self, rows: Vec<RowKey>) {
        let offset = scroll_offset(self.view);
        let selected = self.selected();
        {
            let mut data = self.data.borrow_mut();
            data.index = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
            data.rows = rows.clone();
        }
        let keys: Vec<String> = rows.iter().map(|k| k.0.to_string()).collect();
        self.view.set_str_list("mitsuamiKeys", &keys);
        self.scroll_to(offset);
        let kept: Vec<RowKey> = {
            let data = self.data.borrow();
            selected.iter().copied().filter(|k| data.index.contains_key(k)).collect()
        };
        self.set_selected(&kept);
        if kept != selected {
            self.events.emit_always(self.id, UiEvent::Changed(EventValue::Rows(kept)));
        }
    }

    /// The rows' files, which the view's delegates drag (see `qml::list`).
    pub(crate) fn set_row_files(&self, files: Vec<(RowKey, PathBuf)>) {
        let keys: Vec<String> = files.iter().map(|(k, _)| k.0.to_string()).collect();
        let urls: Vec<String> = files.iter().map(|(_, path)| file_url(path)).collect();
        self.view.set_str_list("mitsuamiFileKeys", &keys);
        self.view.set_str_list("mitsuamiFileUrls", &urls);
        self.data.borrow_mut().files = Some(files);
    }

    pub(crate) fn row_files(&self) -> Option<Vec<(RowKey, PathBuf)>> {
        self.data.borrow().files.clone()
    }

    /// What dragging a mounted row carries: its delegate's drag's URLs,
    /// as QML makes them from the selection.
    pub(crate) fn dragged_files(&self, host: QmlObject) -> Option<Vec<PathBuf>> {
        let urls = delegate_holding(host)?.str_list("mitsuamiDragUrls");
        let paths: Vec<PathBuf> = urls.iter().filter_map(|url| local_path(url)).collect();
        (!paths.is_empty()).then_some(paths)
    }

    /// The selection is ours: keep what the new mode holds, the first row
    /// for one, and report the rows let go.
    pub(crate) fn set_mode(&self, mode: SelectionMode) {
        let selected = self.selected();
        let kept: Vec<RowKey> = match mode {
            SelectionMode::None => Vec::new(),
            SelectionMode::Single => selected.iter().take(1).copied().collect(),
            SelectionMode::Multiple => selected.clone(),
        };
        if kept != selected {
            self.set_selected(&kept);
            self.events.emit_always(self.id, UiEvent::Changed(EventValue::Rows(kept)));
        }
        self.data.borrow_mut().mode = mode;
        let mode = match mode {
            SelectionMode::None => 0,
            SelectionMode::Single => 1,
            SelectionMode::Multiple => 2,
        };
        self.view.set_int("mitsuamiMode", mode);
    }

    pub(crate) fn mode(&self) -> SelectionMode {
        self.data.borrow().mode
    }

    pub(crate) fn set_style(&self, style: ListStyle) {
        self.data.borrow_mut().style = Some(style);
        self.root.set_bool("mitsuamiFramed", style.framed());
    }

    pub(crate) fn style(&self) -> Option<ListStyle> {
        self.data.borrow().style
    }

    /// Selects rows without reporting it; the first is the current row.
    pub(crate) fn set_selected(&self, keys: &[RowKey]) {
        let current = keys.first().and_then(|k| self.data.borrow().index.get(k).copied());
        let keys: Vec<String> = keys.iter().map(|k| k.0.to_string()).collect();
        self.view.set_str_list("mitsuamiSelected", &keys);
        let current = current.map_or(-1, |i| i as i32);
        // A table's current row is ours; a list's is the view's.
        if self.data.borrow().table {
            self.view.set_int("mitsuamiCurrent", current);
            return;
        }
        self.view.set_bool("mitsuamiMuted", true);
        self.view.set_int("currentIndex", current);
        self.view.set_bool("mitsuamiMuted", false);
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        selected(self.view)
    }

    /// Selects a row as the user would, reporting it.
    pub(crate) fn select(&self, key: RowKey) {
        self.set_selected(&[key]);
        self.events.emit(self.id, UiEvent::Changed(EventValue::Rows(vec![key])));
    }

    pub(crate) fn activate(&self, key: RowKey) {
        self.events.emit(self.id, UiEvent::RowActivated(key));
    }

    pub(crate) fn set_estimate(&self, height: f32) {
        self.data.borrow_mut().estimate = Some(height as f64);
        self.view.set_real("mitsuamiEstimate", height as f64);
    }

    pub(crate) fn estimate(&self) -> Option<f32> {
        self.data.borrow().estimate.map(|h| h as f32)
    }

    /// Without the app's estimate, the first row measured sets it.
    pub(crate) fn row_measured(&self, height: f32) {
        let mut data = self.data.borrow_mut();
        if data.estimate.is_none() && data.learned.is_none() && height > 0.0 {
            data.learned = Some(height as f64);
            self.view.set_real("mitsuamiEstimate", height as f64);
        }
    }

    /// Hosts a mounted row, or a table's cell, in its delegate if the
    /// view has one.
    pub(crate) fn insert(&self, key: RowKey, column: usize, id: NodeId, host: QmlObject) {
        self.data.borrow_mut().hosts.insert((key, column), (id, host));
        if let Some(delegate) = delegate_of(self.view, (key, column)) {
            place(host, delegate);
        }
    }

    pub(crate) fn remove(&self, key: RowKey, column: usize) {
        let Some((_, host)) = self.data.borrow_mut().hosts.remove(&(key, column)) else { return };
        if let Some(delegate) = delegate_holding(host) {
            delegate.set_object("mitsuamiHost", None);
        }
        host.set_parent_item(None, 0);
    }

    pub(crate) fn scroll_to(&self, offset: Point) {
        if self.data.borrow().table {
            self.view.set_real("contentX", offset.x as f64 + self.view.real("originX"));
        }
        self.view.set_real("contentY", offset.y as f64 + self.view.real("originY"));
    }

    /// A table's columns, as JSON for its view, which makes its model and
    /// header again.
    pub(crate) fn set_columns(&self, columns: &[ColumnData]) {
        self.data.borrow_mut().columns = columns.to_vec();
        let json: Vec<String> = columns
            .iter()
            .map(|c| {
                format!(
                    r#"{{"title":{},"width":{},"expand":{},"sortable":{}}}"#,
                    json_string(&c.title),
                    c.width.map_or(-1.0, f64::from),
                    c.expand,
                    c.sortable
                )
            })
            .collect();
        self.view.set_str("mitsuamiColumnsJson", &format!("[{}]", json.join(",")));
    }

    pub(crate) fn columns(&self) -> Vec<ColumnData> {
        self.data.borrow().columns.clone()
    }

    /// Shows the sort in the header, without reporting it.
    pub(crate) fn set_sort(&self, sort: Option<ColumnSort>) {
        self.view.set_int("mitsuamiSortColumn", sort.map_or(-1, |s| s.column as i32));
        self.view.set_bool("mitsuamiDescending", sort.is_some_and(|s| s.order == SortOrder::Descending));
    }

    pub(crate) fn sort(&self) -> Option<ColumnSort> {
        sort_of(self.view)
    }

    /// Presses a sortable column's header, as a click on it does: the view
    /// sorts and reports it. False for a column that doesn't sort.
    pub(crate) fn press_header(&self, column: usize) -> bool {
        if !self.data.borrow().columns.get(column).is_some_and(|c| c.sortable) {
            return false;
        }
        self.view.set_int("mitsuamiPressed", column as i32);
        true
    }

    pub(crate) fn scroll_to_row(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let Some(index) = index {
            self.view.set_int("mitsuamiScrollTo", index as i32);
        }
    }

    pub(crate) fn scroll_offset(&self) -> Point {
        scroll_offset(self.view)
    }

    /// Where the view put a row's host (its delegate), in its content; a
    /// table a cell's host, in its content below the header (in the
    /// table's coordinates, as scrolled back to the start).
    pub(crate) fn row_rect(&self, host: QmlObject, size: Rect) -> Rect {
        if self.data.borrow().table {
            if delegate_holding(host).is_none_or(|d| d.str("mitsuamiKey").is_empty()) {
                return size;
            }
            let (at, origin, offset) =
                (host.map_to_scene(Point::ZERO), self.root.map_to_scene(Point::ZERO), scroll_offset(self.view));
            return Rect::new(at.x - origin.x + offset.x, at.y - origin.y + offset.y, size.width(), size.height());
        }
        match host.object("parent") {
            Some(delegate) if !delegate.str("mitsuamiKey").is_empty() => {
                Rect::new(0.0, (delegate.real("y") - self.view.real("originY")) as f32, size.width(), size.height())
            }
            _ => size,
        }
    }

    pub(crate) fn rows(&self) -> Vec<RowKey> {
        let data = self.data.borrow();
        // A table without columns has no model to show its rows.
        let count = match data.table {
            true if data.columns.is_empty() => data.rows.len(),
            true => self.view.int("rows").max(0) as usize,
            false => self.view.int("count").max(0) as usize,
        };
        data.rows.iter().take(count).copied().collect()
    }

    /// The hosted rows (a table's cells), in row order, then column order.
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

/// The content's offset: `ListView` content starts at `originY`, which
/// moves as rows turn out taller or shorter than estimated. A table's
/// scrolls sideways too.
fn scroll_offset(view: QmlObject) -> Point {
    Point::new(
        (view.real("contentX") - view.real("originX")) as f32,
        (view.real("contentY") - view.real("originY")) as f32,
    )
}

/// The sort a table's header shows.
fn sort_of(view: QmlObject) -> Option<ColumnSort> {
    let column = usize::try_from(view.int("mitsuamiSortColumn")).ok()?;
    let order = if view.bool("mitsuamiDescending") { SortOrder::Descending } else { SortOrder::Ascending };
    Some(ColumnSort { column, order })
}

/// A string as JSON.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn selected(view: QmlObject) -> Vec<RowKey> {
    view.str("mitsuamiSelectedKeys").split(',').filter_map(parse).collect()
}

/// The delegates the view has, and their rows (and a table's, columns).
/// A table jumping far lets its delegates go for deleting later, and
/// takes their QML context at once, so they never say they went
/// (`Component.onDestruction`): they're left out from then on.
fn delegates(view: QmlObject) -> Vec<(Slot, QmlObject)> {
    let Some(content) = view.object("contentItem") else { return Vec::new() };
    content
        .child_items()
        .into_iter()
        .filter(|d| d.has_context())
        .filter_map(|d| {
            let column = usize::try_from(d.int("mitsuamiColumn")).unwrap_or(0);
            Some(((parse(&d.str("mitsuamiKey"))?, column), d))
        })
        .collect()
}

fn delegate_of(view: QmlObject, slot: Slot) -> Option<QmlObject> {
    delegates(view).into_iter().find(|(s, _)| *s == slot).map(|(_, d)| d)
}

/// The delegate a host is in: its parent, or a table cell's slot's.
fn delegate_holding(host: QmlObject) -> Option<QmlObject> {
    let parent = host.object("parent")?;
    if parent.str("objectName") == "mitsuamiCellSlot" { parent.object("parent") } else { Some(parent) }
}

/// Puts a host in its delegate, above the delegate's background: a
/// table's cell in the delegate's slot, which centres it in the row.
fn place(host: QmlObject, delegate: QmlObject) {
    let into = delegate.child("mitsuamiCellSlot").unwrap_or(delegate);
    if host.object("parent") != Some(into) {
        host.set_parent_item(Some(into), into.child_items().len());
    }
    delegate.set_object("mitsuamiHost", Some(host));
}

/// Delegates came or went: hosts go in the new ones, and the rows with a
/// delegate are compared with the rows reported.
fn rows_changed(view: QmlObject, data: &Rc<RefCell<Data>>, events: &Events, id: NodeId) {
    let delegates = delegates(view);
    let mut d = data.borrow_mut();
    for (slot, delegate) in &delegates {
        if let Some((_, host)) = d.hosts.get(slot) {
            place(*host, *delegate);
        }
    }
    let now: HashSet<RowKey> = delegates.iter().map(|((k, _), _)| *k).collect();
    let mut hidden: Vec<RowKey> = d.shown.difference(&now).copied().collect();
    let mut shown: Vec<RowKey> = now.difference(&d.shown).copied().collect();
    hidden.sort_by_key(|k| d.index.get(k).copied());
    shown.sort_by_key(|k| d.index.get(k).copied());
    d.shown = now;
    drop(d);
    for row in hidden {
        events.emit(id, UiEvent::RowHidden(row));
    }
    for row in shown {
        events.emit(id, UiEvent::RowShown(row));
    }
}
