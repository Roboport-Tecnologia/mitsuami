//! The shared handle: settling, windows, menus and test hooks.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::prelude::*;
use gtk::{glib, graphene, gsk};
use mitsuami_core::services::MenuBarData;
use mitsuami_core::{Command, NativeAppInfo, NativeIcon, NodeId, Rect, Size, UiEvent};

use crate::host::Host;

use super::window::{requested, resize};
use super::{GtkHandle, State, Widget, pump_until};

impl GtkHandle {
    pub fn command_log(&self) -> Vec<Command> {
        self.state.borrow().log.clone()
    }

    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    /// Number of live native nodes.
    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }

    /// Called after every native event, so the run loop ticks.
    pub fn set_wake(&self, wake: impl Fn() + 'static) {
        self.state.borrow().events.set_wake(wake);
    }

    /// Resizes a window's content like the user would, no smaller than its
    /// minimum (GTK allocates no less), and waits until GTK has allocated
    /// it; the content host reports it as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let Some((gtk_window, host, _)) = self.window_parts(window) else { return };
        // The user can't resize it.
        if !gtk_window.is_resizable() {
            return;
        }
        let (min_width, min_height) = host.size_request();
        let size = Size::new(size.width.max(requested(min_width)), size.height.max(requested(min_height)));
        let (extra_width, extra_height) = self.window_extra(window, size);
        resize(&gtk_window, size.width as i32 + extra_width, size.height as i32 + extra_height);
        let target = (size.width as i32, size.height as i32);
        pump_until(Duration::from_secs(2), || (WidgetExt::width(&host), WidgetExt::height(&host)) == target);
    }

    /// Presents windows whose first layout has been applied.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            if let Some(window) = self.gtk_window(id) {
                window.present();
            }
        }
    }

    /// Lets list views that changed lay out now, rather than at the next
    /// frame: they bind rows (and place them) when allocated, and on a
    /// display nobody watches, frames stall. Each list's scrolled window is
    /// allocated again at its frame, as its host does; then the rows the
    /// list views report are delivered.
    fn layout_lists(&self) {
        let lists: Vec<(gtk::ScrolledWindow, Rect)> = {
            let state = self.state.borrow();
            if !state.lists_dirty.replace(false) {
                return;
            }
            let frames = state.frames.borrow();
            state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::List(list) if list.scrolled.is_mapped() => {
                        let frame = frames.get(list.scrolled.upcast_ref::<gtk::Widget>()).copied()?;
                        Some((list.scrolled.clone(), frame))
                    }
                    _ => None,
                })
                .collect()
        };
        for (scrolled, frame) in lists {
            scrolled.measure(gtk::Orientation::Horizontal, -1);
            let transform = gsk::Transform::new().translate(&graphene::Point::new(frame.x(), frame.y()));
            scrolled.allocate(frame.width().round() as i32, frame.height().round() as i32, -1, Some(transform));
        }
        self.pump();
    }

    /// Waits for mapped windows to reach the size the app or their minimum
    /// asked for, which GTK allocates (and the host reports) at the next
    /// frame; a platform that gives another size is waited for no longer
    /// than a user's resize is.
    fn wait_for_resizes(&self) {
        let ids: Vec<NodeId> = self.windows().into_iter().map(|(id, _)| id).collect();
        for (window, host, _) in ids.into_iter().filter_map(|id| self.window_parts(id)) {
            let Some(root) = host.window_root() else { continue };
            let Some(size) = root.resizing.get() else { continue };
            if !window.is_mapped() {
                continue;
            }
            let target = (size.width as i32, size.height as i32);
            pump_until(Duration::from_secs(2), || (WidgetExt::width(&host), WidgetExt::height(&host)) == target);
            root.resizing.set(None);
        }
    }

    /// Lets GPU surfaces whose frame changed take it now, rather than at
    /// the next frame, as lists do: each reports its new size when
    /// allocated, and the app draws at it.
    fn layout_surfaces(&self) {
        let surfaces: Vec<(gtk::DrawingArea, Rect)> = {
            let state = self.state.borrow();
            let frames = state.frames.borrow();
            state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::GpuSurface(surface) if surface.area.is_mapped() => {
                        let frame = frames.get(surface.area.upcast_ref::<gtk::Widget>()).copied()?;
                        let size = (frame.width().round() as i32, frame.height().round() as i32);
                        let allocated = (WidgetExt::width(&surface.area), WidgetExt::height(&surface.area));
                        (size != allocated).then(|| (surface.area.clone(), frame))
                    }
                    _ => None,
                })
                .collect()
        };
        for (area, frame) in surfaces {
            area.measure(gtk::Orientation::Horizontal, -1);
            let transform = gsk::Transform::new().translate(&graphene::Point::new(frame.x(), frame.y()));
            area.allocate(frame.width().round() as i32, frame.height().round() as i32, -1, Some(transform));
        }
        self.pump();
    }

    /// Lets header bars whose items changed place them now, rather than at
    /// the next frame, as lists do: each is allocated again where it is.
    fn layout_headers(&self) {
        let headers: Vec<adw::HeaderBar> = {
            let state = self.state.borrow();
            state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::Window(parts) if parts.header.is_mapped() && parts.header.should_layout() => {
                        Some(parts.header.clone())
                    }
                    _ => None,
                })
                .collect()
        };
        for header in headers {
            let Some(bounds) = header.parent().and_then(|p| header.compute_bounds(&p)) else { continue };
            header.measure(gtk::Orientation::Horizontal, -1);
            let transform = gsk::Transform::new().translate(&graphene::Point::new(bounds.x(), bounds.y()));
            header.allocate(bounds.width().round() as i32, bounds.height().round() as i32, -1, Some(transform));
        }
        self.pump();
    }

    /// Lets tab views that switched pages, or got new ones, place them now,
    /// rather than at the next frame, as header bars do.
    fn layout_tabs(&self) {
        let views: Vec<gtk::Widget> = {
            let state = self.state.borrow();
            state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::Tabs(tabs) if tabs.root().is_mapped() && tabs.root().should_layout() => {
                        Some(tabs.root().clone())
                    }
                    _ => None,
                })
                .collect()
        };
        for view in views {
            let Some(bounds) = view.parent().and_then(|p| view.compute_bounds(&p)) else { continue };
            view.measure(gtk::Orientation::Horizontal, -1);
            let transform = gsk::Transform::new().translate(&graphene::Point::new(bounds.x(), bounds.y()));
            view.allocate(bounds.width().round() as i32, bounds.height().round() as i32, -1, Some(transform));
        }
        self.pump();
    }

    /// Dispatches whatever the GTK main context has ready, without waiting.
    pub fn pump(&self) {
        let context = glib::MainContext::default();
        while context.iteration(false) {}
    }

    /// Escape hatch: the native window of a window node.
    pub fn gtk_window(&self, id: NodeId) -> Option<gtk::Window> {
        self.window_parts(id).map(|(window, _, _)| window)
    }

    /// The actions of a window's primary menu, for tests: GTK has no getter
    /// for a widget's action groups.
    #[doc(hidden)]
    pub fn menu_actions(&self, window: NodeId) -> Option<gtk::gio::ActionGroup> {
        self.state.borrow().menus.actions(window)
    }

    /// Escape hatch: the native widget of any node (a window's content host).
    pub fn gtk_widget(&self, id: NodeId) -> Option<gtk::Widget> {
        self.state.borrow().nodes.get(&id).map(|n| n.widget.widget().clone())
    }

    fn window_parts(&self, id: NodeId) -> Option<(gtk::Window, Host, i32)> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window(parts) => Some((parts.window.clone(), parts.host.clone(), parts.header_height)),
            _ => None,
        }
    }

    /// How much larger than a content of this size a window is.
    fn window_extra(&self, id: NodeId, content: Size) -> (i32, i32) {
        match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => parts.extra(content),
            _ => (0, 0),
        }
    }

    /// Every window, for services (menus, dialog parents).
    pub(crate) fn windows(&self) -> Vec<(NodeId, gtk::Window)> {
        let state = self.state.borrow();
        let mut windows: Vec<(NodeId, gtk::Window)> = state
            .nodes
            .iter()
            .filter_map(|(id, n)| match &n.widget {
                Widget::Window(parts) => Some((*id, parts.window.clone())),
                _ => None,
            })
            .collect();
        windows.sort_by_key(|(id, _)| *id);
        windows
    }

    /// Asks the app to close every window (the Quit command).
    pub(crate) fn request_quit(&self) {
        let events = self.state.borrow().events.clone();
        for (id, _) in self.windows() {
            events.emit(id, UiEvent::WindowCloseRequested);
        }
    }

    pub(crate) fn from_weak(state: &Weak<RefCell<State>>) -> Option<GtkHandle> {
        state.upgrade().map(|state| GtkHandle { state })
    }

    /// Takes the app's menus (`window` is `None`) or a window's own, and
    /// brings the primary menus showing them up to date.
    pub(crate) fn set_menu(&self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let mut state = self.state.borrow_mut();
        let State { nodes, menus, .. } = &mut *state;
        menus.set(window, menu, activate);
        for (id, node) in nodes.iter() {
            if let Widget::Window(parts) = &node.widget
                && window.is_none_or(|w| w == *id)
            {
                menus.show_in(*id, parts);
            }
        }
    }
}

impl mitsuami_core::TestHooks for GtkHandle {
    fn name(&self) -> &'static str {
        "gtk"
    }

    fn dragged_files(&self, node: NodeId) -> Option<Vec<std::path::PathBuf>> {
        let state = self.state.borrow();
        let host = state.nodes.get(&node)?;
        match &state.nodes.get(&host.parent?)?.widget {
            Widget::List(list) => list.dragged_files(host.row?),
            _ => None,
        }
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        GtkHandle::resize_window(self, window, size);
    }

    /// As the close button does: GTK emits `close-request`.
    fn close_window(&self, window: NodeId) {
        if let Some((gtk_window, _, _)) = self.window_parts(window) {
            gtk_window.close();
        }
    }

    fn take_command_log(&self) -> Vec<Command> {
        GtkHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        GtkHandle::node_count(self)
    }

    fn app_info(&self, window: NodeId) -> NativeAppInfo {
        let icon = self
            .window_parts(window)
            .and_then(|(window, _, _)| window.icon_name())
            .or_else(gtk::Window::default_icon_name);
        NativeAppInfo {
            id: glib::prgname().map(Into::into),
            name: glib::application_name().map(Into::into),
            icon: icon.map(|name| NativeIcon::Named(name.into())),
        }
    }

    /// Windows are shown once laid out, and GTK delivers what it queued
    /// (allocations, scroll adjustments, focus).
    fn settle(&self) {
        self.show_pending_windows();
        self.pump();
        self.layout_lists();
        self.layout_headers();
        self.layout_tabs();
        self.layout_surfaces();
        self.wait_for_resizes();
    }
}
