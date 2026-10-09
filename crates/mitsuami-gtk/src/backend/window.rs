//! Windows: their parts, full screen, maximized, minimum size, header bar
//! items, modality and Escape.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};
use mitsuami_core::a11y::ActionError;
use mitsuami_core::{NodeId, Prop, Size, UiEvent};

use crate::host::{Host, WindowRoot};

use super::{GtkBackend, State, WindowParts, owning_node, pump_until};

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
            let size = root.resizing.get().unwrap_or(root.size.get());
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
    let height = parts.header.measure(gtk::Orientation::Vertical, -1).1;
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
