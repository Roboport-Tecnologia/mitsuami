//! Windows: their parts, full screen, maximized, minimum size, header bar
//! items, modality and Escape.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};
use mitsuami_core::a11y::ActionError;
use mitsuami_core::{NodeId, Prop, Size, UiEvent, WidgetKind};

use crate::host::{Host, WindowRoot};

use super::{GtkBackend, State, Widget, WindowParts, owning_node, pump_until};

impl WindowParts {
    /// How much larger the window is than its content: by the header bar,
    /// and beside a sidebar, by it.
    pub(super) fn extra(&self, content: Size) -> (i32, i32) {
        let width = self.split.as_ref().map_or(0.0, |s| s.extra_width(content.width));
        (width.round() as i32, self.header_height)
    }
}

/// The app's minimum content size, no larger than the window's monitor less
/// the header bar: a machine's mode can be larger than a laptop's screen,
/// and GTK would make the window as large as asked. GTK 4 has no work area
/// (the panels' room) on Wayland, so it's the monitor's whole geometry.
/// Applied again when the window goes to another monitor, or the header
/// bar's height changes.
#[derive(Clone, Default)]
pub(super) struct MinSize {
    pub(super) app: Rc<Cell<Option<Size>>>,
    header_height: Rc<Cell<i32>>,
}

impl MinSize {
    /// The minimum GTK is given, in whole points.
    fn capped(&self, window: &gtk::Window) -> Option<(i32, i32)> {
        let min = self.app.get()?;
        let (mut width, mut height) = (min.width.ceil() as i32, min.height.ceil() as i32);
        if let Some(monitor) = monitor_of(window) {
            let area = monitor.geometry();
            width = width.min(area.width());
            height = height.min((area.height() - self.header_height.get()).max(0));
        }
        Some((width, height))
    }

    /// Asked by the content, which the window's minimum follows (the header
    /// bar's above it); a window smaller grows to it, as GTK allocates no
    /// less.
    pub(super) fn apply(&self, window: &gtk::Window, host: &Host) {
        let Some((width, height)) = self.capped(window) else { return };
        host.set_size_request(width, height);
        if let Some(root) = host.window_root() {
            let size = root.content_size();
            let grown = Size::new(size.width.max(width as f32), size.height.max(height as f32));
            if grown != size {
                root.resizing.set(Some(grown));
            }
        }
    }

    /// The minimum as GTK has it: the app's, if GTK holds it as capped.
    pub(super) fn shown(&self, window: &gtk::Window, host: &Host) -> Size {
        let (width, height) = host.size_request();
        match self.app.get() {
            Some(min) if self.capped(window) == Some((width, height)) => min,
            _ => Size::new(requested(width), requested(height)),
        }
    }
}

/// The monitor the window is on, or before it has a surface, the first.
fn monitor_of(window: &gtk::Window) -> Option<gdk::Monitor> {
    let display = WidgetExt::display(window);
    match window.surface() {
        Some(surface) => display.monitor_at_surface(&surface),
        None => display.monitors().item(0).and_downcast(),
    }
}

/// Full screen as the app wants it. GTK reports its own changes
/// (`notify::fullscreened`) like the user's, and later: the compositor
/// applies them when it configures the window. A change it didn't ask
/// for, or ended somewhere else, is the user's or the platform's.
#[derive(Clone, Default)]
pub(super) struct FullScreen {
    wanted: Rc<Cell<bool>>,
    /// Asked for, and not in effect yet.
    pending: Rc<Cell<bool>>,
}

impl FullScreen {
    pub(super) fn set(&self, window: &gtk::Window, on: bool) {
        self.wanted.set(on);
        self.pending.set(window.is_fullscreen() != on);
        if on { window.fullscreen() } else { window.unfullscreen() }
    }

    /// What the window shows, or while a request is pending (or it isn't
    /// shown yet), what it will.
    pub(super) fn shown(&self, window: &gtk::Window) -> bool {
        if self.pending.get() { self.wanted.get() } else { window.is_fullscreen() }
    }

    pub(super) fn in_effect(&self, window: &gtk::Window) -> bool {
        window.is_fullscreen() || (self.pending.get() && self.wanted.get())
    }
}

/// Maximized as the app wants it, which GTK reports as it does full
/// screen: its own changes like the user's, and later, when the compositor
/// configures the window.
#[derive(Clone, Default)]
pub(super) struct Maximized {
    wanted: Rc<Cell<bool>>,
    /// Asked for, and not in effect yet.
    pending: Rc<Cell<bool>>,
    /// The content's size when the app changed a shown window's state:
    /// GTK lays the window out at its new size at a later frame (Broadway
    /// resizes it at once, a compositor when it configures it), which
    /// settles wait for.
    from: Rc<Cell<Option<(i32, i32)>>>,
}

impl Maximized {
    pub(super) fn set(&self, window: &gtk::Window, host: &Host, on: bool) {
        self.wanted.set(on);
        self.pending.set(window.is_maximized() != on);
        if window.is_mapped() && window.is_maximized() != on {
            self.from.set(Some((WidgetExt::width(host), WidgetExt::height(host))));
        }
        if on { window.maximize() } else { window.unmaximize() }
    }

    /// Waits for the content to leave the size it had when the app last
    /// changed the state, no longer than a user's resize is waited for: a
    /// platform may keep the size (a window as large as the screen).
    pub(super) fn wait_for_size(&self, host: &Host) {
        if let Some(from) = self.from.take() {
            pump_until(Duration::from_secs(2), || (WidgetExt::width(host), WidgetExt::height(host)) != from);
        }
    }

    /// What the window shows, or while a request is pending (or it isn't
    /// shown yet), what it will.
    pub(super) fn shown(&self, window: &gtk::Window) -> bool {
        if self.pending.get() { self.wanted.get() } else { window.is_maximized() }
    }
}

/// Gives a window a new default size. Some backends (Broadway before GTK
/// 4.16) only size a toplevel when its surface is presented, so a mapped
/// window ignores a new default size until then. `GtkWindow::present`
/// would only focus it.
pub(crate) fn resize(window: &gtk::Window, width: i32, height: i32) {
    window.set_default_size(width, height);
    if window.is_mapped()
        && let Some(toplevel) = window.surface().and_downcast::<gdk::Toplevel>()
    {
        let layout = gdk::ToplevelLayout::new();
        layout.set_resizable(window.is_resizable());
        toplevel.present(&layout);
    }
}

/// A size request's side: none (-1) is 0.
pub(super) fn requested(side: i32) -> f32 {
    side.max(0) as f32
}

/// Packs a window's toolbar items at the end of its header bar, in order:
/// `pack_end` packs from the end inwards, so the last item goes first. The
/// main menu button, packed when the window was made, stays at the very
/// end, as GNOME's primary menu is.
pub(super) fn pack_items(parts: &WindowParts) {
    for (_, widget) in &parts.items {
        if widget.parent().is_some() {
            parts.header.remove(widget);
        }
    }
    for (_, widget) in parts.items.iter().rev() {
        parts.header.pack_end(widget);
    }
}

/// An item taller than the header bar makes it taller; the window grows
/// with it, so the content keeps the size the core asked for.
pub(super) fn keep_content_size(parts: &mut WindowParts) {
    let height = match &parts.top_bars {
        // Under a header, the content runs up under it, so what's above
        // the content is the top bars less the header.
        Some((view, header)) => {
            let height = crate::header::above_content(view, header);
            parts.host.set_margin_top(height);
            height
        }
        None => parts.header.measure(gtk::Orientation::Vertical, -1).1,
    };
    if height == parts.header_height {
        return;
    }
    parts.header_height = height;
    parts.min_size.header_height.set(height);
    parts.min_size.apply(&parts.window, &parts.host);
    let size = parts.host.window_root().expect("window hosts have a root").size.get();
    let (width, height) = parts.extra(size);
    parts.window.set_default_size(size.width as i32 + width, size.height as i32 + height);
}

impl State {
    /// Puts document tabs that want the title's row in their window's
    /// header bar, in the title's place, and the others back in their
    /// boxes. One per window has it, the first made; the others are
    /// strips. The header bar keeps its height (the bar asks for none), so
    /// the content keeps its size.
    pub(super) fn place_title_row_tabs(&mut self) {
        let window_of = |mut id: NodeId| loop {
            let node = self.nodes.get(&id)?;
            if let Widget::Window(_) = node.widget {
                return Some(id);
            }
            id = node.parent?;
        };
        let mut tabs: Vec<(NodeId, Option<NodeId>)> = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.widget {
                Widget::DocumentTabs(tabs) => Some((*id, if tabs.wants_title_row() { window_of(*id) } else { None })),
                _ => None,
            })
            .collect();
        tabs.sort_by_key(|(id, _)| *id);
        let mut taken = Vec::new();
        for (_, window) in &mut tabs {
            if let Some(w) = *window {
                if taken.contains(&w) {
                    *window = None;
                } else {
                    taken.push(w);
                }
            }
        }
        // Out first, so a window's header bar is free for the next.
        for leaving in [true, false] {
            for (id, window) in &tabs {
                if leaving != window.is_none() {
                    continue;
                }
                let header = window.and_then(|w| match &self.nodes[&w].widget {
                    Widget::Window(parts) => Some(parts.header.clone()),
                    _ => None,
                });
                let Widget::DocumentTabs(node) = &self.nodes[id].widget else { unreachable!() };
                if node.title_row() != header {
                    node.show_in_title_row(header.as_ref());
                    self.events.emit(*id, UiEvent::Remeasure);
                }
            }
        }
    }

    /// Puts each header at the top of a window's content under that
    /// window's header bar, and takes out those that left it (see
    /// `crate::header`). One a window, the first made; beside a sidebar a
    /// header stays where it is, as on AppKit.
    pub(super) fn place_headers(&mut self) {
        let mut headers: Vec<NodeId> =
            self.nodes.iter().filter(|(_, n)| n.header.is_some()).map(|(id, _)| *id).collect();
        headers.sort();
        let mut taken = Vec::new();
        let mut moves = Vec::new();
        for id in headers {
            let want = self.header_window(id).filter(|w| !taken.contains(w));
            taken.extend(want);
            let had = self.nodes[&id].header.as_ref().unwrap().window.get();
            if had != want {
                moves.push((id, had, want));
            }
        }
        // Out first, so a window's top bars are free for the next.
        for (id, had, _) in &moves {
            if let Some(window) = had {
                self.restore_window(*window);
            }
            let node = &self.nodes[id];
            let Widget::Host(placeholder) = &node.widget else { unreachable!("headers are hosts") };
            node.header.as_ref().unwrap().detach(Some(placeholder));
        }
        for (id, _, want) in moves {
            let Some(window) = want else { continue };
            let header = self.nodes.get_mut(&id).unwrap().header.take().unwrap();
            if let Some(Widget::Window(parts)) = self.nodes.get_mut(&window).map(|n| &mut n.widget) {
                // The content keeps its size: the header bar is as high as
                // it was, or the window follows it (`keep_content_size`).
                let view = header.attach(window, &parts.window, &parts.header, &parts.host);
                parts.top_bars = Some((view, header.content.clone()));
                keep_content_size(parts);
            }
            self.nodes.get_mut(&id).unwrap().header = Some(header);
        }
    }

    /// The window whose top bars a header goes in: the one it's at the top
    /// of the content of, by the core's frames, if that window has no
    /// sidebar.
    fn header_window(&self, id: NodeId) -> Option<NodeId> {
        let node = self.nodes.get(&id)?;
        if !node.header.as_ref()?.on.get() {
            return None;
        }
        let frame = |widget: &Widget| self.frames.borrow().get(widget.widget()).copied().unwrap_or_default();
        let own = frame(&node.widget);
        let mut top = own.y();
        let mut parent = node.parent;
        while let Some(p) = parent {
            let above = self.nodes.get(&p)?;
            match &above.widget {
                Widget::Window(parts) => {
                    return (parts.split.is_none() && top.abs() < 0.5 && own.height() > 0.0).then_some(p);
                }
                // Its own place, or its window's toolbar.
                Widget::Scroll { .. }
                | Widget::List(_)
                | Widget::Tabs(_)
                | Widget::Sidebar(_)
                | Widget::Group(_)
                | Widget::Split(_) => {
                    return None;
                }
                _ if above.kind == WidgetKind::ToolbarItem => return None,
                widget => {
                    top += frame(widget).y();
                    parent = above.parent;
                }
            }
        }
        None
    }

    /// Takes the header under `window`'s header bar, if there's one, back
    /// to its place in the content.
    pub(super) fn detach_header_of(&mut self, window: NodeId) {
        let Some(id) = self
            .nodes
            .iter()
            .find(|(_, n)| n.header.as_ref().is_some_and(|h| h.window.get() == Some(window)))
            .map(|(id, _)| *id)
        else {
            return;
        };
        self.restore_window(window);
        let node = &self.nodes[&id];
        let Widget::Host(placeholder) = &node.widget else { unreachable!("headers are hosts") };
        node.header.as_ref().unwrap().detach(Some(placeholder));
    }

    /// Gives a window its header bar and content back, from the toolbar
    /// view a header made it.
    pub(super) fn restore_window(&mut self, window: NodeId) {
        let Some(Widget::Window(parts)) = self.nodes.get_mut(&window).map(|n| &mut n.widget) else { return };
        let Some((view, _)) = parts.top_bars.take() else { return };
        crate::header::restore_window(&view, &parts.window, &parts.header, &parts.host);
        keep_content_size(parts);
    }

    pub(super) fn create_window(
        &mut self,
        id: NodeId,
        settings_handlers: &mut Vec<(glib::Object, glib::SignalHandlerId)>,
    ) -> WindowParts {
        let events = self.events.clone();
        let window = gtk::Window::new();
        // An explicit header bar has a known height, so the content gets
        // exactly the size the core asks for.
        let header = adw::HeaderBar::new();
        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_tooltip_text(Some(&mitsuami_core::l10n::tr("mitsuami-main-menu", &[])));
        menu_button.set_primary(true);
        header.pack_end(&menu_button);
        window.set_titlebar(Some(&header));
        // Measured with the menu button in, so showing it changes nothing.
        let header_height = header.measure(gtk::Orientation::Vertical, -1).1;
        menu_button.set_visible(false);
        let shortcuts = gtk::ShortcutController::new();
        window.add_controller(shortcuts.clone());

        let root = WindowRoot {
            id,
            events: events.clone(),
            size: Cell::new(Size::ZERO),
            resizing: Cell::new(None),
            focus_order: RefCell::new(Vec::new()),
        };
        let host = Host::new(self.frames.clone(), Some(root));
        window.set_child(Some(&host));

        let e = events.clone();
        // The app decides whether a window closes (e.g. to ask about
        // unsaved changes); the core destroys it if so.
        window.connect_close_request(move |_| {
            e.emit(id, UiEvent::WindowCloseRequested);
            glib::Propagation::Stop
        });
        // One observer for every focus change: clicks, Tab, code.
        let (e, map, focused) = (events.clone(), self.by_widget.clone(), Cell::new(None));
        window.connect_focus_widget_notify(move |window| {
            let now = owning_node(&map, GtkWindowExt::focus(window));
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
        window.connect_scale_factor_notify(move |_| e.emit(id, UiEvent::MetricsChanged));
        let full_screen = FullScreen::default();
        let (e, fs) = (events.clone(), full_screen.clone());
        window.connect_fullscreened_notify(move |window| {
            let now = window.is_fullscreen();
            // The app's own request, in effect.
            if fs.pending.replace(false) && now == fs.wanted.get() {
                return;
            }
            if now != fs.wanted.replace(now) {
                e.emit(id, UiEvent::FullScreenChanged(now));
            }
        });
        if let Some(settings) = gtk::Settings::default() {
            for property in ["gtk-font-name", "gtk-theme-name", "gtk-application-prefer-dark-theme"] {
                let e = events.clone();
                let handler =
                    settings.connect_notify_local(Some(property), move |_, _| e.emit(id, UiEvent::MetricsChanged));
                settings_handlers.push((settings.clone().upcast(), handler));
            }
        }
        if adw::is_initialized() {
            let (manager, e) = (adw::StyleManager::default(), events.clone());
            let handler = manager.connect_dark_notify(move |_| e.emit(id, UiEvent::MetricsChanged));
            settings_handlers.push((manager.upcast(), handler));
        }
        let maximized = Maximized::default();
        let (e, m) = (events.clone(), maximized.clone());
        window.connect_maximized_notify(move |window| {
            let now = window.is_maximized();
            // The app's own request, in effect.
            if m.pending.replace(false) && now == m.wanted.get() {
                return;
            }
            if now != m.wanted.replace(now) {
                e.emit(id, UiEvent::MaximizedChanged(now));
            }
        });
        let min_size = MinSize::default();
        min_size.header_height.set(header_height);
        // Another monitor, another cap on the minimum.
        let (min, h) = (min_size.clone(), host.clone());
        window.connect_realize(move |window| {
            let Some(surface) = window.surface() else { return };
            let (min, host, window) = (min.clone(), h.clone(), window.clone());
            surface.connect_enter_monitor(move |_, _| min.apply(&window, &host));
        });
        self.pending_show.push(id);
        let parts = WindowParts {
            window,
            host,
            header,
            header_height,
            items: Vec::new(),
            menu_button,
            shortcuts,
            full_screen,
            maximized,
            resizable: true,
            height_locked: false,
            min_size,
            split: None,
            top_bars: None,
        };
        self.menus.show_in(id, &parts);
        parts
    }
}

pub(super) fn prop_owner(prop: &Prop) -> Option<NodeId> {
    match prop {
        Prop::Modal { owner, .. } => *owner,
        _ => None,
    }
}

/// The name of a modal window's Escape controller.
const ESCAPE: &str = "mitsuami-escape";

/// What `GtkDialog` does: Escape closes the window, which asks first
/// (`close-request`, which the backend reports and stops). In the bubble
/// phase, so a focused widget that uses Escape itself (an open popover,
/// entry completion) gets it first.
pub(super) fn escape_closes() -> gtk::ShortcutController {
    let controller = gtk::ShortcutController::new();
    controller.set_name(Some(ESCAPE));
    controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
    let close = gtk::CallbackAction::new(|widget, _| {
        if let Some(window) = widget.downcast_ref::<gtk::Window>() {
            window.close();
        }
        glib::Propagation::Stop
    });
    controller.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string("Escape"), Some(close)));
    controller
}

impl GtkBackend {
    /// Escape on a node. GTK 4 can't inject key events, so after focusing
    /// the node, the window's Escape shortcut runs as a key press would
    /// run it; a plain window has none, and nothing happens, as with a
    /// real Escape.
    pub(super) fn escape(&mut self, id: NodeId) -> Result<(), ActionError> {
        // In a shown popover, its own Escape hides it, as a key press
        // would; it reports that itself (`crate::popover`).
        let popover = {
            let state = self.state.borrow();
            let mut node = Some(id);
            let mut found = None;
            while let Some(n) = node.and_then(|n| state.nodes.get(&n)) {
                if let Widget::Popover(popover) = &n.widget
                    && popover.shown()
                {
                    found = Some(popover.popover.clone());
                    break;
                }
                node = n.parent;
            }
            found
        };
        if let Some(popover) = popover {
            popover.popdown();
            return Ok(());
        }
        let widget = {
            let state = self.state.borrow();
            state.nodes.get(&id).ok_or(ActionError::UnknownNode)?.widget.widget().clone()
        };
        if widget.is_focusable() {
            widget.grab_focus();
        }
        let window = widget.root().and_then(|r| r.downcast::<gtk::Window>().ok()).ok_or(ActionError::Unsupported)?;
        let controllers = window.observe_controllers();
        let controller = (0..controllers.n_items())
            .filter_map(|i| controllers.item(i).and_downcast::<gtk::ShortcutController>())
            .find(|c| c.name().as_deref() == Some(ESCAPE));
        if let Some(action) =
            controller.and_then(|c| c.item(0).and_downcast::<gtk::Shortcut>()).and_then(|shortcut| shortcut.action())
        {
            action.activate(gtk::ShortcutActionFlags::empty(), &window, None);
        }
        Ok(())
    }
}
