//! Files' icons: the icon GIO gives a file (its content type's, from the
//! icon theme, as Files shows it), or the thumbnail the desktop has made
//! of it.

use std::path::Path;

use gtk::gio;
use gtk::prelude::*;

/// Shows `path`'s icon in `image`; with `thumbnail`, its thumbnail from
/// the cache instead, if the desktop has made one. GTK makes none itself:
/// file managers do (Files through GNOME Desktop's thumbnailers), and GTK's
/// own file chooser too shows only those.
pub(super) fn show(image: &gtk::Image, path: Option<&Path>, thumbnail: bool) {
    let Some(path) = path else { return image.clear() };
    // A local file's type is read from its name and first bytes: quick
    // enough for the rows in view, which are all GTK builds.
    let attributes = "standard::icon,thumbnail::path,thumbnail::is-valid";
    let Ok(info) =
        gio::File::for_path(path).query_info(attributes, gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
    else {
        return image.clear();
    };
    let cached = (thumbnail && info.boolean("thumbnail::is-valid"))
        .then(|| info.attribute_byte_string("thumbnail::path"))
        .flatten();
    match (cached, info.icon()) {
        (Some(thumbnail), _) => image.set_from_file(Some(thumbnail.as_str())),
        (None, Some(icon)) => image.set_from_gicon(&icon),
        (None, None) => image.clear(),
    }
}
