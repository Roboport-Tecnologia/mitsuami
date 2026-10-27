//! `Tree`: WinUI's `TreeView` of `TreeViewNode`s, a node per row that
//! shows, whose content is its key (a boxed string, as a list's items
//! are). A row that opens but whose children aren't sent (it's closed, or
//! its folder is empty) has `HasUnrealizedChildren`, so the view shows its
//! glyph. The tree view draws the glyphs, indents the levels, and opens and
//! closes rows with its own keys; `Expanding` and `Collapsed` report the
//! user's.
//!
//! Its rows are its own `TreeViewList`'s, a `ListView` in its template
//! (`ListControl`), whose items are the nodes that show: once the template
//! is applied, its containers are followed as a list's are, each showing a
//! `Canvas` cell (the tree view's item template) that holds the row's host.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::{
    EventValue, ListStyle, NodeId, Point, Rect, RowDrop, RowKey, RowMoves, SelectionMode, TreeIndent, TreeRow, UiEvent,
};
use windows_core::{EventRevoker, IInspectable, Interface};

use super::{
    DRAGGED, Data, FRAME_BORDER, ITEM_TEMPLATE, R, container_changed, container_under, dragging, end_drag, files_of,
    find_scroll_viewer, identity, key_of, mark_for, measure_rows, offset_of, own_drag, place_row, put_in, queue_report,
    report, report_offset, row_under, start_drag, start_following, take_out, track, within,
};
use crate::backend::{Events, boxed};
use crate::bindings as w;

/// A framed tree: a card's border and background, as a framed list's.
const FRAMED_STYLE: &str = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="TreeView">
    <Setter Property="BorderThickness" Value="1"/>
    <Setter Property="BorderBrush" Value="{ThemeResource CardStrokeColorDefaultBrush}"/>
    <Setter Property="Background" Value="{ThemeResource CardBackgroundFillColorDefaultBrush}"/>
    <Setter Property="CornerRadius" Value="{ThemeResource ControlCornerRadius}"/>
</Style>"#;

const PLAIN_STYLE: &str =
    r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="TreeView"/>"#;

/// How far `TreeViewItem` indents each level (its template settings'
/// `Indentation` is 16 a level), until a row below the top shows.
const PER_LEVEL: f32 = 16.0;

/// The nodes, and what was sent.
#[derive(Default)]
struct Nodes {
    by_key: HashMap<RowKey, w::TreeViewNode>,
    /// Each row as sent: its parent, depth, and whether it opens and is
    /// open.
    rows: HashMap<RowKey, TreeRow>,
    /// The indent last reported.
    indent: Option<TreeIndent>,
    /// How far short of the tree's inside a top row's content ends: the
    /// room `TreeViewItem` keeps at its sides (`report_row_width`).
    short: Option<f32>,
    /// The closed folder a drag of its own rows rests on, and since when
    /// (`OPEN_AFTER`).
    hover: Option<(RowKey, std::time::Instant)>,
}

/// How long rows dragged onto a closed folder rest there before it opens,
/// so they can go further in, as AppKit's outline view and GTK's tree
/// expander open one: XAML's own (`TreeViewItem`'s `c_dragOverInterval`),
/// which it only keeps while it reorders the rows itself.
const OPEN_AFTER: std::time::Duration = std::time::Duration::from_secs(1);

pub(crate) struct Tree {
    pub view: w::TreeView,
    id: NodeId,
    events: Events,
    data: Rc<RefCell<Data>>,
    nodes: Rc<RefCell<Nodes>>,
    /// Set while the backend opens and closes rows itself: `Expanding` and
    /// `Collapsed` are the user's otherwise.
    expanding: Rc<Cell<bool>>,
    /// The tree view's own list, once its template is applied: a drag over
    /// the tree asks its containers where the pointer is.
    inner: Rc<RefCell<Option<w::ListView>>>,
    /// The scroll offset last reported.
    pub offset: Rc<Cell<Point>>,
    /// What a drag of its own rows over it does, and a drop: on the tree
    /// view, and on its list once that's found, which handles them first.
    drag_over: DragHandler,
    drop_rows: DragHandler,
    revokers: RefCell<Vec<EventRevoker>>,
}

type DragHandler = Rc<dyn Fn(&w::DragEventArgs)>;

/// A row's key: its node's content's, or the item's own.
fn node_key(item: &IInspectable) -> Option<RowKey> {
    match item.cast::<w::ITreeViewNode>() {
        Ok(node) => key_of(&node.Content().ok()?),
        Err(_) => key_of(item),
    }
}

/// Makes a node collection hold these nodes, by the fewest changes: the
/// nodes before and after the part that changed stay, and so do their
/// containers.
fn sync(vector: &windows_collections::IVector<w::TreeViewNode>, nodes: &[w::TreeViewNode]) -> R<()> {
    let old: Vec<usize> = (0..vector.Size()?).filter_map(|i| vector.GetAt(i).ok()).map(|n| identity(&n)).collect();
    let new: Vec<usize> = nodes.iter().map(identity).collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    // From the end, so the nodes still to go don't move.
    for i in (prefix..old.len() - suffix).rev() {
        vector.RemoveAt(i as u32)?;
    }
    for (i, node) in nodes[prefix..nodes.len() - suffix].iter().enumerate() {
        vector.InsertAt((prefix + i) as u32, node)?;
    }
    Ok(())
}

/// Where rows dragged within the tree land: `RowMoves::tree_target` of
/// where the pointer proposes, the rows in the tree's order: only folders
/// take rows onto them, nothing lands over a dragged row (`over`), and a
/// folder never goes inside itself. `None`: refused.
fn drop_target(
    d: &Data,
    sent: &HashMap<RowKey, TreeRow>,
    dragged: &[RowKey],
    over: Option<RowKey>,
    proposed: RowDrop<RowKey>,
) -> Option<(Vec<RowKey>, RowDrop<RowKey>)> {
    let moves = d.moves?;
    let mut rows: Vec<RowKey> = dragged.iter().filter(|row| d.index.contains_key(row)).copied().collect();
    rows.sort_by_key(|row| d.index[row]);
    let to = moves.tree_target(&rows, over, proposed, |row| sent.get(&row).copied())?;
    (!rows.is_empty()).then_some((rows, to))
}

/// The row under a drag's pointer, and what the pointer proposes there, as
/// every tree proposes (`RowMoves::tree_proposal`): below the rows, the end.
fn tree_proposed(list: &w::ListView, d: &Data, nodes: &Nodes, at: w::Point) -> (Option<RowKey>, RowDrop<RowKey>) {
    let Some((row, y, height)) = row_under(list, at, node_key) else { return (None, RowDrop::End) };
    let next = d.index.get(&row).and_then(|i| d.rows.get(i + 1)).copied();
    // Across the list, whose rows start at its edge: it picks the level.
    let x = Some(at.x);
    let indent = nodes.indent.unwrap_or(TreeIndent { start: 0.0, per_level: PER_LEVEL });
    let proposed =
        RowMoves::tree_proposal(row, x, y as f32, height as f32, indent, next, |r| nodes.rows.get(&r).copied());
    (Some(row), proposed)
}

/// Where the tree's own rows dragged to `at` in its list would land: the
/// mark shows it, a line from where the rows of the level they land at
/// start (the row's before one, inside the folder for the end of one, the
/// top rows' for the end), and the list scrolls near its edges.
fn follow_drag(
    view: &w::TreeView,
    list: &w::ListView,
    data: &Rc<RefCell<Data>>,
    nodes: &Rc<RefCell<Nodes>>,
    id: NodeId,
    at: w::Point,
) {
    let Some(dragged) = dragging(id) else { return };
    let Ok(element) = list.cast::<w::UIElement>() else { return };
    let under = container_under(list, at, node_key);
    let (to, line_x, edge, drop_mark) = {
        let d = data.borrow();
        let nodes = nodes.borrow();
        let target = within(&element, at)
            .then(|| {
                let (over, proposed) = tree_proposed(list, &d, &nodes, at);
                drop_target(&d, &nodes.rows, &dragged, over, proposed)
            })
            .flatten();
        let to = target.map(|(_, to)| to);
        let line_x = to.as_ref().map_or(0.0, |to| {
            let level = match to {
                RowDrop::Before(r) => nodes.rows.get(r).map_or(0, |r| r.depth),
                RowDrop::On(f) => nodes.rows.get(f).map_or(0, |f| f.depth + 1),
                RowDrop::End => 0,
            };
            let indent = nodes.indent.unwrap_or(TreeIndent { start: 0.0, per_level: PER_LEVEL });
            f64::from(indent.start + level as f32 * indent.per_level)
        });
        (to, line_x, d.edge.clone(), d.mark.clone())
    };
    if let Some(scroll) = list.cast().ok().and_then(|l| find_scroll_viewer(&l)) {
        edge.drag_at(&scroll, &element, at);
    }
    // Onto the closed folder under the pointer, for long enough: it opens,
    // as the user would open it (`Expanding` reports it).
    let onto = match (&to, &under) {
        (Some(RowDrop::On(folder)), Some((row, ..))) if folder == row => Some(*folder),
        _ => None,
    };
    let open = {
        let mut nodes = nodes.borrow_mut();
        let closed = onto.filter(|f| nodes.rows.get(f).is_some_and(|r| r.expandable && !r.expanded));
        match (closed, nodes.hover) {
            (Some(folder), Some((resting, since))) if resting == folder => {
                let due = since.elapsed() >= OPEN_AFTER;
                if due {
                    nodes.hover = None;
                }
                due.then(|| nodes.by_key.get(&folder).cloned()).flatten()
            }
            (Some(folder), _) => {
                nodes.hover = Some((folder, std::time::Instant::now()));
                None
            }
            (None, _) => {
                nodes.hover = None;
                None
            }
        }
    };
    if let Some(node) = open {
        _ = view.cast::<w::ITreeView>().and_then(|v| v.Expand(&node));
    }
    match to.and_then(|to| mark_for(list, under, &to, line_x)) {
        Some(shown) => drop_mark.show(&element, shown),
        None => _ = drop_mark.hide(),
    }
}

/// A drag of the tree's own rows over `element`, its drop, and its leaving.
fn listen_to_drags(
    element: &w::UIElement,
    data: &Rc<RefCell<Data>>,
    drag_over: &DragHandler,
    drop_rows: &DragHandler,
) -> R<Vec<EventRevoker>> {
    let element = element.cast::<w::IUIElement>()?;
    let (over, dropped, data) = (drag_over.clone(), drop_rows.clone(), data.clone());
    // A row the pointer comes into sets the drag to nothing
    // (`TreeViewItem::OnDragEnter`), which showed as refused until the
    // pointer moved again: coming in is answered as moving over.
    let entered = over.clone();
    Ok(vec![
        element.DragEnter(move |_, args| {
            if let Some(args) = args.as_ref() {
                entered(args);
            }
        })?,
        element.DragOver(move |_, args| {
            if let Some(args) = args.as_ref() {
                over(args);
            }
        })?,
        element.Drop(move |_, args| {
            if let Some(args) = args.as_ref() {
                dropped(args);
            }
        })?,
        element.DragLeave({
            let element = element.cast::<w::UIElement>()?;
            move |_, args| {
                if args.as_ref().is_some_and(|args| super::mark::still_over(&element, args)) {
                    return;
                }
                let d = data.borrow();
                d.edge.stop();
                _ = d.mark.hide();
            }
        })?,
    ])
}

/// The rows a node collection shows, down through the open ones.
fn walk(
    vector: &windows_collections::IVector<w::TreeViewNode>,
    parent: Option<RowKey>,
    depth: u32,
    out: &mut Vec<TreeRow>,
) {
    for node in (0..vector.Size().unwrap_or(0)).filter_map(|i| vector.GetAt(i).ok()) {
        let Some(row) = node.cast::<IInspectable>().ok().as_ref().and_then(node_key) else { continue };
        let Ok(node) = node.cast::<w::ITreeViewNode>() else { continue };
        let expandable = node.HasChildren().unwrap_or(false) || node.HasUnrealizedChildren().unwrap_or(false);
        let expanded = node.IsExpanded().unwrap_or(false);
        out.push(TreeRow { row, parent, depth, expandable, expanded });
        if expanded && let Ok(children) = node.Children() {
            walk(&children, Some(row), depth + 1, out);
        }
    }
}

impl Tree {
    pub(crate) fn new(id: NodeId, events: Events) -> R<Tree> {
        let view = w::TreeView::new()?;
        let template: w::DataTemplate = w::XamlReader::Load(ITEM_TEMPLATE)?.cast()?;
        view.cast::<w::ITreeView2>()?.SetItemTemplate(&template)?;
        let data = Rc::new(RefCell::new(Data::default()));
        let nodes: Rc<RefCell<Nodes>> = Rc::default();
        let inner: Rc<RefCell<Option<w::ListView>>> = Rc::default();
        let expanding = Rc::new(Cell::new(false));
        let mut revokers = Vec::new();

        // XAML's tree view drags its nodes, and moves them itself where
        // they're dropped. The app moves its data here, so the tree view
        // never reorders, and drags only once its rows move within it or
        // carry files (`set_moves`, `set_row_files`).
        let tree = view.cast::<w::ITreeView2>()?;
        tree.SetCanReorderItems(false)?;
        tree.SetCanDragItems(false)?;
        view.cast::<w::IUIElement>()?.SetAllowDrop(false)?;
        // Where its own rows dragged over it would land, followed as the
        // pointer moves (`track`): the mark, and the edge scrolling.
        let follow: track::Follow = Rc::new({
            let (view, data, nodes, inner) = (view.clone(), data.clone(), nodes.clone(), inner.clone());
            move |at| {
                if let Some(list) = inner.borrow().clone() {
                    follow_drag(&view, &list, &data, &nodes, id, at);
                }
            }
        });
        revokers.push(tree.DragItemsStarting({
            let (data, inner) = (data.clone(), inner.clone());
            move |_, args| {
                let Some(args) = args.as_ref() else { return };
                let rows: Vec<RowKey> = args
                    .Items()
                    .map(|items| (&items).into_iter().filter_map(|i| node_key(&i)).collect())
                    .unwrap_or_default();
                let Ok(package) = args.Data() else { return };
                if !start_drag(&data.borrow(), id, rows, &package) {
                    _ = args.SetCancel(true);
                    return;
                }
                if let Some(scroll) = inner.borrow().as_ref().and_then(|l| find_scroll_viewer(&l.cast().ok()?)) {
                    start_following(&data, &scroll, &follow);
                }
            }
        })?);
        // Its own rows dragged over it: they go where `RowMoves::
        // tree_target` puts them. The events come from its list, which
        // takes drops itself only while it reorders, and scrolls near its
        // edges, too eagerly for a short tree (`edge`): its list handles
        // them, once found (`inner`), and the tree view until then.
        //
        // Always taken, and as a copy. A copy, not a move: XAML's tree view
        // moves the nodes itself on a drop accepted as a move, before any
        // handler hears of it (`TreeViewItem::OnDrop` into the row under
        // the pointer, file or folder, and `TreeViewList::OnDrop` to a slot
        // it keeps from its own reorder), however `CanReorderItems` is set;
        // the copy's glyph doesn't show. Always: XAML raises nothing where
        // the pointer comes to rest, so a refusal would show where it had
        // been; the mark shows where they land, and a drop where they land
        // nowhere does nothing.
        let drag_over: DragHandler = Rc::new({
            let (view, data, nodes, inner) = (view.clone(), data.clone(), nodes.clone(), inner.clone());
            move |args| {
                if own_drag(args, id).is_none() {
                    return;
                }
                let Some(list) = inner.borrow().clone() else { return };
                let Ok(element) = list.cast::<w::UIElement>() else { return };
                let Ok(at) = args.GetPosition(&element) else { return };
                _ = args.SetAcceptedOperation(w::DataPackageOperation::Copy);
                _ = args.DragUIOverride().and_then(|ui| ui.SetIsGlyphVisible(false));
                _ = args.SetHandled(true);
                let track = data.borrow().track.clone();
                track.anchor(&element, at);
                follow_drag(&view, &list, &data, &nodes, id, at);
            }
        });
        let drop_rows: DragHandler = Rc::new({
            let (data, events, nodes, inner) = (data.clone(), events.clone(), nodes.clone(), inner.clone());
            move |args| {
                let Some(dragged) = own_drag(args, id) else { return };
                DRAGGED.with(|d| d.borrow_mut().take());
                end_drag(&data);
                let Some(list) = inner.borrow().clone() else { return };
                let Some(at) = list.cast::<w::UIElement>().ok().and_then(|l| args.GetPosition(&l).ok()) else {
                    return;
                };
                let moved = {
                    let d = data.borrow();
                    let nodes = nodes.borrow();
                    let (over, proposed) = tree_proposed(&list, &d, &nodes, at);
                    drop_target(&d, &nodes.rows, &dragged, over, proposed)
                };
                if let Some((rows, to)) = moved {
                    events.emit(id, UiEvent::RowsMoved { rows, to });
                }
                _ = args.SetHandled(true);
            }
        });
        revokers.extend(listen_to_drags(&view.cast()?, &data, &drag_over, &drop_rows)?);
        revokers.push(tree.DragItemsCompleted({
            let data = data.clone();
            move |_, _| end_drag(&data)
        })?);

        // The user opening and closing rows: the glyph, the keys, UI
        // Automation's expand-collapse pattern.
        revokers.push(view.cast::<w::ITreeView>()?.Expanding({
            let (events, expanding) = (events.clone(), expanding.clone());
            move |_, args| {
                let node = args.as_ref().and_then(|a| a.cast::<w::ITreeViewExpandingEventArgs>().ok()?.Node().ok());
                if let Some(key) = node.and_then(|n| node_key(&n.cast().ok()?))
                    && !expanding.get()
                {
                    events.emit(id, UiEvent::RowExpanded(key, true));
                }
            }
        })?);
        revokers.push(view.cast::<w::ITreeView>()?.Collapsed({
            let (events, expanding) = (events.clone(), expanding.clone());
            move |_, args| {
                let node = args.as_ref().and_then(|a| a.cast::<w::ITreeViewCollapsedEventArgs>().ok()?.Node().ok());
                if let Some(key) = node.and_then(|n| node_key(&n.cast().ok()?))
                    && !expanding.get()
                {
                    events.emit(id, UiEvent::RowExpanded(key, false));
                }
            }
        })?);

        revokers.push(view.cast::<w::ITreeView3>()?.SelectionChanged({
            let (data, events, view) = (data.clone(), events.clone(), view.clone());
            move |_, _| {
                if data.borrow().muted {
                    return;
                }
                let now = selected(&view, &data.borrow());
                let mut d = data.borrow_mut();
                if d.selection != now {
                    d.selection = now.clone();
                    drop(d);
                    events.emit(id, UiEvent::Changed(EventValue::Rows(now)));
                }
            }
        })?);

        // Double-click: the row under the pointer. Return: the selected
        // row, without Control or Alt, as a list's.
        revokers.push(view.cast::<w::IUIElement>()?.DoubleTapped({
            let (data, events, view) = (data.clone(), events.clone(), view.clone());
            move |_, args| {
                let source = args.as_ref().and_then(|a| a.cast::<w::IRoutedEventArgs>().ok()?.OriginalSource().ok());
                let key = source.and_then(|s| row_of_element(&view, &s));
                if let Some(key) = key.filter(|k| data.borrow().index.contains_key(k)) {
                    events.emit(id, UiEvent::RowActivated(key));
                }
            }
        })?);
        revokers.push(view.cast::<w::IUIElement>()?.PreviewKeyDown({
            let (data, events, view) = (data.clone(), events.clone(), view.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
                let held = |key: i32| unsafe { w::GetKeyState(key) } < 0;
                if args.Key().is_ok_and(|k| k == w::VirtualKey::Enter)
                    && !held(w::VK_CONTROL)
                    && !held(w::VK_MENU)
                    && let Some(key) = selected(&view, &data.borrow()).first()
                {
                    events.emit(id, UiEvent::RowActivated(*key));
                    _ = args.SetHandled(true);
                }
            }
        })?);

        Ok(Tree {
            view,
            id,
            events,
            data,
            nodes,
            expanding,
            inner,
            offset: Rc::default(),
            drag_over,
            drop_rows,
            revokers: RefCell::new(revokers),
        })
    }

    /// The tree view's own list, from its template once it's applied (in
    /// a window, at its first layout), with its containers followed as a
    /// list's. The containers it realised before are taken in as they are.
    fn inner(&self) -> Option<w::ListView> {
        if let Some(inner) = self.inner.borrow().clone() {
            return Some(inner);
        }
        let inner = find_tree_list(&self.view.cast().ok()?)?;
        let (data, events, id) = (self.data.clone(), self.events.clone(), self.id);
        let revoker = inner.cast::<w::IListViewBase>().ok()?.ContainerContentChanging(move |_, args| {
            if let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IContainerContentChangingEventArgs>().ok()) {
                let key = args.Item().ok().as_ref().and_then(node_key);
                // XAML shows each row's check box as the selection mode
                // changes, on the rows realised then; one recycled with it
                // came back with it after the mode had become single (rows
                // moved by a drag, the maintainer, 2026-10-27).
                let multiple = data.borrow().mode == SelectionMode::Multiple;
                if !multiple
                    && !args.InRecycleQueue().unwrap_or(true)
                    && let Ok(container) = args.ItemContainer()
                {
                    _ = w::VisualStateManager::GoToState(&container, "TreeViewMultiSelectDisabled", false);
                }
                container_changed(&data, &events, id, &args, key);
            }
        });
        if let Ok(revoker) = revoker {
            self.revokers.borrow_mut().push(revoker);
        }
        if let Ok(element) = inner.cast::<w::UIElement>()
            && let Ok(revokers) = listen_to_drags(&element, &self.data, &self.drag_over, &self.drop_rows)
        {
            self.revokers.borrow_mut().extend(revokers);
        }
        *self.inner.borrow_mut() = Some(inner.clone());
        self.take_realised(&inner);
        Some(inner)
    }

    /// The containers the list realised before it was followed: each
    /// takes its row's hosts, as when it's realised.
    fn take_realised(&self, inner: &w::ListView) {
        let Some(panel) = inner.cast::<w::IItemsControl>().ok().and_then(|c| c.ItemsPanelRoot().ok()) else { return };
        let Ok(mapping) = inner.cast::<w::IItemContainerMapping>() else { return };
        let Ok(children) = panel.cast::<w::IPanel>().and_then(|p| p.Children()) else { return };
        let mut d = self.data.borrow_mut();
        for container in (0..children.Size().unwrap_or(0)).filter_map(|i| children.GetAt(i).ok()) {
            let Ok(container) = container.cast::<w::DependencyObject>() else { continue };
            let Some(key) = mapping.ItemFromContainer(&container).ok().as_ref().and_then(node_key) else { continue };
            let Some(cell) = container
                .cast::<w::IContentControl>()
                .and_then(|c| c.ContentTemplateRoot())
                .ok()
                .and_then(|root| root.cast::<w::Canvas>().ok())
            else {
                continue;
            };
            let container_key = identity(&container);
            if d.container_rows.contains_key(&container_key) {
                continue;
            }
            let hosts: Vec<w::UIElement> = super::row_hosts(&d, key).cloned().collect();
            for host in hosts {
                put_in(&cell, &host);
            }
            d.cells.insert(key, cell);
            place_row(&d, key);
            d.container_rows.insert(container_key, key);
            *d.bound.entry(key).or_default() += 1;
        }
        queue_report(&self.data, &mut d, &self.events, self.id);
    }

    /// The rows that show, each with its place in the tree. Each node
    /// collection (the top rows', each opening row's children) takes its
    /// nodes by the fewest changes; a row that opens with no children sent
    /// has unrealised ones, so its glyph shows; then, from the top, each
    /// row is opened or closed as sent. A row that moved to another parent
    /// gets a new node: a node is in one collection only.
    pub(crate) fn set_tree_rows(&self, rows: Vec<TreeRow>) -> R<()> {
        let selected = self.data.borrow().selection.clone();
        {
            let mut d = self.data.borrow_mut();
            d.index = rows.iter().enumerate().map(|(i, r)| (r.row, i)).collect();
            d.rows = rows.iter().map(|r| r.row).collect();
            let index = std::mem::take(&mut d.index);
            d.heights.retain(|(k, _), _| index.contains_key(k));
            d.slots.retain(|k, _| index.contains_key(k));
            d.index = index;
        }
        let mut children: HashMap<Option<RowKey>, Vec<w::TreeViewNode>> = HashMap::new();
        let nodes: Vec<(TreeRow, w::TreeViewNode, bool)> = {
            let mut n = self.nodes.borrow_mut();
            let Nodes { by_key, rows: sent, .. } = &mut *n;
            let new: HashMap<RowKey, TreeRow> = rows.iter().map(|r| (r.row, *r)).collect();
            // A node stays while its row and every row above it keep their
            // parents (parents come first): one whose parent's node is new
            // would be in two collections.
            let mut kept = std::collections::HashSet::new();
            for row in &rows {
                if sent.get(&row.row).is_some_and(|old| old.parent == row.parent)
                    && row.parent.is_none_or(|p| kept.contains(&p))
                {
                    kept.insert(row.row);
                }
            }
            by_key.retain(|k, _| kept.contains(k));
            let mut made = Vec::with_capacity(rows.len());
            for row in &rows {
                let node = match by_key.get(&row.row) {
                    Some(node) => node.clone(),
                    None => {
                        let node = w::TreeViewNode::new()?;
                        node.cast::<w::ITreeViewNode>()?.SetContent(&boxed(&row.row.0.to_string()))?;
                        by_key.insert(row.row, node.clone());
                        node
                    }
                };
                children.entry(row.parent).or_default().push(node.clone());
                made.push((*row, node, false));
            }
            *sent = new;
            for (row, _, empty) in &mut made {
                *empty = !children.contains_key(&Some(row.row));
            }
            made
        };
        self.expanding.set(true);
        self.data.borrow_mut().muted = true;
        let changed = (|| -> R<()> {
            let roots = self.view.cast::<w::ITreeView>()?.RootNodes()?;
            sync(&roots, children.get(&None).map_or(&[], Vec::as_slice))?;
            for (row, node, empty) in &nodes {
                let node = node.cast::<w::ITreeViewNode>()?;
                if row.expandable {
                    sync(&node.Children()?, children.get(&Some(row.row)).map_or(&[], Vec::as_slice))?;
                }
                if node.HasUnrealizedChildren()? != (row.expandable && *empty) {
                    node.SetHasUnrealizedChildren(row.expandable && *empty)?;
                }
            }
            for (row, node, _) in &nodes {
                let node = node.cast::<w::ITreeViewNode>()?;
                if node.IsExpanded()? != row.expanded {
                    node.SetIsExpanded(row.expanded)?;
                }
            }
            Ok(())
        })();
        self.data.borrow_mut().muted = false;
        self.expanding.set(false);
        changed?;
        let kept: Vec<RowKey> = {
            let d = self.data.borrow();
            selected.iter().copied().filter(|k| d.index.contains_key(k)).collect()
        };
        self.set_selected(&kept)?;
        if kept != selected {
            self.events.emit(self.id, UiEvent::Changed(EventValue::Rows(kept)));
        }
        Ok(())
    }

    /// The rows the tree view shows, with their parents, depths, and
    /// whether they open and are open.
    pub(crate) fn tree_rows(&self) -> Vec<TreeRow> {
        let mut out = Vec::new();
        if let Ok(roots) = self.view.cast::<w::ITreeView>().and_then(|t| t.RootNodes()) {
            walk(&roots, None, 0, &mut out);
        }
        out
    }

    /// Opens or closes a row as UI Automation's expand-collapse pattern
    /// does, reporting it. False for a row that doesn't open.
    pub(crate) fn expand(&self, key: RowKey, open: bool) -> R<bool> {
        let (node, expandable) = {
            let n = self.nodes.borrow();
            (n.by_key.get(&key).cloned(), n.rows.get(&key).is_some_and(|r| r.expandable))
        };
        let Some(node) = node.filter(|_| expandable) else { return Ok(false) };
        if node.cast::<w::ITreeViewNode>()?.IsExpanded()? != open {
            self.expanding.set(true);
            let view = self.view.cast::<w::ITreeView>()?;
            let done = if open { view.Expand(&node) } else { view.Collapse(&node) };
            self.expanding.set(false);
            done?;
            self.events.emit(self.id, UiEvent::RowExpanded(key, open));
        }
        Ok(true)
    }

    /// XAML's tree view keeps what it can of the selection when the mode
    /// changes: the rows it let go are reported.
    pub(crate) fn set_mode(&self, mode: SelectionMode) -> R<()> {
        self.data.borrow_mut().mode = mode;
        self.data.borrow_mut().muted = true;
        let set = self.view.cast::<w::ITreeView>()?.SetSelectionMode(match mode {
            SelectionMode::None => w::TreeViewSelectionMode::None,
            SelectionMode::Single => w::TreeViewSelectionMode::Single,
            SelectionMode::Multiple => w::TreeViewSelectionMode::Multiple,
        });
        self.data.borrow_mut().muted = false;
        set?;
        let now = self.selected();
        let changed = {
            let mut d = self.data.borrow_mut();
            let changed = d.selection != now;
            d.selection = now.clone();
            changed
        };
        if changed {
            self.events.emit(self.id, UiEvent::Changed(EventValue::Rows(now)));
        }
        Ok(())
    }

    pub(crate) fn mode(&self) -> SelectionMode {
        self.data.borrow().mode
    }

    /// Selects rows without reporting it, through the tree view's selected
    /// node, or nodes.
    pub(crate) fn set_selected(&self, keys: &[RowKey]) -> R<()> {
        let (nodes, mode) = {
            let mut d = self.data.borrow_mut();
            d.selection = keys.to_vec();
            let mut wanted: Vec<RowKey> = keys.iter().filter(|k| d.index.contains_key(k)).copied().collect();
            wanted.sort_by_key(|k| d.index[k]);
            if selected(&self.view, &d) == wanted {
                return Ok(());
            }
            let n = self.nodes.borrow();
            let nodes: Vec<w::TreeViewNode> = wanted.iter().filter_map(|k| n.by_key.get(k).cloned()).collect();
            (nodes, d.mode)
        };
        self.data.borrow_mut().muted = true;
        let result = (|| -> R<()> {
            match mode {
                SelectionMode::None => Ok(()),
                SelectionMode::Single => {
                    let view = self.view.cast::<w::ITreeView2>()?;
                    match nodes.first() {
                        Some(node) => view.SetSelectedNode(node),
                        None => view.SetSelectedNode(None::<&w::TreeViewNode>),
                    }
                }
                SelectionMode::Multiple => {
                    let selected = self.view.cast::<w::ITreeView>()?.SelectedNodes()?;
                    selected.Clear()?;
                    for node in &nodes {
                        selected.Append(node)?;
                    }
                    Ok(())
                }
            }
        })();
        self.data.borrow_mut().muted = false;
        result
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        selected(&self.view, &self.data.borrow())
    }

    /// Selects a row as the user would, reporting it.
    pub(crate) fn select(&self, key: RowKey) -> R<()> {
        self.set_selected(&[key])?;
        self.events.emit(self.id, UiEvent::Changed(EventValue::Rows(vec![key])));
        self.scroll_to_row(key)
    }

    pub(crate) fn activate(&self, key: RowKey) {
        self.events.emit(self.id, UiEvent::RowActivated(key));
    }

    /// A framed tree has a card's border.
    pub(crate) fn set_style(&self, style: ListStyle) -> R<()> {
        let markup = if style.framed() { FRAMED_STYLE } else { PLAIN_STYLE };
        let xaml: w::Style = w::XamlReader::Load(markup)?.cast()?;
        self.view.cast::<w::IFrameworkElement>()?.SetStyle(&xaml)?;
        self.data.borrow_mut().style = Some(style);
        self.report_row_width();
        Ok(())
    }

    pub(crate) fn style(&self) -> Option<ListStyle> {
        self.data.borrow().style
    }

    /// The tree's width: rows get it, less a frame's border, and their
    /// content less its indent.
    pub(crate) fn set_width(&self, width: f32) {
        self.data.borrow_mut().width = Some(width);
        self.report_row_width();
    }

    /// The rows' width, which their content is laid out in less its
    /// indent: where a top row's content ends in its `TreeViewItem`,
    /// which keeps room at its sides (4 at the start in Windows 11's
    /// style), so the tree's width less what that leaves, once a row
    /// shows. Rows laid out as wide as the tree ran past the item's end,
    /// which cut their last points off (SourceGit's branch filters,
    /// 2026-10-27).
    fn report_row_width(&self) {
        let short = self.nodes.borrow().short.unwrap_or(0.0);
        let mut d = self.data.borrow_mut();
        if d.width.is_none() {
            return;
        }
        let row_width = (inside(&d) - short).max(0.0);
        if d.row_width.replace(row_width) != Some(row_width) {
            drop(d);
            self.events.emit(self.id, UiEvent::RowWidth(row_width));
        }
    }

    /// Rows move within the tree: it drags its rows, and takes their drops.
    pub(crate) fn set_moves(&self, moves: RowMoves) -> R<()> {
        self.data.borrow_mut().moves = Some(moves);
        self.view.cast::<w::ITreeView2>()?.SetCanDragItems(true)?;
        self.view.cast::<w::IUIElement>()?.SetAllowDrop(true)
    }

    /// What the app gave, which XAML can't give back.
    pub(crate) fn moves(&self) -> Option<RowMoves> {
        self.data.borrow().moves
    }

    /// A drag of `rows` dropped where the pointer proposed `to`, through
    /// the same targeting as a real drop: for tests, which can't start a
    /// drag.
    pub(crate) fn move_rows(&self, rows: &[RowKey], to: RowDrop<RowKey>) {
        // The pointer is over the row dropped onto or before, as a drag's.
        let over = match to {
            RowDrop::On(row) | RowDrop::Before(row) => Some(row),
            RowDrop::End => None,
        };
        let moved = drop_target(&self.data.borrow(), &self.nodes.borrow().rows, rows, over, to);
        if let Some((rows, to)) = moved {
            self.events.emit(self.id, UiEvent::RowsMoved { rows, to });
        }
    }

    /// Rows with files drag them out, as a list's do.
    pub(crate) fn set_row_files(&self, files: Vec<(RowKey, PathBuf)>) -> R<()> {
        let moves = self.data.borrow().moves.is_some();
        self.view.cast::<w::ITreeView2>()?.SetCanDragItems(moves || !files.is_empty())?;
        let mut data = self.data.borrow_mut();
        data.file_of = files.iter().cloned().collect();
        data.files = Some(files);
        Ok(())
    }

    pub(crate) fn row_files(&self) -> Option<Vec<(RowKey, PathBuf)>> {
        self.data.borrow().files.clone()
    }

    /// What dragging `row` carries: the rows XAML drags (the selection if
    /// the row is in it, else the row), as `DragItemsStarting` has them.
    pub(crate) fn dragged_files(&self, row: RowKey) -> Option<Vec<PathBuf>> {
        let selected = self.selected();
        let rows = if selected.contains(&row) { selected } else { vec![row] };
        let files = files_of(&self.data.borrow(), &rows);
        (!files.is_empty()).then_some(files)
    }

    pub(crate) fn set_estimate(&self, height: f32) {
        self.data.borrow_mut().estimate = Some(height as f64);
    }

    pub(crate) fn estimate(&self) -> Option<f32> {
        self.data.borrow().estimate.map(|h| h as f32)
    }

    /// How high the tree is with all its rows that show.
    pub(crate) fn natural_height(&self) -> f32 {
        self.data.borrow().natural_height() as f32
    }

    /// Hosts a mounted row in its cell if the row is realised.
    pub(crate) fn insert(&self, key: RowKey, id: NodeId, host: w::UIElement) {
        let mut d = self.data.borrow_mut();
        d.hosts.insert((key, 0), (id, host.clone()));
        d.host_columns = d.host_columns.max(1);
        if let Some(cell) = d.cells.get(&key).cloned() {
            put_in(&cell, &host);
            place_row(&d, key);
        }
    }

    pub(crate) fn remove(&self, key: RowKey) {
        let mut d = self.data.borrow_mut();
        if let Some((_, host)) = d.hosts.remove(&(key, 0))
            && let Some(cell) = d.cells.get(&key).cloned()
        {
            take_out(&cell, &host);
            place_row(&d, key);
        }
    }

    /// A host's new height: its cell follows, and without the app's
    /// estimate, the first row measured sets it.
    pub(crate) fn set_row_height(&self, key: RowKey, height: f32) {
        let mut d = self.data.borrow_mut();
        let height = height as f64;
        d.heights.insert((key, 0), height);
        if d.estimate.is_none() && d.learned.is_none() && height > 0.0 {
            d.learned = Some(height);
        }
        place_row(&d, key);
    }

    /// The tree view's scroll viewer, its list's, from its template once
    /// applied.
    pub(crate) fn scroll_viewer(&self) -> Option<w::IScrollViewer> {
        if let Some(scroll) = self.data.borrow().scroll.clone() {
            return Some(scroll);
        }
        let scroll = find_scroll_viewer(&self.view.cast().ok()?)?;
        let (events, last, id) = (self.events.clone(), self.offset.clone(), self.id);
        if let Ok(revoker) = scroll.ViewChanged(move |sender, _| {
            if let Some(scroll) = sender.as_ref().and_then(|s| s.cast::<w::IScrollViewer>().ok()) {
                report_offset(&events, id, &last, &scroll, false);
            }
        }) {
            self.revokers.borrow_mut().push(revoker);
        }
        self.data.borrow_mut().scroll = Some(scroll.clone());
        Some(scroll)
    }

    pub(crate) fn scroll_to(&self, offset: Point) -> R<()> {
        let Some(scroll) = self.scroll_viewer() else { return Ok(()) };
        let element: w::IUIElement = scroll.cast()?;
        element.UpdateLayout()?;
        scroll.ChangeViewWithOptionalAnimation(None, Some(offset.y as f64), None, true)?;
        element.UpdateLayout()?;
        report_offset(&self.events, self.id, &self.offset, &scroll, false);
        Ok(())
    }

    /// Through the tree view's list, whose items are the nodes that show.
    pub(crate) fn scroll_to_row(&self, key: RowKey) -> R<()> {
        let node = self.nodes.borrow().by_key.get(&key).cloned();
        if let (Some(node), Some(inner)) = (node, self.inner()) {
            inner.cast::<w::IListViewBase>()?.ScrollIntoView(&node)?;
            self.view.cast::<w::IUIElement>()?.UpdateLayout()?;
            if let Some(scroll) = self.scroll_viewer() {
                report_offset(&self.events, self.id, &self.offset, &scroll, false);
            }
        }
        Ok(())
    }

    pub(crate) fn scroll_offset(&self) -> Point {
        let Some(scroll) = self.scroll_viewer() else { return Point::ZERO };
        offset_of(&scroll, false)
    }

    /// Lays the view out now, realising the containers of the rows in view
    /// (and, the first time in a window, applying its template), and
    /// reports the rows realised and the indent.
    pub(crate) fn layout(&self) {
        _ = self.view.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
        self.inner();
        self.measure_indent();
        measure_rows(&self.data, &self.events, self.id);
        if let Some(scroll) = self.scroll_viewer() {
            report_offset(&self.events, self.id, &self.offset, &scroll, false);
        }
        report(&self.data, &self.events, self.id);
    }

    /// How far rows' content starts in their containers: past the glyph
    /// column (and the check box, with multiple selection) at the top
    /// level, and a level's indentation more each level down. Where layout
    /// put a top row's cell, and a deeper one's for the indentation, or
    /// `TreeViewItem`'s 16 a level until one shows.
    fn measure_indent(&self) {
        let cells: Vec<(u32, w::Canvas, w::TreeViewNode)> = {
            let d = self.data.borrow();
            let n = self.nodes.borrow();
            d.cells
                .iter()
                .filter_map(|(key, cell)| Some((n.rows.get(key)?.depth, cell.clone(), n.by_key.get(key)?.clone())))
                .collect()
        };
        let Ok(view) = self.view.cast::<w::ITreeView2>() else { return };
        let at = |cell: &w::Canvas, node: &w::TreeViewNode| -> Option<(f32, f32)> {
            let container: w::UIElement = view.ContainerFromNode(node).ok()?.cast().ok()?;
            inset_in(cell, &container)
        };
        let top = cells.iter().filter(|(depth, ..)| *depth == 0).find_map(|(_, cell, node)| at(cell, node));
        let Some((start, width)) = top else { return };
        let below = cells
            .iter()
            .filter(|(depth, ..)| *depth > 0)
            .find_map(|(depth, cell, node)| at(cell, node).map(|(x, _)| (x - start) / *depth as f32));
        let indent = TreeIndent { start, per_level: below.unwrap_or(PER_LEVEL) };
        let inside = inside(&self.data.borrow());
        let short = (inside - (start + width)).max(0.0).round();
        let mut n = self.nodes.borrow_mut();
        let narrower = n.short.replace(short) != Some(short);
        if n.indent != Some(indent) {
            n.indent = Some(indent);
            drop(n);
            self.events.emit(self.id, UiEvent::TreeIndent(indent));
        } else {
            drop(n);
        }
        if narrower {
            self.report_row_width();
        }
    }

    /// Where the tree view put a row: its host's position in the scroll
    /// viewer's content, in by its indent, at the host's size.
    pub(crate) fn row_rect(&self, host: &w::UIElement, size: Rect) -> Rect {
        match self.placed(host) {
            Some((x, y)) => Rect::new(x, y, size.width(), size.height()),
            None => size,
        }
    }

    fn placed(&self, host: &w::UIElement) -> Option<(f32, f32)> {
        let content = self.scroll_viewer()?.cast::<w::IContentControl>().ok()?.Content().ok()?;
        let content: w::UIElement = content.cast().ok()?;
        let point = host.cast::<w::IUIElement>().ok()?.TransformToVisual(&content).ok()?;
        let point = point.cast::<w::IGeneralTransform>().ok()?.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
        Some((point.x, point.y))
    }

    /// The hosted rows, in row order.
    pub(crate) fn children(&self) -> Vec<NodeId> {
        let d = self.data.borrow();
        let mut hosts: Vec<(usize, NodeId)> =
            d.hosts.iter().filter_map(|((key, _), (id, _))| Some((*d.index.get(key)?, *id))).collect();
        hosts.sort();
        hosts.into_iter().map(|(_, id)| id).collect()
    }

    /// Whether the tree view uses a key itself, ahead of a node's keys: a
    /// list's, and Left and Right, which close and open the focused row.
    pub(crate) fn uses(&self, shortcut: mitsuami_core::services::Shortcut) -> bool {
        use mitsuami_core::Key;
        let selects = self.mode() != SelectionMode::None;
        !shortcut.alt
            && match shortcut.key {
                Key::Up | Key::Down | Key::Left | Key::Right | Key::Home | Key::End | Key::PageUp | Key::PageDown => {
                    true
                }
                Key::Char(' ') => selects,
                Key::Char('a') => shortcut.primary && self.mode() == SelectionMode::Multiple,
                Key::Enter => !shortcut.primary && !self.selected().is_empty(),
                _ => false,
            }
    }

    /// Keyboard navigation: a list's, and Right opens the selected row,
    /// Left closes it, as the tree view does.
    pub(crate) fn key(&self, key: mitsuami_core::Key) -> R<bool> {
        use mitsuami_core::Key;
        let rows = self.data.borrow().rows.clone();
        let selected = self.selected();
        if matches!(key, Key::Left | Key::Right) {
            if let Some(row) = selected.first() {
                self.expand(*row, key == Key::Right)?;
            }
            return Ok(true);
        }
        if key == Key::Enter {
            if let Some(row) = selected.first() {
                self.activate(*row);
            }
            return Ok(true);
        }
        let current = selected.first().and_then(|k| rows.iter().position(|r| r == k));
        let last = rows.len().checked_sub(1);
        let next = match (key, current) {
            (Key::Home, _) | (Key::Down, None) => rows.first().map(|_| 0),
            (Key::End, _) | (Key::Up, None) => last,
            (Key::Up, Some(i)) => Some(i.saturating_sub(1)),
            (Key::Down, Some(i)) => Some((i + 1).min(last.unwrap_or(0))),
            _ => return Ok(false),
        };
        if let Some(row) = next.map(|i| rows[i]) {
            self.select(row)?;
        }
        Ok(true)
    }
}

/// The tree's width, less a frame's border.
fn inside(d: &Data) -> f32 {
    let border = if d.style.is_some_and(ListStyle::framed) { 2.0 * FRAME_BORDER } else { 0.0 };
    (d.width.unwrap_or(0.0) - border).max(0.0)
}

/// How far a cell is into its container, and how wide the container makes
/// it: each element's layout slot up to it. One not laid out yet has an
/// empty slot: not measured.
fn inset_in(cell: &w::Canvas, container: &w::UIElement) -> Option<(f32, f32)> {
    let width = w::LayoutInformation::GetLayoutSlot(&cell.cast::<w::FrameworkElement>().ok()?).ok()?.width;
    let (mut element, mut x): (w::DependencyObject, f32) = (cell.cast().ok()?, 0.0);
    loop {
        let slot = w::LayoutInformation::GetLayoutSlot(&element.cast::<w::FrameworkElement>().ok()?).ok()?;
        if slot.width <= 0.0 {
            return None;
        }
        x += slot.x;
        let parent = w::VisualTreeHelper::GetParent(&element).ok()?;
        if parent.cast::<w::UIElement>().ok()? == *container {
            return Some((x.max(0.0).round(), width));
        }
        element = parent;
    }
}

/// The tree view's `TreeViewList`, in its template.
fn find_tree_list(root: &w::DependencyObject) -> Option<w::ListView> {
    let mut queue = std::collections::VecDeque::from([root.clone()]);
    while let Some(node) = queue.pop_front() {
        if node.cast::<w::TreeViewList>().is_ok() {
            return node.cast().ok();
        }
        for i in 0..w::VisualTreeHelper::GetChildrenCount(&node).unwrap_or(0) {
            if let Ok(child) = w::VisualTreeHelper::GetChild(&node, i) {
                queue.push_back(child);
            }
        }
    }
    None
}

/// The selected rows, in row order: the tree view's selected node, or
/// nodes.
fn selected(view: &w::TreeView, d: &Data) -> Vec<RowKey> {
    let key = |node: w::TreeViewNode| node.cast::<IInspectable>().ok().as_ref().and_then(node_key);
    let mut keys: Vec<RowKey> = match d.mode {
        SelectionMode::None => Vec::new(),
        SelectionMode::Single => {
            view.cast::<w::ITreeView2>().and_then(|v| v.SelectedNode()).ok().and_then(key).into_iter().collect()
        }
        SelectionMode::Multiple => {
            let Ok(nodes) = view.cast::<w::ITreeView>().and_then(|v| v.SelectedNodes()) else { return Vec::new() };
            (0..nodes.Size().unwrap_or(0)).filter_map(|i| nodes.GetAt(i).ok()).filter_map(key).collect()
        }
    };
    keys.retain(|k| d.index.contains_key(k));
    keys.sort_by_key(|k| d.index[k]);
    keys
}

/// The row an element is in: up the visual tree to its `TreeViewItem`, and
/// the node the tree view has for it.
fn row_of_element(view: &w::TreeView, element: &IInspectable) -> Option<RowKey> {
    let view = view.cast::<w::ITreeView2>().ok()?;
    let mut current: w::DependencyObject = element.cast().ok()?;
    loop {
        if current.cast::<w::TreeViewItem>().is_ok() {
            let node = view.NodeFromContainer(&current).ok()?;
            return node_key(&node.cast().ok()?);
        }
        current = w::VisualTreeHelper::GetParent(&current).ok()?;
    }
}
