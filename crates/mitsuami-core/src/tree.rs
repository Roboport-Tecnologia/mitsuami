//! `Tree`: the platform's tree control, rows that nest, with rows built by
//! the core.
//!
//! A tree is a [`List`](crate::List) whose rows have children, shown while
//! the row is open: its data, selection, activation and scrolling work the
//! same way, and the platform realises its rows the same way. The core
//! sends the rows that show, each with its place in the tree
//! (`Prop::TreeRows`); the platform draws the disclosures, indents the
//! levels and reports the user opening and closing rows (`RowExpanded`),
//! which the open set follows.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use mitsuami_reactive::{IntoValue, Signal, Value, signal};

use crate::element::{Element, ElementBuilder};
use crate::list::{FileFn, KeyFn, ListHandle, Mount, MoveFn, RowParts, RowRender, Shape, TreeParts, build_rows};
use crate::services::Shortcut;
use crate::tweak::Tweak;
use crate::ui::Ui;
use crate::units::Length;
use crate::view::{AnyView, View};
use crate::widget::{ListStyle, NodeId, Prop, RowDrop, RowMoves, SelectionMode, WidgetKind};

/// An item's children: `None` for a row that never opens, `Some` for one
/// that does, empty or not.
type ChildrenFn<T> = Rc<dyn Fn(&T) -> Option<Vec<T>>>;

/// Whether an item's children are being read.
type LoadingFn<T> = Rc<dyn Fn(&T) -> bool>;

/// A virtualised tree of rows, shown by the platform's own tree control,
/// which draws the disclosures and indents the levels.
///
/// ```ignore
/// Tree::new(move || folders.get(), |f| f.path.clone(), |f| f.children(), |f| FolderRow(f))
///     .expanded(open)                // Signal<Vec<K>>: the open rows' keys
///     .selected(selected)            // Signal<Vec<K>>: the selected keys
///     .on_activate(|path| open(path))
///     .grow(1.0)
/// ```
///
/// `children` is `None` for a row that never opens (a file) and `Some` for
/// one that does (a folder, empty or not yet read). It's called for each
/// row that shows, so keep it cheap; signals it reads are tracked, so
/// children that load later show when they come. Rows are keyed as a
/// `List`'s, and everything a `List` has works the same way.
///
/// Rows are read by screen readers (and found by tests) as tree items
/// named by their text, with their level and whether they're open.
///
/// The type's defaults make `Tree` alone name the widget, for
/// [`Tweak<Tree>`](Tweak).
pub struct Tree<T: 'static = (), K: 'static = (), R = RowRender<T>> {
    element: Element,
    roots: Value<Vec<T>>,
    key: KeyFn<T, K>,
    children: ChildrenFn<T>,
    render: R,
    expanded: Option<Signal<Vec<K>>>,
    mode: Option<Value<SelectionMode>>,
    style: Option<ListStyle>,
    selected: Option<Signal<Vec<K>>>,
    on_activate: Option<Rc<dyn Fn(K)>>,
    estimate: Option<f32>,
    handle: Option<ListHandle<K>>,
    files: Option<FileFn<T>>,
    moves: Option<(RowMoves, MoveFn<K>)>,
    loading: Option<LoadingFn<T>>,
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> Tree<T, K> {
    pub fn new<V: View>(
        roots: impl IntoValue<Vec<T>>,
        key: impl Fn(&T) -> K + 'static,
        children: impl Fn(&T) -> Option<Vec<T>> + 'static,
        render: impl Fn(T) -> V + 'static,
    ) -> Tree<T, K> {
        Tree::with_element(
            Element::new(WidgetKind::Tree),
            roots.into_value(),
            Rc::new(key),
            Rc::new(children),
            RowRender::new(move |item| AnyView::new(render(item))),
        )
    }
}

impl<T: 'static, K: 'static, R> Tree<T, K, R> {
    fn with_element(
        element: Element,
        roots: Value<Vec<T>>,
        key: KeyFn<T, K>,
        children: ChildrenFn<T>,
        render: R,
    ) -> Tree<T, K, R> {
        Tree {
            element,
            roots,
            key,
            children,
            render,
            expanded: None,
            mode: None,
            style: None,
            selected: None,
            on_activate: None,
            estimate: None,
            handle: None,
            files: None,
            moves: None,
            loading: None,
        }
    }

    /// Rows can be dragged within the tree, with the platform's own row
    /// drag (the selection, when a selected row is dragged), and dropped
    /// where `moves` says, as a `List`'s [`on_move`](crate::List::on_move):
    /// onto a row (into a folder), before one (at its place among its
    /// folder's rows), or at the end (after the top rows). A folder can't
    /// be dropped inside itself. `handler` gets the rows, in the tree's
    /// order, and where; the tree shows nothing moved until the app moves
    /// its data. Offer another way too (a Move command): dragging isn't
    /// reachable from the keyboard or assistive technology.
    pub fn on_move(mut self, moves: RowMoves, handler: impl Fn(Vec<K>, RowDrop<K>) + 'static) -> Self {
        self.moves = Some((moves, Rc::new(handler)));
        self
    }

    /// Which rows' children are being read: a folder whose files take a
    /// moment to come, as on a network share. Once `loading` has said so
    /// for half a second, the platform's spinner shows where the row's
    /// disclosure is, as Finder shows one in place of a folder's triangle;
    /// a folder read quicker opens without one flashing. Signals it reads
    /// are tracked, as `children`'s are. The tree doesn't read children
    /// itself: the app says when they're read, and `children` gives them.
    pub fn loading(mut self, loading: impl Fn(&T) -> bool + 'static) -> Self {
        self.loading = Some(Rc::new(loading));
        self
    }

    /// Binds which rows are open, as their keys, both ways: the user opens
    /// and closes rows (the disclosure, the platform's keys, assistive
    /// technology), and the app can too. A key stays while its row is in a
    /// closed folder, so reopening the folder shows the row open again.
    /// Without it, the tree keeps its own, every row closed at first.
    pub fn expanded(mut self, expanded: Signal<Vec<K>>) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Rows can be dragged out of the app, carrying the files their items
    /// stand for, as a `List`'s [`drag_files`](crate::List::drag_files).
    pub fn drag_files(mut self, file: impl Fn(&T) -> Option<PathBuf> + 'static) -> Self {
        self.files = Some(Rc::new(file));
        self
    }

    /// Binds the selection, as the selected rows' keys, both ways. Trees
    /// with a selection bound select one row at a time unless
    /// [`selection_mode`](Tree::selection_mode) says otherwise. Closing a
    /// folder deselects its rows.
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

    /// How the tree sits in its surroundings, as a `List`'s
    /// [`list_style`](crate::List::list_style).
    pub fn list_style(mut self, style: ListStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Called with a row's key when it's activated: double-clicked, or Enter
    /// pressed on it. It opens nothing: what a folder's activation does is
    /// the app's.
    pub fn on_activate(mut self, handler: impl Fn(K) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Runs `handler` when `key` is pressed while the tree, or a control
    /// inside it, has keyboard focus, and the focused control doesn't use
    /// the key itself (the arrows, the keys that open and close rows, and
    /// what else the platform's tree takes), as a `List`'s
    /// [`on_key`](crate::List::on_key).
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

    /// Connects a [`ListHandle`], to scroll the tree from code. A row in a
    /// closed folder doesn't show, so it's scrolled to once its folders are
    /// open ([`expanded`](Tree::expanded)).
    pub fn handle(mut self, handle: ListHandle<K>) -> Self {
        self.handle = Some(handle);
        self
    }

    /// Raw platform settings: see [`Tweak`]. They run on the tree view, not
    /// the scroll view around it.
    pub fn native(mut self, tweak: Tweak<Tree>) -> Self {
        tweak.apply(&mut self.element);
        self
    }
}

impl<T: 'static, K: 'static, R> ElementBuilder for Tree<T, K, R> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

/// How long a row loads before its spinner shows: a folder read quicker
/// opens without one flashing, as Nautilus waits before showing its own.
const SPINNER_DELAY: Duration = Duration::from_millis(500);

/// The app's `loading`, held back for `SPINNER_DELAY`: when each row that
/// shows began loading, on the `Ui`'s clock, and a signal a timer bumps
/// when a row's time is up. A row that stops loading, or isn't walked (in
/// a closed folder), starts again.
struct Delayed<T: 'static, K: 'static> {
    ui: Ui,
    key: KeyFn<T, K>,
    loading: LoadingFn<T>,
    since: RefCell<HashMap<K, (Duration, u64)>>,
    walk: Cell<u64>,
    due: Signal<u64>,
}

impl<T, K: Eq + Hash + Clone> Delayed<T, K> {
    /// Whether the item's spinner shows. It's asked of each row walked.
    fn shows(&self, item: &T) -> bool {
        if !(self.loading)(item) {
            return false;
        }
        // Walked again when a timer is due.
        self.due.get();
        let now = self.ui.executor().now();
        let mut since = self.since.borrow_mut();
        let walk = self.walk.get();
        let entry = since.entry((self.key)(item)).or_insert_with(|| {
            let (ui, due) = (self.ui.clone(), self.due);
            self.ui.spawn_local(async move {
                ui.sleep(SPINNER_DELAY).await;
                if due.is_alive() {
                    due.update(|n| *n += 1);
                }
            });
            (now, walk)
        });
        entry.1 = walk;
        now.saturating_sub(entry.0) >= SPINNER_DELAY
    }

    /// Walks the rows, forgetting those not loading in it.
    fn walk<R>(&self, f: impl FnOnce() -> R) -> R {
        let walk = self.walk.get() + 1;
        self.walk.set(walk);
        let out = f();
        self.since.borrow_mut().retain(|_, (_, seen)| *seen == walk);
        out
    }
}

/// A row that shows: its item and its place in the tree.
#[derive(Clone)]
struct Flat<T> {
    item: T,
    shape: Shape,
}

/// The rows that show, in order: each root, then, while it's open, its
/// children's rows, and so on down. A row whose key is one of its own
/// folders' doesn't open, so a cycle in the app's data ends there.
fn flatten<T: Clone, K: Eq + Hash>(
    roots: Vec<T>,
    key: &dyn Fn(&T) -> K,
    children: &dyn Fn(&T) -> Option<Vec<T>>,
    loading: Option<&dyn Fn(&T) -> bool>,
    open: &HashSet<K>,
) -> Vec<Flat<T>> {
    struct Walk<'a, T, K> {
        key: &'a dyn Fn(&T) -> K,
        children: &'a dyn Fn(&T) -> Option<Vec<T>>,
        loading: Option<&'a dyn Fn(&T) -> bool>,
        open: &'a HashSet<K>,
        /// The open rows above the one being walked.
        folders: Vec<K>,
        out: Vec<Flat<T>>,
    }

    impl<T: Clone, K: Eq + Hash> Walk<'_, T, K> {
        fn visit(&mut self, items: Vec<T>, parent: Option<usize>, depth: u32) {
            for item in items {
                let k = (self.key)(&item);
                let kids = (self.children)(&item);
                let expandable = kids.is_some();
                let expanded = expandable && self.open.contains(&k) && !self.folders.contains(&k);
                let loading = self.loading.is_some_and(|loading| loading(&item));
                let index = self.out.len();
                self.out.push(Flat { item, shape: Shape { parent, depth, expandable, expanded, loading } });
                if let (true, Some(kids)) = (expanded, kids) {
                    self.folders.push(k);
                    self.visit(kids, Some(index), depth + 1);
                    self.folders.pop();
                }
            }
        }
    }

    let mut walk = Walk { key, children, loading, open, folders: Vec::new(), out: Vec::new() };
    walk.visit(roots, None, 0);
    walk.out
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> View for Tree<T, K> {
    fn build(self, ui: &Ui) -> NodeId {
        let Tree {
            element,
            roots,
            key,
            children,
            render,
            expanded,
            mode,
            style,
            selected,
            on_activate,
            estimate,
            handle,
            files,
            moves,
            loading,
        } = self;
        let expanded = expanded.unwrap_or_else(|| signal(Vec::new()));
        let loads = loading.is_some();
        let delayed = loading.map(|loading| Delayed {
            ui: ui.clone(),
            key: key.clone(),
            loading,
            since: RefCell::default(),
            walk: Cell::new(0),
            due: signal(0),
        });
        let each = {
            let (key, children) = (key.clone(), children.clone());
            Value::Dynamic(Rc::new(move || {
                let open: HashSet<K> = expanded.get().into_iter().collect();
                match &delayed {
                    Some(delayed) => {
                        let shows = |item: &T| delayed.shows(item);
                        delayed.walk(|| flatten(roots.get(), &*key, &*children, Some(&shows), &open))
                    }
                    None => flatten(roots.get(), &*key, &*children, None, &open),
                }
            }))
        };
        let mount: Mount<Flat<T>> = Rc::new(move |ui: &Ui, row, flat: Flat<T>| {
            let host = ui.create(WidgetKind::Container, vec![Prop::Row(row)]);
            let content = render.render(flat.item).build(ui);
            ui.append_child(host, content);
            vec![host]
        });
        let tree = TreeParts {
            shape: Rc::new(|flat: &Flat<T>| flat.shape),
            on_expand: Rc::new(move |k: K, open: bool| {
                let has = expanded.with_untracked(|keys| keys.contains(&k));
                match (open, has) {
                    (true, false) => expanded.update(|keys| keys.push(k)),
                    (false, true) => expanded.update(|keys| keys.retain(|o| *o != k)),
                    _ => {}
                }
            }),
            loading: loads,
        };
        let item_key = key.clone();
        let parts = RowParts {
            each,
            key: Rc::new(move |flat: &Flat<T>| item_key(&flat.item)),
            mount,
            mode,
            style,
            selected,
            on_activate,
            estimate,
            handle,
            files: files.map(|file| Rc::new(move |flat: &Flat<T>| file(&flat.item)) as FileFn<Flat<T>>),
            moves,
            tree: Some(tree),
        };
        build_rows(ui, element, parts)
    }
}

// `view!` tags: `<Tree roots=… key=… children=… let:item>`. `roots`, `key`
// and `children` come first; each step is a type of its own, so a missing
// one is a compile error. Style attributes work at every step, tree
// attributes after `children`.

impl Tree<(), (), ()> {
    #[doc(hidden)]
    pub fn __tag() -> TreeWithoutRoots {
        TreeWithoutRoots(Element::new(WidgetKind::Tree))
    }
}

#[doc(hidden)]
pub struct TreeWithoutRoots(Element);

impl TreeWithoutRoots {
    pub fn roots<T: Clone + 'static>(self, roots: impl IntoValue<Vec<T>>) -> TreeWithoutKey<T> {
        TreeWithoutKey { element: self.0, roots: roots.into_value() }
    }
}

impl ElementBuilder for TreeWithoutRoots {
    fn element(&mut self) -> &mut Element {
        &mut self.0
    }
}

#[doc(hidden)]
pub struct TreeWithoutKey<T: 'static> {
    element: Element,
    roots: Value<Vec<T>>,
}

impl<T: Clone + 'static> TreeWithoutKey<T> {
    pub fn key<K: Eq + Hash + Clone + 'static>(self, key: impl Fn(&T) -> K + 'static) -> TreeWithoutChildren<T, K> {
        TreeWithoutChildren { element: self.element, roots: self.roots, key: Rc::new(key) }
    }
}

impl<T: 'static> ElementBuilder for TreeWithoutKey<T> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

#[doc(hidden)]
pub struct TreeWithoutChildren<T: 'static, K: 'static> {
    element: Element,
    roots: Value<Vec<T>>,
    key: KeyFn<T, K>,
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> TreeWithoutChildren<T, K> {
    pub fn children(self, children: impl Fn(&T) -> Option<Vec<T>> + 'static) -> Tree<T, K, ()> {
        Tree::with_element(self.element, self.roots, self.key, Rc::new(children), ())
    }
}

impl<T: 'static, K: 'static> ElementBuilder for TreeWithoutChildren<T, K> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<T: Clone + 'static, K: Eq + Hash + Clone + 'static> Tree<T, K, ()> {
    #[doc(hidden)]
    pub fn __children<V: View>(self, render: impl Fn(T) -> V + 'static) -> Tree<T, K> {
        let Tree {
            element,
            roots,
            key,
            children,
            expanded,
            mode,
            style,
            selected,
            on_activate,
            estimate,
            handle,
            files,
            moves,
            loading,
            ..
        } = self;
        Tree {
            element,
            roots,
            key,
            children,
            render: RowRender::new(move |item| AnyView::new(render(item))),
            expanded,
            mode,
            style,
            selected,
            on_activate,
            estimate,
            handle,
            files,
            moves,
            loading,
        }
    }
}
