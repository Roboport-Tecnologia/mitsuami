//! Password input: `cargo run -p mitsuami --example password_input`.
//!
//! - A sign-in form: Return in the password field signs in.
//! - A playground: a new password and its confirmation, which must match,
//!   and a switch to turn the fields off.
//! - A raw platform setting, through `.native()`. Password fields have no
//!   semantic options past a text field's: how each platform hides the
//!   text, and whether it can show it, is its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

fn sign_in() -> impl View {
    let user = signal(String::new());
    let password = signal(String::new());
    let signed_in = signal(None::<String>);
    let submit = move || {
        if !password.get_untracked().is_empty() {
            signed_in.set(Some(user.get_untracked()));
        }
    };
    Column::new().gap(Spacing::Md).children((
        heading("Sign in"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("User"),
                TextInput::new().a11y_label("User").placeholder("Name or email").bind(user),
                Text::new("Password"),
                PasswordInput::new().a11y_label("Password").placeholder("Required").bind(password).on_submit(submit),
            )),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Text::new(move || match signed_in.get() {
                Some(user) if !user.is_empty() => format!("Signed in as {user}"),
                Some(_) => "Signed in".to_string(),
                None => "Press Return in the password field to sign in".to_string(),
            })
            .text_style(TextStyle::Caption)
            .grow(1.0),
            Button::new("Sign in")
                .role(ButtonRole::Default)
                .enabled(move || !password.get().is_empty())
                .on_click(submit),
        )),
    ))
}

/// Two fields, and what they say together.
fn playground() -> impl View {
    let new = signal(String::new());
    let confirm = signal(String::new());
    let enabled = signal(true);
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("New"),
                PasswordInput::new().a11y_label("New password").bind(new).enabled(enabled),
                Text::new("Confirm"),
                PasswordInput::new().a11y_label("Confirm password").bind(confirm).enabled(enabled),
                Text::new("Enabled"),
                Switch::new("Enabled").bind(enabled),
            )),
        Text::new(move || {
            let (new, confirm) = (new.get(), confirm.get());
            let length = new.chars().count();
            if new.is_empty() {
                "Type a new password".to_string()
            } else if confirm.is_empty() {
                format!("{length} characters; now confirm it")
            } else if new == confirm {
                format!("{length} characters; they match")
            } else {
                format!("{length} characters; they don't match")
            }
        })
        .text_style(TextStyle::Caption),
    ))
}

/// A setting only this platform has, straight on the native field.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<PasswordInput>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|f: &mitsuami::appkit::objc2_app_kit::NSSecureTextField| {
                use mitsuami::appkit::objc2_app_kit::NSSecureTextFieldCell;
                if let Some(cell) = f.cell().and_then(|c| c.downcast::<NSSecureTextFieldCell>().ok()) {
                    cell.setEchosBullets(false);
                }
            }),
            "AppKit: echosBullets off shows nothing as you type, not even bullets.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|e: &mitsuami::gtk::gtk::PasswordEntry| e.set_show_peek_icon(true)),
            "GTK: show-peek-icon adds the eye button that shows the password.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|f: &mitsuami::kirigami::QmlObject| f.set_bool("showPassword", true)),
            "Kirigami: showPassword starts with the password shown; the eye button hides it.",
        ),
        windows => (
            mitsuami::winui::tweak(|f: &mitsuami::winui::bindings::PasswordBox| {
                use mitsuami::winui::windows_core::Interface;
                f.cast::<mitsuami::winui::bindings::IPasswordBox>()?.SetPasswordChar("*")
            }),
            "WinUI: PasswordChar hides the text behind asterisks instead of dots.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        PasswordInput::new().a11y_label("Plain").value("correct horse"),
        PasswordInput::new().a11y_label("Tweaked").value("correct horse").native(tweak),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Password input", WindowSize::FitHeight(560.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((sign_in(), playground(), platform_option()))
        })
        .run();
}
