//! Windows' sizes (resized, in full screen, maximized) and GPU surfaces' sizes.

use mitsuami_core::{NodeId, Size, SurfaceSize, UiEvent, find_prop};

use super::metrics::{SCREEN, WORK_AREA};
use super::{HeadlessNode, State};

impl State {
    /// Gives a window a content size, no smaller than its minimum; one in
    /// full screen keeps the screen's. Reports it if it changed.
    pub(super) fn resize(&mut self, window: NodeId, size: Size) {
        let Some(node) = self.nodes.get_mut(&window) else { return };
        // A maximized window takes it when it's restored, as GTK's does.
        if node.restored.is_some() {
            node.restored = Some(at_least_min(node, size));
            return;
        }
        if node.windowed.is_some() {
            return;
        }
        let size = at_least_min(node, size);
        if node.frame.size != size {
            node.frame.size = size;
            self.emit(window, UiEvent::WindowResized(size));
        }
    }

    /// A window fills the screen, or goes back to its size before.
    pub(super) fn fill_screen(&mut self, window: NodeId, on: bool) {
        let Some(node) = self.nodes.get_mut(&window) else { return };
        if on == node.windowed.is_some() {
            return;
        }
        let size = if on {
            node.windowed = Some(node.frame.size);
            SCREEN
        } else {
            node.windowed.take().unwrap_or(node.frame.size)
        };
        node.frame.size = size;
        self.emit(window, UiEvent::WindowResized(size));
    }

    /// A window fills the work area, or goes back to its size before.
    pub(super) fn maximize(&mut self, window: NodeId, on: bool) {
        let Some(node) = self.nodes.get_mut(&window) else { return };
        if on == node.restored.is_some() {
            return;
        }
        let size = if on {
            node.restored = Some(node.frame.size);
            WORK_AREA
        } else {
            node.restored.take().unwrap_or(node.frame.size)
        };
        // One in full screen keeps the screen until it leaves it.
        if node.windowed.is_some() {
            node.windowed = Some(size);
            return;
        }
        node.frame.size = size;
        self.emit(window, UiEvent::WindowResized(size));
    }

    /// Reports a GPU surface's size in pixels (its frame at the scale
    /// factor, rounded, as platforms round), if it changed.
    pub(super) fn size_surface(&self, id: NodeId) {
        let node = &self.nodes[&id];
        let Some(surface) = &node.surface else { return };
        let scale = self.metrics.scale_factor;
        let size = SurfaceSize {
            width: (node.frame.width() * scale).round() as u32,
            height: (node.frame.height() * scale).round() as u32,
            scale,
        };
        if surface.set_size(size) {
            self.emit(id, UiEvent::SurfaceResized(size));
        }
    }
}

/// A window's size, grown to its minimum, which goes no larger than the
/// screen.
pub(super) fn at_least_min(node: &HeadlessNode, size: Size) -> Size {
    let min = find_prop!(node.props, MinSize).unwrap_or(Size::ZERO);
    let min = Size::new(min.width.min(SCREEN.width), min.height.min(SCREEN.height));
    Size::new(size.width.max(min.width), size.height.max(min.height))
}
