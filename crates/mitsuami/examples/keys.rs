//! Keys on lists and containers: `cargo run -p mitsuami --example keys`.
//!
//! A folder's files in a list. Focus the list (click a row, or Tab) and
//! press the keys the platform's file manager uses:
//!
//! - Move to Trash, on the list: ⌘⌫ on macOS, Delete elsewhere.
//! - Quick Look on Space, on macOS only: GTK's and XAML's lists keep
//!   Space, as it selects the focused row there.
//! - Rename with F2 and Refresh with F5, off macOS, on the column around
//!   the list: the list doesn't use them, so they go up to it.
//!
//! The focused control's own keys stay its own: the arrows move the
//! selection, and in the Filter field Space and Delete edit the text.
//! Nothing shows keys taken this way, so each command has a button too.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn start() -> Vec<String> {
    ["Notes.txt", "Photo.png", "Report.pdf", "Todo.md", "Budget.numbers"].map(String::from).to_vec()
}

fn row(name: String) -> impl View {
    view! {
        <Row padding_x=Spacing::Md padding_y=Spacing::Xs>
            <Text>{name}</Text>
        </Row>
    }
}

/// The keys the platform's file manager has for these commands: Finder's,
/// and those Nautilus, Dolphin and File Explorer agree on.
struct Keys {
    trash: Shortcut,
    preview: Option<Shortcut>,
    rename: Option<Shortcut>,
    refresh: Option<Shortcut>,
}

fn keys() -> Keys {
    platform! {
        macos => Keys {
            trash: Shortcut::primary(Key::Backspace),
            preview: Some(Shortcut::new(' ')),
            // Finder renames with Return and has no Refresh.
            rename: None,
            refresh: None,
        },
        _ => Keys {
            trash: Shortcut::new(Key::Delete),
            preview: None,
            rename: Some(Shortcut::new(Key::F(2))),
            refresh: Some(Shortcut::new(Key::F(5))),
        },
    }
}

pub fn page() -> impl View {
    let files = signal(start());
    let selected = signal(Vec::<String>::new());
    let filter = signal(String::new());
    let log = signal(String::from("Focus the list and press a key."));
    let say = move |what: String| log.set(what);
    let names = move || selected.get_untracked().join(", ");

    let trash = move || {
        let gone = selected.get_untracked();
        if gone.is_empty() {
            return;
        }
        files.update(|f| f.retain(|name| !gone.contains(name)));
        selected.set(Vec::new());
        say(format!("Moved {} to the Trash.", gone.join(", ")));
    };
    let preview = move || say(format!("Quick Look: {}.", names()));
    let rename = move || say(format!("Rename {}.", names()));
    let refresh = move || {
        files.set(start());
        say("Refreshed.".to_owned());
    };

    let shown = move || {
        let filter = filter.get().to_lowercase();
        files.get().into_iter().filter(|f| f.to_lowercase().contains(&filter)).collect::<Vec<_>>()
    };
    let keys = keys();
    let mut list = List::new(shown, |f: &String| f.clone(), row)
        .selected(selected)
        .selection_mode(SelectionMode::Multiple)
        .a11y_label("Files")
        .height(180)
        .on_key(keys.trash, trash);
    if let Some(key) = keys.preview {
        list = list.on_key(key, preview);
    }
    let some = move || !selected.get().is_empty();
    let mut column = view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <SearchInput bind=filter a11y_label="Filter" placeholder="Filter"/>
            {list}
            <Row gap=Spacing::Sm>
                <Button enabled=some @click=trash>"Move to Trash"</Button>
                <Button enabled=some @click=preview>"Quick Look"</Button>
                <Button enabled=some @click=rename>"Rename"</Button>
                <Button @click=refresh>"Refresh"</Button>
            </Row>
            <Text test_id="log">{log}</Text>
        </Column>
    };
    if let Some(key) = keys.rename {
        column = column.on_key(key, rename);
    }
    if let Some(key) = keys.refresh {
        column = column.on_key(key, refresh);
    }
    column
}

fn main() {
    App::new().window("Keys", WindowSize::FitHeight(420.0), page).run();
}
