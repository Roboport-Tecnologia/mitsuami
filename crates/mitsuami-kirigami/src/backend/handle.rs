//! [`KirigamiHandle`]: shared access to the backend, and the test hooks.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use mitsuami_core::{Command, NativeAppInfo, NativeIcon, NodeId, Size, UiEvent};

use crate::ffi::{self, QmlObject};
use crate::services::Menus;

use super::{KirigamiHandle, State, Widget, WindowRoot, pump_until};

impl KirigamiHandle {
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

    /// Resizes a window's content like the user would, and waits until Qt
    /// has laid it out; the content host reports it as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let Some(root) = self.window_root(window) else { return };
        // The user can't resize it.
        if !root.resizable.get() {
            return;
        }
        // A drag goes no smaller than the minimum, and keeps a locked height.
        let mut size = root.at_least_min(size);
        if root.height_locked() {
            size.height = root.size.get().height;
        }
        root.place(size);
        pump_until(Duration::from_secs(2), || root.host_size() == size);
    }

    /// Shows windows whose first layout has been applied.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            if let Some(root) = self.window_root(id) {
                root.window.set_bool("visible", true);
                root.window.invoke("requestActivate");
            }
        }
    }

    /// Dispatches whatever Qt has ready, without waiting.
    pub fn pump(&self) {
        ffi::process_events();
    }

    /// Escape hatch: the `Kirigami.ApplicationWindow` of a window node.
    pub fn qml_window(&self, id: NodeId) -> Option<QmlObject> {
        self.window_root(id).map(|root| root.window)
    }

    /// Escape hatch: the item of any node (a window's content host).
    pub fn qml_item(&self, id: NodeId) -> Option<QmlObject> {
        self.state.borrow().nodes.get(&id).map(|n| n.widget.item())
    }

    fn window_root(&self, id: NodeId) -> Option<Rc<WindowRoot>> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window { root } => Some(root.clone()),
            _ => None,
        }
    }

    /// Every window, in creation order, for services (menus, dialog parents).
    pub(crate) fn windows(&self) -> Vec<(NodeId, Rc<WindowRoot>)> {
        let state = self.state.borrow();
        let mut windows: Vec<(NodeId, Rc<WindowRoot>)> = state
            .nodes
            .iter()
            .filter_map(|(id, n)| match &n.widget {
                Widget::Window { root } => Some((*id, root.clone())),
                _ => None,
            })
            .collect();
        windows.sort_by_key(|(id, _)| *id);
        windows
    }

    pub(super) fn emit_to_windows(&self, event: UiEvent) {
        let Ok(state) = self.state.try_borrow() else { return };
        let events = state.events.clone();
        drop(state);
        for (id, _) in self.windows() {
            events.emit(id, event.clone());
        }
    }

    /// Asks the app to close every window (the Quit command).
    pub(crate) fn request_quit(&self) {
        self.emit_to_windows(UiEvent::WindowCloseRequested);
    }

    pub(crate) fn weak(&self) -> Weak<RefCell<State>> {
        Rc::downgrade(&self.state)
    }

    pub(crate) fn from_weak(state: &Weak<RefCell<State>>) -> Option<KirigamiHandle> {
        state.upgrade().map(|state| KirigamiHandle { state })
    }

    pub(crate) fn with_menus<R>(&self, f: impl FnOnce(&mut Menus) -> R) -> R {
        f(&mut self.state.borrow_mut().menus)
    }
}

impl mitsuami_core::TestHooks for KirigamiHandle {
    fn name(&self) -> &'static str {
        "kirigami"
    }

    fn dragged_files(&self, node: NodeId) -> Option<Vec<std::path::PathBuf>> {
        let state = self.state.borrow();
        let host = state.nodes.get(&node)?;
        match &state.nodes.get(&host.parent?)?.widget {
            Widget::List(list) => list.dragged_files(host.widget.item()),
            _ => None,
        }
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        KirigamiHandle::resize_window(self, window, size);
    }

    /// `QWindow::close()` asks the platform to close the window, which
    /// sends the close event the close button sends; the window's close
    /// filter reports it.
    fn close_window(&self, window: NodeId) {
        if let Some(root) = self.window_root(window) {
            root.window.invoke("close");
        }
    }

    fn take_command_log(&self) -> Vec<Command> {
        KirigamiHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        KirigamiHandle::node_count(self)
    }

    fn app_info(&self, window: NodeId) -> NativeAppInfo {
        let (id, name) = ffi::app_id_and_name();
        let icon = self.window_root(window).and_then(|root| root.window.window_icon());
        NativeAppInfo {
            id: (!id.is_empty()).then_some(id),
            name: (!name.is_empty()).then_some(name),
            icon: icon.map(|icon| match icon {
                ffi::WindowIcon::Named(name) => NativeIcon::Named(name),
                ffi::WindowIcon::Image { width, height } => NativeIcon::Image { width, height },
            }),
        }
    }

    /// Windows are shown once laid out, and Qt delivers what it queued
    /// (polishing, rendering, focus).
    fn settle(&self) {
        self.show_pending_windows();
        self.pump();
        // List views place and create delegates when they polish, before
        // a frame: have it now, so rows are where the view says.
        for (_, root) in self.windows() {
            if root.has_rendered() {
                root.window.polish_items();
            }
        }
        self.pump();
        // A size the app or a minimum asked for is reported once Qt lays
        // the window out; a platform that gives another is waited for no
        // longer than a user's resize is.
        for (_, root) in self.windows() {
            if root.has_rendered() && root.requested.get().is_some() {
                pump_until(Duration::from_secs(2), || root.requested.get().is_none());
            }
        }
    }
}
