//! The file system side: reading folders and changing them. Everything
//! here blocks, so the browser runs it with `spawn_blocking`.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mitsuami::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// In bytes; folders have none.
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

impl Entry {
    fn read(path: PathBuf) -> Option<Entry> {
        // Follows links, as file managers show what they point at.
        let meta = std::fs::metadata(&path).or_else(|_| std::fs::symlink_metadata(&path)).ok()?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        Some(Entry {
            is_dir: meta.is_dir(),
            size: (!meta.is_dir()).then_some(meta.len()),
            modified: meta.modified().ok(),
            name,
            path,
        })
    }

    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }

    fn extension(&self) -> String {
        Path::new(&self.name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
    }

    pub fn is_image(&self) -> bool {
        !self.is_dir && matches!(self.extension().as_str(), "png" | "jpg" | "jpeg")
    }

    pub fn is_text(&self) -> bool {
        !self.is_dir
            && matches!(
                self.extension().as_str(),
                "txt"
                    | "md"
                    | "rs"
                    | "toml"
                    | "json"
                    | "yml"
                    | "yaml"
                    | "sh"
                    | "py"
                    | "js"
                    | "ts"
                    | "html"
                    | "css"
                    | "c"
                    | "h"
                    | "cpp"
                    | "swift"
                    | "log"
                    | "csv"
                    | "xml"
            )
    }

    pub fn is_app(&self) -> bool {
        self.extension() == "app" || self.extension() == "exe"
    }

    /// What it is, as file managers say in their Kind column.
    pub fn kind(&self) -> String {
        if self.is_app() {
            return "Application".into();
        }
        if self.is_dir {
            return "Folder".into();
        }
        match self.extension().as_str() {
            "" => "Document".into(),
            "txt" => "Plain text".into(),
            "md" => "Markdown".into(),
            "png" => "PNG image".into(),
            "jpg" | "jpeg" => "JPEG image".into(),
            "pdf" => "PDF document".into(),
            "zip" => "ZIP archive".into(),
            "rs" => "Rust source".into(),
            ext => format!("{} file", ext.to_uppercase()),
        }
    }

    /// An SF Symbol, a theme icon, a Segoe Fluent Icons glyph.
    pub fn icon(&self) -> &'static str {
        if self.is_app() {
            platform! {
                macos => "app", gtk => "application-x-executable-symbolic",
                kde => "application-x-executable", windows => "\u{ECAA}",
            }
        } else if self.is_dir {
            platform! { macos => "folder", gtk => "folder-symbolic", kde => "folder", windows => "\u{E8B7}" }
        } else if self.is_image() {
            platform! {
                macos => "photo", gtk => "image-x-generic-symbolic", kde => "image-x-generic", windows => "\u{EB9F}",
            }
        } else {
            platform! {
                macos => "doc", gtk => "text-x-generic-symbolic", kde => "text-plain", windows => "\u{E8A5}",
            }
        }
    }
}

/// The folder's entries, unsorted.
pub fn read_folder(folder: &Path) -> Result<Vec<Entry>, String> {
    let entries = std::fs::read_dir(folder).map_err(|e| describe(&e))?;
    Ok(entries.filter_map(|e| Entry::read(e.ok()?.path())).collect())
}

/// The first lines of a text file, for its preview.
pub fn head(path: &Path, lines: usize) -> Result<String, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path).and_then(|f| f.take(4096).read_to_end(&mut bytes)).map_err(|e| describe(&e))?;
    Ok(String::from_utf8_lossy(&bytes).lines().take(lines).collect::<Vec<_>>().join("\n"))
}

/// How many items a folder holds, for its preview.
pub fn count(folder: &Path) -> Option<usize> {
    Some(std::fs::read_dir(folder).ok()?.count())
}

pub fn describe(error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "You don't have permission.".into(),
        io::ErrorKind::NotFound => "It isn't there any more.".into(),
        io::ErrorKind::AlreadyExists => "An item with that name already exists.".into(),
        _ => error.to_string(),
    }
}

/// `name` in `folder`, or `name 2`, `name 3`… if it's taken. The number
/// goes before the extension.
pub fn free_name(folder: &Path, name: &str) -> PathBuf {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_owned(), format!(".{ext}")),
        _ => (name.to_owned(), String::new()),
    };
    (1..)
        .map(|n| folder.join(if n == 1 { name.to_owned() } else { format!("{stem} {n}{ext}") }))
        .find(|p| !p.exists() && p.symlink_metadata().is_err())
        .expect("some number is free")
}

pub fn new_folder(folder: &Path) -> Result<PathBuf, String> {
    let path = free_name(folder, "untitled folder");
    std::fs::create_dir(&path).map_err(|e| describe(&e))?;
    Ok(path)
}

/// Renames in place. Names can't hold a path separator.
pub fn rename(path: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(format!("\u{201C}{name}\u{201D} can't be used as a name."));
    }
    let to = path.with_file_name(name);
    if to == path {
        return Ok(to);
    }
    // Case-only renames are the same file on case-insensitive disks.
    let same_file = to.to_string_lossy().to_lowercase() == path.to_string_lossy().to_lowercase();
    if to.exists() && !same_file {
        return Err(format!("The name \u{201C}{name}\u{201D} is already taken."));
    }
    std::fs::rename(path, &to).map_err(|e| describe(&e))?;
    Ok(to)
}

/// Copies files and folders into `folder`, beside what's there: a name
/// that's taken gets a number. `suffix` goes after the stem ("copy").
pub fn copy_into(paths: &[PathBuf], folder: &Path, suffix: &str) -> Result<Vec<PathBuf>, String> {
    let mut copies = Vec::new();
    for path in paths {
        let name = path.file_name().ok_or("Can't copy that.")?.to_string_lossy().into_owned();
        let name = match suffix {
            "" => name,
            _ => match name.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() && !path.is_dir() => format!("{stem} {suffix}.{ext}"),
                _ => format!("{name} {suffix}"),
            },
        };
        let to = free_name(folder, &name);
        if folder.starts_with(path) {
            return Err(format!("\u{201C}{name}\u{201D} can't be copied into itself."));
        }
        copy(path, &to).map_err(|e| describe(&e))?;
        copies.push(to);
    }
    Ok(copies)
}

fn copy(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Where trashed items go.
#[derive(Clone, Debug, PartialEq)]
pub enum Trash {
    /// The user's: through `NSFileManager` on macOS and GIO on GNOME, so
    /// Finder's and Nautilus's Put Back work; the freedesktop.org trash by
    /// hand on KDE. Windows' Recycle Bin is only reachable through the
    /// shell's API (`IFileOperation`), so there items are deleted, after
    /// asking.
    Platform,
    /// A folder of the app's own, for tests.
    #[allow(dead_code)]
    Folder(PathBuf),
}

impl Trash {
    /// Whether trashing deletes for good, so the app should ask first.
    pub fn deletes(&self) -> bool {
        *self == Trash::Platform && cfg!(windows)
    }
}

pub fn trash(paths: &[PathBuf], trash: &Trash) -> Result<(), String> {
    for path in paths {
        match trash {
            Trash::Folder(folder) => move_into(path, folder)?,
            Trash::Platform => platform_trash(path)?,
        }
    }
    Ok(())
}

fn move_into(path: &Path, folder: &Path) -> Result<(), String> {
    let name = path.file_name().ok_or("Can't move that.")?.to_string_lossy().into_owned();
    std::fs::create_dir_all(folder).map_err(|e| describe(&e))?;
    std::fs::rename(path, free_name(folder, &name)).map_err(|e| describe(&e))
}

#[cfg(target_os = "macos")]
fn platform_trash(path: &Path) -> Result<(), String> {
    use mitsuami::appkit::objc2_foundation::{NSFileManager, NSString, NSURL};
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|e| e.localizedDescription().to_string())
}

#[cfg(all(target_os = "linux", not(feature = "kde")))]
fn platform_trash(path: &Path) -> Result<(), String> {
    use mitsuami::gtk::gtk::gio::{self, prelude::*};
    gio::File::for_path(path).trash(gio::Cancellable::NONE).map_err(|e| e.message().to_owned())
}

/// The freedesktop.org trash, as KIO fills it: the item in `files`, and a
/// `.trashinfo` beside it in `info` that lets Dolphin put it back.
#[cfg(all(target_os = "linux", feature = "kde"))]
fn platform_trash(path: &Path) -> Result<(), String> {
    let trash = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
        .join("Trash");
    let (files, info) = (trash.join("files"), trash.join("info"));
    std::fs::create_dir_all(&files).and_then(|()| std::fs::create_dir_all(&info)).map_err(|e| describe(&e))?;
    let name = path.file_name().ok_or("Can't move that.")?.to_string_lossy().into_owned();
    let to = free_name(&files, &name);
    let stored = to.file_name().unwrap().to_string_lossy().into_owned();
    let when = civil(SystemTime::now()).replace(' ', "T") + ":00";
    let text = format!("[Trash Info]\nPath={}\nDeletionDate={when}\n", path.display());
    std::fs::write(info.join(format!("{stored}.trashinfo")), text).map_err(|e| describe(&e))?;
    std::fs::rename(path, &to).map_err(|e| describe(&e))
}

#[cfg(windows)]
fn platform_trash(path: &Path) -> Result<(), String> {
    let result = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
    result.map_err(|e| describe(&e))
}

/// Opens a file with the app the platform picks for it: `NSWorkspace` on
/// macOS, GTK's file launcher on GNOME (it asks which app when none is
/// set), and the desktop's opener elsewhere.
pub fn launch(path: &Path) -> Result<(), String> {
    platform! {
        macos => {
            use mitsuami::appkit::objc2_app_kit::NSWorkspace;
            use mitsuami::appkit::objc2_foundation::{NSString, NSURL};
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            if NSWorkspace::sharedWorkspace().openURL(&url) { Ok(()) } else { Err("No app can open it.".into()) }
        },
        gtk => {
            use mitsuami::gtk::gtk::{self, gio};
            gtk::FileLauncher::new(Some(&gio::File::for_path(path))).launch(
                None::<&gtk::Window>,
                gio::Cancellable::NONE,
                |_| {},
            );
            Ok(())
        },
        kde => spawn("xdg-open", path),
        windows => spawn("explorer", path),
    }
}

#[cfg(any(windows, all(target_os = "linux", feature = "kde")))]
fn spawn(opener: &str, path: &Path) -> Result<(), String> {
    std::process::Command::new(opener).arg(path).spawn().map(|_| ()).map_err(|e| describe(&e))
}

pub fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// The folders down the sidebar: title, path, icon.
pub fn favourites() -> Vec<(String, PathBuf, &'static str)> {
    let home = home();
    let user = home.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Home".into());
    let places = [
        (
            user.as_str(),
            home.clone(),
            platform! {
                macos => "house", gtk => "user-home-symbolic", kde => "user-home", windows => "\u{E80F}",
            },
        ),
        (
            "Desktop",
            home.join("Desktop"),
            platform! {
                macos => "menubar.dock.rectangle", gtk => "user-desktop-symbolic", kde => "user-desktop", windows => "\u{E7F4}",
            },
        ),
        (
            "Documents",
            home.join("Documents"),
            platform! {
                macos => "doc", gtk => "folder-documents-symbolic", kde => "folder-documents", windows => "\u{E8A5}",
            },
        ),
        (
            "Downloads",
            home.join("Downloads"),
            platform! {
                macos => "arrow.down.circle", gtk => "folder-download-symbolic", kde => "folder-download", windows => "\u{E896}",
            },
        ),
        (
            "Pictures",
            home.join("Pictures"),
            platform! {
                macos => "photo", gtk => "folder-pictures-symbolic", kde => "folder-pictures", windows => "\u{EB9F}",
            },
        ),
        (
            "Applications",
            PathBuf::from("/Applications"),
            platform! {
                macos => "square.grid.2x2", gtk => "", kde => "", windows => "",
            },
        ),
    ];
    places.into_iter().filter(|(_, path, _)| path.is_dir()).map(|(t, p, i)| (t.to_owned(), p, i)).collect()
}

/// The disk: `/`, or the drive the home folder is on.
pub fn computer() -> (String, PathBuf, &'static str) {
    let root = home().ancestors().last().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/"));
    let title = platform! { macos => "Macintosh HD", linux => "Computer", windows => "Local Disk" };
    let icon = platform! {
        macos => "internaldrive", gtk => "drive-harddisk-symbolic", kde => "drive-harddisk", windows => "\u{EDA2}",
    };
    (title.to_owned(), root, icon)
}

/// `1.2 MB`, in the decimal units file managers use.
pub fn human_size(bytes: u64) -> String {
    match bytes {
        0 => "Zero bytes".into(),
        1 => "1 byte".into(),
        b if b < 1000 => format!("{b} bytes"),
        b => {
            let units = ["KB", "MB", "GB", "TB"];
            let mut value = b as f64 / 1000.0;
            let mut unit = 0;
            while value >= 999.5 && unit < units.len() - 1 {
                value /= 1000.0;
                unit += 1;
            }
            if value < 10.0 { format!("{value:.1} {}", units[unit]) } else { format!("{value:.0} {}", units[unit]) }
        }
    }
}

/// `2026-09-29 14:03`, in UTC: the standard library has no time zones.
pub fn civil(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02} {:02}:{:02}", rest / 3600, rest % 3600 / 60)
}
