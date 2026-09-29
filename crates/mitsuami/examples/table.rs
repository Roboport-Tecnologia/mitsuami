//! Table: `cargo run -p mitsuami --example table`.
//!
//! - Files under column headers: Name (with an icon, taking the room
//!   left), Size, Kind and Modified. Resize the columns by their headers'
//!   edges, as the platform lets you.
//! - Press a header to sort by it; again, the other way round. Kind has no
//!   sort key, so its header doesn't sort. The app sorts the data: the
//!   header shows how.
//! - Select one or several rows (a switch changes the selection mode),
//!   activate one (double-click or Return), add and remove rows, and
//!   scroll to the last.
//! - A raw platform setting, through `.native()`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct File {
    id: u32,
    name: String,
    /// In bytes.
    size: u64,
    kind: &'static str,
    /// Days ago.
    modified: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum By {
    Name,
    Size,
    Modified,
}

const KINDS: [(&str, &str); 4] =
    [("txt", "Plain Text"), ("png", "PNG Image"), ("pdf", "PDF Document"), ("zip", "ZIP Archive")];

fn file(id: u32) -> File {
    let (extension, kind) = KINDS[id as usize % KINDS.len()];
    File {
        id,
        name: format!("File {id}.{extension}"),
        size: (u64::from(id) * 7919 % 5000) * 1024 + 300,
        kind,
        modified: id * 13 % 40,
    }
}

fn human_size(bytes: u64) -> String {
    match bytes {
        0..1_000 => format!("{bytes} bytes"),
        1_000..1_000_000 => format!("{:.0} KB", bytes as f64 / 1e3),
        _ => format!("{:.1} MB", bytes as f64 / 1e6),
    }
}

fn icon_for(kind: &str) -> &'static str {
    let image = kind.starts_with("PNG");
    platform! {
        macos => if image { "photo" } else { "doc" },
        gtk => if image { "image-x-generic" } else { "text-x-generic" },
        kde => if image { "image-x-generic" } else { "text-x-generic" },
        windows => if image { "\u{EB9F}" } else { "\u{E8A5}" },
        _ => "",
    }
}

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn columns() -> Vec<TableColumn<File>> {
    vec![
        TableColumn::new("Name", |f: File| {
            Row::new().gap(Spacing::Sm).align(Align::Center).children((
                Icon::new(icon_for(f.kind)).a11y_hidden(),
                Text::new(f.name).max_lines(1).grow(1.0).shrink(1.0).basis(0),
            ))
        })
        .expand()
        .sort_key(By::Name),
        TableColumn::new("Size", |f: File| Text::new(human_size(f.size)).text_align(TextAlign::End).max_lines(1))
            .width(80)
            .sort_key(By::Size),
        TableColumn::new("Kind", |f: File| Text::new(f.kind).max_lines(1)).width(110),
        TableColumn::new("Modified", |f: File| {
            Text::new(match f.modified {
                0 => "Today".to_string(),
                1 => "Yesterday".to_string(),
                n => format!("{n} days ago"),
            })
            .max_lines(1)
        })
        .width(100)
        .sort_key(By::Modified),
    ]
}

/// Sorted as the header shows.
fn sorted(files: Signal<Vec<File>>, sort: Signal<Sort<By>>) -> impl Fn() -> Vec<File> + Copy {
    move || {
        let mut files = files.get();
        let Sort { by, order } = sort.get();
        match by {
            By::Name => files.sort_by_key(|f| f.id),
            By::Size => files.sort_by_key(|f| f.size),
            By::Modified => files.sort_by_key(|f| f.modified),
        }
        if order == SortOrder::Descending {
            files.reverse();
        }
        files
    }
}

fn playground() -> impl View {
    let files = signal((0..200).map(file).collect::<Vec<_>>());
    let next = signal(200u32);
    let sort = signal(Sort::ascending(By::Name));
    let selected = signal(Vec::<u32>::new());
    let several = signal(true);
    let activated = signal(None::<u32>);
    let handle = ListHandle::new();
    let scroll_handle = handle.clone();
    let add = move || {
        let n = next.get_untracked();
        next.set(n + 1);
        files.update(|f| f.push(file(n)));
        selected.set(vec![n]);
    };
    let remove = move || {
        let gone = selected.get_untracked();
        files.update(|f| f.retain(|f| !gone.contains(&f.id)));
        selected.set(Vec::new());
    };
    let shown = sorted(files, sort);
    view! {
        <Column gap=Spacing::Md grow=1.0>
            {heading("Files")}
            <Row gap=Spacing::Sm align=Align::Center>
                <Button @click=add>"Add"</Button>
                <Button enabled=move || !selected.get().is_empty() @click=remove>"Remove selected"</Button>
                <Button @click=move || {
                    if let Some(last) = shown().last() {
                        scroll_handle.scroll_to(&last.id);
                    }
                }>"Scroll to last"</Button>
                <Text>"Select several"</Text>
                <Switch bind=several>"Select several"</Switch>
            </Row>
            <Table
                each=shown
                key=|f: &File| f.id
                columns=columns()
                sort=sort
                selection_mode=move || if several.get() { SelectionMode::Multiple } else { SelectionMode::Single }
                selected=selected
                @activate=move |id| activated.set(Some(id))
                handle=handle
                list_style=ListStyle::Framed
                height=300
                grow=1.0
            />
            <Text text_style=TextStyle::Caption>
                {move || {
                    let Sort { by, order } = sort.get();
                    let activated = activated.get().map_or("none".to_string(), |id| file(id).name);
                    format!(
                        "Sorted by {by:?}, {order:?}; {} of {} selected; last activated: {activated}",
                        selected.get().len(),
                        files.get().len()
                    )
                }}
            </Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native table view.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Table>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTableView| {
                t.setUsesAlternatingRowBackgroundColors(true)
            }),
            "AppKit: usesAlternatingRowBackgroundColors stripes the rows.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|v: &mitsuami::gtk::gtk::ColumnView| v.set_show_column_separators(true)),
            "GTK: show-column-separators draws a line between columns.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|v: &mitsuami::kirigami::QmlObject| v.set_bool("alternatingRows", true)),
            "Qt Quick: alternatingRows stripes the rows.",
        ),
        windows => (
            mitsuami::winui::tweak(|v: &mitsuami::winui::bindings::ListView| {
                use mitsuami::winui::windows_core::Interface;
                v.cast::<mitsuami::winui::bindings::IListViewBase>()?.SetSingleSelectionFollowsFocus(false)
            }),
            "WinUI: SingleSelectionFollowsFocus off lets the arrow keys move without selecting; Space selects.",
        ),
    };
    let sort = signal(Sort::ascending(By::Name));
    let files = signal((0..8).map(file).collect::<Vec<_>>());
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Table each=sorted(files, sort) key=|f: &File| f.id columns=columns() sort=sort height=180 native=tweak/>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <ScrollView>
            <Column padding=Spacing::Xl gap=Spacing::Xl>
                {playground()}
                {platform_option()}
            </Column>
        </ScrollView>
    }
}

fn main() {
    App::new().window("Table", Size::new(720.0, 720.0), page).run();
}
