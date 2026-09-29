//! The file icon's GTK render: the icon GIO gives the file (its content
//! type's, from the icon theme, as Nautilus shows it), in a `gtk::Image`.

use mitsuami::gtk::gtk::prelude::*;
use mitsuami::gtk::gtk::{self, gio};
use mitsuami::gtk::{GtkCx, NativeRender};
use mitsuami::prelude::*;

use super::{FileIcon, FileIconProps};

fn show(image: &gtk::Image, props: &FileIconProps) {
    // A local file's type is read from its name and first bytes: quick
    // enough for the rows in view, which are all GTK builds.
    let info = gio::File::for_path(&props.path).query_info(
        "standard::icon",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    );
    match info.ok().and_then(|info| info.icon()) {
        Some(icon) => image.set_from_gicon(&icon),
        None => image.set_icon_name(Some(&props.fallback)),
    }
    image.set_pixel_size(props.size.round() as i32);
}

impl NativeRender for FileIcon {
    type Widget = gtk::Image;

    fn create(props: &FileIconProps, _cx: &mut GtkCx) -> gtk::Image {
        let image = gtk::Image::new();
        show(&image, props);
        image
    }

    fn update(image: &gtk::Image, _old: &FileIconProps, new: &FileIconProps) {
        show(image, new);
    }

    fn measure(_image: &gtk::Image, props: &FileIconProps, _request: &MeasureRequest) -> Option<Size> {
        Some(Size::new(props.size, props.size))
    }
}
