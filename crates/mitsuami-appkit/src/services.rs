//! Clipboard, dialogs, the trash, launching and the menu bar on macOS.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use block2::RcBlock;
use mitsuami_core::services::{
    Alert, AlertStyle, FileFilter, Launch, MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData, MenuRole,
    OpenFile, Reply, SaveFile, ServiceError, Services, Shortcut, existing_folder, menu_item_by_id,
};
use mitsuami_core::{Key, NodeId};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertStyle, NSApplication, NSControlStateValueOff, NSControlStateValueOn,
    NSEventModifierFlags, NSMenu, NSMenuItem, NSModalResponse, NSModalResponseOK, NSOpenPanel, NSPasteboard,
    NSPasteboardTypeString, NSRunningApplication, NSSavePanel, NSWindow, NSWindowDidBecomeMainNotification,
    NSWindowDidResignMainNotification, NSWorkspace, NSWorkspaceOpenConfiguration,
};
use objc2_core_foundation::{CFRunLoop, kCFRunLoopCommonModes};
use objc2_foundation::{
    NSArray, NSCocoaErrorDomain, NSError, NSFeatureUnsupportedError, NSFileManager, NSNotification,
    NSNotificationCenter, NSOSStatusErrorDomain, NSString, NSURL, NSUserCancelledError,
};
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

    /// A window's menus are in the bar while it's main. AppKit posts
    /// these while a window closes, which the backend does while it applies
    /// commands, its state borrowed: look the window up once that's over.
    fn main_changed(&self, notification: &NSNotification, became: bool) {
        let Some(window) = notification.object().and_then(|o| o.downcast::<NSWindow>().ok()) else { return };
        let target = self.retain();
        later(move || target.main_changed_now(&window, became));
    }

    fn main_changed_now(&self, window: &NSWindow, became: bool) {
        let Some(menus) = self.ivars().menus.upgrade() else { return };
        let node = menus.borrow().backend.window_node(window);
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
        let bar = menu_bar(menus.mtm, menus.shown(), &menus.backend.app_name(), self);
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

/// The panel's start folder: its `directoryURL`, if the folder is there.
fn start_in(panel: &NSSavePanel, folder: &Option<PathBuf>) {
    if let Some(folder) = existing_folder(folder) {
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath_isDirectory(&ns(&folder.to_string_lossy()), true)));
    }
}

fn path(url: &NSURL) -> Option<PathBuf> {
    url.path().map(|p| PathBuf::from(p.to_string()))
}

/// The types a panel allows: every filter's at once, since AppKit's panels
/// have no filter menu. Every file when a filter lets every file through,
/// so a file the types miss can still be chosen, as on the platforms where
/// the user picks that filter.
fn content_types(filters: &[FileFilter]) -> Option<Retained<NSArray<UTType>>> {
    if filters.iter().any(FileFilter::is_all) {
        return None;
    }
    let types: Vec<Retained<UTType>> = filters
        .iter()
        .flat_map(|f| &f.extensions)
        .filter_map(|ext| UTType::typeWithFilenameExtension(&ns(ext.trim_start_matches('.'))))
        .collect();
    (!types.is_empty()).then(|| NSArray::from_retained_slice(&types))
}

/// Moves an item to its disk's trash, where Finder's Put Back finds it.
fn trash_item(path: &Path) -> Result<(), ServiceError> {
    let url = NSURL::fileURLWithPath(&ns(&path.to_string_lossy()));
    NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url, None).map_err(|error| {
        if error.code() == NSFeatureUnsupportedError && error.domain().isEqualToString(unsafe { NSCocoaErrorDomain }) {
            ServiceError::Unavailable
        } else {
            ServiceError::Failed(error.localizedDescription().to_string())
        }
    })
}

thread_local! {
    /// Replies waiting for `NSWorkspace`, whose completion handlers run on
    /// another thread, by ticket.
    static LAUNCHES: RefCell<HashMap<u64, Reply<Result<(), ServiceError>>>> = RefCell::new(HashMap::new());
    static NEXT_LAUNCH: Cell<u64> = const { Cell::new(0) };
}

/// Launch Services' "no application" and "the user cancelled".
const APPLICATION_NOT_FOUND: isize = -10814;
const USER_CANCELED: isize = -128;

fn launch_error(error: &NSError) -> ServiceError {
    let (domain, code) = (error.domain(), error.code());
    let cocoa = domain.isEqualToString(unsafe { NSCocoaErrorDomain });
    let status = domain.isEqualToString(unsafe { NSOSStatusErrorDomain });
    match code {
        APPLICATION_NOT_FOUND if status => ServiceError::Unavailable,
        USER_CANCELED if status => ServiceError::Cancelled,
        code if cocoa && code == NSUserCancelledError => ServiceError::Cancelled,
        _ => ServiceError::Failed(error.localizedDescription().to_string()),
    }
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
        start_in(&panel, &request.start_folder);
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
        start_in(&panel, &request.start_folder);
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

    /// `NSFileManager` trashes synchronously: within a disk it's a move.
    fn trash(&mut self, _parent: Option<NodeId>, paths: &[PathBuf], reply: Reply<Result<(), ServiceError>>) {
        reply(paths.iter().try_for_each(|path| trash_item(path)));
    }

    /// `NSWorkspace` opens it as Finder would, asking which app (or
    /// saying there's none) itself when no app is set.
    fn launch(&mut self, _parent: Option<NodeId>, target: &Launch, reply: Reply<Result<(), ServiceError>>) {
        let url = match target {
            Launch::Path(path) => NSURL::fileURLWithPath(&ns(&path.to_string_lossy())),
            Launch::Url(url) => match NSURL::URLWithString(&ns(url)) {
                Some(url) => url,
                None => return reply(Err(ServiceError::Failed(format!("\u{201C}{url}\u{201D} isn't a URL")))),
            },
        };
        let ticket = NEXT_LAUNCH.replace(NEXT_LAUNCH.get() + 1);
        LAUNCHES.with(|l| l.borrow_mut().insert(ticket, reply));
        let done = RcBlock::new(move |_app: *mut NSRunningApplication, error: *mut NSError| {
            // SAFETY: AppKit passes a valid error or null.
            let result = unsafe { error.as_ref() }.map_or(Ok(()), |error| Err(launch_error(error)));
            dispatch2::DispatchQueue::main().exec_async(move || {
                if let Some(reply) = LAUNCHES.with(|l| l.borrow_mut().remove(&ticket)) {
                    reply(result);
                }
            });
        });
        let configuration = NSWorkspaceOpenConfiguration::configuration();
        NSWorkspace::sharedWorkspace().openURL_configuration_completionHandler(&url, &configuration, Some(&done));
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

/// The character AppKit names a key by, in key equivalents and key
/// events: the key's own for those that type one, a function key's
/// (`NSUpArrowFunctionKey`) for the others.
pub(crate) fn key_character(key: Key) -> char {
    match key {
        Key::Char(c) => c,
        Key::Enter => '\r',
        Key::Escape => '\u{1b}',
        Key::Tab => '\t',
        // `NSBackspaceCharacter`, as Finder's Move to Trash has it.
        Key::Backspace => '\u{8}',
        Key::Delete => '\u{f728}',
        Key::Up => '\u{f700}',
        Key::Down => '\u{f701}',
        Key::Left => '\u{f702}',
        Key::Right => '\u{f703}',
        Key::Home => '\u{f729}',
        Key::End => '\u{f72b}',
        Key::PageUp => '\u{f72c}',
        Key::PageDown => '\u{f72d}',
        Key::F(n) => char::from_u32(0xf703 + u32::from(n)).unwrap_or('\u{f704}'),
    }
}

/// The character a key event of this key carries, as the keyboard sends
/// it: the Delete (⌫) key's is `NSDeleteCharacter`.
pub(crate) fn event_character(key: Key) -> char {
    match key {
        Key::Backspace => '\u{7f}',
        key => key_character(key),
    }
}

/// The key AppKit names by this character (see [`key_character`]).
/// `NSDeleteCharacter` is the Delete (⌫) key's own.
pub(crate) fn key_of(c: char) -> Key {
    match c {
        '\r' | '\u{3}' => Key::Enter,
        '\u{1b}' => Key::Escape,
        '\t' => Key::Tab,
        '\u{8}' | '\u{7f}' => Key::Backspace,
        '\u{f728}' => Key::Delete,
        '\u{f700}' => Key::Up,
        '\u{f701}' => Key::Down,
        '\u{f702}' => Key::Left,
        '\u{f703}' => Key::Right,
        '\u{f729}' => Key::Home,
        '\u{f72b}' => Key::End,
        '\u{f72c}' => Key::PageUp,
        '\u{f72d}' => Key::PageDown,
        '\u{f704}'..='\u{f726}' => Key::F((c as u32 - 0xf703) as u8),
        c => Key::Char(c),
    }
}

pub(crate) fn modifiers(shortcut: &Shortcut) -> NSEventModifierFlags {
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

/// Where menu items send their action: the item's tag is its id.
#[derive(Clone, Copy)]
pub(crate) struct ItemTarget<'a> {
    pub(crate) object: &'a AnyObject,
    pub(crate) action: Sel,
}

impl<'a> From<&'a MenuTarget> for ItemTarget<'a> {
    fn from(target: &'a MenuTarget) -> ItemTarget<'a> {
        ItemTarget { object: target as &AnyObject, action: sel!(choose:) }
    }
}

/// One of the app's items. The check mark is AppKit's for radio items
/// too: its menus show a group's choice with one.
fn app_item(
    mtm: MainThreadMarker,
    data: &MenuItemData,
    title: &str,
    shortcut: Option<Shortcut>,
    target: ItemTarget,
) -> Retained<NSMenuItem> {
    let key = shortcut.map(|s| key_character(s.key).to_string()).unwrap_or_default();
    let item = item(mtm, title, Some(target.action), &key);
    unsafe { item.setTarget(Some(target.object)) };
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

pub(crate) fn app_items(mtm: MainThreadMarker, entries: &[MenuEntry], target: ItemTarget) -> Vec<Retained<NSMenuItem>> {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Separator => NSMenuItem::separatorItem(mtm),
            MenuEntry::Item(data) => app_item(mtm, data, &data.title, data.shortcut, target),
            MenuEntry::Submenu(menu) => menu_of(mtm, &menu.title, app_items(mtm, &menu.entries, target), false),
        })
        .collect()
}

/// A context menu: the app's entries, enabled as the app says.
pub(crate) fn context_menu(mtm: MainThreadMarker, entries: &[MenuEntry], target: ItemTarget) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    for item in app_items(mtm, entries, target) {
        menu.addItem(&item);
    }
    menu
}

/// What a context menu shows, read back from AppKit. It can't tell a
/// radio item from a check item, nor keep a role: those come from `sent`.
pub(crate) fn context_menu_entries(menu: &NSMenu, sent: &[MenuEntry]) -> Vec<MenuEntry> {
    fn read(menu: &NSMenu, sent: &[MenuEntry]) -> Vec<MenuEntry> {
        menu.itemArray()
            .iter()
            .map(|item| {
                if item.isSeparatorItem() {
                    return MenuEntry::Separator;
                }
                if let Some(submenu) = item.submenu() {
                    return MenuEntry::Submenu(MenuData {
                        title: item.title().to_string(),
                        entries: read(&submenu, sent),
                    });
                }
                let id = item.tag() as u32;
                let original = menu_item_by_id(sent, id);
                let on = item.state() == NSControlStateValueOn;
                let key = item.keyEquivalent().to_string();
                let mask = item.keyEquivalentModifierMask();
                MenuEntry::Item(MenuItemData {
                    id,
                    title: item.title().to_string(),
                    shortcut: key.chars().next().map(|key| Shortcut {
                        key: key_of(key),
                        primary: mask.contains(NSEventModifierFlags::Command),
                        shift: mask.contains(NSEventModifierFlags::Shift),
                        alt: mask.contains(NSEventModifierFlags::Option),
                    }),
                    enabled: item.isEnabled(),
                    check: match original.map(|s| s.check) {
                        Some(MenuCheck::Radio(_)) => MenuCheck::Radio(on),
                        Some(MenuCheck::Check(_)) => MenuCheck::Check(on),
                        _ if on => MenuCheck::Check(true),
                        _ => MenuCheck::None,
                    },
                    role: original.map(|s| s.role).unwrap_or_default(),
                })
            })
            .collect()
    }
    read(menu, sent)
}

/// The item of a context menu with this id, submenus' included, and the
/// menu holding it.
pub(crate) fn find_tagged(menu: &NSMenu, id: u32) -> Option<(Retained<NSMenuItem>, Retained<NSMenu>)> {
    menu.itemArray().iter().find_map(|item| match item.submenu() {
        Some(submenu) => find_tagged(&submenu, id),
        None if !item.isSeparatorItem() && item.tag() as u32 == id => Some((item, menu.retain())),
        None => None,
    })
}

/// The app menu, the app's File menu (if any), Edit (what makes ⌘C/⌘V/⌘Z
/// work in text fields), then the app's other menus. Items with a role go
/// to the app menu, with AppKit's titles and shortcuts, as Qt puts them.
fn menu_bar(mtm: MainThreadMarker, mut menus: MenuBarData, name: &str, target: &MenuTarget) -> Retained<NSMenu> {
    let target = ItemTarget::from(target);
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
    bar.addItem(&menu_of(mtm, name, app_menu, true));
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
