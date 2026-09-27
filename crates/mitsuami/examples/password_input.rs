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
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
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
    view! {
        <Column gap=Spacing::Md>
            {heading("Sign in")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"User"</Text>
                <TextInput a11y_label="User" placeholder="Name or email" bind=user/>
                <Text>"Password"</Text>
                <PasswordInput a11y_label="Password" placeholder="Required" bind=password @submit=submit/>
            </Grid>
            <Row gap=Spacing::Md align=Align::Center>
                <Text text_style=TextStyle::Caption grow=1.0>
                    {move || match signed_in.get() {
                        Some(user) if !user.is_empty() => format!("Signed in as {user}"),
                        Some(_) => "Signed in".to_string(),
                        None => "Press Return in the password field to sign in".to_string(),
                    }}
                </Text>
                <Button role=ButtonRole::Default enabled=move || !password.get().is_empty() @click=submit>
                    "Sign in"
                </Button>
            </Row>
        </Column>
    }
}

/// Two fields, and what they say together.
fn playground() -> impl View {
    let new = signal(String::new());
    let confirm = signal(String::new());
    let enabled = signal(true);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"New"</Text>
                <PasswordInput a11y_label="New password" bind=new enabled=enabled/>
                <Text>"Confirm"</Text>
                <PasswordInput a11y_label="Confirm password" bind=confirm enabled=enabled/>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
            </Grid>
            <Text text_style=TextStyle::Caption>
                {move || {
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
                }}
            </Text>
        </Column>
    }
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
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <PasswordInput a11y_label="Plain" value="correct horse"/>
            <PasswordInput a11y_label="Tweaked" value="correct horse" native=tweak/>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Password input", WindowSize::FitHeight(560.0), || {
            view! {
                <Column padding=Spacing::Xl gap=Spacing::Xl>
                    {sign_in()}
                    {playground()}
                    {platform_option()}
                </Column>
            }
        })
        .run();
}
