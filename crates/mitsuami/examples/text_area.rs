//! Text area: `cargo run -p mitsuami --example text_area`.
//!
//! - A feedback form: Return starts a new line in the message, and the
//!   button sends it.
//! - A playground: how many lines tall it is, whether its lines wrap,
//!   read-only, enabled, and what the text holds.
//! - A raw platform setting, through `.native()`. Lines, wrapping, read-only
//!   and a placeholder are the options text areas have in common (AppKit's and
//!   GTK's show no placeholder); the rest is each platform's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn feedback() -> impl View {
    let subject = signal(String::new());
    let message = signal(String::new());
    let sent = signal(None::<String>);
    let send = move || {
        sent.set(Some(subject.get_untracked()));
        message.set(String::new());
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Feedback")}
            <TextInput a11y_label="Subject" placeholder="Subject" bind=subject/>
            <TextArea a11y_label="Message" placeholder="What happened?" lines=5 bind=message/>
            <Row gap=Spacing::Md align=Align::Center>
                <Text text_style=TextStyle::Caption grow=1.0>
                    {move || match sent.get() {
                        Some(subject) if !subject.is_empty() => format!("Sent “{subject}”"),
                        Some(_) => "Sent".to_string(),
                        None => "Return starts a new line; the button sends it".to_string(),
                    }}
                </Text>
                <Button role=ButtonRole::Default enabled=move || !message.get().trim().is_empty() @click=send>
                    "Send"
                </Button>
            </Row>
        </Column>
    }
}

/// One area, and switches for what it can be.
fn playground() -> impl View {
    let text = signal(
        "Edit me.\nEach line ends in a newline, and a line longer than the area is wide wraps, unless it's told not to."
            .to_string(),
    );
    let lines = signal(3);
    let wrap = signal(true);
    let read_only = signal(false);
    let enabled = signal(true);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <TextArea a11y_label="Playground" bind=text lines=move || lines.get() as u32
                line_wrap=wrap read_only=read_only enabled=enabled/>
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Lines"</Text>
                <NumberInput label="Lines" range_with=(1, 12) bind=lines/>
                <Text>"Wraps"</Text>
                <Switch bind=wrap>"Wraps"</Switch>
                <Text>"Read-only"</Text>
                <Switch bind=read_only>"Read-only"</Switch>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
            </Grid>
            <Text text_style=TextStyle::Caption>
                {move || {
                    let text = text.get();
                    format!("{} lines, {} words, {} characters",
                        text.lines().count(), text.split_whitespace().count(), text.chars().count())
                }}
            </Text>
        </Column>
    }
}

const SAMPLE: &str = "A line long enough to wrap, or not, as the platform sets it: recieve, teh.\nA second line.";

/// A setting only this platform has, straight on the native text area.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<TextArea>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTextView| {
                t.setContinuousSpellCheckingEnabled(true)
            }),
            "AppKit: continuous spell checking underlines words as you type.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|v: &mitsuami::gtk::gtk::TextView| {
                use mitsuami::gtk::gtk::prelude::*;
                v.set_monospace(true)
            }),
            "GTK: monospace sets the text in the fixed-width font.",
        ),
        kde => (
            // TextEdit.WrapAnywhere.
            mitsuami::kirigami::tweak(|a: &mitsuami::kirigami::QmlObject| a.set_int("wrapMode", 3)),
            "Qt Quick: wrapMode WrapAnywhere breaks lines anywhere, even inside a word, instead of between words.",
        ),
        windows => (
            mitsuami::winui::tweak(|t: &mitsuami::winui::bindings::TextBox| {
                use mitsuami::winui::bindings::{ITextBox, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                t.cast::<ITextBox>()?.SetHeader(&PropertyValue::CreateString("Message")?)
            }),
            "WinUI: Header draws a caption above the text box, WinUI's own way to label it.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <TextArea a11y_label="Plain" value=SAMPLE/>
            <TextArea a11y_label="Tweaked" value=SAMPLE native=tweak/>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {feedback()}
            {playground()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Text area", WindowSize::FitHeight(560.0), page).run();
}
