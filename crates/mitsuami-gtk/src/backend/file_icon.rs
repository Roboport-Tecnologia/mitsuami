//! Files' icons: the icon GIO gives a file (its content type's, from the
//! icon theme, as Files shows it), or the thumbnail the desktop has made
//! of it.
//!
//! Read as Files and GTK's file chooser read them, off the UI thread: the
//! file's info asynchronously (its type is sniffed from its first bytes,
//! which on a network mount can take seconds), then its thumbnail decoded
//! on a thread of GIO's. The icon shows first, then the thumbnail.

use std::cell::Cell;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::glib::translate::from_glib_full;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

/// How many loads are in progress: tests' settles wait for them.
pub(crate) type Loads = Rc<Cell<usize>>;

/// One load in progress, until it's dropped, however it ends.
struct Pending(Loads);

impl Pending {
    fn new(loads: &Loads) -> Pending {
        loads.set(loads.get() + 1);
        Pending(loads.clone())
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

/// Shows `path`'s icon in `image`, once it's read; with `thumbnail`, its
/// thumbnail from the cache after, if the desktop has made one. GTK makes
/// none itself: file managers do (Files through GNOME Desktop's
/// thumbnailers), and GTK's own file chooser too shows only those.
/// `shown` runs when the image changes, as its natural size may have.
///
/// Returns the load's cancellable: a new file, or the node going, cancels
/// it, and what it read then isn't shown.
pub(super) fn show(
    image: &gtk::Image,
    path: Option<&Path>,
    thumbnail: bool,
    loads: &Loads,
    shown: impl Fn() + 'static,
) -> Option<gio::Cancellable> {
    // Not the last file's icon while this one's is read.
    image.clear();
    let path = path?;
    let cancellable = gio::Cancellable::new();
    let pending = Pending::new(loads);
    let (weak, current) = (image.downgrade(), cancellable.clone());
    let attributes = "standard::icon,thumbnail::path,thumbnail::is-valid";
    gio::File::for_path(path).query_info_async(
        attributes,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::DEFAULT,
        Some(&cancellable),
        move |result| {
            let (Ok(info), Some(image)) = (result, weak.upgrade()) else { return };
            if current.is_cancelled() {
                return;
            }
            if let Some(icon) = info.icon() {
                image.set_from_gicon(&icon);
                shown();
            }
            let cached = (thumbnail && info.boolean("thumbnail::is-valid"))
                .then(|| info.attribute_byte_string("thumbnail::path"))
                .flatten();
            let Some(cached) = cached else { return };
            let decoded = gio::spawn_blocking(move || texture(&PathBuf::from(cached.as_str())));
            glib::MainContext::default().spawn_local(async move {
                let _pending = pending;
                let Ok(Ok(texture)) = decoded.await else { return };
                if let (false, Some(image)) = (current.is_cancelled(), weak.upgrade()) {
                    image.set_paintable(Some(&texture));
                    shown();
                }
            });
        },
    );
    Some(cancellable)
}

/// An image file decoded, on any thread. GDK's loaders are thread-safe
/// (`gdk_texture_new_from_filename` says so), but its Rust binding asserts
/// the main thread, so it's called directly.
fn texture(path: &Path) -> Result<gdk::Texture, glib::Error> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| glib::Error::new(gio::IOErrorEnum::InvalidFilename, "a NUL in the path"))?;
    let mut error = std::ptr::null_mut();
    // SAFETY: a valid C string; the texture or the error is ours.
    unsafe {
        let texture = gdk::ffi::gdk_texture_new_from_filename(path.as_ptr(), &mut error);
        if error.is_null() { Ok(from_glib_full(texture)) } else { Err(from_glib_full(error)) }
    }
}
