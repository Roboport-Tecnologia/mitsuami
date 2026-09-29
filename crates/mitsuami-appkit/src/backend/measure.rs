//! Measuring widgets: their natural sizes, as AppKit gives them.

use mitsuami_core::backend::{AvailableSpace, MeasureRequest};
use mitsuami_core::{NodeId, Orientation, Size};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::group::group_natural_size;
use super::images::FILE_ICON_SIZE;
use super::{State, Widget};

pub(super) fn measure(state: &State, id: NodeId, request: MeasureRequest) -> Size {
    let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
    let natural = match &node.widget {
        Widget::Label(label) => {
            // TODO: min-content (longest word) — until then text never
            // shrinks below its single-line width in flex rows.
            let width = request.known_width.map(f64::from).or(match request.available_width {
                AvailableSpace::Definite(w) => Some(w as f64),
                AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
            });
            let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width.unwrap_or(1.0e7), 1.0e7));
            match label.cell() {
                Some(cell) => ceil_size(cell.cellSizeForBounds(bounds)),
                None => Size::ZERO,
            }
        }
        Widget::Field(field) => {
            let intrinsic = field.intrinsicContentSize();
            Size::new(
                if intrinsic.width > 0.0 { intrinsic.width.ceil() as f32 } else { 200.0 },
                intrinsic.height.ceil() as f32,
            )
        }
        // A borderless button's intrinsic size leaves out part of its
        // image (a trash symbol measured 15 × 9); it's at least that big.
        Widget::Button(v) => {
            let size = ceil_size(v.intrinsicContentSize());
            match v.image() {
                Some(image) if !v.isBordered() => {
                    let image = ceil_size(image.size());
                    Size::new(size.width.max(image.width), size.height.max(image.height))
                }
                _ => size,
            }
        }
        Widget::Checkbox(v) => ceil_size(v.intrinsicContentSize()),
        Widget::RadioGroup(group) => ceil_size(group.stack.fittingSize()),
        Widget::Switch(v) => ceil_size(v.intrinsicContentSize()),
        // AppKit sizes pop-up buttons for their widest item.
        Widget::Select(v) => ceil_size(v.intrinsicContentSize()),
        Widget::MenuButton { popup, .. } => ceil_size(popup.intrinsicContentSize()),
        // No natural width: they're as wide as the layout makes them.
        Widget::Slider { slider, .. } => intrinsic(slider),
        Widget::NumberInput(n) => n.natural_size(),
        Widget::TextArea(area) => area.natural_size(state.mtm),
        // The image's size in points; nothing shown, none.
        Widget::Image(view) | Widget::Icon(view) => view.image().map_or(Size::ZERO, |image| ceil_size(image.size())),
        // A square, whatever the file.
        Widget::FileIcon(_) => {
            let side = node.icon_size.unwrap_or(FILE_ICON_SIZE);
            Size::new(side, side)
        }
        Widget::Progress(p) => intrinsic(p),
        Widget::Spinner { indicator, .. } => intrinsic(indicator),
        // As thick as AppKit makes it (a 1-point line); the layout
        // gives its length.
        Widget::Separator(line) => {
            let size = intrinsic(line);
            match node.orientation.unwrap_or_default() {
                Orientation::Horizontal => Size::new(0.0, size.height.max(1.0)),
                Orientation::Vertical => Size::new(size.width.max(1.0), 0.0),
            }
        }
        // As large as the layout makes it.
        Widget::GpuSurface(_) => Size::ZERO,
        Widget::Custom { view, render, props } => {
            render.measure(view, props.props(), &request).unwrap_or_else(|| intrinsic(view))
        }
        Widget::Native { view, measure, .. } => match measure {
            Some(measure) => measure(view, &request),
            None => intrinsic(view),
        },
        Widget::Tabs(tabs) => tabs.natural_size(crate::tabs::insets(state.mtm)),
        // Its title and border, empty.
        Widget::Group { frame, .. } => group_natural_size(frame),
        // Measured by the core, or never (the sidebar is the window's).
        Widget::Drawn { .. }
        | Widget::Window { .. }
        | Widget::Host(_)
        | Widget::Scroll(_)
        | Widget::List(_)
        | Widget::Sidebar(_) => Size::ZERO,
    };
    Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
}

fn ceil_size(size: NSSize) -> Size {
    Size::new(size.width.ceil() as f32, size.height.ceil() as f32)
}

/// `intrinsicContentSize`, with "no intrinsic size" (-1) as zero.
fn intrinsic(view: &NSView) -> Size {
    let size = view.intrinsicContentSize();
    ceil_size(NSSize::new(size.width.max(0.0), size.height.max(0.0)))
}
