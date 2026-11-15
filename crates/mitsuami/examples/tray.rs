//! A tray icon: `cargo run -p mitsuami --example tray`.
//!
//! - It starts with no window, only its icon in the tray: macOS's menu
//!   bar, Windows' notification area, and on Linux a StatusNotifierItem
//!   host (Plasma's system tray, waybar, GNOME with the AppIndicator
//!   extension).
//! - A click opens its menu: Open Tray opens the window, Paused swaps the
//!   icon and its tooltip, Quit ends it. Closing the window leaves it
//!   running, and a click on its Dock icon (macOS) or launching it again
//!   opens the window.
//! - The window chooses how the icon is given, each platform taking what
//!   it understands: a theme icon with an image for the others, an image
//!   alone (an `.ico` on Windows), a theme name alone, or none (the app's
//!   own icon).
//! - Where there's no tray (plain GNOME), nothing could bring the window
//!   back, so it opens after a second.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

/// How the window lets the icon be given, in the order of its options.
pub const CHOICES: [&str; 4] =
    ["Theme icon, with an image elsewhere", "An image only", "A theme name only", "The app's own icon"];

/// The image for platforms without an icon theme: a monochrome PNG, which
/// macOS tints as a template for the menu bar, or on Windows an `.ico`
/// with the small sizes (16 to 48 pixels), which it picks from for the
/// screen's DPI.
pub fn image() -> AppIcon {
    if cfg!(windows) {
        AppIcon::bytes(include_bytes!("../tests/assets/tray.ico").as_slice())
    } else {
        AppIcon::bytes(include_bytes!("../tests/assets/tray-32.png").as_slice())
    }
}

/// The icon theme's name for the state: Linux shows it.
fn theme_name(paused: bool) -> &'static str {
    if paused { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }
}

/// The icon and its menu, following `paused`, with the icon given the
/// way `choice` (an index into [`CHOICES`]) says.
pub fn tray(open: Signal<bool>, paused: Signal<bool>, choice: usize) -> TrayIcon {
    let tray = TrayIcon::new("Tray")
        .tooltip(move || if paused.get() { "Paused".to_owned() } else { "Running".to_owned() })
        .menu(Menu::new("").children((
            MenuItem::new("Open Tray").on_select(move || open.set(true)),
            MenuItem::new("Paused").bind(paused),
            MenuSeparator,
            MenuItem::new("Quit").role(MenuRole::Quit).on_select(quit),
        )));
    match choice {
        // Linux shows the theme's icon, macOS and Windows the image.
        0 => tray.icon(move || AppIcon::named(theme_name(paused.get())).image(image())),
        // Every platform shows the image, Linux as pixels.
        1 => tray.icon(image()),
        // Linux only: macOS and Windows have nothing to show.
        2 => tray.icon(move || AppIcon::named(theme_name(paused.get()))),
        // No icon: the app's own, the theme's under its id on Linux, the
        // Dock's on macOS, `App::icon` on Windows.
        _ => tray,
    }
}

pub fn page(paused: Signal<bool>, choice: Signal<usize>) -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Text>"Close this window and the app keeps running in the tray."</Text>
            <Text test_id="state">{move || if paused.get() { "Paused.".to_owned() } else { "Running.".to_owned() }}</Text>
            <Grid columns=[Track::MaxContent, Track::Size(1.fr())] column_gap=Spacing::Md align=Align::Center>
                <Text>"Icon"</Text>
                <Row><Select label="Icon" options=CHOICES bind=choice/></Row>
            </Grid>
            <Text text_style=TextStyle::Caption>
                "Linux shows a theme icon by name, or an image's pixels. macOS shows the image, tinted as a \
                 template for the menu bar. Windows shows the image, or an .ico at its small sizes. With no \
                 icon, each shows the app's own."
            </Text>
            <Row justify=Justify::End gap=Spacing::Sm align=Align::Center>
                <Switch bind=paused>"Paused"</Switch>
                <Button @click=quit>"Quit"</Button>
            </Row>
        </Column>
    }
}

pub fn window() -> Window {
    let open = signal(false);
    let paused = signal(false);
    let choice = signal(0usize);
    // Set again when the choice changes: "the app's own icon" is a tray
    // with no icon given, not an icon that changes.
    effect(move || {
        let choice = choice.get();
        untrack(|| set_tray(tray(open, paused, choice)));
    });
    // A click on the Dock icon, or a second launch, which single instance
    // hands to this one.
    on_reopen(move |_| open.set(true));
    let shown = tray_shown();
    spawn_local(async move {
        sleep(Duration::from_secs(1)).await;
        if !shown.get_untracked() {
            open.set(true);
        }
    });
    Window::new("Tray").size(WindowSize::FitHeight(420.0)).bind(open).content(move || page(paused, choice))
}

fn main() {
    App::new()
        .id("br.com.roboport.mitsuami.Tray")
        .name("Tray")
        .runs_without_windows()
        .single_instance()
        .open(window)
        .run();
}
