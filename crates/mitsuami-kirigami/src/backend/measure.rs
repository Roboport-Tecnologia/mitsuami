//! Measuring a node's widget.

use mitsuami_core::backend::{AvailableSpace, MeasureRequest};
use mitsuami_core::{ImageSource, NodeId, Size};

use crate::ffi::QmlObject;

use super::{KirigamiBackend, Widget, strip};

impl KirigamiBackend {
    pub(super) fn measure_node(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        match &node.widget {
            Widget::Custom { item, render, props } => {
                render.measure(*item, props.props(), &request).unwrap_or_else(|| measure_item(*item, false, request))
            }
            Widget::Native { item, measure: Some(measure), .. } => measure(*item, &request),
            // Pixels are as large as they say, over their scale; files as
            // Qt reads them (nothing when missing or unreadable).
            Widget::Image { source: Some(ImageSource::Pixels(pixels)), .. } => {
                let natural = pixels.size();
                Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
            }
            // As large as the layout makes it.
            Widget::GpuSurface(_) => Size::new(request.known_width.unwrap_or(0.0), request.known_height.unwrap_or(0.0)),
            // With no page: its bar's size. Qt sizes a bar's tabs when it
            // polishes it, before a frame.
            Widget::Tabs { root, .. } => {
                let bar = strip(*root);
                root.invoke("mitsuamiPolishStrip");
                Size::new(
                    request.known_width.unwrap_or(root.real("mitsuamiStripWidth").ceil() as f32),
                    request.known_height.unwrap_or(bar.real("implicitHeight").ceil() as f32),
                )
            }
            // Its buttons, down the column: a layout sizes itself when
            // it's polished, before a frame.
            Widget::RadioGroup(group) => {
                group.invoke("ensurePolished");
                measure_item(*group, false, request)
            }
            // Empty, with its title: the box's own implicit size, which is
            // at least its title's width and its paddings.
            Widget::Group { group, .. } => {
                group.invoke("ensurePolished");
                Size::new(
                    request.known_width.unwrap_or(group.real("implicitWidth").ceil() as f32),
                    request.known_height.unwrap_or(group.real("implicitHeight").ceil() as f32),
                )
            }
            // Measured by the core, or never (the sidebar is the window's).
            Widget::Drawn { .. }
            | Widget::Window { .. }
            | Widget::Host(_)
            | Widget::Scroll { .. }
            | Widget::List(_)
            | Widget::Sidebar { .. } => Size::ZERO,
            widget => measure_item(widget.item(), matches!(widget, Widget::Label(_)), request),
        }
    }
}

/// Implicit sizes, except text: it wraps to the space it's offered, down to
/// its longest word.
fn measure_item(item: QmlObject, wraps: bool, request: MeasureRequest) -> Size {
    let natural = Size::new(item.real("implicitWidth") as f32, item.real("implicitHeight") as f32);
    if !wraps {
        return Size::new(
            request.known_width.unwrap_or(natural.width.ceil()),
            request.known_height.unwrap_or(natural.height.ceil()),
        );
    }
    // Word-wrapped at width 1, a label is as wide as its longest word.
    let frame_width = item.real("width");
    let min_content = || {
        item.set_real("width", 1.0);
        item.real("contentWidth").ceil() as f32
    };
    let width = match (request.known_width, request.available_width) {
        (Some(known), _) => known,
        (None, AvailableSpace::MaxContent) => natural.width.ceil(),
        (None, AvailableSpace::MinContent) => min_content(),
        (None, AvailableSpace::Definite(available)) => {
            let natural = natural.width.ceil();
            if natural <= available { natural } else { available.floor().max(min_content()) }
        }
    };
    // An eliding label also drops the lines past its height, and its
    // frame's may be less than it needs (0 before its first layout).
    let frame_height = item.real("height");
    item.set_real("height", f32::MAX as f64);
    item.set_real("width", width as f64);
    let height = item.real("implicitHeight").ceil() as f32;
    item.set_real("width", frame_width);
    item.set_real("height", frame_height);
    Size::new(width, request.known_height.unwrap_or(height))
}
