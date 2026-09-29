//! Menus as data, as platforms get them: menu bars, items, shortcuts and
//! roles.

use crate::backend::Key;

/// A keyboard shortcut: a key, and the modifiers held with it. `primary`
/// is ⌘ on macOS and Ctrl elsewhere; `alt` is ⌥ on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// A letter is lower case: Shift is `shift`.
    pub key: Key,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Shortcut {
    /// The key on its own: `Shortcut::new(Key::F(2))`, or with modifiers
    /// added, `Shortcut::new(Key::Up).alt()`.
    pub fn new(key: impl Into<Key>) -> Shortcut {
        let key = match key.into() {
            Key::Char(c) => Key::Char(c.to_ascii_lowercase()),
            key => key,
        };
        Shortcut { key, primary: false, shift: false, alt: false }
    }

    /// ⌘+key on macOS, Ctrl+key elsewhere: `Shortcut::primary('s')`,
    /// `Shortcut::primary(Key::Backspace)`.
    pub fn primary(key: impl Into<Key>) -> Shortcut {
        Shortcut { primary: true, ..Shortcut::new(key) }
    }

    pub fn shift(mut self) -> Shortcut {
        self.shift = true;
        self
    }

    pub fn alt(mut self) -> Shortcut {
        self.alt = true;
        self
    }
}

impl From<Key> for Shortcut {
    fn from(key: Key) -> Shortcut {
        Shortcut::new(key)
    }
}

impl From<char> for Shortcut {
    fn from(c: char) -> Shortcut {
        Shortcut::new(c)
    }
}

/// A menu bar as data, for [`Services::set_menu`]: the app's menus, or a
/// window's. Hidden items and menus are left out.
///
/// [`Services::set_menu`]: super::Services::set_menu
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuBarData {
    pub menus: Vec<MenuData>,
}

/// A menu of the bar, or a submenu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuData {
    pub title: String,
    pub entries: Vec<MenuEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    Item(MenuItemData),
    Submenu(MenuData),
    Separator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItemData {
    /// Unique among every menu bar of the app, and the same while the
    /// bar's structure is.
    pub id: u32,
    pub title: String,
    pub shortcut: Option<Shortcut>,
    pub enabled: bool,
    pub check: MenuCheck,
    pub role: MenuRole,
}

/// Whether an item shows a check mark, and which kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuCheck {
    #[default]
    None,
    /// Checked or not, on its own.
    Check(bool),
    /// One of a group: the radio items next to each other in a menu.
    Radio(bool),
}

/// An item every app has, which some platforms put in a place of their
/// own (the app menu on macOS) with their own title and shortcut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuRole {
    #[default]
    None,
    /// "About <app>".
    About,
    /// The app's settings (preferences).
    Settings,
    /// Quitting: it replaces the platform's own Quit item, where there is
    /// one.
    Quit,
}

impl MenuBarData {
    /// The menus of a window that has menus of its own: the app's, then
    /// the window's. A window menu titled like an app menu joins it, after
    /// a separator.
    pub fn merged(&self, window: &MenuBarData) -> MenuBarData {
        let mut merged = self.clone();
        for menu in &window.menus {
            match merged.menus.iter_mut().find(|m| m.title == menu.title) {
                Some(app) => {
                    if !app.entries.is_empty() && !menu.entries.is_empty() {
                        app.entries.push(MenuEntry::Separator);
                    }
                    app.entries.extend(menu.entries.iter().cloned());
                }
                None => merged.menus.push(menu.clone()),
            }
        }
        merged
    }

    /// What a window shows on platforms that put menus in each window
    /// (GTK, WinUI, Kirigami): the app's with its own ([`merged`](Self::merged)).
    /// A modal window is a dialog, and dialogs there have no app menus, so
    /// it shows only its own.
    pub fn for_window(&self, own: Option<&MenuBarData>, modal: bool) -> MenuBarData {
        match (own, modal) {
            (Some(own), false) => self.merged(own),
            (None, false) => self.clone(),
            (Some(own), true) => own.clone(),
            (None, true) => MenuBarData::default(),
        }
    }

    /// Takes out the first item with this role, for a platform that puts
    /// it somewhere of its own. Separators it leaves at an edge or doubled
    /// go too, and so does a menu it leaves empty.
    pub fn take_role(&mut self, role: MenuRole) -> Option<MenuItemData> {
        let taken = self.menus.iter_mut().find_map(|menu| menu.take_role(role))?;
        self.menus.retain(|menu| !menu.entries.is_empty());
        Some(taken)
    }

    /// Every item, submenus' included, in order.
    pub fn items(&self) -> Vec<&MenuItemData> {
        let mut items = Vec::new();
        for menu in &self.menus {
            menu.collect_items(&mut items);
        }
        items
    }

    /// The same menus, submenus, items, titles, shortcuts and roles: only
    /// enabled and checked states differ, which platforms can update
    /// without rebuilding (and closing) an open menu.
    pub fn same_structure(&self, other: &MenuBarData) -> bool {
        fn strip(menu: &MenuData) -> MenuData {
            let entries = menu
                .entries
                .iter()
                .map(|entry| match entry {
                    MenuEntry::Item(item) => MenuEntry::Item(MenuItemData {
                        enabled: true,
                        check: match item.check {
                            MenuCheck::None => MenuCheck::None,
                            MenuCheck::Check(_) => MenuCheck::Check(false),
                            MenuCheck::Radio(_) => MenuCheck::Radio(false),
                        },
                        ..item.clone()
                    }),
                    MenuEntry::Submenu(menu) => MenuEntry::Submenu(strip(menu)),
                    MenuEntry::Separator => MenuEntry::Separator,
                })
                .collect();
            MenuData { title: menu.title.clone(), entries }
        }
        self.menus.len() == other.menus.len() && self.menus.iter().zip(&other.menus).all(|(a, b)| strip(a) == strip(b))
    }
}

/// The item at `path` among these entries: the titles of its submenus,
/// then its own, e.g. `["Sort By", "Name"]`.
pub fn find_menu_item<'a>(entries: &'a [MenuEntry], path: &[&str]) -> Option<&'a MenuItemData> {
    let (title, rest) = path.split_first()?;
    entries.iter().find_map(|entry| match entry {
        MenuEntry::Item(item) if rest.is_empty() && item.title == *title => Some(item),
        MenuEntry::Submenu(submenu) if !rest.is_empty() && submenu.title == *title => {
            find_menu_item(&submenu.entries, rest)
        }
        _ => None,
    })
}

/// The item with this id among these entries, submenus' included.
pub fn menu_item_by_id(entries: &[MenuEntry], id: u32) -> Option<&MenuItemData> {
    entries.iter().find_map(|entry| match entry {
        MenuEntry::Item(item) if item.id == id => Some(item),
        MenuEntry::Submenu(submenu) => menu_item_by_id(&submenu.entries, id),
        _ => None,
    })
}

impl MenuData {
    /// For each entry, the radio group it belongs to, named by the id of
    /// the group's first item: radio items next to each other form one.
    pub fn radio_groups(&self) -> Vec<Option<u32>> {
        let mut group = None;
        self.entries
            .iter()
            .map(|entry| {
                group = match entry {
                    MenuEntry::Item(item @ MenuItemData { check: MenuCheck::Radio(_), .. }) => group.or(Some(item.id)),
                    _ => None,
                };
                group
            })
            .collect()
    }

    fn take_role(&mut self, role: MenuRole) -> Option<MenuItemData> {
        let mut taken = None;
        for (i, entry) in self.entries.iter_mut().enumerate() {
            match entry {
                MenuEntry::Item(item) if item.role == role => {
                    taken = Some((i, item.clone()));
                    break;
                }
                MenuEntry::Submenu(submenu) => {
                    if let Some(item) = submenu.take_role(role) {
                        if submenu.entries.is_empty() {
                            self.entries.remove(i);
                        }
                        self.tidy_separators();
                        return Some(item);
                    }
                }
                _ => {}
            }
        }
        let (i, item) = taken?;
        self.entries.remove(i);
        self.tidy_separators();
        Some(item)
    }

    fn tidy_separators(&mut self) {
        let mut entries: Vec<MenuEntry> = Vec::with_capacity(self.entries.len());
        for entry in self.entries.drain(..) {
            let separator = matches!(entry, MenuEntry::Separator);
            if separator && entries.last().is_none_or(|e| matches!(e, MenuEntry::Separator)) {
                continue;
            }
            entries.push(entry);
        }
        if matches!(entries.last(), Some(MenuEntry::Separator)) {
            entries.pop();
        }
        self.entries = entries;
    }

    fn collect_items<'a>(&'a self, out: &mut Vec<&'a MenuItemData>) {
        for entry in &self.entries {
            match entry {
                MenuEntry::Item(item) => out.push(item),
                MenuEntry::Submenu(menu) => menu.collect_items(out),
                MenuEntry::Separator => {}
            }
        }
    }
}
