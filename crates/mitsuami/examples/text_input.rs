//! Text input: `cargo run -p mitsuami --example text_input`.
//!
//! - A few fields: empty with a placeholder, with text, read-only,
//!   disabled, and one with more text than fits.
//! - Checked fields: an email address and a port, each with an error
//!   while what's typed isn't one, the message under it. GTK marks the
//!   field red; elsewhere the message is the mark, and assistive
//!   technology reads the field as invalid everywhere.
//! - A playground: type, change the placeholder, say what it's for, make
//!   it read-only or turn it off, press Return to submit.
//! - A raw platform setting, through `.native()`. Read-only and what it's
//!   for are the options every platform's text field has: the rest is each
//!   one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

/// What a field can be for: the purposes every platform has, which pick an
/// on-screen keyboard (GTK, WinUI, Qt) or autofill (AppKit).
const PURPOSES: [(&str, InputPurpose); 4] = [
    ("Text", InputPurpose::Text),
    ("Email", InputPurpose::Email),
    ("URL", InputPurpose::Url),
    ("Phone", InputPurpose::Phone),
];

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Fields")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Empty"</Text>
                <TextInput a11y_label="Empty" placeholder="A placeholder"/>
                <Text>"With text"</Text>
                <TextInput a11y_label="With text" value="Ada Lovelace"/>
                // Selectable and copyable, and read as usual.
                <Text>"Read-only"</Text>
                <TextInput a11y_label="Read-only" value="ABCD-1234-EFGH" read_only=true/>
                <Text>"Disabled"</Text>
                <TextInput a11y_label="Disabled" value="Can't edit this" enabled=false/>
                // Scrolls as the platform scrolls it.
                <Text>"Long"</Text>
                <TextInput
                    a11y_label="Long"
                    value="A line of text much longer than the field is wide, to scroll through"
                />
            </Grid>
        </Column>
    }
}

/// What's wrong with an email address, if anything: an empty field isn't
/// wrong yet.
fn email_error(text: &str) -> Option<String> {
    let valid = text.split_once('@').is_some_and(|(user, host)| !user.is_empty() && host.contains('.'));
    (!text.is_empty() && !valid).then(|| "An email address has a name, an @ and a domain".to_string())
}

fn port_error(text: &str) -> Option<String> {
    (!text.is_empty() && !text.parse::<u16>().is_ok_and(|port| port > 0))
        .then(|| "A port is a number from 1 to 65535".to_string())
}

/// Fields that say what's wrong with what's typed.
fn checked() -> impl View {
    let email = signal(String::new());
    let port = signal("8080x".to_string());
    let email_error = move || email_error(&email.get());
    let port_error = move || port_error(&port.get());
    view! {
        <Column gap=Spacing::Md>
            {heading("Checked")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Email"</Text>
                <TextInput a11y_label="Email" placeholder="name@example.com" input_purpose=InputPurpose::Email
                    bind=email error=email_error/>
                // The message under the field, while there's one.
                <Text hidden=move || email_error().is_none()/>
                <Text text_style=TextStyle::Caption hidden=move || email_error().is_none()>
                    {move || email_error().unwrap_or_default()}
                </Text>
                <Text>"Port"</Text>
                <TextInput a11y_label="Port" bind=port error=port_error/>
                <Text hidden=move || port_error().is_none()/>
                <Text text_style=TextStyle::Caption hidden=move || port_error().is_none()>
                    {move || port_error().unwrap_or_default()}
                </Text>
            </Grid>
        </Column>
    }
}

/// One field, and its props to change.
fn playground() -> impl View {
    let text = signal(String::new());
    let placeholder = signal("Your name".to_string());
    let read_only = signal(false);
    let enabled = signal(true);
    let purpose = signal(0);
    let submitted = signal(None::<String>);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Placeholder"</Text>
                <TextInput a11y_label="Placeholder" bind=placeholder/>
                <Text>"For"</Text>
                <Row><Select label="For" options=PURPOSES.map(|(name, _)| name) bind=purpose/></Row>
                <Text>"Read-only"</Text>
                <Switch bind=read_only>"Read-only"</Switch>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
                <Text>"Name"</Text>
                <TextInput
                    a11y_label="Name"
                    placeholder=placeholder
                    bind=text
                    read_only=read_only
                    input_purpose=move || PURPOSES[purpose.get()].1
                    enabled=enabled
                    @submit=move || submitted.set(Some(text.get_untracked()))
                />
            </Grid>
            <Text text_style=TextStyle::Caption>
                {move || {
                    let typed = format!("{} characters", text.get().chars().count());
                    match submitted.get() {
                        Some(name) => format!("{typed}; submitted \"{name}\""),
                        None => format!("{typed}; press Return to submit"),
                    }
                }}
            </Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native field.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<TextInput>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|f: &mitsuami::appkit::objc2_app_kit::NSTextField| f.setBezeled(false)),
            "AppKit: bezeled off draws the field without its border, as fields edited in place are.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|e: &mitsuami::gtk::gtk::Entry| {
                use mitsuami::gtk::gtk::prelude::*;
                e.set_icon_from_icon_name(mitsuami::gtk::gtk::EntryIconPosition::Primary, Some("system-search-symbolic"))
            }),
            "GTK: an entry can show icons at either end; this one a search icon.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|f: &mitsuami::kirigami::QmlObject| f.set_int("maximumLength", 8)),
            "Qt Quick: maximumLength stops the field at 8 characters: type to see.",
        ),
        windows => (
            mitsuami::winui::tweak(|f: &mitsuami::winui::bindings::TextBox| {
                use mitsuami::winui::bindings::{ITextBox, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                f.cast::<ITextBox>()?.SetHeader(&PropertyValue::CreateString("Name")?)
            }),
            "WinUI: Header draws a caption above the text box, WinUI's own way to label it.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <TextInput a11y_label="Plain" placeholder="Plain"/>
            <TextInput a11y_label="Tweaked" placeholder="Tweaked" native=tweak/>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {gallery()}
            {checked()}
            {playground()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Text input", WindowSize::FitHeight(560.0), page).run();
}
