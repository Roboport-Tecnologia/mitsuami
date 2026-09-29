//! Turning built menus into data and handlers, again whenever what they
//! read changes: menu bars, context menus and menu buttons' menus.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_reactive::inject;

use crate::command::UiEvent;
use crate::ui::Ui;
use crate::view::View;
use crate::widget::{CurrentWindow, NodeId, Prop, WidgetKind};

use super::menu::{Entry, Handler, Mark, Menu, MenuBar};
use super::menu_data::{MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData};

impl MenuBar {
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

impl Menu {
    /// The menu's entries as data, for a context menu (whose title isn't
    /// shown), and each item's handler; see [`MenuBar::collect`].
    fn collect_entries(
        &self,
        ids: &mut Vec<u32>,
        new_id: &mut dyn FnMut() -> u32,
    ) -> (Vec<MenuEntry>, Vec<(u32, Handler)>) {
        let mut walk = Walk { ids, position: 0, new_id, handlers: Vec::new() };
        let mut entries = Vec::new();
        walk.entries(&self.entries, &mut entries);
        (entries, walk.handlers)
    }
}

/// Gives a node the context menu `.context_menu(...)` built: sends it as
/// [`Prop::ContextMenu`] again whenever its reactive parts change, and runs
/// an item's handler when the platform reports it chosen. Ids only need to
/// be unique within the node's menu, since the choice comes as the node's
/// event.
pub(crate) fn install_context_menu(ui: &Ui, id: NodeId, menu: Menu) {
    install_menu(ui, id, menu, Prop::ContextMenu, |event| match event {
        UiEvent::ContextMenuItem(item) => Some(*item),
        _ => None,
    });
}

/// Gives a `MenuButton` its menu, as [`Prop::Menu`], as a context menu is
/// given (see `install_context_menu`). The menu's title isn't shown.
#[doc(hidden)]
pub fn install_button_menu(ui: &Ui, id: NodeId, menu: Menu) {
    install_menu(ui, id, menu, Prop::Menu, |event| match event {
        UiEvent::MenuItem(item) => Some(*item),
        _ => None,
    });
}

fn install_menu(
    ui: &Ui,
    id: NodeId,
    menu: Menu,
    prop: fn(Vec<MenuEntry>) -> Prop,
    chosen_item: fn(&UiEvent) -> Option<u32>,
) {
    let handlers: Rc<RefCell<HashMap<u32, Handler>>> = Rc::default();
    // As menu bars' handlers do, they run in the scope that built the menu.
    let scope = mitsuami_reactive::Owner::current();
    let chosen = handlers.clone();
    ui.on_event(id, move |event| {
        let Some(item) = chosen_item(event) else { return };
        let item = &item;
        let Some(handler) = chosen.borrow().get(item).cloned() else { return };
        match scope {
            Some(scope) if scope.is_alive() => scope.with(|| handler()),
            _ => handler(),
        }
    });
    let ui = ui.clone();
    let (mut ids, mut next_id) = (Vec::new(), 0);
    mitsuami_reactive::effect(move || {
        let (entries, collected) = menu.collect_entries(&mut ids, &mut || {
            next_id += 1;
            next_id
        });
        *handlers.borrow_mut() = collected.into_iter().collect();
        ui.set_prop(id, prop(entries));
    });
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
