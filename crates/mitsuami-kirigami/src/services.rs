//! Clipboard, dialogs, the trash, launching and menus on KDE.
//!
//! Kirigami apps put their menus in a global drawer shown as a menu
//! (`isMenu`): a hamburger button in the page toolbar, one submenu per app
//! menu, then Settings, About and Quit, as KDE apps end theirs. Each
//! window's drawer holds the app's menus and the window's own. Shortcuts
//! work in every window that has them. Qt's text fields bring
//! their own Cut/Copy/Paste context menus, so there is no Edit menu.
//! Alerts are `Kirigami.PromptDialog`s; file dialogs are Qt Quick's, which
//! Plasma replaces with its own through its platform theme.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::l10n::tr;
use mitsuami_core::services::{
    Alert, AlertStyle, FileFilter, Launch, MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData, MenuRole,
    OpenFile, Reply, SaveFile, ServiceError, Services, Shortcut, existing_folder,
};
use mitsuami_core::{Key, NodeId};

use crate::backend::{KirigamiHandle, WindowRoot, dialog_parent};
use crate::ffi::{self, QmlObject, js_string};

/// The menus the app installed: the app's, and each window's own.
#[derive(Default)]
pub(crate) struct Menus {
    app: MenuBarData,
    windows: HashMap<NodeId, MenuBarData>,
    /// Dialogs, which show only their own menus.
    pub(crate) modal: HashSet<NodeId>,
    /// Set with the first menus: a window gets a drawer only once there are.
    pub(crate) wiring: Option<Wiring>,
}

impl Menus {
    /// What a window's drawer shows.
    pub(crate) fn of(&self, window: NodeId) -> MenuBarData {
        self.app.for_window(self.windows.get(&window), self.modal.contains(&window))
    }

    /// Forgets a window's menus, when it's destroyed.
    pub(crate) fn forget(&mut self, window: NodeId) {
        self.windows.remove(&window);
        self.modal.remove(&window);
    }
}

/// What the drawers' actions do: run the app's items, or quit.
#[derive(Clone)]
pub(crate) struct Wiring {
    activate: Rc<dyn Fn(u32)>,
    quit: Rc<dyn Fn()>,
}

impl Wiring {
    /// Shows `menu` in a window: in place when only enabled and checked
    /// states changed, as rebuilding the drawer would close it if open.
    fn update(&self, root: &Rc<WindowRoot>, menu: MenuBarData, dialog: bool) {
        let drawer = root.drawer.get();
        let same = drawer.is_some() && root.menu.borrow().same_structure(&menu);
        let empty = drawer.is_none() && menu.menus.is_empty();
        root.menu.replace(menu);
        match drawer {
            Some(drawer) if same => apply_states(drawer, &root.menu.borrow()),
            _ if empty => {}
            _ => self.install(root, dialog),
        }
    }

    /// Puts the window's menus in an existing window: a global drawer of
    /// its own. Kirigami's hamburger button warns about a binding loop when
    /// a drawer or its actions arrive after the window is complete, so
    /// windows get their drawer inline when they're created (see
    /// [`drawer_qml`]); this is only for menus whose structure changes
    /// later.
    fn install(&self, root: &Rc<WindowRoot>, dialog: bool) {
        if let Some(old) = root.drawer.take() {
            root.window.set_object("globalDrawer", None);
            old.destroy();
        }
        let Some(qml) = drawer_qml(&root.menu.borrow(), dialog) else { return };
        let Some(overlay) = root.window.object("overlay") else { return };
        let drawer = QmlObject::load_in(&qml, overlay);
        root.window.set_object("globalDrawer", Some(drawer));
        root.drawer.set(Some(drawer));
        self.connect(root);
    }

    /// Makes the window's drawer's actions run the items. Qt checks a
    /// checkable action itself when it's triggered: the drawer goes back to
    /// what the app shows, and the app's state comes back if it changes.
    pub(crate) fn connect(&self, root: &Rc<WindowRoot>) {
        let Some(drawer) = root.drawer.get() else { return };
        let menu = root.menu.borrow();
        for item in menu.items() {
            if let Some(action) = drawer.child(&item_name(item.id)) {
                let (activate, id, weak) = (self.activate.clone(), item.id, Rc::downgrade(root));
                action.connect("triggered(QObject*)", move || {
                    if let Some(root) = weak.upgrade()
                        && let Some(drawer) = root.drawer.get()
                    {
                        apply_states(drawer, &root.menu.borrow());
                    }
                    activate(id);
                });
            }
        }
        if let Some(quit) = drawer.child("mitsuamiQuit") {
            let request = self.quit.clone();
            quit.connect("triggered(QObject*)", move || request());
        }
    }
}

/// Sets enabled and checked states in place, in the actions under `root`.
fn apply_states(root: QmlObject, menu: &MenuBarData) {
    for item in menu.items() {
        let Some(action) = root.child(&item_name(item.id)) else { continue };
        action.set_bool("enabled", item.enabled);
        if let MenuCheck::Check(on) | MenuCheck::Radio(on) = item.check {
            action.set_bool("checked", on);
        }
    }
}

fn item_name(id: u32) -> String {
    format!("mitsuamiItem{id}")
}

/// `Ctrl+Shift+N`, the portable notation of `QKeySequence`.
fn sequence(shortcut: &Shortcut) -> String {
    let mut keys = String::new();
    if shortcut.primary {
        keys.push_str("Ctrl+");
    }
    if shortcut.shift {
        keys.push_str("Shift+");
    }
    if shortcut.alt {
        keys.push_str("Alt+");
    }
    match shortcut.key {
        Key::Char(' ') => keys.push_str("Space"),
        Key::Char(c) => keys.push(c.to_ascii_uppercase()),
        Key::Enter => keys.push_str("Return"),
        Key::Escape => keys.push_str("Esc"),
        Key::Tab => keys.push_str("Tab"),
        Key::Backspace => keys.push_str("Backspace"),
        Key::Delete => keys.push_str("Del"),
        Key::Up => keys.push_str("Up"),
        Key::Down => keys.push_str("Down"),
        Key::Left => keys.push_str("Left"),
        Key::Right => keys.push_str("Right"),
        Key::Home => keys.push_str("Home"),
        Key::End => keys.push_str("End"),
        Key::PageUp => keys.push_str("PgUp"),
        Key::PageDown => keys.push_str("PgDown"),
        Key::F(n) => _ = write!(keys, "F{n}"),
    }
    keys
}

/// Where a menu's QML goes: the global drawer, whose submenus and
/// separators are Kirigami actions too, or a context menu (`QQC2.Menu`).
#[derive(Clone, Copy, PartialEq)]
enum Form {
    Drawer,
    ContextMenu,
}

/// One item. Radio items are in their group's `ActionGroup`, which is
/// exclusive, so Qt draws them as one choice.
///
/// An action's shortcut works wherever its menu is, open or not, as a menu
/// bar's do. A context menu's only works while it's open, as on the other
/// platforms: otherwise rows with the same menu would make one another's
/// shortcut ambiguous, and so would the menu bar item it repeats. Closed,
/// it has none (`mitsuamiMenu` is the menu, see [`context_menu_qml`]).
fn item_qml(
    item: &MenuItemData,
    shortcut: Option<Shortcut>,
    icon: Option<&str>,
    group: Option<u32>,
    form: Form,
) -> String {
    let mut qml = format!(
        "Kirigami.Action {{ objectName: {}; text: {}; enabled: {}",
        js_string(&item_name(item.id)),
        js_string(&item.title),
        item.enabled
    );
    if let Some(shortcut) = shortcut {
        let keys = js_string(&sequence(&shortcut));
        match form {
            Form::Drawer => _ = write!(qml, "; shortcut: {keys}"),
            Form::ContextMenu => _ = write!(qml, "; shortcut: mitsuamiMenu.visible ? {keys} : undefined"),
        }
    }
    if let Some(icon) = icon {
        _ = write!(qml, "; icon.name: {}", js_string(icon));
    }
    if let MenuCheck::Check(on) | MenuCheck::Radio(on) = item.check {
        _ = write!(qml, "; checkable: true; checked: {on}");
    }
    if let Some(group) = group {
        _ = write!(qml, "; QQC2.ActionGroup.group: mitsuamiGroup{group}");
    }
    qml.push_str(" }");
    qml
}

/// A menu's entries. `groups` collects the radio groups' `ActionGroup`s.
fn entries_qml(menu: &MenuData, groups: &mut Vec<String>, form: Form) -> String {
    let entries: Vec<String> = menu
        .entries
        .iter()
        .zip(menu.radio_groups())
        .map(|(entry, group)| match entry {
            MenuEntry::Item(item) => {
                if group == Some(item.id) {
                    groups.push(format!("QQC2.ActionGroup {{ id: mitsuamiGroup{} }}", item.id));
                }
                item_qml(item, item.shortcut, None, group, form)
            }
            MenuEntry::Submenu(submenu) => menu_qml(submenu, groups, form),
            MenuEntry::Separator if form == Form::Drawer => "Kirigami.Action { separator: true }".into(),
            MenuEntry::Separator => "QQC2.MenuSeparator { }".into(),
        })
        .collect();
    entries.join("\n")
}

/// A menu, or a submenu: in the drawer an action whose children are its
/// entries, in a context menu a `QQC2.Menu`.
fn menu_qml(menu: &MenuData, groups: &mut Vec<String>, form: Form) -> String {
    let entries = entries_qml(menu, groups, form);
    let kind = if form == Form::Drawer { "Kirigami.Action { text" } else { "QQC2.Menu { title" };
    format!("{kind}: {}\n{entries}\n}}", js_string(&menu.title))
}

/// The global drawer's QML for a window's menus, or none without menus.
/// Items with a role end the drawer, as in KDE apps: Settings (with KDE's
/// Ctrl+Shift+, unless it has a shortcut), About, then Quit. The app's
/// Quit item replaces ours, with our title and shortcut; a dialog has none
/// of ours, only its own menus.
pub(crate) fn drawer_qml(menu: &MenuBarData, dialog: bool) -> Option<String> {
    if menu.menus.is_empty() {
        return None;
    }
    let mut menu = menu.clone();
    let settings = menu.take_role(MenuRole::Settings);
    let about = menu.take_role(MenuRole::About);
    let quit = menu.take_role(MenuRole::Quit);
    let mut groups = Vec::new();
    let mut actions: Vec<String> = menu.menus.iter().map(|m| menu_qml(m, &mut groups, Form::Drawer)).collect();
    if let Some(item) = settings {
        let shortcut = item.shortcut.unwrap_or(Shortcut::primary(',').shift());
        actions.push(item_qml(&item, Some(shortcut), Some("settings-configure"), None, Form::Drawer));
    }
    if let Some(item) = about {
        actions.push(item_qml(&item, item.shortcut, Some("help-about"), None, Form::Drawer));
    }
    // Plasma's binding; `StandardKey.Quit` maps to several, which Qt's
    // shortcuts warn about.
    match quit {
        Some(item) => {
            let item = MenuItemData { title: tr("mitsuami-menu-quit", &[]), ..item };
            actions.push(item_qml(&item, Some(Shortcut::primary('q')), Some("application-exit"), None, Form::Drawer));
        }
        None if dialog => {}
        None => actions.push(format!(
            "Kirigami.Action {{ objectName: \"mitsuamiQuit\"; text: {}; icon.name: \"application-exit\"; \
             shortcut: \"Ctrl+Q\" }}",
            js_string(&tr("mitsuami-menu-quit", &[]))
        )),
    }
    Some(format!(
        "Kirigami.GlobalDrawer {{ isMenu: true\n{}\nactions: [\n{}\n] }}",
        groups.join("\n"),
        actions.join(",\n")
    ))
}

/// A context menu's QML, or none without entries: a `QQC2.Menu`, which the
/// desktop style draws, of the same actions as the drawer's, with
/// `QQC2.MenuSeparator`s and submenus. `mitsuamiOwner` is its item while
/// it's open in the window's overlay (see `qml::CONTEXT_MENU`).
fn context_menu_qml(menu: &MenuData) -> Option<String> {
    if menu.entries.is_empty() {
        return None;
    }
    let mut groups = Vec::new();
    let entries = entries_qml(menu, &mut groups, Form::ContextMenu);
    Some(format!(
        "QQC2.Menu {{ id: mitsuamiMenu\n\
         property Item mitsuamiOwner: null\n\
         onClosed: if (mitsuamiOwner) {{ parent = mitsuamiOwner; mitsuamiOwner = null }}\n{}\n{entries}\n}}",
        groups.join("\n")
    ))
}

/// A node's context menu: the app's entries (as the one menu of a bar, for
/// [`MenuBarData`]'s helpers), and the `QQC2.Menu` that shows them. Its
/// parent is the node's item, whose `mitsuamiContextMenu` it is (see
/// `qml::CONTEXT_MENU`). A context menu is made when it's first wanted
/// (`mitsuamiContextMenuWanted()`, as it's about to open, or an item
/// chosen from code): a table's rows each have one per cell, and making a
/// `QQC2.Menu` and its items took most of the time rows took to scroll in.
/// A menu button's menu is one too, its `mitsuamiButtonMenu` (see
/// `qml::menu_button`), made as its entries come.
pub(crate) struct ContextMenu {
    entries: Rc<RefCell<MenuBarData>>,
    menu: Rc<Cell<Option<QmlObject>>>,
    /// Reports the item chosen.
    choose: Rc<dyn Fn(u32)>,
    /// The item's property that holds the menu.
    property: &'static str,
    /// Context menus: the item that asks for it, once connected.
    item: Cell<Option<QmlObject>>,
}

const CONTEXT_MENU_PROPERTY: &str = "mitsuamiContextMenu";

impl ContextMenu {
    pub(crate) fn new(choose: impl Fn(u32) + 'static) -> ContextMenu {
        ContextMenu {
            entries: Rc::default(),
            menu: Rc::default(),
            choose: Rc::new(choose),
            property: CONTEXT_MENU_PROPERTY,
            item: Cell::new(None),
        }
    }

    /// A menu button's menu, which a click on it shows.
    pub(crate) fn for_button(choose: impl Fn(u32) + 'static) -> ContextMenu {
        ContextMenu { property: "mitsuamiButtonMenu", ..ContextMenu::new(choose) }
    }

    fn lazy(&self) -> bool {
        self.property == CONTEXT_MENU_PROPERTY
    }

    /// Shows `entries` on `item`, or only keeps them if the item can't
    /// show a menu (`None`). In place when only enabled and checked states
    /// changed, which keeps an open menu open.
    pub(crate) fn set(&mut self, item: Option<QmlObject>, entries: &[MenuEntry]) {
        let data = MenuBarData { menus: vec![MenuData { title: String::new(), entries: entries.to_vec() }] };
        let same = self.menu.get().is_some() && self.entries.borrow().same_structure(&data);
        self.entries.replace(data);
        if let Some(menu) = self.menu.get().filter(|_| same) {
            return apply_states(menu, &self.entries.borrow());
        }
        if let Some(old) = self.menu.take() {
            if let Some(item) = item {
                item.set_object(self.property, None);
            }
            retire(old);
        }
        let Some(item) = item else { return };
        if !self.lazy() {
            self.menu.set(build(item, self.property, &self.entries, &self.choose));
            return;
        }
        item.set_bool("mitsuamiHasContextMenu", !self.entries.borrow().menus[0].entries.is_empty());
        if self.item.replace(Some(item)) != Some(item) {
            let (menu, entries, choose) = (self.menu.clone(), self.entries.clone(), self.choose.clone());
            item.connect("mitsuamiContextMenuWanted()", move || {
                if menu.get().is_none() {
                    menu.set(build(item, CONTEXT_MENU_PROPERTY, &entries, &choose));
                }
            });
        }
    }

    /// The menu, made now if it's wanted and not made yet.
    fn menu(&self) -> Option<QmlObject> {
        if self.menu.get().is_none()
            && let Some(item) = self.item.get()
        {
            self.menu.set(build(item, self.property, &self.entries, &self.choose));
        }
        self.menu.get()
    }

    /// The action of the item with this id, if the menu is shown.
    pub(crate) fn action(&self, id: u32) -> Option<QmlObject> {
        self.menu()?.child(&item_name(id))
    }

    /// The entries as the menu shows them: items' titles, enabled and
    /// checked states from their actions, the rest as the app gave it
    /// (all of it, before the menu is made).
    pub(crate) fn shown(&self) -> Vec<MenuEntry> {
        fn read(menu: QmlObject, entries: &[MenuEntry]) -> Vec<MenuEntry> {
            entries
                .iter()
                .map(|entry| match entry {
                    MenuEntry::Item(item) => {
                        let Some(action) = menu.child(&item_name(item.id)) else { return entry.clone() };
                        let checked = action.bool("checked");
                        MenuEntry::Item(MenuItemData {
                            title: action.str("text"),
                            enabled: action.bool("enabled"),
                            check: match item.check {
                                MenuCheck::None => MenuCheck::None,
                                MenuCheck::Check(_) => MenuCheck::Check(checked),
                                MenuCheck::Radio(_) => MenuCheck::Radio(checked),
                            },
                            ..item.clone()
                        })
                    }
                    MenuEntry::Submenu(submenu) => MenuEntry::Submenu(MenuData {
                        title: submenu.title.clone(),
                        entries: read(menu, &submenu.entries),
                    }),
                    MenuEntry::Separator => MenuEntry::Separator,
                })
                .collect()
        }
        let entries = self.entries.borrow();
        let entries = entries.menus.first().map(|m| m.entries.as_slice()).unwrap_or_default();
        match self.menu.get() {
            Some(menu) => read(menu, entries),
            None => entries.to_vec(),
        }
    }

    /// The menu goes with its node.
    pub(crate) fn delete_later(&self) {
        if let Some(menu) = self.menu.get() {
            menu.delete_later();
        }
    }
}

/// Makes the `QQC2.Menu` for `entries` in `item`, as its `property`, or
/// none without entries.
fn build(
    item: QmlObject,
    property: &str,
    entries: &Rc<RefCell<MenuBarData>>,
    choose: &Rc<dyn Fn(u32)>,
) -> Option<QmlObject> {
    let qml = context_menu_qml(&entries.borrow().menus[0])?;
    let menu = QmlObject::load_in(&qml, item);
    // As in the drawer, Qt checks a checkable action itself when it's
    // triggered: the menu goes back to what the app shows first.
    for entry in entries.borrow().items() {
        let Some(action) = menu.child(&item_name(entry.id)) else { continue };
        let (entries, choose, id) = (entries.clone(), choose.clone(), entry.id);
        action.connect("triggered(QObject*)", move || {
            apply_states(menu, &entries.borrow());
            choose(id);
        });
    }
    item.set_object(property, Some(menu));
    Some(menu)
}

/// A menu that's been replaced: an open one stays until it closes, as an
/// open `NSMenu` does on AppKit, and what's chosen in it still reports its
/// id; a closed one goes.
fn retire(menu: QmlObject) {
    if menu.bool("visible") {
        menu.connect("closed()", move || menu.delete_later());
    } else {
        menu.delete_later();
    }
}

pub struct KirigamiServices {
    backend: KirigamiHandle,
}

impl KirigamiServices {
    pub(crate) fn new(backend: KirigamiHandle) -> KirigamiServices {
        KirigamiServices { backend }
    }
}

/// A reply several signals may answer; the first one wins, and the dialog
/// goes once it's answered.
fn answer_once<T: 'static>(dialog: QmlObject, backend: &KirigamiHandle, reply: Reply<T>) -> Rc<dyn Fn(T)> {
    let reply = Cell::new(Some(reply));
    let backend = backend.weak();
    Rc::new(move |value| {
        if let Some(reply) = reply.take() {
            if let Some(backend) = KirigamiHandle::from_weak(&backend) {
                backend.forget_dialog(dialog);
            }
            dialog.delete_later();
            reply(value);
        }
    })
}

/// `Images (*.png *.jpg)`, as Qt's name filters read; `All files (*)`
/// for every file.
fn name_filters(filters: &[FileFilter]) -> Vec<String> {
    filters
        .iter()
        .map(|f| {
            let mut patterns: Vec<String> =
                f.extensions.iter().map(|e| format!("*.{}", e.trim_start_matches('.'))).collect();
            if f.is_all() {
                patterns.push("*".into());
            }
            format!("{} ({})", f.name, patterns.join(" "))
        })
        .collect()
}

impl KirigamiServices {
    fn open_dialog(&self, dialog: QmlObject, parent: Option<NodeId>) {
        if let Some(root) = dialog_parent(&self.backend, parent) {
            dialog.set_object("parentWindow", Some(root.window));
        }
        self.backend.remember_dialog(dialog);
        dialog.invoke("open");
    }
}

impl Services for KirigamiServices {
    /// Qt's clipboard answers right away.
    fn clipboard_text(&mut self, reply: Reply<Option<String>>) {
        reply(ffi::clipboard_text());
    }

    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>) {
        ffi::set_clipboard_text(text);
        reply(Ok(()));
    }

    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>) {
        let Some(root) = dialog_parent(&self.backend, parent) else {
            // No window to show it in: the least committal answer.
            return reply(alert.effective_buttons().len() - 1);
        };
        let buttons = alert.effective_buttons();
        let actions: Vec<String> = buttons
            .iter()
            .enumerate()
            .map(|(i, label)| {
                format!("Kirigami.Action {{ objectName: \"mitsuamiButton{i}\"; text: {} }}", js_string(label))
            })
            .collect();
        let dialog_type = match alert.style {
            AlertStyle::Info => "None",
            AlertStyle::Warning => "Warning",
            AlertStyle::Critical => "Error",
        };
        let qml = format!(
            "Kirigami.PromptDialog {{\n\
             dialogType: Kirigami.PromptDialog.{dialog_type}\n\
             // Kirigami's, kept in the window and read from the implicit\n\
             // height, which Qt places the popup by. Setting y lets Qt\n\
             // resize a popup that doesn't fit, and a binding that reads\n\
             // height then loops.\n\
             y: parent ? Math.max(0, Math.min(Math.round((parent.height - implicitHeight) / 2)\n\
                 + Kirigami.Units.gridUnit * 2 * (1 - opacity), parent.height - implicitHeight)) : 0\n\
             standardButtons: Kirigami.Dialog.NoButton\n\
             customFooterActions: [\n{}\n]\n}}",
            actions.join(",\n")
        );
        let Some(overlay) = root.window.object("overlay") else {
            return reply(buttons.len() - 1);
        };
        let dialog = QmlObject::load_in(&qml, overlay);
        dialog.set_str("title", &alert.title);
        dialog.set_str("subtitle", alert.message.as_deref().unwrap_or_default());
        let answer = answer_once(dialog, &self.backend, reply);
        for i in 0..buttons.len() {
            if let Some(action) = dialog.child(&format!("mitsuamiButton{i}")) {
                let answer = answer.clone();
                action.connect("triggered(QObject*)", move || {
                    dialog.invoke("close");
                    answer(i);
                });
            }
        }
        // Escape (or closing the dialog) chooses the last button, which by
        // convention is the least committal one (Cancel).
        let cancel = buttons.len() - 1;
        dialog.connect("rejected()", move || answer(cancel));
        self.backend.remember_dialog(dialog);
        // Opened in a window that hasn't been laid out and drawn yet (an
        // alert as the app starts), Kirigami's dialog loops over its
        // position: it waits for the window's first frame.
        if root.has_rendered() {
            dialog.invoke("open");
        } else {
            let pending = Cell::new(true);
            root.window.connect("frameSwapped()", move || {
                if pending.replace(false) {
                    dialog.invoke("open");
                }
            });
        }
    }

    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>) {
        let dialog = if request.directories {
            // Qt Quick's folder dialog picks one folder.
            QmlObject::load("import QtQuick.Dialogs\nFolderDialog { }")
        } else {
            let mode = if request.multiple { "OpenFiles" } else { "OpenFile" };
            QmlObject::load(&format!("import QtQuick.Dialogs\nFileDialog {{ fileMode: FileDialog.{mode} }}"))
        };
        if let Some(title) = &request.title {
            dialog.set_str("title", title);
        }
        if !request.directories && !request.filters.is_empty() {
            dialog.set_str_list("nameFilters", &name_filters(&request.filters));
        }
        if let Some(folder) = existing_folder(&request.start_folder) {
            dialog.set_url("currentFolder", folder);
        }
        let answer = answer_once(dialog, &self.backend, reply);
        let accepted = answer.clone();
        let property = if request.directories { "selectedFolder" } else { "selectedFiles" };
        dialog.connect("accepted()", move || accepted(Some(dialog.paths(property)).filter(|p| !p.is_empty())));
        dialog.connect("rejected()", move || answer(None));
        self.open_dialog(dialog, parent);
    }

    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>) {
        let dialog = QmlObject::load("import QtQuick.Dialogs\nFileDialog { fileMode: FileDialog.SaveFile }");
        if let Some(title) = &request.title {
            dialog.set_str("title", title);
        }
        if !request.filters.is_empty() {
            dialog.set_str_list("nameFilters", &name_filters(&request.filters));
        }
        let folder = existing_folder(&request.start_folder);
        if let Some(name) = &request.default_name {
            // The name goes in as a file in the folder; with no folder
            // asked for, the home folder, as Qt's own dialogs start there.
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
            dialog.set_url("selectedFile", &folder.unwrap_or(&home).join(name));
        } else if let Some(folder) = folder {
            dialog.set_url("currentFolder", folder);
        }
        let answer = answer_once(dialog, &self.backend, reply);
        let accepted = answer.clone();
        dialog.connect("accepted()", move || accepted(dialog.paths("selectedFile").into_iter().next()));
        dialog.connect("rejected()", move || answer(None));
        self.open_dialog(dialog, parent);
    }

    /// Qt's trash (`QFile::moveToTrash`) is the freedesktop.org one KIO
    /// uses, so Dolphin restores from it. It moves synchronously, and
    /// doesn't say when a disk has no trash, so every failure is `Failed`.
    fn trash(&mut self, _parent: Option<NodeId>, paths: &[PathBuf], reply: Reply<Result<(), ServiceError>>) {
        reply(paths.iter().try_for_each(|path| ffi::trash(path).map_err(ServiceError::Failed)));
    }

    /// `QDesktopServices::openUrl`: kde-open on Plasma, the portal in a
    /// sandbox. Qt only says whether it worked, and what fails is nearly
    /// always that no app opens it, so a failure is `Unavailable`.
    fn launch(&mut self, _parent: Option<NodeId>, target: &Launch, reply: Reply<Result<(), ServiceError>>) {
        let opened = match target {
            Launch::Path(path) => ffi::open_url(&path.to_string_lossy(), true),
            Launch::Url(url) => ffi::open_url(url, false),
        };
        reply(if opened { Ok(()) } else { Err(ServiceError::Unavailable) });
    }

    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let backend = self.backend.weak();
        let quit: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(backend) = KirigamiHandle::from_weak(&backend) {
                backend.request_quit();
            }
        });
        let wiring = Wiring { activate, quit };
        self.backend.with_menus(|menus| {
            menus.wiring = Some(wiring.clone());
            match window {
                None => menus.app = menu.clone(),
                Some(window) if menu.menus.is_empty() => menus.forget(window),
                Some(window) => _ = menus.windows.insert(window, menu.clone()),
            }
        });
        // A window not created yet gets its drawer with it.
        for (id, root) in self.backend.windows() {
            if window.is_none_or(|w| w == id) {
                let (shown, dialog) = self.backend.with_menus(|menus| (menus.of(id), menus.modal.contains(&id)));
                wiring.update(&root, shown, dialog);
            }
        }
    }
}

thread_local! {
    /// Dialogs the services opened and haven't answered yet.
    static DIALOGS: RefCell<Vec<QmlObject>> = const { RefCell::new(Vec::new()) };
}

impl KirigamiHandle {
    fn remember_dialog(&self, dialog: QmlObject) {
        DIALOGS.with(|d| d.borrow_mut().push(dialog));
    }

    fn forget_dialog(&self, dialog: QmlObject) {
        DIALOGS.with(|d| d.borrow_mut().retain(|o| *o != dialog));
    }

    /// Alerts and file dialogs that are open, oldest first: an escape hatch
    /// for tests that answer them.
    pub fn open_dialogs(&self) -> Vec<QmlObject> {
        DIALOGS.with(|d| d.borrow().clone())
    }
}
