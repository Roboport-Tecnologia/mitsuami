//! Select: `cargo run -p mitsuami --example select`.
//!
//! - A few selects: the first option chosen, another chosen, disabled, and
//!   one with a long option (AppKit sizes pop-ups for their widest option,
//!   the others for the chosen one).
//! - A playground: edit the options, choose one, turn it off.
//! - A raw platform setting, through `.native()`. Selects have no semantic
//!   options past their options and choice: what platforms offer is each
//!   one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

/// A select beside its caption: selects draw none, so the label is only
/// the accessible name.
fn labelled(caption: &'static str, select: Select) -> (Text, impl View) {
    (view! { <Text>{caption}</Text> }, view! { <Row>{select}</Row> })
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Selects")}
            <Grid columns=[Track::MaxContent, Track::Size(1.fr())] column_gap=Spacing::Md row_gap=Spacing::Sm align=Align::Center>
                {labelled("First option", view! { <Select label="First option" options=SIZES/> })}
                {labelled("Chosen", view! { <Select label="Chosen" options=SIZES selected=2/> })}
                {labelled("Disabled", view! { <Select label="Disabled" options=SIZES selected=1 enabled=false/> })}
                {labelled("A long option", view! { <Select label="A long option" options=["Short", "A much, much longer option"]/> })}
            </Grid>
        </Column>
    }
}

/// One select, and its props to change.
fn playground() -> impl View {
    let text = signal("Red, Green, Blue".to_string());
    let options =
        move || text.get().split(',').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect::<Vec<_>>();
    let chosen = signal(0);
    let enabled = signal(true);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid columns=[Track::MaxContent, Track::Size(1.fr())] column_gap=Spacing::Md row_gap=Spacing::Sm align=Align::Center>
                <Text>"Options"</Text>
                <TextInput a11y_label="Options" placeholder="Comma-separated" bind=text/>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
                <Text>"Color"</Text>
                <Row><Select label="Color" options=options bind=chosen enabled=enabled/></Row>
            </Grid>
            <Text text_style=TextStyle::Caption>
                {move || match options().get(chosen.get()) {
                    Some(option) => format!("Chosen: {option} (option {})", chosen.get() + 1),
                    None => "No options".to_string(),
                }}
            </Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native select.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Select>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|p: &mitsuami::appkit::objc2_app_kit::NSPopUpButton| p.setBordered(false)),
            "AppKit: bordered off makes a borderless pop-up, as in toolbars and \"Sort by\" menus.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|d: &mitsuami::gtk::gtk::DropDown| d.set_enable_search(true)),
            "GTK: enable-search puts a search entry in the pop-up: open it to see.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|c: &mitsuami::kirigami::QmlObject| c.set_bool("flat", true)),
            "Qt Quick: flat draws the combo box without a frame until hovered.",
        ),
        windows => (
            mitsuami::winui::tweak(|c: &mitsuami::winui::bindings::ComboBox| {
                use mitsuami::winui::bindings::{IComboBox, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                c.cast::<IComboBox>()?.SetHeader(&PropertyValue::CreateString("Size")?)
            }),
            "WinUI: Header draws a caption above the combo box, WinUI's own way to label it.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Lg align=Align::Center>
                <Select label="Plain" options=SIZES selected=1/>
                <Select label="Tweaked" options=SIZES selected=1 native=tweak/>
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
    App::new().window("Select", WindowSize::FitHeight(560.0), page).run();
}
