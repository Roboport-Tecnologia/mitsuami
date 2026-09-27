//! Text: `cargo run -p mitsuami --example text`.
//!
//! - Every text style, from the platform's type ramp.
//! - A playground: a paragraph at a width and a line limit to change.
//! - A raw platform setting, through `.native()`. A line limit is the one
//!   option every platform's label shares: the rest is each one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

const PARAGRAPH: &str = "Being native on each platform is this toolkit's reason to exist. Widgets \
                         behave, size, animate and respond as the platform's own controls do, and \
                         text is set in the platform's own type ramp, wrapping and cut off as it \
                         wraps and cuts off.";

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn gallery() -> impl View {
    let styles = [
        ("Large title", TextStyle::LargeTitle),
        ("Title", TextStyle::Title),
        ("Headline", TextStyle::Headline),
        ("Body", TextStyle::Body),
        ("Callout", TextStyle::Callout),
        ("Caption", TextStyle::Caption),
        ("Monospace", TextStyle::Monospace),
    ];
    view! {
        <Column gap=Spacing::Md>
            {heading("Styles")}
            <Column gap=Spacing::Xs>
                {styles.into_iter().map(|(name, style)| view! { <Text text_style=style>{name}</Text> }).collect::<Vec<_>>()}
            </Column>
        </Column>
    }
}

/// A paragraph, its width and its line limit.
fn playground() -> impl View {
    let lines = signal(2.0_f64);
    let width = signal(320.0_f64);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>
                    {move || match lines.get() as u32 {
                        0 => "Lines (all)".to_string(),
                        n => format!("Lines ({n})"),
                    }}
                </Text>
                <Slider label="Lines" range_with=(0.0, 6.0) step=1.0 bind=lines/>
                <Text>{move || format!("Width ({:.0})", width.get())}</Text>
                <Slider label="Width" range_with=(120.0, 480.0) bind=width/>
            </Grid>
            <Text max_lines=move || lines.get() as u32 width=move || Length::Px(width.get() as f32)>{PARAGRAPH}</Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native label.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Text>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTextField| {
                t.setTextColor(Some(&mitsuami::appkit::objc2_app_kit::NSColor::secondaryLabelColor()))
            }),
            "AppKit: textColor secondaryLabelColor, AppKit's colour for less important text.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|l: &mitsuami::gtk::gtk::Label| {
                use mitsuami::gtk::gtk::prelude::*;
                l.add_css_class("dim-label")
            }),
            "GTK: the dim-label style class, GNOME's dimmed text.",
        ),
        kde => (
            // Text.MarkdownText.
            mitsuami::kirigami::tweak(|l: &mitsuami::kirigami::QmlObject| l.set_int("textFormat", 3)),
            "Qt Quick: textFormat MarkdownText renders the text as Markdown.",
        ),
        windows => (
            mitsuami::winui::tweak(|t: &mitsuami::winui::bindings::TextBlock| {
                use mitsuami::winui::windows_core::Interface;
                t.cast::<mitsuami::winui::bindings::ITextBlock>()?.SetCharacterSpacing(200)
            }),
            "WinUI: CharacterSpacing spreads the letters, in thousandths of an em.",
        ),
    };
    // Markup only where the tweak renders it; elsewhere it would show as is.
    let sample = platform! {
        kde => "text, with **some** of it _marked up_",
        _ => "text, as the tweak shows it",
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Text>{format!("Plain {sample}")}</Text>
            <Text native=tweak>{format!("Tweaked {sample}")}</Text>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Text", WindowSize::FitHeight(640.0), || {
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
