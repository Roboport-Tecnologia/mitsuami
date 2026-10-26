//! ColorWell: `cargo run -p mitsuami --example color_well`.
//!
//! - Colour wells (a few colours, disabled), as each platform draws them.
//! - A playground: pick a colour, or set one from the app (which reports
//!   nothing), and turn the well off.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Colour wells")}
            <Grid
                columns=[Track::MaxContent, Track::MaxContent]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Blue"</Text>
                <ColorWell label="Blue" color=Color::rgb(0, 120, 215)/>
                <Text>"Orange"</Text>
                <ColorWell label="Orange" color=Color::rgb(255, 140, 0)/>
                <Text>"Disabled"</Text>
                <ColorWell label="Disabled" color=Color::rgb(16, 124, 16) enabled=false/>
            </Grid>
        </Column>
    }
}

/// One well, bound to the app's colour, which the buttons set.
fn playground() -> impl View {
    let color = signal(Color::rgb(0, 120, 215));
    let enabled = signal(true);
    let picks = signal(0);
    let hex = move || color.get().hex().unwrap_or_default();
    let set = move |c: Color| move || color.set(c);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Row gap=Spacing::Md align=Align::Center>
                <ColorWell
                    label="Colour"
                    color=color
                    enabled=enabled
                    on_change=move |c| {
                        color.set(c);
                        picks.update(|n| *n += 1);
                    }
                />
                <Text weight=FontWeight::Bold color=color>{hex}</Text>
            </Row>
            <Row gap=Spacing::Sm align=Align::Center>
                <Text>"Set from the app:"</Text>
                <Button on_click=set(Color::rgb(232, 17, 35))>"Red"</Button>
                <Button on_click=set(Color::rgb(0, 204, 106))>"Green"</Button>
                <Button on_click=set(Color::rgb(107, 105, 214))>"Purple"</Button>
            </Row>
            <Switch bind=enabled>"Enabled"</Switch>
            // The app's sets aren't picks: only the user's count.
            <Text text_style=TextStyle::Caption>{move || format!("Picked {} time(s)", picks.get())}</Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native well.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<ColorWell>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|w: &mitsuami::appkit::objc2_app_kit::NSColorWell| {
                w.setColorWellStyle(mitsuami::appkit::objc2_app_kit::NSColorWellStyle::Expanded)
            }),
            "AppKit: the expanded style adds a button for the colour panel beside the swatch.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::ColorDialogButton| {
                if let Some(dialog) = b.dialog() {
                    dialog.set_title("Pick a Highlight");
                }
            }),
            "GTK: the colour dialog takes the app's title.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_bool("flat", true)),
            "Qt Quick: a flat button shows only its swatch until the pointer is over it.",
        ),
        windows => (
            mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::DropDownButton| {
                use mitsuami::winui::bindings::IControl;
                use mitsuami::winui::windows_core::Interface;
                b.cast::<IControl>()?.SetCornerRadius(mitsuami::winui::bindings::CornerRadius {
                    top_left: 16.0,
                    top_right: 16.0,
                    bottom_right: 16.0,
                    bottom_left: 16.0,
                })
            }),
            "WinUI: a corner radius makes the button a pill.",
        ),
    };
    let color = signal(Color::rgb(255, 185, 0));
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Md align=Align::Center>
                <ColorWell label="Highlight" bind=color native=tweak/>
                <Text>{move || color.get().hex().unwrap_or_default()}</Text>
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
    App::new().window("ColorWell", WindowSize::FitHeight(480.0), page).run();
}
