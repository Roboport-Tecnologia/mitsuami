//! NumberInput: `cargo run -p mitsuami --example number_input`.
//!
//! - Spin boxes (small and large ranges, a step, disabled), as each
//!   platform draws them.
//! - A playground: set the range and step, and whether it's enabled.
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
            {heading("Spin boxes")}
            <Grid
                columns=[Track::MaxContent, Track::MaxContent]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Copies (1 to 99)"</Text>
                <NumberInput label="Copies" range_with=(1, 99) value=2/>
                // GTK sizes a spin box for its range's widest number.
                <Text>"Memory, MB (16 to 4096, by 16)"</Text>
                <NumberInput label="Memory" range_with=(16, 4096) step=16 value=512/>
                <Text>"Disabled"</Text>
                <NumberInput label="Disabled" value=7 enabled=false/>
            </Grid>
        </Column>
    }
}

/// One spin box, and its props to change.
fn playground() -> impl View {
    let max = signal(100);
    let step = signal(1);
    let enabled = signal(true);
    let value = signal(10);
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid
                columns=[Track::MaxContent, Track::MaxContent]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Maximum"</Text>
                <NumberInput label="Maximum" range_with=(1, 100_000) bind=max/>
                <Text>"Step"</Text>
                <NumberInput label="Step" range_with=(1, 1000) bind=step/>
                <Text>"Enabled"</Text>
                <Switch bind=enabled>"Enabled"</Switch>
                <Text>"Value"</Text>
                <NumberInput
                    label="Value"
                    range_with=move || (0, max.get())
                    step=step
                    enabled=enabled
                    bind=value
                />
            </Grid>
            <Text>{move || format!("The app has {}", value.get())}</Text>
        </Column>
    }
}

/// A setting only this platform has, straight on the native spin box.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<NumberInput>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|n: &mitsuami::appkit::NumberField| n.stepper().setValueWraps(false)),
            "AppKit: a stepper wraps round at its ends; valueWraps off stops it there instead.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::SpinButton| s.set_wrap(true)),
            "GTK: wrap makes the buttons go round from one end to the other.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_bool("wrap", true)),
            "Qt Quick: wrap makes the buttons go round from one end to the other.",
        ),
        windows => (
            mitsuami::winui::tweak(|n: &mitsuami::winui::bindings::NumberBox| {
                n.SetSpinButtonPlacementMode(mitsuami::winui::bindings::NumberBoxSpinButtonPlacementMode::Compact)
            }),
            "WinUI: compact spin buttons show in a pop-up while the box has focus.",
        ),
    };
    let value = signal(5);
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Row gap=Spacing::Md align=Align::Center>
                <NumberInput label="Tweaked" range_with=(1, 10) bind=value native=tweak/>
                <Text>{move || format!("{}", value.get())}</Text>
            </Row>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("NumberInput", WindowSize::FitHeight(480.0), || {
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
