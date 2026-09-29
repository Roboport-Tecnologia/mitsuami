//! Text selections: a field's through the window's field editor while
//! it's edited, a text area's through its text view. AppKit counts in
//! UTF-16 units, the core in characters.

use std::ops::Range;

use objc2::rc::Retained;
use objc2_app_kit::NSText;
use objc2_foundation::NSRange;

use super::Widget;

/// What edits the widget's text: the field editor while a field has
/// focus, a text area's text view.
fn editor(widget: &Widget) -> Option<Retained<NSText>> {
    match widget {
        Widget::Field(field) => field.currentEditor(),
        Widget::TextArea(area) => Some(Retained::into_super(area.text.clone())),
        _ => None,
    }
}

/// Selects these characters of a focused field's or text area's text.
pub(super) fn select(widget: &Widget, range: Range<usize>) {
    let Some(editor) = editor(widget) else { return };
    let text = editor.string().to_string();
    let units = |chars: usize| text.chars().take(chars).map(char::len_utf16).sum::<usize>();
    let start = units(range.start);
    editor.setSelectedRange(NSRange::new(start, units(range.end) - start));
    editor.scrollRangeToVisible(editor.selectedRange());
}

/// The selection of a field or text area, in characters, while it has
/// focus (`focused`).
pub(super) fn selection(widget: &Widget, focused: bool) -> Option<Range<usize>> {
    let editor = editor(widget).filter(|_| focused)?;
    let text = editor.string().to_string();
    let range = editor.selectedRange();
    let chars = |units: usize| {
        let mut seen = 0;
        text.chars().take_while(|c| (seen + c.len_utf16() <= units).then(|| seen += c.len_utf16()).is_some()).count()
    };
    Some(chars(range.location)..chars(range.location + range.length))
}
