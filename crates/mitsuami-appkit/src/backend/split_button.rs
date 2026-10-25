//! Split buttons: an `NSComboButton` of the split style (macOS 13), or
//! before it, a segmented control with a menu on its second segment, as
//! apps paired them then. Both show their segments to assistive technology
//! as a button and a menu button.

use mitsuami_core::backend::EventSink;
use mitsuami_core::services::MenuEntry;
use mitsuami_core::{NodeId, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSComboButton, NSControl, NSImage, NSMenu, NSMenuItem, NSSegmentSwitchTracking, NSSegmentedControl,
};
use objc2_foundation::{NSArray, NSString};

use crate::classes::ClosureTarget;
use crate::services::ItemTarget;

use super::ns;

pub(super) struct SplitButton {
    /// The `NSComboButton`, or the segmented control.
    pub(super) control: Retained<NSControl>,
    /// The target of the menu's items, which only hold weak references.
    target: Retained<ClosureTarget>,
    /// The menu the core sent, for what AppKit can't give back.
    pub(super) sent: Vec<MenuEntry>,
    /// Its caption, which an icon shown alone keeps as its accessible name.
    title: String,
    icon_only: bool,
}

impl SplitButton {
    /// The button's own action goes to `target`'s `action`.
    pub(super) fn new(
        mtm: MainThreadMarker,
        id: NodeId,
        events: EventSink,
        target: Option<&AnyObject>,
        action: Sel,
    ) -> SplitButton {
        let control: Retained<NSControl> = if AnyClass::get(c"NSComboButton").is_some() {
            let combo = unsafe {
                NSComboButton::comboButtonWithTitle_menu_target_action(&ns(""), None, target, Some(action), mtm)
            };
            combo.into_super()
        } else {
            let segments = NSSegmentedControl::new(mtm);
            segments.setSegmentCount(2);
            segments.setTrackingMode(NSSegmentSwitchTracking::Momentary);
            segments.setShowsMenuIndicator_forSegment(true, 1);
            unsafe {
                segments.setTarget(target);
                segments.setAction(Some(action));
            }
            segments.into_super()
        };
        let menu_target = ClosureTarget::new(mtm, move |sender| {
            if let Some(item) = sender.downcast_ref::<NSMenuItem>() {
                events.emit(id, UiEvent::MenuItem(item.tag() as u32));
            }
        });
        SplitButton { control, target: menu_target, sent: Vec::new(), title: String::new(), icon_only: false }
    }

    /// Shows its caption, icon and menu.
    pub(super) fn show(&mut self, title: &str, icon: Option<Retained<NSImage>>, icon_only: bool) {
        let mtm = MainThreadMarker::from(&*self.control);
        self.title = title.to_owned();
        self.icon_only = icon_only;
        let shown = if icon_only { "" } else { title };
        let menu =
            crate::services::context_menu(mtm, &self.sent, ItemTarget { object: &self.target, action: sel!(fire:) });
        if let Some(combo) = self.control.downcast_ref::<NSComboButton>() {
            combo.setTitle(&ns(shown));
            combo.setImage(icon.as_deref());
            unsafe { combo.setMenu(&menu) };
        } else if let Some(segments) = self.control.downcast_ref::<NSSegmentedControl>() {
            segments.setLabel_forSegment(&ns(shown), 0);
            segments.setImage_forSegment(icon.as_deref(), 0);
            segments.setMenu_forSegment(Some(&menu), 1);
        }
        // The caption stays its accessible name when only the icon shows.
        self.control.setAccessibilityLabel(Some(&ns(title)));
    }

    /// The caption it shows, or keeps as its name behind an icon alone.
    pub(super) fn title(&self) -> String {
        let shown = match self.control.downcast_ref::<NSComboButton>() {
            Some(combo) => combo.title().to_string(),
            None => self
                .control
                .downcast_ref::<NSSegmentedControl>()
                .and_then(|s| s.labelForSegment(0))
                .map(|l| l.to_string())
                .unwrap_or_default(),
        };
        if shown.is_empty() && self.icon_only { self.title.clone() } else { shown }
    }

    /// Whether only its icon shows: it shows no caption, though it has one.
    pub(super) fn icon_only(&self) -> bool {
        let shown = match self.control.downcast_ref::<NSComboButton>() {
            Some(combo) => combo.title().to_string(),
            None => self
                .control
                .downcast_ref::<NSSegmentedControl>()
                .and_then(|s| s.labelForSegment(0))
                .map(|l| l.to_string())
                .unwrap_or_default(),
        };
        shown.is_empty() && (self.icon_only || !self.title.is_empty())
    }

    pub(super) fn menu(&self) -> Option<Retained<NSMenu>> {
        match self.control.downcast_ref::<NSComboButton>() {
            Some(combo) => Some(combo.menu()),
            None => self.control.downcast_ref::<NSSegmentedControl>().and_then(|s| s.menuForSegment(1)),
        }
    }

    /// What VoiceOver presses: the button segment, which sends the action
    /// (pressing the combo button itself does nothing). What it returns is
    /// unreliable for windows that aren't on screen, so it's ignored.
    pub(super) fn press(&self) -> bool {
        fn button(element: &AnyObject) -> Option<Retained<AnyObject>> {
            let children: Option<Retained<NSArray<AnyObject>>> = unsafe { msg_send![element, accessibilityChildren] };
            for child in children?.iter() {
                let role: Option<Retained<NSString>> = unsafe { msg_send![&*child, accessibilityRole] };
                match role.map(|r| r.to_string()).as_deref() {
                    Some("AXButton") => return Some(child),
                    Some("AXMenuButton") => {}
                    _ => {
                        if let Some(found) = button(&child) {
                            return Some(found);
                        }
                    }
                }
            }
            None
        }
        let Some(segment) = button(&self.control) else { return false };
        let _: bool = unsafe { msg_send![&*segment, accessibilityPerformPress] };
        true
    }
}

/// Whether the action came from the button, not the menu's segment.
pub(crate) fn is_press(sender: &AnyObject) -> bool {
    sender.downcast_ref::<NSSegmentedControl>().is_none_or(|s| s.selectedSegment() == 0)
}
