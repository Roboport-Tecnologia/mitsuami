//! `List` and `Table`: a QML `ListView` over the row keys, with Qt Quick
//! Controls' item delegates, in the style's scroll view (see
//! [`qml::list`](crate::qml::list)); or for a table, a QML `TableView` over
//! them with a delegate per cell, under a header (see
//! [`qml::table`](crate::qml::table)).
//!
//! Both views' model is a Qt model of the shim's (`mq_rows_new`), which
//! new rows change as a Qt model of a Kirigami app's would: the runs of
//! rows that went, came or moved (`edits`), so a list view keeps the
//! delegates of the rows that stay, and places the rest as `ListView`
//! does. Too many changes reset it (its scroll position is put back). A
//! `TableView` makes the cells in view again on any change of rows, as it
//! does for every model, but keeps its scroll position.
//!
//! The views virtualise: they create delegates for the rows in view
//! (and the cache buffer) and destroy them when they scroll away. The
//! delegates say so, coalesced to once per event loop turn, and the rows
//! with a delegate are compared with the rows reported: only the difference
//! is reported (`RowShown`, `RowHidden`), so a row whose delegate is
//! recreated (a reset, or a table's new rows) keeps its state. Each row's
//! host goes in its delegate; a table's cells' hosts in their delegates,
//! and a row is shown while any of its cells has one.
//!
//! A tree is a QML `TreeView` over a tree model of the shim's
//! (`mq_tree_new`), changed a run of a parent's rows at a time, with the
//! style's tree delegates (see [`qml::tree`](crate::qml::tree)); its
//! delegates hold hosts as a table's cells do.
//!
//! The list view's signals come from Qt's event processing, never from our
//! own `apply`; they still only touch the list's own data and emit.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mitsuami_core::{
    ColumnData, ColumnSort, EventValue, ListStyle, NodeId, Point, Rect, RowDrop, RowKey, RowMoves, SelectionMode,
    SortOrder, TreeIndent, TreeRow, UiEvent,
};

use crate::events::Events;
use crate::ffi::{QmlObject, RowEdit, TreeEdit};
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
    /// Tables with a column that fills its rows only: the shown rows'
    /// heights without those cells, as reported.
    bare: HashMap<RowKey, f32>,
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
    /// Where rows dragged within it can go (`Prop::RowMoves`).
    moves: Option<RowMoves>,
    /// A tree: a `TreeView` over a tree model. Its model's rows under each
    /// parent (the top rows under 0), whether each opens, each row's parent
    /// as sent, and the indent last reported.
    tree: bool,
    kids: HashMap<u64, Vec<RowKey>>,
    parents: HashMap<RowKey, Option<RowKey>>,
    expandable: HashMap<RowKey, bool>,
    indent: Option<TreeIndent>,
}

impl Data {
    /// Takes a row and the rows under it out of the model's rows.
    fn forget(&mut self, key: RowKey) {
        self.expandable.remove(&key);
        for kid in self.kids.remove(&key.0).unwrap_or_default() {
            self.forget(kid);
        }
    }
}

impl Data {
    /// Where a drag of `rows` (keys, comma-separated, as the drag carries
    /// them) proposed at `target` (`on:KEY`, `before:KEY` or `end`, as the
    /// view proposes) lands: `RowMoves::target` (a tree's `tree_target`:
    /// only folders take rows onto them, nothing lands while the pointer is
    /// over a dragged row, `over`, and a folder doesn't go inside itself),
    /// with the rows that are still here, in the list's order. `None`:
    /// refused.
    fn drop_target(&self, rows: &str, target: &str, over: &str) -> Option<(Vec<RowKey>, RowDrop<RowKey>)> {
        let moves = self.moves?;
        let mut rows: Vec<RowKey> =
            rows.split(',').filter_map(parse).filter(|row| self.index.contains_key(row)).collect();
        rows.sort_by_key(|row| self.index[row]);
        let known = |key: &str| parse(key).filter(|k| self.index.contains_key(k));
        let proposed = match target.split_once(':') {
            Some(("on", key)) => RowDrop::On(known(key)?),
            Some(("before", key)) => RowDrop::Before(known(key)?),
            // A tree's row under the pointer and where in it, across the
            // row and down it (`at:KEY:X:Y:HEIGHT`), proposed as every
            // tree's (`RowMoves::tree_proposal`).
            Some(("at", at)) => {
                let mut parts = at.split(':');
                let key = known(parts.next()?)?;
                let x: f32 = parts.next()?.parse().ok()?;
                let y: f32 = parts.next()?.parse().ok()?;
                let height: f32 = parts.next()?.parse().ok()?;
                let next = self.rows.get(self.index[&key] + 1).copied();
                let x = self.indent.map(|_| x);
                RowMoves::tree_proposal(key, x, y, height, self.indent.unwrap_or_default(), next, |r| self.tree_row(r))
            }
            _ if target == "end" => RowDrop::End,
            _ => return None,
        };
        let to = match self.tree {
            true => {
                let over = parse(over).filter(|k| self.index.contains_key(k));
                moves.tree_target(&rows, over, proposed, |r| self.tree_row(r))?
            }
            false => moves.target(&rows, proposed)?,
        };
        (!rows.is_empty()).then_some((rows, to))
    }
}

impl Data {
    /// A tree's row as sent: its parent, its depth (its parents counted),
    /// and whether it opens.
    fn tree_row(&self, row: RowKey) -> Option<TreeRow> {
        let parent = self.parents.get(&row).copied()?;
        let mut depth = 0;
        let mut above = parent;
        while let Some(p) = above {
            depth += 1;
            above = self.parents.get(&p).copied().flatten();
        }
        let expandable = self.expandable.get(&row).copied().unwrap_or(false);
        Some(TreeRow { row, parent, depth, expandable, expanded: false })
    }
}

/// A drop target as the view shows it: `on:KEY`, `before:KEY` or `end`.
fn target_text(to: RowDrop<RowKey>) -> String {
    match to {
        RowDrop::On(key) => format!("on:{}", key.0),
        RowDrop::Before(key) => format!("before:{}", key.0),
        RowDrop::End => "end".to_string(),
    }
}

pub(crate) struct List {
    /// The scroll view: the item that stands for the list; a table's item
    /// holding its header and scroll view.
    pub root: QmlObject,
    /// The list or table view in it.
    pub view: QmlObject,
    /// The view's model, the view's child.
    model: QmlObject,
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
        // A table's columns come with `set_columns`; a list has one.
        let model = view.row_model(if table { 0 } else { 1 });
        view.set_object("model", Some(model));
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
            {
                // Rows' heights without their filling cells, which those
                // cells are laid out at.
                let (data, events) = (data.clone(), events.clone());
                view.connect("mitsuamiBareChanged()", move || {
                    let text = view.str("mitsuamiBareHeights");
                    let mut d = data.borrow_mut();
                    let mut changed = Vec::new();
                    for (key, height) in text.split(',').filter_map(|pair| pair.split_once(':')) {
                        let (Some(key), Ok(height)) = (parse(key), height.parse::<f32>()) else { continue };
                        if d.shown.contains(&key) && d.bare.insert(key, height) != Some(height) {
                            changed.push((key, height));
                        }
                    }
                    drop(d);
                    for (key, height) in changed {
                        events.emit(id, UiEvent::RowHeight(key, height));
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
            // Rows came, went or turned out higher or lower than estimated.
            let events = events.clone();
            root.connect("mitsuamiNaturalHeightChanged()", move || events.emit(id, UiEvent::Remeasure));
        }
        {
            let events = events.clone();
            view.connect("mitsuamiSelectionChanged()", move || {
                let selected = selected(view);
                events.emit(id, UiEvent::Changed(EventValue::Rows(selected)));
            });
        }
        if !table {
            // A drag of the list's own rows: the view asks where it lands
            // while it's over it, and reports the drop (see `qml::list`).
            let data_ = data.clone();
            view.connect("mitsuamiAskDrop()", move || {
                let answer =
                    data_.borrow().drop_target(&view.str("mitsuamiAskRows"), &view.str("mitsuamiAskTarget"), "");
                view.set_str("mitsuamiDropAnswer", &answer.map(|(_, to)| target_text(to)).unwrap_or_default());
            });
            let (data, events) = (data.clone(), events.clone());
            view.connect("mitsuamiDropped()", move || {
                let moved = data.borrow().drop_target(&view.str("mitsuamiAskRows"), &view.str("mitsuamiAskTarget"), "");
                if let Some((rows, to)) = moved {
                    events.emit(id, UiEvent::RowsMoved { rows, to });
                }
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
        List { root, view, model, id, events, data }
    }

    /// A tree: a `TreeView` over a tree model (see [`qml::tree`]).
    pub(crate) fn new_tree(id: NodeId, events: Events) -> List {
        let root = QmlObject::load(&qml::tree());
        let view = root.child("mitsuamiTreeView").expect("trees have a tree view");
        let model = view.tree_model();
        view.set_object("model", Some(model));
        let data = Rc::new(RefCell::new(Data { tree: true, ..Data::default() }));
        {
            let (data, events) = (data.clone(), events.clone());
            view.connect("mitsuamiRowsChanged()", move || rows_changed(view, &data, &events, id));
        }
        {
            // Rows came, went or turned out higher or lower than estimated.
            let events = events.clone();
            root.connect("mitsuamiNaturalHeightChanged()", move || events.emit(id, UiEvent::Remeasure));
        }
        {
            let events = events.clone();
            view.connect("mitsuamiSelectionChanged()", move || {
                events.emit(id, UiEvent::Changed(EventValue::Rows(selected(view))));
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
        {
            // The view's own opening and closing: its indicator, its keys.
            let events = events.clone();
            view.connect("mitsuamiToggle()", move || {
                if let Some(key) = parse(&view.str("mitsuamiToggled")) {
                    events.emit(id, UiEvent::RowExpanded(key, view.bool("mitsuamiToggledOpen")));
                }
            });
        }
        {
            // Where a delegate put its content: the indent, and the rows'
            // width past it, as the style draws them.
            let (data, events) = (data.clone(), events.clone());
            view.connect("mitsuamiIndented()", move || {
                let text = view.str("mitsuamiIndentText");
                let numbers: Vec<f32> = text.split(',').filter_map(|n| n.parse().ok()).collect();
                let [start, per_level, end] = numbers[..] else { return };
                let indent = TreeIndent { start, per_level };
                let mut d = data.borrow_mut();
                let indented = d.indent.replace(indent) != Some(indent);
                let wider = d.row_width.replace(end as f64) != Some(end as f64);
                drop(d);
                if wider {
                    events.emit(id, UiEvent::RowWidth(end));
                }
                if indented {
                    events.emit(id, UiEvent::TreeIndent(indent));
                }
            });
        }
        {
            // Its own rows dragged within it, as a list's: QML asks where
            // a drag would land while it's over it, and reports the drop
            // (see `qml::tree`).
            let (data_, data, events) = (data.clone(), data.clone(), events.clone());
            view.connect("mitsuamiAskDrop()", move || {
                let answer = data_.borrow().drop_target(
                    &view.str("mitsuamiAskRows"),
                    &view.str("mitsuamiAskTarget"),
                    &view.str("mitsuamiAskOver"),
                );
                view.set_str("mitsuamiDropAnswer", &answer.map(|(_, to)| target_text(to)).unwrap_or_default());
            });
            view.connect("mitsuamiDropped()", move || {
                let moved = data.borrow().drop_target(
                    &view.str("mitsuamiAskRows"),
                    &view.str("mitsuamiAskTarget"),
                    &view.str("mitsuamiAskOver"),
                );
                if let Some((rows, to)) = moved {
                    events.emit(id, UiEvent::RowsMoved { rows, to });
                }
            });
        }
        for signal in ["contentXChanged()", "contentYChanged()", "originXChanged()", "originYChanged()"] {
            let events = events.clone();
            view.connect(signal, move || events.emit(id, UiEvent::Scrolled(scroll_offset(view))));
        }
        List { root, view, model, id, events, data }
    }

    /// A tree's rows that show. The model's rows under each parent become
    /// the rows sent, as runs of rows that went, came or moved, so the view
    /// keeps its delegates and the open rows that stay: first the rows that
    /// don't stay where they were (gone, under another parent, or no longer
    /// opening, or a closed row's children) go, with the rows under them;
    /// then, from the top, each parent's rows come or move into place. The
    /// view then opens and closes rows from the top as sent, which shows or
    /// drops their children's rows, without reporting it.
    pub(crate) fn set_tree_rows(&self, rows: Vec<TreeRow>) {
        let selected = self.selected();
        let mut kids: HashMap<u64, Vec<RowKey>> = HashMap::new();
        kids.insert(0, Vec::new());
        let mut parent: HashMap<RowKey, u64> = HashMap::new();
        for row in &rows {
            let p = row.parent.map_or(0, |p| p.0);
            kids.entry(p).or_default().push(row.row);
            parent.insert(row.row, p);
        }
        let opens: HashMap<RowKey, bool> = rows.iter().map(|r| (r.row, r.expandable)).collect();
        let mut data = self.data.borrow_mut();
        // The rows that go, from each parent still in the model down.
        let mut parents = vec![0u64];
        while let Some(p) = parents.pop() {
            let mut now = data.kids.get(&p).cloned().unwrap_or_default();
            let stays =
                |key: &RowKey, data: &Data| parent.get(key) == Some(&p) && opens.get(key) == data.expandable.get(key);
            let mut end = now.len();
            while end > 0 {
                if stays(&now[end - 1], &data) {
                    end -= 1;
                    continue;
                }
                let mut at = end - 1;
                while at > 0 && !stays(&now[at - 1], &data) {
                    at -= 1;
                }
                self.model.edit_tree(TreeEdit::Remove { parent: p, at, count: end - at });
                for key in now.drain(at..end) {
                    data.forget(key);
                }
                end = at;
            }
            parents.extend(now.iter().map(|k| k.0));
            data.kids.insert(p, now);
        }
        // The rows that come or move, from the top: a row's parent is in
        // the model by the time its rows come.
        let order = std::iter::once(0).chain(rows.iter().filter(|r| r.expanded).map(|r| r.row.0));
        for p in order {
            let old = data.kids.get(&p).cloned().unwrap_or_default();
            let new = kids.remove(&p).unwrap_or_default();
            let insert = |data: &mut Data, at: usize, keys: &[RowKey]| {
                let raw: Vec<u64> = keys.iter().map(|k| k.0).collect();
                let flags: Vec<u8> = keys.iter().map(|k| opens.get(k).copied().unwrap_or(false) as u8).collect();
                self.model.edit_tree(TreeEdit::Insert { parent: p, at, keys: &raw, expandable: &flags });
                for key in keys {
                    data.expandable.insert(*key, opens.get(key).copied().unwrap_or(false));
                    data.kids.insert(key.0, Vec::new());
                }
            };
            match edits(&old, &new) {
                Some(edits) => {
                    for edit in edits {
                        match edit {
                            Edit::Insert { at, count } => insert(&mut data, at, &new[at..at + count]),
                            Edit::Remove { at, count } => {
                                self.model.edit_tree(TreeEdit::Remove { parent: p, at, count });
                                for key in &old[at..at + count] {
                                    data.forget(*key);
                                }
                            }
                            Edit::Move { from, count, to } => {
                                self.model.edit_tree(TreeEdit::Move { parent: p, from, count, to })
                            }
                        }
                    }
                }
                // Too many: all of them again.
                None => {
                    self.model.edit_tree(TreeEdit::Remove { parent: p, at: 0, count: old.len() });
                    for key in &old {
                        data.forget(*key);
                    }
                    insert(&mut data, 0, &new);
                }
            }
            data.kids.insert(p, new);
        }
        data.index = rows.iter().enumerate().map(|(i, r)| (r.row, i)).collect();
        data.rows = rows.iter().map(|r| r.row).collect();
        data.parents = rows.iter().map(|r| (r.row, r.parent)).collect();
        drop(data);
        let keys: Vec<String> = rows.iter().map(|r| r.row.0.to_string()).collect();
        let open: Vec<String> = rows.iter().filter(|r| r.expanded).map(|r| r.row.0.to_string()).collect();
        self.view.set_str_list("mitsuamiKeys", &keys);
        self.view.set_str_list("mitsuamiOpen", &open);
        self.view.invoke("mitsuamiSync");
        let kept: Vec<RowKey> = {
            let data = self.data.borrow();
            selected.iter().copied().filter(|k| data.index.contains_key(k)).collect()
        };
        self.set_selected(&kept);
        if kept != selected {
            self.events.emit_always(self.id, UiEvent::Changed(EventValue::Rows(kept)));
        }
    }

    /// Opens or closes a tree's row as the user does: the view reports it.
    /// False for a row that doesn't open.
    pub(crate) fn expand(&self, key: RowKey, open: bool) -> bool {
        self.view.set_str("mitsuamiExpandKey", &key.0.to_string());
        self.view.set_bool("mitsuamiExpandOpen", open);
        self.view.invoke("mitsuamiExpand");
        self.view.bool("mitsuamiExpandDone")
    }

    /// A tree's rows as its view shows them.
    pub(crate) fn tree_rows(&self) -> Vec<TreeRow> {
        self.view.invoke("mitsuamiReadRows");
        let text = self.view.str("mitsuamiRowsText");
        text.split(',')
            .filter_map(|row| {
                let mut parts = row.split(':');
                let (key, parent, depth, opens, open) =
                    (parts.next()?, parts.next()?, parts.next()?, parts.next()?, parts.next()?);
                Some(TreeRow {
                    row: parse(key)?,
                    parent: parse(parent),
                    depth: depth.parse().ok()?,
                    expandable: opens == "1",
                    expanded: open == "1",
                })
            })
            .collect()
    }

    /// New rows: the model takes the rows that came, went and moved, as a
    /// Qt model reports them, so a list view keeps the delegates of the
    /// rows that stay, where `ListView` puts them. Too many changes reset
    /// it instead, which, as a table's new rows, keeps its scroll position.
    /// The selected rows that stayed stay selected (reporting it if some
    /// went).
    pub(crate) fn set_rows(&self, rows: Vec<RowKey>) {
        let offset = scroll_offset(self.view);
        let selected = self.selected();
        let (old, table) = {
            let mut data = self.data.borrow_mut();
            data.index = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
            (std::mem::replace(&mut data.rows, rows.clone()), data.table)
        };
        let keys: Vec<String> = rows.iter().map(|k| k.0.to_string()).collect();
        self.view.set_str_list("mitsuamiKeys", &keys);
        // A list's current row moves with its row, or goes with it: not
        // the user's doing, and it's set again below.
        if !table {
            self.view.set_bool("mitsuamiMuted", true);
        }
        let raw: Vec<u64> = rows.iter().map(|k| k.0).collect();
        let edits = edits(&old, &rows);
        let reset = edits.is_none();
        match edits {
            Some(edits) => {
                for edit in edits {
                    self.model.edit_rows(match edit {
                        Edit::Insert { at, count } => RowEdit::Insert { at, keys: &raw[at..at + count] },
                        Edit::Remove { at, count } => RowEdit::Remove { at, count },
                        Edit::Move { from, count, to } => RowEdit::Move { from, count, to },
                    });
                }
            }
            None => self.model.edit_rows(RowEdit::Reset { keys: &raw }),
        }
        if reset || table {
            self.scroll_to(offset);
        }
        if !table {
            self.view.set_bool("mitsuamiMuted", false);
        }
        let kept: Vec<RowKey> = {
            let data = self.data.borrow();
            selected.iter().copied().filter(|k| data.index.contains_key(k)).collect()
        };
        self.set_selected(&kept);
        if kept != selected {
            self.events.emit_always(self.id, UiEvent::Changed(EventValue::Rows(kept)));
        }
    }

    /// Rows move within the list: its rows' drags carry their keys too,
    /// and the view takes its own rows' drops (see `qml::list`).
    pub(crate) fn set_moves(&self, moves: RowMoves) {
        self.data.borrow_mut().moves = Some(moves);
        self.view.set_str("mitsuamiListId", &self.id.raw().to_string());
        self.view.set_bool("mitsuamiMoves", true);
    }

    /// What the app gave, which the view can't give back.
    pub(crate) fn moves(&self) -> Option<RowMoves> {
        self.data.borrow().moves
    }

    /// Where a drag of `rows` dropped where the platform proposed `to`
    /// lands, through the same targeting as a real drop, for the backend
    /// to report: for tests, which can't start a drag.
    pub(crate) fn move_rows(&self, rows: &[RowKey], to: RowDrop<RowKey>) -> Option<(Vec<RowKey>, RowDrop<RowKey>)> {
        let keys: Vec<String> = rows.iter().map(|row| row.0.to_string()).collect();
        // The pointer is over the row dropped onto or before, as a drag's.
        let over = match to {
            RowDrop::On(row) | RowDrop::Before(row) => row.0.to_string(),
            RowDrop::End => String::new(),
        };
        self.data.borrow().drop_target(&keys.join(","), &target_text(to), &over)
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
        // A table's and a tree's current row is ours; a list's is the
        // view's.
        if self.data.borrow().table || self.data.borrow().tree {
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

    /// How high the list is with all its rows: the view's content height,
    /// which Qt keeps as it lays rows out and estimates from them for the
    /// rows it hasn't made, a frame's padding and a table's header.
    pub(crate) fn natural_height(&self) -> f32 {
        self.root.real("mitsuamiNaturalHeight").ceil() as f32
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

    /// A table's columns: as many in the model, and as JSON for the view,
    /// which makes its header again.
    pub(crate) fn set_columns(&self, columns: &[ColumnData]) {
        self.data.borrow_mut().columns = columns.to_vec();
        self.model.edit_rows(RowEdit::Columns(columns.len()));
        let json: Vec<String> = columns
            .iter()
            .map(|c| {
                format!(
                    r#"{{"title":{},"width":{},"expand":{},"sortable":{},"fillsRow":{}}}"#,
                    json_string(&c.title),
                    c.width.map_or(-1.0, f64::from),
                    c.expand,
                    c.sortable,
                    c.fills_row
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
        // A tree's host is in its delegate's content, in by its indent.
        if self.data.borrow().tree {
            if delegate_holding(host).is_none_or(|d| d.str("mitsuamiKey").is_empty()) {
                return size;
            }
            let (at, origin, offset) =
                (host.map_to_scene(Point::ZERO), self.view.map_to_scene(Point::ZERO), scroll_offset(self.view));
            return Rect::new(at.x - origin.x + offset.x, at.y - origin.y + offset.y, size.width(), size.height());
        }
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
        // A table without columns has no cells to show its rows.
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

/// A slot's delegate, looked up in the view's own map: going through
/// every delegate for each row mounted took time in the square of the
/// rows in view.
fn delegate_of(view: QmlObject, (key, column): Slot) -> Option<QmlObject> {
    view.set_str("mitsuamiLookup", &format!("{}:{column}", key.0));
    view.invoke("mitsuamiFind");
    let found = view.object("mitsuamiFound");
    view.set_object("mitsuamiFound", None);
    found.filter(|d| d.has_context())
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
    // Shown again, its height is reported again.
    for row in &hidden {
        d.bare.remove(row);
    }
    drop(d);
    for row in hidden {
        events.emit(id, UiEvent::RowHidden(row));
    }
    for row in shown {
        events.emit(id, UiEvent::RowShown(row));
    }
}

/// How many changes `edits` makes before a reset is cheaper: each one goes
/// through the rows, and has the view move its delegates.
const MAX_EDITS: usize = 64;

/// A change to the model's rows, in its rows as they are by then: `to` is
/// where a moved run's first row ends up; the rows inserted are the new
/// rows at `at`.
enum Edit {
    Remove { at: usize, count: usize },
    Insert { at: usize, count: usize },
    Move { from: usize, count: usize, to: usize },
}

/// The changes that make `old` into `new`, as runs of rows: the rows that
/// went, from the end, then from the start the rows that came or moved.
/// `None` for too many, or for rows that repeat, which a model can't follow
/// by key.
fn edits(old: &[RowKey], new: &[RowKey]) -> Option<Vec<Edit>> {
    let wanted: HashSet<RowKey> = new.iter().copied().collect();
    if wanted.len() != new.len() {
        return None;
    }
    let mut edits = Vec::new();
    let mut rows = old.to_vec();
    let mut end = rows.len();
    while end > 0 {
        if wanted.contains(&rows[end - 1]) {
            end -= 1;
            continue;
        }
        let mut at = end - 1;
        while at > 0 && !wanted.contains(&rows[at - 1]) {
            at -= 1;
        }
        rows.drain(at..end);
        edits.push(Edit::Remove { at, count: end - at });
        end = at;
        if edits.len() > MAX_EDITS {
            return None;
        }
    }
    let kept: HashSet<RowKey> = rows.iter().copied().collect();
    if kept.len() != rows.len() {
        return None;
    }
    let mut i = 0;
    while i < new.len() {
        if rows.get(i) == Some(&new[i]) {
            i += 1;
            continue;
        }
        // The rows before `i` are in place: a row kept is further on.
        let count = if kept.contains(&new[i]) {
            let from = i + rows[i..].iter().position(|k| *k == new[i])?;
            let mut count = 1;
            while from + count < rows.len() && i + count < new.len() && rows[from + count] == new[i + count] {
                count += 1;
            }
            let moved: Vec<RowKey> = rows.drain(from..from + count).collect();
            rows.splice(i..i, moved);
            edits.push(Edit::Move { from, count, to: i });
            count
        } else {
            let mut count = 1;
            while i + count < new.len() && !kept.contains(&new[i + count]) {
                count += 1;
            }
            rows.splice(i..i, new[i..i + count].iter().copied());
            edits.push(Edit::Insert { at: i, count });
            count
        };
        if edits.len() > MAX_EDITS {
            return None;
        }
        i += count;
    }
    Some(edits)
}
