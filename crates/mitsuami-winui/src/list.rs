//! `List`: a XAML `ListView` whose items are the row keys (boxed strings).
//!
//! The list view virtualises its containers: it realises `ListViewItem`s
//! for the rows in view (and its cache) and recycles them as rows scroll
//! away, raising `ContainerContentChanging` both times. We report the rows
//! realised (`RowShown`, `RowHidden`), comparing them with the rows
//! reported once the dispatcher is free (and at the end of each `apply`),
//! so a row whose container is recycled and realised again at once keeps
//! its state. Each container's content is a `Canvas` cell that holds the
//! row's host once the core sends it, and is as high as it, or the
//! estimate until then.
//!
//! XAML raises its events while it lays out, which can be inside our own
//! `apply` (`UpdateLayout`): handlers only touch the list's own data, XAML
//! objects, and emit.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mitsuami_core::{EventValue, NodeId, Point, Rect, RowKey, SelectionMode, UiEvent};
use windows_core::{EventRevoker, IInspectable, IUnknown, Interface};

use crate::backend::{Events, boxed};
use crate::bindings as w;
use crate::later;

type R<T> = windows_core::Result<T>;

/// Rows exactly as high as their cells, with nothing around them.
const ITEM_STYLE: &str = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="ListViewItem">
    <Setter Property="Padding" Value="0"/>
    <Setter Property="Margin" Value="0"/>
    <Setter Property="MinHeight" Value="0"/>
    <Setter Property="HorizontalContentAlignment" Value="Stretch"/>
    <Setter Property="VerticalContentAlignment" Value="Stretch"/>
</Style>"#;

#[derive(Default)]
struct Data {
    rows: Vec<RowKey>,
    /// The objects in the view's `Items`, one per row.
    items: Vec<IInspectable>,
    index: HashMap<RowKey, usize>,
    /// The mounted rows' hosts, and their heights.
    hosts: HashMap<RowKey, (NodeId, w::UIElement)>,
    heights: HashMap<RowKey, f64>,
    /// Each container's cell (by COM identity), and the cells of the rows
    /// realised, with how many containers each is realised in.
    container_cells: HashMap<usize, w::Canvas>,
    cells: HashMap<RowKey, w::Canvas>,
    bound: HashMap<RowKey, usize>,
    /// The rows reported shown, and the selection last reported or set.
    shown: HashSet<RowKey>,
    selection: Vec<RowKey>,
    report_queued: bool,
    estimate: Option<f64>,
    /// Without the app's estimate: the first row measured.
    learned: Option<f64>,
    mode: SelectionMode,
    scroll: Option<w::IScrollViewer>,
}

impl Data {
    fn estimate(&self) -> f64 {
        self.estimate.or(self.learned).unwrap_or(32.0)
    }

    fn height(&self, key: RowKey) -> f64 {
        match self.hosts.contains_key(&key) {
            true => self.heights.get(&key).copied().unwrap_or(0.0),
            false => self.estimate(),
        }
    }
}

pub(crate) struct List {
    pub view: w::ListView,
    id: NodeId,
    events: Events,
    data: Rc<RefCell<Data>>,
    /// The scroll offset last reported.
    pub offset: Rc<Cell<Point>>,
    revokers: RefCell<Vec<EventRevoker>>,
}

fn identity(object: &impl Interface) -> usize {
    object.cast::<IUnknown>().map_or(0, |u| u.as_raw() as usize)
}

fn key_of(item: &IInspectable) -> Option<RowKey> {
    item.cast::<w::IPropertyValue>().ok()?.GetString().ok()?.to_string().parse().ok().map(RowKey)
}

/// Removes an element from the panel it's in, if it's in `panel`.
fn take_out(panel: &w::Canvas, element: &w::UIElement) {
    let Ok(children) = panel.cast::<w::IPanel>().and_then(|p| p.Children()) else { return };
    let mut index = 0;
    if children.IndexOf(element, &mut index).unwrap_or(false) {
        _ = children.RemoveAt(index);
    }
}

fn put_in(panel: &w::Canvas, element: &w::UIElement) {
    if let Ok(children) = panel.cast::<w::IPanel>().and_then(|p| p.Children()) {
        let mut index = 0;
        if !children.IndexOf(element, &mut index).unwrap_or(false) {
            _ = children.Append(element);
        }
    }
}

fn set_height(cell: &w::Canvas, height: f64) {
    if let Ok(fe) = cell.cast::<w::IFrameworkElement>() {
        _ = fe.SetHeight(height);
    }
}

impl List {
    pub(crate) fn new(id: NodeId, events: Events) -> R<List> {
        let view = w::ListView::new()?;
        let style: w::Style = w::XamlReader::Load(ITEM_STYLE)?.cast()?;
        view.cast::<w::IItemsControl>()?.SetItemContainerStyle(&style)?;
        let data = Rc::new(RefCell::new(Data::default()));
        let mut revokers = Vec::new();

        revokers.push(view.cast::<w::IListViewBase>()?.ContainerContentChanging({
            let (data, events) = (data.clone(), events.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IContainerContentChangingEventArgs>().ok())
                else {
                    return;
                };
                _ = args.SetHandled(true);
                let (Ok(container), Some(key)) = (args.ItemContainer(), args.Item().ok().as_ref().and_then(key_of))
                else {
                    return;
                };
                let recycled = args.InRecycleQueue().unwrap_or(false);
                let mut d = data.borrow_mut();
                let cell = match d.container_cells.get(&identity(&container)) {
                    Some(cell) => cell.clone(),
                    None => {
                        let Ok(cell) = w::Canvas::new() else { return };
                        _ = container.cast::<w::IContentControl>().and_then(|c| c.SetContent(&cell));
                        d.container_cells.insert(identity(&container), cell.clone());
                        cell
                    }
                };
                let host = d.hosts.get(&key).map(|(_, host)| host.clone());
                if recycled {
                    if let Some(host) = host {
                        take_out(&cell, &host);
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
                } else {
                    if let Some(host) = &host {
                        if let Some(old) = d.cells.get(&key).filter(|c| **c != cell) {
                            take_out(old, host);
                        }
                        put_in(&cell, host);
                    }
                    set_height(&cell, d.height(key));
                    d.cells.insert(key, cell);
                    *d.bound.entry(key).or_default() += 1;
                }
                queue_report(&data, &mut d, &events, id);
            }
        })?);

        revokers.push(view.cast::<w::ISelector>()?.SelectionChanged({
            let (data, events, view) = (data.clone(), events.clone(), view.clone());
            move |_, _| {
                let now = selected(&view, &data.borrow());
                let mut d = data.borrow_mut();
                if d.selection != now {
                    d.selection = now.clone();
                    drop(d);
                    events.emit(id, UiEvent::Changed(EventValue::Rows(now)));
                }
            }
        })?);

        // Double-click: the row under the pointer. Return: the selected row.
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
                if args.Key().is_ok_and(|k| k == w::VirtualKey::Enter)
                    && let Some(key) = selected(&view, &data.borrow()).first()
                {
                    events.emit(id, UiEvent::RowActivated(*key));
                    _ = args.SetHandled(true);
                }
            }
        })?);

        Ok(List { view, id, events, data, offset: Rc::default(), revokers: RefCell::new(revokers) })
    }

    fn items(&self) -> R<w::ItemCollection> {
        self.view.cast::<w::IItemsControl>()?.Items()
    }

    /// The list view's own scroll viewer, from its template, once applied.
    pub(crate) fn scroll_viewer(&self) -> Option<w::IScrollViewer> {
        if let Some(scroll) = self.data.borrow().scroll.clone() {
            return Some(scroll);
        }
        let scroll = find_scroll_viewer(&self.view.cast().ok()?)?;
        let (events, last, id) = (self.events.clone(), self.offset.clone(), self.id);
        if let Ok(revoker) = scroll.ViewChanged(move |sender, _| {
            if let Some(scroll) = sender.as_ref().and_then(|s| s.cast::<w::IScrollViewer>().ok()) {
                report_offset(&events, id, &last, &scroll);
            }
        }) {
            self.revokers.borrow_mut().push(revoker);
        }
        self.data.borrow_mut().scroll = Some(scroll.clone());
        Some(scroll)
    }

    /// New rows, as the fewest changes to the view's items: rows before and
    /// after the part that changed keep their containers. Selected rows
    /// that stayed stay selected; selected rows that went are reported.
    pub(crate) fn set_rows(&self, rows: Vec<RowKey>) -> R<()> {
        let selected = self.data.borrow().selection.clone();
        let (prefix, removed, added) = {
            let d = self.data.borrow();
            let old = &d.rows;
            let prefix = old.iter().zip(&rows).take_while(|(a, b)| a == b).count();
            let suffix =
                old[prefix..].iter().rev().zip(rows[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
            (prefix, old.len() - prefix - suffix, rows[prefix..rows.len() - suffix].to_vec())
        };
        let new_items: Vec<IInspectable> = added.iter().map(|k| boxed(&k.0.to_string())).collect();
        {
            let mut d = self.data.borrow_mut();
            d.items.splice(prefix..prefix + removed, new_items.iter().cloned());
            d.index = rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
            let index = std::mem::take(&mut d.index);
            d.heights.retain(|k, _| index.contains_key(k));
            d.index = index;
            d.rows = rows;
        }
        let items = self.items()?;
        for _ in 0..removed {
            items.RemoveAt(prefix as u32)?;
        }
        for (i, item) in new_items.iter().enumerate() {
            items.InsertAt((prefix + i) as u32, item)?;
        }
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

    pub(crate) fn set_mode(&self, mode: SelectionMode) -> R<()> {
        self.data.borrow_mut().mode = mode;
        self.view.cast::<w::IListViewBase>()?.SetSelectionMode(match mode {
            SelectionMode::None => w::ListViewSelectionMode::None,
            SelectionMode::Single => w::ListViewSelectionMode::Single,
            SelectionMode::Multiple => w::ListViewSelectionMode::Multiple,
        })
    }

    pub(crate) fn mode(&self) -> SelectionMode {
        self.data.borrow().mode
    }

    /// Selects rows without reporting it: the selection it expects is set
    /// first, so the view's `SelectionChanged` finds it reported.
    pub(crate) fn set_selected(&self, keys: &[RowKey]) -> R<()> {
        let (indexes, items, mode) = {
            let mut d = self.data.borrow_mut();
            d.selection = keys.to_vec();
            let indexes: Vec<usize> = keys.iter().filter_map(|k| d.index.get(k).copied()).collect();
            let items: Vec<IInspectable> = indexes.iter().map(|i| d.items[*i].clone()).collect();
            (indexes, items, d.mode)
        };
        match mode {
            SelectionMode::None => Ok(()),
            SelectionMode::Single => {
                self.view.cast::<w::ISelector>()?.SetSelectedIndex(indexes.first().map_or(-1, |i| *i as i32))
            }
            SelectionMode::Multiple => {
                let selected = self.view.cast::<w::IListViewBase>()?.SelectedItems()?;
                selected.Clear()?;
                for item in &items {
                    selected.Append(item)?;
                }
                Ok(())
            }
        }
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

    pub(crate) fn set_estimate(&self, height: f32) {
        self.data.borrow_mut().estimate = Some(height as f64);
    }

    pub(crate) fn estimate(&self) -> Option<f32> {
        self.data.borrow().estimate.map(|h| h as f32)
    }

    /// Hosts a mounted row, in its cell if the row is realised.
    pub(crate) fn insert(&self, key: RowKey, id: NodeId, host: w::UIElement) {
        let mut d = self.data.borrow_mut();
        if let Some(cell) = d.cells.get(&key).cloned() {
            put_in(&cell, &host);
            set_height(&cell, d.heights.get(&key).copied().unwrap_or(0.0));
        }
        d.hosts.insert(key, (id, host));
    }

    pub(crate) fn remove(&self, key: RowKey) {
        let mut d = self.data.borrow_mut();
        if let Some((_, host)) = d.hosts.remove(&key)
            && let Some(cell) = d.cells.get(&key).cloned()
        {
            take_out(&cell, &host);
            set_height(&cell, d.estimate());
        }
    }

    /// A host's new height: its cell follows, and without the app's
    /// estimate, the first row measured sets it.
    pub(crate) fn set_row_height(&self, key: RowKey, height: f32) {
        let mut d = self.data.borrow_mut();
        let height = height as f64;
        d.heights.insert(key, height);
        if d.estimate.is_none() && d.learned.is_none() && height > 0.0 {
            d.learned = Some(height);
        }
        if let Some(cell) = d.cells.get(&key) {
            set_height(cell, height);
        }
    }

    pub(crate) fn scroll_to(&self, offset: Point) -> R<()> {
        let Some(scroll) = self.scroll_viewer() else { return Ok(()) };
        let element: w::IUIElement = scroll.cast()?;
        element.UpdateLayout()?;
        scroll.ChangeViewWithOptionalAnimation(None, Some(offset.y as f64), None, true)?;
        element.UpdateLayout()?;
        report_offset(&self.events, self.id, &self.offset, &scroll);
        Ok(())
    }

    pub(crate) fn scroll_to_row(&self, key: RowKey) -> R<()> {
        let item = {
            let d = self.data.borrow();
            d.index.get(&key).map(|i| d.items[*i].clone())
        };
        if let Some(item) = item {
            self.view.cast::<w::IListViewBase>()?.ScrollIntoView(&item)?;
            self.view.cast::<w::IUIElement>()?.UpdateLayout()?;
            if let Some(scroll) = self.scroll_viewer() {
                report_offset(&self.events, self.id, &self.offset, &scroll);
            }
        }
        Ok(())
    }

    /// Lays the view out now, realising the containers of the rows in view,
    /// and reports the rows realised. XAML would at its next layout pass.
    pub(crate) fn layout(&self) {
        _ = self.view.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
        if let Some(scroll) = self.scroll_viewer() {
            report_offset(&self.events, self.id, &self.offset, &scroll);
        }
        report(&self.data, &self.events, self.id);
    }

    pub(crate) fn scroll_offset(&self) -> Point {
        let Some(scroll) = self.scroll_viewer() else { return Point::ZERO };
        Point::new(0.0, scroll.VerticalOffset().unwrap_or(0.0) as f32)
    }

    /// Where the view put a row: its host's position in the view, in its
    /// content's coordinates, at the host's size.
    pub(crate) fn row_rect(&self, host: &w::UIElement, size: Rect) -> Rect {
        let placed = (|| {
            let view: w::UIElement = self.view.cast().ok()?;
            let point = host.cast::<w::IUIElement>().ok()?.TransformToVisual(&view).ok()?;
            let point = point.cast::<w::IGeneralTransform>().ok()?.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
            Some(point.y + self.scroll_offset().y)
        })();
        match placed {
            Some(y) => Rect::new(0.0, y, size.width(), size.height()),
            None => size,
        }
    }

    pub(crate) fn rows(&self) -> Vec<RowKey> {
        let count = self.items().and_then(|i| i.Size()).unwrap_or(0) as usize;
        let d = self.data.borrow();
        d.items.iter().take(count).filter_map(key_of).collect()
    }

    /// The hosted rows, in row order.
    pub(crate) fn children(&self) -> Vec<NodeId> {
        let d = self.data.borrow();
        let mut hosts: Vec<(usize, NodeId)> =
            d.hosts.iter().filter_map(|(key, (id, _))| Some((*d.index.get(key)?, *id))).collect();
        hosts.sort();
        hosts.into_iter().map(|(_, id)| id).collect()
    }

    /// Keyboard navigation: what the list view does with arrows, Home and
    /// End (move the selection and show it) and Return (activate).
    pub(crate) fn key(&self, key: mitsuami_core::Key) -> R<bool> {
        use mitsuami_core::Key;
        let rows = self.data.borrow().rows.clone();
        let selected = self.selected();
        let current = selected.first().and_then(|k| rows.iter().position(|r| r == k));
        if key == Key::Enter {
            if let Some(row) = selected.first() {
                self.activate(*row);
            }
            return Ok(true);
        }
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

/// Reports a scroll offset once per change: XAML's `ViewChanged` arrives
/// after the change we made and reported.
fn report_offset(events: &Events, id: NodeId, last: &Cell<Point>, scroll: &w::IScrollViewer) {
    let offset = Point::new(0.0, scroll.VerticalOffset().unwrap_or(0.0) as f32);
    if last.replace(offset) != offset {
        events.emit(id, UiEvent::Scrolled(offset));
    }
}

fn selected(view: &w::ListView, d: &Data) -> Vec<RowKey> {
    match d.mode {
        SelectionMode::None => Vec::new(),
        SelectionMode::Single => {
            let index = view.cast::<w::ISelector>().and_then(|s| s.SelectedIndex()).unwrap_or(-1);
            usize::try_from(index).ok().and_then(|i| d.rows.get(i).copied()).into_iter().collect()
        }
        SelectionMode::Multiple => {
            let Ok(items) = view.cast::<w::IListViewBase>().and_then(|l| l.SelectedItems()) else { return Vec::new() };
            let mut keys: Vec<(usize, RowKey)> = (0..items.Size().unwrap_or(0))
                .filter_map(|i| key_of(&items.GetAt(i).ok()?))
                .filter_map(|k| Some((*d.index.get(&k)?, k)))
                .collect();
            keys.sort();
            keys.into_iter().map(|(_, k)| k).collect()
        }
    }
}

/// The row an element is in: up the visual tree to its container.
fn row_of_element(view: &w::ListView, element: &IInspectable) -> Option<RowKey> {
    let mut current: w::DependencyObject = element.cast().ok()?;
    loop {
        if current.cast::<w::SelectorItem>().is_ok() {
            let item = view.cast::<w::IItemContainerMapping>().ok()?.ItemFromContainer(&current).ok()?;
            return key_of(&item);
        }
        current = w::VisualTreeHelper::GetParent(&current).ok()?;
    }
}

fn find_scroll_viewer(root: &w::DependencyObject) -> Option<w::IScrollViewer> {
    let mut queue = std::collections::VecDeque::from([root.clone()]);
    while let Some(node) = queue.pop_front() {
        if let Ok(scroll) = node.cast::<w::IScrollViewer>() {
            return Some(scroll);
        }
        for i in 0..w::VisualTreeHelper::GetChildrenCount(&node).unwrap_or(0) {
            if let Ok(child) = w::VisualTreeHelper::GetChild(&node, i) {
                queue.push_back(child);
            }
        }
    }
    None
}

/// Reports the rows realised and let go since the last report.
fn report(data: &Rc<RefCell<Data>>, events: &Events, id: NodeId) {
    let mut d = data.borrow_mut();
    d.report_queued = false;
    let now: HashSet<RowKey> = d.bound.keys().copied().collect();
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

/// Reports once the dispatcher is free: a row whose container is recycled
/// and realised again in one layout pass is kept.
fn queue_report(data: &Rc<RefCell<Data>>, d: &mut Data, events: &Events, id: NodeId) {
    if d.report_queued {
        return;
    }
    d.report_queued = true;
    let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() else { return };
    let (data, events) = (data.clone(), events.clone());
    let ticket = later::park(Box::new(move || report(&data, &events, id)) as Box<dyn FnOnce()>);
    later::on_ui(&queue, move || {
        if let Some(report) = later::take::<Box<dyn FnOnce()>>(ticket) {
            report();
        }
    });
}
