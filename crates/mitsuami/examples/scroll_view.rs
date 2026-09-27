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
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

fn rows(count: usize) -> Vec<Text> {
    (1..=count).map(|i| Text::new(format!("Row {i}"))).collect()
}

/// A tile for the horizontal and two-way scroll views.
fn tile(label: String) -> impl View {
    Column::new().width(72).height(48).justify(Justify::Center).align(Align::Center).child(Text::new(label))
}

fn gallery() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("Axes"),
        Row::new().gap(Spacing::Lg).align(Align::Start).children((
            Column::new().gap(Spacing::Sm).children((
                Text::new("Vertical").text_style(TextStyle::Caption),
                ScrollView::new().width(160).height(120).child(Column::new().gap(Spacing::Xs).children(rows(20))),
            )),
            // Without a minimum width of 0, the column is as wide as the
            // widest content in it (as in CSS), and the scroll views in it
            // with it: they'd have nothing to scroll.
            Column::new().gap(Spacing::Sm).grow(1.0).min_width(0).children((
                Text::new("Horizontal").text_style(TextStyle::Caption),
                ScrollView::horizontal().height(56).child(
                    Row::new().gap(Spacing::Sm).children((1..=12).map(|i| tile(format!("{i}"))).collect::<Vec<_>>()),
                ),
                Text::new("Both").text_style(TextStyle::Caption),
                ScrollView::both().height(120).child(
                    Grid::new()
                        .columns((0..8).map(|_| Track::Size(72.into())).collect::<Vec<_>>())
                        .gap(Spacing::Sm)
                        .children((0..48).map(|i| tile(format!("{},{}", i / 8 + 1, i % 8 + 1))).collect::<Vec<_>>()),
                ),
            )),
        )),
    ))
}

/// One scroll view, its scroll bars to turn off, and its offset.
fn playground() -> impl View {
    let bars = signal(true);
    let offset = signal(Point::new(0.0, 0.0));
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Text::new("Scroll bars"),
            Switch::new("Scroll bars").bind(bars),
            Text::new(move || format!("Scrolled to {:.0}", offset.get().y)).text_style(TextStyle::Caption),
        )),
        ScrollView::new()
            .height(120)
            .scroll_bars(bars)
            .on_scroll(move |p| offset.set(p))
            .child(Column::new().gap(Spacing::Xs).children(rows(40))),
    ))
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
                // Flickable.DragAndOvershootBounds.
                if let Some(flickable) = s.object("contentItem") {
                    flickable.set_int("boundsBehavior", 3);
                }
            }),
            "Qt Quick: boundsBehavior lets the content be dragged past its ends and spring back.",
        ),
        windows => (
            mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::ScrollViewer| {
                use mitsuami::winui::windows_core::Interface;
                s.cast::<mitsuami::winui::bindings::IScrollViewer>()?.SetIsScrollInertiaEnabled(false)
            }),
            "WinUI: IsScrollInertiaEnabled off stops a touch or touchpad scroll as soon as you let go.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        Row::new().gap(Spacing::Lg).children((
            ScrollView::new().width(160).height(120).child(Column::new().gap(Spacing::Xs).children(rows(20))),
            ScrollView::new()
                .width(160)
                .height(120)
                .native(tweak)
                .child(Column::new().gap(Spacing::Xs).children(rows(20))),
        )),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Scroll view", WindowSize::FitHeight(560.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
