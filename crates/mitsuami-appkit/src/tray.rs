//! The tray icon on macOS: a status item in the menu bar
//! (`NSStatusBar.systemStatusBar`), its image a template that AppKit
//! tints for the bar's light, dark and highlighted looks, and its menu
//! built as context menus are. A click opens the menu, as every status
//! item's does. Its autosave name is the app's id, so macOS keeps where
//! the user dragged it.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::services::TrayData;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSSize, NSString};

use crate::backend::app_icon_image;
use crate::classes::ClosureTarget;
use crate::services::{ItemTarget, context_menu};

/// How tall a status item's image is, in points: the menu bar's.
const IMAGE_HEIGHT: f64 = 18.0;

/// What a chosen item reports to: the last `activate` the core gave.
type Activate = Rc<RefCell<Option<Rc<dyn Fn(u32)>>>>;

pub(crate) struct Tray {
    mtm: MainThreadMarker,
    item: Option<Retained<NSStatusItem>>,
    /// Its menu's items' target, which they hold weakly.
    target: Retained<ClosureTarget>,
    activate: Activate,
    shown: bool,
}

impl Tray {
    pub(crate) fn new(mtm: MainThreadMarker) -> Tray {
        let activate: Activate = Rc::default();
        let target = ClosureTarget::new(mtm, {
            let activate = activate.clone();
            move |sender| {
                let chosen = activate.borrow().clone();
                if let (Some(item), Some(activate)) = (sender.downcast_ref::<NSMenuItem>(), chosen) {
                    activate(item.tag() as u32);
                }
            }
        });
        Tray { mtm, item: None, target, activate, shown: false }
    }

    pub(crate) fn set(
        &mut self,
        tray: Option<&TrayData>,
        app_id: Option<&str>,
        activate: Rc<dyn Fn(u32)>,
        shown: Rc<dyn Fn(bool)>,
    ) {
        *self.activate.borrow_mut() = Some(activate);
        let Some(tray) = tray else {
            if let Some(item) = self.item.take() {
                NSStatusBar::systemStatusBar().removeStatusItem(&item);
            }
            self.report(false, &shown);
            return;
        };
        let item = self.item.get_or_insert_with(|| {
            let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
            if let Some(id) = app_id {
                item.setAutosaveName(Some(&NSString::from_str(id)));
            }
            item
        });
        if let Some(button) = item.button(self.mtm) {
            // The app's own icon, in colour, when it gives none.
            let image = match &tray.icon {
                Some(icon) => app_icon_image(icon).inspect(|image| image.setTemplate(true)),
                None => NSApplication::sharedApplication(self.mtm).applicationIconImage(),
            };
            if let Some(image) = &image {
                let size = image.size();
                let width = if size.height > 0.0 { size.width * IMAGE_HEIGHT / size.height } else { IMAGE_HEIGHT };
                image.setSize(NSSize::new(width, IMAGE_HEIGHT));
            }
            button.setImage(image.as_deref());
            let tooltip = (!tray.tooltip.is_empty()).then(|| NSString::from_str(&tray.tooltip));
            button.setToolTip(tooltip.as_deref());
            button.setAccessibilityLabel(Some(&NSString::from_str(&tray.title)));
        }
        let target = ItemTarget { object: &self.target, action: sel!(fire:) };
        item.setMenu(Some(&context_menu(self.mtm, &tray.menu, target)));
        self.report(true, &shown);
    }

    fn report(&mut self, showing: bool, shown: &Rc<dyn Fn(bool)>) {
        if self.shown != showing {
            self.shown = showing;
            shown(showing);
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        if let Some(item) = self.item.take() {
            NSStatusBar::systemStatusBar().removeStatusItem(&item);
        }
    }
}
