//! Switch: `cargo run -p mitsuami --example switch`.
//!
//! - Off and on, enabled and disabled. Switches draw no caption on most
//!   platforms, so each sits beside a `Text`; the label is its accessible
//!   name.
//! - A playground: the switch, and its props to change.
//! - A raw platform setting, through `.native()`. Switches have no semantic
//!   options past on and off: what platforms offer is each one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

/// A caption and a switch, as settings screens pair them.
fn setting(caption: impl IntoValue<String>, switch: Switch) -> impl View {
    view! {
        <Row gap=Spacing::Md align=Align::Center>
            <Text grow=1.0>{caption}</Text>
            {switch}
        </Row>
    }
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Sm>
            {heading("States")}
            {setting("Off", view! { <Switch>"Off"</Switch> })}
            {setting("On", view! { <Switch checked=true>"On"</Switch> })}
            {setting("Off, disabled", view! { <Switch enabled=false>"Off, disabled"</Switch> })}
            {setting("On, disabled", view! { <Switch checked=true enabled=false>"On, disabled"</Switch> })}
        </Column>
    }
}

/// One switch, and its props to change.
fn playground() -> impl View {
    let label = signal("Wi-Fi".to_string());
    let on = signal(true);
    let enabled = signal(true);
    let flips = signal(0);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid columns=[Track::MaxContent, Track::Size(1.fr())] column_gap=Spacing::Md row_gap=Spacing::Sm align=Align::Center>
                <Text>"Label"</Text>
                <TextInput a11y_label="Label" bind=label/>
                <Text>"On"</Text>
                <Checkbox bind=on>"On"</Checkbox>
                <Text>"Enabled"</Text>
                <Checkbox bind=enabled>"Enabled"</Checkbox>
            </Grid>
            {setting(label, view! {
                <Switch bind=on enabled=enabled @change=move |_| flips.update(|f| *f += 1)>{label}</Switch>
            })}
            <Text text_style=TextStyle::Caption>
                {move || match flips.get() {
                    0 => "Not flipped yet".to_string(),
                    1 => "Flipped once".to_string(),
                    n => format!("Flipped {n} times"),
                }}
            </Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native switch.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Switch>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSSwitch| {
                s.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Small)
            }),
            "AppKit: controlSize small, as macOS uses in dense settings lists.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Switch| {
                use mitsuami::gtk::gtk::{glib, prelude::*};
                // Once: a tweak runs again whenever the props change.
                if s.widget_name() != "delayed" {
                    s.set_widget_name("delayed");
                    s.connect_state_set(|s, on| {
                        let s = s.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || s.set_state(on));
                        glib::Propagation::Stop
                    });
                }
            }),
            "GTK: state-set delays the switch's state by a second after it's flipped, as GNOME shows a \
             setting that takes time to apply.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_str("text", "Tweaked")),
            "Qt Quick: a Switch draws its own text beside the track.",
        ),
        windows => (
            mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::ToggleSwitch| {
                use mitsuami::winui::bindings::{IToggleSwitch, PropertyValue};
                use mitsuami::winui::windows_core::Interface;
                let s = s.cast::<IToggleSwitch>()?;
                s.SetOnContent(&PropertyValue::CreateString("On")?)?;
                s.SetOffContent(&PropertyValue::CreateString("Off")?)
            }),
            "WinUI: OnContent and OffContent say On or Off beside the track.",
        ),
    };
    view! {
        <Column gap=Spacing::Sm>
            {heading("A platform option")}
            {setting("Plain", view! { <Switch checked=true>"Plain"</Switch> })}
            {setting("Tweaked", view! { <Switch checked=true native=tweak>"Tweaked"</Switch> })}
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Switch", WindowSize::FitHeight(560.0), || {
            view! {
                <Column padding=Spacing::Xl gap=Spacing::Xl>
                    {gallery()}
                    {playground()}
                    {platform_option()}
                </Column>
            }
        })
        .run();
}
