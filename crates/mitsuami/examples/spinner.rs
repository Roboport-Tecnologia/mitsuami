//! Spinner: `cargo run -p mitsuami --example spinner`.
//!
//! - A spinner running and one stopped: stopped, it shows nothing but keeps
//!   its place.
//! - A playground: start and stop it, or run a pretend load.
//! - A raw platform setting, through `.native()`: a different one on each
//!   platform, since each has its own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    Text::new(text.to_string()).text_style(TextStyle::Headline)
}

/// A spinner beside what it's for, as apps show them.
fn beside(spinner: Spinner, text: impl IntoValue<String>) -> impl View {
    Row::new().gap(Spacing::Sm).align(Align::Center).children((spinner, Text::new(text)))
}

fn gallery() -> impl View {
    Column::new().gap(Spacing::Md).children((
        heading("States"),
        beside(Spinner::new("Running"), "Running"),
        beside(Spinner::new("Stopped").running(false), "Stopped: nothing shows, and the text stays put"),
    ))
}

/// One spinner, started and stopped by hand or by a task.
fn playground() -> impl View {
    let running = signal(true);
    let loading = signal(false);
    let load = move || {
        loading.set(true);
        running.set(true);
        spawn_local(async move {
            sleep(Duration::from_secs(2)).await;
            running.set(false);
            loading.set(false);
        });
    };
    Column::new().gap(Spacing::Md).children((
        heading("Try it"),
        Row::new().gap(Spacing::Md).align(Align::Center).children((
            Text::new("Running"),
            Switch::new("Running").bind(running).enabled(move || !loading.get()),
            Button::new("Load for two seconds").enabled(move || !loading.get()).on_click(load),
        )),
        beside(Spinner::new("Photos").running(running), move || {
            if running.get() { "Loading photos…".to_string() } else { "Photos loaded".to_string() }
        }),
    ))
}

/// A setting only this platform has, straight on the native spinner.
fn platform_option() -> impl View {
    let (tweak, about): (Tweak<Spinner>, &str) = platform! {
        macos => (
            mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSProgressIndicator| {
                s.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Small)
            }),
            "AppKit: controlSize small, as in a text field or a list row.",
        ),
        gtk => (
            mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Spinner| {
                use mitsuami::gtk::gtk::prelude::*;
                s.set_size_request(32, 32)
            }),
            "GTK: a 32 px size request, as GNOME shows a page that's loading.",
        ),
        kde => (
            mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| {
                s.set_real("implicitWidth", 48.0);
                s.set_real("implicitHeight", 48.0);
            }),
            "Qt Quick: a larger implicit size, as Kirigami pages show one while loading.",
        ),
        windows => (
            mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::ProgressRing| {
                use mitsuami::winui::windows_core::Interface;
                let ring = s.cast::<mitsuami::winui::bindings::IProgressRing>()?;
                ring.SetIsIndeterminate(false)?;
                ring.SetValue(60.0)
            }),
            "WinUI: a ProgressRing can show a value: IsIndeterminate off, at 60 %.",
        ),
    };
    Column::new().gap(Spacing::Md).children((
        heading("A platform option"),
        beside(Spinner::new("Plain"), "Plain"),
        beside(Spinner::new("Tweaked").native(tweak), "Tweaked"),
        Text::new(about).text_style(TextStyle::Caption),
    ))
}

fn main() {
    App::new()
        .window("Spinner", WindowSize::FitHeight(460.0), || {
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((gallery(), playground(), platform_option()))
        })
        .run();
}
