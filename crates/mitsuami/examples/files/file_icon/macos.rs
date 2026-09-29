//! The file icon's AppKit render: the icon Finder shows, from
//! `NSWorkspace`, in an `NSImageView`.

use std::path::Path;

use mitsuami::appkit::objc2::rc::Retained;
use mitsuami::appkit::objc2_app_kit::{NSImageScaling, NSImageView, NSWorkspace};
use mitsuami::appkit::objc2_foundation::{NSSize, NSString};
use mitsuami::appkit::{AppKitCx, NativeRender};
use mitsuami::prelude::*;

use super::{FileIcon, FileIconProps};

fn show(view: &NSImageView, path: &Path, size: f32) {
    let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path.to_string_lossy()));
    // Icons come in several sizes; this picks the one drawn.
    icon.setSize(NSSize::new(size as f64, size as f64));
    view.setImage(Some(&icon));
}

impl NativeRender for FileIcon {
    type View = NSImageView;

    fn create(props: &FileIconProps, cx: &mut AppKitCx) -> Retained<NSImageView> {
        let view = NSImageView::new(cx.mtm());
        view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        show(&view, &props.path, props.size);
        view
    }

    fn update(view: &NSImageView, old: &FileIconProps, new: &FileIconProps) {
        if (&old.path, old.size) != (&new.path, new.size) {
            show(view, &new.path, new.size);
        }
    }

    fn measure(_view: &NSImageView, props: &FileIconProps, _request: &MeasureRequest) -> Option<Size> {
        Some(Size::new(props.size, props.size))
    }
}
