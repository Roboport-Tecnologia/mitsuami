//! Rows dragged within their list (`List::on_move`): the platform's own row
//! drag, with the drop put where the list takes it (`RowMoves`). Nothing
//! here can start a drag: the test kit drops through each backend's own
//! drop handling (`drag_onto`, `drag_before`, `drag_to_end`).

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn item(name: &str) -> Query {
    by_role(Role::ListItem, name)
}

const NAMES: [&str; 4] = ["Work", "mitsuami", "sourcegit", "bemtvi"];

/// A list of `names` taking `moves`, recording each move as "rows where".
fn list(names: [&'static str; 4], moves: RowMoves, mode: SelectionMode, heard: Signal<Vec<String>>) -> impl View {
    selecting(names, moves, mode, heard, signal(Vec::new()))
}

fn selecting(
    names: [&'static str; 4],
    moves: RowMoves,
    mode: SelectionMode,
    heard: Signal<Vec<String>>,
    selected: Signal<Vec<&'static str>>,
) -> impl View {
    let (id, names) = (names[0], names.to_vec());
    List::new(names, |name: &&'static str| *name, Text::new)
        .selected(selected)
        .selection_mode(mode)
        .on_move(moves, move |rows, to| {
            let to = match to {
                RowDrop::On(row) => format!("onto {row}"),
                RowDrop::Before(row) => format!("before {row}"),
                RowDrop::End => "at the end".to_string(),
            };
            heard.update(|heard| heard.push(format!("{} {to}", rows.join(","))));
        })
        .test_id(id)
        .height(300.0)
}

#[mitsuami_test::test]
async fn a_row_dropped_onto_another_moves_into_it(app: TestApp) {
    let heard = signal(Vec::new());
    app.mount(move || list(NAMES, RowMoves::onto_rows(), SelectionMode::Single, heard));
    app.expect(item("bemtvi")).to_be_visible().await;

    app.get(item("mitsuami")).drag_onto(&app.get(item("Work"))).await;
    app.get(item("bemtvi")).drag_to_end().await;

    assert_eq!(heard.get_untracked(), ["mitsuami onto Work", "bemtvi at the end"]);
    let list = app.get(by_test_id("Work")).native_state();
    assert!(list.props.contains(&Prop::RowMoves(RowMoves::onto_rows())));
}

/// As every platform's list drags: a selected row carries the selection,
/// in the list's order.
#[mitsuami_test::test]
async fn a_selected_row_carries_the_selection(app: TestApp) {
    let (heard, selected) = (signal(Vec::new()), signal(Vec::new()));
    app.mount(move || selecting(NAMES, RowMoves::onto_rows(), SelectionMode::Multiple, heard, selected));
    app.expect(item("bemtvi")).to_be_visible().await;
    // The test kit selects one row at a time: the app selects these.
    selected.set(vec!["bemtvi", "mitsuami"]);

    app.get(item("bemtvi")).drag_onto(&app.get(item("Work"))).await;
    // A row that isn't selected goes alone.
    app.get(item("sourcegit")).drag_onto(&app.get(item("Work"))).await;

    assert_eq!(heard.get_untracked(), ["mitsuami,bemtvi onto Work", "sourcegit onto Work"]);
}

/// A list that only reorders takes a drop onto a row as one before it, and
/// one that only groups takes a drop between rows as one onto the row there.
#[mitsuami_test::test]
async fn drops_land_where_the_list_takes_them(app: TestApp) {
    let (between, onto) = (signal(Vec::new()), signal(Vec::new()));
    let others = ["Home", "nxvim", "baleforgit", "2ksbox"];
    app.mount(move || {
        Row::new().children((
            Column::new().grow(1.0).child(list(NAMES, RowMoves::between_rows(), SelectionMode::Single, between)),
            Column::new().grow(1.0).child(list(others, RowMoves::onto_rows(), SelectionMode::Single, onto)),
        ))
    });
    app.expect(item("2ksbox")).to_be_visible().await;

    app.get(item("bemtvi")).drag_onto(&app.get(item("Work"))).await;
    app.get(item("Work")).drag_to_end().await;
    app.get(item("2ksbox")).drag_before(&app.get(item("Home"))).await;

    assert_eq!(between.get_untracked(), ["bemtvi before Work", "Work at the end"]);
    assert_eq!(onto.get_untracked(), ["2ksbox onto Home"]);
}

#[mitsuami_test::test]
async fn a_row_is_not_dropped_onto_itself(app: TestApp) {
    let heard = signal(Vec::new());
    app.mount(move || list(NAMES, RowMoves::onto_rows().and_between_rows(), SelectionMode::Single, heard));
    app.expect(item("Work")).to_be_visible().await;

    app.get(item("Work")).drag_onto(&app.get(item("Work"))).await;
    app.get(item("Work")).drag_before(&app.get(item("sourcegit"))).await;

    assert_eq!(heard.get_untracked(), ["Work before sourcegit"]);
}

mitsuami_test::main!();
