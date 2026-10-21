//! Moving rows: `cargo run -p mitsuami --example row_moves`.
//!
//! - Groups: repositories under groups, as SourceGit's welcome page has
//!   them. Drag a repository onto a group to put it in, onto another
//!   repository to put it in that one's group, or onto the empty part of
//!   the list to take it out of every group. The list takes drops onto
//!   rows only (`RowMoves::onto_rows`): between two rows lands on one.
//! - Playlist: songs that reorder. Drag one between two others, or to the
//!   empty part for the end. The list takes drops between rows only
//!   (`RowMoves::between_rows`): onto a song lands before it.
//! - Select several songs and drag one to move them all, as each platform
//!   drags a selection.
//! - Repositories in a group are indented as the platform indents a
//!   tree's levels (`Spacing::Indent`).
//! - Dragging isn't reachable from the keyboard or a screen reader, so Move
//!   Up and Move Down move the selected song too.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Group(&'static str),
    Repository { name: &'static str, group: Option<&'static str> },
}

impl Entry {
    fn name(&self) -> &'static str {
        match self {
            Entry::Group(name) | Entry::Repository { name, .. } => name,
        }
    }
}

const GROUPS: [&str; 2] = ["Work", "Personal"];

/// The rows the groups list shows: each group and its repositories under
/// it, then the repositories in none.
fn rows(repositories: &[(&'static str, Option<&'static str>)]) -> Vec<Entry> {
    let mut rows = Vec::new();
    for group in GROUPS {
        rows.push(Entry::Group(group));
        for &(name, in_group) in repositories {
            if in_group == Some(group) {
                rows.push(Entry::Repository { name, group: Some(group) });
            }
        }
    }
    for &(name, group) in repositories {
        if group.is_none() {
            rows.push(Entry::Repository { name, group: None });
        }
    }
    rows
}

fn groups() -> impl View {
    let repositories = signal(vec![
        ("mitsuami", Some("Work")),
        ("sourcegit", Some("Work")),
        ("2ksbox", Some("Personal")),
        ("dotfiles", None),
    ]);
    let status = signal(String::from("Drag a repository onto a group."));
    let moved = move |names: Vec<&'static str>, to: RowDrop<&'static str>| {
        // Onto a group: into it; onto a repository: into its group; the
        // empty part: out of every group.
        let group = match to {
            RowDrop::On(target) if GROUPS.contains(&target) => Some(target),
            RowDrop::On(target) => {
                repositories.with_untracked(|r| r.iter().find(|(n, _)| *n == target).and_then(|(_, g)| *g))
            }
            RowDrop::Before(_) | RowDrop::End => None,
        };
        let names: Vec<&'static str> = names.into_iter().filter(|n| !GROUPS.contains(n)).collect();
        repositories.update(|r| {
            for (name, in_group) in r.iter_mut() {
                if names.contains(name) {
                    *in_group = group;
                }
            }
        });
        status.set(format!("Moved {} to {}", names.join(", "), group.unwrap_or("no group")));
    };
    Column::new().gap(Spacing::Sm).grow(1.0).children((
        Text::new("Groups").text_style(TextStyle::Headline),
        List::new(
            move || rows(&repositories.get()),
            |entry: &Entry| entry.name(),
            move |entry| match entry {
                Entry::Group(name) => Row::new()
                    .padding_x(Spacing::Md)
                    .padding_y(Spacing::Xs)
                    .child(Text::new(name).weight(FontWeight::Bold)),
                Entry::Repository { name, .. } => {
                    // In a group, one level in, by the platform's own tree
                    // indentation. A row stays while its key does, so it
                    // follows the data rather than the item it was made for.
                    let nested = move || repositories.with(|r| r.iter().any(|(n, g)| *n == name && g.is_some()));
                    let indent = move || if nested() { Length::from(Spacing::Indent) } else { Length::Px(0.0) };
                    Row::new().padding_x(Spacing::Md).padding_y(Spacing::Xs).child(Text::new(name).margin_start(indent))
                }
            },
        )
        .selected(signal(Vec::new()))
        .on_move(RowMoves::onto_rows(), moved)
        .list_style(ListStyle::Framed)
        .grow(1.0),
        Text::new(status).color(Color::SecondaryLabel),
    ))
}

fn playlist() -> impl View {
    let songs = signal(vec!["Intro", "Blue Train", "So What", "Naima", "Take Five"]);
    let selected = signal(Vec::<&'static str>::new());
    let moved = move |moving: Vec<&'static str>, to: RowDrop<&'static str>| {
        songs.update(|songs| {
            songs.retain(|s| !moving.contains(s));
            let at = match to {
                RowDrop::Before(song) | RowDrop::On(song) => {
                    songs.iter().position(|s| *s == song).unwrap_or(songs.len())
                }
                RowDrop::End => songs.len(),
            };
            for (i, song) in moving.iter().enumerate() {
                songs.insert(at + i, song);
            }
        });
    };
    // The keyboard's way: the selected songs one step up or down.
    let step = move |down: bool| {
        let chosen = selected.get_untracked();
        songs.update(|songs| {
            let mut order: Vec<usize> = (0..songs.len()).filter(|i| chosen.contains(&songs[*i])).collect();
            if down {
                order.reverse();
            }
            for i in order {
                let j = if down { i + 1 } else { i.wrapping_sub(1) };
                if j < songs.len() && !chosen.contains(&songs[j]) {
                    songs.swap(i, j);
                }
            }
        });
    };
    Column::new().gap(Spacing::Sm).grow(1.0).children((
        Text::new("Playlist").text_style(TextStyle::Headline),
        List::new(
            songs,
            |song: &&'static str| *song,
            |song| Row::new().padding_x(Spacing::Md).padding_y(Spacing::Xs).child(Text::new(song)),
        )
        .selected(selected)
        .selection_mode(SelectionMode::Multiple)
        .on_move(RowMoves::between_rows(), moved)
        .list_style(ListStyle::Framed)
        .grow(1.0),
        Row::new().gap(Spacing::Sm).children((
            Button::new("Move Up").enabled(move || !selected.get().is_empty()).on_click(move || step(false)),
            Button::new("Move Down").enabled(move || !selected.get().is_empty()).on_click(move || step(true)),
        )),
    ))
}

pub fn page() -> impl View {
    Row::new().padding(Spacing::Xl).gap(Spacing::Xl).align(Align::Stretch).grow(1.0).children((groups(), playlist()))
}

fn main() {
    App::new().window("Moving rows", Size::new(640.0, 420.0), page).run();
}
