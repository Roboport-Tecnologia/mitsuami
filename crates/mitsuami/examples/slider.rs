//! Slider: `cargo run -p mitsuami --example slider`.
//!
//! - Horizontal sliders (without a step, with one, disabled) and vertical
//!   ones, as each platform draws them.
//! - A playground: pick the orientation and whether it's enabled, and move
//!   it.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

/// How far a wheel notch moves the slider on GTK: its page increment,
/// which the backend makes ten steps, the whole range without a step.
fn wheel(page: f64) -> Tweak<Slider> {
    platform! {
        gtk => mitsuami::gtk::tweak(move |s: &mitsuami::gtk::gtk::Scale| {
            use mitsuami::gtk::gtk::prelude::*;
            s.adjustment().set_page_increment(page)
        }),
        // Qt, WinUI and AppKit's wheels are their own.
        _ => {
            let _ = page;
            Tweak::none()
        }
    }
}

fn gallery() -> impl View {
    let vertical = |name: &str, value: f64| {
        view! { <Slider label=name orientation=Orientation::Vertical value=value native=wheel(1.0)/> }
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Horizontal")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"No step"</Text>
                <Slider label="No step" value=30.0 native=wheel(1.0)/>
                <Text>"A step of 20"</Text>
                // What a step does is the platform's: tick marks the knob
                // stops at on AppKit, snapping on WinUI, keyboard moves on
                // GTK and Qt.
                <Slider label="A step of 20" step=20.0 value=40.0 native=wheel(20.0)/>
                <Text>"Disabled"</Text>
                <Slider label="Disabled" value=70.0 enabled=false/>
            </Grid>
            {heading("Vertical")}
            <Row gap=Spacing::Xl height=140>
                {vertical("Bass", 20.0)}
                {vertical("Mid", 50.0)}
                {vertical("Treble", 80.0)}
            </Row>
        </Column>
    }
}

/// One slider, and its props to change.
fn playground() -> impl View {
    let vertical = signal(0);
    let enabled = signal(true);
    let value = signal(50.0);
    let is_vertical = move || vertical.get() == 1;
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Orientation"</Text>
                <Row>
                    <Select label="Orientation" options=["Horizontal", "Vertical"] bind=vertical/>
                </Row>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
            </Grid>
            <Row gap=Spacing::Md align=Align::Center>
                // Tall when vertical, as wide as the row when horizontal:
                // sliders take their length from the layout.
                <Slider
                    label="Value"
                    orientation=move || if is_vertical() { Orientation::Vertical } else { Orientation::Horizontal }
                    enabled=enabled
                    bind=value
                    height=move || if is_vertical() { Length::Px(140.0) } else { Length::Auto }
                    grow=move || if is_vertical() { 0.0 } else { 1.0 }
                    native=wheel(1.0)
                />
                <Text>{move || format!("{:.0}", value.get())}</Text>
            </Row>
        </Column>
    }
}

/// A setting only this platform has, straight on the native slider.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Slider>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSSlider| {
                s.setSliderType(mitsuami::appkit::objc2_app_kit::NSSliderType::Circular)
            }),
            "AppKit: sliderType circular makes a dial, turned around its centre.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Scale| {
                use mitsuami::gtk::gtk::prelude::*;
                s.set_draw_value(true);
                // A step per wheel notch, as `wheel` does elsewhere.
                s.adjustment().set_page_increment(1.0)
            }),
            "GTK: draw-value shows the value beside the scale.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_int("snapMode", 1)),
            "Qt Quick: snapMode SnapAlways makes the handle jump from step to step while dragged.",
        ),
        windows => (
            mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::Slider| {
                use mitsuami::winui::bindings::{ISlider, TickPlacement};
                use mitsuami::winui::windows_core::Interface;
                let s = s.cast::<ISlider>()?;
                s.SetTickFrequency(1.0)?;
                s.SetTickPlacement(TickPlacement::Outside)
            }),
            "WinUI: TickFrequency and TickPlacement draw tick marks at every step.",
        ),
    };
    let value = signal(4.0);
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Md align=Align::Center>
                // As wide as the row, as sliders take their length from the
                // layout; AppKit's dial has a size of its own.
                <Slider
                    label="Tweaked"
                    range_with=(0.0, 10.0)
                    step=1.0
                    bind=value
                    grow=platform! { macos => 0.0, _ => 1.0 }
                    native=tweak
                />
                <Text>{move || format!("{:.0}", value.get())}</Text>
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
    App::new().window("Slider", WindowSize::FitHeight(640.0), page).run();
}
