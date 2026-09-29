//! `Table`: a list whose rows are cells under column headers. The rows the
//! platform shows are mounted, a cell per column; cells are laid out at
//! their columns' widths, and centred in rows as high as their highest;
//! pressing a header sorts; selection, activation and keys are a list's.
//!
//! Where cells go, how wide columns start without a width, how high the
//! header and the smallest rows are, and the room between cells are each
//! platform's: the suite checks what every platform does (cells side by
//! side in column order, a row's cells in one line, an expanding column
//! taking the room left), not numbers.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, ColumnSort, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, PartialEq, Debug)]
struct File {
    id: u32,
    name: String,
    size: u32,
}

fn files(n: u32) -> Vec<File> {
    (0..n).map(|id| File { id, name: format!("File {id}"), size: (id * 37) % 100 }).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum By {
    Name,
    Size,
}

fn name_column() -> TableColumn<File> {
    TableColumn::new("Name", |f: File| Text::new(f.name)).expand().sort_key(By::Name)
}

fn size_column() -> TableColumn<File> {
    TableColumn::new("Size", |f: File| Text::new(format!("{} KB", f.size))).width(80).sort_key(By::Size)
}

/// Name, which expands, and Size, 80 wide, sorted by name.
fn table(data: Signal<Vec<File>>, sort: Signal<Sort<By>>) -> Table<File, u32> {
    Table::new(data, |f: &File| f.id)
        .column(name_column())
        .column(size_column())
        .column(TableColumn::new("Kind", |_: File| Text::new("Document")).width(100))
        .sort(sort)
        .width(400)
        .height(200)
        .test_id("table")
}

fn by_name() -> Signal<Sort<By>> {
    signal(Sort::ascending(By::Name))
}

fn row_names(app: &TestApp) -> Vec<String> {
    app.a11y_tree().walk().into_iter().filter(|n| n.role == Role::Row).filter_map(|n| n.name.clone()).collect()
}

fn cell<'a>(app: &'a TestApp, name: &str) -> Locator<'a> {
    app.get_by_role(Role::Cell, name)
}

#[mitsuami_test::test]
async fn shows_headers_then_rows_of_cells(app: TestApp) {
    let data = signal(files(3));
    app.mount(move || table(data, by_name()));
    app.settle().await;

    let tree = app.a11y_tree();
    let table = tree.walk().into_iter().find(|n| n.role == Role::Table).expect("a table").clone();
    let headers: Vec<_> =
        table.children.iter().filter(|n| n.role == Role::ColumnHeader).filter_map(|n| n.name.clone()).collect();
    assert_eq!(headers, ["Name", "Size", "Kind"]);
    assert_eq!(row_names(&app), ["File 0 0 KB Document", "File 1 37 KB Document", "File 2 74 KB Document"]);

    // A row's cells side by side, in column order, in one line.
    let (name, size) = (cell(&app, "File 1").frame(), cell(&app, "37 KB").frame());
    let kind = app.get_by_role(Role::Row, "File 1 37 KB Document").node().children[2].frame;
    assert!(name.max_x() <= size.x() && size.max_x() <= kind.x(), "{name:?} {size:?} {kind:?}");
    let middle = |r: Rect| r.y() + r.height() / 2.0;
    assert!((middle(name) - middle(size)).abs() <= 1.0 && (middle(size) - middle(kind)).abs() <= 1.0);
    // Rows below one another, below the header.
    let (first, second) = (cell(&app, "File 0").frame(), cell(&app, "File 1").frame());
    let top = app.get_by_test_id("table").frame().y();
    assert!(first.y() > top && second.y() > first.max_y() - 1.0, "{first:?} {second:?}");

    let native = app.get_by_test_id("table").native_state();
    assert!(native.props.iter().any(|p| matches!(p, Prop::Columns(c) if c.len() == 3)));
}

#[mitsuami_test::test]
async fn only_the_rows_near_the_viewport_are_mounted(app: TestApp) {
    let data = signal(files(1000));
    app.mount(move || table(data, by_name()));
    app.settle().await;

    let rows = row_names(&app);
    assert!(rows.len() >= 3 && rows.len() < 250, "{} rows mounted", rows.len());
    assert!(rows[0].starts_with("File 0 "));
    assert!(!rows.iter().any(|r| r.starts_with("File 500 ")));

    app.get_by_test_id("table").scroll_by(0.0, 10_000.0).await;
    assert!(!row_names(&app).iter().any(|r| r.starts_with("File 0 ")) || row_names(&app).len() > 200);
    assert!(row_names(&app).iter().any(|r| r.split(' ').nth(1).and_then(|n| n.parse::<u32>().ok()) > Some(200)));
}

/// Cells are as wide as their column's cells, as the platform reports
/// them: a fixed column keeps the width the app gave it (less the room
/// between cells, where the platform takes it from the column), and the
/// expanding one takes the room left, so it grows with the table.
#[mitsuami_test::test]
async fn cells_are_as_wide_as_their_columns(app: TestApp) {
    let data = signal(files(3));
    // As wide as the window.
    app.mount(move || Column::new().align(Align::Stretch).child(table(data, by_name()).width(Length::Auto)));
    app.resize(400.0, 300.0).await;

    let (name, size) = (cell(&app, "File 0").frame(), cell(&app, "0 KB").frame());
    assert!(size.width() <= 80.0 && size.width() >= 60.0, "size column: {size:?}");
    assert!(name.width() > 100.0, "the name column expands: {name:?}");
    // Every cell of a column is as wide.
    assert_eq!(cell(&app, "File 2").frame().width(), name.width());

    app.resize(600.0, 300.0).await;
    let (wider, same) = (cell(&app, "File 0").frame(), cell(&app, "0 KB").frame());
    assert!(wider.width() >= name.width() + 150.0, "{name:?} then {wider:?}");
    assert_eq!(same.width(), size.width());
}

/// A row is as high as its highest cell, and its other cells are centred
/// in it; rows are never lower than the platform's.
#[mitsuami_test::test]
async fn a_row_is_as_high_as_its_highest_cell(app: TestApp) {
    let data = signal(files(3));
    app.mount(move || {
        Table::new(data, |f: &File| f.id)
            .column(TableColumn::new("Name", |f: File| Text::new(f.name)).expand())
            .column(TableColumn::new("Preview", |f: File| {
                Container::new().height(if f.id == 1 { 60 } else { 10 }).a11y_label(format!("Preview {}", f.id))
            }))
            .height(300)
            .test_id("table")
    });
    app.settle().await;

    let tall = app.get_by_role(Role::Group, "Preview 1").frame();
    assert_eq!(tall.height(), 60.0);
    let name = cell(&app, "File 1").frame();
    let middle = |r: Rect| r.y() + r.height() / 2.0;
    assert!((middle(name) - middle(tall)).abs() <= 1.0, "{name:?} isn't centred beside {tall:?}");
    // The rows after it are below it.
    assert!(cell(&app, "File 2").frame().y() >= tall.max_y());
    // A short row is at least as high as its text.
    let (first, second) = (cell(&app, "File 0").frame(), cell(&app, "File 1").frame());
    assert!(second.y() - first.y() >= first.height());
}

/// Pressing a sortable column's header sorts by it, ascending, and again
/// the other way round; the app's data follows, and the header shows it.
#[mitsuami_test::test]
async fn pressing_a_header_sorts(app: TestApp) {
    let sort = by_name();
    let data = signal(files(5));
    let sorted = move || {
        let mut files = data.get();
        let Sort { by, order } = sort.get();
        match by {
            By::Name => files.sort_by(|a, b| a.name.cmp(&b.name)),
            By::Size => files.sort_by_key(|f| f.size),
        }
        if order == SortOrder::Descending {
            files.reverse();
        }
        files
    };
    app.mount(move || {
        Table::new(sorted, |f: &File| f.id)
            .column(name_column())
            .column(size_column())
            .sort(sort)
            .height(300)
            .test_id("table")
    });
    app.settle().await;
    let header = |title: &str| app.get_by_role(Role::ColumnHeader, title);
    assert_eq!(header("Name").node().value.as_deref(), Some("Ascending"));
    assert_eq!(header("Size").node().value, None);

    header("Size").click().await;
    assert_eq!(sort.get(), Sort::ascending(By::Size));
    assert_eq!(header("Size").node().value.as_deref(), Some("Ascending"));
    assert_eq!(header("Name").node().value, None);
    let sizes: Vec<String> = row_names(&app).iter().map(|r| r.split(' ').nth(2).unwrap().to_owned()).collect();
    assert_eq!(sizes, ["0", "11", "37", "48", "74"]);

    header("Size").click().await;
    assert_eq!(sort.get(), Sort::descending(By::Size));
    assert!(row_names(&app)[0].starts_with("File 2 74"), "{:?}", row_names(&app));

    let table = app.get_by_test_id("table");
    let shown = |sort: ColumnSort| table.native_state().props.contains(&Prop::Sort(Some(sort)));
    assert!(shown(ColumnSort { column: 1, order: SortOrder::Descending }));

    // The app sorts: the header follows.
    sort.set(Sort::descending(By::Name));
    app.settle().await;
    assert!(shown(ColumnSort { column: 0, order: SortOrder::Descending }));
    assert_eq!(header("Name").node().value.as_deref(), Some("Descending"));
}

#[mitsuami_test::test]
async fn a_column_without_a_sort_key_doesnt_sort(app: TestApp) {
    let data = signal(files(3));
    let sort = by_name();
    app.mount(move || table(data, sort));
    app.settle().await;
    let table = app.get_by_test_id("table").id();
    assert_eq!(app.ui().perform(table, &A11yAction::PressHeader(2)), Err(ActionError::Unsupported));
    assert_eq!(sort.get(), Sort::ascending(By::Name));
}

#[mitsuami_test::test]
async fn the_selection_binds_both_ways(app: TestApp) {
    let data = signal(files(10));
    let selected = signal(Vec::<u32>::new());
    app.mount(move || table(data, by_name()).selected(selected));

    // A row is selected through any of its cells.
    cell(&app, "37 KB").select().await;
    assert_eq!(selected.get(), [1]);
    assert_eq!(app.get_by_role(Role::Row, "File 1 37 KB Document").node().selected, Some(true));

    selected.set(vec![3]);
    app.settle().await;
    assert_eq!(app.get_by_role(Role::Row, "File 3 11 KB Document").node().selected, Some(true));
    assert_eq!(app.get_by_role(Role::Row, "File 1 37 KB Document").node().selected, Some(false));
}

#[mitsuami_test::test]
async fn activating_a_row_reports_its_key(app: TestApp) {
    let data = signal(files(10));
    let activated = Rc::new(RefCell::new(Vec::new()));
    let a = activated.clone();
    app.mount(move || table(data, by_name()).on_activate(move |id| a.borrow_mut().push(id)));

    cell(&app, "File 4").click().await;
    assert_eq!(*activated.borrow(), [4]);
}

#[mitsuami_test::test]
async fn the_keyboard_moves_the_selection(app: TestApp) {
    let data = signal(files(10));
    let selected = signal(Vec::<u32>::new());
    app.mount(move || table(data, by_name()).selected(selected));
    let table = app.get_by_test_id("table");

    table.press(Key::Down).await;
    table.press(Key::Down).await;
    assert_eq!(selected.get(), [1]);
    assert!(table.is_focused());
}

/// Reordering the data (sorting it) keeps each row's cells and their
/// state.
#[mitsuami_test::test]
async fn cells_keep_their_state_when_rows_move(app: TestApp) {
    let data = signal(files(3));
    app.mount(move || {
        Table::new(data, |f: &File| f.id)
            .column(TableColumn::new("Done", |f: File| {
                let done = signal(false);
                Checkbox::new(f.name).bind(done)
            }))
            .height(200)
    });
    app.get_by_role(Role::Checkbox, "File 1").click().await;
    data.update(|files| files.reverse());
    app.settle().await;
    assert!(app.get_by_role(Role::Checkbox, "File 1").is_checked());
    let y = |name: &str| app.get_by_role(Role::Checkbox, name).frame().y();
    assert!(y("File 2") < y("File 1") && y("File 1") < y("File 0"));
}

#[mitsuami_test::test]
async fn tables_are_tags(app: TestApp) {
    let data = signal(files(2));
    let sort = by_name();
    app.mount(move || {
        view! {
            <Table each=data key=|f: &File| f.id column=name_column() column=size_column() sort=sort height=200/>
        }
    });
    app.settle().await;
    assert_eq!(row_names(&app), ["File 0 0 KB", "File 1 37 KB"]);
}

/// Multiple selection leaves the columns the table's width: WinUI's rows
/// show a check box before their cells, and the columns (with the header)
/// start after it.
#[mitsuami_test::test]
async fn multiple_selection_keeps_the_columns_in_the_table(app: TestApp) {
    let data = signal(files(3));
    let mode = signal(SelectionMode::Single);
    app.mount(move || table(data, by_name()).selection_mode(mode));
    app.settle().await;
    let kind = || app.get_by_role(Role::Row, "File 0 0 KB Document").node().children[2].frame;
    let (table, single) = (app.get_by_test_id("table").frame(), kind());

    mode.set(SelectionMode::Multiple);
    app.settle().await;
    let multiple = kind();
    assert!(multiple.max_x() <= table.max_x(), "{multiple:?} in {table:?}");
    assert_eq!(multiple.width(), single.width());
}

mitsuami_test::main!();
