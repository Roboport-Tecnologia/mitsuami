//! `List` and `Table`: a XAML `ListView` whose items are the row keys
//! (boxed strings). WinUI has no table, so a table is built as Fluent apps
//! (File Explorer, Files) build one: the list view under a header row of
//! column buttons, each row's cells side by side at the columns' widths.
//!
//! The list view virtualises its containers: it realises `ListViewItem`s
//! for the rows in view (and its cache) and recycles them as rows scroll
//! away, raising `ContainerContentChanging` both times. We report the rows
//! realised (`RowShown`, `RowHidden`), comparing them with the rows
//! reported once the dispatcher is free (and at the end of each `apply`),
//! so a row whose container is recycled and realised again at once keeps
//! its state. Each container's content is a `Canvas` cell (the item
//! template's root) that holds the
//! row's host once the core sends it, and is as high as it, or the
//! estimate until then.
//!
//! XAML raises its events while it lays out, which can be inside our own
//! `apply` (`UpdateLayout`): handlers only touch the list's own data, XAML
//! objects, and emit.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::{
    ColumnData, ColumnSort, EventValue, ListStyle, NodeId, Point, Rect, RowKey, SelectionMode, SortOrder, UiEvent,
};
use windows_core::{EventRevoker, IInspectable, IUnknown, Interface};

use crate::backend::{Events, boxed};
use crate::bindings as w;
use crate::later;

type R<T> = windows_core::Result<T>;

/// Each container shows a `Canvas`, our cell: XAML creates it from the
/// item template (content set on a container directly is replaced by the
/// item when XAML prepares it).
const ITEM_TEMPLATE: &str =
    r#"<DataTemplate xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><Canvas/></DataTemplate>"#;

/// Rows exactly as high as their cells, with nothing around them.
const ITEM_STYLE: &str = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="ListViewItem">
    <Setter Property="Padding" Value="0"/>
    <Setter Property="Margin" Value="0"/>
    <Setter Property="MinHeight" Value="0"/>
    <Setter Property="HorizontalContentAlignment" Value="Stretch"/>
    <Setter Property="VerticalContentAlignment" Value="Stretch"/>
</Style>"#;

/// A framed list: a card's border and background, which follow the theme.
/// Without `BasedOn`, the default style still gives the template (as for
/// `ITEM_STYLE`).
const FRAMED_STYLE: &str = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="ListView">
    <Setter Property="BorderThickness" Value="1"/>
    <Setter Property="BorderBrush" Value="{ThemeResource CardStrokeColorDefaultBrush}"/>
    <Setter Property="Background" Value="{ThemeResource CardBackgroundFillColorDefaultBrush}"/>
    <Setter Property="CornerRadius" Value="{ThemeResource ControlCornerRadius}"/>
</Style>"#;

/// A plain list: the default style's.
const PLAIN_STYLE: &str =
    r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="ListView"/>"#;

/// The framed style's border, on each side.
const FRAME_BORDER: f32 = 1.0;

/// A framed table: the card's border and background around its header and
/// rows, which are in a `Grid`.
const FRAMED_TABLE_STYLE: &str = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="Grid">
    <Setter Property="BorderThickness" Value="1"/>
    <Setter Property="BorderBrush" Value="{ThemeResource CardStrokeColorDefaultBrush}"/>
    <Setter Property="Background" Value="{ThemeResource CardBackgroundFillColorDefaultBrush}"/>
    <Setter Property="CornerRadius" Value="{ThemeResource ControlCornerRadius}"/>
</Style>"#;

const PLAIN_TABLE_STYLE: &str =
    r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="Grid"/>"#;

/// A table: its header above its rows.
const TABLE_ROOT: &str = r#"<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation">
    <Grid.RowDefinitions><RowDefinition Height="32"/><RowDefinition Height="*"/></Grid.RowDefinitions>
</Grid>"#;

/// The header row's height (`TABLE_ROOT`'s first row).
const HEADER_HEIGHT: f64 = 32.0;
/// A table's rows are at least as high as a `ListViewItem`
/// (`ListViewItemMinHeight`), which a list's rows are not held to.
const TABLE_ROW_MIN: f64 = 40.0;
/// Room at each side of a cell's content, as in a column header's title.
const CELL_PADDING: f64 = 12.0;
/// A column without a width, and the narrowest the user can make one.
const COLUMN_WIDTH: f64 = 120.0;
const COLUMN_MIN: f64 = 2.0 * CELL_PADDING + 16.0;
/// The gripper at a header's trailing edge that resizes its column.
const GRIPPER: f64 = 8.0;

/// Glyphs of Segoe Fluent Icons: the sort's chevron, up or down.
const ASCENDING: &str = "\u{E70E}";
const DESCENDING: &str = "\u{E70D}";

/// A column's header: a borderless button with its title and the sort's
/// glyph, as Fluent tables have.
fn header_markup(title: &str) -> String {
    let title = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    format!(
        r#"<Button xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Background="Transparent" BorderThickness="0" CornerRadius="0" Padding="{CELL_PADDING},0,{CELL_PADDING},0" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Center" HorizontalAlignment="Stretch" VerticalAlignment="Stretch">
    <Grid>
        <TextBlock Text="{title}" TextTrimming="CharacterEllipsis" Margin="0,0,16,0" VerticalAlignment="Center"/>
        <FontIcon Glyph="" FontSize="10" HorizontalAlignment="Right" VerticalAlignment="Center"/>
    </Grid>
</Button>"#
    )
}

/// The gripper: transparent, but hit-testable, with the divider line down
/// its middle.
const GRIPPER_MARKUP: &str = r#"<Border xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Background="Transparent">
    <Border Width="1" Margin="0,8,0,8" HorizontalAlignment="Center" Background="{ThemeResource DividerStrokeColorDefaultBrush}"/>
</Border>"#;

/// A cell: its row, and its column (a list's is 0).
type Slot = (RowKey, usize);

/// A table's header, and the column the gripper being dragged resizes.
struct Header {
    canvas: w::Canvas,
    /// Each column's button and the glyph in it, and its gripper.
    columns: Vec<(w::Button, w::FontIcon, w::Border)>,
    revokers: Vec<EventRevoker>,
    /// The column being resized, where the drag started and its width then.
    drag: Option<(usize, f64, f64)>,
}

#[derive(Default)]
struct Data {
    /// A table's header; none for a list.
    header: Option<Header>,
    /// Tables only: the columns as sent, the widths the user gave some,
    /// the columns' widths now (their cells' content's and padding), the
    /// widths last reported, the sort shown, and how far the rows are
    /// scrolled sideways (the header follows).
    columns: Vec<ColumnData>,
    resized: Vec<Option<f64>>,
    widths: Vec<f64>,
    reported: Vec<f32>,
    sort: Option<ColumnSort>,
    sideways: f64,
    rows: Vec<RowKey>,
    /// The objects in the view's `Items`, one per row.
    items: Vec<IInspectable>,
    index: HashMap<RowKey, usize>,
    /// The mounted hosts (a list's rows', a table's cells'), and their
    /// heights.
    hosts: HashMap<Slot, (NodeId, w::UIElement)>,
    heights: HashMap<Slot, f64>,
    /// The cells of the rows realised, with how many containers each is
    /// realised in.
    /// The row each container was last realised for.
    container_rows: HashMap<usize, RowKey>,
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
    /// XAML can't tell `Automatic` from `Plain`.
    style: Option<ListStyle>,
    /// The list's width, and the width last reported for rows.
    width: Option<f32>,
    row_width: Option<f32>,
    scroll: Option<w::IScrollViewer>,
    /// Where a row's content starts in its container: after the check box
    /// `ListViewItem`s show with multiple selection, else 0.
    inset: f64,
    /// Set while the backend changes the items or the selection: XAML
    /// reports selections in between (items removed) that aren't the
    /// user's.
    muted: bool,
    /// The rows' files, as the app gave them (`Prop::RowFiles`), and by row.
    files: Option<Vec<(RowKey, PathBuf)>>,
    file_of: HashMap<RowKey, PathBuf>,
}

impl Data {
    fn estimate(&self) -> f64 {
        self.estimate.or(self.learned).unwrap_or(32.0)
    }

    fn is_table(&self) -> bool {
        self.header.is_some()
    }

    /// A row's cell height: as measured last (a table's highest cell's, and
    /// at least a `ListViewItem`'s height), else the estimate. Never 0
    /// while a new host waits for its height: rows above the view that
    /// shrank and grew back made XAML shift the offset to keep the rows in
    /// view still, a little further down every layout pass.
    fn height(&self, key: RowKey) -> f64 {
        if !self.is_table() {
            return self.heights.get(&(key, 0)).copied().unwrap_or_else(|| self.estimate());
        }
        let cells = (0..self.columns.len()).filter_map(|c| self.heights.get(&(key, c)).copied());
        cells.reduce(f64::max).unwrap_or_else(|| self.estimate()).max(TABLE_ROW_MIN)
    }

    /// Where a table's column starts, in a row.
    fn column_x(&self, column: usize) -> f64 {
        self.widths.iter().take(column).sum()
    }

    /// How wide a table's rows are: its columns', or the list's if wider.
    fn rows_width(&self) -> f64 {
        let columns: f64 = self.widths.iter().sum();
        columns.max(self.available() as f64)
    }

    /// The room the rows' content has: the list's width, less a frame's
    /// border and the check box.
    fn available(&self) -> f32 {
        (self.inside() - self.inset as f32).max(0.0)
    }

    /// The list's width, less a frame's border.
    fn inside(&self) -> f32 {
        let border = if self.style.is_some_and(ListStyle::framed) { 2.0 * FRAME_BORDER } else { 0.0 };
        (self.width.unwrap_or(0.0) - border).max(0.0)
    }
}

/// Sizes a table's row cell, and places its cells' hosts: across at their
/// columns, each centred in the row's height.
fn place_row(d: &Data, key: RowKey) {
    let Some(cell) = d.cells.get(&key) else { return };
    let height = d.height(key);
    set_height(cell, height);
    if !d.is_table() {
        return;
    }
    if let Ok(fe) = cell.cast::<w::IFrameworkElement>() {
        _ = fe.SetWidth(d.rows_width());
    }
    for column in 0..d.columns.len() {
        let Some((_, host)) = d.hosts.get(&(key, column)) else { continue };
        let own = d.heights.get(&(key, column)).copied().unwrap_or(0.0);
        _ = w::Canvas::SetLeft(host, d.column_x(column) + CELL_PADDING);
        _ = w::Canvas::SetTop(host, ((height - own) / 2.0).round().max(0.0));
    }
}

/// Sizes a table's columns: each as wide as the user made it, or else the
/// app, or else the default; the ones that expand share what's left of
/// the table's width. Returns the widths their cells get if they changed.
fn size_columns(d: &mut Data) -> Option<Vec<f32>> {
    let mut widths: Vec<f64> = d
        .columns
        .iter()
        .zip(&d.resized)
        .map(|(c, resized)| resized.or(c.width.map(f64::from)).unwrap_or(COLUMN_WIDTH).max(COLUMN_MIN))
        .collect();
    let expanding: Vec<usize> = (0..widths.len()).filter(|i| d.columns[*i].expand && d.resized[*i].is_none()).collect();
    let left = d.available() as f64 - widths.iter().sum::<f64>();
    if left > 0.0 && !expanding.is_empty() {
        let share = (left / expanding.len() as f64).floor();
        for i in expanding {
            widths[i] += share;
        }
    }
    d.widths = widths;
    let cells: Vec<f32> = d.widths.iter().map(|w| (w - 2.0 * CELL_PADDING).max(0.0) as f32).collect();
    (cells != d.reported).then(|| {
        d.reported = cells.clone();
        cells
    })
}

/// Puts a table's header buttons and grippers over their columns (past
/// the rows' check boxes), moved with the rows' sideways scroll, and shows
/// the sort.
fn place_header(d: &Data) {
    let Some(header) = &d.header else { return };
    for (column, (button, icon, gripper)) in header.columns.iter().enumerate() {
        let (x, width) = (d.inset + d.column_x(column) - d.sideways, d.widths.get(column).copied().unwrap_or(0.0));
        if let Ok(fe) = button.cast::<w::IFrameworkElement>() {
            _ = fe.SetWidth(width);
            _ = fe.SetHeight(HEADER_HEIGHT);
        }
        _ = w::Canvas::SetLeft(button, x);
        if let Ok(fe) = gripper.cast::<w::IFrameworkElement>() {
            _ = fe.SetWidth(GRIPPER);
            _ = fe.SetHeight(HEADER_HEIGHT);
        }
        _ = w::Canvas::SetLeft(gripper, x + width - GRIPPER / 2.0);
        let glyph = match d.sort.filter(|s| s.column == column).map(|s| s.order) {
            Some(SortOrder::Ascending) => ASCENDING,
            Some(SortOrder::Descending) => DESCENDING,
            None => "",
        };
        _ = icon.cast::<w::IFontIcon>().and_then(|i| i.SetGlyph(glyph));
    }
    // The header shows only what's above the rows.
    if let Ok(clip) = w::RectangleGeometry::new() {
        let rect = w::Rect { x: 0.0, y: 0.0, width: d.inside(), height: HEADER_HEIGHT as f32 };
        if clip.cast::<w::IRectangleGeometry>().and_then(|g| g.SetRect(rect)).is_ok() {
            _ = header.canvas.cast::<w::IUIElement>().and_then(|e| e.SetClip(&clip));
        }
    }
}

/// Lays a table out again after its columns changed: the header, every
/// realised row, and the widths reported if they changed.
fn relayout(data: &Rc<RefCell<Data>>, events: &Events, id: NodeId) {
    let widths = {
        let mut d = data.borrow_mut();
        if !d.is_table() {
            return;
        }
        let widths = size_columns(&mut d);
        place_header(&d);
        for key in d.cells.keys().copied().collect::<Vec<_>>() {
            place_row(&d, key);
        }
        widths
    };
    if let Some(widths) = widths {
        events.emit(id, UiEvent::ColumnWidths(widths));
    }
}

/// The user pressed a column's header: sorts as tables do (the same column
/// the other way round, another one ascending), shows it and reports it.
fn press(data: &Rc<RefCell<Data>>, events: &Events, id: NodeId, column: usize) -> bool {
    let sort = {
        let mut d = data.borrow_mut();
        if !d.columns.get(column).is_some_and(|c| c.sortable) {
            return false;
        }
        let order = match d.sort {
            Some(sort) if sort.column == column => sort.order.reversed(),
            _ => SortOrder::Ascending,
        };
        let sort = ColumnSort { column, order };
        d.sort = Some(sort);
        place_header(&d);
        sort
    };
    events.emit(id, UiEvent::Changed(EventValue::Sort(sort)));
    true
}

pub(crate) struct List {
    pub view: w::ListView,
    /// A table's `Grid` of its header and the list view: the node's
    /// element. None for a list, whose element is the view.
    pub root: Option<w::Grid>,
    id: NodeId,
    events: Events,
    data: Rc<RefCell<Data>>,
    /// The scroll offset last reported.
    pub offset: Rc<Cell<Point>>,
    revokers: RefCell<Vec<EventRevoker>>,
}

/// The files dragging these rows carries: XAML's dragged items, which are
/// the selected rows when a selected row is dragged, else that row.
fn files_of(d: &Data, rows: &[RowKey]) -> Vec<PathBuf> {
    rows.iter().filter_map(|row| d.file_of.get(row).cloned()).collect()
}

/// Gives the drop target the dragged files when it asks for them, as
/// storage items, which Explorer and other apps take: loading them is
/// asynchronous, so it happens on a thread of its own, and the request
/// waits (its deferral).
fn provide_files(request: &w::DataProviderRequest, paths: Vec<PathBuf>) {
    let (Ok(deferral), request) = (request.GetDeferral(), request.clone()) else { return };
    std::thread::spawn(move || {
        let com = unsafe { w::CoInitializeEx(std::ptr::null(), w::COINIT_MULTITHREADED as u32) }.is_ok();
        // Interfaces go in a vector as options (their default, null).
        let items: Vec<Option<w::IStorageItem>> = paths
            .iter()
            .filter_map(|path| {
                let name = path.to_string_lossy();
                if path.is_dir() {
                    w::StorageFolder::GetFolderFromPathAsync(&name).and_then(|op| op.join()).ok()?.cast().ok()
                } else {
                    w::StorageFile::GetFileFromPathAsync(&name).and_then(|op| op.join()).ok()?.cast().ok()
                }
            })
            .map(Some)
            .collect();
        let items: windows_collections::IVector<w::IStorageItem> = items.into();
        _ = items.cast::<IInspectable>().and_then(|items| request.SetData(&items));
        _ = deferral.Complete();
        if com {
            unsafe { w::CoUninitialize() };
        }
    });
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
    /// A list, or with `table` a table, whose columns come with
    /// `set_columns`.
    pub(crate) fn new(id: NodeId, events: Events, table: bool) -> R<List> {
        let view = w::ListView::new()?;
        let style: w::Style = w::XamlReader::Load(ITEM_STYLE)?.cast()?;
        view.cast::<w::IItemsControl>()?.SetItemContainerStyle(&style)?;
        let template: w::DataTemplate = w::XamlReader::Load(ITEM_TEMPLATE)?.cast()?;
        view.cast::<w::IItemsControl>()?.SetItemTemplate(&template)?;
        let data = Rc::new(RefCell::new(Data::default()));
        let mut revokers = Vec::new();

        revokers.push(view.cast::<w::IListViewBase>()?.ContainerContentChanging({
            let (data, events) = (data.clone(), events.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IContainerContentChangingEventArgs>().ok())
                else {
                    return;
                };
                let (Ok(container), Some(key)) = (args.ItemContainer(), args.Item().ok().as_ref().and_then(key_of))
                else {
                    return;
                };
                let recycled = args.InRecycleQueue().unwrap_or(false);
                let mut d = data.borrow_mut();
                let Some(cell) = container
                    .cast::<w::IContentControl>()
                    .and_then(|c| c.ContentTemplateRoot())
                    .ok()
                    .and_then(|root| root.cast::<w::Canvas>().ok())
                else {
                    return;
                };
                // XAML may reuse a container without recycling it first:
                // whatever row it showed goes, either way.
                let container_key = identity(&container);
                if let Some(old) = d.container_rows.remove(&container_key)
                    && (recycled || old != key)
                {
                    release(&mut d, old, &cell);
                }
                if !recycled {
                    let hosts: Vec<w::UIElement> =
                        d.hosts.iter().filter(|((row, _), _)| *row == key).map(|(_, (_, host))| host.clone()).collect();
                    for host in hosts {
                        if let Some(old) = d.cells.get(&key).filter(|c| **c != cell) {
                            take_out(old, &host);
                        }
                        put_in(&cell, &host);
                    }
                    d.cells.insert(key, cell);
                    place_row(&d, key);
                    d.container_rows.insert(container_key, key);
                    *d.bound.entry(key).or_default() += 1;
                }
                queue_report(&data, &mut d, &events, id);
            }
        })?);

        revokers.push(view.cast::<w::ISelector>()?.SelectionChanged({
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
        // row, without Control or Alt, which leave it to a node's keys.
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
        // Rows with files (`Prop::RowFiles`) drag them out, as a copy;
        // dragging is turned on once there are some.
        revokers.push(view.cast::<w::IListViewBase>()?.DragItemsStarting({
            let data = data.clone();
            move |_, args| {
                let Some(args) = args.as_ref() else { return };
                let rows: Vec<RowKey> = args
                    .Items()
                    .map(|items| (&items).into_iter().filter_map(|i| key_of(&i)).collect())
                    .unwrap_or_default();
                let paths = files_of(&data.borrow(), &rows);
                let Ok(package) = args.Data() else { return };
                if paths.is_empty() {
                    _ = args.SetCancel(true);
                    return;
                }
                _ = package.SetRequestedOperation(w::DataPackageOperation::Copy);
                let Ok(format) = w::StandardDataFormats::StorageItems() else { return };
                let provider = w::DataProviderHandler::new(move |request| {
                    if let Some(request) = request.as_ref() {
                        provide_files(request, paths.clone());
                    }
                });
                _ = package.SetDataProvider(&format.to_string(), &provider);
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

        let root = if table { Some(Self::table_parts(&view, &data)?) } else { None };
        Ok(List { view, root, id, events, data, offset: Rc::default(), revokers: RefCell::new(revokers) })
    }

    /// A table's `Grid`: its header, an empty canvas until the columns
    /// come, above the list view.
    fn table_parts(view: &w::ListView, data: &Rc<RefCell<Data>>) -> R<w::Grid> {
        let root: w::Grid = w::XamlReader::Load(TABLE_ROOT)?.cast()?;
        let canvas = w::Canvas::new()?;
        let children = root.cast::<w::IPanel>()?.Children()?;
        children.Append(&canvas.cast::<w::UIElement>()?)?;
        children.Append(&view.cast::<w::UIElement>()?)?;
        w::Grid::SetRow(&view.cast::<w::FrameworkElement>()?, 1)?;
        data.borrow_mut().header = Some(Header { canvas, columns: Vec::new(), revokers: Vec::new(), drag: None });
        Ok(root)
    }

    /// A table's columns: a header button each, which sorts by it if it's
    /// sortable, with a gripper at its trailing edge that resizes it.
    pub(crate) fn set_columns(&self, columns: &[ColumnData]) -> R<()> {
        let (id, events) = (self.id, self.events.clone());
        let canvas = {
            let mut d = self.data.borrow_mut();
            let Some(header) = d.header.as_mut() else { return Ok(()) };
            header.revokers.clear();
            header.columns.clear();
            header.drag = None;
            header.canvas.clone()
        };
        let children = canvas.cast::<w::IPanel>()?.Children()?;
        children.Clear()?;
        let mut parts = Vec::new();
        let mut revokers = Vec::new();
        for (column, data) in columns.iter().enumerate() {
            let button: w::Button = w::XamlReader::Load(&header_markup(&data.title))?.cast()?;
            let content: w::Grid = button.cast::<w::IContentControl>()?.Content()?.cast()?;
            let icon: w::FontIcon = content.cast::<w::IPanel>()?.Children()?.GetAt(1)?.cast()?;
            w::AutomationProperties::SetName(&button.cast::<w::UIElement>()?, &data.title)?;
            revokers.push(button.cast::<w::IButtonBase>()?.Click({
                let (data, events) = (self.data.clone(), events.clone());
                move |_, _| {
                    press(&data, &events, id, column);
                }
            })?);
            let gripper: w::Border = w::XamlReader::Load(GRIPPER_MARKUP)?.cast()?;
            revokers.extend(self.resizes(&gripper, column)?);
            children.Append(&button.cast::<w::UIElement>()?)?;
            children.Append(&gripper.cast::<w::UIElement>()?)?;
            parts.push((button, icon, gripper));
        }
        {
            let mut d = self.data.borrow_mut();
            d.columns = columns.to_vec();
            d.resized = vec![None; columns.len()];
            d.sort = d.sort.filter(|s| columns.get(s.column).is_some_and(|c| c.sortable));
            let header = d.header.as_mut().expect("a table");
            header.columns = parts;
            header.revokers = revokers;
        }
        relayout(&self.data, &self.events, self.id);
        Ok(())
    }

    /// Dragging a gripper resizes its column, as the Files app's do: the
    /// column keeps the width the user gave it from then on.
    fn resizes(&self, gripper: &w::Border, column: usize) -> R<Vec<EventRevoker>> {
        let element: w::IUIElement = gripper.cast()?;
        let (id, events) = (self.id, self.events.clone());
        // Where the pointer is along the header, which doesn't move.
        let x = |data: &Rc<RefCell<Data>>, args: &windows_core::Ref<w::PointerRoutedEventArgs>| -> Option<f64> {
            let canvas: w::UIElement = data.borrow().header.as_ref()?.canvas.cast().ok()?;
            let args: w::IPointerRoutedEventArgs = args.as_ref()?.cast().ok()?;
            Some(args.GetCurrentPoint(&canvas).ok()?.cast::<w::IPointerPoint>().ok()?.Position().ok()?.x as f64)
        };
        let pressed = element.PointerPressed({
            let (data, element) = (self.data.clone(), element.clone());
            move |_, args| {
                let Some(start) = x(&data, &args) else { return };
                if let Some(pointer) =
                    args.as_ref().and_then(|a| a.cast::<w::IPointerRoutedEventArgs>().ok()?.Pointer().ok())
                {
                    _ = element.CapturePointer(&pointer);
                }
                let mut d = data.borrow_mut();
                let width = d.widths.get(column).copied().unwrap_or(COLUMN_WIDTH);
                if let Some(header) = d.header.as_mut() {
                    header.drag = Some((column, start, width));
                }
                if let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IPointerRoutedEventArgs>().ok()) {
                    _ = args.SetHandled(true);
                }
            }
        })?;
        let moved = element.PointerMoved({
            let (data, events) = (self.data.clone(), events.clone());
            move |_, args| {
                let Some(now) = x(&data, &args) else { return };
                {
                    let mut d = data.borrow_mut();
                    let Some((column, start, width)) = d.header.as_ref().and_then(|h| h.drag) else { return };
                    let width = (width + now - start).max(COLUMN_MIN);
                    if let Some(resized) = d.resized.get_mut(column) {
                        *resized = Some(width);
                    }
                }
                relayout(&data, &events, id);
            }
        })?;
        let released = element.PointerReleased({
            let data = self.data.clone();
            move |_, _| {
                if let Some(header) = data.borrow_mut().header.as_mut() {
                    header.drag = None;
                }
            }
        })?;
        Ok(vec![pressed, moved, released])
    }

    pub(crate) fn columns(&self) -> Vec<ColumnData> {
        self.data.borrow().columns.clone()
    }

    /// Shows the sort in the header, without reporting it.
    pub(crate) fn set_sort(&self, sort: Option<ColumnSort>) {
        let mut d = self.data.borrow_mut();
        d.sort = sort;
        place_header(&d);
    }

    pub(crate) fn sort(&self) -> Option<ColumnSort> {
        self.data.borrow().sort
    }

    /// Presses a column's header, as a click does.
    pub(crate) fn press_header(&self, column: usize) -> bool {
        press(&self.data, &self.events, self.id, column)
    }

    pub(crate) fn is_table(&self) -> bool {
        self.data.borrow().is_table()
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
        // A table's columns can be wider than it: its rows scroll sideways,
        // and the header with them.
        let table = self.data.borrow().is_table();
        if table {
            _ = scroll.SetHorizontalScrollMode(w::ScrollMode::Enabled);
            _ = scroll.SetHorizontalScrollBarVisibility(w::ScrollBarVisibility::Auto);
        }
        let (events, last, id, data) = (self.events.clone(), self.offset.clone(), self.id, self.data.clone());
        if let Ok(revoker) = scroll.ViewChanged(move |sender, _| {
            if let Some(scroll) = sender.as_ref().and_then(|s| s.cast::<w::IScrollViewer>().ok()) {
                report_offset(&events, id, &last, &scroll, table);
                if table {
                    let mut d = data.borrow_mut();
                    d.sideways = scroll.HorizontalOffset().unwrap_or(0.0);
                    place_header(&d);
                }
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
            d.heights.retain(|(k, _), _| index.contains_key(k));
            d.index = index;
            d.rows = rows;
        }
        let items = self.items()?;
        self.data.borrow_mut().muted = true;
        let spliced = (|| -> R<()> {
            for _ in 0..removed {
                items.RemoveAt(prefix as u32)?;
            }
            for (i, item) in new_items.iter().enumerate() {
                items.InsertAt((prefix + i) as u32, item)?;
            }
            Ok(())
        })();
        self.data.borrow_mut().muted = false;
        spliced?;
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

    /// XAML clears the selection when the mode changes, whichever way:
    /// report the rows it let go.
    pub(crate) fn set_mode(&self, mode: SelectionMode) -> R<()> {
        self.data.borrow_mut().mode = mode;
        self.data.borrow_mut().muted = true;
        let set = self.view.cast::<w::IListViewBase>()?.SetSelectionMode(match mode {
            SelectionMode::None => w::ListViewSelectionMode::None,
            SelectionMode::Single => w::ListViewSelectionMode::Single,
            SelectionMode::Multiple => w::ListViewSelectionMode::Multiple,
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

    /// A framed list has a card's border; a framed table has it around its
    /// header and rows.
    pub(crate) fn set_style(&self, style: ListStyle) -> R<()> {
        match &self.root {
            Some(root) => {
                let markup = if style.framed() { FRAMED_TABLE_STYLE } else { PLAIN_TABLE_STYLE };
                let xaml: w::Style = w::XamlReader::Load(markup)?.cast()?;
                root.cast::<w::IFrameworkElement>()?.SetStyle(&xaml)?;
            }
            None => {
                let markup = if style.framed() { FRAMED_STYLE } else { PLAIN_STYLE };
                let xaml: w::Style = w::XamlReader::Load(markup)?.cast()?;
                self.view.cast::<w::IFrameworkElement>()?.SetStyle(&xaml)?;
            }
        }
        self.data.borrow_mut().style = Some(style);
        self.report_row_width();
        relayout(&self.data, &self.events, self.id);
        Ok(())
    }

    pub(crate) fn style(&self) -> Option<ListStyle> {
        self.data.borrow().style
    }

    /// The list's width: rows get it, less a frame's border.
    /// A table's columns follow its width.
    pub(crate) fn set_width(&self, width: f32) {
        self.data.borrow_mut().width = Some(width);
        self.report_row_width();
        relayout(&self.data, &self.events, self.id);
    }

    /// A table reports its columns' widths instead.
    fn report_row_width(&self) {
        let mut d = self.data.borrow_mut();
        if d.width.is_none() || d.is_table() {
            return;
        }
        let row_width = d.available();
        if d.row_width.replace(row_width) != Some(row_width) {
            drop(d);
            self.events.emit(self.id, UiEvent::RowWidth(row_width));
        }
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
        self.data.borrow_mut().muted = true;
        let result = self.select_natively(mode, &indexes, &items);
        self.data.borrow_mut().muted = false;
        result
    }

    fn select_natively(&self, mode: SelectionMode, indexes: &[usize], items: &[IInspectable]) -> R<()> {
        match mode {
            SelectionMode::None => Ok(()),
            SelectionMode::Single => {
                self.view.cast::<w::ISelector>()?.SetSelectedIndex(indexes.first().map_or(-1, |i| *i as i32))
            }
            SelectionMode::Multiple => {
                let selected = self.view.cast::<w::IListViewBase>()?.SelectedItems()?;
                selected.Clear()?;
                for item in items {
                    selected.Append(item)?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        selected(&self.view, &self.data.borrow())
    }

    pub(crate) fn set_row_files(&self, files: Vec<(RowKey, PathBuf)>) -> R<()> {
        self.view.cast::<w::IListViewBase>()?.SetCanDragItems(!files.is_empty())?;
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

    /// Hosts a mounted row, or a table's cell, in its row's cell if the
    /// row is realised.
    pub(crate) fn insert(&self, key: RowKey, column: usize, id: NodeId, host: w::UIElement) {
        let mut d = self.data.borrow_mut();
        d.hosts.insert((key, column), (id, host.clone()));
        if let Some(cell) = d.cells.get(&key).cloned() {
            put_in(&cell, &host);
            place_row(&d, key);
        }
    }

    pub(crate) fn remove(&self, key: RowKey, column: usize) {
        let mut d = self.data.borrow_mut();
        if let Some((_, host)) = d.hosts.remove(&(key, column))
            && let Some(cell) = d.cells.get(&key).cloned()
        {
            take_out(&cell, &host);
            place_row(&d, key);
        }
    }

    /// A host's new height: its row follows (a table's is as high as its
    /// highest cell, which are centred in it), and without the app's
    /// estimate, the first row measured sets it.
    pub(crate) fn set_row_height(&self, key: RowKey, column: usize, height: f32) {
        let mut d = self.data.borrow_mut();
        let height = height as f64;
        d.heights.insert((key, column), height);
        if d.estimate.is_none() && d.learned.is_none() && height > 0.0 {
            d.learned = Some(height);
        }
        place_row(&d, key);
    }

    pub(crate) fn scroll_to(&self, offset: Point) -> R<()> {
        let Some(scroll) = self.scroll_viewer() else { return Ok(()) };
        let element: w::IUIElement = scroll.cast()?;
        element.UpdateLayout()?;
        let x = self.is_table().then_some(offset.x as f64);
        scroll.ChangeViewWithOptionalAnimation(x, Some(offset.y as f64), None, true)?;
        element.UpdateLayout()?;
        report_offset(&self.events, self.id, &self.offset, &scroll, self.is_table());
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
                report_offset(&self.events, self.id, &self.offset, &scroll, self.is_table());
            }
        }
        Ok(())
    }

    /// Lays the view out now, realising the containers of the rows in view,
    /// and reports the rows realised. XAML would at its next layout pass.
    pub(crate) fn layout(&self) {
        _ = self.view.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
        self.measure_inset();
        if let Some(scroll) = self.scroll_viewer() {
            report_offset(&self.events, self.id, &self.offset, &scroll, self.is_table());
        }
        report(&self.data, &self.events, self.id);
    }

    /// Finds where rows' content starts in their containers, from a row
    /// realised: XAML's `ListViewItem` puts a check box before it with
    /// multiple selection. The header and columns (a list's rows' width)
    /// follow it.
    fn measure_inset(&self) {
        let inset = {
            let d = self.data.borrow();
            let Some((key, cell)) = d.cells.iter().next() else { return };
            let Some(item) = d.index.get(key).and_then(|i| d.items.get(*i)) else { return };
            let measured = (|| -> R<f32> {
                let container = self.view.cast::<w::IItemContainerMapping>()?.ContainerFromItem(item)?;
                let transform = cell.cast::<w::IUIElement>()?.TransformToVisual(&container.cast::<w::UIElement>()?)?;
                Ok(transform.cast::<w::IGeneralTransform>()?.TransformPoint(w::Point { x: 0.0, y: 0.0 })?.x)
            })();
            let Ok(inset) = measured else { return };
            f64::from(inset.max(0.0).round())
        };
        if std::mem::replace(&mut self.data.borrow_mut().inset, inset) != inset {
            self.report_row_width();
            relayout(&self.data, &self.events, self.id);
        }
    }

    pub(crate) fn scroll_offset(&self) -> Point {
        let Some(scroll) = self.scroll_viewer() else { return Point::ZERO };
        offset_of(&scroll, self.is_table())
    }

    /// Where the view put a row: its host's position in the scroll
    /// viewer's content, at the host's size. Measured from the content,
    /// not the view: XAML applies a scroll at its next layout, so right
    /// after one the view's transform and the offset disagree.
    pub(crate) fn row_rect(&self, host: &w::UIElement, size: Rect) -> Rect {
        let placed = self.placed(host);
        match placed {
            Some(y) => Rect::new(0.0, y.1, size.width(), size.height()),
            None => size,
        }
    }

    /// Where the table put a cell's host: its place in the scroll viewer's
    /// content, below the header, at the host's size.
    pub(crate) fn cell_rect(&self, host: &w::UIElement, size: Rect) -> Rect {
        match self.placed(host) {
            Some((x, y)) => Rect::new(x, y + HEADER_HEIGHT as f32, size.width(), size.height()),
            None => size,
        }
    }

    /// A host's position in the scroll viewer's content.
    fn placed(&self, host: &w::UIElement) -> Option<(f32, f32)> {
        let content = self.scroll_viewer()?.cast::<w::IContentControl>().ok()?.Content().ok()?;
        let content: w::UIElement = content.cast().ok()?;
        let point = host.cast::<w::IUIElement>().ok()?.TransformToVisual(&content).ok()?;
        let point = point.cast::<w::IGeneralTransform>().ok()?.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
        Some((point.x, point.y))
    }

    pub(crate) fn rows(&self) -> Vec<RowKey> {
        let count = self.items().and_then(|i| i.Size()).unwrap_or(0) as usize;
        let d = self.data.borrow();
        d.items.iter().take(count).filter_map(key_of).collect()
    }

    /// The hosted rows (a table's cells), in row order, then column order.
    pub(crate) fn children(&self) -> Vec<NodeId> {
        let d = self.data.borrow();
        let mut hosts: Vec<((usize, usize), NodeId)> =
            d.hosts.iter().filter_map(|((key, column), (id, _))| Some(((*d.index.get(key)?, *column), *id))).collect();
        hosts.sort();
        hosts.into_iter().map(|(_, id)| id).collect()
    }

    /// Whether the list view uses a key itself, ahead of a node's keys: it
    /// moves focus and the selection with the arrows, Home, End and the
    /// page keys (with Shift and Control too), selects with Space, and
    /// selects every row with Control+A. Return activates the selected
    /// row (our handler).
    pub(crate) fn uses(&self, shortcut: mitsuami_core::services::Shortcut) -> bool {
        use mitsuami_core::Key;
        let selects = self.mode() != SelectionMode::None;
        !shortcut.alt
            && match shortcut.key {
                Key::Up | Key::Down | Key::Home | Key::End | Key::PageUp | Key::PageDown => true,
                Key::Char(' ') => selects,
                Key::Char('a') => shortcut.primary && self.mode() == SelectionMode::Multiple,
                Key::Enter => !shortcut.primary && !self.selected().is_empty(),
                _ => false,
            }
    }

    /// Keyboard navigation: what the list view does with arrows, Home and
    /// End (move the selection and show it), Space (select) and Return
    /// (activate).
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
        // Space selects the focused row, which is where the selection
        // starts (or the first row) here; a multiple selection toggles it.
        if key == Key::Char(' ') {
            let Some(row) = selected.first().or(rows.first()).copied() else { return Ok(true) };
            let now = match self.mode() {
                SelectionMode::Multiple if selected.contains(&row) => {
                    selected.iter().copied().filter(|k| *k != row).collect()
                }
                SelectionMode::Multiple => [selected.clone(), vec![row]].concat(),
                _ => vec![row],
            };
            self.set_selected(&now)?;
            let now = self.selected();
            if now != selected {
                self.events.emit(self.id, UiEvent::Changed(EventValue::Rows(now)));
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

/// How far a list is scrolled: down, and a table's also sideways.
fn offset_of(scroll: &w::IScrollViewer, sideways: bool) -> Point {
    let x = if sideways { scroll.HorizontalOffset().unwrap_or(0.0) as f32 } else { 0.0 };
    Point::new(x, scroll.VerticalOffset().unwrap_or(0.0) as f32)
}

/// Reports a scroll offset once per change: XAML's `ViewChanged` arrives
/// after the change we made and reported.
fn report_offset(events: &Events, id: NodeId, last: &Cell<Point>, scroll: &w::IScrollViewer, sideways: bool) {
    let offset = offset_of(scroll, sideways);
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

/// A container lets go of a row: its host leaves the cell.
fn release(d: &mut Data, key: RowKey, cell: &w::Canvas) {
    for ((row, _), (_, host)) in &d.hosts {
        if *row == key {
            take_out(cell, host);
        }
    }
    if d.cells.get(&key) == Some(cell) {
        d.cells.remove(&key);
    }
    if let Some(count) = d.bound.get_mut(&key) {
        *count -= 1;
        if *count == 0 {
            d.bound.remove(&key);
        }
    }
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
