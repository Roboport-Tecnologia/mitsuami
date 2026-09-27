//! Scroll view: `cargo run -p mitsuami --example scroll_view`.
//!
//! - Scroll views along each axis: a vertical list of rows, a horizontal
//!   strip, and a grid that scrolls both ways.
//! - A playground: hide the scroll bars (it still scrolls), and watch the
//!   offset as it scrolls.
//! - A raw platform setting, through `.native()`. Scroll bars are the one
//!   option every platform shares: the rest is each one's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn rows(count: usize) -> Vec<Text> {
    (1..=count).map(|i| view! { <Text>{format!("Row {i}")}</Text> }).collect()
}

/// A tile for the horizontal and two-way scroll views.
fn tile(label: String) -> impl View {
    view! {
        <Column width=72 height=48 justify=Justify::Center align=Align::Center>
            <Text>{label}</Text>
        </Column>
    }
}

fn gallery() -> impl View {
    let strip = (1..=12).map(|i| tile(format!("{i}"))).collect::<Vec<_>>();
    let grid = (0..48).map(|i| tile(format!("{},{}", i / 8 + 1, i % 8 + 1))).collect::<Vec<_>>();
    let columns = (0..8).map(|_| Track::Size(72.into())).collect::<Vec<_>>();
    view! {
        <Column gap=Spacing::Md>
            {heading("Axes")}
            <Row gap=Spacing::Lg align=Align::Start>
                <Column gap=Spacing::Sm>
                    <Text text_style=TextStyle::Caption>"Vertical"</Text>
                    <ScrollView width=160 height=120>
                        <Column gap=Spacing::Xs>{rows(20)}</Column>
                    </ScrollView>
                </Column>
                // Without a minimum width of 0, the column is as wide as the
                // widest content in it (as in CSS), and the scroll views in it
                // with it: they'd have nothing to scroll.
                <Column gap=Spacing::Sm grow=1.0 min_width=0>
                    <Text text_style=TextStyle::Caption>"Horizontal"</Text>
                    <ScrollView axes=ScrollAxes::Horizontal height=56>
                        <Row gap=Spacing::Sm>{strip}</Row>
                    </ScrollView>
                    <Text text_style=TextStyle::Caption>"Both"</Text>
                    <ScrollView axes=ScrollAxes::Both height=120>
                        <Grid columns=columns gap=Spacing::Sm>{grid}</Grid>
                    </ScrollView>
                </Column>
            </Row>
        </Column>
    }
}

/// One scroll view, its scroll bars to turn off, and its offset.
fn playground() -> impl View {
    let bars = signal(true);
    let offset = signal(Point::new(0.0, 0.0));
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Row gap=Spacing::Md align=Align::Center>
                <Text>"Scroll bars"</Text>
                <Switch bind=bars>"Scroll bars"</Switch>
                <Text text_style=TextStyle::Caption>{move || format!("Scrolled to {:.0}", offset.get().y)}</Text>
            </Row>
            <ScrollView height=120 scroll_bars=bars @scroll=move |p| offset.set(p)>
                <Column gap=Spacing::Xs>{rows(40)}</Column>
            </ScrollView>
        </Column>
    }
}

/// A setting only this platform has, straight on the native scroll view.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<ScrollView>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSScrollView| {
                s.setBorderType(mitsuami::appkit::objc2_app_kit::NSBorderType::BezelBorder)
            }),
            "AppKit: borderType bezel draws a sunken border around the scroll view.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::ScrolledWindow| s.set_overlay_scrolling(false)),
            "GTK: overlay-scrolling off gives classic scroll bars, beside the content and always shown.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| {
                // The desktop style's Kirigami.WheelHandler, the one object
                // with this property.
                if let Some(wheel) = s.find("scrollFlickableTarget", "true") {
                    wheel.set_real("verticalStepSize", 20.0);
                }
            }),
            "Kirigami: WheelHandler's verticalStepSize sets how far a wheel notch scrolls: one line here, instead of the system's three.",
        ),
        windows => (
            mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::ScrollViewer| {
                use mitsuami::winui::windows_core::Interface;
                s.cast::<mitsuami::winui::bindings::IScrollViewer>()?.SetIsScrollInertiaEnabled(false)
            }),
            "WinUI: IsScrollInertiaEnabled off stops a touch or touchpad scroll as soon as you let go.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Lg>
                <ScrollView width=160 height=120>
                    <Column gap=Spacing::Xs>{rows(20)}</Column>
                </ScrollView>
                <ScrollView width=160 height=120 native=tweak>
                    <Column gap=Spacing::Xs>{rows(20)}</Column>
                </ScrollView>
            </Row>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Scroll view", WindowSize::FitHeight(560.0), || {
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
