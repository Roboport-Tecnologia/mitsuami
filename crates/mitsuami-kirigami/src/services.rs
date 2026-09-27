//! Clipboard, dialogs and menus on KDE.
//!
//! Kirigami apps put their menus in a global drawer shown as a menu
//! (`isMenu`): a hamburger button in the page toolbar, one submenu per app
//! menu, then Quit. Shortcuts work in every window. Qt's text fields bring
//! their own Cut/Copy/Paste context menus, so there is no Edit menu.
//! Alerts are `Kirigami.PromptDialog`s; file dialogs are Qt Quick's, which
//! Plasma replaces with its own through its platform theme.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::NodeId;
use mitsuami_core::services::{
    Alert, AlertStyle, FileFilter, MenuBarData, MenuEntry, OpenFile, Reply, SaveFile, ServiceError, Services, Shortcut,
};

use crate::backend::{KirigamiHandle, WindowRoot, dialog_parent};
use crate::ffi::{self, QmlObject, js_string};

/// The app's menu, as QML for a global drawer, and what its items do.
pub(crate) struct MenuParts {
    qml: String,
    activate: Rc<dyn Fn(u32)>,
    items: Vec<u32>,
    has_app_menus: bool,
    backend: KirigamiHandle,
}

impl MenuParts {
    /// The global drawer's QML, for windows created while this menu is
    /// installed: they get it inline. A window without app menus gets none.
    pub(crate) fn drawer_qml(&self) -> Option<&str> {
        self.has_app_menus.then_some(self.qml.as_str())
    }

    /// Puts the menu in an existing window: a global drawer of its own.
    /// Kirigami's hamburger button warns about a binding loop when a drawer
    /// or its actions arrive after the window is complete, so windows get
    /// their drawer inline when they're created (see [`MenuParts::drawer_qml`]);
    /// this is only for a menu whose structure changes later.
    pub(crate) fn install(&self, root: &WindowRoot) {
        if let Some(old) = root.drawer.take() {
            root.window.set_object("globalDrawer", None);
            old.destroy();
        }
        let Some(qml) = self.drawer_qml() else { return };
        let Some(overlay) = root.window.object("overlay") else { return };
        let drawer = QmlObject::load_in(qml, overlay);
        root.window.set_object("globalDrawer", Some(drawer));
        root.drawer.set(Some(drawer));
        self.connect(root);
    }

    /// Makes the window's drawer's actions run the items.
    pub(crate) fn connect(&self, root: &WindowRoot) {
        let Some(drawer) = root.drawer.get() else { return };
        for id in &self.items {
            if let Some(action) = drawer.child(&item_name(*id)) {
                let (activate, id) = (self.activate.clone(), *id);
                action.connect("triggered(QObject*)", move || activate(id));
            }
        }
        if let Some(quit) = drawer.child("mitsuamiQuit") {
            let backend = self.backend.weak();
            quit.connect("triggered(QObject*)", move || {
                if let Some(backend) = KirigamiHandle::from_weak(&backend) {
                    backend.request_quit();
                }
            });
        }
    }
}

fn item_name(id: u32) -> String {
    format!("mitsuamiItem{id}")
}

/// `Ctrl+Shift+N`, the notation of `QKeySequence`.
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
    keys.push(shortcut.key.to_ascii_uppercase());
    keys
}

fn menu_qml(menu: &MenuBarData) -> (String, Vec<u32>) {
    let mut items = Vec::new();
    let mut menus = Vec::new();
    for app_menu in &menu.menus {
        let mut entries = Vec::new();
        for entry in &app_menu.entries {
            match entry {
                MenuEntry::Item { id, title, shortcut, enabled } => {
                    items.push(*id);
                    let shortcut = shortcut.as_ref().map(|s| format!("; shortcut: {}", js_string(&sequence(s))));
                    entries.push(format!(
                        "Kirigami.Action {{ objectName: {}; text: {}; enabled: {enabled}{} }}",
                        js_string(&item_name(*id)),
                        js_string(title),
                        shortcut.unwrap_or_default()
                    ));
                }
                MenuEntry::Separator => entries.push("Kirigami.Action { separator: true }".into()),
            }
        }
        menus.push(format!("Kirigami.Action {{ text: {}\n{}\n}}", js_string(&app_menu.title), entries.join("\n")));
    }
    menus.push(
        // Plasma's binding; `StandardKey.Quit` maps to several, which Qt's
        // shortcuts warn about.
        "Kirigami.Action { objectName: \"mitsuamiQuit\"; text: \"Quit\"; icon.name: \"application-exit\"; \
         shortcut: \"Ctrl+Q\" }"
            .into(),
    );
    (format!("Kirigami.GlobalDrawer {{ isMenu: true\nactions: [\n{}\n] }}", menus.join(",\n")), items)
}

pub struct KirigamiServices {
    backend: KirigamiHandle,
    /// The last menu installed, to update enabled states in place instead
    /// of rebuilding (which would close an open menu).
    menu: Option<MenuBarData>,
}

impl KirigamiServices {
    pub(crate) fn new(backend: KirigamiHandle) -> KirigamiServices {
        KirigamiServices { backend, menu: None }
    }
}

/// Same menus, items and shortcuts; only enabled states may differ.
fn same_structure(a: &MenuBarData, b: &MenuBarData) -> bool {
    let strip = |m: &MenuBarData| {
        m.menus
            .iter()
            .map(|menu| {
                let entries: Vec<_> = menu
                    .entries
                    .iter()
                    .map(|e| match e {
                        MenuEntry::Item { id, title, shortcut, .. } => Some((*id, title.clone(), *shortcut)),
                        MenuEntry::Separator => None,
                    })
                    .collect();
                (menu.title.clone(), entries)
            })
            .collect::<Vec<_>>()
    };
    strip(a) == strip(b)
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

/// `Images (*.png *.jpg)`, as Qt's name filters read.
fn name_filters(filters: &[FileFilter]) -> Vec<String> {
    filters
        .iter()
        .map(|f| {
            let patterns: Vec<String> =
                f.extensions.iter().map(|e| format!("*.{}", e.trim_start_matches('.'))).collect();
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
             // Kirigami's, kept in the window: its opening slide ends\n\
             // past a short window's bottom, and Qt moving it back in\n\
             // loops the binding.\n\
             y: parent ? Math.min(Math.round((parent.height - height) / 2)\n\
                 + Kirigami.Units.gridUnit * 2 * (1 - opacity), parent.height - height) : 0\n\
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
        if let Some(name) = &request.default_name {
            let folder = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
            dialog.set_url("selectedFile", &folder.join(name));
        }
        let answer = answer_once(dialog, &self.backend, reply);
        let accepted = answer.clone();
        dialog.connect("accepted()", move || accepted(dialog.paths("selectedFile").into_iter().next()));
        dialog.connect("rejected()", move || answer(None));
        self.open_dialog(dialog, parent);
    }

    fn set_menu(&mut self, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let same = self.menu.as_ref().is_some_and(|installed| same_structure(installed, menu))
            && self.backend.with_menu(|_| ()).is_some();
        if same {
            for (_, root) in self.backend.windows() {
                let Some(drawer) = root.drawer.get() else { continue };
                for entry in menu.menus.iter().flat_map(|m| &m.entries) {
                    if let MenuEntry::Item { id, enabled, .. } = entry
                        && let Some(action) = drawer.child(&item_name(*id))
                    {
                        action.set_bool("enabled", *enabled);
                    }
                }
            }
            // Windows opened later build their drawer from this.
            let (qml, _) = menu_qml(menu);
            self.backend.update_menu_qml(qml);
        } else {
            let (qml, items) = menu_qml(menu);
            let parts = MenuParts {
                qml,
                activate,
                items,
                has_app_menus: !menu.menus.is_empty(),
                backend: self.backend.clone(),
            };
            self.backend.set_menu(parts);
        }
        self.menu = Some(menu.clone());
    }
}

impl KirigamiHandle {
    fn update_menu_qml(&self, qml: String) {
        self.with_menu_mut(|menu| menu.qml = qml);
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
