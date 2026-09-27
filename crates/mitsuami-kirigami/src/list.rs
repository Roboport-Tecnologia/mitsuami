//! `List`: a QML `ListView` over the row keys, with Qt Quick Controls'
//! item delegates, in the style's scroll view (see
//! [`qml::list`](crate::qml::list)).
//!
//! The list view virtualises: it creates delegates for the rows in view
//! (and its cache buffer) and destroys them when they scroll away. The
//! delegates say so, coalesced to once per event loop turn, and the rows
//! with a delegate are compared with the rows reported: only the difference
//! is reported (`RowShown`, `RowHidden`), so a row whose delegate is
//! recreated (a data change resets the view) keeps its state. Each row's
//! host goes in its delegate.
//!
//! The list view's signals come from Qt's event processing, never from our
//! own `apply`; they still only touch the list's own data and emit.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mitsuami_core::{EventValue, ListStyle, NodeId, Point, Rect, RowKey, SelectionMode, UiEvent};

use crate::events::Events;
use crate::ffi::QmlObject;
use crate::qml;

#[derive(Default)]
struct Data {
    rows: Vec<RowKey>,
    index: HashMap<RowKey, usize>,
    /// The mounted rows' hosts.
    hosts: HashMap<RowKey, (NodeId, QmlObject)>,
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
}

pub(crate) struct List {
    /// The scroll view: the item that stands for the list.
    pub root: QmlObject,
    /// The list view in it.
    pub view: QmlObject,
    id: NodeId,
    events: Events,
    data: Rc<RefCell<Data>>,
}

fn parse(key: &str) -> Option<RowKey> {
    key.parse().ok().map(RowKey)
}

impl List {
    pub(crate) fn new(id: NodeId, events: Events) -> List {
        let root = QmlObject::load(&qml::list());
        let view = root.child("mitsuamiListView").expect("lists have a list view");
        let data = Rc::new(RefCell::new(Data::default()));
        {
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
        for signal in ["contentYChanged()", "originYChanged()"] {
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
        self.view.set_bool("mitsuamiMuted", true);
        self.view.set_int("currentIndex", current.map_or(-1, |i| i as i32));
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

    /// Hosts a mounted row, in its delegate if the view has one.
    pub(crate) fn insert(&self, key: RowKey, id: NodeId, host: QmlObject) {
        self.data.borrow_mut().hosts.insert(key, (id, host));
        if let Some(delegate) = delegate_of(self.view, key) {
            place(host, delegate);
        }
    }

    pub(crate) fn remove(&self, key: RowKey) {
        let Some((_, host)) = self.data.borrow_mut().hosts.remove(&key) else { return };
        if let Some(delegate) = host.object("parent") {
            delegate.set_object("mitsuamiHost", None);
        }
        host.set_parent_item(None, 0);
    }

    pub(crate) fn scroll_to(&self, offset: Point) {
        self.view.set_real("contentY", offset.y as f64 + self.view.real("originY"));
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

    /// Where the view put a row's host (its delegate), in its content.
    pub(crate) fn row_rect(&self, host: QmlObject, size: Rect) -> Rect {
        match host.object("parent") {
            Some(delegate) if !delegate.str("mitsuamiKey").is_empty() => {
                Rect::new(0.0, (delegate.real("y") - self.view.real("originY")) as f32, size.width(), size.height())
            }
            _ => size,
        }
    }

    pub(crate) fn rows(&self) -> Vec<RowKey> {
        let count = self.view.int("count").max(0) as usize;
        let data = self.data.borrow();
        data.rows.iter().take(count).copied().collect()
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

/// The content's offset: `ListView` content starts at `originY`, which
/// moves as rows turn out taller or shorter than estimated.
fn scroll_offset(view: QmlObject) -> Point {
    Point::new(0.0, (view.real("contentY") - view.real("originY")) as f32)
}

fn selected(view: QmlObject) -> Vec<RowKey> {
    view.str("mitsuamiSelectedKeys").split(',').filter_map(parse).collect()
}

/// The delegates the view has, and their rows.
fn delegates(view: QmlObject) -> Vec<(RowKey, QmlObject)> {
    let Some(content) = view.object("contentItem") else { return Vec::new() };
    content.child_items().into_iter().filter_map(|d| Some((parse(&d.str("mitsuamiKey"))?, d))).collect()
}

fn delegate_of(view: QmlObject, key: RowKey) -> Option<QmlObject> {
    delegates(view).into_iter().find(|(k, _)| *k == key).map(|(_, d)| d)
}

/// Puts a host in its row's delegate, above the delegate's background.
fn place(host: QmlObject, delegate: QmlObject) {
    if host.object("parent") != Some(delegate) {
        host.set_parent_item(Some(delegate), delegate.child_items().len());
    }
    delegate.set_object("mitsuamiHost", Some(host));
}

/// Delegates came or went: hosts go in the new ones, and the rows with a
/// delegate are compared with the rows reported.
fn rows_changed(view: QmlObject, data: &Rc<RefCell<Data>>, events: &Events, id: NodeId) {
    let delegates = delegates(view);
    let mut d = data.borrow_mut();
    for (key, delegate) in &delegates {
        if let Some((_, host)) = d.hosts.get(key) {
            place(*host, *delegate);
        }
    }
    let now: HashSet<RowKey> = delegates.iter().map(|(k, _)| *k).collect();
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
