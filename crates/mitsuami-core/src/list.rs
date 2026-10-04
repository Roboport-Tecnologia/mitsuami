//! `List`: the platform's list control, with rows built by the core.
//!
//! The native control (NSTableView, ListView, gtk::ListView, QML ListView)
//! virtualises: it scrolls, decides which rows to realise, recycles them,
//! and draws and handles the selection. When it realises a row it reports
//! `RowShown`, and the list mounts that row, in a host `Container` carrying
//! its [`RowKey`]; when it lets a row go (`RowHidden`), the list disposes
//! it. Rows are keyed like [`For`](crate::For)'s.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_reactive::{IntoValue, Owner, Signal, Value, effect, untrack};

use crate::command::{EventValue, UiEvent};
use crate::element::{Element, ElementBuilder};
use crate::services::Shortcut;
use crate::tweak::Tweak;
use crate::ui::{Ui, WeakUi};
use crate::units::Length;
use crate::view::{AnyView, View};
use crate::widget::{ListStyle, NodeId, Prop, RowKey, SelectionMode, WidgetKind};

pub(crate) type KeyFn<T, K> = Rc<dyn Fn(&T) -> K>;
/// The file a row's item stands for, which dragging the row carries.
pub(crate) type FileFn<T> = Rc<dyn Fn(&T) -> Option<PathBuf>>;
type RowOf<K> = Rc<dyn Fn(&K) -> Option<RowKey>>;

/// How a `List` renders a row.
pub struct RowRender<T>(Rc<dyn Fn(T) -> AnyView>);

/// A virtualised list of rows, shown by the platform's own list control.
///
/// ```ignore
/// List::new(move || contacts.get(), |c| c.id, |c| ContactRow(c))
///     .selected(selected)            // Signal<Vec<K>>: the selected keys
///     .on_activate(|id| open(id))    // double-click or Enter
///     .grow(1.0)
/// ```
///
/// Only the rows the platform shows (in view, or about to be) are mounted;
/// the others exist only as their keys. Rows can hold any view. Each row is
/// laid out at the width the list gives its rows, and is as high as its
/// content.
///
/// Rows are read by screen readers (and found by tests) as list items named
/// by their text.
///
/// The type's defaults make `List` alone name the widget, for
/// [`Tweak<List>`](Tweak).
pub struct List<T: 'static = (), K: 'static = (), R = RowRender<T>> {
    element: Element,
    each: Value<Vec<T>>,
    key: KeyFn<T, K>,
    render: R,
    mode: Option<Value<SelectionMode>>,
    style: Option<ListStyle>,
    selected: Option<Signal<Vec<K>>>,
    on_activate: Option<Rc<dyn Fn(K)>>,
    estimate: Option<f32>,
    handle: Option<ListHandle<K>>,
    files: Option<FileFn<T>>,
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> List<T, K> {
    pub fn new<V: View>(
        each: impl IntoValue<Vec<T>>,
        key: impl Fn(&T) -> K + 'static,
        render: impl Fn(T) -> V + 'static,
    ) -> List<T, K> {
        List::keyed(each.into_value(), Rc::new(key), RowRender(Rc::new(move |item| AnyView::new(render(item)))))
    }
}

impl<T: 'static, K: 'static, R> List<T, K, R> {
    fn keyed(each: Value<Vec<T>>, key: KeyFn<T, K>, render: R) -> List<T, K, R> {
        List::with_element(Element::new(WidgetKind::List), each, key, render)
    }

    fn with_element(element: Element, each: Value<Vec<T>>, key: KeyFn<T, K>, render: R) -> List<T, K, R> {
        List {
            element,
            each,
            key,
            render,
            mode: None,
            style: None,
            selected: None,
            on_activate: None,
            estimate: None,
            handle: None,
            files: None,
        }
    }

    /// Rows can be dragged out of the app, carrying the files their items
    /// stand for (`None`: that row doesn't drag), to the file manager,
    /// another app or a folder, as a file manager's rows are: the
    /// platform's own drag, which drags the selected rows when a selected
    /// one is dragged. The drag offers to copy, so the files stay where
    /// they are. Dragging isn't reachable from the keyboard or assistive
    /// technology, so offer another way too (Copy, Share, Export).
    ///
    /// ```ignore
    /// List::new(..).drag_files(|entry: &Entry| Some(entry.path.clone()))
    /// ```
    pub fn drag_files(mut self, file: impl Fn(&T) -> Option<PathBuf> + 'static) -> Self {
        self.files = Some(Rc::new(file));
        self
    }

    /// Binds the selection, as the selected rows' keys, both ways. Lists
    /// with a selection bound select one row at a time unless
    /// [`selection_mode`](List::selection_mode) says otherwise.
    pub fn selected(mut self, selected: Signal<Vec<K>>) -> Self {
        self.selected = Some(selected);
        self
    }

    /// How many rows can be selected. It can change while the list shows:
    /// the selection keeps what the new mode can hold (none, or the row the
    /// platform keeps), and the rows let go are reported as deselected.
    pub fn selection_mode(mut self, mode: impl IntoValue<SelectionMode>) -> Self {
        self.mode = Some(mode.into_value());
        self
    }

    /// How the list sits in its surroundings: edge to edge (the default) or
    /// framed. Pick per platform with `platform!`:
    ///
    /// ```ignore
    /// List::new(..).list_style(platform! { kde => ListStyle::Framed, _ => ListStyle::Plain })
    /// ```
    pub fn list_style(mut self, style: ListStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Called with a row's key when it's activated: double-clicked, or Enter
    /// pressed on it.
    pub fn on_activate(mut self, handler: impl Fn(K) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Runs `handler` when `key` is pressed while the list, or a control
    /// inside it, has keyboard focus, and the focused control doesn't use
    /// the key itself (the arrows, Home and End, typing to
    /// select, and what else the platform's list takes): Space for a preview, Delete for Move to Trash.
    /// A key goes to the nearest node around the focused control that
    /// takes it. Nothing shows keys taken this way, so give the command a
    /// menu item or a button too, and pick the keys the platform's own apps
    /// use (`platform!`).
    ///
    /// ```ignore
    /// List::new(..).on_key(' ', preview)
    /// ```
    pub fn on_key(mut self, key: impl Into<Shortcut>, handler: impl Fn() + 'static) -> Self {
        self.element.on_key(key.into(), handler);
        self
    }

    /// How high rows are likely to be, for platforms that size rows before
    /// they're shown (AppKit). By default, the rows shown so far tell.
    pub fn estimated_row_height(mut self, height: impl Into<Length>) -> Self {
        self.estimate = match height.into() {
            Length::Px(px) => Some(px),
            _ => None,
        };
        self
    }

    /// Connects a [`ListHandle`], to scroll the list from code.
    pub fn handle(mut self, handle: ListHandle<K>) -> Self {
        self.handle = Some(handle);
        self
    }

    /// Raw platform settings: see [`Tweak`]. They run on the list view,
    /// not the scroll view around it. Lists have no semantic options past
    /// their rows, selection and style: what the platforms offer
    /// (alternating rows on AppKit, separators on GTK, key navigation on Qt
    /// and WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<List>) -> Self {
        tweak.apply(&mut self.element);
        self
    }
}

impl<T: 'static, K: 'static, R> ElementBuilder for List<T, K, R> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

/// Scrolls a [`List`] from code: create one, pass it to
/// [`List::handle`], and call [`scroll_to`](ListHandle::scroll_to).
pub struct ListHandle<K: 'static>(Rc<RefCell<HandleState<K>>>);

struct HandleState<K> {
    connection: Option<Connection<K>>,
    /// A key asked for before the list had it: an app's effect can run
    /// before the list's own takes the items. Scrolled to when the rows
    /// next change, if it's among them.
    pending: Option<K>,
}

struct Connection<K> {
    ui: WeakUi,
    id: NodeId,
    row_of: RowOf<K>,
}

impl<K: 'static> ListHandle<K> {
    pub fn new() -> ListHandle<K> {
        ListHandle(Rc::new(RefCell::new(HandleState { connection: None, pending: None })))
    }

    /// The list's node, once it's built.
    pub fn id(&self) -> Option<NodeId> {
        self.0.borrow().connection.as_ref().map(|c| c.id)
    }

    /// Scrolls just enough to show the row with this key, mounted or not.
    /// A key whose item the list doesn't have yet, arriving with the same
    /// change, is scrolled to once the list has it.
    pub fn scroll_to(&self, key: &K)
    where
        K: Clone,
    {
        let mut state = self.0.borrow_mut();
        let found = state
            .connection
            .as_ref()
            .and_then(|Connection { ui, id, row_of }| Some((ui.upgrade()?, *id, row_of(key)?)));
        match found {
            Some((ui, id, row)) => {
                state.pending = None;
                drop(state);
                ui.scroll_to_row(id, row);
            }
            None => state.pending = Some(key.clone()),
        }
    }
}

impl<K: 'static> Clone for ListHandle<K> {
    fn clone(&self) -> Self {
        ListHandle(self.0.clone())
    }
}

impl<K: 'static> Default for ListHandle<K> {
    fn default() -> Self {
        ListHandle::new()
    }
}

/// The list's items and mounted rows, shared by its effects and handlers.
struct Rows<T, K> {
    /// The rows, in order.
    order: Vec<RowKey>,
    items: HashMap<RowKey, T>,
    position: HashMap<RowKey, usize>,
    row_keys: HashMap<K, RowKey>,
    keys: HashMap<RowKey, K>,
    next_key: u64,
    /// Mounted rows: their hosts (a list's row host, a table's cell hosts,
    /// in column order) and scope.
    mounted: HashMap<RowKey, (Vec<NodeId>, Owner)>,
}

/// Builds a mounted row's hosts, in the scope of the row.
pub(crate) type Mount<T> = Rc<dyn Fn(&Ui, RowKey, T) -> Vec<NodeId>>;

/// What a `List` and a `Table` share: their data, selection, activation
/// and handle, and the rows the platform shows.
pub(crate) struct RowParts<T: 'static, K: 'static> {
    pub each: Value<Vec<T>>,
    pub key: KeyFn<T, K>,
    pub mount: Mount<T>,
    pub mode: Option<Value<SelectionMode>>,
    pub style: Option<ListStyle>,
    pub selected: Option<Signal<Vec<K>>>,
    pub on_activate: Option<Rc<dyn Fn(K)>>,
    pub estimate: Option<f32>,
    pub handle: Option<ListHandle<K>>,
    pub files: Option<FileFn<T>>,
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> View for List<T, K> {
    fn build(self, ui: &Ui) -> NodeId {
        let List { element, each, key, render, mode, style, selected, on_activate, estimate, handle, files } = self;
        let mount: Mount<T> = Rc::new(move |ui: &Ui, row, item| {
            let host = ui.create(WidgetKind::Container, vec![Prop::Row(row)]);
            let content = (render.0)(item).build(ui);
            ui.append_child(host, content);
            vec![host]
        });
        let parts = RowParts { each, key, mount, mode, style, selected, on_activate, estimate, handle, files };
        build_rows(ui, element, parts)
    }
}

/// Builds a `List` or `Table` from its element, and mounts the rows the
/// platform shows.
pub(crate) fn build_rows<T: Clone + 'static, K: Eq + Hash + Clone + 'static>(
    ui: &Ui,
    mut element: Element,
    parts: RowParts<T, K>,
) -> NodeId {
    let RowParts { each, key, mount, mode, style, selected, on_activate, estimate, handle, files } = parts;
    let mode = mode.unwrap_or(Value::Static(match selected {
        Some(_) => SelectionMode::Single,
        None => SelectionMode::None,
    }));
    element.prop(mode, Prop::SelectionMode);
    element.prop(Value::Static(Vec::new()), Prop::Rows);
    element.prop(Value::Static(Vec::new()), Prop::Selected);
    if let Some(style) = style {
        element.prop(Value::Static(style), Prop::ListStyle);
    }
    if let Some(estimate) = estimate {
        element.prop(Value::Static(estimate), Prop::EstimatedRowHeight);
    }
    let id = element.build(ui);

    let rows: Rc<RefCell<Rows<T, K>>> = Rc::new(RefCell::new(Rows {
        order: Vec::new(),
        items: HashMap::new(),
        position: HashMap::new(),
        row_keys: HashMap::new(),
        keys: HashMap::new(),
        next_key: 1,
        mounted: HashMap::new(),
    }));
    // Row scopes hang off a scope of their own in the building owner, so
    // they survive re-runs of the list's effects.
    let rows_scope = Owner::current().map(|o| o.child()).unwrap_or_else(Owner::new_root);

    // Both take the `Ui`: the row handler, kept in the tree, holds it weakly.
    // Puts the mounted rows' hosts in row order.
    let arrange = {
        let rows = rows.clone();
        move |ui: &Ui| {
            let rows = rows.borrow();
            let mut mounted: Vec<(usize, &Vec<NodeId>)> =
                rows.mounted.iter().map(|(row, (hosts, _))| (rows.position[row], hosts)).collect();
            mounted.sort_by_key(|(position, _)| *position);
            ui.set_children(id, mounted.into_iter().flat_map(|(_, hosts)| hosts.iter().copied()).collect());
        }
    };
    let unmount = move |ui: &Ui, rows: &mut Rows<T, K>, row: RowKey| {
        if let Some((hosts, owner)) = rows.mounted.remove(&row) {
            owner.dispose();
            for host in hosts {
                ui.destroy(host);
            }
        }
    };
    // The platform shows and lets go of rows: mount and dispose them.
    {
        let (weak, rows, arrange, unmount) = (ui.downgrade(), rows.clone(), arrange.clone(), unmount);
        ui.on_event(id, move |event| {
            let Some(ui) = weak.upgrade() else { return };
            match event {
                UiEvent::RowShown(row) => {
                    let item = {
                        let rows = rows.borrow();
                        if rows.mounted.contains_key(row) {
                            return;
                        }
                        let Some(item) = rows.items.get(row).cloned() else { return };
                        item
                    };
                    let owner = rows_scope.child();
                    let hosts = owner.with(|| mount(&ui, *row, item));
                    rows.borrow_mut().mounted.insert(*row, (hosts, owner));
                }
                UiEvent::RowHidden(row) => unmount(&ui, &mut rows.borrow_mut(), *row),
                _ => return,
            }
            arrange(&ui);
        });
    }

    // The data: keep the row keys of items that stay, give new ones
    // theirs, and dispose the rows of items that went.
    {
        let (ui, rows) = (ui.clone(), rows.clone());
        let pending = handle.as_ref().map(|h| h.0.clone());
        effect(move || {
            let items = each.get();
            untrack(|| {
                let mut row_files = Vec::new();
                let (order, was) = {
                    let mut rows = rows.borrow_mut();
                    let rows = &mut *rows;
                    let was: HashSet<RowKey> = rows.order.iter().copied().collect();
                    let mut row_keys = HashMap::with_capacity(items.len());
                    let mut keys = HashMap::with_capacity(items.len());
                    let mut by_key = HashMap::with_capacity(items.len());
                    let mut order = Vec::with_capacity(items.len());
                    for item in items {
                        let k = key(&item);
                        let row = match rows.row_keys.get(&k) {
                            Some(row) => *row,
                            None => {
                                rows.next_key += 1;
                                RowKey(rows.next_key - 1)
                            }
                        };
                        if let Some(path) = files.as_ref().and_then(|file| file(&item)) {
                            row_files.push((row, path));
                        }
                        row_keys.insert(k.clone(), row);
                        keys.insert(row, k);
                        by_key.insert(row, item);
                        order.push(row);
                    }
                    rows.position = order.iter().enumerate().map(|(i, row)| (*row, i)).collect();
                    rows.items = by_key;
                    rows.row_keys = row_keys;
                    rows.keys = keys;
                    rows.order = order.clone();
                    let gone: Vec<RowKey> =
                        rows.mounted.keys().filter(|row| !rows.items.contains_key(row)).copied().collect();
                    for row in gone {
                        unmount(&ui, rows, row);
                    }
                    (order, was)
                };
                // Before the rows, so a row can be dragged once it's there.
                if files.is_some() {
                    ui.set_prop(id, Prop::RowFiles(row_files));
                }
                // A key scrolled to before its item came.
                let pending = pending.as_ref().and_then(|state| state.borrow_mut().pending.take());
                let scroll = pending.and_then(|k| rows.borrow().row_keys.get(&k).copied());
                let Some(selected) = selected else {
                    ui.set_prop(id, Prop::Rows(order));
                    if let Some(row) = scroll {
                        ui.scroll_to_row(id, row);
                    }
                    return arrange(&ui);
                };
                // Selected rows that go are deselected here, before the
                // rows change: the platform would report it later, after
                // whatever the app selects next.
                let keys = selected.get_untracked();
                let (kept, shown) = {
                    let rows = rows.borrow();
                    let kept: Vec<K> = keys.iter().filter(|k| rows.row_keys.contains_key(k)).cloned().collect();
                    let mut shown: Vec<RowKey> = kept.iter().map(|k| rows.row_keys[k]).collect();
                    shown.sort_by_key(|row| rows.position[row]);
                    (kept, shown)
                };
                let staying = shown.iter().filter(|row| was.contains(row)).copied().collect();
                ui.set_prop(id, Prop::Selected(staying));
                ui.set_prop(id, Prop::Rows(order));
                // With the rows in, keys selected before theirs came.
                ui.set_prop(id, Prop::Selected(shown));
                if let Some(row) = scroll {
                    ui.scroll_to_row(id, row);
                }
                if kept.len() != keys.len() {
                    selected.set(kept);
                }
                arrange(&ui);
            });
        });
    }

    // The selection, both ways.
    if let Some(selected) = selected {
        let (ui_, rows_) = (ui.clone(), rows.clone());
        effect(move || {
            let keys = selected.get();
            let rows = rows_.borrow();
            // In row order, as platforms report their selection.
            let mut selection: Vec<RowKey> = keys.iter().filter_map(|k| rows.row_keys.get(k).copied()).collect();
            selection.sort_by_key(|row| rows.position[row]);
            drop(rows);
            untrack(|| ui_.set_prop(id, Prop::Selected(selection)));
        });
        let rows = rows.clone();
        ui.on_event(id, move |event| {
            if let UiEvent::Changed(EventValue::Rows(selection)) = event {
                let keys: Vec<K> = {
                    let rows = rows.borrow();
                    selection.iter().filter_map(|row| rows.keys.get(row).cloned()).collect()
                };
                selected.set(keys);
            }
        });
    }
    if let Some(on_activate) = on_activate {
        let rows = rows.clone();
        ui.on_event(id, move |event| {
            if let UiEvent::RowActivated(row) = event {
                let key = rows.borrow().keys.get(row).cloned();
                if let Some(key) = key {
                    on_activate(key);
                }
            }
        });
    }
    if let Some(handle) = handle {
        let rows = rows.clone();
        handle.0.borrow_mut().connection = Some(Connection {
            ui: ui.downgrade(),
            id,
            row_of: Rc::new(move |k| rows.borrow().row_keys.get(k).copied()),
        });
    }
    id
}

// `view!` tags: `<List each=… key=… let:item>`. `each` and `key` come
// first; each step is a type of its own, so a missing one is a compile
// error. Style attributes work at every step, list attributes after `key`.

impl List<(), (), ()> {
    #[doc(hidden)]
    pub fn __tag() -> ListWithoutEach {
        ListWithoutEach(Element::new(WidgetKind::List))
    }
}

#[doc(hidden)]
pub struct ListWithoutEach(Element);

impl ListWithoutEach {
    pub fn each<T: Clone + 'static>(self, each: impl IntoValue<Vec<T>>) -> ListWithoutKey<T> {
        ListWithoutKey { element: self.0, each: each.into_value() }
    }
}

impl ElementBuilder for ListWithoutEach {
    fn element(&mut self) -> &mut Element {
        &mut self.0
    }
}

#[doc(hidden)]
pub struct ListWithoutKey<T: 'static> {
    element: Element,
    each: Value<Vec<T>>,
}

impl<T: Clone + 'static> ListWithoutKey<T> {
    pub fn key<K: Eq + Hash + Clone + 'static>(self, key: impl Fn(&T) -> K + 'static) -> List<T, K, ()> {
        List::with_element(self.element, self.each, Rc::new(key), ())
    }
}

impl<T: 'static> ElementBuilder for ListWithoutKey<T> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> List<T, K, ()> {
    #[doc(hidden)]
    pub fn __children<V: View>(self, render: impl Fn(T) -> V + 'static) -> List<T, K> {
        let List { element, each, key, mode, style, selected, on_activate, estimate, handle, files, .. } = self;
        let render = RowRender(Rc::new(move |item| AnyView::new(render(item))) as Rc<dyn Fn(T) -> AnyView>);
        List { element, each, key, render, mode, style, selected, on_activate, estimate, handle, files }
    }
}
