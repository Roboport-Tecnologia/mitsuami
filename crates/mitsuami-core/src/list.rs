//! `List`: a native list control that mounts only the rows in view.
//!
//! The native control (NSTableView, ListView, gtk::ListView, QML ListView)
//! scrolls, selects and draws rows. The core decides which rows exist:
//! after each layout it works out the rows in view, or within a viewport of
//! it, and the list mounts those, each in a host `Container` carrying its
//! [`RowKey`], and disposes the rest. Rows are keyed like [`For`](crate::For)'s.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Range;
use std::rc::Rc;

use mitsuami_reactive::{IntoValue, Owner, Signal, Value, effect, untrack};

use crate::command::{EventValue, UiEvent};
use crate::element::{Element, ElementBuilder};
use crate::ui::{Ui, WeakUi};
use crate::units::Length;
use crate::view::{AnyView, View};
use crate::widget::{NodeId, Prop, RowKey, SelectionMode, WidgetKind};

type KeyFn<T, K> = Rc<dyn Fn(&T) -> K>;
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
/// Only the rows in view, and a viewport's worth on either side, are
/// mounted; the others exist only as their keys and heights. Rows can hold
/// any view. Each row is laid out at the list's width and is as high as its
/// content; rows not mounted yet count as [`estimated_row_height`](List::estimated_row_height).
///
/// Rows are read by screen readers (and found by tests) as list items named
/// by their text.
pub struct List<T: 'static, K: 'static, R = RowRender<T>> {
    element: Element,
    each: Value<Vec<T>>,
    key: KeyFn<T, K>,
    render: R,
    mode: Option<SelectionMode>,
    selected: Option<Signal<Vec<K>>>,
    on_activate: Option<Rc<dyn Fn(K)>>,
    estimate: Option<f32>,
    handle: Option<ListHandle<K>>,
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
        List { element, each, key, render, mode: None, selected: None, on_activate: None, estimate: None, handle: None }
    }

    /// Binds the selection, as the selected rows' keys, both ways. Lists
    /// with a selection bound select one row at a time unless
    /// [`selection_mode`](List::selection_mode) says otherwise.
    pub fn selected(mut self, selected: Signal<Vec<K>>) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Called with a row's key when it's activated: double-clicked, or Enter
    /// pressed on it.
    pub fn on_activate(mut self, handler: impl Fn(K) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// How high rows are before they are mounted and measured. By default,
    /// the mean height of the rows measured so far.
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
}

impl<T: 'static, K: 'static, R> ElementBuilder for List<T, K, R> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

/// Scrolls a [`List`] from code: create one, pass it to
/// [`List::handle`], and call [`scroll_to`](ListHandle::scroll_to).
pub struct ListHandle<K: 'static>(Rc<RefCell<Option<Connection<K>>>>);

struct Connection<K> {
    ui: WeakUi,
    id: NodeId,
    row_of: RowOf<K>,
}

impl<K: 'static> ListHandle<K> {
    pub fn new() -> ListHandle<K> {
        ListHandle(Rc::default())
    }

    /// The list's node, once it's built.
    pub fn id(&self) -> Option<NodeId> {
        self.0.borrow().as_ref().map(|c| c.id)
    }

    /// Scrolls just enough to show the row with this key, mounted or not.
    pub fn scroll_to(&self, key: &K) {
        let connection = self.0.borrow();
        let Some(Connection { ui, id, row_of }) = connection.as_ref() else { return };
        if let (Some(ui), Some(row)) = (ui.upgrade(), row_of(key)) {
            ui.scroll_to_row(*id, row);
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

/// The list's items and mounted rows, shared by its effects and the core's
/// mount requests.
struct Rows<T, K> {
    /// The items, in order, with their row keys.
    items: Vec<(RowKey, T)>,
    row_keys: HashMap<K, RowKey>,
    keys: HashMap<RowKey, K>,
    next_key: u64,
    /// Mounted rows: host node and scope.
    mounted: HashMap<RowKey, (NodeId, Owner)>,
    range: Range<usize>,
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> View for List<T, K> {
    fn build(self, ui: &Ui) -> NodeId {
        let List { mut element, each, key, render, mode, selected, on_activate, estimate, handle } = self;
        let mode = mode.unwrap_or(if selected.is_some() { SelectionMode::Single } else { SelectionMode::None });
        element.prop(Value::Static(mode), Prop::SelectionMode);
        element.prop(Value::Static(Vec::new()), Prop::Rows);
        element.prop(Value::Static(Vec::new()), Prop::Selected);
        let id = element.build(ui);

        let rows: Rc<RefCell<Rows<T, K>>> = Rc::new(RefCell::new(Rows {
            items: Vec::new(),
            row_keys: HashMap::new(),
            keys: HashMap::new(),
            next_key: 1,
            mounted: HashMap::new(),
            range: 0..0,
        }));
        // Row scopes hang off a scope of their own in the building owner, so
        // they survive re-runs of the list's effects.
        let rows_scope = Owner::current().map(|o| o.child()).unwrap_or_else(Owner::new_root);

        // Mounts the rows in `range` that aren't, disposes those outside it,
        // and puts the hosts in row order.
        let remount = {
            let (ui, rows) = (ui.clone(), rows.clone());
            move |range: Range<usize>| {
                let mut rows = rows.borrow_mut();
                let range = range.start.min(rows.items.len())..range.end.min(rows.items.len());
                let wanted: Vec<(RowKey, T)> = rows.items[range.clone()].to_vec();
                let keep: std::collections::HashSet<RowKey> = wanted.iter().map(|(k, _)| *k).collect();
                let gone: Vec<RowKey> = rows.mounted.keys().filter(|k| !keep.contains(k)).copied().collect();
                for row in gone {
                    let (host, owner) = rows.mounted.remove(&row).expect("mounted");
                    owner.dispose();
                    ui.destroy(host);
                }
                for (row, item) in wanted {
                    if rows.mounted.contains_key(&row) {
                        continue;
                    }
                    let host = ui.create(WidgetKind::Container, vec![Prop::Row(row)]);
                    let owner = rows_scope.child();
                    let content = owner.with(|| (render.0)(item).build(&ui));
                    ui.append_child(host, content);
                    rows.mounted.insert(row, (host, owner));
                }
                rows.range = range;
                let order: Vec<NodeId> =
                    rows.items[rows.range.clone()].iter().map(|(k, _)| rows.mounted[k].0).collect();
                ui.set_children(id, order);
            }
        };
        let remount = Rc::new(remount);
        ui.register_list(id, estimate, {
            let remount = remount.clone();
            Rc::new(move |range| remount(range))
        });

        // The data: keep the row keys of items that stay, give new ones
        // theirs, and drop the rows of items that went.
        {
            let (ui, rows) = (ui.clone(), rows.clone());
            effect(move || {
                let items = each.get();
                untrack(|| {
                    let order = {
                        let mut rows = rows.borrow_mut();
                        let rows = &mut *rows;
                        let mut row_keys = HashMap::with_capacity(items.len());
                        let mut keys = HashMap::with_capacity(items.len());
                        let mut next = Vec::with_capacity(items.len());
                        for item in items {
                            let k = key(&item);
                            let row = match rows.row_keys.get(&k) {
                                Some(row) => *row,
                                None => {
                                    rows.next_key += 1;
                                    RowKey(rows.next_key - 1)
                                }
                            };
                            row_keys.insert(k.clone(), row);
                            keys.insert(row, k);
                            next.push((row, item));
                        }
                        rows.items = next;
                        rows.row_keys = row_keys;
                        rows.keys = keys;
                        rows.items.iter().map(|(k, _)| *k).collect::<Vec<_>>()
                    };
                    ui.set_list_rows(id, order);
                    // Keep the mounted rows that stay, where they are now.
                    let range = rows.borrow().range.clone();
                    let kept: Vec<RowKey> = {
                        let rows = rows.borrow();
                        rows.mounted.keys().filter(|k| rows.keys.contains_key(k)).copied().collect()
                    };
                    let range = {
                        let rows = rows.borrow();
                        let indices: Vec<usize> = rows
                            .items
                            .iter()
                            .enumerate()
                            .filter(|(_, (k, _))| kept.contains(k))
                            .map(|(i, _)| i)
                            .collect();
                        match (indices.first(), indices.last()) {
                            (Some(first), Some(last)) => *first..last + 1,
                            _ => range.start.min(rows.items.len())..range.start.min(rows.items.len()),
                        }
                    };
                    remount(range);
                });
            });
        }

        // The selection, both ways.
        if let Some(selected) = selected {
            let (ui_, rows_) = (ui.clone(), rows.clone());
            effect(move || {
                let keys = selected.get();
                let rows = rows_.borrow();
                let selection: Vec<RowKey> = keys.iter().filter_map(|k| rows.row_keys.get(k).copied()).collect();
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
            *handle.0.borrow_mut() = Some(Connection {
                ui: ui.downgrade(),
                id,
                row_of: Rc::new(move |k| rows.borrow().row_keys.get(k).copied()),
            });
        }
        id
    }
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
        let List { element, each, key, mode, selected, on_activate, estimate, handle, .. } = self;
        let render = RowRender(Rc::new(move |item| AnyView::new(render(item))) as Rc<dyn Fn(T) -> AnyView>);
        List { element, each, key, render, mode, selected, on_activate, estimate, handle }
    }
}
