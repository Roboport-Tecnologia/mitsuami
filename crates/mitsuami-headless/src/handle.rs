//! The handle tests keep: the command log, and the user's window and system changes.

use mitsuami_core::a11y::A11yProps;
use mitsuami_core::backend::PlatformMetrics;
use mitsuami_core::{
    Command, NativeAppInfo, NativeIcon, NodeId, Prop, Size, SurfaceInput, UiEvent, WidgetKind, find_prop,
};

use super::HeadlessHandle;
use super::measure::png_size;
use super::metrics::{SCREEN, WORK_AREA};
use super::windows::at_least_min;

impl HeadlessHandle {
    /// Every command applied so far, in order.
    pub fn command_log(&self) -> Vec<Command> {
        self.state.borrow().log.clone()
    }

    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    /// The app's language as the core last gave it (`Backend::set_locale`),
    /// and whether it's right to left.
    pub fn app_locale(&self) -> Option<(String, bool)> {
        self.state.borrow().locale.as_ref().map(|(language, rtl)| (language.to_string(), *rtl))
    }

    /// Simulates the user resizing a window, which goes no smaller than
    /// its minimum size, and keeps its height if its content sets it. A
    /// window that isn't resizable keeps its size.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let mut state = self.state.borrow_mut();
        if state.nodes.get(&window).and_then(|n| find_prop!(n.props, Resizable)) == Some(false) {
            return;
        }
        let size = state.nodes.get(&window).map_or(size, |node| {
            let size = at_least_min(node, size);
            match find_prop!(node.props, HeightFollowsContent) {
                Some(true) => Size::new(size.width, node.frame.size.height),
                _ => size,
            }
        });
        if let Some(node) = state.nodes.get_mut(&window) {
            node.frame.size = size;
        }
        state.emit(window, UiEvent::WindowResized(size));
    }

    /// The size of the screen a window in full screen fills.
    pub fn screen(&self) -> Size {
        SCREEN
    }

    /// Simulates the user putting a window in full screen, or taking it
    /// out (the title bar's button, the window manager's key).
    pub fn set_full_screen(&self, window: NodeId, on: bool) {
        let mut state = self.state.borrow_mut();
        if !state.nodes.contains_key(&window) {
            return;
        }
        state.set_prop(window, Prop::FullScreen(on));
        state.fill_screen(window, on);
        state.emit(window, UiEvent::FullScreenChanged(on));
    }

    /// The size of a maximized window's content.
    pub fn work_area(&self) -> Size {
        WORK_AREA
    }

    /// Simulates the user maximizing a window, or restoring it (the title
    /// bar's button, a double-click on it, the window manager's key).
    pub fn set_maximized(&self, window: NodeId, on: bool) {
        let mut state = self.state.borrow_mut();
        if !state.nodes.contains_key(&window) {
            return;
        }
        state.set_prop(window, Prop::Maximized(on));
        state.maximize(window, on);
        state.emit(window, UiEvent::MaximizedChanged(on));
    }

    /// Simulates the user showing or hiding a window's sidebar (the
    /// platform's toggle, its divider dragged away).
    pub fn set_sidebar_shown(&self, sidebar: NodeId, shown: bool) {
        let mut state = self.state.borrow_mut();
        let Some(node) = state.nodes.get(&sidebar) else { return };
        if find_prop!(node.props, SidebarShown).unwrap_or(true) == shown {
            return;
        }
        state.set_prop(sidebar, Prop::SidebarShown(shown));
        state.emit(sidebar, UiEvent::SidebarShownChanged(shown));
    }

    /// Simulates a change of system settings (text size, dark mode, …).
    pub fn set_metrics(&self, metrics: PlatformMetrics) {
        let mut state = self.state.borrow_mut();
        state.metrics = metrics;
        let surfaces: Vec<NodeId> =
            state.nodes.iter().filter(|(_, n)| n.surface.is_some()).map(|(id, _)| *id).collect();
        for surface in surfaces {
            state.size_surface(surface);
        }
        let windows: Vec<NodeId> =
            state.nodes.iter().filter(|(_, n)| n.kind == WidgetKind::Window).map(|(id, _)| *id).collect();
        for window in windows {
            state.emit(window, UiEvent::MetricsChanged);
        }
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.state.borrow().focused
    }

    /// Simulates the user switching to another app: the window's GPU
    /// surfaces lose their pointer lock and keyboard grab, as every
    /// platform ends them.
    pub fn deactivate(&self, window: NodeId) {
        let mut state = self.state.borrow_mut();
        let surfaces: Vec<NodeId> = state
            .nodes
            .iter()
            .filter(|(id, n)| n.kind == WidgetKind::GpuSurface && state.window_of(**id) == Some(window))
            .map(|(id, _)| *id)
            .collect();
        for surface in surfaces {
            state.end_lock(surface, Prop::PointerLock(false));
            state.end_lock(surface, Prop::KeyboardGrab(false));
        }
    }

    /// Simulates the mouse moving while a surface holds the pointer: its
    /// move with the host's acceleration, and before it. Headless has no
    /// acceleration, and one count to a point.
    pub fn move_locked_pointer(&self, surface: NodeId, dx: f32, dy: f32) {
        let state = self.state.borrow();
        if state.nodes.get(&surface).and_then(|n| find_prop!(n.props, PointerLock)) != Some(true) {
            return;
        }
        state.emit(surface, UiEvent::SurfaceInput(SurfaceInput::Motion { dx, dy }));
        state.emit(surface, UiEvent::SurfaceInput(SurfaceInput::RawMotion { dx, dy }));
    }

    pub fn a11y(&self, id: NodeId) -> Option<A11yProps> {
        self.state.borrow().nodes.get(&id).map(|n| n.a11y.clone())
    }

    /// Number of live native nodes: a leak detector for tests.
    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }
}

impl mitsuami_core::TestHooks for HeadlessHandle {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        HeadlessHandle::resize_window(self, window, size);
    }

    fn close_window(&self, window: NodeId) {
        self.state.borrow_mut().emit(window, UiEvent::WindowCloseRequested);
    }

    fn take_command_log(&self) -> Vec<Command> {
        HeadlessHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        HeadlessHandle::node_count(self)
    }

    /// The selected rows' files if the row is selected, else its own, as
    /// platforms' lists drag.
    fn dragged_files(&self, node: NodeId) -> Option<Vec<std::path::PathBuf>> {
        let state = self.state.borrow();
        let host = state.nodes.get(&node)?;
        let row = find_prop!(host.props, Row).or_else(|| find_prop!(host.props, Cell).map(|cell| cell.row))?;
        let list = &state.nodes[&host.parent?];
        let files = find_prop!(list.props, RowFiles)?;
        let selected = find_prop!(list.props, Selected).unwrap_or_default();
        let rows = if selected.contains(&row) { selected } else { vec![row] };
        let dragged: Vec<_> =
            rows.iter().filter_map(|r| files.iter().find(|(f, _)| f == r).map(|(_, path)| path.clone())).collect();
        (!dragged.is_empty()).then_some(dragged)
    }

    /// All of it, as every platform has a place for some of it; the icon's
    /// size from its PNG header.
    fn app_info(&self, _window: NodeId) -> NativeAppInfo {
        let app = self.state.borrow().app.clone();
        let icon = app.icon.and_then(|icon| icon.read()).and_then(|bytes| png_size(&bytes));
        NativeAppInfo {
            id: app.id,
            name: app.name,
            icon: icon.map(|size| NativeIcon::Image { width: size.width as u32, height: size.height as u32 }),
        }
    }
}
