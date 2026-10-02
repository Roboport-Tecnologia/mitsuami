//! Scroll views and lists: scroll offsets and scrolling into view.

use crate::command::Command;
use crate::geometry::{Point, Size};
use crate::widget::{NodeId, RowKey, WidgetKind};

use super::{Inner, Ui};

impl Ui {
    /// Current scroll offset of a `ScrollView` or `List`.
    pub fn scroll_offset(&self, id: NodeId) -> Option<Point> {
        let inner = self.inner.borrow();
        let node = inner.nodes.get(&id)?;
        node.kind.scrolls().then_some(node.scroll_offset)
    }

    /// Scrolls a `ScrollView` or `List`, clamping to its content (a list
    /// clamps it itself: its content is the platform's).
    pub fn scroll_to(&self, id: NodeId, offset: Point) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(offset) = inner.clamp_scroll(id, offset) else { return };
            let node = inner.nodes.get_mut(&id).unwrap();
            if node.scroll_offset == offset {
                return;
            }
            node.scroll_offset = offset;
            inner.pending.push(Command::ScrollTo { id, offset });
        }
        self.changed();
    }

    /// Scrolls every enclosing `ScrollView` and `List` just enough to show
    /// the node.
    pub fn scroll_into_view(&self, id: NodeId) {
        let mut target = id;
        loop {
            let scroll_view = {
                let inner = self.inner.borrow();
                let mut current = inner.nodes.get(&target).and_then(|n| n.native_parent);
                while let Some(ancestor) = current {
                    if inner.nodes[&ancestor].kind.scrolls() {
                        break;
                    }
                    current = inner.nodes[&ancestor].native_parent;
                }
                current
            };
            let Some(scroll_view) = scroll_view else { return };
            // In a list, show the target's row: where rows are is the
            // platform's business.
            let row = {
                let inner = self.inner.borrow();
                let mut current = Some(target);
                while let Some(node) = current.and_then(|c| inner.nodes.get(&c)) {
                    if node.native_parent == Some(scroll_view) {
                        break;
                    }
                    current = node.native_parent;
                }
                inner.nodes[&scroll_view].kind.has_rows().then(|| {
                    current.and_then(|host| {
                        let props = &inner.nodes[&host].props;
                        crate::find_prop!(props, Row).or_else(|| crate::find_prop!(props, Cell).map(|cell| cell.row))
                    })
                })
            };
            if let Some(row) = row {
                if let Some(row) = row {
                    self.scroll_to_row(scroll_view, row);
                }
                target = scroll_view;
                continue;
            }
            let (Some(rect), Some(viewport)) = (self.window_frame(target), self.viewport(scroll_view)) else {
                return;
            };
            let offset = self.scroll_offset(scroll_view).unwrap_or_default();
            // The target in content coordinates, and what currently shows.
            let x = rect.x() - viewport.x() + offset.x;
            let y = rect.y() - viewport.y() + offset.y;
            let next = Point::new(
                fit(x, rect.width(), offset.x, viewport.width()),
                fit(y, rect.height(), offset.y, viewport.height()),
            );
            self.scroll_to(scroll_view, next);
            target = scroll_view;
        }
    }

    /// Scrolls a `List` just enough to show a row, mounted or not. The
    /// platform scrolls and reports it.
    pub(crate) fn scroll_to_row(&self, id: NodeId, row: RowKey) {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.nodes.get(&id).is_none_or(|n| !n.kind.has_rows()) {
                return;
            }
            inner.pending.push(Command::ScrollToRow { id, row });
        }
        self.changed();
    }
}

impl Inner {
    fn clamp_scroll(&self, id: NodeId, offset: Point) -> Option<Point> {
        let node = self.nodes.get(&id)?;
        let (axes, content) = match node.kind {
            WidgetKind::ScrollView => (
                crate::find_prop!(node.props, ScrollAxes).unwrap_or_default(),
                node.native_children.first().map_or(Size::ZERO, |c| self.nodes[c].frame.size),
            ),
            WidgetKind::List => (crate::ScrollAxes::Vertical, Size::new(node.frame.width(), f32::INFINITY)),
            // Its columns can be wider than it: the platform clamps.
            WidgetKind::Table => (crate::ScrollAxes::Both, Size::new(f32::INFINITY, f32::INFINITY)),
            _ => return None,
        };
        let viewport = node.frame.inset(node.viewport_insets).size;
        let clamp = |v: f32, content: f32, viewport: f32, enabled: bool| {
            if enabled { v.clamp(0.0, (content - viewport).max(0.0)) } else { 0.0 }
        };
        Some(Point::new(
            clamp(offset.x, content.width, viewport.width, axes.horizontal()),
            clamp(offset.y, content.height, viewport.height, axes.vertical()),
        ))
    }
}

/// Where to scroll along one axis so `len` at `start` shows, moving as
/// little as possible from `current` with `visible` in view.
fn fit(start: f32, len: f32, current: f32, visible: f32) -> f32 {
    if start < current {
        start
    } else if start + len > current + visible {
        (start + len - visible).min(start)
    } else {
        current
    }
}
