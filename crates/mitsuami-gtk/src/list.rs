//! `List`, `Table` and `Tree`: a `gtk::ListView`, or a `gtk::ColumnView`
//! with a column per table column, in a `gtk::ScrolledWindow`, over a
//! `gio::ListStore` of row keys; a tree's over a `gtk::TreeListModel` of
//! a store of its top rows' keys and one of each opening row's children,
//! each row in a `gtk::TreeExpander`.
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
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;

use gtk::glib::translate::from_glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene, gsk};
use mitsuami_core::{
    ColumnData, ColumnSort, EventValue, ListStyle, NodeId, Point, Rect, RowDrop, RowKey, RowMoves, SelectionMode,
    SortOrder, TreeIndent, TreeRow, UiEvent,
};

use crate::host::Events;

/// A cell: its row, and its column (a list's is 0).
type Slot = (RowKey, usize);

/// Reports a table column's cells' width, from a cell as it's allocated.
type WidthReport = Rc<dyn Fn(usize, i32)>;

/// Hears a tree's cell allocated, to read how far GTK indents it.
type Placed = Rc<dyn Fn(&gtk::Widget)>;

/// What the list shows, shared with the factories' and the selection's
/// signal handlers.
struct Data {
    rows: Vec<RowKey>,
    index: HashMap<RowKey, usize>,
    /// The mounted hosts: a list's rows', a table's cells'.
    hosts: HashMap<Slot, (NodeId, gtk::Widget)>,
    /// The cells bound (`ItemCell`s), and how
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
    /// A row scrolled to before its host had its size: GTK places it by
    /// the size it had, so it's scrolled to again once it's measured.
    scrolling_to: Option<RowKey>,
    mode: SelectionMode,
    /// GTK can't tell `Automatic` from `Plain`.
    style: Option<ListStyle>,
    /// The width last reported for rows.
    row_width: Option<f64>,
    /// The rows' files, as the app gave them (`Prop::RowFiles`), and by row.
    files: Option<Vec<(RowKey, PathBuf)>>,
    file_of: HashMap<RowKey, PathBuf>,
    /// Where rows dragged within it can go (`Prop::RowMoves`).
    moves: Option<RowMoves>,
    /// The cell a drag over the list is shown at.
    drop_cell: Option<glib::WeakRef<gtk::Widget>>,
    /// Tables with a column that fills its rows only: the shown rows'
    /// heights without those cells, as reported, and whether a report is on
    /// its way.
    bare_heights: HashMap<RowKey, f32>,
    heights_queued: bool,
    /// The natural height last measured once idle, whether it was, and
    /// whether a measure is on its way (`List::queue_remeasure`).
    natural: f32,
    remeasured: bool,
    remeasure_queued: bool,
    /// Trees only: each row that shows, as sent; a store of the children
    /// of each that opens, which the tree list model shows while it's
    /// open; the handler hearing each row's `expanded`, on the row object
    /// it was connected to; whether the backend is opening and closing rows
    /// itself; and the indent last reported.
    tree: HashMap<RowKey, TreeRow>,
    stores: HashMap<RowKey, gio::ListStore>,
    expanders: HashMap<RowKey, (glib::WeakRef<gtk::TreeListRow>, glib::SignalHandlerId)>,
    expanding: bool,
    indent: Option<TreeIndent>,
    /// Some row opens: rows that don't are indented by the disclosure's
    /// width, as GTK's trees indent them. A tree where none does is a flat
    /// list, unindented, as GTK's apps show one.
    any_opens: bool,
    /// The rows whose children are being read, as sent (`TreeLoading`).
    loading: Option<HashSet<RowKey>>,
}

/// What a drag of rows within a list carries: the list's id, so another
/// list refuses it, and the rows' keys. A drag within the app hands its
/// value over as it is; other apps can't read it.
#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "MitsuamiDraggedRows")]
struct DraggedRows {
    list: u32,
    rows: Vec<u64>,
}

/// Where a drop lands, by `RowMoves::target`: the rows (those still in the
/// list, in its order) and where. The drop targets and the test hook both
/// ask it.
fn landing(
    data: &Data,
    rows: &[RowKey],
    over: Option<RowKey>,
    proposed: RowDrop<RowKey>,
) -> Option<(Vec<RowKey>, RowDrop<RowKey>)> {
    let moves = data.moves?;
    let mut rows: Vec<RowKey> = rows.iter().filter(|row| data.index.contains_key(row)).copied().collect();
    rows.sort_by_key(|row| data.index[row]);
    // A tree's rows nest: only folders take rows onto them, and nothing
    // lands over a dragged row.
    let to = match data.tree.is_empty() {
        true => moves.target(&rows, proposed)?,
        false => moves.tree_target(&rows, over, proposed, |row| data.tree.get(&row).copied())?,
    };
    (!rows.is_empty()).then_some((rows, to))
}

/// What a drag at `x`, `y` over `key`'s cell, `height` high, proposes:
/// before the row in its top quarter, before the next one (or the end) in
/// its bottom quarter, onto it in between. A tree's, as every tree's
/// (`RowMoves::tree_proposal`), with `x` across the tree: a cell starts
/// where its row's content does.
fn proposal(data: &Data, key: RowKey, x: f64, y: f64, height: f64) -> RowDrop<RowKey> {
    if let Some(row) = data.tree.get(&key) {
        let next = data.index.get(&key).and_then(|i| data.rows.get(i + 1)).copied();
        let indent = data.indent.unwrap_or_default();
        let x = data.indent.map(|i| x as f32 + i.at(row.depth));
        return RowMoves::tree_proposal(key, x, y as f32, height as f32, indent, next, |r| data.tree.get(&r).copied());
    }
    if y < height / 4.0 {
        return RowDrop::Before(key);
    }
    if y > height * 3.0 / 4.0 {
        let next = data.index.get(&key).and_then(|i| data.rows.get(i + 1));
        return next.map_or(RowDrop::End, |next| RowDrop::Before(*next));
    }
    RowDrop::On(key)
}

impl Data {
    fn estimate(&self) -> i32 {
        self.estimate.or(self.learned).unwrap_or(24)
    }
}

/// The view: a list's, a table's, or a tree's (a list view over the tree
/// list model).
enum View {
    List(gtk::ListView),
    Table(gtk::ColumnView),
    Tree(gtk::ListView, gtk::TreeListModel),
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

/// A row's key: its item's, or, in a tree, its tree list row's item's.
fn key_of(item: &glib::Object) -> Option<RowKey> {
    if let Some(row) = item.downcast_ref::<gtk::TreeListRow>() {
        return key_of(&row.item()?);
    }
    item.downcast_ref::<gtk::StringObject>()?.string().parse().ok().map(RowKey)
}

/// Makes a store hold these keys, by the fewest changes: the items before
/// and after the part that changed stay, and their rows stay bound.
fn splice_keys(store: &gio::ListStore, keys: &[RowKey]) {
    let old: Vec<Option<RowKey>> = (0..store.n_items()).map(|i| store.item(i).as_ref().and_then(key_of)).collect();
    let prefix = old.iter().zip(keys).take_while(|(a, b)| **a == Some(**b)).count();
    let suffix =
        old[prefix..].iter().rev().zip(keys[prefix..].iter().rev()).take_while(|(a, b)| **a == Some(**b)).count();
    if prefix == old.len() && prefix == keys.len() {
        return;
    }
    let added: Vec<glib::Object> = keys[prefix..keys.len() - suffix].iter().map(|k| item(*k).upcast()).collect();
    store.splice(prefix as u32, (old.len() - prefix - suffix) as u32, &added);
}

fn item(key: RowKey) -> gtk::StringObject {
    gtk::StringObject::new(&key.0.to_string())
}

/// Rows exactly as high as their hosts, with nothing between them. The
/// theme draws `show-separators` as a row border, so that one stays. A
/// table's cells that fill their rows have none of the theme's padding.
fn install_css() {
    thread_local!(static INSTALLED: Cell<bool> = const { Cell::new(false) });
    if INSTALLED.replace(true) {
        return;
    }
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "listview.mitsuami-list > row { padding: 0; margin: 0; min-height: 0; }
         listview.mitsuami-list:not(.separators) > row { border: none; }
         columnview > listview > row > cell.mitsuami-fills-row { padding: 0; }
         .mitsuami-drop-on { outline: 2px solid @accent_bg_color; outline-offset: -2px; }
         .mitsuami-cell:drop(active) { box-shadow: none; }
         .mitsuami-drop-line { background-color: @accent_bg_color; }",
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
        scrolling_to: None,
        mode: SelectionMode::None,
        style: None,
        row_width: None,
        files: None,
        file_of: HashMap::new(),
        moves: None,
        drop_cell: None,
        bare_heights: HashMap::new(),
        heights_queued: false,
        natural: 0.0,
        remeasured: false,
        remeasure_queued: false,
        tree: HashMap::new(),
        stores: HashMap::new(),
        expanders: HashMap::new(),
        expanding: false,
        indent: None,
        any_opens: false,
        loading: None,
    }))
}

/// The rows dragging `key` carries: the selection when it's selected, as
/// GTK's lists drag, else the row.
fn dragged_rows(key: RowKey, model: Option<&gtk::SelectionModel>) -> Vec<RowKey> {
    let selected = model.map(selected_keys).unwrap_or_default();
    if selected.contains(&key) { selected } else { vec![key] }
}

/// Shows where a drag over `cell` lands: a ring on the row it drops onto,
/// a line above or below it otherwise, over the row's content. `None`
/// takes it away.
fn show_drop(data: &mut Data, cell: Option<(&gtk::Widget, RowKey, RowDrop<RowKey>)>) {
    if let Some(old) = data.drop_cell.take().and_then(|w| w.upgrade()) {
        old.remove_css_class("mitsuami-drop-on");
        if let Some(old) = old.downcast_ref::<ItemCell>() {
            old.show_line(None);
        }
    }
    let Some((cell, key, to)) = cell else { return };
    // A line goes at the top of the row the rows go before, where it's
    // bound: in a tree its indent says the level they land at, which the
    // row under the pointer's needn't (the last row in a folder, before the
    // row after the folder).
    // The end of a folder from a row inside it: under that row, from
    // where the folder's rows start, left of the row's own if it's deeper.
    let level = |folder: RowKey| {
        let indent = data.indent.unwrap_or_default();
        let (Some(folder), Some(row)) = (data.tree.get(&folder), data.tree.get(&key)) else { return 0.0 };
        indent.at(folder.depth + 1) - indent.at(row.depth)
    };
    let (cell, edge, x) = match to {
        RowDrop::On(row) if row == key => {
            cell.add_css_class("mitsuami-drop-on");
            data.drop_cell = Some(cell.downgrade());
            return;
        }
        RowDrop::Before(row) => match data.cells.get(&(row, 0)) {
            Some(before) => (before.clone(), Edge::Top, 0.0),
            None if row == key => (cell.clone(), Edge::Top, 0.0),
            None => (cell.clone(), Edge::Bottom, 0.0),
        },
        RowDrop::On(folder) => (cell.clone(), Edge::Bottom, level(folder)),
        RowDrop::End => (
            cell.clone(),
            Edge::Bottom,
            -data.tree.get(&key).map_or(0.0, |r| {
                let indent = data.indent.unwrap_or_default();
                indent.at(r.depth) - indent.at(0)
            }),
        ),
    };
    if let Some(item) = cell.downcast_ref::<ItemCell>() {
        item.show_line(Some((edge, x)));
    }
    data.drop_cell = Some(cell.downgrade());
}

/// Makes a cell take rows dragged within its list: shown where they'd
/// land while over it, reported as `RowsMoved` when dropped. The drop is a
/// copy to GTK, so no file a row carries is ever moved: the app moves its
/// data.
fn row_drop_target(data: &Rc<RefCell<Data>>, events: &Events, id: NodeId, cell: &gtk::Widget) {
    let target = gtk::DropTarget::new(DraggedRows::static_type(), gdk::DragAction::COPY);
    target.set_preload(true);
    let land = {
        let (data, weak) = (Rc::downgrade(data), cell.downgrade());
        move |target: &gtk::DropTarget, x: f64, y: f64| -> Option<(RowKey, Vec<RowKey>, RowDrop<RowKey>)> {
            let (data, cell) = (data.upgrade()?, weak.upgrade()?);
            let dragged = target.value()?.get::<DraggedRows>().ok().filter(|d| d.list == id.raw())?;
            let data = data.borrow();
            let key = data.cells.iter().find(|(_, c)| **c == cell).map(|((key, _), _)| *key)?;
            let rows: Vec<RowKey> = dragged.rows.iter().map(|r| RowKey(*r)).collect();
            let proposed = proposal(&data, key, x, y, f64::from(cell.height()));
            let (rows, to) = landing(&data, &rows, Some(key), proposed)?;
            Some((key, rows, to))
        }
    };
    let land = Rc::new(land);
    {
        let (land, data, weak) = (land.clone(), Rc::downgrade(data), cell.downgrade());
        target.connect_motion(move |target, x, y| {
            let (Some(data), Some(cell)) = (data.upgrade(), weak.upgrade()) else { return gdk::DragAction::empty() };
            let landed = land(target, x, y);
            let mut data = data.borrow_mut();
            match landed {
                Some((key, _, to)) => {
                    show_drop(&mut data, Some((&cell, key, to)));
                    gdk::DragAction::COPY
                }
                None => {
                    show_drop(&mut data, None);
                    gdk::DragAction::empty()
                }
            }
        });
    }
    {
        let data = Rc::downgrade(data);
        target.connect_leave(move |_| {
            if let Some(data) = data.upgrade() {
                show_drop(&mut data.borrow_mut(), None);
            }
        });
    }
    {
        let (events, data) = (events.clone(), Rc::downgrade(data));
        target.connect_drop(move |target, _, x, y| {
            if let Some(data) = data.upgrade() {
                show_drop(&mut data.borrow_mut(), None);
            }
            match land(target, x, y) {
                Some((_, rows, to)) => {
                    events.emit(id, UiEvent::RowsMoved { rows, to });
                    true
                }
                None => false,
            }
        });
    }
    cell.add_controller(target);
}

/// Makes the list view take rows dropped on its empty part, at the end.
fn end_drop_target(data: &Rc<RefCell<Data>>, events: &Events, id: NodeId, view: &gtk::Widget) {
    let target = gtk::DropTarget::new(DraggedRows::static_type(), gdk::DragAction::COPY);
    target.set_preload(true);
    let land = {
        let data = Rc::downgrade(data);
        move |target: &gtk::DropTarget| -> Option<(Vec<RowKey>, RowDrop<RowKey>)> {
            let data = data.upgrade()?;
            let dragged = target.value()?.get::<DraggedRows>().ok().filter(|d| d.list == id.raw())?;
            let rows: Vec<RowKey> = dragged.rows.iter().map(|r| RowKey(*r)).collect();
            landing(&data.borrow(), &rows, None, RowDrop::End)
        }
    };
    let land = Rc::new(land);
    {
        let land = land.clone();
        target.connect_motion(move |target, _, _| match land(target) {
            Some(_) => gdk::DragAction::COPY,
            None => gdk::DragAction::empty(),
        });
    }
    let events = events.clone();
    target.connect_drop(move |target, _, _, _| match land(target) {
        Some((rows, to)) => {
            events.emit(id, UiEvent::RowsMoved { rows, to });
            true
        }
        None => false,
    });
    view.add_controller(target);
}

/// The files dragging `key` carries: the selected rows' when it's
/// selected, as Files drags a selection, else its own.
fn dragged(data: &Data, key: RowKey, model: Option<&gtk::SelectionModel>) -> Vec<PathBuf> {
    let selected = model.map(selected_keys).unwrap_or_default();
    let rows = if selected.contains(&key) { selected } else { vec![key] };
    rows.iter().filter_map(|row| data.file_of.get(row).cloned()).collect()
}

/// Makes a cell a drag source, as Files' rows are: dragging it carries its
/// row's file (or the selection's) as a `gdk::FileList`, which file
/// managers and other apps take, offered as a copy, and where rows move
/// within the list, the rows (`DraggedRows`). A row with neither doesn't
/// drag.
fn drag_source(data: &Rc<RefCell<Data>>, id: NodeId, cell: &gtk::Widget) {
    let source = gtk::DragSource::new();
    source.set_actions(gdk::DragAction::COPY);
    let (data, weak) = (Rc::downgrade(data), cell.downgrade());
    source.connect_prepare(move |_, _, _| {
        let (data, cell) = (data.upgrade()?, weak.upgrade()?);
        let data = data.borrow();
        let key = data.cells.iter().find(|(_, c)| **c == cell).map(|((key, _), _)| *key)?;
        let model = view_model(&cell);
        let files = dragged(&data, key, model.as_ref());
        let files: Vec<gio::File> = files.iter().map(gio::File::for_path).collect();
        let mut providers = Vec::new();
        if !files.is_empty() {
            providers.push(gdk::ContentProvider::for_value(&gdk::FileList::from_array(&files).to_value()));
        }
        if data.moves.is_some() {
            let rows = dragged_rows(key, model.as_ref()).iter().map(|r| r.0).collect();
            providers.push(gdk::ContentProvider::for_value(&DraggedRows { list: id.raw(), rows }.to_value()));
        }
        match providers.len() {
            0 => None,
            1 => providers.pop(),
            _ => Some(gdk::ContentProvider::new_union(&providers)),
        }
    });
    // The row itself under the pointer, as Files shows a dragged row.
    let weak = cell.downgrade();
    source.connect_drag_begin(move |source, _| {
        if let Some(cell) = weak.upgrade() {
            source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&cell))), 0, 0);
        }
    });
    cell.add_controller(source);
}

/// The selection model of the list or table view a cell is in.
fn view_model(cell: &gtk::Widget) -> Option<gtk::SelectionModel> {
    if let Some(view) = cell.ancestor(gtk::ListView::static_type()).and_downcast::<gtk::ListView>() {
        return view.model();
    }
    cell.ancestor(gtk::ColumnView::static_type()).and_downcast::<gtk::ColumnView>()?.model()
}

/// Puts a host in its cell, under a drop's line if it shows one.
fn attach(cell: &gtk::Widget, host: &gtk::Widget) {
    host.unparent();
    host.set_parent(cell);
    cell.set_height_request(-1);
    if let Some(line) = cell.downcast_ref::<ItemCell>().and_then(|c| c.imp().line.borrow().clone()) {
        line.insert_before(cell, None::<&gtk::Widget>);
    }
}

/// Unpads the column view's cell widget around a cell of a column that
/// fills its rows. Done as the cell is bound, not as it's parented: GTK
/// parents it while it's still making the cell widget, and a handler on
/// `notify::parent` that changed the widget's classes there left GTK with
/// no parent to read (a crash opening SourceGit's own repository).
fn fill_row(cell: &gtk::Widget) {
    if cell.has_css_class("mitsuami-fills")
        && let Some(parent) = cell.parent()
    {
        parent.add_css_class("mitsuami-fills-row");
    }
}

/// Takes the hosts out of a cell, which waits at the estimate's height.
fn detach(cell: &gtk::Widget) {
    while let Some(child) = host_of(cell) {
        child.unparent();
    }
}

/// A cell's host: its child that isn't a drop's line.
fn host_of(cell: &gtk::Widget) -> Option<gtk::Widget> {
    let mut child = cell.first_child();
    while let Some(c) = child {
        if !c.has_css_class("mitsuami-drop-line") {
            return Some(c);
        }
        child = c.next_sibling();
    }
    None
}

/// Which edge of a row a drop's line is on.
#[derive(Clone, Copy, PartialEq)]
enum Edge {
    Top,
    Bottom,
}

/// A list item's cell: its child, or, in a tree, its expander's.
fn cell_of(item: &gtk::ListItem) -> Option<gtk::Widget> {
    let child = item.child()?;
    match expander_of(item) {
        Some(expander) => expander.child(),
        None => Some(child),
    }
}

/// A tree's list item's expander, in the overlay that holds its spinner.
fn expander_of(item: &gtk::ListItem) -> Option<gtk::TreeExpander> {
    item.child().and_downcast::<gtk::Overlay>()?.child().and_downcast()
}

/// libadwaita's spinner, if the libadwaita the app runs with has one (1.6
/// and later). It's looked up at run time, so the backend builds against
/// libadwaita 1.4.
fn adw_spinner_type() -> Option<glib::Type> {
    static TYPE: OnceLock<Option<glib::Type>> = OnceLock::new();
    *TYPE.get_or_init(|| {
        // SAFETY: the symbol, if libadwaita has it, is its type's getter,
        // `GType adw_spinner_get_type(void)`.
        unsafe {
            let getter = libc::dlsym(libc::RTLD_DEFAULT, c"adw_spinner_get_type".as_ptr());
            (!getter.is_null()).then(|| {
                let getter: extern "C" fn() -> glib::ffi::GType = std::mem::transmute(getter);
                from_glib(getter())
            })
        }
    })
}

/// A tree row's spinner, hidden, showing at once when it's shown: Nautilus's
/// rows' `AdwSpinner`, which spins while it's mapped and draws in full from
/// its first frame. GTK's own spinner, since 4.20 an animated icon, fades
/// in over half a second as it starts spinning, after the core's own wait
/// (`Tree::loading`); without libadwaita 1.6 it's the one, and it spins
/// from the start, so only its visibility changes.
fn row_spinner() -> gtk::Widget {
    let spinner = match adw_spinner_type().and_then(|t| glib::Object::with_type(t).downcast::<gtk::Widget>().ok()) {
        Some(spinner) => spinner,
        None => {
            let spinner = gtk::Spinner::new();
            spinner.set_spinning(true);
            spinner.upcast()
        }
    };
    spinner.set_visible(false);
    spinner.set_can_target(false);
    spinner
}

/// A tree's row in an expander, with a spinner over the room of the
/// expander's arrow, which shows while the row's children are being read
/// (Nautilus shows a spinner as a folder loads). GTK's tree expander has
/// no spinner, so the arrow is hidden, and its room kept
/// (`indent-for-icon`), while it shows.
fn tree_item(cell: &gtk::Widget) -> gtk::Overlay {
    let expander = gtk::TreeExpander::new();
    expander.set_child(Some(cell));
    let spinner = row_spinner();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&expander));
    overlay.add_overlay(&spinner);
    // In the arrow's room: the indent before the cell, as the expander
    // lays it out without its arrow.
    overlay.connect_get_child_position(|overlay, spinner| {
        let expander = overlay.child().and_downcast::<gtk::TreeExpander>()?;
        let room = expander.child()?.prev_sibling()?.compute_bounds(overlay)?;
        let (_, width, _, _) = spinner.measure(gtk::Orientation::Horizontal, -1);
        let (_, height, _, _) = spinner.measure(gtk::Orientation::Vertical, width);
        let x = room.x() + (room.width() - width as f32) / 2.0;
        let y = room.y() + (room.height() - height as f32) / 2.0;
        Some(gdk::Rectangle::new(x.round() as i32, y.round() as i32, width, height))
    });
    overlay
}

/// Shows a tree item's spinner in place of its arrow while it loads.
fn show_loading(expander: &gtk::TreeExpander, loading: bool) {
    let Some(spinner) = expander.parent().and_downcast::<gtk::Overlay>().and_then(|o| o.last_child()) else { return };
    if spinner.get_visible() == loading {
        return;
    }
    expander.set_hide_expander(loading);
    expander.update_state(&[gtk::accessible::State::Busy(loading)]);
    spinner.set_visible(loading);
}

/// Hears the user open and close a tree's row (its expander, GTK's keys),
/// once per row object: GTK makes a new one when the row comes again.
fn hear_expanded(
    data: &Rc<RefCell<Data>>,
    d: &mut Data,
    events: &Events,
    id: NodeId,
    key: RowKey,
    row: &gtk::TreeListRow,
) {
    if d.expanders.get(&key).and_then(|(r, _)| r.upgrade()).as_ref() == Some(row) {
        return;
    }
    let (data, events) = (Rc::downgrade(data), events.clone());
    let handler = row.connect_expanded_notify(move |row| {
        let Some(data) = data.upgrade() else { return };
        if !data.try_borrow().is_ok_and(|d| d.expanding) {
            events.emit(id, UiEvent::RowExpanded(key, row.is_expanded()));
        }
    });
    if let Some((old, handler)) = d.expanders.insert(key, (row.downgrade(), handler))
        && let Some(old) = old.upgrade()
    {
        old.disconnect(handler);
    }
}

/// A factory binding one column's cells: a list's one, or a table's. `make`
/// makes an empty cell. A tree's cells are in expanders, bound to their
/// rows.
fn factory(
    data: &Rc<RefCell<Data>>,
    events: &Events,
    id: NodeId,
    column: usize,
    tree: bool,
    make: impl Fn() -> gtk::Widget + 'static,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    {
        let (data, events) = (data.clone(), events.clone());
        factory.connect_setup(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let cell = make();
            drag_source(&data, id, &cell);
            row_drop_target(&data, &events, id, &cell);
            if tree {
                // An expander gives its child its natural width, and a cell
                // asks for none: it takes the rest of the row, as a list's
                // does, so what's drawn on it (a drop's ring) shows.
                cell.set_hexpand(true);
                item.set_child(Some(&tree_item(&cell)));
            } else {
                item.set_child(Some(&cell));
            }
        });
    }
    {
        let (data, events) = (data.clone(), events.clone());
        factory.connect_bind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let (Some(key), Some(cell)) = (item.item().as_ref().and_then(key_of), cell_of(item)) else { return };
            fill_row(&cell);
            if let (Some(expander), Some(row)) = (expander_of(item), item.item().and_downcast::<gtk::TreeListRow>()) {
                let (any_opens, loading) = {
                    let d = data.borrow();
                    (d.any_opens, d.loading.as_ref().is_some_and(|l| l.contains(&key)))
                };
                expander.set_indent_for_icon(any_opens);
                expander.set_list_row(Some(&row));
                show_loading(&expander, loading);
                hear_expanded(&data, &mut data.borrow_mut(), &events, id, key, &row);
            }
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
            if let Some(expander) = expander_of(item) {
                expander.set_list_row(None);
            }
            let (Some(key), Some(cell)) = (item.item().as_ref().and_then(key_of), cell_of(item)) else { return };
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
        let factory = factory(&data, &events, id, 0, false, || ItemCell::new(0, None).upcast());
        let view = gtk::ListView::new(None::<gtk::NoSelection>, Some(factory));
        view.add_css_class("mitsuami-list");
        end_drop_target(&data, &events, id, view.upcast_ref());
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
        let store = gio::ListStore::new::<gtk::StringObject>();
        List::finish(scrolled, View::List(view), store, id, events, data)
    }

    /// A tree: a list view over a tree list model of the top rows' store,
    /// which gives each row that opens its own store of its children's
    /// keys. GTK doesn't open rows itself, and its rows are in expanders.
    pub(crate) fn new_tree(id: NodeId, events: Events) -> List {
        install_css();
        let data = new_data();
        let store = gio::ListStore::new::<gtk::StringObject>();
        let model = {
            let data = Rc::downgrade(&data);
            gtk::TreeListModel::new(store.clone(), false, false, move |item| {
                let (data, key) = (data.upgrade()?, key_of(item)?);
                let d = data.try_borrow().ok()?;
                d.tree.get(&key).filter(|r| r.expandable)?;
                d.stores.get(&key).map(|s| s.clone().upcast())
            })
        };
        let placed: Placed = {
            let (data, events) = (Rc::downgrade(&data), events.clone());
            Rc::new(move |cell: &gtk::Widget| {
                if let Some(data) = data.upgrade() {
                    report_indent(&data, &events, id, cell);
                }
            })
        };
        let factory = factory(&data, &events, id, 0, true, move || {
            let cell = ItemCell::new(0, None);
            *cell.imp().placed.borrow_mut() = Some(placed.clone());
            cell.upcast()
        });
        let view = gtk::ListView::new(None::<gtk::NoSelection>, Some(factory));
        view.add_css_class("mitsuami-list");
        end_drop_target(&data, &events, id, view.upcast_ref());
        // Activation is double-click or Enter, as in Files.
        view.set_single_click_activate(false);
        {
            let (data, events) = (data.clone(), events.clone());
            view.connect_activate(move |_, position| activated(&data, &events, id, position));
        }
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        {
            let (data, events) = (data.clone(), events.clone());
            scrolled.hadjustment().connect_page_size_notify(move |h| {
                let width = h.page_size();
                if width > 0.0 && data.borrow_mut().row_width.replace(width) != Some(width) {
                    events.emit(id, UiEvent::RowWidth(width as f32));
                }
            });
        }
        List::finish(scrolled, View::Tree(view, model), store, id, events, data)
    }

    /// A table: a column view, with the theme's own cells and header. Its
    /// columns come with `set_columns`. Its columns can be wider than it.
    pub(crate) fn new_table(id: NodeId, events: Events) -> List {
        install_css();
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
        let store = gio::ListStore::new::<gtk::StringObject>();
        let list = List::finish(scrolled, View::Table(view.clone()), store, id, events, data);
        // Pressing a sortable header changes the view's sorter: the app
        // sorts. The view owns its sorter, so it's held weakly here, or it
        // would never be freed.
        if let Some(sorter) = view.sorter() {
            let (view, events, muted) = (view.downgrade(), list.events.clone(), list.muted.clone());
            sorter.connect_changed(move |_, _| {
                if !muted.get()
                    && let Some(view) = view.upgrade()
                    && let Some(sort) = sort_of(&view)
                {
                    events.emit(id, UiEvent::Changed(EventValue::Sort(sort)));
                }
            });
        }
        list
    }

    fn finish(
        scrolled: gtk::ScrolledWindow,
        typed: View,
        store: gio::ListStore,
        id: NodeId,
        events: Events,
        data: Rc<RefCell<Data>>,
    ) -> List {
        let view: gtk::Widget = match &typed {
            View::List(v) | View::Tree(v, _) => v.clone().upcast(),
            View::Table(v) => v.clone().upcast(),
        };
        scrolled.set_child(Some(&view));
        // Its natural height is its rows' (`natural_height`).
        scrolled.set_propagate_natural_height(true);
        // Each adjustment's handler reads the other one weakly: a table's
        // would otherwise hold each other.
        let (h, v) = (scrolled.hadjustment(), scrolled.vadjustment());
        {
            let (events, h) = (events.clone(), h.downgrade());
            v.connect_value_changed(move |v| {
                let x = h.upgrade().map_or(0.0, |h| h.value());
                events.emit(id, UiEvent::Scrolled(Point::new(x as f32, v.value() as f32)))
            });
        }
        if matches!(typed, View::Table(_)) {
            let (events, v) = (events.clone(), v.downgrade());
            h.connect_value_changed(move |h| {
                let y = v.upgrade().map_or(0.0, |v| v.value());
                events.emit(id, UiEvent::Scrolled(Point::new(h.value() as f32, y as f32)))
            });
        }
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
            let fills = column.fills_row;
            let make = move || {
                let cell = ItemCell::new(index, Some(report.clone()));
                // Empty until its host comes, at the estimate's height.
                cell.set_height_request(data.borrow().estimate());
                // Its parent, the column view's cell widget that the theme
                // pads, is unpadded as it's bound (`fill_row`).
                if fills {
                    cell.add_css_class("mitsuami-fills");
                }
                cell.upcast()
            };
            let factory = factory(&self.data, &self.events, self.id, index, false, make);
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
            View::List(_) | View::Tree(..) => None,
        }
    }

    /// What the view shows, which its selection model wraps: the store, or
    /// a tree's tree list model of it.
    fn items(&self) -> gio::ListModel {
        match &self.typed {
            View::Tree(_, model) => model.clone().upcast(),
            View::List(_) | View::Table(_) => self.store.clone().upcast(),
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
        self.queue_remeasure();
    }

    /// A tree's rows that show, each with its place in the tree. The top
    /// rows' store and each opening row's store take their children's keys
    /// by the fewest changes; then, from the top, each row is opened or
    /// closed as sent, which shows or drops its children's rows, so the
    /// tree list model shows the rows in their order. A closed row's store
    /// is emptied: its children come with the rows that open it.
    pub(crate) fn set_tree_rows(&self, rows: Vec<TreeRow>) {
        let View::Tree(_, model) = &self.typed else { return };
        let selected = self.selected();
        let mut children: HashMap<Option<RowKey>, Vec<RowKey>> = HashMap::new();
        for row in &rows {
            children.entry(row.parent).or_default().push(row.row);
        }
        // Expanders to indent again, set once the data is let go: GTK asks
        // `create_func` whether an unbound row opens, and keeps the answer.
        let mut reindent: Vec<gtk::TreeExpander> = Vec::new();
        let stores: Vec<(gio::ListStore, Vec<RowKey>)> = {
            let mut data = self.data.borrow_mut();
            data.index = rows.iter().enumerate().map(|(i, r)| (r.row, i)).collect();
            data.rows = rows.iter().map(|r| r.row).collect();
            data.tree = rows.iter().map(|r| (r.row, *r)).collect();
            let any_opens = rows.iter().any(|r| r.expandable);
            if data.any_opens != any_opens {
                data.any_opens = any_opens;
                reindent = data.cells.values().filter_map(|c| c.parent().and_downcast::<gtk::TreeExpander>()).collect();
            }
            let Data { stores, tree, expanders, .. } = &mut *data;
            stores.retain(|key, _| tree.get(key).is_some_and(|r| r.expandable));
            expanders.retain(|key, _| tree.contains_key(key));
            rows.iter()
                .filter(|r| r.expandable)
                .map(|r| {
                    let store = stores.entry(r.row).or_insert_with(gio::ListStore::new::<gtk::StringObject>).clone();
                    (store, children.remove(&Some(r.row)).unwrap_or_default())
                })
                .collect()
        };
        let any_opens = self.data.borrow().any_opens;
        for expander in reindent {
            expander.set_indent_for_icon(any_opens);
        }
        let focused = self.focused();
        self.muted.set(true);
        self.data.borrow_mut().expanding = true;
        splice_keys(&self.store, &children.remove(&None).unwrap_or_default());
        for (store, keys) in &stores {
            splice_keys(store, keys);
        }
        for (position, row) in rows.iter().enumerate() {
            let Some(native) = model.row(position as u32) else { break };
            if key_of(native.upcast_ref()) != Some(row.row) {
                break;
            }
            if native.is_expanded() != row.expanded {
                native.set_expanded(row.expanded);
            }
        }
        self.data.borrow_mut().expanding = false;
        self.muted.set(false);
        if focused && !self.focused() {
            self.view.grab_focus();
        }
        let kept: Vec<RowKey> = {
            let data = self.data.borrow();
            selected.iter().copied().filter(|k| data.index.contains_key(k)).collect()
        };
        self.set_selected(&kept);
        if kept != selected {
            self.report_selection();
        }
        self.queue_remeasure();
    }

    /// The rows whose children are being read: their spinners show, in the
    /// rows bound now and in those bound later.
    pub(crate) fn set_tree_loading(&self, rows: &[RowKey]) {
        let loading: HashSet<RowKey> = rows.iter().copied().collect();
        let cells: Vec<(RowKey, gtk::Widget)> = {
            let mut data = self.data.borrow_mut();
            data.loading = Some(loading.clone());
            data.cells.iter().map(|((key, _), cell)| (*key, cell.clone())).collect()
        };
        for (key, cell) in cells {
            if let Some(expander) = cell.parent().and_downcast::<gtk::TreeExpander>() {
                show_loading(&expander, loading.contains(&key));
            }
        }
    }

    /// The rows whose spinners show: of those sent, the ones bound with
    /// their spinner showing, and those not bound, which show it when
    /// they are.
    pub(crate) fn tree_loading(&self) -> Option<Vec<RowKey>> {
        let data = self.data.borrow();
        let shows = |key: &RowKey| match data.cells.get(&(*key, 0)) {
            Some(cell) => cell
                .parent()
                .and_then(|expander| expander.parent())
                .and_then(|overlay| overlay.last_child())
                .is_some_and(|spinner| spinner.get_visible()),
            None => true,
        };
        let loading = data.loading.as_ref()?;
        Some(data.rows.iter().filter(|key| loading.contains(key) && shows(key)).copied().collect())
    }

    /// Opens or closes a tree's row as assistive technology does, reporting
    /// it. Closing it drops its children's rows; opening it shows them once
    /// the core sends them. False for a row that doesn't open.
    pub(crate) fn expand(&self, key: RowKey, open: bool) -> bool {
        let View::Tree(_, model) = &self.typed else { return false };
        let Some(position) = self.data.borrow().index.get(&key).copied() else { return false };
        let Some(native) = model.row(position as u32).filter(|r| r.is_expandable()) else { return false };
        if native.is_expanded() != open {
            self.data.borrow_mut().expanding = true;
            native.set_expanded(open);
            self.data.borrow_mut().expanding = false;
            self.events.emit(self.id, UiEvent::RowExpanded(key, open));
        }
        true
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
        let items = self.items();
        let model: gtk::SelectionModel = match mode {
            SelectionMode::None => gtk::NoSelection::new(Some(items)).upcast(),
            SelectionMode::Single => {
                let single = gtk::SingleSelection::new(Some(items));
                single.set_autoselect(false);
                single.set_can_unselect(true);
                single.set_selected(gtk::INVALID_LIST_POSITION);
                single.upcast()
            }
            SelectionMode::Multiple => gtk::MultiSelection::new(Some(items)).upcast(),
        };
        let (events, muted, id) = (self.events.clone(), self.muted.clone(), self.id);
        model.connect_selection_changed(move |model, _, _| {
            if !muted.get() {
                let keys = selected_keys(model);
                events.emit(id, UiEvent::Changed(EventValue::Rows(keys)));
            }
        });
        // The swap takes the focused row out of the view, but the window
        // keeps it as its focus, so keys go nowhere. Focus stays in the
        // list, as in AppKit's tables.
        let focused = self.focused();
        match &self.typed {
            View::List(view) | View::Tree(view, _) => view.set_model(Some(&model)),
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

    /// A list shorter than its scroll bar's minimum has no room to show
    /// one, and GTK would make it as high as the scroll bar, over what's
    /// under it (a one-row list in a sidebar). It scrolls without a scroll
    /// bar then (`External`), and shows one again once it's high enough.
    pub(crate) fn fit_scroll_bar(&self, height: f32) {
        let bar = self.scrolled.vscrollbar().measure(gtk::Orientation::Vertical, -1).0;
        let wanted = if (height.round() as i32) < bar { gtk::PolicyType::External } else { gtk::PolicyType::Automatic };
        let (horizontal, vertical) = self.scrolled.policy();
        if vertical != wanted {
            self.scrolled.set_policy(horizontal, wanted);
        }
    }

    pub(crate) fn set_style(&self, style: ListStyle) {
        self.data.borrow_mut().style = Some(style);
        self.scrolled.set_has_frame(style.framed());
        self.queue_remeasure();
    }

    /// How high the list is with all its rows: its scrolled window's
    /// natural height, which is its view's, and a frame's border. GTK's
    /// list view measures only the rows it has bound; the others count as
    /// high as those on average. A column view adds its header. As last
    /// measured once idle: while the core lays the list out, the hosts of
    /// rows it just mounted have no height yet.
    pub(crate) fn natural_height(&self) -> f32 {
        let d = self.data.borrow();
        match d.remeasured {
            true => d.natural,
            false => natural_height(&self.scrolled),
        }
    }

    /// Measures the list again once the main loop is idle, when GTK has
    /// bound and measured its rows, and reports `Remeasure` if its natural
    /// height changed.
    fn queue_remeasure(&self) {
        let mut d = self.data.borrow_mut();
        if std::mem::replace(&mut d.remeasure_queued, true) {
            return;
        }
        let (data, scrolled) = (Rc::downgrade(&self.data), self.scrolled.downgrade());
        let (events, id) = (self.events.clone(), self.id);
        glib::idle_add_local_once(move || {
            let (Some(data), Some(scrolled)) = (data.upgrade(), scrolled.upgrade()) else { return };
            let natural = natural_height(&scrolled);
            let mut d = data.borrow_mut();
            d.remeasure_queued = false;
            d.remeasured = true;
            if std::mem::replace(&mut d.natural, natural) != natural {
                events.emit(id, UiEvent::Remeasure);
            }
        });
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
            model.set_selection(&selection, &gtk::Bitset::new_range(0, self.items().n_items()));
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
            View::List(view) | View::Tree(view, _) => view.model(),
            View::Table(view) => view.model(),
        }
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        match self.model() {
            Some(model) => selected_keys(&model),
            None => Vec::new(),
        }
    }

    pub(crate) fn set_estimate(&self, height: f32) {
        self.data.borrow_mut().estimate = Some(height.round() as i32);
        self.queue_remeasure();
    }

    pub(crate) fn set_row_files(&self, files: Vec<(RowKey, PathBuf)>) {
        let mut data = self.data.borrow_mut();
        data.file_of = files.iter().cloned().collect();
        data.files = Some(files);
    }

    /// Rows move within the list: its cells drag them, and take them.
    pub(crate) fn set_moves(&self, moves: RowMoves) {
        self.data.borrow_mut().moves = Some(moves);
    }

    pub(crate) fn moves(&self) -> Option<RowMoves> {
        self.data.borrow().moves
    }

    /// A drag of `rows` dropped where `to` proposes, through the targeting
    /// the drop targets use: for tests, as GTK can't start a drag.
    pub(crate) fn move_rows(&self, rows: &[RowKey], to: RowDrop<RowKey>) {
        // The pointer is over the row dropped onto or before, as a drag's.
        let over = match to {
            RowDrop::On(row) | RowDrop::Before(row) => Some(row),
            RowDrop::End => None,
        };
        let landed = landing(&self.data.borrow(), rows, over, to);
        if let Some((rows, to)) = landed {
            self.events.emit(self.id, UiEvent::RowsMoved { rows, to });
        }
    }

    pub(crate) fn row_files(&self) -> Option<Vec<(RowKey, PathBuf)>> {
        self.data.borrow().files.clone()
    }

    /// What dragging `key` carries, as its cells' drag source makes it.
    pub(crate) fn dragged_files(&self, key: RowKey) -> Option<Vec<PathBuf>> {
        let files = dragged(&self.data.borrow(), key, self.model().as_ref());
        (!files.is_empty()).then_some(files)
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

    /// Without the app's estimate, the first row measured sets it. A row
    /// scrolled to before it had its size is scrolled to again. A table's
    /// cells that fill their rows follow the others.
    pub(crate) fn row_measured(&self, key: RowKey, height: f32) {
        let again = {
            let mut data = self.data.borrow_mut();
            if data.estimate.is_none() && data.learned.is_none() && height > 0.0 {
                data.learned = Some(height.round() as i32);
            }
            queue_heights(&self.data, &mut data, &self.events, self.id);
            data.scrolling_to.take_if(|k| *k == key).is_some()
        };
        self.queue_remeasure();
        if again {
            self.scroll_to_row(key);
        }
    }

    /// The list view that scrolls to items: the list's, or the one inside
    /// a table's column view (`ColumnView::scroll_to` needs GTK 4.12).
    fn list_view(&self) -> Option<gtk::Widget> {
        match &self.typed {
            View::List(view) | View::Tree(view, _) => Some(view.clone().upcast()),
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
        let index = {
            let mut data = self.data.borrow_mut();
            data.scrolling_to = (!data.hosts.contains_key(&(key, 0))).then_some(key);
            data.index.get(&key).copied()
        };
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

    /// A tree's rows as its tree list model shows them: each with its
    /// parent, depth, and whether it opens and is open.
    pub(crate) fn tree_rows(&self) -> Vec<TreeRow> {
        let View::Tree(_, model) = &self.typed else { return Vec::new() };
        (0..model.n_items())
            .filter_map(|i| {
                let row = model.row(i)?;
                Some(TreeRow {
                    row: key_of(row.upcast_ref())?,
                    parent: row.parent().and_then(|p| key_of(p.upcast_ref())),
                    depth: row.depth(),
                    expandable: row.is_expandable(),
                    expanded: row.is_expanded(),
                })
            })
            .collect()
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
        let host = host_of(&key)?;
        let size = frames.get(host).copied()?.size;
        let y = match self.placed(host) {
            Some(y) => y,
            None => {
                let hosted =
                    data.hosts.keys().filter(|(_, c)| *c == 0).filter_map(|(k, _)| Some((*data.index.get(k)?, *k)));
                beside_placed(&data, index, hosted.collect(), |k| host_of(k).and_then(|h| self.placed(h)), height)?
            }
        };
        // A tree's row is in by its indent.
        let x = match (data.indent, data.tree.get(&key)) {
            (Some(indent), Some(row)) => indent.at(row.depth),
            _ => 0.0,
        };
        Some(Rect { origin: Point::new(x, y), size })
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
        let placed = |k: &RowKey| hosts(k).find_map(|h| self.placed(h));
        let x = data.hosts.iter().filter(|((_, c), _)| *c == column).find_map(|(_, (_, h))| self.placed_at(h))?.x;
        let index = *data.index.get(&key)?;
        let hosted = data.hosts.keys().filter_map(|(k, _)| Some((*data.index.get(k)?, *k)));
        let y = beside_placed(&data, index, hosted.collect(), placed, height)?;
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

/// Where an unplaced row at `index` is: beside the hosted row placed
/// nearest it (above, when two are as near), the rows between at their
/// heights. Only hosted rows can be placed, and rows between without a host
/// are at the estimate, so this walks the hosts, not every row.
fn beside_placed(
    data: &Data,
    index: usize,
    hosted: BTreeMap<usize, RowKey>,
    placed: impl Fn(&RowKey) -> Option<f32>,
    height: impl Fn(&RowKey) -> f32,
) -> Option<f32> {
    let between = |from: usize, to: usize| {
        let (sum, count) = hosted.range(from..to).fold((0.0, 0), |(sum, count), (_, k)| (sum + height(k), count + 1));
        sum + (to - from - count) as f32 * data.estimate() as f32
    };
    let mut above = hosted.range(..index).rev().peekable();
    let mut below = hosted.range(index + 1..).peekable();
    loop {
        let up = match (above.peek(), below.peek()) {
            (None, None) => return None,
            (Some((a, _)), Some((b, _))) => index - **a <= **b - index,
            (up, _) => up.is_some(),
        };
        if up {
            let (&i, k) = above.next()?;
            if let Some(y) = placed(k) {
                return Some(y + between(i, index));
            }
        } else {
            let (&i, k) = below.next()?;
            if let Some(y) = placed(k) {
                return Some(y - between(index, i));
            }
        }
    }
}

/// The selected rows, by the model's own items: a tree's rows move as GTK
/// opens and closes folders, before the core sends them.
fn selected_keys(model: &gtk::SelectionModel) -> Vec<RowKey> {
    let selection = model.selection();
    (0..selection.size()).filter_map(|i| model.item(selection.nth(i as u32)).as_ref().and_then(key_of)).collect()
}

/// Reports how far a tree indents its rows' content, from a cell GTK just
/// allocated in its expander: where a top row's starts, and what each level
/// adds, from a deeper row's (until one shows, as much as the start, since
/// GTK indents each level by an expander's width).
fn report_indent(data: &Rc<RefCell<Data>>, events: &Events, id: NodeId, cell: &gtk::Widget) {
    let Some(expander) = cell.parent().and_downcast::<gtk::TreeExpander>() else { return };
    let (Some(row), Some(item)) = (expander.list_row(), expander.parent()) else { return };
    let Some(x) = cell.compute_point(&item, &graphene::Point::new(0.0, 0.0)).map(|p| p.x()) else { return };
    let Ok(mut d) = data.try_borrow_mut() else { return };
    let indent = match (row.depth(), d.indent) {
        (0, old) => TreeIndent { start: x, per_level: old.map_or(x, |o| o.per_level) },
        (_, None) => return,
        (depth, Some(old)) => TreeIndent { per_level: (x - old.start) / depth as f32, ..old },
    };
    if d.indent != Some(indent) {
        d.indent = Some(indent);
        drop(d);
        events.emit(id, UiEvent::TreeIndent(indent));
    }
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
            // Shown again, its height is reported again.
            d.bare_heights.remove(&row);
            events.emit(id, UiEvent::RowHidden(row));
        }
        for row in shown {
            events.emit(id, UiEvent::RowShown(row));
        }
        d.shown = now;
    });
}

/// Reports, once the main loop is idle, the shown rows' heights without
/// their cells that fill them, in a table with such a column: as high as
/// GTK makes their other cells, the theme's padding in. Those cells are
/// laid out at least as high, so they fill the row GTK makes.
fn queue_heights(data: &Rc<RefCell<Data>>, d: &mut Data, events: &Events, id: NodeId) {
    if d.heights_queued || !d.columns.iter().any(|c| c.fills_row) {
        return;
    }
    d.heights_queued = true;
    let (data, events) = (Rc::downgrade(data), events.clone());
    glib::idle_add_local_once(move || {
        let Some(data) = data.upgrade() else { return };
        let mut d = data.borrow_mut();
        d.heights_queued = false;
        let mut bare: HashMap<RowKey, f32> = HashMap::new();
        for ((key, column), cell) in &d.cells {
            if !d.shown.contains(key) || d.columns.get(*column).is_none_or(|c| c.fills_row) {
                continue;
            }
            // The column view's cell widget, with the theme's padding.
            let around = cell.parent().unwrap_or_else(|| cell.clone());
            let width = Some(around.width()).filter(|w| *w > 0).unwrap_or(-1);
            let height = around.measure(gtk::Orientation::Vertical, width).1 as f32;
            let row = bare.entry(*key).or_default();
            *row = row.max(height);
        }
        let Data { bare_heights, shown, .. } = &mut *d;
        bare_heights.retain(|key, _| shown.contains(key));
        for (key, height) in bare {
            if height > 0.0 && bare_heights.insert(key, height) != Some(height) {
                events.emit(id, UiEvent::RowHeight(key, height));
            }
        }
    });
}

/// A list's scrolled window's natural height at its width, with all its
/// rows: GTK's list view measures only the rows it has bound, which are
/// those its allocation shows. Its view's and its own border's, not the
/// scrolled window's: that is no less than its scroll bar's natural
/// height (64 px with Adwaita), so two rows and three were as high, and a
/// list that short has no bar (`fit_scroll_bar`).
fn natural_height(scrolled: &gtk::ScrolledWindow) -> f32 {
    let width = Some(scrolled.width()).filter(|w| *w > 0).unwrap_or(-1);
    let Some(content) = scrolled.child() else { return scrolled.measure(gtk::Orientation::Vertical, width).1 as f32 };
    #[allow(deprecated)] // The only way to read a widget's CSS padding.
    let (padding, border) = { (scrolled.style_context().padding(), scrolled.style_context().border()) };
    let around = (padding.top() + padding.bottom() + border.top() + border.bottom()) as f32;
    let measured = content.measure(gtk::Orientation::Vertical, width).1 as f32 + around;
    // The rows GTK hasn't bound, as high as those it has on average, as
    // GTK estimates them when it scrolls.
    let Some(view) = rows_view(&content) else { return measured };
    let items = view.model().map_or(0, |model| model.n_items()) as f32;
    let mut bound = 0.0;
    let mut child = view.first_child();
    while let Some(c) = child {
        bound += 1.0;
        child = c.next_sibling();
    }
    if bound == 0.0 || bound >= items {
        return measured;
    }
    let rows = view.measure(gtk::Orientation::Vertical, width).1 as f32;
    measured + rows / bound * (items - bound)
}

/// The list view that holds the rows: the view itself, or the one inside
/// a table's column view.
fn rows_view(view: &gtk::Widget) -> Option<gtk::ListView> {
    if let Some(list) = view.downcast_ref::<gtk::ListView>() {
        return Some(list.clone());
    }
    let mut child = view.first_child();
    while let Some(c) = child {
        if let Ok(list) = c.clone().downcast::<gtk::ListView>() {
            return Some(list);
        }
        child = c.next_sibling();
    }
    None
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

    /// A list's row, or a table's cell: it holds its host at the host's
    /// size, centred in the row's height, and asks for no width, so the
    /// list's or the column's width is its own (the host's width follows
    /// it, as the core lays it out at the width reported). A host asking
    /// for its width would hold the list at the width it had.
    #[derive(Default)]
    pub(crate) struct ItemCell {
        pub(super) column: Cell<usize>,
        pub(super) report: RefCell<Option<WidthReport>>,
        /// A tree's cell: hears it allocated.
        pub(super) placed: RefCell<Option<Placed>>,
        /// A drop's line, and the edge it's on while a drag shows it, from
        /// how far across the cell (left of it, for a level out).
        pub(super) line: RefCell<Option<gtk::Widget>>,
        pub(super) line_at: Cell<Option<(Edge, f32)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ItemCell {
        const NAME: &'static str = "MitsuamiItemCell";
        type Type = super::ItemCell;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ItemCell {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for ItemCell {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                return (0, 0, -1, -1);
            }
            let height = host_of(self.obj().upcast_ref()).map_or(0, |c| c.measure(gtk::Orientation::Vertical, -1).0);
            (height, height, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            // A drop's line, over the row's content at its edge.
            if let (Some(line), Some((edge, x))) = (self.line.borrow().as_ref(), self.line_at.get()) {
                let y = if edge == Edge::Top { 0 } else { height - 2 };
                let transform = gsk::Transform::new().translate(&graphene::Point::new(x, y as f32));
                line.allocate((width as f32 - x).round() as i32, 2, -1, Some(transform));
            }
            if let Some(child) = host_of(self.obj().upcast_ref()) {
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
            let placed = self.placed.borrow().clone();
            if let Some(placed) = placed {
                placed(self.obj().upcast_ref());
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct ItemCell(ObjectSubclass<imp::ItemCell>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ItemCell {
    /// A cell in `column`, which reports its width to `report` (a table's).
    fn new(column: usize, report: Option<WidthReport>) -> ItemCell {
        let cell: ItemCell = glib::Object::new();
        // The theme rings every widget a drag is over (`:drop(active)`);
        // a cell shows where a drop lands itself, as libadwaita's places
        // sidebar does, so it has no ring of the theme's.
        cell.add_css_class("mitsuami-cell");
        cell.imp().column.set(column);
        *cell.imp().report.borrow_mut() = report;
        cell
    }

    /// Shows a drop's line at an edge, or takes it away. It's a child made
    /// last, so it's drawn over the row's content: the theme's inset shadow
    /// on the cell was drawn under it, and the content hid it.
    fn show_line(&self, edge: Option<(Edge, f32)>) {
        self.imp().line_at.set(edge);
        let line = self
            .imp()
            .line
            .borrow_mut()
            .get_or_insert_with(|| {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                line.add_css_class("mitsuami-drop-line");
                line.set_can_target(false);
                line.upcast()
            })
            .clone();
        match edge {
            Some(_) => {
                line.insert_before(self, None::<&gtk::Widget>);
                line.set_visible(true);
            }
            None => line.set_visible(false),
        }
        self.queue_allocate();
    }
}
