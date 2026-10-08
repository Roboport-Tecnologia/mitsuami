//! Menus: the menu bar, context menus and menu buttons' menus, built and
//! read back.

use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::a11y::ActionError;
use mitsuami_core::services::{MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData, Shortcut};
use mitsuami_core::{Key, NodeId};
use windows_core::{EventRevoker, Interface};

use super::windows::{apply_min_size, resize_client, title_content, update_title_bar_height};
use super::{ContextMenu, MenuBarPlace, MenuItems, Menus, R, WinUiBackend, WindowParts, key, ok};
use crate::bindings as w;

impl ContextMenu {
    pub(super) fn new(activate: Rc<dyn Fn(u32)>, own: Option<w::FlyoutBase>) -> ContextMenu {
        ContextMenu { sent: Vec::new(), flyout: None, items: MenuItems::default(), revokers: Vec::new(), activate, own }
    }

    pub(super) fn has_items(menu: &Option<ContextMenu>) -> bool {
        menu.as_ref().is_some_and(|menu| menu.flyout.is_some())
    }

    /// Shows `entries`: in place when only enabled and checked states
    /// changed, so an open menu stays open, as the menu bar does; else a
    /// new flyout (none for no entries). Whether the flyout was replaced.
    pub(super) fn update(&mut self, entries: &[MenuEntry], scope: String) -> R<bool> {
        let replaced = if self.flyout.is_some() && as_menu_bar(entries).same_structure(&as_menu_bar(&self.sent)) {
            update_items(&self.items, &as_menu_bar(entries));
            false
        } else if self.flyout.is_some() || !entries.is_empty() {
            self.revokers.clear();
            self.items.borrow_mut().clear();
            self.flyout = None;
            if !entries.is_empty() {
                let mut built =
                    MenuBuild { activate: &self.activate, revokers: &mut self.revokers, items: &self.items, scope };
                self.flyout = Some(built.flyout(entries)?);
            }
            true
        } else {
            false
        };
        self.sent = entries.to_vec();
        Ok(replaced)
    }
}

/// Entries as one menu, to compare and update them as menu bars are.
fn as_menu_bar(entries: &[MenuEntry]) -> MenuBarData {
    MenuBarData { menus: vec![MenuData { title: String::new(), entries: entries.to_vec() }] }
}

impl Menus {
    /// What a window's bar shows: the app's menus, and its own. A dialog
    /// shows only its own: Windows dialogs have no menu bar.
    fn shown(&self, window: NodeId, modal: bool) -> MenuBarData {
        self.app.for_window(self.windows.get(&window), modal)
    }
}

/// Brings a window's bar up to date with the menus it shows: in place
/// when only enabled and checked states changed, so an open menu stays
/// open, and rebuilt otherwise.
pub(super) fn refresh_menu(parts: &mut WindowParts, menus: &Menus) {
    let shown = menus.shown(parts.node, parts.modal.is_some());
    if parts.menu_bar.is_some() && shown.same_structure(&parts.menu_shown) {
        update_menu(parts, &shown);
    } else if shown != parts.menu_shown {
        let activate = menus.activate.clone().unwrap_or_else(|| Rc::new(|_| {}));
        install_menu(parts, &shown, &activate);
    }
    parts.menu_shown = shown;
}

fn update_menu(parts: &WindowParts, menu: &MenuBarData) {
    update_items(&parts.menu_items, menu);
}

/// Shows new enabled and checked states on built items.
fn update_items(items: &MenuItems, menu: &MenuBarData) {
    let mut items = items.borrow_mut();
    for data in menu.items() {
        let Some((item, check)) = items.get_mut(&data.id) else { continue };
        _ = item.cast::<w::IControl>().and_then(|c| c.SetIsEnabled(data.enabled));
        *check = data.check;
        show_check(item, data.check);
    }
}

/// Sets what a toggle or radio item shows. Only `Click` reports a choice,
/// so this never does.
fn show_check(item: &w::MenuFlyoutItemBase, check: MenuCheck) {
    match check {
        MenuCheck::None => {}
        MenuCheck::Check(on) => _ = item.cast::<w::IToggleMenuFlyoutItem>().and_then(|t| t.SetIsChecked(on)),
        MenuCheck::Radio(on) => _ = item.cast::<w::IRadioMenuFlyoutItem>().and_then(|r| r.SetIsChecked(on)),
    }
}

fn install_menu(parts: &mut WindowParts, menu: &MenuBarData, activate: &Rc<dyn Fn(u32)>) {
    let in_title_bar = parts.menu_bar_place == MenuBarPlace::InTitleBar;
    let parent: R<w::Panel> =
        if in_title_bar { title_content(parts).and_then(|g| g.cast()) } else { parts.bars.cast() };
    let children = ok(parent.and_then(|p| p.cast::<w::IPanel>()?.Children()), "menu bar's parent's children");
    if let Some(old) = parts.menu_bar.take() {
        let old: w::UIElement = ok(old.cast(), "menu bar element");
        let mut index = 0;
        if children.IndexOf(&old, &mut index).unwrap_or(false) {
            _ = children.RemoveAt(index);
        }
    }
    parts.menu_revokers.clear();
    parts.menu_items.borrow_mut().clear();
    if !menu.menus.is_empty() {
        let mut built = MenuBuild {
            activate,
            revokers: &mut parts.menu_revokers,
            items: &parts.menu_items,
            scope: format!("window-{}", parts.node),
        };
        let menu_bar = ok(built.menu_bar(menu), "building the menu bar");
        let element: w::UIElement = ok(menu_bar.cast(), "menu bar element");
        if in_title_bar {
            // Centred in the title bar's height, on its background.
            let fe = ok(element.cast::<w::IFrameworkElement>(), "menu bar element");
            _ = fe.SetVerticalAlignment(w::VerticalAlignment::Center);
            _ = menu_bar.cast::<w::IControl>().and_then(|c| c.SetBackground(None::<&w::Brush>));
        }
        _ = children.Append(&element);
        parts.menu_bar = Some(menu_bar);
    }
    if in_title_bar {
        _ = update_title_bar_height(parts);
    }
    apply_min_size(parts);
    if let Some(size) = parts.requested {
        resize_client(parts, size);
    }
}

/// Builds a window's `MenuBar`, or a context menu's `MenuFlyout`: the
/// same items. Items with a role stay where the app put them: Windows has
/// no standard place of its own for About, Settings or Exit, and no
/// standard titles or shortcuts for them.
struct MenuBuild<'a> {
    activate: &'a Rc<dyn Fn(u32)>,
    revokers: &'a mut Vec<EventRevoker>,
    items: &'a MenuItems,
    /// Where its radio groups' names are unique: XAML's are the thread's,
    /// and ids only the menu's.
    scope: String,
}

impl MenuBuild<'_> {
    fn menu_bar(&mut self, menu: &MenuBarData) -> R<w::MenuBar> {
        let menu_bar = w::MenuBar::new()?;
        let menus = menu_bar.cast::<w::IMenuBar>()?.Items()?;
        for data in &menu.menus {
            let item = w::MenuBarItem::new()?;
            let item_iface: w::IMenuBarItem = item.cast()?;
            item_iface.SetTitle(&data.title)?;
            self.entries(&item_iface.Items()?, data)?;
            menus.Append(&item)?;
        }
        Ok(menu_bar)
    }

    fn flyout(&mut self, entries: &[MenuEntry]) -> R<w::MenuFlyout> {
        let flyout = w::MenuFlyout::new()?;
        let menu = MenuData { title: String::new(), entries: entries.to_vec() };
        self.entries(&flyout.cast::<w::IMenuFlyout>()?.Items()?, &menu)?;
        Ok(flyout)
    }

    fn entries(&mut self, entries: &windows_collections::IVector<w::MenuFlyoutItemBase>, menu: &MenuData) -> R<()> {
        for (entry, group) in menu.entries.iter().zip(menu.radio_groups()) {
            let element = match entry {
                MenuEntry::Item(data) => self.item(data, group)?,
                MenuEntry::Submenu(submenu) => {
                    let sub = w::MenuFlyoutSubItem::new()?;
                    let iface: w::IMenuFlyoutSubItem = sub.cast()?;
                    iface.SetText(&submenu.title)?;
                    self.entries(&iface.Items()?, submenu)?;
                    sub.cast()?
                }
                MenuEntry::Separator => w::MenuFlyoutSeparator::new()?.cast()?,
            };
            entries.Append(&element)?;
        }
        Ok(())
    }

    /// A `MenuFlyoutItem`, or for a check mark a `ToggleMenuFlyoutItem`, or
    /// a `RadioMenuFlyoutItem` grouped with the radio items next to it.
    fn item(&mut self, data: &MenuItemData, group: Option<u32>) -> R<w::MenuFlyoutItemBase> {
        let element: w::MenuFlyoutItemBase = match data.check {
            MenuCheck::None => w::MenuFlyoutItem::new()?.cast()?,
            MenuCheck::Check(_) => w::ToggleMenuFlyoutItem::new()?.cast()?,
            MenuCheck::Radio(_) => {
                let radio = w::RadioMenuFlyoutItem::new()?;
                let name = format!("mitsuami-{}-{}", self.scope, group.unwrap_or(data.id));
                radio.cast::<w::IRadioMenuFlyoutItem>()?.SetGroupName(&name)?;
                radio.cast()?
            }
        };
        show_check(&element, data.check);
        let iface: w::IMenuFlyoutItem = element.cast()?;
        iface.SetText(&data.title)?;
        element.cast::<w::IControl>()?.SetIsEnabled(data.enabled)?;
        if let Some(shortcut) = data.shortcut
            && let Some(key) = virtual_key(shortcut.key)
        {
            let accelerator = w::KeyboardAccelerator::new()?;
            let accel: w::IKeyboardAccelerator = accelerator.cast()?;
            accel.SetKey(key)?;
            let mut modifiers = w::VirtualKeyModifiers::None;
            if shortcut.primary {
                modifiers |= w::VirtualKeyModifiers::Control;
            }
            if shortcut.shift {
                modifiers |= w::VirtualKeyModifiers::Shift;
            }
            if shortcut.alt {
                modifiers |= w::VirtualKeyModifiers::Menu;
            }
            accel.SetModifiers(modifiers)?;
            element.cast::<w::IUIElement>()?.KeyboardAccelerators()?.Append(&accelerator)?;
        }
        // XAML flips a toggle or radio item as it's clicked. What it shows
        // is the app's state, so put it back: the app's choice comes back
        // from the core.
        let (id, activate, items) = (data.id, self.activate.clone(), Rc::downgrade(self.items));
        self.revokers.push(iface.Click(move |_, _| {
            if let Some(items) = items.upgrade() {
                show_checks(&items);
            }
            activate(id);
        })?);
        self.items.borrow_mut().insert(data.id, (element.clone(), data.check));
        Ok(element)
    }
}

/// What a context menu shows, read back from its items. XAML can't keep
/// ids or roles: those come from what was sent.
pub(super) fn read_menu(
    items: &windows_collections::IVector<w::MenuFlyoutItemBase>,
    menu: &ContextMenu,
) -> Vec<MenuEntry> {
    let ids: HashMap<usize, u32> = menu.items.borrow().iter().map(|(id, (item, _))| (key(item), *id)).collect();
    let read_item = |item: &w::MenuFlyoutItemBase| -> Option<MenuItemData> {
        let id = *ids.get(&key(item))?;
        let sent = mitsuami_core::services::menu_item_by_id(&menu.sent, id);
        let check = if let Ok(radio) = item.cast::<w::IRadioMenuFlyoutItem>() {
            MenuCheck::Radio(radio.IsChecked().ok()?)
        } else if let Ok(toggle) = item.cast::<w::IToggleMenuFlyoutItem>() {
            MenuCheck::Check(toggle.IsChecked().ok()?)
        } else {
            MenuCheck::None
        };
        let accelerators = item.cast::<w::IUIElement>().ok()?.KeyboardAccelerators().ok()?;
        let shortcut = (accelerators.Size().ok()? > 0)
            .then(|| accelerators.GetAt(0).ok()?.cast::<w::IKeyboardAccelerator>().ok())
            .flatten()
            .and_then(|accel| {
                let key = key_of(accel.Key().ok()?)?;
                let modifiers = accel.Modifiers().ok()?;
                Some(Shortcut {
                    key,
                    primary: modifiers.contains(w::VirtualKeyModifiers::Control),
                    shift: modifiers.contains(w::VirtualKeyModifiers::Shift),
                    alt: modifiers.contains(w::VirtualKeyModifiers::Menu),
                })
            });
        Some(MenuItemData {
            id,
            title: item.cast::<w::IMenuFlyoutItem>().ok()?.Text().ok()?.to_string(),
            shortcut,
            enabled: item.cast::<w::IControl>().ok()?.IsEnabled().ok()?,
            check,
            role: sent.map(|s| s.role).unwrap_or_default(),
        })
    };
    fn read(
        items: &windows_collections::IVector<w::MenuFlyoutItemBase>,
        read_item: &dyn Fn(&w::MenuFlyoutItemBase) -> Option<MenuItemData>,
    ) -> Vec<MenuEntry> {
        (0..items.Size().unwrap_or(0))
            .filter_map(|i| items.GetAt(i).ok())
            .filter_map(|item| {
                if item.cast::<w::MenuFlyoutSeparator>().is_ok() {
                    Some(MenuEntry::Separator)
                } else if let Ok(sub) = item.cast::<w::IMenuFlyoutSubItem>() {
                    Some(MenuEntry::Submenu(MenuData {
                        title: sub.Text().ok()?.to_string(),
                        entries: read(&sub.Items().ok()?, read_item),
                    }))
                } else {
                    read_item(&item).map(MenuEntry::Item)
                }
            })
            .collect()
    }
    read(items, &read_item)
}

/// Puts back the app's checked states on items XAML flipped.
fn show_checks(items: &MenuItems) {
    for (item, check) in items.borrow().values() {
        show_check(item, *check);
    }
}

/// A US keyboard's punctuation keys, by their unshifted character: the
/// `VK_OEM_*` keys, which XAML's `VirtualKey` has no names for.
const OEM: [(char, i32); 11] = [
    (';', 0xBA),
    ('=', 0xBB),
    (',', 0xBC),
    ('-', 0xBD),
    ('.', 0xBE),
    ('/', 0xBF),
    ('`', 0xC0),
    ('[', 0xDB),
    ('\\', 0xDC),
    (']', 0xDD),
    ('\'', 0xDE),
];

/// The virtual key of a key: a letter's or digit's is its upper case
/// character.
pub(crate) fn virtual_key(key: Key) -> Option<w::VirtualKey> {
    let code = match key {
        Key::Char(c) => {
            let c = c.to_ascii_uppercase();
            if c.is_ascii_uppercase() || c.is_ascii_digit() || c == ' ' {
                c as i32
            } else {
                OEM.iter().find(|(o, _)| *o == c)?.1
            }
        }
        Key::Enter => 0x0D,
        Key::Escape => 0x1B,
        Key::Tab => 0x09,
        Key::Backspace => 0x08,
        Key::Delete => 0x2E,
        Key::Up => 0x26,
        Key::Down => 0x28,
        Key::Left => 0x25,
        Key::Right => 0x27,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::F(n @ 1..=24) => 0x6F + i32::from(n),
        Key::F(_) => return None,
    };
    Some(w::VirtualKey(code))
}

/// The key a virtual key stands for; a letter's is lower case.
pub(crate) fn key_of(key: w::VirtualKey) -> Option<Key> {
    Some(match key.0 {
        0x0D => Key::Enter,
        0x1B => Key::Escape,
        0x09 => Key::Tab,
        0x08 => Key::Backspace,
        0x2E => Key::Delete,
        0x26 => Key::Up,
        0x28 => Key::Down,
        0x25 => Key::Left,
        0x27 => Key::Right,
        0x24 => Key::Home,
        0x23 => Key::End,
        0x21 => Key::PageUp,
        0x22 => Key::PageDown,
        code @ 0x70..=0x87 => Key::F((code - 0x6F) as u8),
        code @ (0x20 | 0x30..=0x39 | 0x41..=0x5A) => Key::Char(char::from_u32(code as u32)?.to_ascii_lowercase()),
        code => Key::Char(OEM.iter().find(|(_, o)| *o == code)?.0),
    })
}

impl WinUiBackend {
    /// What Narrator does once it has shown a control's context menu: the
    /// item's UIA Invoke (Toggle for a check item), which clicks it as the
    /// pointer does, so the item's `Click` handler reports it. A disabled
    /// control gets no input, so shows no menu.
    /// Chooses an item of the node's context menu, or of a menu button's
    /// menu, without opening it.
    pub(super) fn choose_menu_item(&self, id: NodeId, item: u32, button: bool) -> Result<(), ActionError> {
        let (element, items, activate) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.control().cast::<w::IControl>().and_then(|c| c.IsEnabled()).is_ok_and(|on| !on) {
                return Err(ActionError::Disabled);
            }
            let menu = if button { &node.button_menu } else { &node.context_menu };
            let menu = menu.as_ref().filter(|menu| menu.flyout.is_some()).ok_or(ActionError::Unsupported)?;
            let element = menu.items.borrow().get(&item).map(|(element, _)| element.clone());
            (element.ok_or(ActionError::Unsupported)?, menu.items.clone(), menu.activate.clone())
        };
        if !element.cast::<w::IControl>().and_then(|c| c.IsEnabled()).unwrap_or(false) {
            return Err(ActionError::Disabled);
        }
        // No state borrow: the item's `Click` handler emits the choice.
        let peer = w::FrameworkElementAutomationPeer::CreatePeerForElement(
            &element.cast::<w::UIElement>().map_err(|_| ActionError::Unsupported)?,
        )
        .map_err(|_| ActionError::Unsupported)?;
        let pattern = |which| peer.GetPattern(which).ok();
        if let Some(invoke) = pattern(w::PatternInterface::Invoke).and_then(|p| p.cast::<w::IInvokeProvider>().ok()) {
            invoke.Invoke().map_err(|_| ActionError::Unsupported)
        } else if let Some(toggle) =
            pattern(w::PatternInterface::Toggle).and_then(|p| p.cast::<w::IToggleProvider>().ok())
        {
            toggle.Toggle().map_err(|_| ActionError::Unsupported)
        } else {
            // An item whose peer has neither (a radio item may not): what
            // its `Click` handler does.
            show_checks(&items);
            activate(item);
            Ok(())
        }
    }
}
