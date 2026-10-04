//! Localization: `cargo run -p mitsuami --example l10n`.
//!
//! - The app is in the first of your languages it has (English, German,
//!   Brazilian Portuguese, Arabic or Japanese), else in English. The
//!   Language select switches it while it runs, as an in-app setting does.
//! - Plural forms follow each language's rules: the list counts files from
//!   0 to 8, then 11, which in Arabic takes five of its six forms.
//! - Dates, times and numbers are the platform's, written as your region
//!   settings write them, whatever the language.
//! - Arabic lays the window out right to left, and each control mirrors
//!   itself the platform's way. On macOS the title bar and menus mirror
//!   only when the app starts in Arabic (put it first in System Settings,
//!   or run with `-AppleLanguages '(ar)'`).
//! - mitsuami's own strings (the app menu on macOS, GNOME's and KDE's
//!   Quit, the alert's OK) are in the app's language: its translations
//!   give them.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::SystemTime;

use mitsuami::prelude::*;

/// Each language by its own name, as language settings list them.
const LANGUAGES: [(&str, &str); 5] =
    [("en-US", "English"), ("de", "Deutsch"), ("pt-BR", "Português (Brasil)"), ("ar", "العربية"), ("ja", "日本語")];

fn language_select() -> impl View {
    // 0 is the system's.
    let chosen = signal(0usize);
    watch(move || chosen.get(), |index, _| l10n::set_language(index.checked_sub(1).map(|i| LANGUAGES[i].0)));
    let options = move || {
        std::iter::once(tr!("system-language")).chain(LANGUAGES.iter().map(|(_, name)| name.to_string())).collect()
    };
    view! {
        <Row gap=Spacing::Sm align=Align::Center>
            <Text>{t!("language")}</Text>
            <Select label=t!("language") options=options bind=chosen/>
        </Row>
    }
}

fn messages() -> impl View {
    let name = signal("Ada".to_string());
    view! {
        <Group title=t!("greeting", name = name)>
            <Column gap=Spacing::Sm align=Align::Stretch>
                <TextInput a11y_label=t!("name") placeholder=t!("name") bind=name/>
                <Column gap=Spacing::Xs>
                    {(0..=8).chain([11]).map(|n: i64| view! { <Text>{t!("files", count = n)}</Text> }).collect::<Vec<_>>()}
                </Column>
            </Column>
        </Group>
    }
}

fn formats() -> impl View {
    let now = SystemTime::now();
    view! {
        <Group>
            <Column gap=Spacing::Xs>
                <Text>{t!("today", date = now)}</Text>
                <Text>{t!("now", date = now)}</Text>
                <Text>{t!("size", bytes = 1_234_567)}</Text>
                <Text>{t!("done", fraction = 0.425)}</Text>
                <Text>{t!("price", amount = 1234.5)}</Text>
            </Column>
        </Group>
    }
}

fn form() -> impl View {
    let plan = signal(Some(0));
    let saved = move || {
        spawn_local(async {
            // No buttons: the alert's OK is mitsuami's, in the app's language.
            alert(Alert::new(tr!("saved", count = 3))).await;
        });
    };
    view! {
        <Column gap=Spacing::Sm>
            <Checkbox>{t!("remember")}</Checkbox>
            <RadioGroup label=t!("plan") options={move || vec![tr!("plan-free"), tr!("plan-pro")]} bind=plan/>
            <Row justify=Justify::End>
                <Button role=ButtonRole::Default @click=saved>{t!("save")}</Button>
            </Row>
        </Column>
    }
}

pub fn page() -> impl View {
    // The About and Quit items are titled by the platform's rules, in the
    // app's language.
    set_menu(view! {
        <MenuBar>
            <Menu title=t!("menu-help")>
                <MenuItem role=MenuRole::About>{t!("about")}</MenuItem>
            </Menu>
        </MenuBar>
    });
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md align=Align::Stretch>
            {language_select()}
            {messages()}
            {formats()}
            {form()}
        </Column>
    }
}

fn main() {
    App::new()
        .id("br.com.roboport.mitsuami.Languages")
        .name("Languages")
        .locales(locales!("locales"))
        .window(t!("app-title"), WindowSize::FitHeight(420.0), page)
        .run();
}
