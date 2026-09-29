//! Shortcuts with keys that type no character: `cargo run -p mitsuami
//! --example shortcuts`.
//!
//! A folder browser's Go and File menus, with the keys the platform's
//! file manager uses. Each platform shows them its own way (⌘↑ on macOS,
//! Alt+Up elsewhere), from the same `Shortcut`.
//!
//! - Go › Back, Forward and Enclosing Folder: ⌘[, ⌘] and ⌘↑ in Finder;
//!   Alt+Left, Alt+Right and Alt+Up in Nautilus, Dolphin and File
//!   Explorer.
//! - Go › Home: ⇧⌘H in Finder, Alt+Home elsewhere.
//! - File › Move to Trash: ⌘⌫ in Finder, Delete elsewhere.
//! - File › Rename: F2 off macOS; Finder has no shortcut for it.
//! - File › Open has none of its own: the list opens a row on a
//!   double-click or Return, as the platform's list does.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

/// The folders under each folder, by path.
fn folders(path: &[String]) -> Vec<String> {
    let names: &[&str] = match path.len() {
        0 => &["Documents", "Music", "Pictures"],
        1 => &["2025", "2026"],
        2 => &["Spring", "Autumn"],
        _ => &[],
    };
    names.iter().map(|n| n.to_string()).collect()
}

fn row(name: String) -> impl View {
    view! {
        <Row padding_x=Spacing::Md padding_y=Spacing::Xs>
            <Text>{name}</Text>
        </Row>
    }
}

struct Keys {
    back: Shortcut,
    forward: Shortcut,
    up: Shortcut,
    home: Shortcut,
    trash: Shortcut,
    rename: Option<Shortcut>,
}

fn keys() -> Keys {
    platform! {
        macos => Keys {
            back: Shortcut::primary('['),
            forward: Shortcut::primary(']'),
            up: Shortcut::primary(Key::Up),
            home: Shortcut::primary('h').shift(),
            trash: Shortcut::primary(Key::Backspace),
            rename: None,
        },
        _ => Keys {
            back: Shortcut::new(Key::Left).alt(),
            forward: Shortcut::new(Key::Right).alt(),
            up: Shortcut::new(Key::Up).alt(),
            home: Shortcut::new(Key::Home).alt(),
            trash: Shortcut::new(Key::Delete),
            rename: Some(Shortcut::new(Key::F(2))),
        },
    }
}

pub fn page() -> impl View {
    let path = signal(Vec::<String>::new());
    let (back, forward) = (signal(Vec::<Vec<String>>::new()), signal(Vec::<Vec<String>>::new()));
    let selected = signal(Vec::<String>::new());
    let trashed = signal(Vec::<Vec<String>>::new());
    let log = signal(String::from("Nothing chosen yet."));

    let go = move |to: Vec<String>| {
        back.update(|b| b.push(path.get_untracked()));
        forward.set(Vec::new());
        path.set(to);
        selected.set(Vec::new());
    };
    let go_back = move || {
        let Some(to) = back.get_untracked().last().cloned() else { return };
        back.update(|b| _ = b.pop());
        forward.update(|f| f.push(path.get_untracked()));
        path.set(to);
    };
    let go_forward = move || {
        let Some(to) = forward.get_untracked().last().cloned() else { return };
        forward.update(|f| _ = f.pop());
        back.update(|b| b.push(path.get_untracked()));
        path.set(to);
    };
    let up = move || {
        let mut to = path.get_untracked();
        if to.pop().is_some() {
            go(to);
        }
    };
    let home = move || {
        if !path.get_untracked().is_empty() {
            go(Vec::new());
        }
    };
    let open = move || {
        if let Some(name) = selected.get_untracked().first() {
            let mut to = path.get_untracked();
            to.push(name.clone());
            go(to);
        }
    };
    let trash = move || {
        let here = path.get_untracked();
        for name in selected.get_untracked() {
            let mut item = here.clone();
            item.push(name);
            trashed.update(|t| t.push(item));
        }
        selected.set(Vec::new());
        log.set("Moved to the Trash.".to_owned());
    };
    let rename = move || log.set(format!("Rename {}.", selected.get_untracked().join(", ")));

    let shown = move || {
        let here = path.get();
        let trashed = trashed.get();
        folders(&here)
            .into_iter()
            .filter(|name| {
                let mut item = here.clone();
                item.push(name.clone());
                !trashed.contains(&item)
            })
            .collect::<Vec<_>>()
    };
    let title = move || {
        let here = path.get();
        if here.is_empty() { "Home".to_owned() } else { format!("Home › {}", here.join(" › ")) }
    };
    let some = move || !selected.get().is_empty();
    let keys = keys();
    let rename_item = MenuItem::new("Rename…").enabled(some).on_select(rename);
    let rename_item = match keys.rename {
        Some(key) => rename_item.shortcut(key),
        None => rename_item,
    };
    let list = List::new(shown, |f: &String| f.clone(), row)
        .selected(selected)
        .on_activate(move |name: String| {
            let mut to = path.get_untracked();
            to.push(name);
            go(to);
        })
        .a11y_label("Folders")
        .height(160);
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <MenuBar>
                <Menu title="File">
                    <MenuItem enabled=some @select=open>"Open"</MenuItem>
                    <MenuItem shortcut=keys.trash enabled=some @select=trash>"Move to Trash"</MenuItem>
                    {rename_item}
                </Menu>
                <Menu title="Go">
                    <MenuItem shortcut=keys.back enabled=move || !back.get().is_empty() @select=go_back>"Back"</MenuItem>
                    <MenuItem shortcut=keys.forward enabled=move || !forward.get().is_empty() @select=go_forward>"Forward"</MenuItem>
                    <MenuItem shortcut=keys.up enabled=move || !path.get().is_empty() @select=up>"Enclosing Folder"</MenuItem>
                    <MenuSeparator/>
                    <MenuItem shortcut=keys.home enabled=move || !path.get().is_empty() @select=home>"Home"</MenuItem>
                </Menu>
            </MenuBar>
            <Text text_style=TextStyle::Headline>{title}</Text>
            {list}
            <Text test_id="log">{log}</Text>
        </Column>
    }
}

fn main() {
    App::new().window("Shortcuts", WindowSize::FitHeight(380.0), page).run();
}
