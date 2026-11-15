//! The tray icon on Linux, for both backends: a StatusNotifierItem, the
//! spec Plasma's system tray, waybar, swaybar and GNOME's AppIndicator
//! extension show, with its menu as `com.canonical.dbusmenu`. GTK 4 has no
//! tray, and KDE's `KStatusNotifierItem` needs KDE Frameworks, which the
//! Kirigami shim doesn't link, so both backends export the item here, over
//! GIO's D-Bus: GTK runs GLib's loop, and Qt's loop on Linux is GLib's too
//! (its GLib event dispatcher), so the calls come in on either.
//!
//! The item owns `org.kde.StatusNotifierItem-<pid>-1` and registers with
//! `org.kde.StatusNotifierWatcher`, again whenever the watcher comes back
//! (a panel restarting). It says `ItemIsMenu`, so hosts open the menu on a
//! click rather than calling `Activate`. Whether it shows is whether a
//! watcher with a host is there; plain GNOME has none.
//!
//! Plasma's tray and GNOME's AppIndicator extension call
//! `ProvideXdgActivationToken` before they send a menu item's click: the
//! compositor's token for that click, which the backend gets just before
//! the item runs, so a window the item opens or presents can take the
//! focus on Wayland.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gio::prelude::*;
use glib::{Variant, VariantTy};
use mitsuami_core::AppIcon;
use mitsuami_core::services::{MenuCheck, MenuEntry, TrayData};

const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";
const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// Separators and submenus have no ids of their own in the core's menus:
/// theirs are counted from here, above any item's.
const OWN_IDS: i32 = 0x4000_0000;

const XML: &str = r#"<node>
  <interface name="org.kde.StatusNotifierItem">
    <property name="Category" type="s" access="read"/>
    <property name="Id" type="s" access="read"/>
    <property name="Title" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="WindowId" type="i" access="read"/>
    <property name="IconThemePath" type="s" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="IconPixmap" type="a(iiay)" access="read"/>
    <property name="OverlayIconName" type="s" access="read"/>
    <property name="OverlayIconPixmap" type="a(iiay)" access="read"/>
    <property name="AttentionIconName" type="s" access="read"/>
    <property name="AttentionIconPixmap" type="a(iiay)" access="read"/>
    <property name="AttentionMovieName" type="s" access="read"/>
    <property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
    <property name="ItemIsMenu" type="b" access="read"/>
    <property name="Menu" type="o" access="read"/>
    <method name="ContextMenu"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="Activate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="SecondaryActivate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="Scroll"><arg name="delta" type="i" direction="in"/><arg name="orientation" type="s" direction="in"/></method>
    <method name="ProvideXdgActivationToken"><arg name="token" type="s" direction="in"/></method>
    <signal name="NewTitle"/>
    <signal name="NewIcon"/>
    <signal name="NewAttentionIcon"/>
    <signal name="NewOverlayIcon"/>
    <signal name="NewToolTip"/>
    <signal name="NewStatus"><arg name="status" type="s"/></signal>
  </interface>
  <interface name="com.canonical.dbusmenu">
    <property name="Version" type="u" access="read"/>
    <property name="TextDirection" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="IconThemePath" type="as" access="read"/>
    <method name="GetLayout">
      <arg name="parentId" type="i" direction="in"/>
      <arg name="recursionDepth" type="i" direction="in"/>
      <arg name="propertyNames" type="as" direction="in"/>
      <arg name="revision" type="u" direction="out"/>
      <arg name="layout" type="(ia{sv}av)" direction="out"/>
    </method>
    <method name="GetGroupProperties">
      <arg name="ids" type="ai" direction="in"/>
      <arg name="propertyNames" type="as" direction="in"/>
      <arg name="properties" type="a(ia{sv})" direction="out"/>
    </method>
    <method name="GetProperty">
      <arg name="id" type="i" direction="in"/>
      <arg name="name" type="s" direction="in"/>
      <arg name="value" type="v" direction="out"/>
    </method>
    <method name="Event">
      <arg name="id" type="i" direction="in"/>
      <arg name="eventId" type="s" direction="in"/>
      <arg name="data" type="v" direction="in"/>
      <arg name="timestamp" type="u" direction="in"/>
    </method>
    <method name="EventGroup">
      <arg name="events" type="a(isvu)" direction="in"/>
      <arg name="idErrors" type="ai" direction="out"/>
    </method>
    <method name="AboutToShow">
      <arg name="id" type="i" direction="in"/>
      <arg name="needUpdate" type="b" direction="out"/>
    </method>
    <method name="AboutToShowGroup">
      <arg name="ids" type="ai" direction="in"/>
      <arg name="updatesNeeded" type="ai" direction="out"/>
      <arg name="idErrors" type="ai" direction="out"/>
    </method>
    <signal name="ItemsPropertiesUpdated">
      <arg name="updatedProps" type="a(ia{sv})"/>
      <arg name="removedProps" type="a(ias)"/>
    </signal>
    <signal name="LayoutUpdated"><arg name="revision" type="u"/><arg name="parent" type="i"/></signal>
    <signal name="ItemActivationRequested"><arg name="id" type="i"/><arg name="timestamp" type="u"/></signal>
  </interface>
</node>"#;

type Activate = Rc<dyn Fn(u32)>;
type Shown = Rc<dyn Fn(bool)>;
type Token = Rc<dyn Fn(String)>;

/// The app's tray icon: give it to the backend's services and pass
/// `Services::set_tray` on.
pub struct Tray {
    state: Rc<RefCell<State>>,
}

struct State {
    app_id: Option<String>,
    data: Option<TrayData>,
    menu: Menu,
    revision: u32,
    activate: Option<Activate>,
    shown: Option<Shown>,
    showing: bool,
    exported: Option<Exported>,
    /// Whether putting the item on the bus failed (no session bus): it's
    /// tried again once the icon is taken away and set again, not on each
    /// change.
    export_failed: bool,
    /// The icon as a pixmap, made when it changes: hosts read every
    /// property again after each signal, a tooltip's too.
    pixmap: Option<Variant>,
    /// Hands the backend a host's activation token for the click that
    /// follows.
    on_token: Token,
    /// The token the host gave for the next click.
    token: Option<String>,
}

/// What's on the bus while the icon is set.
struct Exported {
    bus: gio::DBusConnection,
    name: String,
    registrations: Vec<gio::RegistrationId>,
    owner: Option<gio::OwnerId>,
    /// Stops following the watcher (gio names its id type twice, so it's
    /// kept in here).
    unwatch: Option<Box<dyn FnOnce()>>,
    hosts: Vec<gio::SignalSubscription>,
}

impl Tray {
    /// `on_token` gets a host's activation token for a menu click, just
    /// before the item runs: the backend brings forward with it the window
    /// the item presents or opens.
    pub fn new(on_token: impl Fn(String) + 'static) -> Tray {
        let state = State {
            app_id: None,
            data: None,
            menu: Menu::default(),
            revision: 0,
            activate: None,
            shown: None,
            showing: false,
            exported: None,
            export_failed: false,
            pixmap: None,
            on_token: Rc::new(on_token),
            token: None,
        };
        Tray { state: Rc::new(RefCell::new(state)) }
    }

    /// The app's id: the icon theme's name for the app's own icon, shown
    /// when the app gives none, and the item's `Id`.
    pub fn set_app_id(&self, id: Option<&str>) {
        self.state.borrow_mut().app_id = id.map(str::to_owned);
    }

    /// `Services::set_tray`: exports the item the first time, sends what
    /// changed afterwards, and takes it off the bus for `None`.
    pub fn set(&self, tray: Option<&TrayData>, activate: Activate, shown: Shown) {
        {
            let mut state = self.state.borrow_mut();
            state.activate = Some(activate);
            state.shown = Some(shown);
        }
        let Some(tray) = tray else {
            let exported = {
                let mut state = self.state.borrow_mut();
                state.data = None;
                state.pixmap = None;
                state.export_failed = false;
                state.exported.take()
            };
            if let Some(exported) = exported {
                exported.remove();
            }
            report(&self.state, false);
            return;
        };
        let previous = self.state.borrow_mut().data.replace(tray.clone());
        if previous.as_ref().is_none_or(|previous| previous.icon != tray.icon) {
            self.state.borrow_mut().pixmap = tray.icon.as_ref().and_then(pixmap);
        }
        let menu = Menu::new(&tray.menu);
        let menu_changed = self.state.borrow().menu != menu;
        if menu_changed {
            let mut state = self.state.borrow_mut();
            state.menu = menu;
            state.revision += 1;
        }
        let Some(previous) = previous.filter(|_| self.state.borrow().exported.is_some()) else {
            if !self.state.borrow().export_failed {
                export(&self.state);
            }
            return;
        };
        let state = self.state.borrow();
        let Some(exported) = &state.exported else { return };
        let emit = |path: &str, interface: &str, signal: &str, args: Option<Variant>| {
            _ = exported.bus.emit_signal(None, path, interface, signal, args.as_ref());
        };
        if previous.title != tray.title {
            emit(ITEM_PATH, ITEM, "NewTitle", None);
        }
        if previous.icon != tray.icon {
            emit(ITEM_PATH, ITEM, "NewIcon", None);
        }
        if previous.tooltip != tray.tooltip || previous.title != tray.title {
            emit(ITEM_PATH, ITEM, "NewToolTip", None);
        }
        if menu_changed {
            emit(MENU_PATH, MENU, "LayoutUpdated", Some((state.revision, 0i32).to_variant()));
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        if let Some(exported) = self.state.borrow_mut().exported.take() {
            exported.remove();
        }
    }
}

impl Exported {
    fn remove(self) {
        for registration in self.registrations {
            _ = self.bus.unregister_object(registration);
        }
        if let Some(unwatch) = self.unwatch {
            unwatch();
        }
        if let Some(owner) = self.owner {
            gio::bus_unown_name(owner);
        }
        drop(self.hosts);
    }
}

/// Tells the app whether the icon shows, when that changes.
fn report(state: &Rc<RefCell<State>>, showing: bool) {
    let shown = {
        let mut state = state.borrow_mut();
        if state.showing == showing {
            return;
        }
        state.showing = showing;
        state.shown.clone()
    };
    if let Some(shown) = shown {
        shown(showing);
    }
}

fn export(state: &Rc<RefCell<State>>) {
    let bus = match gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        Ok(bus) => bus,
        Err(_) => {
            state.borrow_mut().export_failed = true;
            return report(state, false);
        }
    };
    let node = gio::DBusNodeInfo::for_xml(XML).expect("the tray's D-Bus interfaces");
    let mut registrations = Vec::new();
    let weak = Rc::downgrade(state);
    let item = bus
        .register_object(ITEM_PATH, &node.lookup_interface(ITEM).expect("the item's interface"))
        .method_call({
            let weak = weak.clone();
            move |_, _, _, _, method, args, invocation| {
                if method == "ProvideXdgActivationToken" {
                    let token = args.child_value(0).get::<String>().filter(|token| !token.is_empty());
                    if let Some(state) = weak.upgrade() {
                        state.borrow_mut().token = token;
                    }
                }
                invocation.return_value(None)
            }
        })
        .property({
            let weak = weak.clone();
            move |_, _, _, _, name| item_property(&weak, name)
        })
        .build();
    let menu = bus
        .register_object(MENU_PATH, &node.lookup_interface(MENU).expect("the menu's interface"))
        .method_call({
            let weak = weak.clone();
            move |_, _, _, _, method, args, invocation| menu_call(&weak, method, &args, invocation)
        })
        .property(|_, _, _, _, name| match name {
            "Version" => 3u32.to_variant(),
            "TextDirection" => "ltr".to_variant(),
            "Status" => "normal".to_variant(),
            _ => Vec::<String>::new().to_variant(),
        })
        .build();
    registrations.extend(item.ok());
    registrations.extend(menu.ok());
    let name = format!("{ITEM}-{}-1", std::process::id());
    state.borrow_mut().exported = Some(Exported {
        bus: bus.clone(),
        name: name.clone(),
        registrations,
        owner: None,
        unwatch: None,
        hosts: Vec::new(),
    });
    // Registered with the watcher once the name is ours, so the watcher
    // finds it there. The name has the pid, which isn't unique across PID
    // namespaces sharing the bus (each Flatpak app is pid 2), and a sandbox
    // may not let the app own it: then the item registers by the
    // connection's unique name, as watchers allow (libayatana-appindicator
    // does the same).
    let owner = gio::bus_own_name_on_connection(
        &bus,
        &name,
        gio::BusNameOwnerFlags::DO_NOT_QUEUE,
        {
            let weak = weak.clone();
            move |bus, _| watch(&weak, &bus)
        },
        {
            let weak = weak.clone();
            move |bus, _| {
                let Some(state) = weak.upgrade() else { return };
                let Some(unique) = bus.unique_name() else { return };
                let first = {
                    let mut state = state.borrow_mut();
                    let Some(exported) = state.exported.as_mut() else { return };
                    exported.name = unique.to_string();
                    exported.unwatch.is_none()
                };
                if first {
                    watch(&weak, &bus);
                }
            }
        },
    );
    if let Some(exported) = state.borrow_mut().exported.as_mut() {
        exported.owner = Some(owner);
    }
}

/// Follows the watcher: registers with it each time it appears, and asks
/// whether a host shows items, again whenever one comes or goes.
fn watch(weak: &Weak<RefCell<State>>, bus: &gio::DBusConnection) {
    let Some(state) = weak.upgrade() else { return };
    let appeared = {
        let weak = weak.clone();
        move |bus: gio::DBusConnection, _: &str, _: &str| {
            let Some(state) = weak.upgrade() else { return };
            let Some(name) = state.borrow().exported.as_ref().map(|e| e.name.clone()) else { return };
            let weak = weak.clone();
            bus.call(
                Some(WATCHER),
                WATCHER_PATH,
                WATCHER,
                "RegisterStatusNotifierItem",
                Some(&(name,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                -1,
                None::<&gio::Cancellable>,
                move |registered| match registered {
                    Ok(_) => ask_hosts(&weak),
                    Err(_) => {
                        if let Some(state) = weak.upgrade() {
                            report(&state, false);
                        }
                    }
                },
            );
        }
    };
    let vanished = {
        let weak = weak.clone();
        move |_: gio::DBusConnection, _: &str| {
            if let Some(state) = weak.upgrade() {
                report(&state, false);
            }
        }
    };
    let watcher = gio::bus_watch_name_on_connection(bus, WATCHER, gio::BusNameWatcherFlags::NONE, appeared, vanished);
    let hosts = ["StatusNotifierHostRegistered", "StatusNotifierHostUnregistered"].map(|signal| {
        let weak = weak.clone();
        bus.subscribe_to_signal(
            Some(WATCHER),
            Some(WATCHER),
            Some(signal),
            Some(WATCHER_PATH),
            None,
            gio::DBusSignalFlags::NONE,
            move |_| ask_hosts(&weak),
        )
    });
    if let Some(exported) = state.borrow_mut().exported.as_mut() {
        exported.unwatch = Some(Box::new(move || gio::bus_unwatch_name(watcher)));
        exported.hosts.extend(hosts);
    }
}

/// Reads the watcher's `IsStatusNotifierHostRegistered`: the icon shows
/// while a host does.
fn ask_hosts(weak: &Weak<RefCell<State>>) {
    let Some(state) = weak.upgrade() else { return };
    let Some(bus) = state.borrow().exported.as_ref().map(|e| e.bus.clone()) else { return };
    let weak = weak.clone();
    bus.call(
        Some(WATCHER),
        WATCHER_PATH,
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(&(WATCHER, "IsStatusNotifierHostRegistered").to_variant()),
        Some(VariantTy::new("(v)").unwrap()),
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
        move |reply| {
            let Some(state) = weak.upgrade() else { return };
            let hosts = reply.ok().and_then(|reply| reply.child_value(0).as_variant()).and_then(|v| v.get::<bool>());
            // A watcher that doesn't say is taken at its word that it's there.
            report(&state, state.borrow().data.is_some() && hosts.unwrap_or(true));
        },
    );
}

fn item_property(weak: &Weak<RefCell<State>>, name: &str) -> Variant {
    let Some(state) = weak.upgrade() else { return "".to_variant() };
    let state = state.borrow();
    let data = state.data.as_ref();
    let title = data.map(|d| d.title.clone()).unwrap_or_default();
    let no_pixmaps =
        || Variant::array_from_iter_with_type(VariantTy::new("(iiay)").unwrap(), std::iter::empty::<Variant>());
    match name {
        "Category" => "ApplicationStatus".to_variant(),
        "Id" => state.app_id.clone().unwrap_or_else(|| title.clone()).to_variant(),
        "Title" => title.to_variant(),
        "Status" => "Active".to_variant(),
        "WindowId" => 0i32.to_variant(),
        "IconName" => match data.and_then(|d| d.icon.as_ref()) {
            Some(icon) => icon.name().unwrap_or_default().to_variant(),
            // The app's own icon, which its theme has under its id.
            None => state.app_id.clone().unwrap_or_default().to_variant(),
        },
        "IconPixmap" => match state.pixmap.clone() {
            Some(pixmap) => Variant::array_from_iter_with_type(VariantTy::new("(iiay)").unwrap(), [pixmap]),
            None => no_pixmaps(),
        },
        "OverlayIconPixmap" | "AttentionIconPixmap" => no_pixmaps(),
        "ToolTip" => {
            let tooltip = data.map(|d| d.tooltip.clone()).unwrap_or_default();
            Variant::tuple_from_iter(["".to_variant(), no_pixmaps(), title.to_variant(), tooltip.to_variant()])
        }
        "ItemIsMenu" => true.to_variant(),
        "Menu" => glib::variant::ObjectPath::try_from(MENU_PATH.to_owned()).unwrap().to_variant(),
        _ => "".to_variant(),
    }
}

/// An image as the spec's pixmap: width, height and ARGB32 in network byte
/// order. Only a named icon's name is shown where it has one, as hosts
/// take the theme's icon over a pixmap.
fn pixmap(icon: &AppIcon) -> Option<Variant> {
    if icon.name().is_some_and(|name| !name.is_empty()) {
        return None;
    }
    let bytes = icon.read()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes.as_ref()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let pixels = &buffer[..frame.buffer_size()];
    let argb: Vec<u8> = match frame.color_type {
        png::ColorType::Rgba => pixels.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect(),
        png::ColorType::Rgb => pixels.chunks_exact(3).flat_map(|p| [255, p[0], p[1], p[2]]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.chunks_exact(2).flat_map(|p| [p[1], p[0], p[0], p[0]]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [255, g, g, g]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Variant::tuple_from_iter([
        (frame.width as i32).to_variant(),
        (frame.height as i32).to_variant(),
        Variant::array_from_fixed_array(&argb),
    ]))
}

/// The menu as dbusmenu has it: items by id, the root's (0) and
/// submenus' children in order.
#[derive(Default, PartialEq)]
struct Menu {
    items: HashMap<i32, MenuItem>,
}

#[derive(PartialEq)]
struct MenuItem {
    props: Vec<(&'static str, Prop)>,
    children: Vec<i32>,
    /// What a click activates: the core's id, for items.
    activates: Option<u32>,
}

#[derive(Clone, PartialEq)]
enum Prop {
    Str(String),
    Bool(bool),
    Int(i32),
}

impl Prop {
    fn variant(&self) -> Variant {
        match self {
            Prop::Str(s) => s.to_variant(),
            Prop::Bool(b) => b.to_variant(),
            Prop::Int(i) => i.to_variant(),
        }
    }
}

impl Menu {
    fn new(entries: &[MenuEntry]) -> Menu {
        let mut menu = Menu::default();
        let mut own = OWN_IDS;
        let children = menu.add(entries, &mut own);
        let root = vec![("children-display", Prop::Str("submenu".into()))];
        menu.items.insert(0, MenuItem { props: root, children, activates: None });
        menu
    }

    fn add(&mut self, entries: &[MenuEntry], own: &mut i32) -> Vec<i32> {
        let mut ids = Vec::new();
        for entry in entries {
            let (id, item) = match entry {
                MenuEntry::Item(item) => {
                    let mut props = vec![
                        // dbusmenu takes `_` for a mnemonic.
                        ("label", Prop::Str(item.title.replace('_', "__"))),
                        ("enabled", Prop::Bool(item.enabled)),
                    ];
                    match item.check {
                        MenuCheck::None => {}
                        MenuCheck::Check(on) => props.extend([
                            ("toggle-type", Prop::Str("checkmark".into())),
                            ("toggle-state", Prop::Int(on.into())),
                        ]),
                        MenuCheck::Radio(on) => props.extend([
                            ("toggle-type", Prop::Str("radio".into())),
                            ("toggle-state", Prop::Int(on.into())),
                        ]),
                    }
                    (item.id as i32, MenuItem { props, children: Vec::new(), activates: Some(item.id) })
                }
                MenuEntry::Separator => {
                    *own += 1;
                    let props = vec![("type", Prop::Str("separator".into()))];
                    (*own, MenuItem { props, children: Vec::new(), activates: None })
                }
                MenuEntry::Submenu(submenu) => {
                    *own += 1;
                    let id = *own;
                    let children = self.add(&submenu.entries, own);
                    let props = vec![
                        ("label", Prop::Str(submenu.title.replace('_', "__"))),
                        ("children-display", Prop::Str("submenu".into())),
                    ];
                    (id, MenuItem { props, children, activates: None })
                }
            };
            self.items.insert(id, item);
            ids.push(id);
        }
        ids
    }

    /// An item's properties, those named or all.
    fn props(&self, id: i32, names: &[String]) -> Variant {
        let dict = glib::VariantDict::new(None);
        if let Some(item) = self.items.get(&id) {
            for (name, value) in &item.props {
                if names.is_empty() || names.iter().any(|n| n == name) {
                    dict.insert_value(name, &value.variant());
                }
            }
        }
        dict.end()
    }

    /// `(ia{sv}av)`, with children down to `depth` (-1: all).
    fn layout(&self, id: i32, depth: i32, names: &[String]) -> Variant {
        // Any negative depth is all of it, and `i32::MIN - 1` would overflow.
        let next = if depth < 0 { -1 } else { depth - 1 };
        let children: Vec<Variant> = match self.items.get(&id) {
            Some(item) if depth != 0 => {
                item.children.iter().map(|child| Variant::from_variant(&self.layout(*child, next, names))).collect()
            }
            _ => Vec::new(),
        };
        Variant::tuple_from_iter([
            id.to_variant(),
            self.props(id, names),
            Variant::array_from_iter_with_type(VariantTy::VARIANT, children),
        ])
    }
}

fn menu_call(weak: &Weak<RefCell<State>>, method: &str, args: &Variant, invocation: gio::DBusMethodInvocation) {
    let Some(state) = weak.upgrade() else { return invocation.return_value(None) };
    let reply = match method {
        "GetLayout" => {
            let (parent, depth, names) = args.get::<(i32, i32, Vec<String>)>().unwrap_or_default();
            let state = state.borrow();
            Some(Variant::tuple_from_iter([state.revision.to_variant(), state.menu.layout(parent, depth, &names)]))
        }
        "GetGroupProperties" => {
            let (ids, names) = args.get::<(Vec<i32>, Vec<String>)>().unwrap_or_default();
            let state = state.borrow();
            let ids: Vec<i32> = if ids.is_empty() { state.menu.items.keys().copied().collect() } else { ids };
            // Each item once, however often it's asked for: the reply is
            // built in memory, and any client on the bus may ask.
            let mut seen = std::collections::HashSet::new();
            let entries = ids
                .into_iter()
                .filter(|id| state.menu.items.contains_key(id) && seen.insert(*id))
                .map(|id| Variant::tuple_from_iter([id.to_variant(), state.menu.props(id, &names)]));
            let array = Variant::array_from_iter_with_type(VariantTy::new("(ia{sv})").unwrap(), entries);
            Some(Variant::tuple_from_iter([array]))
        }
        "GetProperty" => {
            let (id, name) = args.get::<(i32, String)>().unwrap_or_default();
            let state = state.borrow();
            let value = state
                .menu
                .items
                .get(&id)
                .and_then(|item| item.props.iter().find(|(n, _)| *n == name))
                .map_or_else(|| "".to_variant(), |(_, value)| value.variant());
            Some(Variant::tuple_from_iter([Variant::from_variant(&value)]))
        }
        "Event" => {
            let id = args.child_value(0).get::<i32>().unwrap_or_default();
            let event = args.child_value(1).get::<String>().unwrap_or_default();
            invocation.return_value(None);
            event_on(&state, id, &event);
            return;
        }
        "EventGroup" => {
            let events = args.child_value(0);
            let mut errors = Vec::new();
            let mut clicked = Vec::new();
            for event in events.iter() {
                let id = event.child_value(0).get::<i32>().unwrap_or_default();
                let name = event.child_value(1).get::<String>().unwrap_or_default();
                if state.borrow().menu.items.contains_key(&id) {
                    clicked.push((id, name));
                } else {
                    errors.push(id);
                }
            }
            invocation.return_value(Some(&(errors,).to_variant()));
            for (id, name) in clicked {
                event_on(&state, id, &name);
            }
            return;
        }
        "AboutToShow" => Some((false,).to_variant()),
        "AboutToShowGroup" => Some((Vec::<i32>::new(), Vec::<i32>::new()).to_variant()),
        _ => None,
    };
    invocation.return_value(reply.as_ref());
}

/// A host reports an event on an item: a click chooses it, with the
/// activation token the host gave for it, if any.
fn event_on(state: &Rc<RefCell<State>>, id: i32, event: &str) {
    if event != "clicked" {
        return;
    }
    let (activates, activate, token, on_token) = {
        let mut state = state.borrow_mut();
        let activates = state.menu.items.get(&id).and_then(|item| item.activates);
        (activates, state.activate.clone(), state.token.take(), state.on_token.clone())
    };
    let (Some(id), Some(activate)) = (activates, activate) else { return };
    if let Some(token) = token {
        on_token(token);
    }
    activate(id);
}
