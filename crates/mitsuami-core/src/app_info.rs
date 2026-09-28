//! Who the app is to the platform: its id, name and icon.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

/// The app's id, name and icon, which the platform shows in its task
/// switcher, dock or taskbar, and uses to match the app's windows to it.
/// Each platform takes what it has a place for, and a packaged app's own
/// (a macOS bundle's, an MSIX package's) wins over what's set here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppInfo {
    /// Reverse DNS, as `org.example.Player`: the Wayland app id and X11
    /// class on Linux, which match the app's `.desktop` file and its icon
    /// in the theme; the AppUserModelID of an unpackaged Windows app. A
    /// macOS app's id is its bundle's.
    pub id: Option<String>,
    /// The name people know it by: in the app menu's About, Hide and Quit
    /// on macOS, after each window's title on KDE, as GLib's application
    /// name on GTK. Windows takes it from the executable or its shortcut.
    pub name: Option<String>,
    /// Shown for the app on macOS (unless its bundle has an icon), on each
    /// window on Windows, and on KDE when the icon theme has none named
    /// after the id. GTK shows only the theme's icon named after the id.
    pub icon: Option<AppIcon>,
}

impl AppInfo {
    pub fn new() -> AppInfo {
        AppInfo::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> AppInfo {
        self.id = Some(id.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> AppInfo {
        self.name = Some(name.into());
        self
    }

    pub fn icon(mut self, icon: AppIcon) -> AppInfo {
        self.icon = Some(icon);
        self
    }
}

/// The app's icon, decoded by the platform: PNG everywhere, other formats
/// where the platform reads them (an `.ico` file on Windows, with its
/// sizes). Give it large (256 or 512 pixels square): platforms scale it
/// down.
#[derive(Clone, PartialEq)]
pub enum AppIcon {
    File(PathBuf),
    /// An image file's bytes, as `include_bytes!` gives them.
    Bytes(Arc<[u8]>),
}

impl AppIcon {
    pub fn file(path: impl Into<PathBuf>) -> AppIcon {
        AppIcon::File(path.into())
    }

    pub fn bytes(bytes: impl Into<Arc<[u8]>>) -> AppIcon {
        AppIcon::Bytes(bytes.into())
    }

    /// The image's bytes: read from its file, or as given.
    pub fn read(&self) -> Option<Arc<[u8]>> {
        match self {
            AppIcon::File(path) => std::fs::read(path).ok().map(Into::into),
            AppIcon::Bytes(bytes) => Some(bytes.clone()),
        }
    }
}

/// Its path or length, not its bytes.
impl fmt::Debug for AppIcon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppIcon::File(path) => f.debug_tuple("File").field(path).finish(),
            AppIcon::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
        }
    }
}

/// What a window shows of the app, read back from the platform for tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeAppInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub icon: Option<NativeIcon>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeIcon {
    /// An image this large: in pixels, or in points on AppKit, which keeps
    /// a copy at the screen's scale (a PNG without a resolution has as many
    /// points as pixels).
    Image { width: u32, height: u32 },
    /// The icon theme's icon of this name.
    Named(String),
}
