//! Checkbox: `cargo run -p mitsuami --example checkbox`.
//!
//! - Every state, enabled and disabled: what the semantic props look like
//!   on this platform.
//! - A playground: pick the label, and whether it's checked, mixed and
//!   enabled. Click the box to see where it lands from the mixed state.
//! - "Select all": what the mixed state is for.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

/// Unchecked, checked and mixed, then the same disabled.
fn gallery() -> impl View {
    let row = |enabled: bool| {
        view! {
            <Row gap=Spacing::Lg>
                <Checkbox enabled=enabled>"Unchecked"</Checkbox>
                <Checkbox checked=true enabled=enabled>"Checked"</Checkbox>
                <Checkbox mixed=true enabled=enabled>"Mixed"</Checkbox>
            </Row>
        }
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("States")}
            {row(true)}
            {row(false)}
        </Column>
    }
}

/// One checkbox, and its props to change.
fn playground() -> impl View {
    let label = signal("Show hidden files".to_string());
    let checked = signal(false);
    let mixed = signal(true);
    let enabled = signal(true);
    let last = signal(None::<bool>);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid columns=[Track::MaxContent, Track::Size(1.fr())] column_gap=Spacing::Md row_gap=Spacing::Sm align=Align::Center>
                <Text>"Label"</Text>
                <TextInput a11y_label="Label" bind=label/>
                <Text>"Checked"</Text>
                <Switch bind=checked>"Checked"</Switch>
                <Text>"Mixed"</Text>
                <Switch bind=mixed>"Mixed"</Switch>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
            </Grid>
            <Row gap=Spacing::Md align=Align::Center>
                // A click leaves the mixed state wherever the platform lands.
                <Checkbox checked=checked mixed=mixed enabled=enabled @change=move |value| {
                    checked.set(value);
                    mixed.set(false);
                    last.set(Some(value));
                }>
                    {label}
                </Checkbox>
                <Text text_style=TextStyle::Caption>
                    {move || match last.get() {
                        None => "Not clicked yet".to_string(),
                        Some(true) => "Last click checked it".to_string(),
                        Some(false) => "Last click unchecked it".to_string(),
                    }}
                </Text>
            </Row>
        </Column>
    }
}

/// A box that checks the others: mixed while only some are.
fn select_all() -> impl View {
    let names = ["Apples", "Pears", "Plums"];
    let fruit = [signal(true), signal(false), signal(false)];
    let all = move || fruit.iter().all(|f| f.get());
    let some = move || fruit.iter().any(|f| f.get()) && !all();
    view! {
        <Column gap=Spacing::Sm>
            {heading("Select all")}
            <Checkbox checked=all mixed=some @change=move |checked| {
                fruit.iter().for_each(|f| f.set(checked));
            }>
                "All fruit"
            </Checkbox>
            <Column padding_x=Spacing::Xl gap=Spacing::Sm>
                {names.iter().zip(fruit).map(|(name, f)| view! { <Checkbox bind=f>{*name}</Checkbox> }).collect::<Vec<_>>()}
            </Column>
        </Column>
    }
}

/// A setting only this platform has, straight on the native checkbox.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Checkbox>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSButton| {
                b.setImagePosition(mitsuami::appkit::objc2_app_kit::NSCellImagePosition::ImageTrailing)
            }),
            "AppKit: imagePosition trailing puts the box after its label.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::CheckButton| {
                use mitsuami::gtk::gtk::prelude::*;
                b.add_css_class("selection-mode")
            }),
            "GTK: the selection-mode style class, which themes draw as a round check.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_real("spacing", 24.0)),
            "Qt Quick: a spacing of 24 puts more room between the box and its label.",
        ),
        windows => (
            mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::CheckBox| {
                use mitsuami::winui::bindings::{CornerRadius, IControl};
                use mitsuami::winui::windows_core::Interface;
                let round = CornerRadius { top_left: 10.0, top_right: 10.0, bottom_right: 10.0, bottom_left: 10.0 };
                b.cast::<IControl>()?.SetCornerRadius(round)
            }),
            "WinUI: a CornerRadius of 10 makes the box round.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Lg align=Align::Center>
                <Checkbox checked=true>"Plain"</Checkbox>
                <Checkbox checked=true native=tweak>"Tweaked"</Checkbox>
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
            {select_all()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Checkbox", WindowSize::FitHeight(640.0), page).run();
}
