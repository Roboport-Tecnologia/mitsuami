//! Capturing a node as an image.

use mitsuami_core::backend::{CaptureError, Image};
use mitsuami_core::services::Reply;
use mitsuami_core::{NodeId, Point, Rect};

use super::{KirigamiBackend, Widget, size_of};

impl KirigamiBackend {
    /// Renders the window's scene right away, and crops it to the node.
    pub(super) fn capture_node(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        let target = {
            let state = self.state.borrow();
            let Some(node) = state.nodes.get(&id) else { return reply(Err(CaptureError::UnknownNode)) };
            let mut top = id;
            while let Some(parent) = state.nodes.get(&top).and_then(|n| n.parent) {
                top = parent;
            }
            let window = match state.nodes.get(&top).map(|n| &n.widget) {
                Some(Widget::Window { root }) => root.window,
                _ => return reply(Err(CaptureError::Failed("the node is not in a window".into()))),
            };
            let item = node.widget.item();
            let size = match &node.widget {
                Widget::Window { root } => root.size.get(),
                _ => size_of(item),
            };
            let origin = item.map_to_scene(Point::new(0.0, 0.0));
            (window, Rect::new(origin.x, origin.y, size.width, size.height))
        };
        let (window, rect) = target;
        reply(match window.grab(Some(rect)) {
            Some((rgba, width, height, scale_factor)) => Ok(Image { width, height, scale_factor, rgba }),
            None => Err(CaptureError::Failed("Qt rendered nothing".into())),
        });
    }
}
