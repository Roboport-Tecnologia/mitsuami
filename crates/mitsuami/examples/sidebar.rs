//! A window's sidebar: `cargo run -p mitsuami --example sidebar`.
//!
//! A settings window, as macOS's System Settings and GNOME's Settings lay
//! theirs out: the pages down the leading side, in sections, and the page
//! chosen beside them.
//!
//! - Choose a page with the pointer or the arrow keys: the content follows.
//! - "Next page" chooses from the app's side: the sidebar follows.
//! - Make the window narrow: each platform collapses the sidebar its own way
//!   (hidden on macOS, a page of its own on GNOME and KDE, icons only, then
//!   a menu button, on Windows).
//! - Hide the sidebar, the platform's way (`Sidebar::shown`): collapsed on
//!   macOS, its pane closed on Windows (icons only in a wide window), its
//!   page out of the row on KDE. GNOME's split view shows its sidebar
//!   while the window is wide: there it hides only in a narrow window,
//!   showing the page. Hiding it from the platform's own controls (macOS's
//!   View menu or divider, Windows' menu button) changes the button too.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Page {
    General,
    Appearance,
    WiFi,
    Bluetooth,
    Keyboard,
}

const PAGES: [Page; 5] = [Page::General, Page::Appearance, Page::WiFi, Page::Bluetooth, Page::Keyboard];

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Appearance => "Appearance",
            Page::WiFi => "Wi-Fi",
            Page::Bluetooth => "Bluetooth",
            Page::Keyboard => "Keyboard",
        }
    }

    fn about(self) -> &'static str {
        match self {
            Page::General => "Name, language and region.",
            Page::Appearance => "Light, dark, or following the time of day.",
            Page::WiFi => "Networks nearby, and the ones remembered.",
            Page::Bluetooth => "Devices paired with this computer.",
            Page::Keyboard => "Key repeat, layouts and shortcuts.",
        }
    }

    /// An SF Symbol, a symbolic theme icon, a Segoe Fluent Icons glyph.
    fn icon(self) -> &'static str {
        match self {
            Page::General => platform! {
                macos => "gearshape", linux => "preferences-system-symbolic", windows => "\u{E713}",
            },
            Page::Appearance => platform! {
                macos => "paintbrush", linux => "applications-graphics-symbolic", windows => "\u{E790}",
            },
            Page::WiFi => platform! {
                macos => "wifi", linux => "network-wireless-symbolic", windows => "\u{E701}",
            },
            Page::Bluetooth => platform! {
                macos => "dot.radiowaves.left.and.right", linux => "bluetooth-symbolic", windows => "\u{E702}",
            },
            Page::Keyboard => platform! {
                macos => "keyboard", linux => "input-keyboard-symbolic", windows => "\u{E765}",
            },
        }
    }

    fn item(self) -> SidebarItem<Page> {
        SidebarItem::new(self.title(), self).icon(self.icon())
    }
}

fn settings() -> impl View {
    let page = signal(Page::Appearance);
    let sidebar = signal(true);
    let next = move || {
        let at = PAGES.iter().position(|p| *p == page.get()).unwrap_or(0);
        page.set(PAGES[(at + 1) % PAGES.len()]);
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Sidebar selection=page shown=sidebar>
                {Page::General.item()}
                {Page::Appearance.item()}
                <SidebarSection title="Network">
                    {Page::WiFi.item()}
                    {Page::Bluetooth.item()}
                </SidebarSection>
                <SidebarSection title="Devices">
                    {Page::Keyboard.item()}
                </SidebarSection>
            </Sidebar>
            <Text text_style=TextStyle::Title>{move || page.get().title().to_owned()}</Text>
            <Text>{move || page.get().about().to_owned()}</Text>
            <Row gap=Spacing::Md>
                <Button @click=next>"Next page"</Button>
                <Button @click=move || sidebar.update(|s| *s = !*s)>
                    {move || if sidebar.get() { "Hide the sidebar" } else { "Show the sidebar" }.to_owned()}
                </Button>
            </Row>
        </Column>
    }
}

fn main() {
    App::new().window("Settings", Size::new(560.0, 360.0), settings).run();
}
