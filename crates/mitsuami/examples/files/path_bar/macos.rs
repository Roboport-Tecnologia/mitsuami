//! The path bar's AppKit render: `NSPathControl`, as Finder's View › Show
//! Path Bar shows it. It shows each folder's icon and name, and shortens
//! the middle of a path too long for it.

use std::path::PathBuf;

use mitsuami::appkit::objc2::rc::Retained;
use mitsuami::appkit::objc2_app_kit::{NSControlSize, NSPathControl, NSPathStyle};
use mitsuami::appkit::objc2_foundation::{NSString, NSURL};
use mitsuami::appkit::{AppKitCx, NativeRender};
use mitsuami::prelude::*;

use super::{PathBar, PathBarEvent, PathBarProps};

fn url(path: &std::path::Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

fn path_of(url: &NSURL) -> Option<PathBuf> {
    Some(PathBuf::from(url.path()?.to_string()))
}

impl NativeRender for PathBar {
    type View = NSPathControl;

    fn create(props: &PathBarProps, cx: &mut AppKitCx) -> Retained<NSPathControl> {
        let control = NSPathControl::new(cx.mtm());
        control.setPathStyle(NSPathStyle::Standard);
        // Finder's is small, along the bottom of the window.
        control.setControlSize(NSControlSize::Small);
        control.setURL(Some(&url(&props.path)));
        // A click sends the action with the folder clicked; the control
        // doesn't go there itself, the app does.
        let emitter = cx.emitter();
        cx.on_action(&*control, move |control: &NSPathControl| {
            if let Some(folder) = control.clickedPathItem().and_then(|item| item.URL()).and_then(|url| path_of(&url)) {
                emitter.emit(PathBarEvent::Chosen(folder));
            }
        });
        control
    }

    fn update(control: &NSPathControl, _old: &PathBarProps, new: &PathBarProps) {
        control.setURL(Some(&url(&new.path)));
    }

    /// As tall as AppKit makes it; as wide as the layout lets it be, since
    /// it shortens the path to fit.
    fn measure(control: &NSPathControl, _props: &PathBarProps, _request: &MeasureRequest) -> Option<Size> {
        let height = control.intrinsicContentSize().height;
        Some(Size::new(0.0, height.ceil() as f32))
    }

    fn read(control: &NSPathControl, props: &PathBarProps) -> PathBarProps {
        let path = control.URL().and_then(|url| path_of(&url)).unwrap_or_else(|| props.path.clone());
        PathBarProps { path }
    }
}
