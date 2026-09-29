//! Radio group: `cargo run -p mitsuami --example radio_group`.
//!
//! - A few groups: none chosen, one chosen, and disabled.
//! - A playground: edit the options, choose one, clear the choice, turn it
//!   off.
//! - A raw platform setting, through `.native()`. Radio groups have no
//!   semantic options past their options and choice: what platforms offer
//!   is each one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

/// A group under its caption: groups draw none, so the label is only the
/// accessible name.
fn labelled(caption: &'static str, group: RadioGroup) -> impl View {
    view! {
        <Column gap=Spacing::Xs>
            <Text>{caption}</Text>
            {group}
        </Column>
    }
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Radio groups")}
            <Row gap=Spacing::Xl align=Align::Start>
                {labelled("None chosen", view! { <RadioGroup label="None chosen" options=SIZES/> })}
                {labelled("Chosen", view! { <RadioGroup label="Chosen" options=SIZES selected=Some(1)/> })}
                {labelled("Disabled", view! { <RadioGroup label="Disabled" options=SIZES selected=Some(2) enabled=false/> })}
            </Row>
        </Column>
    }
}

/// One group, and its props to change.
fn playground() -> impl View {
    let text = signal("Red, Green, Blue".to_string());
    let options =
        move || text.get().split(',').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect::<Vec<_>>();
    let chosen = signal(None);
    let enabled = signal(true);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid columns=[Track::MaxContent, Track::Size(1.fr())] column_gap=Spacing::Md row_gap=Spacing::Sm align=Align::Center>
                <Text>"Options"</Text>
                <TextInput a11y_label="Options" placeholder="Comma-separated" bind=text/>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
            </Grid>
            <RadioGroup label="Color" options=options bind=chosen enabled=enabled/>
            <Row gap=Spacing::Md align=Align::Center>
                <Button @click=move || chosen.set(None)>"Clear"</Button>
                <Text text_style=TextStyle::Caption>
                    {move || match chosen.get().and_then(|i| options().get(i).cloned()) {
                        Some(option) => format!("Chosen: {option}"),
                        None => "None chosen".to_string(),
                    }}
                </Text>
            </Row>
        </Column>
    }
}

/// A setting only this platform has, straight on the native group.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<RadioGroup>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSStackView| {
                s.setOrientation(mitsuami::appkit::objc2_app_kit::NSUserInterfaceLayoutOrientation::Horizontal)
            }),
            "AppKit: a horizontal stack view puts the buttons side by side.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::Box| {
                use mitsuami::gtk::gtk::prelude::*;
                b.set_orientation(mitsuami::gtk::gtk::Orientation::Horizontal)
            }),
            "GTK: a horizontal box puts the buttons side by side.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|c: &mitsuami::kirigami::QmlObject| c.set_int("spacing", 0)),
            "Qt Quick: the column's spacing, here none.",
        ),
        windows => (
            mitsuami::winui::tweak(|r: &mitsuami::winui::bindings::RadioButtons| {
                use mitsuami::winui::bindings::{IRadioButtons, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                r.cast::<IRadioButtons>()?.SetHeader(&PropertyValue::CreateString("Size")?)
            }),
            "WinUI: Header draws a caption above the buttons, WinUI's own way to label them.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Xl align=Align::Start>
                <RadioGroup label="Plain" options=SIZES selected=Some(1)/>
                <RadioGroup label="Tweaked" options=SIZES selected=Some(1) native=tweak/>
            </Row>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {gallery()}
            {playground()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Radio group", WindowSize::FitHeight(560.0), page).run();
}
