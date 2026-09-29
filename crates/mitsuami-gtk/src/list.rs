//! `List` and `Table`: a `gtk::ListView`, or a `gtk::ColumnView` with a
//! column per table column, in a `gtk::ScrolledWindow`, over a
//! `gio::ListStore` of row keys.
//!
//! The view virtualises: its factories bind the rows it shows to item
//! widgets and unbind them when they scroll away, and we report the rows
//! bound (`RowShown`, `RowHidden`) so the core mounts and disposes them.
//! Each item's child holds its host once the core sends it (a list's row
//! host, a table's cell host), and is as high as the estimate until then.
//!
//! GTK binds and unbinds during layout, and rebinds a row it keeps when the
//! model changes around it, so the rows bound are compared with the rows
//! reported once the main loop is idle, and only the difference is
//! reported: rows that stay keep their state.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, graphene, gsk};
use mitsuami_core::{
    ColumnData, ColumnSort, EventValue, ListStyle, NodeId, Point, Rect, RowKey, SelectionMode, SortOrder, UiEvent,
};

use crate::host::Events;

/// A cell: its row, and its column (a list's is 0).
type Slot = (RowKey, usize);

/// Reports a table column's cells' width, from a cell as it's allocated.
type WidthReport = Rc<dyn Fn(usize, i32)>;

/// What the list shows, shared with the factories' and the selection's
/// signal handlers.
struct Data {
    rows: Vec<RowKey>,
    index: HashMap<RowKey, usize>,
    /// The mounted hosts: a list's rows', a table's cells'.
    hosts: HashMap<Slot, (NodeId, gtk::Widget)>,
    /// The cells bound (a list's boxes, a table's `TableCell`s), and how
    /// many item widgets each is bound to (briefly two, while GTK moves a
    /// row between them).
    cells: HashMap<Slot, gtk::Widget>,
    bound: HashMap<Slot, usize>,
    /// Tables only: the columns as sent, and the widths reported.
    columns: Vec<ColumnData>,
    widths: Vec<f32>,
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

/// The view: a list's, or a table's.
enum View {
    List(gtk::ListView),
    Table(gtk::ColumnView),
}

pub(crate) struct List {
    pub scrolled: gtk::ScrolledWindow,
    /// The `gtk::ListView`, or a table's `gtk::ColumnView`.
    pub view: gtk::Widget,
    typed: View,
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

fn new_data() -> Rc<RefCell<Data>> {
    Rc::new(RefCell::new(Data {
        rows: Vec::new(),
        index: HashMap::new(),
        hosts: HashMap::new(),
        cells: HashMap::new(),
        bound: HashMap::new(),
        columns: Vec::new(),
        widths: Vec::new(),
        shown: HashSet::new(),
        report_queued: false,
        estimate: None,
        learned: None,
        mode: SelectionMode::None,
        style: None,
        row_width: None,
    }))
}

/// Puts a host in its cell: a list's box, or a table's cell.
fn attach(cell: &gtk::Widget, host: &gtk::Widget) {
    host.unparent();
    match cell.downcast_ref::<gtk::Box>() {
        Some(cell) => cell.append(host),
        None => host.set_parent(cell),
    }
    cell.set_height_request(-1);
}

/// Takes the hosts out of a cell, which waits at the estimate's height.
fn detach(cell: &gtk::Widget) {
    while let Some(child) = cell.first_child() {
        match cell.downcast_ref::<gtk::Box>() {
            Some(cell) => cell.remove(&child),
            None => child.unparent(),
        }
    }
}

/// A factory binding one column's cells: a list's one, or a table's. `make`
/// makes an empty cell.
fn factory(
    data: &Rc<RefCell<Data>>,
    events: &Events,
    id: NodeId,
    column: usize,
    make: impl Fn() -> gtk::Widget + 'static,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
        item.set_child(Some(&make()));
    });
    {
        let (data, events) = (data.clone(), events.clone());
        factory.connect_bind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let (Some(key), Some(cell)) = (item.item().as_ref().and_then(key_of), item.child()) else { return };
            let mut d = data.borrow_mut();
            match d.hosts.get(&(key, column)).map(|(_, host)| host.clone()) {
                Some(host) => attach(&cell, &host),
                None => cell.set_height_request(d.estimate()),
            }
            d.cells.insert((key, column), cell);
            *d.bound.entry((key, column)).or_default() += 1;
            queue_report(&data, &mut d, &events, id);
        });
    }
    {
        let (data, events) = (data.clone(), events.clone());
        factory.connect_unbind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let (Some(key), Some(cell)) = (item.item().as_ref().and_then(key_of), item.child()) else { return };
            let mut d = data.borrow_mut();
            // The host goes with the row, wherever it's bound next.
            detach(&cell);
            if d.cells.get(&(key, column)) == Some(&cell) {
                d.cells.remove(&(key, column));
            }
            if let Some(count) = d.bound.get_mut(&(key, column)) {
                *count -= 1;
                if *count == 0 {
                    d.bound.remove(&(key, column));
                }
            }
            queue_report(&data, &mut d, &events, id);
        });
    }
    factory
}

impl List {
    pub(crate) fn new(id: NodeId, events: Events) -> List {
        install_css();
        let data = new_data();
        let factory = factory(&data, &events, id, 0, || gtk::Box::new(gtk::Orientation::Vertical, 0).upcast());
        let view = gtk::ListView::new(None::<gtk::NoSelection>, Some(factory));
        view.add_css_class("mitsuami-list");
        // Activation is double-click or Enter, as in Files.
        view.set_single_click_activate(false);
        {
            let (data, events) = (data.clone(), events.clone());
            view.connect_activate(move |_, position| activated(&data, &events, id, position));
        }
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
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
        List::finish(scrolled, View::List(view), id, events, data)
    }

    /// A table: a column view, with the theme's own cells and header. Its
    /// columns come with `set_columns`. Its columns can be wider than it.
    pub(crate) fn new_table(id: NodeId, events: Events) -> List {
        let data = new_data();
        let view = gtk::ColumnView::new(None::<gtk::NoSelection>);
        view.set_single_click_activate(false);
        view.set_reorderable(false);
        {
            let (data, events) = (data.clone(), events.clone());
            view.connect_activate(move |_, position| activated(&data, &events, id, position));
        }
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        let list = List::finish(scrolled, View::Table(view.clone()), id, events, data);
        // Pressing a sortable header changes the view's sorter: the app
        // sorts.
        if let Some(sorter) = view.sorter() {
            let (view, events, muted) = (view.clone(), list.events.clone(), list.muted.clone());
            sorter.connect_changed(move |_, _| {
                if !muted.get()
                    && let Some(sort) = sort_of(&view)
                {
                    events.emit(id, UiEvent::Changed(EventValue::Sort(sort)));
                }
            });
        }
        list
    }

    fn finish(scrolled: gtk::ScrolledWindow, typed: View, id: NodeId, events: Events, data: Rc<RefCell<Data>>) -> List {
        let view: gtk::Widget = match &typed {
            View::List(v) => v.clone().upcast(),
            View::Table(v) => v.clone().upcast(),
        };
        scrolled.set_child(Some(&view));
        let v = scrolled.vadjustment();
        {
            let (events, h) = (events.clone(), scrolled.hadjustment());
            v.connect_value_changed(move |v| {
                events.emit(id, UiEvent::Scrolled(Point::new(h.value() as f32, v.value() as f32)))
            });
        }
        if matches!(typed, View::Table(_)) {
            let (events, v) = (events.clone(), v.clone());
            scrolled.hadjustment().connect_value_changed(move |h| {
                events.emit(id, UiEvent::Scrolled(Point::new(h.value() as f32, v.value() as f32)))
            });
        }
        let store = gio::ListStore::new::<gtk::StringObject>();
        let list = List { scrolled, view, typed, store, id, events, data, muted: Rc::default() };
        list.set_mode(SelectionMode::None);
        list
    }

    /// A table's columns: titled, resizable, at the width the app gave (or
    /// 100, as AppKit's and the core's default), the ones that expand
    /// sharing the room left, and a sorter on the sortable ones, which
    /// makes their headers sort. Each binds its own cells.
    pub(crate) fn set_columns(&self, columns: &[ColumnData]) {
        let View::Table(view) = &self.typed else { return };
        let sort = sort_of(view);
        self.muted.set(true);
        while let Some(column) = view.columns().item(0).and_downcast::<gtk::ColumnViewColumn>() {
            view.remove_column(&column);
        }
        {
            let mut data = self.data.borrow_mut();
            data.columns = columns.to_vec();
            data.widths = columns.iter().map(|c| c.width.unwrap_or(100.0)).collect();
        }
        for (index, column) in columns.iter().enumerate() {
            let data = self.data.clone();
            let report = self.width_reporter();
            let make = move || {
                let cell = TableCell::new(index, report.clone());
                // Empty until its host comes, at the estimate's height.
                cell.set_height_request(data.borrow().estimate());
                cell.upcast()
            };
            let factory = factory(&self.data, &self.events, self.id, index, make);
            let native = gtk::ColumnViewColumn::new(Some(&column.title), Some(factory));
            native.set_resizable(true);
            native.set_expand(column.expand);
            match column.width {
                Some(width) => native.set_fixed_width(width.round() as i32),
                None if !column.expand => native.set_fixed_width(100),
                None => {}
            }
            if column.sortable {
                native.set_sorter(Some(&gtk::CustomSorter::new(|_, _| gtk::Ordering::Equal)));
            }
            view.append_column(&native);
        }
        self.set_sort(sort.filter(|s| columns.get(s.column).is_some_and(|c| c.sortable)));
        self.muted.set(false);
    }

    pub(crate) fn columns(&self) -> Vec<ColumnData> {
        self.data.borrow().columns.clone()
    }

    /// Reports a column's cells' width when it changes, from the cells as
    /// GTK allocates them (inside the theme's cell padding).
    fn width_reporter(&self) -> WidthReport {
        let (data, events, id) = (Rc::downgrade(&self.data), self.events.clone(), self.id);
        Rc::new(move |column, width| {
            let Some(data) = data.upgrade() else { return };
            let Ok(mut d) = data.try_borrow_mut() else { return };
            let width = width as f32;
            match d.widths.get_mut(column) {
                Some(w) if *w != width => *w = width,
                _ => return,
            }
            let widths = d.widths.clone();
            drop(d);
            events.emit(id, UiEvent::ColumnWidths(widths));
        })
    }

    /// Shows the sort in the header, without reporting it.
    pub(crate) fn set_sort(&self, sort: Option<ColumnSort>) {
        let View::Table(view) = &self.typed else { return };
        let column = sort.and_then(|s| view.columns().item(s.column as u32)).and_downcast::<gtk::ColumnViewColumn>();
        let order = match sort.map(|s| s.order) {
            Some(SortOrder::Descending) => gtk::SortType::Descending,
            _ => gtk::SortType::Ascending,
        };
        let muted = self.muted.replace(true);
        view.sort_by_column(column.as_ref(), order);
        self.muted.set(muted);
    }

    pub(crate) fn sort(&self) -> Option<ColumnSort> {
        match &self.typed {
            View::Table(view) => sort_of(view),
            View::List(_) => None,
        }
    }

    /// Presses a sortable column's header, as a click does: the same column
    /// the other way round, another one ascending. The sorter reports it.
    pub(crate) fn press_header(&self, column: usize) -> bool {
        let View::Table(view) = &self.typed else { return false };
        let Some(native) = view.columns().item(column as u32).and_downcast::<gtk::ColumnViewColumn>() else {
            return false;
        };
        if native.sorter().is_none() {
            return false;
        }
        let order = match sort_of(view) {
            Some(sort) if sort.column == column && sort.order == SortOrder::Ascending => gtk::SortType::Descending,
            _ => gtk::SortType::Ascending,
        };
        view.sort_by_column(Some(&native), order);
        true
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
        let focused = self.focused();
        self.muted.set(true);
        self.store.splice(prefix as u32, (old.len() - prefix - suffix) as u32, &added);
        self.muted.set(false);
        // A removed row's item widget leaves the view, but the window
        // keeps it as its focus, without saying so. Focus stays in the
        // list, as when a mode change swaps the model.
        if focused && !self.focused() {
            self.view.grab_focus();
        }
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

    /// Whether the focus is in the view: on it or on a row.
    fn focused(&self) -> bool {
        self.view.root().and_then(|r| r.focus()).is_some_and(|f| f.is_ancestor(&self.view))
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
        // The swap takes the focused row out of the view, but the window
        // keeps it as its focus, so keys go nowhere. Focus stays in the
        // list, as in AppKit's tables.
        let focused = self.focused();
        match &self.typed {
            View::List(view) => view.set_model(Some(&model)),
            View::Table(view) => view.set_model(Some(&model)),
        }
        if focused {
            self.view.grab_focus();
        }
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
        let Some(model) = self.model() else { return };
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

    /// The selection model: the view's.
    fn model(&self) -> Option<gtk::SelectionModel> {
        match &self.typed {
            View::List(view) => view.model(),
            View::Table(view) => view.model(),
        }
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        match self.model() {
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

    /// Hosts a mounted row, or a table's cell, in its cell if it's bound.
    pub(crate) fn insert(&self, key: RowKey, column: usize, id: NodeId, host: gtk::Widget) {
        let mut data = self.data.borrow_mut();
        if let Some(cell) = data.cells.get(&(key, column)) {
            attach(cell, &host);
        }
        data.hosts.insert((key, column), (id, host));
    }

    pub(crate) fn remove(&self, key: RowKey, column: usize) {
        let mut data = self.data.borrow_mut();
        if let Some((_, host)) = data.hosts.remove(&(key, column))
            && let Some(cell) = host.parent()
            && data.cells.get(&(key, column)) == Some(&cell)
        {
            detach(&cell);
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

    /// The list view that scrolls to items: the list's, or the one inside
    /// a table's column view (`ColumnView::scroll_to` needs GTK 4.12).
    fn list_view(&self) -> Option<gtk::Widget> {
        match &self.typed {
            View::List(view) => Some(view.clone().upcast()),
            View::Table(view) => {
                let mut child = view.first_child();
                while let Some(c) = child {
                    if c.is::<gtk::ListView>() {
                        return Some(c);
                    }
                    child = c.next_sibling();
                }
                None
            }
        }
    }

    pub(crate) fn scroll_to_row(&self, key: RowKey) {
        let index = self.data.borrow().index.get(&key).copied();
        if let (Some(index), Some(view)) = (index, self.list_view()) {
            let _ = view.activate_action("list.scroll-to-item", Some(&(index as u32).to_variant()));
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
        if let (Some(index), Some(model)) = (index, self.model()) {
            model.select_item(index as u32, true);
            self.scroll_to_row(key);
        }
    }

    pub(crate) fn rows(&self) -> Vec<RowKey> {
        (0..self.store.n_items()).filter_map(|i| self.store.item(i).as_ref().and_then(key_of)).collect()
    }

    /// Where GTK placed a host, in the view's content (a table's below its
    /// header), if it did: it only allocates the rows in view.
    fn placed_at(&self, host: &gtk::Widget) -> Option<Point> {
        let row = host.parent()?.parent()?;
        if !row.is_child_visible() || !host.is_mapped() {
            return None;
        }
        let point = host.compute_point(&self.view, &gtk::graphene::Point::new(0.0, 0.0))?;
        let (h, v) = (self.scrolled.hadjustment().value() as f32, self.scrolled.vadjustment().value() as f32);
        Some(Point::new(point.x() + h, point.y() + v))
    }

    fn placed(&self, host: &gtk::Widget) -> Option<f32> {
        self.placed_at(host).map(|p| p.y)
    }

    /// Where a row is in the list's content, at its host's size. GTK keeps
    /// rows around the view that it doesn't place; they're where the rows
    /// between them and the nearest placed row put them (hosts' heights,
    /// or the estimate for rows without one).
    pub(crate) fn row_rect(&self, key: RowKey, frames: &crate::host::Frames) -> Option<Rect> {
        let data = self.data.borrow();
        let frames = frames.borrow();
        let host_of = |key: &RowKey| data.hosts.get(&(*key, 0)).map(|(_, host)| host);
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

    /// Where a table put a cell's host, in its content, at the host's
    /// size: where GTK placed it, or else across from where it placed the
    /// column's other cells, below the row placed nearest (as `row_rect`,
    /// with each row as high as its highest cell).
    pub(crate) fn cell_rect(&self, key: RowKey, column: usize, frames: &crate::host::Frames) -> Option<Rect> {
        let data = self.data.borrow();
        let size = data.hosts.get(&(key, column)).and_then(|(_, h)| frames.borrow().get(h).copied())?.size;
        if let Some(origin) = data.hosts.get(&(key, column)).and_then(|(_, h)| self.placed_at(h)) {
            return Some(Rect { origin, size });
        }
        let frames = frames.borrow();
        let (columns, all) = (data.columns.len().max(1), &data.hosts);
        let hosts = |key: &RowKey| {
            let key = *key;
            (0..columns).filter_map(move |c| all.get(&(key, c)).map(|(_, h)| h))
        };
        let height = |key: &RowKey| {
            hosts(key)
                .filter_map(|h| frames.get(h))
                .map(|f| f.height())
                .reduce(f32::max)
                .unwrap_or(data.estimate() as f32)
        };
        let placed = |i: usize| data.rows.get(i).and_then(|k| hosts(k).find_map(|h| self.placed_at(h)));
        let x = data.hosts.iter().filter(|((_, c), _)| *c == column).find_map(|(_, (_, h))| self.placed_at(h))?.x;
        let index = *data.index.get(&key)?;
        let y = (1..=data.rows.len()).find_map(|d| {
            if let Some((i, p)) = index.checked_sub(d).and_then(|i| Some((i, placed(i)?))) {
                return Some(p.y + data.rows[i..index].iter().map(height).sum::<f32>());
            }
            let below = index + d;
            let p = placed(below)?;
            Some(p.y - data.rows[index..below].iter().map(height).sum::<f32>())
        })?;
        Some(Rect { origin: Point::new(x, y), size })
    }

    /// The hosts, in row order, then column order.
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
        let now: HashSet<RowKey> = d.bound.keys().map(|(key, _)| *key).collect();
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

/// A row was activated: double-clicked, or Enter pressed on it.
fn activated(data: &Rc<RefCell<Data>>, events: &Events, id: NodeId, position: u32) {
    let key = data.borrow().rows.get(position as usize).copied();
    if let Some(key) = key {
        events.emit(id, UiEvent::RowActivated(key));
    }
}

/// A table's sort, as its sorter has it: its primary column and order.
fn sort_of(view: &gtk::ColumnView) -> Option<ColumnSort> {
    let sorter = view.sorter()?.downcast::<gtk::ColumnViewSorter>().ok()?;
    let primary = sorter.primary_sort_column()?;
    let columns = view.columns();
    let column = (0..columns.n_items()).find(|i| columns.item(*i).as_ref() == Some(primary.upcast_ref()))? as usize;
    let order = match sorter.primary_sort_order() {
        gtk::SortType::Descending => SortOrder::Descending,
        _ => SortOrder::Ascending,
    };
    Some(ColumnSort { column, order })
}

mod imp {
    use super::*;

    /// A table's cell: it holds its host at the host's size, centred in
    /// the row's height, and asks for no width, so the column's width is
    /// the column's (the host's width follows it, as the core lays it out
    /// at the width reported).
    #[derive(Default)]
    pub(crate) struct TableCell {
        pub(super) column: Cell<usize>,
        pub(super) report: RefCell<Option<WidthReport>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TableCell {
        const NAME: &'static str = "MitsuamiTableCell";
        type Type = super::TableCell;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for TableCell {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for TableCell {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                return (0, 0, -1, -1);
            }
            let height = self.obj().first_child().map_or(0, |c| c.measure(gtk::Orientation::Vertical, -1).0);
            (height, height, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                let (w, ..) = child.measure(gtk::Orientation::Horizontal, -1);
                let (h, ..) = child.measure(gtk::Orientation::Vertical, w);
                let y = ((height - h) / 2).max(0);
                let transform = gsk::Transform::new().translate(&graphene::Point::new(0.0, y as f32));
                child.allocate(w, h, -1, Some(transform));
            }
            let report = self.report.borrow().clone();
            if let Some(report) = report {
                report(self.column.get(), width);
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct TableCell(ObjectSubclass<imp::TableCell>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl TableCell {
    fn new(column: usize, report: WidthReport) -> TableCell {
        let cell: TableCell = glib::Object::new();
        cell.imp().column.set(column);
        *cell.imp().report.borrow_mut() = Some(report);
        cell
    }
}
