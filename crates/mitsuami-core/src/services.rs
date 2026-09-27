//! Platform services: clipboard, dialogs and the menu bar.
//!
//! [`Services`] is the contract each platform implements, separately from
//! the widget [`Backend`](crate::Backend). Tests install a scripted fake in
//! its place, so they never open real dialogs or touch the real clipboard.
//!
//! App code uses the async functions here (`alert(...).await`, …) or the
//! same methods on [`Ui`].

use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::task::{Poll, Waker};

use mitsuami_reactive::{IntoValue, Signal, Value, inject};

use crate::ui::Ui;
use crate::view::View;
use crate::widget::{CurrentWindow, NodeId, WidgetKind};

/// Delivers the user's answer. Called once, possibly long after the request.
pub type Reply<T> = Box<dyn FnOnce(T)>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertStyle {
    #[default]
    Info,
    Warning,
    /// Destructive or irreversible consequences.
    Critical,
}

/// Why a service request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The platform doesn't offer this service (or not right now).
    Unavailable,
    Failed(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::Unavailable => f.write_str("the service is not available"),
            ServiceError::Failed(why) => write!(f, "the service failed: {why}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// A message with buttons. The reply is the index of the chosen button.
#[derive(Clone, Debug, PartialEq)]
pub struct Alert {
    pub title: String,
    pub message: Option<String>,
    /// In order of importance: the first is the default (Return) button.
    pub buttons: Vec<String>,
    pub style: AlertStyle,
}

impl Alert {
    pub fn new(title: impl Into<String>) -> Alert {
        Alert { title: title.into(), message: None, buttons: Vec::new(), style: AlertStyle::Info }
    }

    pub fn message(mut self, message: impl Into<String>) -> Alert {
        self.message = Some(message.into());
        self
    }

    pub fn button(mut self, title: impl Into<String>) -> Alert {
        self.buttons.push(title.into());
        self
    }

    pub fn style(mut self, style: AlertStyle) -> Alert {
        self.style = style;
        self
    }

    /// The buttons to show: "OK" when none were given.
    pub fn effective_buttons(&self) -> Vec<String> {
        if self.buttons.is_empty() { vec!["OK".to_string()] } else { self.buttons.clone() }
    }
}

/// Files to offer in a file dialog, e.g. `FileFilter::new("Images", ["png", "jpg"])`.
#[derive(Clone, Debug, PartialEq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileFilter {
    pub fn new<S: Into<String>>(name: impl Into<String>, extensions: impl IntoIterator<Item = S>) -> FileFilter {
        FileFilter { name: name.into(), extensions: extensions.into_iter().map(Into::into).collect() }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenFile {
    pub title: Option<String>,
    pub multiple: bool,
    /// Choose folders instead of files.
    pub directories: bool,
    pub filters: Vec<FileFilter>,
}

impl OpenFile {
    pub fn new() -> OpenFile {
        OpenFile::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> OpenFile {
        self.title = Some(title.into());
        self
    }

    pub fn multiple(mut self) -> OpenFile {
        self.multiple = true;
        self
    }

    pub fn directories(mut self) -> OpenFile {
        self.directories = true;
        self
    }

    pub fn filter(mut self, filter: FileFilter) -> OpenFile {
        self.filters.push(filter);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaveFile {
    pub title: Option<String>,
    pub default_name: Option<String>,
    pub filters: Vec<FileFilter>,
}

impl SaveFile {
    pub fn new() -> SaveFile {
        SaveFile::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> SaveFile {
        self.title = Some(title.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> SaveFile {
        self.default_name = Some(name.into());
        self
    }

    pub fn filter(mut self, filter: FileFilter) -> SaveFile {
        self.filters.push(filter);
        self
    }
}

/// A keyboard shortcut. `primary` is ⌘ on macOS and Ctrl elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: char,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Shortcut {
    /// ⌘+key on macOS, Ctrl+key elsewhere.
    pub fn primary(key: char) -> Shortcut {
        Shortcut { key: key.to_ascii_lowercase(), primary: true, shift: false, alt: false }
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

/// A menu bar as data, for [`Services::set_menu`]: the app's menus, or a
/// window's. Hidden items and menus are left out.
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

/// What a platform provides beyond widgets.
pub trait Services {
    /// Reads the clipboard's text. Async because GTK and WinUI only read the
    /// clipboard asynchronously; platforms that can may reply right away.
    fn clipboard_text(&mut self, reply: Reply<Option<String>>);
    /// Writes the clipboard (it can fail: e.g. another process holds it).
    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>);

    /// Shows an alert, attached to `parent` if given (a sheet on macOS) or
    /// to the active window. Must not block: reply when the user answers.
    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>);
    /// Replies with the chosen paths, or `None` if cancelled.
    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>);
    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>);

    /// Installs a menu bar: the app's (`window` is `None`), or one
    /// window's own menus, which it shows with the app's
    /// ([`MenuBarData::merged`]). Called again whenever the bar changes;
    /// an empty bar removes a window's menus. A window's menus may arrive
    /// before the window is created, and after it's destroyed.
    ///
    /// Platforms keep their standard menus (e.g. the macOS app and Edit
    /// menus) and call `activate` with an item's id when it is chosen.
    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>);
}

// --------------------------------------------------------- menu builder

type Handler = Rc<dyn Fn()>;

enum Mark {
    None,
    Check(Value<bool>),
    Radio(Value<bool>),
}

/// A menu item: a title, what choosing it does, and optionally a
/// shortcut, a check mark and a role. Its title, enabled state, check mark
/// and visibility take a literal, a signal or a closure.
///
/// ```ignore
/// MenuItem::new("Save").shortcut(Shortcut::primary('s')).on_select(save)
/// MenuItem::new("Show Sidebar").bind(sidebar)
/// MenuItem::new("Large").radio((zoom, Zoom::Large))
/// ```
///
/// In `view!`, its text is its title:
/// `<MenuItem shortcut=Shortcut::primary('s') @select=save>"Save"</MenuItem>`.
pub struct MenuItem {
    title: Value<String>,
    shortcut: Option<Shortcut>,
    enabled: Value<bool>,
    visible: Value<bool>,
    check: Mark,
    role: MenuRole,
    /// What `bind` and `radio` do, before the handler.
    bound: Option<Handler>,
    handler: Option<Handler>,
}

impl MenuItem {
    pub fn new(title: impl IntoValue<String>) -> MenuItem {
        MenuItem {
            title: title.into_value(),
            shortcut: None,
            enabled: Value::Static(true),
            visible: Value::Static(true),
            check: Mark::None,
            role: MenuRole::None,
            bound: None,
            handler: None,
        }
    }

    /// Called when the item is chosen: clicked, or through its shortcut.
    pub fn on_select(mut self, handler: impl Fn() + 'static) -> MenuItem {
        self.handler = Some(Rc::new(handler));
        self
    }

    pub fn shortcut(mut self, shortcut: Shortcut) -> MenuItem {
        self.shortcut = Some(shortcut);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> MenuItem {
        self.enabled = enabled.into_value();
        self
    }

    /// In the menu while true.
    pub fn visible(mut self, visible: impl IntoValue<bool>) -> MenuItem {
        self.visible = visible.into_value();
        self
    }

    /// Shows a check mark while true. Choosing the item only calls its
    /// handler: [`bind`](Self::bind) toggles a signal instead.
    pub fn checked(mut self, checked: impl IntoValue<bool>) -> MenuItem {
        self.check = Mark::Check(checked.into_value());
        self
    }

    /// A check mark showing the signal; choosing the item toggles it.
    pub fn bind(mut self, signal: Signal<bool>) -> MenuItem {
        self.check = Mark::Check(signal.into_value());
        self.bound = Some(Rc::new(move || signal.update(|on| *on = !*on)));
        self
    }

    /// One of a group of choices: checked while `signal` holds `value`,
    /// and choosing it sets it. Radio items next to each other in a menu
    /// form a group. A pair, so `view!` can write `radio=(zoom, Zoom::Large)`.
    pub fn radio<T: PartialEq + Clone + 'static>(mut self, (signal, value): (Signal<T>, T)) -> MenuItem {
        let chosen = value.clone();
        self.check = Mark::Radio(Value::Dynamic(Rc::new(move || signal.with(|v| *v == chosen))));
        self.bound = Some(Rc::new(move || signal.set(value.clone())));
        self
    }

    /// An item every app has, which platforms may move to a place of their
    /// own and give their own title and shortcut: see [`MenuRole`].
    pub fn role(mut self, role: MenuRole) -> MenuItem {
        self.role = role;
        self
    }

    #[doc(hidden)]
    pub fn __tag() -> MenuItem {
        MenuItem::new(String::new())
    }

    /// `<MenuItem>"Save"</MenuItem>`: its text is its title.
    #[doc(hidden)]
    pub fn __children<S: IntoValue<String>>(mut self, title: impl FnOnce() -> S) -> MenuItem {
        self.title = title().into_value();
        self
    }

    fn select_handler(&self) -> Handler {
        let (bound, handler) = (self.bound.clone(), self.handler.clone());
        Rc::new(move || {
            if let Some(bound) = &bound {
                bound();
            }
            if let Some(handler) = &handler {
                handler();
            }
        })
    }
}

/// A line between groups of items.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuSeparator;

impl MenuSeparator {
    pub fn new() -> MenuSeparator {
        MenuSeparator
    }

    #[doc(hidden)]
    pub fn __tag() -> MenuSeparator {
        MenuSeparator
    }
}

enum Entry {
    Item(MenuItem),
    Separator,
    Submenu(Menu),
    /// Entries built again whenever what they read changes.
    Dynamic(Rc<dyn Fn() -> Vec<Entry>>),
}

/// A menu of the bar, or a submenu of another menu.
///
/// ```ignore
/// Menu::new("File")
///     .item(MenuItem::new("New").shortcut(Shortcut::primary('n')).on_select(new))
///     .submenu(Menu::new("Open Recent").children_with(move || {
///         recent.get().into_iter().map(|path| MenuItem::new(path.clone()).on_select(move || open(&path))).collect::<Vec<_>>()
///     }))
///     .separator()
///     .item(MenuItem::new("Settings…").role(MenuRole::Settings).on_select(settings))
/// ```
///
/// In `view!`, `<Menu title="File">…</Menu>` takes items, `<MenuSeparator/>`s
/// and `Menu`s as children.
pub struct Menu {
    title: Value<String>,
    visible: Value<bool>,
    entries: Vec<Entry>,
}

impl Menu {
    pub fn new(title: impl IntoValue<String>) -> Menu {
        Menu { title: title.into_value(), visible: Value::Static(true), entries: Vec::new() }
    }

    pub fn title(mut self, title: impl IntoValue<String>) -> Menu {
        self.title = title.into_value();
        self
    }

    /// In the bar (or its menu) while true.
    pub fn visible(mut self, visible: impl IntoValue<bool>) -> Menu {
        self.visible = visible.into_value();
        self
    }

    pub fn item(mut self, item: MenuItem) -> Menu {
        self.entries.push(Entry::Item(item));
        self
    }

    pub fn separator(mut self) -> Menu {
        self.entries.push(Entry::Separator);
        self
    }

    pub fn submenu(mut self, menu: Menu) -> Menu {
        self.entries.push(Entry::Submenu(menu));
        self
    }

    /// Items, separators and submenus, in order.
    pub fn children(self, children: impl MenuEntries) -> Menu {
        children.add_to(self)
    }

    /// Entries built by `children`, and built again whenever what it reads
    /// changes: a list of recent files, say.
    pub fn children_with<E: MenuEntries>(mut self, children: impl Fn() -> E + 'static) -> Menu {
        self.entries.push(Entry::Dynamic(Rc::new(move || children().add_to(Menu::new(String::new())).entries)));
        self
    }

    #[doc(hidden)]
    pub fn __tag() -> Menu {
        Menu::new(String::new())
    }

    #[doc(hidden)]
    pub fn __children<E: MenuEntries>(self, children: impl FnOnce() -> E) -> Menu {
        self.children(children())
    }
}

/// What a [`Menu`] takes as children: [`MenuItem`]s, [`MenuSeparator`]s,
/// [`Menu`]s (as submenus), and tuples, `Vec`s and `Option`s of them.
pub trait MenuEntries {
    fn add_to(self, menu: Menu) -> Menu;
}

impl MenuEntries for MenuItem {
    fn add_to(self, menu: Menu) -> Menu {
        menu.item(self)
    }
}

impl MenuEntries for MenuSeparator {
    fn add_to(self, menu: Menu) -> Menu {
        menu.separator()
    }
}

impl MenuEntries for Menu {
    fn add_to(self, menu: Menu) -> Menu {
        menu.submenu(self)
    }
}

impl<E: MenuEntries> MenuEntries for Vec<E> {
    fn add_to(self, menu: Menu) -> Menu {
        self.into_iter().fold(menu, |menu, entry| entry.add_to(menu))
    }
}

impl<E: MenuEntries> MenuEntries for Option<E> {
    fn add_to(self, menu: Menu) -> Menu {
        match self {
            Some(entry) => entry.add_to(menu),
            None => menu,
        }
    }
}

impl MenuEntries for () {
    fn add_to(self, menu: Menu) -> Menu {
        menu
    }
}

/// The menus of a bar: [`Menu`]s, and tuples, `Vec`s and `Option`s of them.
pub trait Menus {
    fn add_to(self, bar: MenuBar) -> MenuBar;
}

impl Menus for Menu {
    fn add_to(self, bar: MenuBar) -> MenuBar {
        bar.menu(self)
    }
}

impl<M: Menus> Menus for Vec<M> {
    fn add_to(self, bar: MenuBar) -> MenuBar {
        self.into_iter().fold(bar, |bar, menu| menu.add_to(bar))
    }
}

impl<M: Menus> Menus for Option<M> {
    fn add_to(self, bar: MenuBar) -> MenuBar {
        match self {
            Some(menu) => menu.add_to(bar),
            None => bar,
        }
    }
}

impl Menus for () {
    fn add_to(self, bar: MenuBar) -> MenuBar {
        bar
    }
}

macro_rules! tuple_menus {
    ($($name:ident),+) => {
        impl<$($name: MenuEntries),+> MenuEntries for ($($name,)+) {
            #[allow(non_snake_case)]
            fn add_to(self, menu: Menu) -> Menu {
                let ($($name,)+) = self;
                $(let menu = $name.add_to(menu);)+
                menu
            }
        }

        impl<$($name: Menus),+> Menus for ($($name,)+) {
            #[allow(non_snake_case)]
            fn add_to(self, bar: MenuBar) -> MenuBar {
                let ($($name,)+) = self;
                $(let bar = $name.add_to(bar);)+
                bar
            }
        }
    };
}

tuple_menus!(A);
tuple_menus!(A, B);
tuple_menus!(A, B, C);
tuple_menus!(A, B, C, D);
tuple_menus!(A, B, C, D, E);
tuple_menus!(A, B, C, D, E, F);
tuple_menus!(A, B, C, D, E, F, G);
tuple_menus!(A, B, C, D, E, F, G, H);
tuple_menus!(A, B, C, D, E, F, G, H, I);
tuple_menus!(A, B, C, D, E, F, G, H, I, J);
tuple_menus!(A, B, C, D, E, F, G, H, I, J, K);
tuple_menus!(A, B, C, D, E, F, G, H, I, J, K, L);

/// Menus for the app ([`set_menu`]) or for one window: declared anywhere
/// in a window's content, like a `Toolbar`, a `MenuBar` holds that
/// window's own menus while it's there. A window
/// shows the app's menus and its own; on macOS, whose menu bar is the
/// app's, a window's menus are there while it's the main window.
/// Platforms add their standard menus around them.
///
/// ```ignore
/// view! {
///     <MenuBar>
///         <Menu title="Machine">
///             <MenuItem shortcut=Shortcut::primary('r') @select=start>"Start"</MenuItem>
///             <MenuSeparator/>
///             <MenuItem bind=show_log>"Show Log"</MenuItem>
///         </Menu>
///     </MenuBar>
/// }
/// ```
#[derive(Default)]
pub struct MenuBar {
    menus: Vec<Menu>,
}

impl MenuBar {
    pub fn new() -> MenuBar {
        MenuBar::default()
    }

    pub fn menu(mut self, menu: Menu) -> MenuBar {
        self.menus.push(menu);
        self
    }

    pub fn children(self, menus: impl Menus) -> MenuBar {
        menus.add_to(self)
    }

    #[doc(hidden)]
    pub fn __tag() -> MenuBar {
        MenuBar::new()
    }

    #[doc(hidden)]
    pub fn __children<M: Menus>(self, menus: impl FnOnce() -> M) -> MenuBar {
        self.children(menus())
    }

    /// The bar as data, reading reactive values, and each item's handler.
    /// `ids` holds the id given to each item position, so ids stay the
    /// same while the structure does; `new_id` makes more.
    pub(crate) fn collect(
        &self,
        ids: &mut Vec<u32>,
        new_id: &mut dyn FnMut() -> u32,
    ) -> (MenuBarData, Vec<(u32, Handler)>) {
        let mut walk = Walk { ids, position: 0, new_id, handlers: Vec::new() };
        let menus = self.menus.iter().filter_map(|menu| walk.menu(menu)).collect();
        (MenuBarData { menus }, walk.handlers)
    }
}

struct Walk<'a> {
    ids: &'a mut Vec<u32>,
    position: usize,
    new_id: &'a mut dyn FnMut() -> u32,
    handlers: Vec<(u32, Handler)>,
}

impl Walk<'_> {
    fn menu(&mut self, menu: &Menu) -> Option<MenuData> {
        if !menu.visible.get() {
            return None;
        }
        let mut entries = Vec::new();
        self.entries(&menu.entries, &mut entries);
        Some(MenuData { title: menu.title.get(), entries })
    }

    fn entries(&mut self, entries: &[Entry], out: &mut Vec<MenuEntry>) {
        for entry in entries {
            match entry {
                Entry::Item(item) => {
                    if !item.visible.get() {
                        continue;
                    }
                    let id = self.next_id();
                    self.handlers.push((id, item.select_handler()));
                    out.push(MenuEntry::Item(MenuItemData {
                        id,
                        title: item.title.get(),
                        shortcut: item.shortcut,
                        enabled: item.enabled.get(),
                        check: match &item.check {
                            Mark::None => MenuCheck::None,
                            Mark::Check(checked) => MenuCheck::Check(checked.get()),
                            Mark::Radio(checked) => MenuCheck::Radio(checked.get()),
                        },
                        role: item.role,
                    }));
                }
                Entry::Separator => out.push(MenuEntry::Separator),
                Entry::Submenu(menu) => out.extend(self.menu(menu).map(MenuEntry::Submenu)),
                Entry::Dynamic(children) => self.entries(&children(), out),
            }
        }
    }

    fn next_id(&mut self) -> u32 {
        if self.position == self.ids.len() {
            self.ids.push((self.new_id)());
        }
        self.position += 1;
        self.ids[self.position - 1]
    }
}

impl View for MenuBar {
    /// A placeholder in the tree, which takes no room: the menus are the
    /// window's, or the app's outside any window.
    fn build(self, ui: &Ui) -> NodeId {
        let placeholder = ui.create(WidgetKind::Fragment, Vec::new());
        match inject::<CurrentWindow>() {
            Some(CurrentWindow(window)) => ui.set_window_menu(window, self),
            None => ui.set_menu(self),
        }
        placeholder
    }
}

// ----------------------------------------------------------- async glue

struct OneShot<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

/// A reply callback and the future that resolves when it is called.
pub(crate) fn reply_future<T: 'static>() -> (Reply<T>, impl Future<Output = T> + use<T>) {
    let shared = Rc::new(RefCell::new(OneShot { value: None, waker: None }));
    let sender = shared.clone();
    let reply: Reply<T> = Box::new(move |value| {
        let waker = {
            let mut shared = sender.borrow_mut();
            shared.value = Some(value);
            shared.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    });
    let future = std::future::poll_fn(move |cx| {
        let mut shared = shared.borrow_mut();
        match shared.value.take() {
            Some(value) => Poll::Ready(value),
            None => {
                shared.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    });
    (reply, future)
}

fn ui() -> Ui {
    crate::task::current_ui()
}

pub fn clipboard_text() -> impl Future<Output = Option<String>> + use<> {
    ui().clipboard_text()
}

/// Puts text on the clipboard right away; await to learn whether it worked.
pub fn set_clipboard_text(text: &str) -> impl Future<Output = Result<(), ServiceError>> + use<> {
    ui().set_clipboard_text(text)
}

/// Shows an alert on the active window; resolves to the chosen button's index.
pub fn alert(alert: Alert) -> impl Future<Output = usize> + use<> {
    ui().alert(None, alert)
}

pub fn open_file(request: OpenFile) -> impl Future<Output = Option<Vec<PathBuf>>> + use<> {
    ui().open_file(None, request)
}

pub fn save_file(request: SaveFile) -> impl Future<Output = Option<PathBuf>> + use<> {
    ui().save_file(None, request)
}

/// Installs the app's menus; see [`Ui::set_menu`]. A window's own menus
/// are a [`MenuBar`] in its content.
pub fn set_menu(menu: MenuBar) {
    ui().set_menu(menu);
}
