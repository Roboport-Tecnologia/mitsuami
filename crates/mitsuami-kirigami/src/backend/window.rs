//! Windows: their content host's size and states, and making them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{Command, NodeId, Prop, SidebarSectionData, Size, UiEvent};

use crate::events::node_from_key;
use crate::ffi::QmlObject;
use crate::qml;
use crate::services::drawer_qml;

use super::{KirigamiHandle, State, Widget, WindowRoot, size_of};

/// `Qt::WindowFullScreen`.
const FULL_SCREEN: i32 = 0x4;
/// `Qt::WindowMaximized`.
const MAXIMIZED: i32 = 0x2;

/// `QWINDOWSIZE_MAX`, a window's largest side and its default maximum.
const WINDOW_SIZE_MAX: i32 = 16_777_215;

impl WindowRoot {
    pub(super) fn host_size(&self) -> Size {
        size_of(self.host)
    }

    /// The toolbar's height. Kirigami lays out its page stack when it
    /// polishes, before a frame, so the window's items are polished first.
    fn header(&self) -> f64 {
        if let Some(header) = self.header.get() {
            return header;
        }
        self.window.polish_items();
        let header = self.window.real("height") - self.host.real("height");
        header.max(0.0)
    }

    /// How much wider the window is than its content: by the sidebar's
    /// column, where the page row shows it beside the content.
    fn side(&self) -> f64 {
        if self.sidebar.get().is_none() {
            return 0.0;
        }
        self.window.polish_items();
        self.window.real("mitsuamiSidebarWidth").max(0.0)
    }

    /// Sizes the window so its content area is `size`. Qt sizes windows in
    /// whole pixels.
    pub(super) fn place(&self, size: Size) {
        let header = self.header();
        let height = (size.height as f64 + header).round();
        let width = (size.width as f64 + self.side()).round();
        // A locked height moves with it, and a fixed size: let go first,
        // so neither bound is past the other on the way.
        if !self.resizable.get() {
            self.window.set_int("minimumWidth", 0);
            self.window.set_int("maximumWidth", WINDOW_SIZE_MAX);
            self.window.set_int("maximumWidth", width as i32);
            self.window.set_int("minimumWidth", width as i32);
        }
        if self.height_locked.get() || !self.resizable.get() {
            self.window.set_int("minimumHeight", 0);
            self.window.set_int("maximumHeight", WINDOW_SIZE_MAX);
            self.window.set_int("maximumHeight", height as i32);
            self.window.set_int("minimumHeight", height as i32);
        }
        self.window.set_real("width", width);
        self.window.set_real("height", height);
    }

    /// The user can't resize it: Qt has no such flag on Linux, and KWin
    /// holds a window whose minimum is its maximum, as KDE's fixed-size
    /// dialogs are.
    pub(super) fn set_resizable(&self, on: bool) {
        self.resizable.set(on);
        self.apply_min();
    }

    pub(super) fn width_fixed(&self) -> bool {
        self.window.int("maximumWidth") < WINDOW_SIZE_MAX
    }

    pub(super) fn is_maximized(&self) -> bool {
        self.window.window_states() & MAXIMIZED != 0
    }

    /// As KDE's maximize action does: full screen stays as it is.
    pub(super) fn set_maximized(&self, on: bool) {
        self.maximized.set(on);
        let states = self.window.window_states();
        let wanted = if on { states | MAXIMIZED } else { states & !MAXIMIZED };
        if wanted != states {
            self.window.set_window_states(wanted);
        }
    }

    /// The content sets the height: a minimum and a maximum at the height
    /// it has, so the user resizes only the width, as 2ksbox's launcher
    /// holds its content-sized dialogs.
    pub(super) fn set_height_locked(&self, locked: bool) {
        self.height_locked.set(locked);
        self.apply_min();
    }

    /// A fixed size holds the height too: then the app's.
    pub(super) fn height_locked(&self) -> bool {
        if self.resizable.get() { self.window.int("maximumHeight") < WINDOW_SIZE_MAX } else { self.height_locked.get() }
    }

    /// Full screen as Qt has it: what it asked the platform for, until the
    /// platform says otherwise.
    pub(super) fn in_full_screen(&self) -> bool {
        self.window.window_states() & FULL_SCREEN != 0
    }

    /// As KDE's full screen action does: the other states (maximized)
    /// stay, for when it comes back.
    pub(super) fn set_full_screen(&self, on: bool) {
        self.full_screen.set(on);
        let states = self.window.window_states();
        let wanted = if on { states | FULL_SCREEN } else { states & !FULL_SCREEN };
        if wanted != states {
            self.window.set_window_states(wanted);
        }
    }

    /// Qt applied a state: on Wayland once the compositor has, and when
    /// the user changed it (the window manager's key). Only what the app
    /// didn't ask for is reported, a refusal too.
    fn states_changed(&self) {
        let now = self.in_full_screen();
        if self.full_screen.replace(now) != now {
            self.events.emit(self.id, UiEvent::FullScreenChanged(now));
        }
        let now = self.is_maximized();
        if self.maximized.replace(now) != now {
            self.events.emit(self.id, UiEvent::MaximizedChanged(now));
        }
    }

    /// The window's minimum is the content's and Kirigami's toolbar above
    /// it, in whole points, no larger than its screen takes.
    fn min_window_size(&self, min: Size) -> (i32, i32) {
        let min = self.capped(min);
        ((min.width as f64 + self.side()).ceil() as i32, (min.height as f64 + self.header()).ceil() as i32)
    }

    /// A minimum no larger than the content of a window filling its
    /// screen's available area (a machine's mode can be larger than a
    /// laptop's screen), in whole points.
    fn capped(&self, min: Size) -> Size {
        let Some((width, height)) = self.window.available_size() else { return min };
        let most =
            Size::new((width - self.side()).floor().max(0.0) as f32, (height - self.header()).floor().max(0.0) as f32);
        Size::new(min.width.min(most.width), min.height.min(most.height))
    }

    /// Another screen, another cap on the minimum.
    fn screen_changed(&self) {
        self.apply_min();
    }

    pub(super) fn apply_min(&self) {
        let min = self.min.get().map(|min| self.min_window_size(min));
        let fixed = !self.resizable.get();
        let current = |side: &str| self.window.real(side).round() as i32;
        let locked = (self.height_locked.get() || fixed).then(|| current("height"));
        // Nothing to set, or to take back.
        let held = self.window.int("maximumHeight") < WINDOW_SIZE_MAX || self.width_fixed();
        if min.is_none() && locked.is_none() && !held {
            return;
        }
        let (width, height) = min.unwrap_or((0, 0));
        let fixed_width = fixed.then(|| current("width"));
        self.window.set_int("maximumWidth", fixed_width.unwrap_or(WINDOW_SIZE_MAX));
        self.window.set_int("minimumWidth", fixed_width.unwrap_or(width));
        self.window.set_int("maximumHeight", locked.unwrap_or(WINDOW_SIZE_MAX));
        self.window.set_int("minimumHeight", locked.unwrap_or(height));
    }

    /// The minimum as Qt has it: the app's, if Qt has what it was given
    /// (capped by the screen). A locked height hides the minimum's.
    pub(super) fn min_size(&self) -> Size {
        // A fixed size hides the minimum: the app's.
        if !self.resizable.get() {
            return self.min.get().unwrap_or(Size::ZERO);
        }
        let (width, height) = (self.window.int("minimumWidth"), self.window.int("minimumHeight"));
        let locked = self.height_locked();
        match self.min.get() {
            Some(min) if self.min_window_size(min).0 == width && (locked || self.min_window_size(min).1 == height) => {
                min
            }
            _ => {
                Size::new((width as f64 - self.side()).max(0.0) as f32, (height as f64 - self.header()).max(0.0) as f32)
            }
        }
    }

    /// A size no smaller than the minimum.
    pub(super) fn at_least_min(&self, size: Size) -> Size {
        let min = self.min.get().map_or(Size::ZERO, |min| self.capped(min));
        Size::new(size.width.max(min.width), size.height.max(min.height))
    }

    /// Whether the window has drawn a frame: it's shown and laid out.
    pub(crate) fn has_rendered(&self) -> bool {
        self.header.get().is_some()
    }

    /// The first frame is laid out for real: the toolbar's height is known.
    fn rendered(&self) {
        if self.header.get().is_some() {
            return;
        }
        let header = self.window.real("height") - self.host.real("height");
        self.header.set(Some(header.max(0.0)));
        self.apply_min();
        if let Some(requested) = self.requested.get() {
            self.place(requested);
        }
        self.host_resized();
    }

    /// The host's height changed while the window's didn't: the toolbar
    /// did (a toolbar item taller than it, or one gone). The content keeps
    /// the size the core has; the window grows or shrinks instead. Only on
    /// the host's height: its width changes first when both do, while its
    /// height still lags the window's.
    fn toolbar_resized(&self) {
        let Some(header) = self.header.get() else { return };
        let now = (self.window.real("height") - self.host.real("height")).max(0.0);
        if (now - header).abs() < 0.5 {
            return;
        }
        self.header.set(Some(now));
        self.apply_min();
        match self.requested.get() {
            Some(size) => self.place(size),
            None => self.request(self.size.get()),
        }
    }

    /// The host changed size: report it, unless it's on its way to a size
    /// the core asked for.
    fn host_resized(&self) {
        let size = self.host_size();
        if let Some(requested) = self.requested.get() {
            let close = (size.width - requested.width).abs() < 1.0 && (size.height - requested.height).abs() < 1.0;
            if !close || self.header.get().is_none() {
                return;
            }
            self.requested.set(None);
        }
        if self.size.replace(size) != size {
            self.events.emit(self.id, UiEvent::WindowResized(size));
        }
    }

    fn request(&self, size: Size) {
        self.size.set(size);
        self.requested.set(Some(size));
        self.place(size);
    }

    /// Resizes it to what the app asked for, no smaller than its minimum,
    /// and reports the size it gets. Before it's shown, the core's size
    /// is what it asked for: a minimum that grows it is reported too.
    pub(super) fn resize_to(&self, asked: Size) {
        if !self.has_rendered() {
            self.size.set(asked);
        }
        let size = self.at_least_min(asked);
        self.requested.set(Some(size));
        self.place(size);
    }
}

impl State {
    pub(super) fn create_window(&mut self, id: NodeId, command: &Command) -> Widget {
        let events = self.events.clone();
        // A dialog's drawer holds only its own menus. Its modality comes
        // with its Create, so the drawer can come with the window.
        if let Command::Create { props, .. } = command
            && props.iter().any(|p| matches!(p, Prop::Modal { .. }))
        {
            self.menus.modal.insert(id);
        }
        let menu = self.menus.of(id);
        let drawer = drawer_qml(&menu, self.menus.modal.contains(&id));
        let window = QmlObject::load(&qml::window(drawer.as_deref()));
        let host = window.child("mitsuamiHost").expect("windows have a content host");
        let root = Rc::new(WindowRoot {
            id,
            window,
            host,
            events: events.clone(),
            size: Cell::new(Size::ZERO),
            requested: Cell::new(None),
            header: Cell::new(None),
            drawer: Cell::new(None),
            menu: RefCell::new(menu),
            toolbar: RefCell::new(Vec::new()),
            focused_first: Cell::new(false),
            full_screen: Cell::new(false),
            maximized: Cell::new(false),
            resizable: Cell::new(true),
            min: Cell::new(None),
            height_locked: Cell::new(false),
            sidebar: Cell::new(None),
        });
        let weak = Rc::downgrade(&root);
        window.connect("windowStateChanged(Qt::WindowState)", move || {
            if let Some(root) = weak.upgrade() {
                root.states_changed();
            }
        });
        let weak = Rc::downgrade(&root);
        window.connect("screenChanged(QScreen*)", move || {
            if let Some(root) = weak.upgrade() {
                root.screen_changed();
            }
        });
        for signal in ["widthChanged()", "heightChanged()"] {
            let root = Rc::downgrade(&root);
            let height = signal == "heightChanged()";
            host.connect(signal, move || {
                if let Some(root) = root.upgrade() {
                    if height {
                        root.toolbar_resized();
                    }
                    root.host_resized();
                }
            });
        }
        let weak = Rc::downgrade(&root);
        window.connect("frameSwapped()", move || {
            if let Some(root) = weak.upgrade() {
                root.rendered();
            }
        });
        // The app decides whether a window closes (e.g. to ask about
        // unsaved changes); the core destroys it if so.
        let e = events.clone();
        window.watch_close(move || e.emit(id, UiEvent::WindowCloseRequested));
        // One observer for every focus change: clicks, Tab, code.
        let (e, focused) = (events.clone(), Cell::new(None));
        window.connect("activeFocusItemChanged()", move || {
            let now = window.focus_item().and_then(|item| item.node()).map(node_from_key);
            let before = focused.replace(now);
            if before != now {
                if let Some(old) = before {
                    e.emit(old, UiEvent::FocusOut);
                }
                if let Some(new) = now {
                    e.emit(new, UiEvent::FocusIn);
                }
            }
        });
        let e = events.clone();
        window.connect("devicePixelRatioChanged()", move || e.emit(id, UiEvent::MetricsChanged));
        self.pending_show.push(id);
        if drawer.is_some()
            && let Some(wiring) = &self.menus.wiring
        {
            // Its drawer came with the window, if it has menus.
            root.drawer.set(window.object("globalDrawer"));
            wiring.connect(&root);
        }
        Widget::Window { root }
    }
}

/// Takes a window's sidebar out of its page row: the content's page is
/// titled after the window again, and the content keeps its size.
pub(super) fn hide_sidebar(root: &WindowRoot) {
    let Some((_, page)) = root.sidebar.take() else { return };
    page.set_object("mitsuamiContent", None);
    root.window.invoke("mitsuamiHideSidebar");
    if let Some(content) = root.window.child("mitsuamiPage") {
        content.set_str("title", &root.window.str("title"));
    }
    let size = root.size.get();
    if !size.is_empty() {
        root.resize_to(size);
    }
}

/// A sidebar's sections as the JSON its page reads (`qml::sidebar`).
pub(super) fn sections_json(sections: &[SidebarSectionData]) -> String {
    fn string(s: &str) -> String {
        let mut out = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    let optional = |s: &Option<String>| s.as_deref().map_or("null".to_owned(), string);
    let sections: Vec<String> = sections
        .iter()
        .map(|section| {
            let items: Vec<String> = section
                .items
                .iter()
                .map(|item| format!(r#"{{"title":{},"icon":{}}}"#, string(&item.title), optional(&item.icon)))
                .collect();
            format!(r#"{{"title":{},"items":[{}]}}"#, optional(&section.title), items.join(","))
        })
        .collect();
    format!("[{}]", sections.join(","))
}

/// Opens dialogs on this window, or the active one.
pub(crate) fn dialog_parent(handle: &KirigamiHandle, parent: Option<NodeId>) -> Option<Rc<WindowRoot>> {
    let windows = handle.windows();
    // Not by `active`: a modal window's owner, which it blocks, reports it
    // too, and so do the owner's other dialogs.
    parent
        .and_then(|id| windows.iter().find(|(w, _)| *w == id).map(|(_, root)| root.clone()))
        .or_else(|| windows.iter().find(|(_, root)| root.window.bool("mitsuamiFocused")).map(|(_, root)| root.clone()))
        .or_else(|| windows.first().map(|(_, root)| root.clone()))
}
