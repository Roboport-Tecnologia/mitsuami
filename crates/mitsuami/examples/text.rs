//! Text: `cargo run -p mitsuami --example text`.
//!
//! - Every text style, from the platform's type ramp.
//! - A playground: a paragraph at a width and a line limit, in a colour,
//!   a weight, italics and an alignment, left to right or right to left.
//! - Selectable text, which can be selected and copied.
//! - A raw platform setting, through `.native()`. Those are the options
//!   every platform's label shares: the rest is each one's own.

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

const COLORS: [(&str, Color); 7] = [
    ("Label", Color::Label),
    ("Secondary label", Color::SecondaryLabel),
    ("Accent", Color::Accent),
    ("Error", Color::Error),
    ("Warning", Color::Warning),
    ("Success", Color::Success),
    ("Fixed purple", Color::rgb(0x80, 0x40, 0xc0)),
];

const WEIGHTS: [(&str, FontWeight); 4] = [
    ("Regular", FontWeight::Regular),
    ("Medium", FontWeight::Medium),
    ("Semibold", FontWeight::Semibold),
    ("Bold", FontWeight::Bold),
];

const ALIGNMENTS: [(&str, TextAlign); 3] =
    [("Start", TextAlign::Start), ("Center", TextAlign::Center), ("End", TextAlign::End)];

/// A paragraph and every option it takes.
fn playground() -> impl View {
    let lines = signal(2.0_f64);
    let width = signal(320.0_f64);
    let color = signal(0_usize);
    let weight = signal(0_usize);
    let align = signal(0_usize);
    let italic = signal(false);
    let rtl = signal(false);
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
                <Text>"Colour"</Text>
                <Row><Select label="Colour" options=COLORS.map(|(name, _)| name) bind=color/></Row>
                <Text>"Weight"</Text>
                <Row><Select label="Weight" options=WEIGHTS.map(|(name, _)| name) bind=weight/></Row>
                <Text>"Alignment"</Text>
                <Row><Select label="Alignment" options=ALIGNMENTS.map(|(name, _)| name) bind=align/></Row>
                <Text>"Italic"</Text>
                <Row><Switch bind=italic>"Italic"</Switch></Row>
                <Text>"Right to left"</Text>
                <Row><Switch bind=rtl>"Right to left"</Switch></Row>
            </Grid>
            <Text
                max_lines=move || lines.get() as u32
                width=move || Length::Px(width.get() as f32)
                color=move || COLORS[color.get()].1
                weight=move || WEIGHTS[weight.get()].1
                italic=italic
                text_align=move || ALIGNMENTS[align.get()].1
                direction=move || if rtl.get() { TextDirection::Rtl } else { TextDirection::Ltr }
            >
                {PARAGRAPH}
            </Text>
        </Column>
    }
}

/// Text that can be selected and copied, beside a label that can't: drawn
/// the same, as each platform draws a selectable label.
fn selectable() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Selectable")}
            <Grid columns=[Track::MaxContent, Track::MaxContent] column_gap=Spacing::Md row_gap=Spacing::Sm>
                <Text>"Licence key"</Text>
                <Text selectable=true>"ABCD-1234-EFGH-5678"</Text>
                <Text>"Machine"</Text>
                <Text selectable=true>"Windows 98 SE, 64 MB"</Text>
            </Grid>
            <Text text_style=TextStyle::Caption>"Drag across the values to select them, and copy them."</Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native label.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Text>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTextField| t.setAllowsExpansionToolTips(true)),
            "AppKit: allowsExpansionToolTips shows the whole text in a tooltip, under the pointer, when it's cut off.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|l: &mitsuami::gtk::gtk::Label| {
                use mitsuami::gtk::gtk::pango::{AttrInt, AttrList, Underline};
                let attributes = AttrList::new();
                attributes.insert(AttrInt::new_underline(Underline::Single));
                l.set_attributes(Some(&attributes))
            }),
            "GTK: Pango attributes style runs of the text, here all of it underlined.",
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
    // Markup only where the tweak renders it; elsewhere it would show as
    // is. On AppKit, a line too long to fit, for the tooltip.
    let sample = platform! {
        macos => "text, cut off where the line ends: hover over it to read the rest, as Finder shows a file \
                  name too long for its column",
        kde => "text, with **some** of it _marked up_",
        _ => "text, as the tweak shows it",
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Text max_lines=1>{format!("Plain {sample}")}</Text>
            <Text max_lines=1 native=tweak>{format!("Tweaked {sample}")}</Text>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Xl>
            {gallery()}
            {playground()}
            {selectable()}
            {platform_option()}
        </Column>
    }
}

fn main() {
    App::new().window("Text", WindowSize::FitHeight(640.0), page).run();
}
