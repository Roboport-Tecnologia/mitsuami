//! The backend's handle: its command log and test hooks.

use mitsuami_core::{Command, NativeAppInfo, NativeIcon, NodeId, Size, WidgetKind};
use objc2::Message;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSView};
use objc2_foundation::{NSBundle, NSDefaultRunLoopMode, NSProcessInfo, NSRunLoop};

use super::{AppKitHandle, State, Widget};

impl AppKitHandle {
    /// The app's name as its menu shows it: the app's, or else the
    /// process's (a bundled app's executable).
    pub(crate) fn app_name(&self) -> String {
        let name = self.state.borrow().app_name.clone();
        name.unwrap_or_else(|| NSProcessInfo::processInfo().processName().to_string())
    }

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

    /// Escape hatch: the native view of any node (a window's content view).
    pub fn ns_view(&self, id: NodeId) -> Option<Retained<NSView>> {
        let state = self.state.borrow();
        let view = state.nodes.get(&id)?.widget.view();
        Some(view.retain())
    }
}

impl mitsuami_core::TestHooks for AppKitHandle {
    fn name(&self) -> &'static str {
        "appkit"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        AppKitHandle::resize_window(self, window, size);
    }

    /// As the close button does: the window asks its delegate.
    fn close_window(&self, window: NodeId) {
        if let Some(window) = self.ns_window(window) {
            window.performClose(None);
        }
    }

    fn take_command_log(&self) -> Vec<Command> {
        AppKitHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        AppKitHandle::node_count(self)
    }

    /// The id is the bundle's; the name is kept, as only the menus show
    /// it; the icon is the Dock's, in points.
    fn app_info(&self, _window: NodeId) -> NativeAppInfo {
        let mtm = self.state.borrow().mtm;
        let id = NSBundle::mainBundle().bundleIdentifier().map(|id| id.to_string());
        // In points: AppKit keeps a snapshot at the screen's scale.
        let icon = NSApplication::sharedApplication(mtm).applicationIconImage().map(|image| {
            let size = image.size();
            NativeIcon::Image { width: size.width.round() as u32, height: size.height.round() as u32 }
        });
        NativeAppInfo { id, name: self.state.borrow().app_name.clone(), icon }
    }

    /// Offscreen windows get no display cycle, where tables add the rows
    /// scrolling brought into view and toolbars place their items. While a
    /// search field is shown, timers that are due fire, as in the app's run
    /// loop: it searches once typing pauses. Not always yet: tables and tab
    /// views run timers too, which change what their tests capture.
    fn settle(&self) {
        if self.state.borrow().nodes.values().any(|n| n.kind == WidgetKind::SearchInput) {
            NSRunLoop::currentRunLoop().limitDateForMode(unsafe { NSDefaultRunLoopMode });
        }
        let state = self.state.borrow();
        state.layout_lists();
        state.layout_toolbars();
    }
}

impl State {
    /// Lets toolbars place their items now, and split windows their
    /// content: a window that was never shown (as in tests) doesn't even
    /// make its toolbar's views before.
    pub(super) fn layout_toolbars(&self) {
        for node in self.nodes.values() {
            if let Widget::Window { window, toolbar, split, .. } = &node.widget
                && (toolbar.is_some() || split.is_some())
            {
                window.layoutIfNeeded();
            }
        }
    }

    /// Lets tables lay out their rows now rather than at the next display:
    /// they report the rows they show, and the core builds those in the
    /// same run-loop turn, before anything is drawn. Their callbacks only
    /// emit.
    pub(super) fn layout_lists(&self) {
        for node in self.nodes.values() {
            if let Widget::List(list) = &node.widget {
                // Scrolling doesn't mark the table as needing layout; its
                // rows follow at the next display.
                list.table.setNeedsLayout(true);
                list.scroll.layoutSubtreeIfNeeded();
            }
        }
    }
}
