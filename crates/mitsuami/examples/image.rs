//! Image: `cargo run -p mitsuami --example image`.
//!
//! - Images from pixels (at 1× and at 2×) and from a file, at their own
//!   size.
//! - Live pixels: an image the app draws again as a slider moves, as a
//!   preview does.
//! - A playground: fit a picture into a frame of another shape.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

/// A checkerboard of `square`-pixel squares.
fn checkerboard(width: u32, height: u32, square: u32, a: [u8; 4], b: [u8; 4]) -> Pixels {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(if (x / square + y / square).is_multiple_of(2) { &a } else { &b });
        }
    }
    Pixels::new(width, height, rgba)
}

/// Rings around the middle, `phase` turning them.
fn rings(size: u32, phase: f64) -> Pixels {
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let middle = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let distance = ((x as f64 - middle).powi(2) + (y as f64 - middle).powi(2)).sqrt();
            let wave = ((distance / 6.0 - phase).sin() + 1.0) / 2.0;
            rgba.extend_from_slice(&[(40.0 + 200.0 * wave) as u8, 80, (240.0 - 200.0 * wave) as u8, 255]);
        }
    }
    Pixels::new(size, size, rgba)
}

fn file() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets/blue-red-20x10.png")
}

fn gallery() -> impl View {
    let blue = [40, 90, 200, 255];
    let light = [240, 240, 240, 255];
    view! {
        <Column gap=Spacing::Md>
            {heading("At their own size")}
            <Row gap=Spacing::Lg align=Align::End>
                <Column gap=Spacing::Xs align=Align::Start>
                    <Image source=ImageSource::Pixels(checkerboard(64, 64, 8, blue, light)) label="Pixels at 1×"/>
                    <Text text_style=TextStyle::Caption>"64 px at 1×"</Text>
                </Column>
                // Half the points, each pixel its own on a 2× display.
                <Column gap=Spacing::Xs align=Align::Start>
                    <Image source=ImageSource::Pixels(checkerboard(64, 64, 8, blue, light).scale(2.0)) label="Pixels at 2×"/>
                    <Text text_style=TextStyle::Caption>"64 px at 2×"</Text>
                </Column>
                <Column gap=Spacing::Xs align=Align::Start>
                    <Image source=ImageSource::File(file()) label="A PNG file"/>
                    <Text text_style=TextStyle::Caption>"A PNG file"</Text>
                </Column>
            </Row>
        </Column>
    }
}

/// Pixels the app makes again whenever what they show changes.
fn live() -> impl View {
    let phase = signal(0.0);
    view! {
        <Column gap=Spacing::Md>
            {heading("Live pixels")}
            <Row gap=Spacing::Md align=Align::Center>
                <Image source=move || ImageSource::Pixels(rings(96, phase.get() / 10.0)) label="Rings"/>
                <Slider label="Phase" range_with=(0.0, 100.0) bind=phase grow=1.0/>
            </Row>
        </Column>
    }
}

/// A picture in a wide frame, fitted as chosen.
fn playground() -> impl View {
    let fit = signal(0);
    let fits = [ImageFit::Contain, ImageFit::Stretch];
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Row gap=Spacing::Md align=Align::Center>
                <Text>"Fit"</Text>
                <Select label="Fit" options=["Contain", "Stretch"] bind=fit/>
            </Row>
            <Image
                source=ImageSource::Pixels(checkerboard(64, 64, 8, [200, 60, 60, 255], [240, 240, 240, 255]))
                label="Fitted"
                fit=move || fits[fit.get()]
                width=240
                height=80
            />
        </Column>
    }
}

/// A setting only this platform has, straight on the native image view.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Image>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|v: &mitsuami::appkit::objc2_app_kit::NSImageView| {
                v.setImageFrameStyle(mitsuami::appkit::objc2_app_kit::NSImageFrameStyle::Photo)
            }),
            "AppKit: imageFrameStyle photo draws a photo's frame and shadow around it.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|p: &mitsuami::gtk::gtk::Picture| p.set_content_fit(mitsuami::gtk::gtk::ContentFit::Cover)),
            "GTK: content-fit cover fills the frame, cropping what doesn't fit.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|i: &mitsuami::kirigami::QmlObject| i.set_bool("smooth", false)),
            "Qt Quick: smooth off scales it pixel by pixel, sharp edges and all.",
        ),
        windows => (
            mitsuami::winui::tweak(|i: &mitsuami::winui::bindings::Image| {
                i.SetStretch(mitsuami::winui::bindings::Stretch::UniformToFill)
            }),
            "WinUI: Stretch UniformToFill fills the frame, cropping what doesn't fit.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Image
                source=ImageSource::Pixels(checkerboard(24, 12, 4, [30, 30, 30, 255], [250, 200, 40, 255]))
                label="Tweaked"
                native=tweak
                width=192
                height=64
            />
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Image", WindowSize::FitHeight(560.0), || {
            view! {
                <Column padding=Spacing::Xl gap=Spacing::Xl>
                    {gallery()}
                    {live()}
                    {playground()}
                    {platform_option()}
                </Column>
            }
        })
        .run();
}
