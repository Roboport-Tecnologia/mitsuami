//! A tray icon: `cargo run -p mitsuami --example tray`.
//!
//! - It starts with no window, only its icon in the tray: macOS's menu
//!   bar, Windows' notification area, and on Linux a StatusNotifierItem
//!   host (Plasma's system tray, waybar, GNOME with the AppIndicator
//!   extension).
//! - A click opens its menu: Open Tray opens the window, Paused swaps the
//!   icon and its tooltip, Quit ends it. Closing the window leaves it
//!   running.
//! - Where there's no tray (plain GNOME), nothing could bring the window
//!   back, so it opens after a second.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use mitsuami::prelude::*;

/// The image macOS and Windows show: they have no icon theme.
fn image() -> AppIcon {
    AppIcon::bytes(include_bytes!("../tests/assets/tray-32.png").as_slice())
}

/// The icon and its menu, following `paused`.
pub fn tray(open: Signal<bool>, paused: Signal<bool>) -> TrayIcon {
    TrayIcon::new("Tray")
        .icon(move || {
            let name = if paused.get() { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" };
            AppIcon::named(name).image(image())
        })
        .tooltip(move || if paused.get() { "Paused".to_owned() } else { "Running".to_owned() })
        .menu(Menu::new("").children((
            MenuItem::new("Open Tray").on_select(move || open.set(true)),
            MenuItem::new("Paused").bind(paused),
            MenuSeparator,
            MenuItem::new("Quit").role(MenuRole::Quit).on_select(quit),
        )))
}

pub fn page(paused: Signal<bool>) -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Text>"Close this window and the app keeps running in the tray."</Text>
            <Text test_id="state">{move || if paused.get() { "Paused.".to_owned() } else { "Running.".to_owned() }}</Text>
            <Row justify=Justify::End gap=Spacing::Sm>
                <Switch bind=paused>"Paused"</Switch>
                <Button @click=quit>"Quit"</Button>
            </Row>
        </Column>
    }
}

pub fn window() -> Window {
    let open = signal(false);
    let paused = signal(false);
    set_tray(tray(open, paused));
    let shown = tray_shown();
    spawn_local(async move {
        sleep(Duration::from_secs(1)).await;
        if !shown.get_untracked() {
            open.set(true);
        }
    });
    Window::new("Tray").size(WindowSize::FitHeight(380.0)).bind(open).content(move || page(paused))
}

fn main() {
    App::new().id("br.com.roboport.mitsuami.Tray").name("Tray").runs_without_windows().open(window).run();
}
