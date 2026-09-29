//! Text selections of fields and text areas: Qt Quick's text inputs and
//! edits count in UTF-16 units (QString positions), the core in characters.

use std::ops::Range;

use crate::ffi::QmlObject;

use super::Widget;

/// What edits the widget's text: a field itself, a text area's area.
fn editor(widget: &Widget) -> Option<QmlObject> {
    match widget {
        Widget::Field(field) => Some(*field),
        Widget::TextArea { area, .. } => Some(*area),
        _ => None,
    }
}

/// Selects these characters of a field's or text area's text. Qt puts the
/// cursor at the end, and a text area in a scroll view scrolls to it.
pub(super) fn select(widget: &Widget, range: Range<usize>) {
    let Some(editor) = editor(widget) else { return };
    let text = editor.str("text");
    let units = |chars: usize| text.chars().take(chars).map(char::len_utf16).sum::<usize>() as i32;
    editor.select_text(units(range.start), units(range.end));
}

/// The selection of a field or text area, in characters. Nothing
/// selected, both ends are the cursor.
pub(super) fn selection(widget: &Widget) -> Option<Range<usize>> {
    let editor = editor(widget)?;
    let text = editor.str("text");
    let chars = |units: i32| {
        let mut seen = 0;
        text.chars()
            .take_while(|c| (seen + c.len_utf16() <= units as usize).then(|| seen += c.len_utf16()).is_some())
            .count()
    };
    Some(chars(editor.int("selectionStart"))..chars(editor.int("selectionEnd")))
}
