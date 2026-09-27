//! Clipboard, dialogs and the menu bar on macOS.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use block2::RcBlock;
use mitsuami_core::NodeId;
use mitsuami_core::services::{
    Alert, AlertStyle, FileFilter, MenuBarData, MenuCheck, MenuEntry, MenuItemData, MenuRole, OpenFile, Reply,
    SaveFile, ServiceError, Services, Shortcut,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertStyle, NSApplication, NSControlStateValueOff, NSControlStateValueOn,
    NSEventModifierFlags, NSMenu, NSMenuItem, NSModalResponse, NSModalResponseOK, NSOpenPanel, NSPasteboard,
    NSPasteboardTypeString, NSSavePanel, NSWindow, NSWindowDidBecomeMainNotification,
    NSWindowDidResignMainNotification,
};
use objc2_core_foundation::{CFRunLoop, kCFRunLoopCommonModes};
use objc2_foundation::{NSArray, NSNotification, NSNotificationCenter, NSProcessInfo, NSString, NSURL};
use objc2_uniform_type_identifiers::UTType;

use crate::backend::AppKitHandle;

/// The menus the bar shows: the app's, and the main window's own.
struct Menus {
    mtm: MainThreadMarker,
    backend: AppKitHandle,
    app: MenuBarData,
    windows: HashMap<NodeId, MenuBarData>,
    /// The main window, the one the menus act on.
    main: Option<NodeId>,
    activate: Option<Rc<dyn Fn(u32)>>,
}

impl Menus {
    fn shown(&self) -> MenuBarData {
        match self.main.and_then(|main| self.windows.get(&main)) {
            Some(window) => self.app.merged(window),
            None => self.app.clone(),
        }
    }
}

pub(crate) struct MenuIvars {
    menus: Weak<RefCell<Menus>>,
}

define_class!(
    /// Target of the app's own menu items (the item's tag is its id), and
    /// observer of which window is main.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuIvars]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(choose:))]
        fn choose(&self, sender: &NSMenuItem) {
            let activate = self.ivars().menus.upgrade().and_then(|menus| menus.borrow().activate.clone());
            if let Some(activate) = activate {
                activate(sender.tag() as u32);
            }
        }

        #[unsafe(method(windowDidBecomeMain:))]
        fn window_did_become_main(&self, notification: &NSNotification) {
            self.main_changed(notification, true);
        }

        #[unsafe(method(windowDidResignMain:))]
        fn window_did_resign_main(&self, notification: &NSNotification) {
            self.main_changed(notification, false);
        }
    }

    unsafe impl NSObjectProtocol for MenuTarget {}
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker, menus: Weak<RefCell<Menus>>) -> Retained<MenuTarget> {
        let this = MenuTarget::alloc(mtm).set_ivars(MenuIvars { menus });
        unsafe { msg_send![super(this), init] }
    }

    /// A window's menus are in the bar while it's main.
    fn main_changed(&self, notification: &NSNotification, became: bool) {
        let Some(menus) = self.ivars().menus.upgrade() else { return };
        let window = notification.object().and_then(|o| o.downcast::<NSWindow>().ok());
        let node = window.and_then(|w| menus.borrow().backend.window_node(&w));
        let changed = {
            let mut menus = menus.borrow_mut();
            let before = menus.main;
            if became {
                menus.main = node;
            } else if node.is_some() && before == node {
                menus.main = None;
            }
            let has_menus = |id: Option<NodeId>| id.is_some_and(|id| menus.windows.contains_key(&id));
            before != menus.main && (has_menus(before) || has_menus(menus.main))
        };
        if changed {
            self.install(&menus.borrow());
        }
    }

    fn install(&self, menus: &Menus) {
        let bar = menu_bar(menus.mtm, menus.shown(), self);
        NSApplication::sharedApplication(menus.mtm).setMainMenu(Some(&bar));
    }
}

pub struct AppKitServices {
    mtm: MainThreadMarker,
    backend: AppKitHandle,
    pasteboard: Retained<NSPasteboard>,
    menus: Rc<RefCell<Menus>>,
    /// Menu items only hold weak references to their target.
    menu_target: Retained<MenuTarget>,
}

impl Drop for AppKitServices {
    fn drop(&mut self) {
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(&self.menu_target) };
    }
}

fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn path(url: &NSURL) -> Option<PathBuf> {
    url.path().map(|p| PathBuf::from(p.to_string()))
}

fn content_types(filters: &[FileFilter]) -> Option<Retained<NSArray<UTType>>> {
    let types: Vec<Retained<UTType>> = filters
        .iter()
        .flat_map(|f| &f.extensions)
        .filter_map(|ext| UTType::typeWithFilenameExtension(&ns(ext.trim_start_matches('.'))))
        .collect();
    (!types.is_empty()).then(|| NSArray::from_retained_slice(&types))
}

/// Runs `f` on the main run loop soon, outside whatever is calling us now.
fn later(f: impl Fn() + 'static) {
    let block = RcBlock::new(f);
    if let Some(run_loop) = CFRunLoop::main() {
        unsafe { run_loop.perform_block(kCFRunLoopCommonModes.map(|m| &**m), Some(&block)) };
        run_loop.wake_up();
    }
}

/// Wraps a one-shot reply for AppKit's (reusable) completion blocks.
fn once<T>(reply: Reply<T>) -> impl Fn(T) {
    let reply = Cell::new(Some(reply));
    move |value| {
        if let Some(reply) = reply.take() {
            reply(value);
        }
    }
}

impl AppKitServices {
    pub(crate) fn new(mtm: MainThreadMarker, backend: AppKitHandle, private_clipboard: bool) -> AppKitServices {
        let pasteboard = if private_clipboard {
            NSPasteboard::pasteboardWithUniqueName()
        } else {
            NSPasteboard::generalPasteboard()
        };
        let menus = Rc::new(RefCell::new(Menus {
            mtm,
            backend: backend.clone(),
            app: MenuBarData::default(),
            windows: HashMap::new(),
            main: None,
            activate: None,
        }));
        let menu_target = MenuTarget::new(mtm, Rc::downgrade(&menus));
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: the target is removed as an observer when the services go.
        unsafe {
            center.addObserver_selector_name_object(
                &menu_target,
                sel!(windowDidBecomeMain:),
                Some(NSWindowDidBecomeMainNotification),
                None,
            );
            center.addObserver_selector_name_object(
                &menu_target,
                sel!(windowDidResignMain:),
                Some(NSWindowDidResignMainNotification),
                None,
            );
        }
        AppKitServices { mtm, backend, pasteboard, menus, menu_target }
    }

    /// The window a dialog belongs to: the requested one, or the active one.
    fn parent(&self, parent: Option<NodeId>) -> Option<Retained<NSWindow>> {
        let app = NSApplication::sharedApplication(self.mtm);
        parent.and_then(|id| self.backend.ns_window(id)).or_else(|| app.keyWindow()).or_else(|| app.mainWindow())
    }
}

impl Services for AppKitServices {
    fn clipboard_text(&mut self, reply: Reply<Option<String>>) {
        // NSPasteboard reads synchronously: answer right away.
        reply(self.pasteboard.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string()));
    }

    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>) {
        self.pasteboard.clearContents();
        let written = self.pasteboard.setString_forType(&ns(text), unsafe { NSPasteboardTypeString });
        reply(if written { Ok(()) } else { Err(ServiceError::Failed("the pasteboard refused the text".into())) });
    }

    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>) {
        let ns_alert = NSAlert::new(self.mtm);
        ns_alert.setMessageText(&ns(&alert.title));
        if let Some(message) = &alert.message {
            ns_alert.setInformativeText(&ns(message));
        }
        ns_alert.setAlertStyle(match alert.style {
            AlertStyle::Info => NSAlertStyle::Informational,
            AlertStyle::Warning => NSAlertStyle::Warning,
            AlertStyle::Critical => NSAlertStyle::Critical,
        });
        for button in alert.effective_buttons() {
            ns_alert.addButtonWithTitle(&ns(&button));
        }
        let reply = once(reply);
        let answer = move |response: NSModalResponse| reply((response - NSAlertFirstButtonReturn).max(0) as usize);
        match self.parent(parent) {
            // A sheet on the window it belongs to.
            Some(window) => {
                let done = RcBlock::new(answer);
                ns_alert.beginSheetModalForWindow_completionHandler(&window, Some(&done));
            }
            // No window: an app-modal alert, run once the caller has returned.
            None => later(move || answer(ns_alert.runModal())),
        }
    }

    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>) {
        let panel = NSOpenPanel::openPanel(self.mtm);
        panel.setCanChooseFiles(!request.directories);
        panel.setCanChooseDirectories(request.directories);
        panel.setAllowsMultipleSelection(request.multiple);
        if let Some(title) = &request.title {
            panel.setMessage(Some(&ns(title)));
        }
        if let Some(types) = content_types(&request.filters) {
            panel.setAllowedContentTypes(&types);
        }
        let reply = once(reply);
        let chosen = panel.clone();
        let done = RcBlock::new(move |response: NSModalResponse| {
            reply((response == NSModalResponseOK).then(|| chosen.URLs().iter().filter_map(|u| path(&u)).collect()));
        });
        match self.parent(parent) {
            Some(window) => panel.beginSheetModalForWindow_completionHandler(&window, &done),
            None => panel.beginWithCompletionHandler(&done),
        }
    }

    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>) {
        let panel = NSSavePanel::savePanel(self.mtm);
        if let Some(title) = &request.title {
            panel.setMessage(Some(&ns(title)));
        }
        if let Some(name) = &request.default_name {
            panel.setNameFieldStringValue(&ns(name));
        }
        if let Some(types) = content_types(&request.filters) {
            panel.setAllowedContentTypes(&types);
        }
        let reply = once(reply);
        let chosen = panel.clone();
        let done = RcBlock::new(move |response: NSModalResponse| {
            reply(if response == NSModalResponseOK { chosen.URL().and_then(|u| path(&u)) } else { None });
        });
        match self.parent(parent) {
            Some(window) => panel.beginSheetModalForWindow_completionHandler(&window, &done),
            None => panel.beginWithCompletionHandler(&done),
        }
    }

    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        {
            let mut menus = self.menus.borrow_mut();
            menus.activate = Some(activate);
            match window {
                None => menus.app = menu.clone(),
                Some(window) if menu.menus.is_empty() => _ = menus.windows.remove(&window),
                Some(window) => _ = menus.windows.insert(window, menu.clone()),
            }
            if menus.main.is_none() {
                let main = NSApplication::sharedApplication(self.mtm).mainWindow();
                menus.main = main.and_then(|w| self.backend.window_node(&w));
            }
            // Another window's menus aren't in the bar.
            if window.is_some() && window != menus.main {
                return;
            }
        }
        self.menu_target.install(&self.menus.borrow());
    }
}

fn item(mtm: MainThreadMarker, title: &str, action: Option<Sel>, key: &str) -> Retained<NSMenuItem> {
    unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &ns(title), action, &ns(key)) }
}

/// `auto_enable`: let AppKit enable items by responder chain (standard
/// menus); otherwise the app's `enabled` state decides.
fn menu_of(
    mtm: MainThreadMarker,
    title: &str,
    items: Vec<Retained<NSMenuItem>>,
    auto_enable: bool,
) -> Retained<NSMenuItem> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &ns(title));
    menu.setAutoenablesItems(auto_enable);
    for item in items {
        menu.addItem(&item);
    }
    let holder = item(mtm, title, None, "");
    holder.setSubmenu(Some(&menu));
    holder
}

fn modifiers(shortcut: &Shortcut) -> NSEventModifierFlags {
    let mut mask = NSEventModifierFlags::empty();
    if shortcut.primary {
        mask |= NSEventModifierFlags::Command;
    }
    if shortcut.shift {
        mask |= NSEventModifierFlags::Shift;
    }
    if shortcut.alt {
        mask |= NSEventModifierFlags::Option;
    }
    mask
}

/// One of the app's items. The check mark is AppKit's for radio items
/// too: its menus show a group's choice with one.
fn app_item(
    mtm: MainThreadMarker,
    data: &MenuItemData,
    title: &str,
    shortcut: Option<Shortcut>,
    target: &MenuTarget,
) -> Retained<NSMenuItem> {
    let key = shortcut.map(|s| s.key.to_string()).unwrap_or_default();
    let item = item(mtm, title, Some(sel!(choose:)), &key);
    unsafe { item.setTarget(Some(target as &AnyObject)) };
    item.setTag(data.id as isize);
    item.setEnabled(data.enabled);
    if let Some(shortcut) = shortcut {
        item.setKeyEquivalentModifierMask(modifiers(&shortcut));
    }
    if let MenuCheck::Check(on) | MenuCheck::Radio(on) = data.check {
        item.setState(if on { NSControlStateValueOn } else { NSControlStateValueOff });
    }
    item
}

fn app_items(mtm: MainThreadMarker, entries: &[MenuEntry], target: &MenuTarget) -> Vec<Retained<NSMenuItem>> {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Separator => NSMenuItem::separatorItem(mtm),
            MenuEntry::Item(data) => app_item(mtm, data, &data.title, data.shortcut, target),
            MenuEntry::Submenu(menu) => menu_of(mtm, &menu.title, app_items(mtm, &menu.entries, target), false),
        })
        .collect()
}

/// The app menu, the app's File menu (if any), Edit (what makes ⌘C/⌘V/⌘Z
/// work in text fields), then the app's other menus. Items with a role go
/// to the app menu, with AppKit's titles and shortcuts, as Qt puts them.
fn menu_bar(mtm: MainThreadMarker, mut menus: MenuBarData, target: &MenuTarget) -> Retained<NSMenu> {
    let name = NSProcessInfo::processInfo().processName().to_string();
    let bar = NSMenu::new(mtm);
    let command = |key| Some(Shortcut::primary(key));
    let mut app_menu = Vec::new();
    if let Some(about) = menus.take_role(MenuRole::About) {
        app_menu.push(app_item(mtm, &about, &format!("About {name}"), None, target));
        app_menu.push(NSMenuItem::separatorItem(mtm));
    }
    if let Some(settings) = menus.take_role(MenuRole::Settings) {
        app_menu.push(app_item(mtm, &settings, "Settings…", command(','), target));
        app_menu.push(NSMenuItem::separatorItem(mtm));
    }
    app_menu.push(item(mtm, &format!("Hide {name}"), Some(sel!(hide:)), "h"));
    app_menu.push(NSMenuItem::separatorItem(mtm));
    app_menu.push(match menus.take_role(MenuRole::Quit) {
        Some(quit) => app_item(mtm, &quit, &format!("Quit {name}"), command('q'), target),
        None => item(mtm, &format!("Quit {name}"), Some(sel!(terminate:)), "q"),
    });
    bar.addItem(&menu_of(mtm, &name, app_menu, true));
    let (file, others): (Vec<_>, Vec<_>) = menus.menus.iter().partition(|m| m.title == "File");
    for menu in file {
        bar.addItem(&menu_of(mtm, &menu.title, app_items(mtm, &menu.entries, target), false));
    }
    bar.addItem(&menu_of(
        mtm,
        "Edit",
        vec![
            item(mtm, "Undo", Some(sel!(undo:)), "z"),
            item(mtm, "Redo", Some(sel!(redo:)), "Z"),
            NSMenuItem::separatorItem(mtm),
            item(mtm, "Cut", Some(sel!(cut:)), "x"),
            item(mtm, "Copy", Some(sel!(copy:)), "c"),
            item(mtm, "Paste", Some(sel!(paste:)), "v"),
            item(mtm, "Select All", Some(sel!(selectAll:)), "a"),
        ],
        true,
    ));
    for menu in others {
        bar.addItem(&menu_of(mtm, &menu.title, app_items(mtm, &menu.entries, target), false));
    }
    bar
}
