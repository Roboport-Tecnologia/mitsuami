//! Tree: `cargo run -p mitsuami --example tree`.
//!
//! - The two styles: edge to edge, as in a sidebar, and framed, as inside
//!   a form. The platform draws the disclosures and indents the levels.
//! - A playground: open and close folders (the disclosure, the platform's
//!   keys, or the buttons, through the app's set of open rows), add a file
//!   to the selected folder, activate a row (double-click or Return), open
//!   a folder whose files take a moment to read, and drag rows: onto a
//!   folder to put them in it, between rows to put them there. The tree
//!   shows nothing moved until the app moves its data, as here.
//! - "Select several rows" switches between selecting several rows, the
//!   platform's way (check boxes on Windows, whose tree shows several
//!   selected rows no other way), and one at a time; a drag then moves the
//!   selection, or the one row.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

/// A file or a folder; a folder's files are `Some`, empty or not.
#[derive(Clone, PartialEq)]
struct Entry {
    path: String,
    kids: Option<Vec<Entry>>,
}

impl Entry {
    fn name(&self) -> String {
        self.path.rsplit('/').next().unwrap_or_default().to_string()
    }
}

fn file(path: &str) -> Entry {
    Entry { path: path.to_string(), kids: None }
}

fn folder(path: &str, kids: Vec<Entry>) -> Entry {
    Entry { path: path.to_string(), kids: Some(kids) }
}

fn project() -> Vec<Entry> {
    vec![
        folder(
            "src",
            vec![
                file("src/main.rs"),
                folder("src/ui", vec![file("src/ui/button.rs"), file("src/ui/list.rs"), file("src/ui/tree.rs")]),
                file("src/app.rs"),
            ],
        ),
        folder("docs", vec![file("docs/guide.md"), file("docs/faq.md")]),
        folder("assets", vec![]),
        file("Cargo.toml"),
        file("README.md"),
    ]
}

fn row(entry: Entry) -> impl View {
    let name = entry.name();
    view! {
        <Row padding_x=Spacing::Xs padding_y=Spacing::Xs>
            <Text>{name}</Text>
        </Row>
    }
}

fn project_tree(style: ListStyle) -> Tree<Entry, String> {
    let open = signal(vec!["src".to_string()]);
    Tree::new(project, |e: &Entry| e.path.clone(), |e| e.kids.clone(), row)
        .expanded(open)
        .list_style(style)
        .height(180)
        .grow(1.0)
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Styles")}
            <Row gap=Spacing::Lg>
                <Column gap=Spacing::Sm grow=1.0>
                    <Text text_style=TextStyle::Caption>"Plain"</Text>
                    {project_tree(ListStyle::Plain)}
                </Column>
                <Column gap=Spacing::Sm grow=1.0>
                    <Text text_style=TextStyle::Caption>"Framed"</Text>
                    {project_tree(ListStyle::Framed)}
                </Column>
            </Row>
        </Column>
    }
}

/// Every folder's path, all the way down.
fn folders(entries: &[Entry], out: &mut Vec<String>) {
    for entry in entries {
        if let Some(kids) = &entry.kids {
            out.push(entry.path.clone());
            folders(kids, out);
        }
    }
}

/// Adds a file to the folder at `path`.
fn add_to(entries: &mut [Entry], path: &str, name: &str) {
    for entry in entries {
        if let Some(kids) = &mut entry.kids {
            if entry.path == path {
                kids.push(file(&format!("{path}/{name}")));
                return;
            }
            add_to(kids, path, name);
        }
    }
}

/// The folder a path is in, `None` at the top.
fn parent_of(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

/// Takes the entry at `path` out of the tree.
fn take(entries: &mut Vec<Entry>, path: &str) -> Option<Entry> {
    if let Some(i) = entries.iter().position(|e| e.path == path) {
        return Some(entries.remove(i));
    }
    entries.iter_mut().filter_map(|e| e.kids.as_mut()).find_map(|kids| take(kids, path))
}

/// The entry, and everything in it, as it is in `parent`: its paths change.
fn rebase(entry: Entry, parent: Option<&str>) -> Entry {
    let path = match parent {
        Some(parent) => format!("{parent}/{}", entry.name()),
        None => entry.name(),
    };
    let kids = entry.kids.map(|kids| kids.into_iter().map(|k| rebase(k, Some(&path))).collect());
    Entry { path, kids }
}

/// The rows of the folder at `parent` (the top rows for `None`).
fn rows_of<'a>(entries: &'a mut Vec<Entry>, parent: Option<&str>) -> Option<&'a mut Vec<Entry>> {
    let Some(parent) = parent else { return Some(entries) };
    entries.iter_mut().find_map(|e| match (e.path == parent, e.kids.as_mut()) {
        (true, kids) => kids,
        (false, Some(kids)) => rows_of(kids, Some(parent)),
        (false, None) => None,
    })
}

/// Moves the entries at `paths` where they were dropped: into a folder
/// dropped onto (only folders take drops onto them), before a row, or at
/// the end of the top rows. Returns each moved path's new one.
fn move_entries(entries: &mut Vec<Entry>, paths: &[String], to: &RowDrop<String>) -> Vec<(String, String)> {
    let (parent, before) = match to {
        RowDrop::On(path) if folder_at(entries, path) => (Some(path.clone()), None),
        RowDrop::On(path) | RowDrop::Before(path) => (parent_of(path).map(str::to_string), Some(path.clone())),
        RowDrop::End => (None, None),
    };
    let mut moved = Vec::new();
    let mut renamed = Vec::new();
    for path in paths {
        if before.as_ref() == Some(path) {
            continue;
        }
        if let Some(entry) = take(entries, path) {
            let entry = rebase(entry, parent.as_deref());
            renamed.push((path.clone(), entry.path.clone()));
            moved.push(entry);
        }
    }
    if let Some(rows) = rows_of(entries, parent.as_deref()) {
        let at = before.and_then(|b| rows.iter().position(|e| e.path == b)).unwrap_or(rows.len());
        rows.splice(at..at, moved);
    }
    renamed
}

fn folder_at(entries: &[Entry], path: &str) -> bool {
    let mut all = Vec::new();
    folders(entries, &mut all);
    all.iter().any(|p| p == path)
}

/// A path, after the moves: a moved entry's, or one inside it.
fn renamed(path: &str, moves: &[(String, String)]) -> String {
    for (from, to) in moves {
        if path == from {
            return to.clone();
        }
        if let Some(rest) = path.strip_prefix(&format!("{from}/")) {
            return format!("{to}/{rest}");
        }
    }
    path.to_string()
}

/// A tree to open and close, add to, activate and drag rows in, with a
/// folder read as it opens.
fn playground() -> impl View {
    let entries = signal(project());
    let open = signal(Vec::<String>::new());
    let selected = signal(Vec::<String>::new());
    let several = signal(true);
    let activated = signal(None::<String>);
    let next = signal(1u32);
    // The remote folder's files, once read: they take a moment, as a
    // folder on a network share does.
    let remote = signal(None::<Vec<Entry>>);
    effect(move || {
        if open.get().iter().any(|p| p == "remote") && remote.get_untracked().is_none() {
            spawn_local(async move {
                sleep(Duration::from_millis(700)).await;
                remote.set(Some((1..=3).map(|n| file(&format!("remote/report {n}.pdf"))).collect()));
            });
        }
    });
    let roots = move || {
        let mut roots = vec![folder("remote", Vec::new())];
        roots.extend(entries.get());
        roots
    };
    let children = move |e: &Entry| match e.path.as_str() {
        "remote" => Some(remote.get().unwrap_or_default()),
        _ => e.kids.clone(),
    };
    let open_all = move || {
        let mut all = vec!["remote".to_string()];
        folders(&entries.get_untracked(), &mut all);
        open.set(all);
    };
    let add = move || {
        let Some(path) = selected.get_untracked().into_iter().next() else { return };
        let n = next.get_untracked();
        next.set(n + 1);
        entries.update(|e| add_to(e, &path, &format!("new {n}.txt")));
        // The folder opens to show it.
        if !open.get_untracked().contains(&path) {
            open.update(|o| o.push(path));
        }
    };
    let is_folder = move |path: &str| {
        let mut all = Vec::new();
        folders(&entries.get(), &mut all);
        all.iter().any(|p| p == path)
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Row gap=Spacing::Sm>
                <Button @click=open_all>"Open all"</Button>
                <Button @click=move || open.set(Vec::new())>"Close all"</Button>
                <Button
                    enabled=move || selected.get().first().is_some_and(|p| is_folder(p))
                    @click=add
                >"Add a file to the folder"</Button>
                <Switch bind=several>"Select several rows"</Switch>
            </Row>
            {Tree::new(roots, |e: &Entry| e.path.clone(), children, row)
                .expanded(open)
                .selected(selected)
                .selection_mode(move || if several.get() { SelectionMode::Multiple } else { SelectionMode::Single })
                .on_activate(move |path| activated.set(Some(path)))
                .on_move(RowMoves::onto_rows().and_between_rows(), move |paths, to| {
                    // The remote folder isn't ours to change.
                    let remote = |p: &String| p == "remote" || p.starts_with("remote/");
                    let into_remote = matches!(&to, RowDrop::On(p) | RowDrop::Before(p) if remote(p));
                    let paths: Vec<String> = paths.into_iter().filter(|p| !remote(p)).collect();
                    if into_remote || paths.is_empty() {
                        return;
                    }
                    let mut now = entries.get_untracked();
                    let moves = move_entries(&mut now, &paths, &to);
                    entries.set(now);
                    // Open folders and the selection follow them.
                    open.update(|o| o.iter_mut().for_each(|p| *p = renamed(p, &moves)));
                    selected.update(|s| s.iter_mut().for_each(|p| *p = renamed(p, &moves)));
                })
                .list_style(ListStyle::Framed)
                .height(220)}
            <Text text_style=TextStyle::Caption>
                {move || {
                    let selected = selected.get().first().cloned().unwrap_or_else(|| "nothing".into());
                    let activated = activated.get().unwrap_or_else(|| "none".into());
                    format!("{} open; selected: {selected}; last activated: {activated}. Drag rows onto a folder, or between rows.", open.get().len())
                }}
            </Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <ScrollView>
            <Column padding=Spacing::Xl gap=Spacing::Xl>
                {gallery()}
                {playground()}
            </Column>
        </ScrollView>
    }
}

fn main() {
    App::new().window("Tree", WindowSize::FitHeight(640.0), page).run();
}
