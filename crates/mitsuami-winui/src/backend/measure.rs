//! Measuring nodes.

use mitsuami_core::backend::{AvailableSpace, MeasureRequest};
use mitsuami_core::{ImageSource, NodeId, Size};
use windows_core::Interface;

use super::fields::{inner_text_box, measure_lines};
use super::{R, Widget, WinUiBackend, ok};
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

/// A number box with room for its clear button. Its text box shows the
/// button in a column of its own while it has focus, and a frame measured
/// without it would leave the button over the value. While the button
/// shows, XAML's measure has it already; while it's hidden, it's shown for
/// its own measure and hidden again, as the text box's state had it.
fn measure_number_box(element: &w::UIElement) -> w::Size {
    let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
    // Measuring applies the templates the button is in.
    let size = measure_element(element, infinite);
    let Some(clear) = inner_text_box(element).and_then(|field| clear_button(&field)) else { return size };
    if clear.Visibility().is_ok_and(|v| v == w::Visibility::Visible) {
        return size;
    }
    _ = clear.SetVisibility(w::Visibility::Visible);
    _ = clear.Measure(infinite);
    let button = clear.DesiredSize().unwrap_or_default();
    _ = clear.SetVisibility(w::Visibility::Collapsed);
    w::Size { width: size.width + button.width, ..size }
}

/// The button in a text box's template: its clear button.
fn clear_button(field: &w::ITextBox) -> Option<w::IUIElement> {
    let mut queue = std::collections::VecDeque::from([field.cast::<w::DependencyObject>().ok()?]);
    while let Some(node) = queue.pop_front() {
        if node.cast::<w::IButton>().is_ok() {
            return node.cast().ok();
        }
        for i in 0..w::VisualTreeHelper::GetChildrenCount(&node).unwrap_or(0) {
            if let Ok(child) = w::VisualTreeHelper::GetChild(&node, i) {
                queue.push_back(child);
            }
        }
    }
    None
}

/// How narrow text can be: a line cut off at one line, its ellipsis; text
/// that wraps, its longest word, as GTK's and Qt's labels measure. XAML has
/// no such measure: a text block measured narrower than a word wraps it by
/// letters, and asks for no more than it's given. So the pieces are
/// measured on their own, in a text block with the label's font.
fn min_content_width(label: &w::TextBlock) -> R<f32> {
    let text: w::ITextBlock = label.cast()?;
    let probe = w::TextBlock::new()?;
    let piece: w::ITextBlock = probe.cast()?;
    piece.SetFontFamily(&text.FontFamily()?)?;
    piece.SetFontSize(text.FontSize()?)?;
    piece.SetFontWeight(text.FontWeight()?)?;
    piece.SetFontStyle(text.FontStyle()?)?;
    piece.SetCharacterSpacing(text.CharacterSpacing()?)?;
    let element: w::UIElement = probe.cast()?;
    let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
    let width = |part: &str| -> R<f32> {
        piece.SetText(part)?;
        Ok(measure_element(&element, infinite).width)
    };
    if text.MaxLines()? == 1 {
        return width("\u{2026}");
    }
    let mut widest = 0.0f32;
    for word in text.Text()?.split_whitespace() {
        widest = widest.max(width(word)?);
    }
    Ok(widest)
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
            Widget::Label(label) => {
                let width = request.known_width.or(match request.available_width {
                    AvailableSpace::Definite(w) => Some(w),
                    AvailableSpace::MinContent => min_content_width(label).ok(),
                    AvailableSpace::MaxContent => None,
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
            | Widget::Progress(_)
            | Widget::Spinner(_)
            | Widget::Icon(_) => ceil(measure_element(&node.element, infinite)),
            Widget::Number { .. } => ceil(measure_number_box(&node.element)),
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
            // A square, whatever the file.
            Widget::FileIcon { size, .. } => {
                let side = size.unwrap_or(super::props::FILE_ICON_SIZE);
                Size::new(side, side)
            }
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
