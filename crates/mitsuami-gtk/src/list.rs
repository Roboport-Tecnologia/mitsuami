//! `List`: a `gtk::ListView` in a `gtk::ScrolledWindow`, over a
//! `gio::ListStore` of row keys.
//!
//! The list view virtualises: its factory binds the rows it shows to item
//! widgets and unbinds them when they scroll away, and we report the rows
//! bound (`RowShown`, `RowHidden`) so the core mounts and disposes them.
//! Each item's child is a box that holds the row's host once the core
//! sends it, and is as high as the estimate until then.
//!
//! GTK binds and unbinds during layout, and rebinds a row it keeps when the
//! model changes around it, so the rows bound are compared with the rows
//! reported once the main loop is idle, and only the difference is
//! reported: rows that stay keep their state.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};
use mitsuami_core::{EventValue, ListStyle, NodeId, Point, Rect, RowKey, SelectionMode, UiEvent};

use crate::host::Events;

/// What the list shows, shared with the factory's and the selection's
/// signal handlers.
struct Data {
    rows: Vec<RowKey>,
    index: HashMap<RowKey, usize>,
    /// The mounted rows' hosts.
    hosts: HashMap<RowKey, (NodeId, gtk::Widget)>,
    /// The cells of the rows bound, and how many item widgets each is
    /// bound to (briefly two, while GTK moves a row between them).
    cells: HashMap<RowKey, gtk::Box>,
    bound: HashMap<RowKey, usize>,
    /// The rows reported shown.
    shown: HashSet<RowKey>,
    /// A report of the rows bound is on its way.
    report_queued: bool,
    estimate: Option<i32>,
    /// Without the app's estimate: the first row measured.
    learned: Option<i32>,
    mode: SelectionMode,
    /// GTK can't tell `Automatic` from `Plain`.
    style: Option<ListStyle>,
    /// The width last reported for rows.
    row_width: Option<f64>,
}

impl Data {
    fn estimate(&self) -> i32 {
        self.estimate.or(self.learned).unwrap_or(24)
    }
}

pub(crate) struct List {
    pub scrolled: gtk::ScrolledWindow,
    pub view: gtk::ListView,
    store: gio::ListStore,
    id: NodeId,
    events: Events,
    data: Rc<RefCell<Data>>,
    /// Set while the backend changes the selection itself.
    muted: Rc<Cell<bool>>,
}

fn key_of(item: &glib::Object) -> Option<RowKey> {
    item.downcast_ref::<gtk::StringObject>()?.string().parse().ok().map(RowKey)
}

fn item(key: RowKey) -> gtk::StringObject {
    gtk::StringObject::new(&key.0.to_string())
}

/// Rows exactly as high as their hosts, with nothing between them. The
/// theme draws `show-separators` as a row border, so that one stays.
fn install_css() {
    thread_local!(static INSTALLED: Cell<bool> = const { Cell::new(false) });
    if INSTALLED.replace(true) {
        return;
    }
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "listview.mitsuami-list > row { padding: 0; margin: 0; min-height: 0; }
         listview.mitsuami-list:not(.separators) > row { border: none; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

impl List {
    pub(crate) fn new(id: NodeId, events: Events) -> List {
        install_css();
        let data = Rc::new(RefCell::new(Data {
            rows: Vec::new(),
            index: HashMap::new(),
            hosts: HashMap::new(),
            cells: HashMap::new(),
            bound: HashMap::new(),
            shown: HashSet::new(),
            report_queued: false,
            estimate: None,
            learned: None,
            mode: SelectionMode::None,
            style: None,
            row_width: None,
        }));
        let store = gio::ListStore::new::<gtk::StringObject>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            item.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
        });
        {
            let (data, events) = (data.clone(), events.clone());
            factory.connect_bind(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let (Some(key), Some(cell)) = (item.item().as_ref().and_then(key_of), item.child()) else { return };
                let Ok(cell) = cell.downcast::<gtk::Box>() else { return };
                let mut d = data.borrow_mut();
                match d.hosts.get(&key).map(|(_, host)| host.clone()) {
                    Some(host) => {
                        host.unparent();
                        cell.append(&host);
                        cell.set_height_request(-1);
                    }
                    None => cell.set_height_request(d.estimate()),
                }
                d.cells.insert(key, cell);
                *d.bound.entry(key).or_default() += 1;
                queue_report(&data, &mut d, &events, id);
            });
        }
        {
            let (data, events) = (data.clone(), events.clone());
            factory.connect_unbind(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let (Some(key), Some(cell)) = (item.item().as_ref().and_then(key_of), item.child()) else { return };
                let Ok(cell) = cell.downcast::<gtk::Box>() else { return };
                let mut d = data.borrow_mut();
                // The host goes with the row, wherever it's bound next.
                while let Some(child) = cell.first_child() {
                    cell.remove(&child);
                }
                if d.cells.get(&key) == Some(&cell) {
                    d.cells.remove(&key);
                }
                if let Some(count) = d.bound.get_mut(&key) {
                    *count -= 1;
                    if *count == 0 {
                        d.bound.remove(&key);
                    }
                }
                queue_report(&data, &mut d, &events, id);
            });
        }
        let view = gtk::ListView::new(None::<gtk::NoSelection>, Some(factory));
        view.add_css_class("mitsuami-list");
        // Activation is double-click or Enter, as in Files.
        view.set_single_click_activate(false);
        {
            let (data, events) = (data.clone(), events.clone());
            view.connect_activate(move |_, position| {
                let key = data.borrow().rows.get(position as usize).copied();
                if let Some(key) = key {
                    events.emit(id, UiEvent::RowActivated(key));
                }
            });
        }
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scrolled.set_child(Some(&view));
        let v = scrolled.vadjustment();
        {
            let (events, h) = (events.clone(), scrolled.hadjustment());
            v.connect_value_changed(move |v| {
                events.emit(id, UiEvent::Scrolled(Point::new(h.value() as f32, v.value() as f32)))
            });
        }
        {
            // The view is as wide as its horizontal page: a frame takes
            // room from the rows.
            let (data, events) = (data.clone(), events.clone());
            scrolled.hadjustment().connect_page_size_notify(move |h| {
                let width = h.page_size();
                if width > 0.0 && data.borrow_mut().row_width.replace(width) != Some(width) {
                    events.emit(id, UiEvent::RowWidth(width as f32));
                }
            });
        }
        let list = List { scrolled, view, store, id, events, data, muted: Rc::default() };
        list.set_mode(SelectionMode::None);
        list
    }

    /// New rows, as the fewest changes to the store: rows before and after
    /// the part that changed stay bound. Selected rows that went are
    /// reported.
    pub(crate) fn set_rows(&self, rows: Vec<RowKey>) {
        let selected = self.selected();
        let old = std::mem::take(&mut self.data.borrow_mut().rows);
        let prefix = old.iter().zip(&rows).take_while(|(a, b)| a == b).count();
        let suffix = old[prefix..].iter().rev().zip(rows[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
        {
            let mut data = self.data.borrow_mut();
            data.index = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
            data.rows = rows.clone();
        }
        let added: Vec<glib::Object> = rows[prefix..rows.len() - suffix].iter().map(|k| item(*k).upcast()).collect();
        self.muted.set(true);
        self.store.splice(prefix as u32, (old.len() - prefix - suffix) as u32, &added);
        self.muted.set(false);
        let kept: Vec<RowKey> = {
            let data = self.data.borrow();
            selected.iter().copied().filter(|k| data.index.contains_key(k)).collect()
        };
        // Replaced items lose their selection even when their keys stay.
        self.set_selected(&kept);
        if kept != selected {
            self.report_selection();
        }
    }

    /// GTK has no mode to change, only selection models to swap: the new
    /// one keeps what it can hold of the old one's selection, as AppKit's
    /// tables do, and the rows let go are reported.
    pub(crate) fn set_mode(&self, mode: SelectionMode) {
        let selected = self.selected();
        self.data.borrow_mut().mode = mode;
        let model: gtk::SelectionModel = match mode {
            SelectionMode::None => gtk::NoSelection::new(Some(self.store.clone())).upcast(),
            SelectionMode::Single => {
                let single = gtk::SingleSelection::new(Some(self.store.clone()));
                single.set_autoselect(false);
                single.set_can_unselect(true);
                single.set_selected(gtk::INVALID_LIST_POSITION);
                single.upcast()
            }
            SelectionMode::Multiple => gtk::MultiSelection::new(Some(self.store.clone())).upcast(),
        };
        let (data, events, muted, id) = (self.data.clone(), self.events.clone(), self.muted.clone(), self.id);
        model.connect_selection_changed(move |model, _, _| {
            if !muted.get() {
                let keys = selected_keys(&data.borrow(), model);
                events.emit(id, UiEvent::Changed(EventValue::Rows(keys)));
            }
        });
        self.view.set_model(Some(&model));
        let kept: Vec<RowKey> = match mode {
            SelectionMode::None => Vec::new(),
            SelectionMode::Single => selected.iter().take(1).copied().collect(),
            SelectionMode::Multiple => selected.clone(),
        };
        self.set_selected(&kept);
        if kept != selected {
            self.report_selection();
        }
    }

    pub(crate) fn mode(&self) -> SelectionMode {
        self.data.borrow().mode
    }

    pub(crate) fn set_style(&self, style: ListStyle) {
        self.data.borrow_mut().style = Some(style);
        self.scrolled.set_has_frame(style.framed());
    }

    pub(crate) fn style(&self) -> Option<ListStyle> {
        self.data.borrow().style
    }

    /// Selects rows without reporting it. A single selection only takes
    /// `set_selected`; a multiple one takes a whole bitset.
    pub(crate) fn set_selected(&self, keys: &[RowKey]) {
        let Some(model) = self.view.model() else { return };
        let indexes: Vec<u32> = {
            let data = self.data.borrow();
            keys.iter().filter_map(|k| data.index.get(k).map(|i| *i as u32)).collect()
        };
        self.muted.set(true);
        if let Some(single) = model.downcast_ref::<gtk::SingleSelection>() {
            single.set_selected(indexes.first().copied().unwrap_or(gtk::INVALID_LIST_POSITION));
        } else {
            let selection = gtk::Bitset::new_empty();
            for index in indexes {
                selection.add(index);
            }
            model.set_selection(&selection, &gtk::Bitset::new_range(0, self.store.n_items()));
        }
        self.muted.set(false);
    }

    /// Reports the selection as the user's: a screen reader's select, or
    /// selected rows removed.
    pub(crate) fn report_selection(&self) {
        self.events.emit_always(self.id, UiEvent::Changed(EventValue::Rows(self.selected())));
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        match self.view.model() {
            Some(model) => selected_keys(&self.data.borrow(), &model),
            None => Vec::new(),
        }
    }

    pub(crate) fn set_estimate(&self, height: f32) {
        self.data.borrow_mut().estimate = Some(height.round() as i32);
    }

    pub(crate) fn estimate(&self) -> Option<f32> {
        self.data.borrow().estimate.map(|h| h as f32)
    }

    /// Hosts a mounted row, in its cell if it's bound.
    pub(crate) fn insert(&self, key: RowKey, id: NodeId, host: gtk::Widget) {
        let mut data = self.data.borrow_mut();
        if let Some(cell) = data.cells.get(&key) {
            host.unparent();
            cell.append(&host);
            cell.set_height_request(-1);
        }
        data.hosts.insert(key, (id, host));
    }

    pub(crate) fn remove(&self, key: RowKey) {
        let mut data = self.data.borrow_mut();
        if let Some((_, host)) = data.hosts.remove(&key)
            && let Some(cell) = host.parent().and_then(|p| p.downcast::<gtk::Box>().ok())
        {
            cell.remove(&host);
            cell.set_height_request(data.estimate());
        }
    }

    /// Without the app's estimate, the first row measured sets it.
    pub(crate) fn row_measured(&self, height: f32) {
        let mut data = self.data.borrow_mut();
        if data.estimate.is_none() && data.learned.is_none() && height > 0.0 {
            data.learned = Some(height.round() as i32);
        }
    }

    pub(crate) fn scroll_to_row(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let Some(index) = index {
            let _ = self.view.activate_action("list.scroll-to-item", Some(&(index as u32).to_variant()));
        }
    }

    pub(crate) fn activate(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let Some(index) = index {
            self.view.emit_by_name::<()>("activate", &[&(index as u32)]);
        }
    }

    /// Selects a row as the user would: alone, and shown.
    pub(crate) fn select(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let (Some(index), Some(model)) = (index, self.view.model()) {
            model.select_item(index as u32, true);
            self.scroll_to_row(key);
        }
    }

    pub(crate) fn rows(&self) -> Vec<RowKey> {
        (0..self.store.n_items()).filter_map(|i| self.store.item(i).as_ref().and_then(key_of)).collect()
    }

    /// Where GTK placed a row's host, in the list's content, if it did:
    /// it only allocates the rows in view.
    fn placed(&self, host: &gtk::Widget) -> Option<f32> {
        let row = host.parent()?.parent()?;
        if !row.is_child_visible() || !host.is_mapped() {
            return None;
        }
        let point = host.compute_point(&self.view, &gtk::graphene::Point::new(0.0, 0.0))?;
        Some(point.y() + self.scrolled.vadjustment().value() as f32)
    }

    /// Where a row is in the list's content, at its host's size. GTK keeps
    /// rows around the view that it doesn't place; they're where the rows
    /// between them and the nearest placed row put them (hosts' heights,
    /// or the estimate for rows without one).
    pub(crate) fn row_rect(&self, key: RowKey, frames: &crate::host::Frames) -> Option<Rect> {
        let data = self.data.borrow();
        let frames = frames.borrow();
        let host_of = |key: &RowKey| data.hosts.get(key).map(|(_, host)| host);
        let height =
            |key: &RowKey| host_of(key).and_then(|h| frames.get(h)).map_or(data.estimate() as f32, |f| f.height());
        let index = *data.index.get(&key)?;
        let size = host_of(&key).and_then(|h| frames.get(h)).copied()?.size;
        let placed = |i: usize| data.rows.get(i).and_then(host_of).and_then(|h| self.placed(h));
        let y = match placed(index) {
            Some(y) => y,
            None => (1..=data.rows.len()).find_map(|d| {
                if let Some(above) = index.checked_sub(d).and_then(|i| Some((i, placed(i)?))) {
                    return Some(above.1 + data.rows[above.0..index].iter().map(height).sum::<f32>());
                }
                let below = index + d;
                let y = placed(below)?;
                Some(y - data.rows[index..below].iter().map(height).sum::<f32>())
            })?,
        };
        Some(Rect { origin: Point::new(0.0, y), size })
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

fn selected_keys(data: &Data, model: &gtk::SelectionModel) -> Vec<RowKey> {
    let selection = model.selection();
    (0..selection.size()).filter_map(|i| data.rows.get(selection.nth(i as u32) as usize).copied()).collect()
}

/// Reports the rows bound and unbound since the last report, once the main
/// loop is idle: a row GTK unbinds and binds again in one layout is kept.
fn queue_report(data: &Rc<RefCell<Data>>, d: &mut Data, events: &Events, id: NodeId) {
    if d.report_queued {
        return;
    }
    d.report_queued = true;
    let (data, events) = (data.clone(), events.clone());
    glib::idle_add_local_once(move || {
        let mut d = data.borrow_mut();
        d.report_queued = false;
        let now: HashSet<RowKey> = d.bound.keys().copied().collect();
        let mut hidden: Vec<RowKey> = d.shown.difference(&now).copied().collect();
        let mut shown: Vec<RowKey> = now.difference(&d.shown).copied().collect();
        hidden.sort_by_key(|k| d.index.get(k).copied());
        shown.sort_by_key(|k| d.index.get(k).copied());
        for row in hidden {
            events.emit(id, UiEvent::RowHidden(row));
        }
        for row in shown {
            events.emit(id, UiEvent::RowShown(row));
        }
        d.shown = now;
    });
}
