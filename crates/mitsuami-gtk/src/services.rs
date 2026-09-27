//! Clipboard, dialogs and menus on GTK 4.
//!
//! GNOME apps have no menu bar: the app's menus go in a menu button at the
//! end of each window's header bar (the "primary menu", F10), one section
//! per menu, plus Quit. A window's own menus join the app's in its button.
//! Shortcuts work in every window. GTK's text widgets
//! bring their own Cut/Copy/Paste context menus and keybindings, so there is
//! no Edit menu to add.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use mitsuami_core::NodeId;
use mitsuami_core::services::{
    Alert, MenuBarData, MenuCheck, MenuEntry, MenuItemData, MenuRole, OpenFile, Reply, SaveFile, ServiceError,
    Services, Shortcut,
};

use crate::backend::{GtkHandle, State, WindowParts, dialog_parent, file_filters};

/// A window's primary menu as GTK objects, and the data it was built from.
pub(crate) struct MenuParts {
    data: MenuBarData,
    model: gio::Menu,
    actions: gio::SimpleActionGroup,
    /// `(trigger, action, target)`, e.g. `("<Control>n", "mitsuami.item-1", None)`.
    shortcuts: Vec<(String, String, Option<glib::Variant>)>,
    has_menus: bool,
}

const GROUP: &str = "mitsuami";

impl MenuParts {
    /// Puts the menu in a window: the header bar's menu button, the actions
    /// and the keyboard shortcuts.
    fn install(&self, window: &WindowParts) {
        window.window.insert_action_group(GROUP, Some(&self.actions));
        window.menu_button.set_menu_model(Some(&self.model));
        window.menu_button.set_visible(self.has_menus);
        let controller = &window.shortcuts;
        while let Some(old) = controller.item(0).and_downcast::<gtk::Shortcut>() {
            controller.remove_shortcut(&old);
        }
        for (trigger, action, target) in &self.shortcuts {
            if let Some(trigger) = gtk::ShortcutTrigger::parse_string(trigger) {
                let shortcut = gtk::Shortcut::new(Some(trigger), Some(gtk::NamedAction::new(action)));
                // A radio item's action takes the item's id.
                shortcut.set_arguments(target.as_ref());
                controller.add_shortcut(shortcut);
            }
        }
    }

    /// Enabled and checked states, in place: rebuilding would close the
    /// menu if it's open.
    fn update(&mut self, data: MenuBarData) {
        for item in data.items() {
            let Some(action) = self.actions.lookup_action(&action_name(item.id)).and_downcast::<gio::SimpleAction>()
            else {
                continue;
            };
            action.set_enabled(item.enabled);
            if let Some(state) = check_state(item) {
                action.set_state(&state);
            }
        }
        self.data = data;
    }
}

/// The menus windows show: the app's, and each window's own with them.
#[derive(Default)]
pub(crate) struct Menus {
    app: MenuBarData,
    /// Windows' own menus, which may come before the window does.
    windows: HashMap<NodeId, MenuBarData>,
    activate: Option<Rc<dyn Fn(u32)>>,
    /// Each window's primary menu.
    installed: HashMap<NodeId, MenuParts>,
    /// For Quit, which asks every window to close.
    pub(crate) backend: Weak<RefCell<State>>,
}

impl Menus {
    /// Takes the app's menus (`None`) or a window's; an empty bar removes a
    /// window's.
    pub(crate) fn set(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        self.activate = Some(activate);
        match window {
            None => self.app = menu.clone(),
            Some(window) if menu.menus.is_empty() => _ = self.windows.remove(&window),
            Some(window) => _ = self.windows.insert(window, menu.clone()),
        }
    }

    /// Brings a window's primary menu up to date: in place when only
    /// states changed, otherwise built again.
    pub(crate) fn show_in(&mut self, id: NodeId, window: &WindowParts) {
        let data = match self.windows.get(&id) {
            Some(own) => self.app.merged(own),
            None => self.app.clone(),
        };
        if let Some(parts) = self.installed.get_mut(&id)
            && parts.data.same_structure(&data)
        {
            parts.update(data);
            return;
        }
        let parts = self.build(data);
        parts.install(window);
        self.installed.insert(id, parts);
    }

    pub(crate) fn actions(&self, id: NodeId) -> Option<gio::ActionGroup> {
        self.installed.get(&id).map(|parts| parts.actions.clone().upcast())
    }

    /// The window is gone. Its own menus stay until the app removes them.
    pub(crate) fn forget(&mut self, id: NodeId) {
        self.installed.remove(&id);
    }

    /// One section per menu, split at its separators and labelled with its
    /// title; submenus in their section. Items with a role go last, in
    /// GNOME's order: Preferences, About, Quit.
    fn build(&self, data: MenuBarData) -> MenuParts {
        let mut builder = Builder {
            actions: gio::SimpleActionGroup::new(),
            shortcuts: Vec::new(),
            activate: self.activate.clone().unwrap_or_else(|| Rc::new(|_| {})),
        };
        let model = gio::Menu::new();
        let mut rest = data.clone();
        let settings = rest.take_role(MenuRole::Settings);
        let about = rest.take_role(MenuRole::About);
        let quit = rest.take_role(MenuRole::Quit);
        for menu in &rest.menus {
            for (i, section) in builder.sections(&menu.entries).into_iter().enumerate() {
                // The menu's title labels its first section.
                let label = (i == 0).then_some(menu.title.as_str());
                model.append_section(label, &section);
            }
        }
        let last = gio::Menu::new();
        if let Some(settings) = &settings {
            // GNOME's shortcut for Preferences.
            let shortcut = settings.shortcut.or(Some(Shortcut::primary(',')));
            last.append_item(&builder.item(settings, &settings.title, shortcut));
        }
        if let Some(about) = &about {
            last.append_item(&builder.item(about, &about.title, about.shortcut));
        }
        match &quit {
            // The app's own Quit, in place of ours.
            Some(quit) => last.append_item(&builder.item(quit, "Quit", Some(Shortcut::primary('q')))),
            None => {
                let action = gio::SimpleAction::new("quit", None);
                let backend = self.backend.clone();
                action.connect_activate(move |_, _| {
                    if let Some(backend) = GtkHandle::from_weak(&backend) {
                        backend.request_quit();
                    }
                });
                builder.actions.add_action(&action);
                let item = gio::MenuItem::new(Some("Quit"), Some(&format!("{GROUP}.quit")));
                item.set_attribute_value("accel", Some(&"<Control>q".to_variant()));
                last.append_item(&item);
                builder.shortcuts.push(("<Control>q".into(), format!("{GROUP}.quit"), None));
            }
        }
        model.append_section(None, &last);
        let has_menus = !data.menus.is_empty();
        MenuParts { data, model, actions: builder.actions, shortcuts: builder.shortcuts, has_menus }
    }
}

struct Builder {
    actions: gio::SimpleActionGroup,
    shortcuts: Vec<(String, String, Option<glib::Variant>)>,
    activate: Rc<dyn Fn(u32)>,
}

impl Builder {
    /// Separators split entries into sections.
    fn sections(&mut self, entries: &[MenuEntry]) -> Vec<gio::Menu> {
        entries
            .split(|e| matches!(e, MenuEntry::Separator))
            .filter(|group| !group.is_empty())
            .map(|group| {
                let section = gio::Menu::new();
                for entry in group {
                    match entry {
                        MenuEntry::Item(item) => section.append_item(&self.item(item, &item.title, item.shortcut)),
                        MenuEntry::Submenu(menu) => {
                            let submenu = gio::Menu::new();
                            for part in self.sections(&menu.entries) {
                                submenu.append_section(None, &part);
                            }
                            section.append_submenu(Some(&menu.title), &submenu);
                        }
                        MenuEntry::Separator => {}
                    }
                }
                section
            })
            .collect()
    }

    /// An item and its action. A check item's action holds a boolean; a
    /// radio item's holds its own id while chosen and "" otherwise, with
    /// its id as the item's target, which is how GTK tells radio items.
    /// Neither changes its own state: the app's comes back from the core.
    fn item(&mut self, data: &MenuItemData, title: &str, shortcut: Option<Shortcut>) -> gio::MenuItem {
        let name = action_name(data.id);
        let target = matches!(data.check, MenuCheck::Radio(_)).then(|| data.id.to_string().to_variant());
        let action = match check_state(data) {
            Some(state) => gio::SimpleAction::new_stateful(&name, target.as_ref().map(|t| t.type_()), &state),
            None => gio::SimpleAction::new(&name, None),
        };
        action.set_enabled(data.enabled);
        let (activate, id) = (self.activate.clone(), data.id);
        action.connect_activate(move |_, _| activate(id));
        self.actions.add_action(&action);
        let detailed = format!("{GROUP}.{name}");
        let item = gio::MenuItem::new(Some(title), None);
        item.set_action_and_target_value(Some(&detailed), target.as_ref());
        if let Some(shortcut) = shortcut {
            let trigger = trigger(&shortcut);
            item.set_attribute_value("accel", Some(&trigger.to_variant()));
            self.shortcuts.push((trigger, detailed, target));
        }
        item
    }
}

fn action_name(id: u32) -> String {
    format!("item-{id}")
}

/// The state of a check or radio item's action.
fn check_state(item: &MenuItemData) -> Option<glib::Variant> {
    match item.check {
        MenuCheck::None => None,
        MenuCheck::Check(on) => Some(on.to_variant()),
        MenuCheck::Radio(on) => Some(if on { item.id.to_string() } else { String::new() }.to_variant()),
    }
}

/// `<Control><Shift>n`, the notation of `gtk_shortcut_trigger_parse_string`,
/// which names keys like `,` (`comma`).
fn trigger(shortcut: &Shortcut) -> String {
    let mut trigger = String::new();
    if shortcut.primary {
        trigger.push_str("<Control>");
    }
    if shortcut.shift {
        trigger.push_str("<Shift>");
    }
    if shortcut.alt {
        trigger.push_str("<Alt>");
    }
    // SAFETY: any keyval is a valid `Key`; unknown ones have no name.
    let key: gdk::Key = unsafe { glib::translate::from_glib(gdk::unicode_to_keyval(shortcut.key as u32)) };
    match key.name() {
        Some(name) => trigger.push_str(&name),
        None => trigger.push(shortcut.key),
    }
    trigger
}

pub struct GtkServices {
    backend: GtkHandle,
}

impl GtkServices {
    pub(crate) fn new(backend: GtkHandle) -> GtkServices {
        GtkServices { backend }
    }
}

/// Wraps a one-shot reply for callbacks GTK types as reusable.
fn once<T>(reply: Reply<T>) -> impl Fn(T) {
    let reply = Cell::new(Some(reply));
    move |value| {
        if let Some(reply) = reply.take() {
            reply(value);
        }
    }
}

fn path(file: &gio::File) -> Option<PathBuf> {
    file.path()
}

impl Services for GtkServices {
    fn clipboard_text(&mut self, reply: Reply<Option<String>>) {
        let Some(display) = gdk::Display::default() else { return reply(None) };
        display.clipboard().read_text_async(None::<&gio::Cancellable>, move |result| {
            reply(result.ok().flatten().map(|text| text.to_string()))
        });
    }

    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>) {
        match gdk::Display::default() {
            Some(display) => {
                display.clipboard().set_text(text);
                reply(Ok(()));
            }
            None => reply(Err(ServiceError::Unavailable)),
        }
    }

    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>) {
        let buttons = alert.effective_buttons();
        let dialog = gtk::AlertDialog::builder().message(&alert.title).modal(true).default_button(0).build();
        if let Some(message) = &alert.message {
            dialog.set_detail(message);
        }
        let labels: Vec<&str> = buttons.iter().map(String::as_str).collect();
        dialog.set_buttons(&labels);
        // Escape (or closing the dialog) chooses the last button, which by
        // convention is the least committal one (Cancel).
        let cancel = buttons.len() - 1;
        dialog.set_cancel_button(cancel as i32);
        let window = dialog_parent(&self.backend, parent);
        dialog.choose(window.as_ref(), None::<&gio::Cancellable>, move |result| {
            reply(result.map_or(cancel, |i| (i.max(0) as usize).min(cancel)))
        });
    }

    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>) {
        let dialog = gtk::FileDialog::new();
        if let Some(title) = &request.title {
            dialog.set_title(title);
        }
        if let Some(filters) = file_filters(&request.filters) {
            dialog.set_filters(Some(&filters));
        }
        let window = dialog_parent(&self.backend, parent);
        let reply = Rc::new(once(reply));
        let many =
            {
                let reply = reply.clone();
                move |result: Result<gio::ListModel, glib::Error>| {
                    reply(result.ok().map(|files| {
                        files.iter::<gio::File>().filter_map(|f| f.ok()).filter_map(|f| path(&f)).collect()
                    }))
                }
            };
        let one =
            move |result: Result<gio::File, glib::Error>| reply(result.ok().and_then(|f| path(&f)).map(|p| vec![p]));
        let cancellable = None::<&gio::Cancellable>;
        match (request.directories, request.multiple) {
            (false, false) => dialog.open(window.as_ref(), cancellable, one),
            (false, true) => dialog.open_multiple(window.as_ref(), cancellable, many),
            (true, false) => dialog.select_folder(window.as_ref(), cancellable, one),
            (true, true) => dialog.select_multiple_folders(window.as_ref(), cancellable, many),
        }
    }

    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>) {
        let dialog = gtk::FileDialog::new();
        if let Some(title) = &request.title {
            dialog.set_title(title);
        }
        if let Some(name) = &request.default_name {
            dialog.set_initial_name(Some(name));
        }
        if let Some(filters) = file_filters(&request.filters) {
            dialog.set_filters(Some(&filters));
        }
        let window = dialog_parent(&self.backend, parent);
        dialog
            .save(window.as_ref(), None::<&gio::Cancellable>, move |result| reply(result.ok().and_then(|f| path(&f))));
    }

    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        self.backend.set_menu(window, menu, activate);
    }
}
