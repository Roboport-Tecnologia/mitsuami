//! Measuring leaves.

use gtk::prelude::*;
use mitsuami_core::backend::{AvailableSpace, MeasureRequest};
use mitsuami_core::{ImageSource, NodeId, Size};

use super::slider::set_travel;
use super::text::TEXT_AREA_WIDTH;
use super::{GtkBackend, Widget};

/// Natural sizes, except text: it wraps to the space it's offered, down to
/// its longest word.
fn measure_widget(widget: &gtk::Widget, wraps: bool, request: MeasureRequest) -> Size {
    let (min_width, natural_width, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
    let width = match (request.known_width, wraps) {
        (Some(known), _) => known.round() as i32,
        (None, false) => natural_width,
        (None, true) => match request.available_width {
            AvailableSpace::Definite(available) => natural_width.min(available.floor() as i32),
            AvailableSpace::MinContent => min_width,
            AvailableSpace::MaxContent => natural_width,
        },
    };
    // GTK can't measure narrower than the minimum (it warns); the text
    // overflows instead.
    let width = width.max(min_width);
    let height = widget.measure(gtk::Orientation::Vertical, width).1;
    Size::new(request.known_width.unwrap_or(width as f32), request.known_height.unwrap_or(height as f32))
}

impl GtkBackend {
    pub(super) fn measure_node(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        match &node.widget {
            Widget::Custom { widget, render, props } => render
                .measure(widget, props.props(), &request)
                .unwrap_or_else(|| measure_widget(widget, false, request)),
            Widget::Native { widget, measure: Some(measure), .. } => measure(widget, &request),
            // Marks make a scale thicker, and whether they fit depends on
            // its length: decide for the length being measured.
            Widget::Slider { scale, steps } => {
                let length = match scale.orientation() {
                    gtk::Orientation::Vertical => request.known_height,
                    _ => request.known_width,
                };
                if let Some(length) = length {
                    set_travel(scale, steps, length);
                }
                measure_widget(scale.upcast_ref(), false, request)
            }
            // A texture measures at its pixel count; pixels made at a scale
            // take that many fewer points. Files measure as GTK reads them.
            Widget::Picture { source: Some(ImageSource::Pixels(pixels)), .. } => {
                let size = pixels.size();
                Size::new(request.known_width.unwrap_or(size.width), request.known_height.unwrap_or(size.height))
            }
            // Its heading and the card's margins, empty.
            Widget::Group(group) => {
                let title = group.title();
                let insets = crate::group::insets(!title.is_empty());
                let heading = if title.is_empty() {
                    0.0
                } else {
                    group.heading.measure(gtk::Orientation::Horizontal, -1).1 as f32
                };
                Size::new(heading + insets.left + insets.right, insets.top + insets.bottom)
            }
            // A text field's width, and its lines of the view's font inside
            // its margins and the frame.
            Widget::TextArea { scrolled, view, lines, .. } => {
                let line = view.create_pango_layout(Some("X")).pixel_size().1;
                scrolled.set_min_content_height(line * *lines as i32 + view.top_margin() + view.bottom_margin());
                let width = request.known_width.unwrap_or(TEXT_AREA_WIDTH);
                let height = scrolled.measure(gtk::Orientation::Vertical, width.round() as i32).1;
                Size::new(width, request.known_height.unwrap_or(height as f32))
            }
            // As large as the layout makes it.
            Widget::GpuSurface(_) => Size::new(request.known_width.unwrap_or(0.0), request.known_height.unwrap_or(0.0)),
            // Measured by the core, or never (the sidebar is the window's).
            Widget::Drawn { .. }
            | Widget::Window(_)
            | Widget::Host(_)
            | Widget::Scroll { .. }
            | Widget::List(_)
            | Widget::Sidebar(_) => Size::ZERO,
            widget => measure_widget(widget.widget(), matches!(widget, Widget::Label(_)), request),
        }
    }
}
