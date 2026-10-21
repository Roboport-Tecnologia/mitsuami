//! The app's own icons, as GTK shows a theme's symbolic ones: each is an
//! SVG file whose name ends in `-symbolic.svg`, which GTK recolours with
//! the widget's CSS colour, shown through a `gio::FileIcon`.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};
use mitsuami_core::VectorIcon;

thread_local! {
    /// Each icon's file, by its SVG, so an icon is written once.
    static FILES: std::cell::RefCell<HashMap<String, Option<gio::Icon>>> = Default::default();
}

/// The side the SVG is drawn at: GTK scales it to the size it shows it.
const SIDE: f32 = 16.0;

/// The icon as a file GTK reads as symbolic; `None` if it can't be
/// written, and the widget then shows its named icon, if any.
pub(super) fn gicon(icon: &VectorIcon) -> Option<gio::Icon> {
    let svg = icon.svg(SIDE);
    FILES.with(|files| files.borrow_mut().entry(svg.clone()).or_insert_with(|| write(&svg)).clone())
}

fn write(svg: &str) -> Option<gio::Icon> {
    let mut hasher = DefaultHasher::new();
    svg.hash(&mut hasher);
    let dir: PathBuf = glib::user_cache_dir().join("mitsuami").join("icons");
    let path = dir.join(format!("{:016x}-symbolic.svg", hasher.finish()));
    if !path.exists() {
        // Written aside and moved in, so another app never reads half a file.
        let written = std::fs::create_dir_all(&dir).and_then(|_| {
            let partial = path.with_extension(format!("{}.part", std::process::id()));
            std::fs::write(&partial, svg)?;
            std::fs::rename(&partial, &path)
        });
        if let Err(e) = written {
            eprintln!("mitsuami: can't write an icon to {}: {e}", path.display());
            return None;
        }
    }
    Some(gio::FileIcon::new(&gio::File::for_path(&path)).upcast())
}

/// Whether an image shows one of these icons (a file icon, not a name).
pub(super) fn shows_gicon(image: &gtk::Image) -> bool {
    image.storage_type() == gtk::ImageType::Gicon
}
