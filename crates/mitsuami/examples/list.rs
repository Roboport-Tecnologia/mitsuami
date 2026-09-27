//! List: `cargo run -p mitsuami --example list`.
//!
//! - The two styles: edge to edge, as in a sidebar or a window's main
//!   content, and framed, as inside a form.
//! - A playground: add and remove rows, select one or several (a switch
//!   changes the selection mode), activate one (double-click or Return),
//!   and scroll to the last.
//! - A raw platform setting, through `.native()`. Lists have no semantic
//!   options past their rows, selection and style: what platforms offer is
//!   each one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

const FRUIT: [&str; 12] = [
    "Apple",
    "Banana",
    "Cherry",
    "Date",
    "Elderberry",
    "Fig",
    "Grape",
    "Honeydew",
    "Kiwi",
    "Lemon",
    "Mango",
    "Nectarine",
];

fn row(text: String) -> impl View {
    Row::new().padding_x(Spacing::Md).padding_y(Spacing::Xs).child(Text::new(text))
}

fn fruit_list(style: ListStyle) -> List<&'static str, &'static str> {
    List::new(|| FRUIT.to_vec(), |f: &&str| *f, |f| row(f.to_string())).list_style(style).height(140).grow(1.0)
}

fn gallery() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("Styles"),
        Row::new().gap(Spacing::Lg).children((
            Column::new()
                .gap(Spacing::Sm)
                .grow(1.0)
                .children((Text::new("Plain").text_style(TextStyle::Caption), fruit_list(ListStyle::Plain))),
            Column::new()
                .gap(Spacing::Sm)
                .grow(1.0)
                .children((Text::new("Framed").text_style(TextStyle::Caption), fruit_list(ListStyle::Framed))),
        )),
    ))
}

/// A list of numbered rows to add to, select in, remove from and scroll.
fn playground() -> impl View {
    let rows = signal((1..=20).collect::<Vec<u32>>());
    let next = signal(21u32);
    let selected = signal(Vec::<u32>::new());
    let several = signal(true);
    let activated = signal(None::<u32>);
    let handle = ListHandle::new();
    let scroll_handle = handle.clone();
    let add = move || {
        let n = next.get_untracked();
        next.set(n + 1);
        rows.update(|r| r.push(n));
    };
    let remove = move || {
        let gone = selected.get_untracked();
        rows.update(|r| r.retain(|n| !gone.contains(n)));
        selected.set(Vec::new());
    };
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Row::new().gap(Spacing::Sm).children((
            Button::new("Add").on_click(add),
            Button::new("Remove selected").enabled(move || !selected.get().is_empty()).on_click(remove),
            Button::new("Scroll to last").on_click(move || {
                if let Some(last) = rows.get_untracked().last() {
                    scroll_handle.scroll_to(last);
                }
            }),
        )),
        Row::new()
            .gap(Spacing::Md)
            .align(Align::Center)
            .children((Text::new("Select several"), Switch::new("Select several").bind(several))),
        List::new(rows, |n: &u32| *n, |n| row(format!("Row {n}")))
            .selection_mode(move || if several.get() { SelectionMode::Multiple } else { SelectionMode::Single })
            .selected(selected)
            .on_activate(move |n| activated.set(Some(n)))
            .handle(handle)
            .list_style(ListStyle::Framed)
            .height(160),
        Text::new(move || {
            let selected = selected.get();
            let activated = activated.get().map_or("none".to_string(), |n| format!("Row {n}"));
            format!("{} of {} selected; last activated: {activated}", selected.len(), rows.get().len())
        })
        .text_style(TextStyle::Caption),
    ))
}

/// A setting only this platform has, straight on the native list view.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<List>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTableView| {
                t.setUsesAlternatingRowBackgroundColors(true)
            }),
            "AppKit: usesAlternatingRowBackgroundColors stripes the rows.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|v: &mitsuami::gtk::gtk::ListView| v.set_show_separators(true)),
            "GTK: show-separators draws a line between rows.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|v: &mitsuami::kirigami::QmlObject| v.set_bool("keyNavigationWraps", true)),
            "Qt Quick: keyNavigationWraps moves from the last row to the first with the arrow keys, and back.",
        ),
        windows => (
            mitsuami::winui::tweak(|v: &mitsuami::winui::bindings::ListView| {
                use mitsuami::winui::windows_core::Interface;
                v.cast::<mitsuami::winui::bindings::IListViewBase>()?.SetSingleSelectionFollowsFocus(false)
            }),
            "WinUI: SingleSelectionFollowsFocus off lets the arrow keys move without selecting; Space selects.",
        ),
    };
    let selected = signal(Vec::<&'static str>::new());
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        List::new(|| FRUIT.to_vec(), |f: &&str| *f, |f| row(f.to_string()))
            .selected(selected)
            .list_style(ListStyle::Framed)
            .height(140)
            .native(tweak),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("List", WindowSize::FitHeight(640.0), || {
            ScrollView::new().child(Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((
                gallery(),
                playground(),
                platform_option(),
            )))
        })
        .run();
}
