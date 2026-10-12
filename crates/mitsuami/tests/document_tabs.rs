//! `DocumentTabs`: tabs without pages, for the documents the app has open.
//! The platform draws the tabs (in the title bar on macOS and Windows, a
//! strip on GNOME and KDE); the user chooses, closes, moves and asks
//! for tabs where the app lets them, and the app decides.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct Doc {
    id: u32,
    name: String,
}

fn docs(names: &[&str]) -> Signal<Vec<Doc>> {
    signal(names.iter().enumerate().map(|(i, n)| Doc { id: i as u32 + 1, name: n.to_string() }).collect())
}

/// A tab for each document, choosing by id.
fn tabs(docs: Signal<Vec<Doc>>, current: Signal<u32>) -> DocumentTabs<u32> {
    DocumentTabs::new(current).children_with(move || {
        docs.with(|d| d.iter().map(|d| DocumentTab::new(d.name.clone(), d.id)).collect::<Vec<_>>())
    })
}

/// The tabs, as `title` lines, the chosen one starred.
fn strip(app: &TestApp) -> Vec<String> {
    let tree = app.a11y_tree();
    tree.walk()
        .into_iter()
        .filter(|n| n.role == Role::Tab)
        .map(|n| format!("{}{}", n.name.clone().unwrap_or_default(), if n.selected == Some(true) { " *" } else { "" }))
        .collect()
}

fn new_tab_button(app: &TestApp) -> bool {
    let tree = app.a11y_tree();
    let list = tree.walk().into_iter().find(|n| n.role == Role::TabList).expect("a tab list").clone();
    list.children.iter().any(|c| c.role == Role::Button)
}

#[mitsuami_test::test]
async fn it_reads_as_a_tab_list(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt", "todo.md", "log.txt"]), signal(2));
    app.mount(move || tabs(docs, current));

    assert_eq!(strip(&app), ["notes.txt", "todo.md *", "log.txt"]);
    let tree = app.a11y_tree();
    let list = tree.walk().into_iter().find(|n| n.role == Role::TabList).expect("a tab list").clone();
    assert!(list.children.iter().all(|c| c.role == Role::Tab), "no button without on_new_tab");
}

#[mitsuami_test::test]
async fn choosing_a_tab_sets_the_selection(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt", "todo.md", "log.txt"]), signal(1));
    app.mount(move || tabs(docs, current));

    app.get_by_role(Role::Tab, "log.txt").click().await;
    assert_eq!(current.get(), 3);
    assert_eq!(strip(&app), ["notes.txt", "todo.md", "log.txt *"]);
}

#[mitsuami_test::test]
async fn setting_the_selection_chooses_its_tab(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt", "todo.md"]), signal(1));
    app.mount(move || tabs(docs, current));

    current.set(2);
    app.settle().await;
    assert_eq!(strip(&app), ["notes.txt", "todo.md *"]);
}

/// One is always chosen while there are tabs, as on every platform, and
/// the app hears which.
#[mitsuami_test::test]
async fn a_value_no_tab_has_chooses_the_first(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt", "todo.md"]), signal(9));
    app.mount(move || tabs(docs, current));

    assert_eq!(strip(&app), ["notes.txt *", "todo.md"]);
    assert_eq!(current.get(), 1);
}

#[mitsuami_test::test]
async fn titles_follow_the_documents(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt", "todo.md"]), signal(2));
    app.mount(move || tabs(docs, current));

    docs.update(|d| d[1].name = "todo (edited).md".into());
    docs.update(|d| d.push(Doc { id: 3, name: "new.txt".into() }));
    app.settle().await;
    assert_eq!(strip(&app), ["notes.txt", "todo (edited).md *", "new.txt"]);
}

/// A close only asks: the tab stays until the app takes its document away,
/// so it can ask about unsaved changes first.
#[mitsuami_test::test]
async fn closing_asks_the_app(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt", "todo.md"]), signal(1));
    let asked = signal(Vec::<u32>::new());
    app.mount(move || tabs(docs, current).on_close(move |id| asked.update(|a| a.push(id))));

    app.get_by_role(Role::Tab, "todo.md").close().await;
    assert_eq!(asked.get(), [2]);
    assert_eq!(strip(&app), ["notes.txt *", "todo.md"]);
}

/// When the chosen tab goes, the one now in its place is chosen, as
/// browsers and every platform's tab views do; the last one's, the one
/// before it.
#[mitsuami_test::test]
async fn the_tab_in_a_closed_ones_place_is_chosen(app: TestApp) {
    let (docs, current) = (docs(&["a.txt", "b.txt", "c.txt"]), signal(2));
    app.mount(move || tabs(docs, current).on_close(move |id| docs.update(|d| d.retain(|d| d.id != id))));

    app.get_by_role(Role::Tab, "b.txt").close().await;
    assert_eq!(strip(&app), ["a.txt", "c.txt *"]);
    assert_eq!(current.get(), 3);
    app.get_by_role(Role::Tab, "c.txt").close().await;
    assert_eq!(strip(&app), ["a.txt *"]);
    assert_eq!(current.get(), 1);
    // A tab that isn't chosen leaves the choice as it is.
    docs.update(|d| d.push(Doc { id: 4, name: "d.txt".into() }));
    app.settle().await;
    app.get_by_role(Role::Tab, "d.txt").close().await;
    assert_eq!(strip(&app), ["a.txt *"]);
}

/// The strip shows a moved tab in its new place, still chosen, and the app
/// hears where it went.
#[mitsuami_test::test]
async fn a_moved_tab_stays_chosen(app: TestApp) {
    let (docs, current) = (docs(&["a.txt", "b.txt", "c.txt"]), signal(1));
    let moves = signal(Vec::<(u32, usize)>::new());
    app.mount(move || {
        tabs(docs, current).on_reorder(move |id, to| {
            moves.update(|m| m.push((id, to)));
            docs.update(|d| {
                let from = d.iter().position(|d| d.id == id).unwrap();
                let doc = d.remove(from);
                d.insert(to, doc);
            });
        })
    });

    app.get_by_role(Role::Tab, "a.txt").move_to(2).await;
    assert_eq!(moves.get(), [(1, 2)]);
    assert_eq!(strip(&app), ["b.txt", "c.txt", "a.txt *"]);
    assert_eq!(current.get(), 1);
    // Another one moved past the chosen one.
    app.get_by_role(Role::Tab, "c.txt").move_to(0).await;
    assert_eq!(strip(&app), ["c.txt", "b.txt", "a.txt *"]);
    app.get_by_role(Role::Tab, "b.txt").click().await;
    assert_eq!(current.get(), 2);
}

#[mitsuami_test::test]
async fn the_new_tab_button_asks_for_a_tab(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt"]), signal(1));
    app.mount(move || {
        tabs(docs, current).on_new_tab(move || {
            let id = docs.with(|d| d.len() as u32 + 1);
            docs.update(|d| d.push(Doc { id, name: format!("Untitled {id}") }));
            current.set(id);
        })
    });

    assert!(new_tab_button(&app));
    app.get_by_role(Role::Button, mitsuami::core::l10n::tr("mitsuami-tab-new", &[])).click().await;
    assert_eq!(strip(&app), ["notes.txt", "Untitled 2 *"]);
    assert_eq!(current.get(), 2);
}

/// A strip in the content is as high as the platform draws it, and grows
/// across; the window's tabs (a strip in the title bar on macOS, a tab
/// view in WinUI's) take no room there. GNOME and KDE have none, so it's a
/// strip.
#[mitsuami_test::test]
async fn a_strip_takes_room_and_the_windows_tabs_none(app: TestApp) {
    let (docs, current) = (docs(&["notes.txt", "todo.md"]), signal(1));
    let style = signal(DocumentTabsStyle::Strip);
    app.mount(move || Column::new().children((tabs(docs, current).tabs_style(style), Text::new("Content").grow(1.0))));
    let list = app.a11y_tree().walk().into_iter().find(|n| n.role == Role::TabList).unwrap().id;
    let strip_frame = app.ui().window_frame(list).unwrap();
    assert!(strip_frame.height() > 0.0, "{strip_frame:?}");

    style.set(DocumentTabsStyle::Window);
    app.settle().await;
    let frame = app.ui().window_frame(list).unwrap();
    // Linux has no tabs of the window's.
    if matches!(app.backend_name(), "gtk" | "kirigami") {
        assert_eq!(frame.height(), strip_frame.height());
    } else {
        assert_eq!(frame.height(), 0.0, "{frame:?}");
    }
    // Still the same tabs.
    assert_eq!(strip(&app), ["notes.txt *", "todo.md"]);
}

#[mitsuami_test::test]
async fn in_view(app: TestApp) {
    let current = signal(2);
    app.mount(move || {
        view! {
            <DocumentTabs selection=current tabs_style=DocumentTabsStyle::Strip>
                <DocumentTab value=1>"notes.txt"</DocumentTab>
                <DocumentTab value=2>"todo.md"</DocumentTab>
            </DocumentTabs>
        }
    });

    assert_eq!(strip(&app), ["notes.txt", "todo.md *"]);
}

mitsuami_test::main!();
