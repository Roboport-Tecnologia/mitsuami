//! Measuring widgets with fixed metrics, and reading PNG sizes.

use mitsuami_core::backend::{AvailableSpace, MeasureRequest};
use mitsuami_core::{ImageSource, NodeId, Orientation, Size, TextStyle, WidgetKind, find_prop};

use super::HeadlessBackend;
use super::metrics::{GROUP_INSETS, RADIO_GAP, TAB_INSETS, TAB_PADDING, TITLED_GROUP_INSETS};

impl HeadlessBackend {
    pub(super) fn measure_node(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        let fonts = &state.metrics.font_sizes;
        let font = fonts.get(find_prop!(node.props, TextStyle).unwrap_or(TextStyle::Body));
        let line = (font * 1.25).round();
        let label = || find_prop!(node.props, Label).unwrap_or_default();
        let natural = match node.kind {
            WidgetKind::Text => {
                let text = find_prop!(node.props, Text).unwrap_or_default();
                let wrap = request.known_width.or(match request.available_width {
                    AvailableSpace::Definite(w) => Some(w),
                    AvailableSpace::MinContent => Some(0.0),
                    AvailableSpace::MaxContent => None,
                });
                text_size(&text, font, wrap, find_prop!(node.props, MaxLines).flatten())
            }
            // An icon takes a 16-point square and a 6-point gap before
            // the caption, or the caption's place when it's shown alone.
            // A menu button is a button with a 16-point arrow after it.
            WidgetKind::Button | WidgetKind::ToggleButton | WidgetKind::MenuButton => {
                let arrow = if node.kind == WidgetKind::MenuButton { 16.0 } else { 0.0 };
                let icon = find_prop!(node.props, Icon).is_some_and(|name| !name.is_empty());
                let content = match (icon, find_prop!(node.props, IconOnly) == Some(true)) {
                    (true, true) => 16.0,
                    (true, false) => 16.0 + 6.0 + text_size(&label(), font, None, None).width,
                    (false, _) => text_size(&label(), font, None, None).width,
                };
                Size::new(content + arrow + 24.0, (line + 8.0).max(28.0))
            }
            WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput => Size::new(200.0, line + 8.0),
            // A text field's width, and its lines inside the same insets.
            WidgetKind::TextArea => {
                let lines = find_prop!(node.props, Lines).unwrap_or(1) as f32;
                Size::new(200.0, lines * line + 8.0)
            }
            WidgetKind::Checkbox => {
                let text = text_size(&label(), font, None, None);
                Size::new(16.0 + 6.0 + text.width, line.max(16.0))
            }
            WidgetKind::Switch => Size::new(40.0, 24.0),
            WidgetKind::Slider => match find_prop!(node.props, Orientation).unwrap_or_default() {
                Orientation::Horizontal => Size::new(160.0, 20.0),
                Orientation::Vertical => Size::new(20.0, 160.0),
            },
            // A field for a few digits, and its buttons.
            WidgetKind::NumberInput => Size::new(96.0, line + 8.0),
            WidgetKind::Progress => Size::new(160.0, 8.0),
            WidgetKind::Spinner => Size::new(16.0, 16.0),
            // A 1-point line; the layout gives it its length.
            WidgetKind::Separator => match find_prop!(node.props, Orientation).unwrap_or_default() {
                Orientation::Horizontal => Size::new(0.0, 1.0),
                Orientation::Vertical => Size::new(1.0, 0.0),
            },
            // A square, 16 points unless sized; no name shows nothing.
            WidgetKind::Icon => match find_prop!(node.props, Icon) {
                Some(name) if !name.is_empty() => {
                    let side = find_prop!(node.props, IconSize).unwrap_or(16.0);
                    Size::new(side, side)
                }
                _ => Size::ZERO,
            },
            // Pixels over their scale; a PNG file by its header. Anything
            // else is a file the headless backend can't read: no size.
            WidgetKind::Image => match find_prop!(node.props, Image) {
                Some(ImageSource::Pixels(pixels)) => pixels.size(),
                Some(ImageSource::File(path)) => png_file_size(&path).unwrap_or(Size::ZERO),
                None => Size::ZERO,
            },
            // Sized for its chosen option, with room for the arrow.
            WidgetKind::Select => {
                let options = find_prop!(node.props, Options).unwrap_or_default();
                let chosen = find_prop!(node.props, SelectedIndex).flatten().and_then(|i| options.get(i).cloned());
                Size::new(text_size(&chosen.unwrap_or_default(), font, None, None).width + 32.0, (line + 8.0).max(28.0))
            }
            // A checkbox's box and text for each option, down a column.
            WidgetKind::RadioGroup => {
                let options = find_prop!(node.props, Options).unwrap_or_default();
                let widest = options.iter().map(|o| text_size(o, font, None, None).width).fold(0.0, f32::max);
                let count = options.len() as f32;
                let height = count * line.max(16.0) + (count - 1.0).max(0.0) * RADIO_GAP;
                Size::new(if options.is_empty() { 0.0 } else { 16.0 + 6.0 + widest }, height)
            }
            // Its strip: a tab for each title, side by side, and its border.
            WidgetKind::Tabs => {
                let titles = find_prop!(node.props, TabTitles).unwrap_or_default();
                let tabs: f32 = titles.iter().map(|t| text_size(t, font, None, None).width + TAB_PADDING).sum();
                Size::new(tabs + TAB_INSETS.left + TAB_INSETS.right, TAB_INSETS.top + TAB_INSETS.bottom)
            }
            // Its heading and border, empty.
            WidgetKind::Group => {
                let title = find_prop!(node.props, Title).unwrap_or_default();
                let insets = if title.is_empty() { GROUP_INSETS } else { TITLED_GROUP_INSETS };
                let heading = text_size(&title, font, None, None).width;
                Size::new(heading + insets.left + insets.right, insets.top + insets.bottom)
            }
            // Native renders are stood in for by the drawn one, if any.
            // Native views have no stand-in: size them with styles.
            WidgetKind::Custom(_) => find_prop!(node.props, Custom)
                .and_then(|c| c.measure_drawn(&request, &state.metrics))
                .unwrap_or(Size::ZERO),
            _ => Size::ZERO,
        };
        Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
    }
}

/// Greedy word wrap with fixed-width characters.
/// At most `max_lines` lines: the rest are cut off.
pub(super) fn text_size(text: &str, font: f32, wrap_width: Option<f32>, max_lines: Option<u32>) -> Size {
    let char_width = font * 0.5;
    let line_height = (font * 1.25).round();
    let mut lines: Vec<usize> = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = 0usize;
        for word in paragraph.split_whitespace() {
            let len = word.chars().count();
            let candidate = if current == 0 { len } else { current + 1 + len };
            let fits = wrap_width.is_none_or(|w| candidate as f32 * char_width <= w + 0.01);
            if current == 0 || fits {
                current = candidate;
            } else {
                lines.push(current);
                current = len;
            }
        }
        lines.push(current);
    }
    if let Some(max) = max_lines {
        lines.truncate(max as usize);
    }
    let widest = lines.iter().copied().max().unwrap_or(0);
    Size::new(widest as f32 * char_width, lines.len().max(1) as f32 * line_height)
}

pub(super) fn png_file_size(path: &std::path::Path) -> Option<Size> {
    use std::io::Read;
    let mut header = [0u8; 24];
    std::fs::File::open(path).ok()?.read_exact(&mut header).ok()?;
    png_size(&header)
}

/// A PNG's size, from its header (the IHDR chunk comes first).
pub(super) fn png_size(header: &[u8]) -> Option<Size> {
    if header.len() < 24 || header[..8] != *b"\x89PNG\r\n\x1a\n" || header[12..16] != *b"IHDR" {
        return None;
    }
    let number = |at: usize| u32::from_be_bytes(header[at..at + 4].try_into().unwrap()) as f32;
    Some(Size::new(number(16), number(20)))
}
