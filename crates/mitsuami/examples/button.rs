//! Button: `cargo run -p mitsuami --example button`.
//!
//! - Every role in every style, and disabled: what the semantic props look
//!   like on this platform.
//! - A playground: pick the role, style, label and whether it's enabled.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::core::AnyView;
use mitsuami::prelude::*;

const ROLES: [(&str, ButtonRole); 4] = [
    ("Normal", ButtonRole::Normal),
    ("Default", ButtonRole::Default),
    ("Cancel", ButtonRole::Cancel),
    ("Destructive", ButtonRole::Destructive),
];

const STYLES: [(&str, ButtonStyle); 3] = [
    ("Automatic", ButtonStyle::Automatic),
    ("Bordered", ButtonStyle::Bordered),
    ("Borderless", ButtonStyle::Borderless),
];

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

/// A row per role, a column per style, and the same disabled.
fn gallery() -> impl View {
    let mut cells: Vec<AnyView> =
        ["", "Bordered", "Borderless", "Disabled"].map(|h| AnyView::new(view! { <Text>{h}</Text> })).into();
    for (name, role) in ROLES {
        cells.extend([
            AnyView::new(view! { <Text>{name}</Text> }),
            AnyView::new(view! { <Row><Button role=role>{name}</Button></Row> }),
            AnyView::new(view! { <Row><Button role=role button_style=ButtonStyle::Borderless>{name}</Button></Row> }),
            AnyView::new(view! { <Row><Button role=role enabled=false>{name}</Button></Row> }),
        ]);
    }
    view! {
        <Column gap=Spacing::Md>
            {heading("Roles and styles")}
            <Grid
                columns=[Track::MaxContent, Track::MaxContent, Track::MaxContent, Track::MaxContent]
                column_gap=Spacing::Lg
                row_gap=Spacing::Sm
                align=Align::Center
            >
                {cells}
            </Grid>
        </Column>
    }
}

/// One button, and its props to change.
fn playground() -> impl View {
    let role = signal(1);
    let style = signal(0);
    let label = signal("Save".to_string());
    let enabled = signal(true);
    let clicks = signal(0);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid columns=[Track::MaxContent, Track::Size(1.fr())] column_gap=Spacing::Md row_gap=Spacing::Sm align=Align::Center>
                <Text>"Role"</Text>
                <Row><Select label="Role" options=ROLES.map(|(name, _)| name) bind=role/></Row>
                <Text>"Style"</Text>
                <Row><Select label="Style" options=STYLES.map(|(name, _)| name) bind=style/></Row>
                <Text>"Label"</Text>
                <TextInput a11y_label="Label" bind=label/>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
            </Grid>
            <Row gap=Spacing::Md align=Align::Center>
                <Button
                    role=move || ROLES[role.get()].1
                    button_style=move || STYLES[style.get()].1
                    enabled=enabled
                    @click=move || clicks.update(|c| *c += 1)
                >
                    {label}
                </Button>
                <Text>
                    {move || match clicks.get() {
                        0 => "Not clicked yet".to_string(),
                        1 => "Clicked once".to_string(),
                        n => format!("Clicked {n} times"),
                    }}
                </Text>
            </Row>
        </Column>
    }
}

/// A setting only this platform has, straight on the native button.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Button>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSButton| {
                b.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Large)
            }),
            "AppKit: controlSize large makes a larger button, as macOS uses in sheets and onboarding.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::Button| {
                use mitsuami::gtk::gtk::prelude::*;
                b.add_css_class("circular")
            }),
            "GTK: the circular style class rounds the button's ends.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_bool("checkable", true)),
            "Qt Quick: checkable makes the button stay down when clicked, as a toggle.",
        ),
        windows => (
            mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::Button| {
                use mitsuami::winui::bindings::{CornerRadius, IControl};
                use mitsuami::winui::windows_core::Interface;
                let round = CornerRadius { top_left: 16.0, top_right: 16.0, bottom_right: 16.0, bottom_left: 16.0 };
                b.cast::<IControl>()?.SetCornerRadius(round)
            }),
            "WinUI: a CornerRadius of 16 rounds the button's ends.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Md align=Align::Center>
                <Button role=ButtonRole::Default>"Default"</Button>
                <Button role=ButtonRole::Default native=tweak>"Tweaked"</Button>
            </Row>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Button", WindowSize::FitHeight(560.0), || {
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
