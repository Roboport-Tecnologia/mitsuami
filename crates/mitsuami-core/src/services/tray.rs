//! The app's icon in the platform's tray: macOS's menu bar extras, the
//! StatusNotifierItem hosts on Linux (Plasma's system tray, waybar, GNOME
//! with the AppIndicator extension), Windows' notification area.

use mitsuami_reactive::{IntoValue, Value};

use super::menu::{Menu, MenuEntries};
use super::menu_data::MenuEntry;
use crate::app_info::AppIcon;

/// The app's icon in the platform's tray, with a menu: what brings back an
/// app that runs without windows, shows it's still running and holds its
/// Quit. One per app: [`set_tray`](super::set_tray) replaces it, and it goes
/// when the scope that installed it ends. A click opens the menu on every
/// platform, so give it an item that opens the window.
///
/// ```ignore
/// set_tray(
///     TrayIcon::new("Player")
///         .icon(AppIcon::named("player-symbolic").image(AppIcon::bytes(include_bytes!("tray.png"))))
///         .tooltip(move || format!("{} songs queued", queue.get().len()))
///         .menu(Menu::new("").children((
///             MenuItem::new("Open Player").on_select(move || open.set(true)),
///             MenuSeparator,
///             MenuItem::new("Quit").role(MenuRole::Quit).on_select(quit),
///         ))),
/// );
/// ```
///
/// In `view!`, in a component that holds it as long as it should show,
/// its children are its menu's:
/// `<TrayIcon title="Player" icon=icon><MenuItem @select=quit>"Quit"</MenuItem></TrayIcon>`.
pub struct TrayIcon {
    pub(crate) title: Value<String>,
    pub(crate) icon: Option<Value<AppIcon>>,
    pub(crate) tooltip: Value<String>,
    pub(crate) menu: Menu,
}

impl TrayIcon {
    /// `title` is its accessible name, and its name where the platform
    /// lists tray items (Plasma's system tray settings, Windows' taskbar
    /// settings).
    pub fn new(title: impl IntoValue<String>) -> TrayIcon {
        TrayIcon {
            title: title.into_value(),
            icon: None,
            tooltip: Value::Static(String::new()),
            menu: Menu::new(String::new()),
        }
    }

    pub fn title(mut self, title: impl IntoValue<String>) -> TrayIcon {
        self.title = title.into_value();
        self
    }

    /// What it shows, which the app swaps to show state. Each platform
    /// wants its own: a monochrome template image on macOS (AppKit tints
    /// it), an icon name from the theme on Linux ([`AppIcon::named`], a
    /// `-symbolic` one where the theme has it), an `.ico` with the small
    /// sizes on Windows. Without, the app's icon. An image is read when
    /// the icon changes: to show other pixels, give other bytes or another
    /// path.
    pub fn icon(mut self, icon: impl IntoValue<AppIcon>) -> TrayIcon {
        self.icon = Some(icon.into_value());
        self
    }

    /// Text shown over it, and read by screen readers.
    pub fn tooltip(mut self, tooltip: impl IntoValue<String>) -> TrayIcon {
        self.tooltip = tooltip.into_value();
        self
    }

    /// What a click opens: the same menus the menu bar takes (its title
    /// isn't shown).
    pub fn menu(mut self, menu: Menu) -> TrayIcon {
        self.menu = menu;
        self
    }

    #[doc(hidden)]
    pub fn __tag() -> TrayIcon {
        TrayIcon::new(String::new())
    }

    /// `<TrayIcon>…</TrayIcon>`: its children are its menu's.
    #[doc(hidden)]
    pub fn __children<E: MenuEntries>(mut self, children: impl FnOnce() -> E) -> TrayIcon {
        self.menu = Menu::new(String::new()).children(children());
        self
    }
}

/// A tray icon as data, for [`Services::set_tray`](super::Services::set_tray).
/// Hidden items are left out.
#[derive(Clone, Debug, PartialEq)]
pub struct TrayData {
    pub title: String,
    /// `None`: the app's icon (`AppInfo::icon`, or the platform's for it).
    pub icon: Option<AppIcon>,
    /// Empty for none.
    pub tooltip: String,
    pub menu: Vec<MenuEntry>,
}
