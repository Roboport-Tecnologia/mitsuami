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

/// How the app runs, beyond who it is: what `App` gives its backend's
/// `run_with`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RunOptions {
    /// One process per app id and user session: a later launch hands its
    /// arguments to the running one ([`Ui::reopen`](crate::Ui::reopen)) and
    /// ends, without starting the app. Needs [`AppInfo::id`].
    pub single_instance: bool,
}

impl RunOptions {
    pub fn new() -> RunOptions {
        RunOptions::default()
    }

    pub fn single_instance(mut self, single: bool) -> RunOptions {
        self.single_instance = single;
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
    /// An icon from the user's icon theme, by name, as Linux apps give
    /// theirs (installed with the app); `image` where there's no theme
    /// (macOS, Windows). See [`AppIcon::named`].
    Named {
        name: String,
        image: Option<Arc<AppIcon>>,
    },
}

impl AppIcon {
    pub fn file(path: impl Into<PathBuf>) -> AppIcon {
        AppIcon::File(path.into())
    }

    pub fn bytes(bytes: impl Into<Arc<[u8]>>) -> AppIcon {
        AppIcon::Bytes(bytes.into())
    }

    /// An icon from the icon theme, by name: Linux shows it; macOS and
    /// Windows show the [`image`](Self::image) given with it, or nothing.
    pub fn named(name: impl Into<String>) -> AppIcon {
        AppIcon::Named { name: name.into(), image: None }
    }

    /// The image a [`named`](Self::named) icon shows where there's no icon
    /// theme. Other icons are images already, and stay as they are.
    pub fn image(self, image: AppIcon) -> AppIcon {
        match self {
            AppIcon::Named { name, .. } => AppIcon::Named { name, image: Some(Arc::new(image)) },
            icon => icon,
        }
    }

    /// A named icon's name in the theme.
    pub fn name(&self) -> Option<&str> {
        match self {
            AppIcon::Named { name, .. } => Some(name),
            AppIcon::File(_) | AppIcon::Bytes(_) => None,
        }
    }

    /// The image's bytes: read from its file, or as given; a named icon's
    /// image's, if it has one.
    pub fn read(&self) -> Option<Arc<[u8]>> {
        match self {
            AppIcon::File(path) => std::fs::read(path).ok().map(Into::into),
            AppIcon::Bytes(bytes) => Some(bytes.clone()),
            AppIcon::Named { image, .. } => image.as_ref()?.read(),
        }
    }
}

/// Its path or length, not its bytes.
impl fmt::Debug for AppIcon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppIcon::File(path) => f.debug_tuple("File").field(path).finish(),
            AppIcon::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
            AppIcon::Named { name, image } => {
                f.debug_struct("Named").field("name", name).field("image", image).finish()
            }
        }
    }
}

impl mitsuami_reactive::IntoValue<AppIcon> for AppIcon {
    fn into_value(self) -> mitsuami_reactive::Value<AppIcon> {
        mitsuami_reactive::Value::Static(self)
    }
}

/// A name from the icon theme: `.icon("player-symbolic")`.
impl mitsuami_reactive::IntoValue<AppIcon> for &str {
    fn into_value(self) -> mitsuami_reactive::Value<AppIcon> {
        mitsuami_reactive::Value::Static(AppIcon::named(self))
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
