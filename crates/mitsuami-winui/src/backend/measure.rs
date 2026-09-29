//! Measuring nodes.

use mitsuami_core::backend::{AvailableSpace, MeasureRequest};
use mitsuami_core::{ImageSource, NodeId, Size};
use windows_core::Interface;

use super::fields::measure_lines;
use super::{Widget, WinUiBackend, ok};
use crate::bindings as w;

const NAN_SIZE: f64 = f64::NAN;

/// Measures with the frame size we imposed lifted: XAML's `Measure` honours
/// an explicit `Width`/`Height`, which would hide the content's own size.
pub(crate) fn measure_element(element: &w::UIElement, available: w::Size) -> w::Size {
    let fe: w::IFrameworkElement = ok(element.cast(), "cast to FrameworkElement");
    let (width, height) = (fe.Width().unwrap_or(NAN_SIZE), fe.Height().unwrap_or(NAN_SIZE));
    _ = fe.SetWidth(NAN_SIZE);
    _ = fe.SetHeight(NAN_SIZE);
    let ui: w::IUIElement = ok(element.cast(), "cast to UIElement");
    _ = ui.Measure(available);
    let desired = ui.DesiredSize().unwrap_or_default();
    _ = fe.SetWidth(width);
    _ = fe.SetHeight(height);
    desired
}

fn ceil(size: w::Size) -> Size {
    Size::new(size.width.ceil(), size.height.ceil())
}

impl WinUiBackend {
    pub(super) fn measure_node(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
        let natural = match &node.widget {
            Widget::Label(_) => {
                // TODO: min-content (longest word). XAML wraps per character
                // at width 0, so min-content uses max-content for now.
                let width = request.known_width.or(match request.available_width {
                    AvailableSpace::Definite(w) => Some(w),
                    AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                });
                ceil(measure_element(&node.element, w::Size { width: width.unwrap_or(f32::INFINITY), ..infinite }))
            }
            Widget::Field(_) | Widget::Password(_) | Widget::Search(_) => {
                // Text boxes have no useful intrinsic width.
                let size = ceil(measure_element(&node.element, infinite));
                Size::new(size.width.max(200.0), size.height)
            }
            // A text field's width, and as tall as an empty text box in
            // its font with that many lines: a text box is as big as its
            // text, which mustn't count.
            Widget::TextArea { field, lines } => {
                let size = state
                    .window_of(id)
                    .and_then(|parts| measure_lines(field, &parts.text_probe, *lines).ok())
                    .unwrap_or_default();
                Size::new(size.width.ceil().max(200.0), size.height.ceil())
            }
            Widget::Button(_)
            | Widget::Toggle(_)
            | Widget::MenuButton(_)
            | Widget::Checkbox(_)
            | Widget::Switch(_)
            | Widget::Select(_)
            | Widget::RadioGroup(_)
            | Widget::Slider { .. }
            | Widget::Number { .. }
            | Widget::Progress(_)
            | Widget::Spinner(_)
            | Widget::Icon(_) => ceil(measure_element(&node.element, infinite)),
            // To the nearest, not up: XAML rounds its 1 epx to whole
            // pixels (1.33 at 150 %), and up would make it 2.
            Widget::Separator(_) => {
                let size = measure_element(&node.element, infinite);
                Size::new(size.width.round(), size.height.round())
            }
            // Pixels over their scale. A file at its pixel count in
            // effective pixels, as XAML shows it; nothing until it's
            // decoded, or if it can't be.
            Widget::Image { source: Some(ImageSource::Pixels(pixels)), .. } => pixels.size(),
            Widget::Image { bitmap: Some(bitmap), failed, .. } if !failed.get() => {
                let bitmap: w::IBitmapSource = ok(bitmap.cast(), "cast to BitmapSource");
                Size::new(
                    bitmap.PixelWidth().unwrap_or(0).max(0) as f32,
                    bitmap.PixelHeight().unwrap_or(0).max(0) as f32,
                )
            }
            Widget::Image { .. } => Size::ZERO,
            // As large as the layout makes it.
            Widget::GpuSurface(_) => Size::ZERO,
            Widget::Custom { render, props } => render
                .measure(node.control(), props.props(), &request)
                .unwrap_or_else(|| ceil(measure_element(&node.element, infinite))),
            Widget::Native { measure: Some(measure), .. } => measure(node.control(), &request),
            Widget::Native { measure: None, .. } => ceil(measure_element(&node.element, infinite)),
            // Its bar: the core adds the pages.
            Widget::Tabs(tabs) => ceil(tabs.strip()),
            // Its heading and card: the core adds the content.
            Widget::Group(group) => ceil(group.strip()),
            // Measured by the core, or never (the sidebar is the window's).
            Widget::Drawn { .. }
            | Widget::Window(_)
            | Widget::Host(_)
            | Widget::Scroll(_)
            | Widget::List(_)
            | Widget::Sidebar(_) => Size::ZERO,
        };
        Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
    }
}
