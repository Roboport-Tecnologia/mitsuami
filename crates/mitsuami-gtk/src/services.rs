//! Clipboard, dialogs and menus on GTK 4.
//!
//! GNOME apps have no menu bar: the app's menus go in a menu button at the
//! end of each window's header bar (the "primary menu", F10), one section
//! per menu, plus Quit. A window's own menus join the app's in its button.
//! Shortcuts work in every window. GTK's text widgets
//! bring their own Cut/Copy/Paste context menus and keybindings, so there is
//! no Edit menu to add.
//!
//! Widgets' context menus are built as the app's menus are, and shown in a
//! popover at the pointer, or added to a text widget's own menu.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use mitsuami_core::services::{
    Alert, MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData, MenuRole, OpenFile, Reply, SaveFile,
    ServiceError, Services, Shortcut, existing_folder, menu_item_by_id,
};
use mitsuami_core::{ActionError, NodeId};

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
        update_actions(&self.actions, &data.items());
        self.data = data;
    }
}

/// Items' enabled and checked states, on their actions.
fn update_actions(actions: &gio::SimpleActionGroup, items: &[&MenuItemData]) {
    for item in items {
        let Some(action) = actions.lookup_action(&action_name(item.id)).and_downcast::<gio::SimpleAction>() else {
            continue;
        };
        action.set_enabled(item.enabled);
        if let Some(state) = check_state(item) {
            action.set_state(&state);
        }
    }
}

/// The menus windows show: the app's, and each window's own with them.
#[derive(Default)]
pub(crate) struct Menus {
    app: MenuBarData,
    /// Windows' own menus, which may come before the window does.
    windows: HashMap<NodeId, MenuBarData>,
    /// Dialogs, which show only their own menus.
    modal: HashSet<NodeId>,
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
        let modal = self.modal.contains(&id);
        let data = self.app.for_window(self.windows.get(&id), modal);
        if let Some(parts) = self.installed.get_mut(&id)
            && parts.data.same_structure(&data)
        {
            parts.update(data);
            return;
        }
        let parts = self.build(data, modal);
        parts.install(window);
        self.installed.insert(id, parts);
    }

    pub(crate) fn actions(&self, id: NodeId) -> Option<gio::ActionGroup> {
        self.installed.get(&id).map(|parts| parts.actions.clone().upcast())
    }

    /// The window is gone. Its own menus stay until the app removes them.
    pub(crate) fn forget(&mut self, id: NodeId) {
        self.installed.remove(&id);
        self.modal.remove(&id);
    }

    /// Marks a window as a dialog; returns whether it wasn't one already.
    pub(crate) fn set_modal(&mut self, id: NodeId) -> bool {
        self.modal.insert(id)
    }

    /// One section per menu, split at its separators and labelled with its
    /// title; submenus in their section. Items with a role go last, in
    /// GNOME's order: Preferences, About, Quit. Dialogs have no Quit of
    /// their own, as GNOME's don't.
    fn build(&self, data: MenuBarData, modal: bool) -> MenuParts {
        let mut builder = Builder {
            group: GROUP,
            exact: false,
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
            None if modal => {}
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
        if last.n_items() > 0 {
            model.append_section(None, &last);
        }
        let has_menus = !data.menus.is_empty();
        MenuParts { data, model, actions: builder.actions, shortcuts: builder.shortcuts, has_menus }
    }
}

struct Builder {
    /// The action group's name on the widget: `mitsuami.item-1`.
    group: &'static str,
    /// Keeps empty sections (from leading, trailing or doubled separators),
    /// so the entries read back as they came; GTK shows none of them.
    exact: bool,
    actions: gio::SimpleActionGroup,
    shortcuts: Vec<(String, String, Option<glib::Variant>)>,
    activate: Rc<dyn Fn(u32)>,
}

impl Builder {
    /// Separators split entries into sections.
    fn sections(&mut self, entries: &[MenuEntry]) -> Vec<gio::Menu> {
        let exact = self.exact;
        entries
            .split(|e| matches!(e, MenuEntry::Separator))
            .filter(|group| exact || !group.is_empty())
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
        let detailed = format!("{}.{name}", self.group);
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

/// The group of a widget's context menu actions: `context.item-1`.
const CONTEXT_GROUP: &str = "context";

/// A widget's context menu as GTK objects: the model a popover (or a text
/// widget's own menu) shows, and the items' actions, which the widget
/// carries. Both are kept across changes, so a shown menu follows them.
pub(crate) struct ContextMenu {
    data: Vec<MenuEntry>,
    model: gio::Menu,
    actions: gio::SimpleActionGroup,
    activate: Rc<dyn Fn(u32)>,
    /// The popover while it's shown, parented to the widget.
    popover: Rc<RefCell<Option<gtk::PopoverMenu>>>,
}

impl ContextMenu {
    /// A menu for `widget`, whose items call `activate` with their id.
    /// Text widgets add it to their own Cut/Copy/Paste menu, as their
    /// `extra-menu`; other widgets show it in a popover of their own.
    pub(crate) fn new(widget: &gtk::Widget, activate: Rc<dyn Fn(u32)>) -> ContextMenu {
        let menu = ContextMenu {
            data: Vec::new(),
            model: gio::Menu::new(),
            actions: gio::SimpleActionGroup::new(),
            activate,
            popover: Rc::default(),
        };
        widget.insert_action_group(CONTEXT_GROUP, Some(&menu.actions));
        match text_widget(widget) {
            Some(TextWidget::Entry(entry)) => entry.set_extra_menu(Some(&menu.model)),
            Some(TextWidget::Password(entry)) => entry.set_extra_menu(Some(&menu.model)),
            Some(TextWidget::Text(text)) => text.set_extra_menu(Some(&menu.model)),
            None => menu.attach(widget),
        }
        menu
    }

    /// How GTK's widgets open their menu ("menu.popup"): a secondary click
    /// or a long press at the pointer, Shift+F10 or the Menu key at the
    /// widget. Bubbling, so children that claim these (a text field's own
    /// menu, a child's menu) keep them, and children without one show
    /// this. Without items, the event goes on to the container.
    fn attach(&self, widget: &gtk::Widget) {
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let (model, popover) = (self.model.clone(), self.popover.clone());
        click.connect_pressed(move |gesture, _, x, y| {
            if has_items(&model) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                popup(&gesture.widget().expect("attached"), &model, &popover, Some((x, y)));
            } else {
                gesture.set_state(gtk::EventSequenceState::Denied);
            }
        });
        widget.add_controller(click);

        let press = gtk::GestureLongPress::new();
        press.set_touch_only(true);
        let (model, popover) = (self.model.clone(), self.popover.clone());
        press.connect_pressed(move |gesture, x, y| {
            if has_items(&model) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                popup(&gesture.widget().expect("attached"), &model, &popover, Some((x, y)));
            } else {
                gesture.set_state(gtk::EventSequenceState::Denied);
            }
        });
        widget.add_controller(press);

        let keys = gtk::ShortcutController::new();
        let (model, popover) = (self.model.clone(), self.popover.clone());
        let action = gtk::CallbackAction::new(move |widget, _| {
            if !has_items(&model) {
                return glib::Propagation::Proceed;
            }
            popup(widget, &model, &popover, None);
            glib::Propagation::Stop
        });
        keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string("<Shift>F10|Menu"), Some(action)));
        widget.add_controller(keys);
    }

    /// Takes the app's entries: states in place when only they changed,
    /// otherwise the model and actions filled again.
    pub(crate) fn set(&mut self, widget: &gtk::Widget, entries: &[MenuEntry]) {
        let bar = |entries: &[MenuEntry]| MenuBarData {
            menus: vec![MenuData { title: String::new(), entries: entries.to_vec() }],
        };
        let new = bar(entries);
        if bar(&self.data).same_structure(&new) {
            update_actions(&self.actions, &new.items());
        } else {
            for name in self.actions.list_actions() {
                self.actions.remove_action(&name);
            }
            let mut builder = Builder {
                group: CONTEXT_GROUP,
                exact: true,
                actions: self.actions.clone(),
                shortcuts: Vec::new(),
                activate: self.activate.clone(),
            };
            let sections = builder.sections(entries);
            // Refilled in place, a shown popover adds the new submenus'
            // pages before the old ones go, and GTK warns about their
            // names. Given the model again, it starts over.
            self.show_model(widget, None);
            self.model.remove_all();
            for section in &sections {
                self.model.append_section(None, section);
            }
            self.show_model(widget, Some(&self.model));
        }
        self.data = entries.to_vec();
    }

    /// Gives the model to what shows it: the popover, if it's there, or
    /// the text widget's own menu.
    fn show_model(&self, widget: &gtk::Widget, model: Option<&gio::Menu>) {
        match text_widget(widget) {
            Some(TextWidget::Entry(entry)) => entry.set_extra_menu(model),
            Some(TextWidget::Password(entry)) => entry.set_extra_menu(model),
            Some(TextWidget::Text(text)) => text.set_extra_menu(model),
            None => {
                if let Some(popover) = &*self.popover.borrow() {
                    popover.set_menu_model(model);
                }
            }
        }
    }

    /// The popover goes before the widget: GTK wants a widget's children
    /// gone before it's finalized.
    pub(crate) fn close(&self) {
        if let Some(popover) = self.popover.borrow_mut().take() {
            popover.unparent();
        }
    }

    /// What the menu shows, read from the model the widget has (its own
    /// popover's, or a text widget's `extra-menu`) and the actions' states.
    /// Roles mean nothing here and come from the app's entries.
    pub(crate) fn entries(&self, widget: &gtk::Widget) -> Vec<MenuEntry> {
        let model = match text_widget(widget) {
            Some(TextWidget::Entry(entry)) => entry.extra_menu(),
            Some(TextWidget::Password(entry)) => entry.extra_menu(),
            Some(TextWidget::Text(text)) => text.extra_menu(),
            None => Some(self.model.clone().upcast()),
        };
        model.map(|model| self.read_sections(&model)).unwrap_or_default()
    }

    fn read_sections(&self, model: &gio::MenuModel) -> Vec<MenuEntry> {
        let mut entries = Vec::new();
        for i in 0..model.n_items() {
            if i > 0 {
                entries.push(MenuEntry::Separator);
            }
            if let Some(section) = model.item_link(i, "section") {
                entries.extend(self.read_items(&section));
            }
        }
        entries
    }

    fn read_items(&self, section: &gio::MenuModel) -> Vec<MenuEntry> {
        let string = |i, name| {
            section.item_attribute_value(i, name, Some(glib::VariantTy::STRING)).and_then(|v| v.get::<String>())
        };
        (0..section.n_items())
            .filter_map(|i| {
                let title = string(i, "label").unwrap_or_default();
                if let Some(submenu) = section.item_link(i, "submenu") {
                    let entries = self.read_sections(&submenu);
                    return Some(MenuEntry::Submenu(MenuData { title, entries }));
                }
                let action = string(i, "action")?;
                let name = action.strip_prefix(&format!("{CONTEXT_GROUP}."))?;
                let id: u32 = name.strip_prefix("item-")?.parse().ok()?;
                let action = self.actions.lookup_action(name)?;
                let sent = menu_item_by_id(&self.data, id);
                let target = string(i, "target");
                let state = action.state();
                let check = match (state.as_ref().and_then(|s| s.get::<bool>()), state.and_then(|s| s.get::<String>()))
                {
                    (Some(on), _) => MenuCheck::Check(on),
                    (None, Some(chosen)) => MenuCheck::Radio(target.as_ref() == Some(&chosen)),
                    (None, None) => MenuCheck::None,
                };
                // The accel label GTK shows; parsed only if it isn't the
                // app's shortcut as written.
                let shortcut = string(i, "accel").and_then(|accel| match sent.and_then(|s| s.shortcut) {
                    Some(shortcut) if trigger(&shortcut) == accel => Some(shortcut),
                    _ => parse_trigger(&accel),
                });
                Some(MenuEntry::Item(MenuItemData {
                    id,
                    title,
                    shortcut,
                    enabled: action.is_enabled(),
                    check,
                    role: sent.map(|s| s.role).unwrap_or_default(),
                }))
            })
            .collect()
    }

    /// The items' actions, for [`choose_context_item`] once no backend
    /// state is borrowed.
    pub(crate) fn chooser(&self) -> gio::SimpleActionGroup {
        self.actions.clone()
    }
}

/// What assistive technology does once it has shown the menu: activate the
/// item's action, whose handler reports the choice. A radio item's action
/// takes its id.
pub(crate) fn choose_context_item(actions: &gio::SimpleActionGroup, id: u32) -> Result<(), ActionError> {
    let name = action_name(id);
    let action = actions.lookup_action(&name).ok_or(ActionError::Unsupported)?;
    if !action.is_enabled() {
        return Err(ActionError::Disabled);
    }
    let target = action.parameter_type().map(|_| id.to_string().to_variant());
    actions.activate_action(&name, target.as_ref());
    Ok(())
}

/// Text widgets, which have their own context menu.
enum TextWidget {
    Entry(gtk::Entry),
    Password(gtk::PasswordEntry),
    /// A spin button's text.
    Text(gtk::Text),
}

fn text_widget(widget: &gtk::Widget) -> Option<TextWidget> {
    if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
        return Some(TextWidget::Entry(entry.clone()));
    }
    if let Some(entry) = widget.downcast_ref::<gtk::PasswordEntry>() {
        return Some(TextWidget::Password(entry.clone()));
    }
    // The spin buttons' own secondary clicks go to their minimum and
    // maximum, so a spin button's menu is its text's.
    let spin = widget.downcast_ref::<gtk::SpinButton>()?;
    spin.delegate().and_downcast::<gtk::Text>().map(TextWidget::Text)
}

/// Whether the menu has an item to show: empty sections show nothing.
fn has_items(model: &gio::Menu) -> bool {
    (0..model.n_items()).any(|i| model.item_link(i, "section").is_some_and(|s| s.n_items() > 0))
}

/// Shows the menu as GTK's labels and text fields show theirs: without an
/// arrow, below and after the pointer, or at the whole widget from the
/// keyboard. The popover is made on demand and unparented once closed,
/// after the chosen item's action runs (GTK closes the menu first).
fn popup(
    widget: &gtk::Widget,
    model: &gio::Menu,
    slot: &Rc<RefCell<Option<gtk::PopoverMenu>>>,
    at: Option<(f64, f64)>,
) {
    let existing = slot.borrow().clone();
    let popover = existing.unwrap_or_else(|| {
        let popover = gtk::PopoverMenu::from_model(Some(model));
        popover.set_has_arrow(false);
        popover.set_halign(gtk::Align::Start);
        popover.set_parent(widget);
        let weak = Rc::downgrade(slot);
        popover.connect_closed(move |popover| {
            let (popover, slot) = (popover.clone(), weak.clone());
            glib::idle_add_local_once(move || {
                // Shown again meanwhile, or already gone with its widget.
                if popover.is_visible() || popover.parent().is_none() {
                    return;
                }
                popover.unparent();
                if let Some(slot) = slot.upgrade() {
                    slot.borrow_mut().take_if(|p| *p == popover);
                }
            });
        });
        *slot.borrow_mut() = Some(popover.clone());
        popover
    });
    let rect = at.map(|(x, y)| gdk::Rectangle::new(x as i32, y as i32, 1, 1));
    popover.set_pointing_to(rect.as_ref());
    popover.popup();
}

/// A shortcut from GTK's notation, e.g. `<Control><Shift>n`.
fn parse_trigger(accel: &str) -> Option<Shortcut> {
    let (key, modifiers) = gtk::accelerator_parse(accel)?;
    Some(Shortcut {
        key: key.to_unicode()?,
        primary: modifiers.contains(gdk::ModifierType::CONTROL_MASK),
        shift: modifiers.contains(gdk::ModifierType::SHIFT_MASK),
        alt: modifiers.contains(gdk::ModifierType::ALT_MASK),
    })
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
        if let Some(folder) = existing_folder(&request.start_folder) {
            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
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
        if let Some(folder) = existing_folder(&request.start_folder) {
            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
        }
        let window = dialog_parent(&self.backend, parent);
        dialog
            .save(window.as_ref(), None::<&gio::Cancellable>, move |result| reply(result.ok().and_then(|f| path(&f))));
    }

    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        self.backend.set_menu(window, menu, activate);
    }
}
