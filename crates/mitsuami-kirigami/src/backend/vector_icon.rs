//! The app's own icons as SVG files, which Kirigami's icons and Qt Quick's
//! buttons take as their source: written once per icon to the user's cache,
//! named by their content, and reused for as long as they're there.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use mitsuami_core::VectorIcon;

/// The side the file is drawn at; it's a vector, so it scales to any size.
const SIDE: f32 = 16.0;

/// A `file://` URL of the icon's SVG, written if it isn't there yet; empty
/// if it can't be written, which shows no icon.
pub(super) fn file_url(icon: &VectorIcon) -> String {
    let svg = icon.svg(SIDE);
    let mut hasher = DefaultHasher::new();
    svg.hash(&mut hasher);
    let dir = cache_dir().join("mitsuami").join("icons");
    // `-symbolic`, as icon themes name the icons that take the text colour.
    let path = dir.join(format!("{:016x}-symbolic.svg", hasher.finish()));
    if !path.exists() && (std::fs::create_dir_all(&dir).is_err() || std::fs::write(&path, svg).is_err()) {
        return String::new();
    }
    format!("file://{}", path.display())
}

/// XDG's cache folder, as KDE's apps keep theirs.
fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
}
