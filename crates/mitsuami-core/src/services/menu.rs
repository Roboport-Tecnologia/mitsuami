//! The menu builder: items, separators, menus and menu bars.

use std::rc::Rc;

use mitsuami_reactive::{IntoValue, Signal, Value};

use super::menu_data::{MenuRole, Shortcut};

pub(super) type Handler = Rc<dyn Fn()>;

pub(super) enum Mark {
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
    pub(super) title: Value<String>,
    pub(super) shortcut: Option<Shortcut>,
    pub(super) enabled: Value<bool>,
    pub(super) visible: Value<bool>,
    pub(super) check: Mark,
    pub(super) role: MenuRole,
    /// What `bind` and `radio` do, before the handler.
    pub(super) bound: Option<Handler>,
    pub(super) handler: Option<Handler>,
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

    pub(super) fn select_handler(&self) -> Handler {
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

pub(super) enum Entry {
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
    pub(super) title: Value<String>,
    pub(super) visible: Value<bool>,
    pub(super) entries: Vec<Entry>,
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
///
/// [`set_menu`]: super::set_menu
#[derive(Default)]
pub struct MenuBar {
    pub(super) menus: Vec<Menu>,
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
}
