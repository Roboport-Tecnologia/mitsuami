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
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
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
    view! {
        <Row padding_x=Spacing::Md padding_y=Spacing::Xs>
            <Text>{text}</Text>
        </Row>
    }
}

fn fruit_list(style: ListStyle) -> List<&'static str, &'static str> {
    view! {
        <List each=|| FRUIT.to_vec() key=|f: &&str| *f list_style=style height=140 grow=1.0 let:f>
            {row(f.to_string())}
        </List>
    }
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Styles")}
            <Row gap=Spacing::Lg>
                <Column gap=Spacing::Sm grow=1.0>
                    <Text text_style=TextStyle::Caption>"Plain"</Text>
                    {fruit_list(ListStyle::Plain)}
                </Column>
                <Column gap=Spacing::Sm grow=1.0>
                    <Text text_style=TextStyle::Caption>"Framed"</Text>
                    {fruit_list(ListStyle::Framed)}
                </Column>
            </Row>
        </Column>
    }
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
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Row gap=Spacing::Sm>
                <Button @click=add>"Add"</Button>
                <Button enabled=move || !selected.get().is_empty() @click=remove>"Remove selected"</Button>
                <Button @click=move || {
                    if let Some(last) = rows.get_untracked().last() {
                        scroll_handle.scroll_to(last);
                    }
                }>"Scroll to last"</Button>
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Text>"Select several"</Text>
                <Switch bind=several>"Select several"</Switch>
            </Row>
            <List
                each=rows
                key=|n: &u32| *n
                selection_mode=move || if several.get() { SelectionMode::Multiple } else { SelectionMode::Single }
                selected=selected
                @activate=move |n| activated.set(Some(n))
                handle=handle
                list_style=ListStyle::Framed
                height=160
                let:n
            >
                {row(format!("Row {n}"))}
            </List>
            <Text text_style=TextStyle::Caption>
                {move || {
                    let selected = selected.get();
                    let activated = activated.get().map_or("none".to_string(), |n| format!("Row {n}"));
                    format!("{} of {} selected; last activated: {activated}", selected.len(), rows.get().len())
                }}
            </Text>
        </Column>
    }
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
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <List
                each=|| FRUIT.to_vec()
                key=|f: &&str| *f
                selected=selected
                list_style=ListStyle::Framed
                height=140
                native=tweak
                let:f
            >
                {row(f.to_string())}
            </List>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("List", WindowSize::FitHeight(640.0), || {
            view! {
                <ScrollView>
                    <Column padding=Spacing::Xl gap=Spacing::Xl>
                        {gallery()}
                        {playground()}
                        {platform_option()}
                    </Column>
                </ScrollView>
            }
        })
        .run();
}
