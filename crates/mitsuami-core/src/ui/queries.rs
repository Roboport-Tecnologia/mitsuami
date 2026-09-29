//! Reading the tree: nodes, their frames and windows, and inspection.

use crate::backend::NativeState;
use crate::geometry::{Point, Rect, Size};
use crate::services::reply_future;
use crate::widget::{NodeId, Prop, WidgetKind};

use super::{Inner, Node, NodeInfo, Ui, in_chrome};

impl Ui {
    pub fn windows(&self) -> Vec<NodeId> {
        self.inner.borrow().windows.clone()
    }

    pub fn exists(&self, id: NodeId) -> bool {
        self.inner.borrow().nodes.contains_key(&id)
    }

    pub fn kind(&self, id: NodeId) -> Option<WidgetKind> {
        self.inner.borrow().nodes.get(&id).map(|n| n.kind)
    }

    pub fn props(&self, id: NodeId) -> Vec<Prop> {
        self.inner.borrow().nodes.get(&id).map(|n| n.props.clone()).unwrap_or_default()
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.inner.borrow().nodes.get(&id).and_then(|n| n.parent)
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.inner.borrow().nodes.get(&id).map(|n| n.children.clone()).unwrap_or_default()
    }

    pub fn native_children(&self, id: NodeId) -> Vec<NodeId> {
        self.inner.borrow().nodes.get(&id).map(|n| n.native_children.clone()).unwrap_or_default()
    }

    /// Frame relative to the native parent, as last sent to the backend.
    /// A list's rows are where the platform put them, at the size sent.
    pub fn frame(&self, id: NodeId) -> Option<Rect> {
        let inner = self.inner.borrow();
        inner.nodes.contains_key(&id).then(|| inner.placed_frame(id))
    }

    /// Frame in the coordinates of the node's window.
    pub fn window_frame(&self, id: NodeId) -> Option<Rect> {
        let inner = self.inner.borrow();
        let node = inner.nodes.get(&id)?;
        if node.kind == WidgetKind::Window {
            return Some(node.frame);
        }
        Some(inner.placed_frame(id).offset(inner.window_origin(id)?))
    }

    /// The part of a node that can be seen: its window frame, clipped by
    /// every enclosing scroll view and by the window (or, in the toolbar,
    /// by its toolbar item; the sidebar, by itself). `None` if nothing is.
    pub fn visible_rect(&self, id: NodeId) -> Option<Rect> {
        let window = self.window_of(id)?;
        let size = self.window_size(window)?;
        let mut visible = self.window_frame(id)?;
        let mut clip = Rect::new(0.0, 0.0, size.width, size.height);
        let mut current = Some(id);
        while let Some(node) = current {
            match self.kind(node)? {
                kind if in_chrome(kind) => clip = self.window_frame(node)?,
                kind if kind.scrolls() && node != id => visible = visible.intersection(&self.window_frame(node)?)?,
                _ => {}
            }
            current = self.inner.borrow().nodes.get(&node)?.native_parent;
        }
        visible.intersection(&clip)
    }

    /// The window a node is attached to, if any.
    pub fn window_of(&self, id: NodeId) -> Option<NodeId> {
        self.inner.borrow().window_of(id)
    }

    pub fn window_size(&self, window: NodeId) -> Option<Size> {
        self.inner.borrow().nodes.get(&window).map(|n| n.window_size)
    }

    pub fn native_state(&self, id: NodeId) -> Option<NativeState> {
        self.inner.borrow().backend.native_state(id)
    }

    /// An offscreen screenshot of a window or node.
    pub fn capture(
        &self,
        id: NodeId,
    ) -> impl std::future::Future<Output = Result<crate::backend::Image, crate::backend::CaptureError>> + use<> {
        let (reply, image) = reply_future();
        self.inner.borrow_mut().backend.capture(id, reply);
        image
    }

    /// The native tree under `id`, with window-coordinate frames.
    pub fn inspect(&self, id: NodeId) -> Option<NodeInfo> {
        let inner = self.inner.borrow();
        let origin = inner.window_origin(id)?;
        Some(inner.inspect(id, origin))
    }
}

impl Inner {
    pub(super) fn window_of(&self, id: NodeId) -> Option<NodeId> {
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.nodes.get(&node_id)?;
            if node.kind == WidgetKind::Window {
                return Some(node_id);
            }
            current = node.parent;
        }
        None
    }

    /// Window coordinates of the point `id`'s children are positioned
    /// from: its own top-left, moved by its scroll offset.
    fn content_origin(&self, id: NodeId) -> Point {
        let node = &self.nodes[&id];
        if node.kind == WidgetKind::Window {
            return Point::ZERO;
        }
        let parent = node.native_parent.map_or(Point::ZERO, |p| self.content_origin(p));
        let origin = self.placed_frame(id).origin;
        Point::new(parent.x + origin.x - node.scroll_offset.x, parent.y + origin.y - node.scroll_offset.y)
    }

    /// Window coordinates of the origin `id`'s frame is relative to.
    fn window_origin(&self, id: NodeId) -> Option<Point> {
        Some(self.nodes.get(&id)?.native_parent.map_or(Point::ZERO, |p| self.content_origin(p)))
    }

    /// A node's frame in its native parent. A list's rows and a window's
    /// toolbar items are where the platform placed them (as its
    /// `native_state` says), at the size the core sent; toolbar items the
    /// platform hides are empty. A window's sidebar is where the platform
    /// placed it, at the size it gave it. So is a tab view's page, at the
    /// size the core gave it; the pages it doesn't show are empty.
    pub(super) fn placed_frame(&self, id: NodeId) -> Rect {
        let node = &self.nodes[&id];
        let placed_natively = in_chrome(node.kind)
            || node.native_parent.is_some_and(|p| matches!(self.nodes[&p].kind, WidgetKind::List | WidgetKind::Tabs));
        if !placed_natively {
            return node.frame;
        }
        if self.is_hidden_page(id) {
            return Rect::ZERO;
        }
        let native = self.backend.native_state(id).map(|s| s.frame);
        // A toolbar may hide an item that doesn't fit (in an overflow
        // menu, on AppKit and WinUI), and a window its sidebar (collapsed,
        // or a page of its own in a narrow window): it isn't shown at all.
        if in_chrome(node.kind) && native.is_some_and(|f| f.size.is_empty()) {
            return Rect::ZERO;
        }
        if node.kind == WidgetKind::Sidebar {
            return native.unwrap_or(Rect::ZERO);
        }
        Rect { origin: native.map_or(node.frame.origin, |f| f.origin), size: node.frame.size }
    }

    pub(super) fn child_origin(node: &Node, frame: Rect) -> Point {
        match node.kind {
            WidgetKind::Window => Point::ZERO,
            _ => Point::new(frame.origin.x - node.scroll_offset.x, frame.origin.y - node.scroll_offset.y),
        }
    }

    fn inspect(&self, id: NodeId, parent_origin: Point) -> NodeInfo {
        let node = &self.nodes[&id];
        let frame =
            if node.kind == WidgetKind::Window { node.frame } else { self.placed_frame(id).offset(parent_origin) };
        let origin = Inner::child_origin(node, frame);
        NodeInfo {
            id,
            kind: node.kind,
            props: node.props.clone(),
            test_id: node.test_id.clone(),
            frame,
            children: node.native_children.iter().map(|c| self.inspect(*c, origin)).collect(),
        }
    }
}
