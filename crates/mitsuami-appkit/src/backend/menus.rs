//! Menu buttons' pull-downs, and choosing items of them and of context menus.

use mitsuami_core::NodeId;
use mitsuami_core::a11y::ActionError;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAccessibility, NSMenuItem};

use crate::services::ItemTarget;

use super::images::{image_position, symbol};
use super::{AppKitBackend, Widget, ns};

/// Shows a menu button's title, icon and menu: a pull-down shows its
/// first item as its title, so the menu is made again with that first.
pub(super) fn show_pull_down(widget: &Widget, title: &str, icon: Option<&str>, icon_only: Option<bool>) {
    let Widget::MenuButton { popup, target, sent } = widget else { return };
    let mtm = MainThreadMarker::from(&**popup);
    let menu = crate::services::context_menu(mtm, sent, ItemTarget { object: target, action: sel!(fire:) });
    let item = NSMenuItem::new(mtm);
    item.setTitle(&ns(title));
    item.setImage(icon.and_then(|name| symbol(name, None)).as_deref());
    menu.insertItem_atIndex(&item, 0);
    popup.setMenu(Some(&menu));
    popup.setImagePosition(image_position(icon_only));
    // The title stays its accessible name when only the image shows.
    popup.setAccessibilityLabel(Some(&ns(title)));
}

/// The title a menu button shows: its pull-down's first item's.
pub(super) fn pull_down_title(widget: &Widget) -> String {
    let Widget::MenuButton { popup, .. } = widget else { return String::new() };
    popup.itemAtIndex(0).map(|item| item.title().to_string()).unwrap_or_default()
}

impl AppKitBackend {
    /// Chooses an item of a menu button's pull-down, as the menu would.
    pub(super) fn choose_pull_down_item(&self, id: NodeId, item: u32) -> Result<(), ActionError> {
        let menu = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let Widget::MenuButton { popup, .. } = &node.widget else { return Err(ActionError::Unsupported) };
            if !popup.isEnabled() {
                return Err(ActionError::Disabled);
            }
            popup.menu()
        };
        // Tag 0 is the title item, never an app item.
        let (item, menu) = menu
            .filter(|_| item != 0)
            .and_then(|menu| crate::services::find_tagged(&menu, item))
            .ok_or(ActionError::Unsupported)?;
        if !item.isEnabled() {
            return Err(ActionError::Disabled);
        }
        // No state borrow: the item's target emits the choice.
        menu.performActionForItemAtIndex(menu.indexOfItem(&item));
        Ok(())
    }

    /// What VoiceOver does once it has shown a view's menu: press the
    /// item, which sends its action. A disabled control shows no menu.
    pub(super) fn choose_context_menu_item(&self, id: NodeId, item: u32) -> Result<(), ActionError> {
        let (menu, enabled) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let menu = match node.widget {
                Widget::Select(_) | Widget::MenuButton { .. } => None,
                _ => node.widget.view().menu(),
            };
            (menu, node.widget.control().is_none_or(|c| c.isEnabled()))
        };
        if !enabled {
            return Err(ActionError::Disabled);
        }
        let (item, menu) =
            menu.and_then(|menu| crate::services::find_tagged(&menu, item)).ok_or(ActionError::Unsupported)?;
        if !item.isEnabled() {
            return Err(ActionError::Disabled);
        }
        // No state borrow: the item's target emits the choice.
        menu.performActionForItemAtIndex(menu.indexOfItem(&item));
        Ok(())
    }
}
