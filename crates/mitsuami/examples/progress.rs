//! Progress: `cargo run -p mitsuami --example progress`.
//!
//! - Bars at a few values, and one for work of unknown length.
//! - A playground: set the value, or make it indeterminate, or run a
//!   pretend download that connects first.
//! - A raw platform setting, through `.native()`. Progress bars have no
//!   semantic options past the value: what platforms offer is each one's
//!   own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    view! { <Text text_style=TextStyle::Headline>{text.to_string()}</Text> }
}

fn gallery() -> impl View {
    view! {
        <Column gap=Spacing::Md>
            {heading("Values")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Md
                align=Align::Center
            >
                <Text>"Not started"</Text>
                <Progress label="Not started" value=0.0/>
                <Text>"A third"</Text>
                <Progress label="A third" value={1.0 / 3.0}/>
                <Text>"Done"</Text>
                <Progress label="Done" value=1.0/>
                // Animated as the platform animates it.
                <Text>"Unknown"</Text>
                <Progress label="Unknown"/>
            </Grid>
        </Column>
    }
}

/// One bar, its props to change, and a task to drive it.
fn playground() -> impl View {
    let value = signal(40.0_f64);
    let indeterminate = signal(false);
    let running = signal(false);
    let download = move || {
        running.set(true);
        indeterminate.set(true);
        value.set(0.0);
        spawn_local(async move {
            sleep(Duration::from_secs(1)).await;
            indeterminate.set(false);
            for step in 1..=20 {
                sleep(Duration::from_millis(100)).await;
                value.set(step as f64 * 5.0);
            }
            running.set(false);
        });
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("Try it")}
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>{move || format!("Value ({:.0}%)", value.get())}</Text>
                <Slider label="Value" range_with=(0.0, 100.0) bind=value enabled=move || !running.get()/>
                <Text>"Indeterminate"</Text>
                <Switch bind=indeterminate enabled=move || !running.get()>"Indeterminate"</Switch>
            </Grid>
            <Row gap=Spacing::Md align=Align::Center>
                <Progress
                    label="Download"
                    value=move || value.get() / 100.0
                    indeterminate=indeterminate
                    grow=1.0
                />
                <Button enabled=move || !running.get() @click=download>"Download"</Button>
            </Row>
        </Column>
    }
}

/// A setting only this platform has, straight on the native bar.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Progress>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|p: &mitsuami::appkit::objc2_app_kit::NSProgressIndicator| {
                p.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Small)
            }),
            "AppKit: controlSize small draws a thinner bar, as in a status bar or a list row.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|p: &mitsuami::gtk::gtk::ProgressBar| p.set_show_text(true)),
            "GTK: show-text draws the percentage with the bar.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|p: &mitsuami::kirigami::QmlObject| {
                if let Some(palette) = p.object("palette") {
                    palette.set_str("highlight", "#8e44ad");
                }
            }),
            "Qt Quick: palette.highlight colours the filled part, in styles that draw it with the palette.",
        ),
        windows => (
            mitsuami::winui::tweak(|p: &mitsuami::winui::bindings::ProgressBar| {
                use mitsuami::winui::windows_core::Interface;
                p.cast::<mitsuami::winui::bindings::IProgressBar>()?.SetShowPaused(true)
            }),
            "WinUI: ShowPaused draws the bar in its paused state.",
        ),
    };
    view! {
        <Column gap=Spacing::Md>
            {heading("A platform option")}
            <Progress label="Plain" value=0.6/>
            <Progress label="Tweaked" value=0.6 native=tweak/>
            <Text text_style=TextStyle::Caption>{about}</Text>
        </Column>
    }
}

fn main() {
    App::new()
        .window("Progress", WindowSize::FitHeight(560.0), || {
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
