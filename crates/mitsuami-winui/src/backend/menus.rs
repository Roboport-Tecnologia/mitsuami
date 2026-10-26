//! Menus: the menu bar, context menus and menu buttons' menus, built and
//! read back.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::a11y::ActionError;
use mitsuami_core::services::{MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData, MenuOnOpen, Shortcut};
use mitsuami_core::{Key, NodeId};
use windows_core::{EventRevoker, Interface};

use super::styles::style;
use super::windows::{apply_min_size, chrome_height, resize_client, title_content, update_title_bar_height};
use super::{ContextMenu, MenuBarPlace, MenuItems, Menus, R, WinUiBackend, WindowParts, boxed, key, ok};
use crate::bindings as w;

impl ContextMenu {
    pub(crate) fn new(activate: Rc<dyn Fn(u32)>, own: Option<w::FlyoutBase>) -> ContextMenu {
        ContextMenu {
            sent: Rc::default(),
            flyout: None,
            items: MenuItems::default(),
            revokers: Rc::default(),
            activate,
            own,
            on_open: None,
        }
    }

    /// The flyout showing it, none while it's empty.
    pub(crate) fn flyout(&self) -> Option<&w::MenuFlyout> {
        self.flyout.as_ref()
    }

    pub(super) fn has_items(menu: &Option<ContextMenu>) -> bool {
        menu.as_ref().is_some_and(|menu| menu.flyout.is_some())
    }

    /// Shows `entries`: in place when only enabled and checked states
    /// changed, so an open menu stays open, as the menu bar does; else a
    /// new flyout (none for no entries). Whether the flyout was replaced.
    /// One built as it opens keeps its flyout, refilled.
    pub(crate) fn update(&mut self, entries: &[MenuEntry], scope: String) -> R<bool> {
        if let (Some(_), Some(flyout)) = (&self.on_open, &self.flyout) {
            fill(flyout, &self.sent, &self.revokers, &self.items, &self.activate, entries, scope)?;
            return Ok(false);
        }
        let same = as_menu_bar(entries).same_structure(&as_menu_bar(&self.sent.borrow()));
        let replaced = if self.flyout.is_some() && same {
            update_items(&self.items, &as_menu_bar(entries));
            false
        } else if self.flyout.is_some() || !entries.is_empty() {
            self.revokers.borrow_mut().clear();
            self.items.borrow_mut().clear();
            self.flyout = None;
            if !entries.is_empty() {
                let mut revokers = self.revokers.borrow_mut();
                let mut built = MenuBuild {
                    activate: &self.activate,
                    revokers: &mut revokers,
                    items: &self.items,
                    closed: None,
                    scope,
                };
                self.flyout = Some(built.flyout(entries)?);
            }
            true
        } else {
            false
        };
        *self.sent.borrow_mut() = entries.to_vec();
        Ok(replaced)
    }

    /// Builds the menu as it opens, asking `on_open` (`ContextMenuOnOpen`,
    /// or a menu button's `MenuOnOpen`),
    /// or with `None` shows what was sent, as `update` does. Whether the
    /// flyout was replaced.
    pub(crate) fn set_on_open(&mut self, on_open: Option<&MenuOnOpen>, scope: String) -> R<bool> {
        match (on_open, &self.on_open) {
            (Some(on_open), Some((build, _))) => {
                *build.borrow_mut() = on_open.clone();
                Ok(false)
            }
            (Some(on_open), None) => {
                // A flyout of its own, set even while it's empty, so XAML
                // opens it and raises `Opening`: the moment WinUI apps fill
                // a menu that depends on what's under the pointer.
                let flyout = w::MenuFlyout::new()?;
                let sent = std::mem::take(&mut *self.sent.borrow_mut());
                self.revokers.borrow_mut().clear();
                self.items.borrow_mut().clear();
                fill(&flyout, &self.sent, &self.revokers, &self.items, &self.activate, &sent, scope.clone())?;
                let build = Rc::new(RefCell::new(on_open.clone()));
                let (asked, shared) = (build.clone(), self.sent.clone());
                let (revokers, items, activate) = (self.revokers.clone(), self.items.clone(), self.activate.clone());
                let opening = flyout.cast::<w::IFlyoutBase>()?.Opening(move |sender, _| {
                    let Some(flyout) = sender.as_ref().and_then(|s| s.cast::<w::MenuFlyout>().ok()) else { return };
                    // Outside any core call, unless a nested pump runs
                    // inside one: then it shows what it built last.
                    if !super::super::runtime::nested() {
                        let on_open = asked.borrow().clone();
                        let entries = on_open.build();
                        _ = fill(&flyout, &shared, &revokers, &items, &activate, &entries, scope.clone());
                    }
                    // Nothing to show: XAML has no way to cancel `Opening`,
                    // and an empty flyout would show as an empty box, so
                    // it's hidden at once, as a control without a menu
                    // shows nothing.
                    if shared.borrow().iter().all(|e| matches!(e, MenuEntry::Separator)) {
                        _ = flyout.cast::<w::IFlyoutBase>().and_then(|f| f.Hide());
                    }
                })?;
                self.flyout = Some(flyout);
                self.on_open = Some((build, opening));
                Ok(true)
            }
            (None, Some(_)) => {
                self.on_open = None;
                let sent = std::mem::take(&mut *self.sent.borrow_mut());
                self.revokers.borrow_mut().clear();
                self.items.borrow_mut().clear();
                self.flyout = None;
                self.update(&sent, scope)?;
                Ok(true)
            }
            (None, None) => Ok(false),
        }
    }

    /// The app's function, for `native_state`.
    pub(crate) fn on_open(&self) -> Option<MenuOnOpen> {
        self.on_open.as_ref().map(|(build, _)| build.borrow().clone())
    }
}

/// Fills a flyout built as it opens with `entries`, in place, unless it
/// holds them already, so an open menu isn't rebuilt under the pointer.
fn fill(
    flyout: &w::MenuFlyout,
    sent: &RefCell<Vec<MenuEntry>>,
    revokers: &RefCell<Vec<EventRevoker>>,
    items: &MenuItems,
    activate: &Rc<dyn Fn(u32)>,
    entries: &[MenuEntry],
    scope: String,
) -> R<()> {
    if *sent.borrow() == entries {
        return Ok(());
    }
    let shown = flyout.cast::<w::IMenuFlyout>()?.Items()?;
    shown.Clear()?;
    revokers.borrow_mut().clear();
    items.borrow_mut().clear();
    let mut revokers = revokers.borrow_mut();
    let mut built = MenuBuild { activate, revokers: &mut revokers, items, closed: None, scope };
    built.entries(&shown, &MenuData { title: String::new(), entries: entries.to_vec() })?;
    *sent.borrow_mut() = entries.to_vec();
    Ok(())
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
    let built = parts.menu_bar.is_some() || parts.menu_button.is_some();
    if built && shown.same_structure(&parts.menu_shown) {
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
    if parts.menu_bar_place == MenuBarPlace::ButtonInTitleBar {
        install_menu_button(parts, menu, activate);
        return;
    }
    let before = chrome_height(parts);
    // Back from the full-screen popup first, where it's replaced, and the
    // new bar back in it after.
    let revealed = parts.revealed;
    _ = super::reveal::hide_reveal(parts);
    let in_title_bar = parts.menu_bar_place.in_title_bar();
    let parent: R<w::Panel> =
        if in_title_bar { title_content(parts).and_then(|g| g.cast()) } else { parts.bars.cast() };
    let children = ok(parent.and_then(|p| p.cast::<w::IPanel>()?.Children()), "menu bar's parent's children");
    parts.menu_revokers.clear();
    if let Some(old) = parts.menu_bar.take() {
        let old: w::UIElement = ok(old.cast(), "menu bar element");
        let mut index = 0;
        if children.IndexOf(&old, &mut index).unwrap_or(false) {
            _ = children.RemoveAt(index);
        }
    }
    parts.menu_items.borrow_mut().clear();
    if !menu.menus.is_empty() {
        let mut built = MenuBuild {
            activate,
            revokers: &mut parts.menu_revokers,
            items: &parts.menu_items,
            closed: Some(parts.menu_stale.clone()),
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
    if revealed {
        _ = super::reveal::show_reveal(parts);
    }
    // The content keeps its size only if the bar's height changed: the same
    // menus built again (`rebuild_closed_menu`, after every menu closes)
    // put the window back to the last size the app asked for, which the
    // user had since resized it from (2ksbox's player shrank to its picture
    // on every menu click after Fit Window to Picture).
    _ = parts.root.cast::<w::IUIElement>().and_then(|r| r.UpdateLayout());
    if chrome_height(parts) != before {
        apply_min_size(parts);
        if let Some(size) = parts.requested {
            resize_client(parts, size);
        }
    }
}

/// The menus behind a button at the title bar's start, its `LeftHeader`,
/// before document tabs there: the title bar's own pane toggle button,
/// WinUI's hamburger, opens a menu of the app's menus as submenus, as
/// Visual Studio's compact menu does. The title bar and the content keep
/// their size. It's built again whenever the menus change shape, as the
/// bar is; XAML's flyout doesn't widen as the bar's menus do, so not after
/// each close.
fn install_menu_button(parts: &mut WindowParts, menu: &MenuBarData, activate: &Rc<dyn Fn(u32)>) {
    parts.menu_revokers.clear();
    parts.menu_items.borrow_mut().clear();
    let title_bar = ok(parts.title_bar.cast::<w::ITitleBar>(), "title bar");
    parts.menu_button = None;
    if menu.menus.is_empty() {
        _ = title_bar.SetLeftHeader(None::<&w::UIElement>);
        return;
    }
    let mut built = MenuBuild {
        activate,
        revokers: &mut parts.menu_revokers,
        items: &parts.menu_items,
        closed: None,
        scope: format!("window-{}", parts.node),
    };
    let button = ok(built.menu_button(menu), "building the menu button");
    _ = title_bar.SetLeftHeader(&ok(button.cast::<w::UIElement>(), "menu button element"));
    parts.menu_button = Some(button);
}

/// Builds the window's bar again, the same menus, once a menu of it has
/// closed and none is open. XAML's menu widens by about 33 epx from its
/// second opening on, and a checked toggle item's shortcut keeps the
/// column of the first, so it no longer lines up with the others (2ksbox's
/// Send Shortcuts to Guest, checked, with Ctrl+Alt+K); a new bar's menus
/// open as the first time.
pub(super) fn rebuild_closed_menu(parts: &mut WindowParts, menus: &Menus) {
    if !parts.menu_stale.get() || super::reveal::menu_open(parts) {
        return;
    }
    parts.menu_stale.set(false);
    let shown = menus.shown(parts.node, parts.modal.is_some());
    let activate = menus.activate.clone().unwrap_or_else(|| Rc::new(|_| {}));
    install_menu(parts, &shown, &activate);
    parts.menu_shown = shown;
}

/// Builds a window's `MenuBar`, or a context menu's `MenuFlyout`: the
/// same items. Items with a role stay where the app put them: Windows has
/// no standard place of its own for About, Settings or Exit, and no
/// standard titles or shortcuts for them.
struct MenuBuild<'a> {
    activate: &'a Rc<dyn Fn(u32)>,
    revokers: &'a mut Vec<EventRevoker>,
    items: &'a MenuItems,
    /// A menu bar's: set when one of its menus closes (`menu_stale`).
    closed: Option<Rc<Cell<bool>>>,
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
            let items = item_iface.Items()?;
            self.entries(&items, data)?;
            // Its items unload as its menu closes.
            if let (Some(closed), Ok(first)) = (&self.closed, items.GetAt(0)) {
                let closed = closed.clone();
                self.revokers.push(first.cast::<w::IFrameworkElement>()?.Unloaded(move |_, _| {
                    closed.set(true);
                    crate::app::schedule_tick();
                })?);
            }
            menus.Append(&item)?;
        }
        Ok(menu_bar)
    }

    /// A button opening the bar's menus as submenus, named by mitsuami's
    /// "Main Menu", as GNOME's primary menu button is.
    fn menu_button(&mut self, menu: &MenuBarData) -> R<w::Button> {
        let button = w::Button::new()?;
        button.cast::<w::IFrameworkElement>()?.SetStyle(&style("TitleBarPaneToggleButtonStyle"))?;
        let name = mitsuami_core::l10n::tr("mitsuami-main-menu", &[]);
        w::AutomationProperties::SetName(&button, &name)?;
        w::ToolTipService::SetToolTip(&button, &boxed(&name))?;
        let flyout = w::MenuFlyout::new()?;
        let items = flyout.cast::<w::IMenuFlyout>()?.Items()?;
        for data in &menu.menus {
            let sub = w::MenuFlyoutSubItem::new()?;
            let iface: w::IMenuFlyoutSubItem = sub.cast()?;
            iface.SetText(&data.title)?;
            self.entries(&iface.Items()?, data)?;
            items.Append(&sub.cast::<w::MenuFlyoutItemBase>()?)?;
        }
        flyout.cast::<w::IFlyoutBase>()?.SetPlacement(w::FlyoutPlacementMode::BottomEdgeAlignedLeft)?;
        button.cast::<w::IButton>()?.SetFlyout(&flyout)?;
        // The title bar centres what's in its `LeftHeader`, so the button
        // was only as tall as its icon; its own pane toggle button, in the
        // column before, is as tall as the title bar. Its presenter is
        // stretched, once the button is in it.
        self.revokers.push(button.cast::<w::IFrameworkElement>()?.Loaded(|sender, _| {
            let presenter = sender
                .as_ref()
                .and_then(|s| w::VisualTreeHelper::GetParent(&s.cast::<w::DependencyObject>().ok()?).ok());
            if let Some(presenter) = presenter.and_then(|p| p.cast::<w::IFrameworkElement>().ok()) {
                _ = presenter.SetVerticalAlignment(w::VerticalAlignment::Stretch);
            }
        })?);
        Ok(button)
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
            // A surface that heard of the key first holds it back for this,
            // and drops it: the menu item took it.
            self.revokers.push(accel.Invoked(|_, _| crate::surface::key_taken())?);
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
    let sent_entries = menu.sent.borrow().clone();
    let read_item = |item: &w::MenuFlyoutItemBase| -> Option<MenuItemData> {
        let id = *ids.get(&key(item))?;
        let sent = mitsuami_core::services::menu_item_by_id(&sent_entries, id);
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
            // A sidebar's items each have a menu of their own.
            if let (false, super::Widget::Sidebar(sidebar)) = (button, &node.widget) {
                let found = sidebar.with_menu(item, |menu| {
                    let element = menu.items.borrow().get(&item).map(|(element, _)| element.clone());
                    element.map(|e| (e, menu.items.clone(), menu.activate.clone()))
                });
                found.flatten().ok_or(ActionError::Unsupported)?
            } else {
                let menu = if button { &node.button_menu } else { &node.context_menu };
                let menu = menu.as_ref().filter(|menu| menu.flyout.is_some()).ok_or(ActionError::Unsupported)?;
                let element = menu.items.borrow().get(&item).map(|(element, _)| element.clone());
                (element.ok_or(ActionError::Unsupported)?, menu.items.clone(), menu.activate.clone())
            }
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
