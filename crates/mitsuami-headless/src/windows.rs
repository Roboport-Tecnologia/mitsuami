//! Windows' sizes (resized, in full screen, maximized) and GPU surfaces' sizes.

use mitsuami_core::{NodeId, Placement, Point, Prop, Size, SurfaceSize, UiEvent, WidgetKind, WindowFrame, find_prop};

use super::metrics::{BAR, NEW_WINDOW_AT, SCREEN, TITLE_BAR, WORK_AREA};
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
        // One in full screen keeps the screen, and takes it when it leaves.
        if node.windowed.is_some() {
            node.windowed = Some(at_least_min(node, size));
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

    /// Places the windows made since the last batch, as they show: where
    /// their `Frame` puts them, else as their `Placement` says.
    pub(super) fn place_new_windows(&mut self) {
        let new: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind == WidgetKind::Window && n.position.is_none())
            .map(|(id, _)| *id)
            .collect();
        for window in new {
            let node = &self.nodes[&window];
            let frame = find_prop!(node.props, Frame);
            let centred = find_prop!(node.props, Placement) == Some(Placement::Centred) || node.centre_on_place;
            self.nodes.get_mut(&window).unwrap().position = Some(NEW_WINDOW_AT);
            match frame {
                Some(frame) => self.place(window, frame),
                None if centred => self.centre(window),
                None => {}
            }
        }
    }

    /// Moves and sizes a window to a frame, onto the work area: no larger
    /// than it, and moved into it.
    pub(super) fn place(&mut self, window: NodeId, frame: WindowFrame) {
        // One whose content sets its height keeps it: the frame gives its width.
        let height = match self.nodes.get(&window) {
            Some(node) if find_prop!(node.props, HeightFollowsContent) == Some(true) => restored_size(node).height,
            _ => frame.size.height,
        };
        let size = Size::new(frame.size.width.min(WORK_AREA.width), height.min(WORK_AREA.height));
        self.resize(window, size);
        let Some(node) = self.nodes.get_mut(&window) else { return };
        let size = restored_size(node);
        let at = frame.position.or(node.position).unwrap_or(NEW_WINDOW_AT);
        node.position = Some(Point::new(
            at.x.clamp(0.0, (SCREEN.width - size.width).max(0.0)),
            at.y.clamp(BAR, (SCREEN.height - size.height - TITLE_BAR).max(BAR)),
        ));
    }

    /// Centres a window on the work area.
    pub(super) fn centre(&mut self, window: NodeId) {
        let Some(node) = self.nodes.get_mut(&window) else { return };
        let size = restored_size(node);
        node.position = Some(Point::new(
            ((SCREEN.width - size.width) / 2.0).max(0.0),
            BAR + ((SCREEN.height - BAR - size.height - TITLE_BAR) / 2.0).max(0.0),
        ));
    }

    /// Reports each placed window's restored frame that isn't the `Frame`
    /// it has, as platforms report the user's moves and resizes, and keeps
    /// it as the window's prop (its native state).
    pub(super) fn report_frames(&mut self) {
        let windows: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind == WidgetKind::Window && n.position.is_some())
            .map(|(id, _)| *id)
            .collect();
        for window in windows {
            let node = &self.nodes[&window];
            let frame = WindowFrame { position: node.position, size: restored_size(node) };
            if find_prop!(node.props, Frame) != Some(frame) {
                self.set_prop(window, Prop::Frame(frame));
                self.emit(window, UiEvent::FrameChanged(frame));
            }
        }
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

/// A window's size when it's neither maximized nor in full screen.
pub(super) fn restored_size(node: &HeadlessNode) -> Size {
    node.restored.or(node.windowed).unwrap_or(node.frame.size)
}

/// A window's size, grown to its minimum, which goes no larger than the
/// screen.
pub(super) fn at_least_min(node: &HeadlessNode, size: Size) -> Size {
    let min = find_prop!(node.props, MinSize).unwrap_or(Size::ZERO);
    let min = Size::new(min.width.min(SCREEN.width), min.height.min(SCREEN.height));
    Size::new(size.width.max(min.width), size.height.max(min.height))
}
