//! Button: `cargo run -p mitsuami --example button`.
//!
//! - Every role in every style, and disabled: what the semantic props look
//!   like on this platform.
//! - A playground: pick the role, style, label and whether it's enabled.
//! - A suggested action: Commit, prominent among the others with the accent
//!   a default button has, but Return doesn't press it.
//! - Toggle buttons, which stay pressed until they're clicked again.
//! - Icons in colours of their own, as a bookmark picker has them: only the
//!   icon takes the colour, the caption and bezel stay the platform's.
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

const STYLES: [(&str, ButtonStyle); 4] = [
    ("Automatic", ButtonStyle::Automatic),
    ("Bordered", ButtonStyle::Bordered),
    ("Borderless", ButtonStyle::Borderless),
    ("Prominent", ButtonStyle::Prominent),
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

/// The action a view suggests among others, in the accent fill, as a Git
/// client's Commit: unlike a default button, it doesn't take Return.
fn suggested() -> impl View {
    let done = signal("Nothing done yet");
    view! {
        <Column gap=Spacing::Md>
            {heading("A suggested action")}
            <Row gap=Spacing::Md align=Align::Center>
                <Button @click=move || done.set("Stashed")>"Stash"</Button>
                <Button button_style=ButtonStyle::Prominent @click=move || done.set("Committed")>"Commit"</Button>
                <Text>{move || done.get().to_string()}</Text>
            </Row>
        </Column>
    }
}

/// Buttons that stay pressed, bordered and borderless, one with only an
/// icon, as a text editor's formatting bar has them.
fn toggles() -> impl View {
    let (bold, italic) = (signal(true), signal(false));
    let icon = platform! {
        macos => "italic",
        gtk => "format-text-italic-symbolic",
        kde => "format-text-italic",
        windows => "\u{E8DB}",
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Toggle buttons")}
            <Row gap=Spacing::Md align=Align::Center>
                <ToggleButton bind=bold>"Bold"</ToggleButton>
                <ToggleButton bind=italic icon=icon icon_only=true>"Italic"</ToggleButton>
                <ToggleButton bind=bold button_style=ButtonStyle::Borderless>"Bold"</ToggleButton>
                <Text>
                    {move || match (bold.get(), italic.get()) {
                        (true, true) => "Bold and italic",
                        (true, false) => "Bold",
                        (false, true) => "Italic",
                        (false, false) => "Plain",
                    }
                    .to_string()}
                </Text>
            </Row>
        </Column>
    }
}

/// A bookmark per colour, as a Git client's bookmark picker: the icon takes
/// the colour, the rest of the button stays the platform's.
fn icon_colors() -> impl View {
    let icon = platform! {
        macos => "bookmark.fill",
        gtk => "user-bookmarks-symbolic",
        kde => "bookmarks",
        windows => "\u{E8A4}",
    };
    let colors = [
        ("Red", Color::rgb(230, 60, 60)),
        ("Orange", Color::rgb(240, 150, 30)),
        ("Green", Color::rgb(50, 170, 80)),
        ("Blue", Color::rgb(50, 120, 230)),
    ];
    let picked = signal("None");
    let starred = signal(true);
    let buttons: Vec<AnyView> = colors
        .into_iter()
        .map(|(name, color)| {
            AnyView::new(view! {
                <Button icon=icon icon_only=true icon_color=color button_style=ButtonStyle::Borderless
                    @click=move || picked.set(name)>{name}</Button>
            })
        })
        .collect();
    view! {
        <Column gap=Spacing::Md>
            {heading("Icon colours")}
            <Row gap=Spacing::Sm align=Align::Center>
                {buttons}
                <Text>{move || format!("Bookmark: {}", picked.get())}</Text>
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <ToggleButton bind=starred icon=icon icon_color=Color::rgb(240, 150, 30)>"Bookmarked"</ToggleButton>
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
            mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_real("padding", 16.0)),
            "Qt Quick: a padding of 16 puts more room around the caption.",
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

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {gallery()}
            {playground()}
            {suggested()}
            {toggles()}
            {icon_colors()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Button", WindowSize::FitHeight(560.0), page).run();
}
