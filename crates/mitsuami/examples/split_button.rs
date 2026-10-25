//! Split buttons: `cargo run -p mitsuami --example split_button`.
//!
//! Click a button for its own action, or its arrow for a menu of others,
//! as the platform shows them (a combo button on macOS, a split button on
//! GNOME and Windows, a button with an arrow button joined to it on KDE).
//!
//! - A gallery: a caption, an icon before it, the icon alone, borderless,
//!   and disabled. Each platform draws its own arrow.
//! - A rebase stopped at a commit: Continue applies the next one; its menu
//!   commits the message typed as the commit to edit (only once there's
//!   one), skips the commit, or aborts. The log under it says what
//!   happened.
//! - A raw platform setting, through `.native()`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

fn plus() -> &'static str {
    platform! {
        macos => "plus",
        gtk => "list-add-symbolic",
        kde => "list-add",
        windows => "\u{E710}",
    }
}

fn gallery() -> impl View {
    let menu = || (MenuItem::new("Folder…"), MenuItem::new("Disc image…"));
    view! {
        <Column gap=Spacing::Md>
            {heading("Split buttons")}
            <Row gap=Spacing::Md align=Align::Center>
                <SplitButton menu=menu()>"Add"</SplitButton>
                <SplitButton icon=plus() menu=menu()>"Add"</SplitButton>
                <SplitButton icon=plus() icon_only=true tooltip="Add" menu=menu()>"Add"</SplitButton>
                <SplitButton icon=plus() button_style=ButtonStyle::Borderless menu=menu()>"Add"</SplitButton>
                <SplitButton enabled=false menu=menu()>"Add"</SplitButton>
            </Row>
        </Column>
    }
}

/// The commits the rebase applies, in order.
const COMMITS: [&str; 4] = ["Add the parser", "Fix a typo", "Speed up the lexer", "Update the docs"];

/// A rebase stopped at a commit, as a Git client shows one.
fn rebase() -> impl View {
    // The commit it stopped at; past the end, it's done.
    let at = signal(1usize);
    let message = signal(String::new());
    let log = signal(String::from("Stopped at “Fix a typo”"));
    let done = move || at.get() >= COMMITS.len();
    let next = move |what: String| {
        log.set(what);
        at.update(|i| *i += 1);
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A rebase")}
            <Text>
                {move || match COMMITS.get(at.get()) {
                    Some(subject) => format!("Rebasing: commit {} of {}, “{subject}”", at.get() + 1, COMMITS.len()),
                    None => "The rebase is done".to_string(),
                }}
            </Text>
            <Row gap=Spacing::Md align=Align::Center>
                <TextInput grow=1.0 placeholder="A new message for this commit" value=message enabled=move || !done() />
                <SplitButton
                    enabled=move || !done()
                    on_click=move || next(format!("Applied “{}”", COMMITS[at.get_untracked()]))
                    menu_with=move || (
                        MenuItem::new("Commit to Edit").enabled(!message.get().trim().is_empty()).on_select(move || {
                            next(format!("Committed “{}”", message.get_untracked().trim()));
                            message.set(String::new());
                        }),
                        MenuItem::new("Skip").on_select(move || next(format!("Skipped “{}”", COMMITS[at.get_untracked()]))),
                        MenuSeparator,
                        MenuItem::new("Abort").on_select(move || {
                            at.set(1);
                            message.set(String::new());
                            log.set("Aborted, and started again at “Fix a typo”".into());
                        }),
                    )
                >"Continue"</SplitButton>
            </Row>
            <Text text_style=TextStyle::Caption>{log}</Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native button.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<SplitButton>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|c: &mitsuami::appkit::objc2_app_kit::NSControl| {
                use mitsuami::appkit::objc2_app_kit::{NSComboButton, NSComboButtonStyle};
                if let Some(combo) = c.downcast_ref::<NSComboButton>() {
                    combo.setStyle(NSComboButtonStyle::Unified);
                }
            }),
            "AppKit: the unified style, one segment whose menu opens when it's held down.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::adw::SplitButton| b.set_direction(mitsuami::gtk::gtk::ArrowType::Up)),
            "GTK: direction up opens the popover above the button, and turns its arrow.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_bool("flat", true)),
            "Qt Quick: flat draws the buttons without a frame until hovered.",
        ),
        windows => (
            mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::SplitButton| {
                use mitsuami::winui::bindings::IControl;
                use mitsuami::winui::windows_core::Interface;
                b.cast::<IControl>()?.SetCornerRadius(mitsuami::winui::bindings::CornerRadius {
                    top_left: 16.0,
                    top_right: 16.0,
                    bottom_right: 16.0,
                    bottom_left: 16.0,
                })
            }),
            "WinUI: CornerRadius rounds the button into a pill.",
        ),
    };
    let menu = || (MenuItem::new("Folder…"), MenuItem::new("Disc image…"));
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Lg align=Align::Center>
                <SplitButton menu=menu()>"Plain"</SplitButton>
                <SplitButton native=tweak menu=menu()>"Tweaked"</SplitButton>
            </Row>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {gallery()}
            {rebase()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Split buttons", WindowSize::FitHeight(560.0), page).run();
}
