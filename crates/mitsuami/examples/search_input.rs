//! Search input: `cargo run -p mitsuami --example search_input`.
//!
//! - A list of fruit, filtered as the platform asks for searches: once
//!   typing pauses on AppKit, GTK and Qt, at once on WinUI, on Return, and
//!   when the field is cleared.
//! - A playground: what's typed next to what was searched for, and how
//!   many searches came, with a switch to turn the field off.
//! - A raw platform setting, through `.native()`: when each platform
//!   searches is its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

const FRUIT: [&str; 12] = [
    "Apple",
    "Apricot",
    "Banana",
    "Blackberry",
    "Cherry",
    "Grape",
    "Lemon",
    "Mango",
    "Orange",
    "Peach",
    "Pear",
    "Plum",
];

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn find_fruit() -> impl View {
    let query = signal(String::new());
    let found = move || {
        let query = query.get().to_lowercase();
        FRUIT.iter().filter(|f| f.to_lowercase().contains(&query)).copied().collect::<Vec<_>>()
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Find a fruit")}
            <SearchInput a11y_label="Fruit" placeholder="Search fruit" @search=move |q| query.set(q)/>
            <Text>
                {move || match found() {
                    found if found.is_empty() => "No fruit matches".to_string(),
                    found => found.join(", "),
                }}
            </Text>
        </Column>
    }
}

/// What the field holds, and what the platform asked to search for.
fn playground() -> impl View {
    let text = signal(String::new());
    let searched = signal(None::<String>);
    let searches = signal(0);
    let enabled = signal(true);
    let search = move |q: String| {
        searched.set(Some(q));
        searches.update(|n| *n += 1);
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Search"</Text>
                <SearchInput a11y_label="Playground" bind=text enabled=enabled @search=search/>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
            </Grid>
            <Text text_style=TextStyle::Caption>
                {move || match searched.get() {
                    None => format!("Typed \"{}\"; no search yet", text.get()),
                    Some(q) => format!("Typed \"{}\"; searched for \"{q}\" ({} searches)", text.get(), searches.get()),
                }}
            </Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native field.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<SearchInput>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|f: &mitsuami::appkit::objc2_app_kit::NSSearchField| {
                f.setSendsSearchStringImmediately(true)
            }),
            "AppKit: sendsSearchStringImmediately searches on every keystroke, without waiting for a pause.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|e: &mitsuami::gtk::gtk::SearchEntry| e.set_search_delay(1000)),
            "GTK: search-delay waits a second after typing stops before it searches, not 150 ms.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|f: &mitsuami::kirigami::QmlObject| f.set_bool("delaySearch", true)),
            "Kirigami: delaySearch waits 2 seconds after typing stops before it searches, for slow searches.",
        ),
        windows => (
            mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::AutoSuggestBox| {
                use mitsuami::winui::bindings::{IAutoSuggestBox, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                s.cast::<IAutoSuggestBox>()?.SetHeader(&PropertyValue::CreateString("Find in mail")?)
            }),
            "WinUI: Header draws a caption above the search box, WinUI's own way to label it.",
        ),
    };
    let plain = signal(0);
    let tweaked = signal(0);
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <SearchInput a11y_label="Plain" placeholder="Plain" @search=move |_| plain.update(|n| *n += 1)/>
            <SearchInput a11y_label="Tweaked" placeholder="Tweaked" native=tweak @search=move |_| tweaked.update(|n| *n += 1)/>
            <Text text_style=TextStyle::Caption>
                {move || format!("Searches: {} plain, {} tweaked", plain.get(), tweaked.get())}
            </Text>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {find_fruit()}
            {playground()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Search input", WindowSize::FitHeight(560.0), page).run();
}
