//! `Table`: the platform's table control, rows of cells under column
//! headers, with cells built by the core.
//!
//! A table is a [`List`](crate::List) whose rows are cells: its data,
//! selection, activation and scrolling work the same way, and the
//! platform realises its rows the same way. For each row it shows, the
//! core mounts a cell host per column (a `Container` carrying its
//! [`CellKey`]), and disposes them when it lets the row go. The platform
//! sizes the columns, and the user resizes them; each cell is laid out at
//! the width its column gives it (`ColumnWidths`).
//!
//! The app sorts: the platform shows which column the table is sorted by
//! and reports the user pressing a header, and the app's data follows.

use std::any::Any;
use std::hash::Hash;
use std::rc::Rc;

use mitsuami_reactive::{IntoValue, Signal, Value, effect};

use crate::command::{EventValue, UiEvent};
use crate::element::{Element, ElementBuilder};
use crate::list::{KeyFn, ListHandle, Mount, RowParts, build_rows};
use crate::services::Shortcut;
use crate::tweak::Tweak;
use crate::ui::Ui;
use crate::units::Length;
use crate::view::{AnyView, View};
use crate::widget::{CellKey, ColumnData, ColumnSort, ListStyle, NodeId, Prop, SelectionMode, SortOrder, WidgetKind};

/// How a [`Table`] is sorted: by the column with this sort key, and which
/// way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sort<S> {
    pub by: S,
    pub order: SortOrder,
}

impl<S> Sort<S> {
    pub fn ascending(by: S) -> Sort<S> {
        Sort { by, order: SortOrder::Ascending }
    }

    pub fn descending(by: S) -> Sort<S> {
        Sort { by, order: SortOrder::Descending }
    }
}

/// A column of a [`Table`]: its header's title, and the view each row
/// shows in its cell.
///
/// ```ignore
/// TableColumn::new("Size", |entry: Entry| Text::new(human_size(entry.size)))
///     .width(80)
///     .sort_key(SortBy::Size)
/// ```
pub struct TableColumn<T: 'static> {
    title: String,
    width: Option<f32>,
    expand: bool,
    sort_key: Option<Rc<dyn Any>>,
    render: Rc<dyn Fn(T) -> AnyView>,
}

impl<T: 'static> TableColumn<T> {
    pub fn new<V: View>(title: impl Into<String>, render: impl Fn(T) -> V + 'static) -> TableColumn<T> {
        TableColumn {
            title: title.into(),
            width: None,
            expand: false,
            sort_key: None,
            render: Rc::new(move |item| AnyView::new(render(item))),
        }
    }

    /// How wide it starts; the user can resize it, as the platform lets
    /// them. Without, it's as wide as the platform makes a column.
    pub fn width(mut self, width: impl Into<Length>) -> TableColumn<T> {
        self.width = match width.into() {
            Length::Px(px) => Some(px),
            _ => None,
        };
        self
    }

    /// Takes the room the table has past its columns' widths, shared with
    /// the other columns that do: a file's name, say.
    pub fn expand(mut self) -> TableColumn<T> {
        self.expand = true;
        self
    }

    /// Makes the column sortable: pressing its header sorts the table by
    /// this key, which the table's [`sort`](Table::sort) signal holds. The
    /// key is of the signal's type.
    pub fn sort_key<S: PartialEq + Clone + 'static>(mut self, key: S) -> TableColumn<T> {
        self.sort_key = Some(Rc::new(key));
        self
    }
}

/// Columns' sort keys, in order: `None` for a column that doesn't sort.
type SortKeys = Vec<Option<Rc<dyn Any>>>;

/// Installs the binding on the built table.
type InstallSort = Box<dyn FnOnce(&Ui, NodeId, SortKeys)>;

/// Binds a table's sort to the app's signal, once the columns are known.
struct SortBinding {
    type_name: &'static str,
    is_key: fn(&dyn Any) -> bool,
    install: InstallSort,
}

/// A virtualised table: rows of cells under column headers, shown by the
/// platform's own table control.
///
/// ```ignore
/// Table::new(move || files.get(), |f| f.path.clone())
///     .column(TableColumn::new("Name", |f: File| Text::new(f.name)).expand().sort_key(By::Name))
///     .column(TableColumn::new("Size", |f: File| Text::new(f.size_text())).width(80).sort_key(By::Size))
///     .sort(sort)                    // Signal<Sort<By>>: the column and order
///     .selected(selected)            // Signal<Vec<K>>: the selected keys
///     .on_activate(|path| open(path))
/// ```
///
/// As in a [`List`](crate::List), only the rows the platform shows are
/// mounted. A row is as high as its highest cell, or as the platform's
/// rows are, if they're higher; a cell is as high as its content, where
/// the platform places it in its row.
///
/// Rows are read by screen readers (and found by tests) as rows named by
/// their cells' text, and the cells as cells named by theirs.
///
/// The type's defaults make `Table` alone name the widget, for
/// [`Tweak<Table>`](Tweak).
pub struct Table<T: 'static = (), K: 'static = ()> {
    element: Element,
    each: Value<Vec<T>>,
    key: KeyFn<T, K>,
    columns: Vec<TableColumn<T>>,
    sort: Option<SortBinding>,
    mode: Option<Value<SelectionMode>>,
    style: Option<ListStyle>,
    selected: Option<Signal<Vec<K>>>,
    on_activate: Option<Rc<dyn Fn(K)>>,
    estimate: Option<f32>,
    handle: Option<ListHandle<K>>,
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> Table<T, K> {
    pub fn new(each: impl IntoValue<Vec<T>>, key: impl Fn(&T) -> K + 'static) -> Table<T, K> {
        Table::with_element(Element::new(WidgetKind::Table), each.into_value(), Rc::new(key))
    }
}

impl<T: 'static, K: 'static> Table<T, K> {
    fn with_element(element: Element, each: Value<Vec<T>>, key: KeyFn<T, K>) -> Table<T, K> {
        Table {
            element,
            each,
            key,
            columns: Vec::new(),
            sort: None,
            mode: None,
            style: None,
            selected: None,
            on_activate: None,
            estimate: None,
            handle: None,
        }
    }

    /// Adds a column, after the others.
    pub fn column(mut self, column: TableColumn<T>) -> Self {
        self.columns.push(column);
        self
    }

    /// Adds columns, after the others.
    pub fn columns(mut self, columns: impl IntoIterator<Item = TableColumn<T>>) -> Self {
        self.columns.extend(columns);
        self
    }

    /// Binds how the table is sorted, both ways: its header shows it, and
    /// pressing a sortable column's header sets it, as the platform sorts
    /// (the same column the other way round, another one ascending). The
    /// app sorts the data by it. Columns are sortable by their
    /// [`sort_key`](TableColumn::sort_key)s, of this signal's type.
    pub fn sort<S: PartialEq + Clone + 'static>(mut self, sort: Signal<Sort<S>>) -> Self {
        self.sort = Some(SortBinding {
            type_name: std::any::type_name::<S>(),
            is_key: |key| key.is::<S>(),
            install: Box::new(move |ui, id, keys| install_sort(ui, id, sort, keys)),
        });
        self
    }

    /// Binds the selection, as the selected rows' keys, both ways. Tables
    /// with a selection bound select one row at a time unless
    /// [`selection_mode`](Table::selection_mode) says otherwise.
    pub fn selected(mut self, selected: Signal<Vec<K>>) -> Self {
        self.selected = Some(selected);
        self
    }

    /// How many rows can be selected, as a `List`'s
    /// [`selection_mode`](crate::List::selection_mode).
    pub fn selection_mode(mut self, mode: impl IntoValue<SelectionMode>) -> Self {
        self.mode = Some(mode.into_value());
        self
    }

    /// How the table sits in its surroundings, as a `List`'s
    /// [`list_style`](crate::List::list_style).
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

    /// Runs `handler` when `key` is pressed while the table, or a control
    /// inside it, has keyboard focus, and the focused control doesn't use
    /// the key itself (the arrows, Home and End, typing to
    /// select, and what else the platform's table takes): Space for a preview, Delete for Move to Trash.
    /// A key goes to the nearest node around the focused control that
    /// takes it. Nothing shows keys taken this way, so give the command a
    /// menu item or a button too, and pick the keys the platform's own apps
    /// use (`platform!`).
    ///
    /// ```ignore
    /// Table::new(..).on_key(' ', preview)
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

    /// Connects a [`ListHandle`], to scroll the table from code.
    pub fn handle(mut self, handle: ListHandle<K>) -> Self {
        self.handle = Some(handle);
        self
    }

    /// Raw platform settings: see [`Tweak`]. They run on the table view,
    /// not the scroll view around it.
    pub fn native(mut self, tweak: Tweak<Table>) -> Self {
        tweak.apply(&mut self.element);
        self
    }
}

impl<T: 'static, K: 'static> ElementBuilder for Table<T, K> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> View for Table<T, K> {
    fn build(self, ui: &Ui) -> NodeId {
        let Table { mut element, each, key, columns, sort, mode, style, selected, on_activate, estimate, handle } =
            self;
        let data: Vec<ColumnData> = columns
            .iter()
            .map(|column| {
                let sortable = match (&sort, &column.sort_key) {
                    (Some(sort), Some(key)) if (sort.is_key)(&**key) => true,
                    (Some(sort), Some(_)) => panic!(
                        "mitsuami: the sort key of the table's column \"{}\" isn't a {}, the type of its sort",
                        column.title, sort.type_name
                    ),
                    _ => false,
                };
                ColumnData { title: column.title.clone(), width: column.width, expand: column.expand, sortable }
            })
            .collect();
        element.prop(Value::Static(data), Prop::Columns);
        let keys: SortKeys = columns.iter().map(|c| c.sort_key.clone()).collect();
        let renders: Vec<Rc<dyn Fn(T) -> AnyView>> = columns.into_iter().map(|c| c.render).collect();
        let mount: Mount<T> = Rc::new(move |ui: &Ui, row, item: T| {
            renders
                .iter()
                .enumerate()
                .map(|(column, render)| {
                    let host = ui.create(WidgetKind::Container, vec![Prop::Cell(CellKey { row, column })]);
                    let content = render(item.clone()).build(ui);
                    ui.append_child(host, content);
                    host
                })
                .collect()
        });
        let id = build_rows(
            ui,
            element,
            RowParts { each, key, mount, mode, style, selected, on_activate, estimate, handle },
        );
        if let Some(sort) = sort {
            (sort.install)(ui, id, keys);
        }
        id
    }
}

/// Shows the app's sort in the header, and sets it from the user's.
fn install_sort<S: PartialEq + Clone + 'static>(ui: &Ui, id: NodeId, sort: Signal<Sort<S>>, keys: SortKeys) {
    let keys: Rc<Vec<Option<S>>> =
        Rc::new(keys.into_iter().map(|k| k.and_then(|k| k.downcast_ref::<S>().cloned())).collect());
    {
        let (ui, keys) = (ui.clone(), keys.clone());
        effect(move || {
            let Sort { by, order } = sort.get();
            let column = keys.iter().position(|k| k.as_ref() == Some(&by));
            ui.set_prop(id, Prop::Sort(column.map(|column| ColumnSort { column, order })));
        });
    }
    ui.on_event(id, move |event| {
        if let UiEvent::Changed(EventValue::Sort(ColumnSort { column, order })) = event
            && let Some(Some(by)) = keys.get(*column)
        {
            sort.set(Sort { by: by.clone(), order: *order });
        }
    });
}

// `view!` tags: `<Table each=… key=… column=… column=…/>`. `each` and
// `key` come first; each step is a type of its own, so a missing one is a
// compile error. Style attributes work at every step, table attributes
// after `key`.

impl Table<(), ()> {
    #[doc(hidden)]
    pub fn __tag() -> TableWithoutEach {
        TableWithoutEach(Element::new(WidgetKind::Table))
    }
}

#[doc(hidden)]
pub struct TableWithoutEach(Element);

impl TableWithoutEach {
    pub fn each<T: Clone + 'static>(self, each: impl IntoValue<Vec<T>>) -> TableWithoutKey<T> {
        TableWithoutKey { element: self.0, each: each.into_value() }
    }
}

impl ElementBuilder for TableWithoutEach {
    fn element(&mut self) -> &mut Element {
        &mut self.0
    }
}

#[doc(hidden)]
pub struct TableWithoutKey<T: 'static> {
    element: Element,
    each: Value<Vec<T>>,
}

impl<T: Clone + 'static> TableWithoutKey<T> {
    pub fn key<K: Eq + Hash + Clone + 'static>(self, key: impl Fn(&T) -> K + 'static) -> Table<T, K> {
        Table::with_element(self.element, self.each, Rc::new(key))
    }
}

impl<T: 'static> ElementBuilder for TableWithoutKey<T> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}
