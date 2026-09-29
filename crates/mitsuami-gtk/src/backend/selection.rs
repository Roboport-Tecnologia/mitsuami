//! Text selections: a field's through its editable, a text area's through
//! its text buffer. Both count in characters, as the core does.

use std::ops::Range;

use gtk::prelude::*;

use super::Widget;

/// A field's editable: entries forward to their inner text widget.
fn editable(widget: &Widget) -> Option<&gtk::Editable> {
    match widget {
        Widget::Entry(w) => Some(w.upcast_ref()),
        Widget::Password(w) => Some(w.upcast_ref()),
        Widget::Search(w) => Some(w.upcast_ref()),
        _ => None,
    }
}

/// Selects these characters of a field's or text area's text, with the
/// cursor at the end, as GTK's own `select_region` puts it. Whether the
/// widget has text to select.
pub(super) fn select(widget: &Widget, range: Range<usize>) -> bool {
    if let Some(editable) = editable(widget) {
        // The field scrolls to its cursor itself.
        editable.select_region(range.start as i32, range.end as i32);
        return true;
    }
    let Widget::TextArea { view, .. } = widget else { return false };
    let buffer = view.buffer();
    let (start, end) = (buffer.iter_at_offset(range.start as i32), buffer.iter_at_offset(range.end as i32));
    buffer.select_range(&end, &start);
    view.scroll_mark_onscreen(&buffer.get_insert());
    true
}

/// The selection of a field or text area, in characters, while it has
/// focus (`focused`): the cursor, when nothing is selected.
pub(super) fn selection(widget: &Widget, focused: bool) -> Option<Range<usize>> {
    if !focused {
        return None;
    }
    if let Some(editable) = editable(widget) {
        let (start, end) = editable.selection_bounds().unwrap_or((editable.position(), editable.position()));
        return Some(start.min(end) as usize..start.max(end) as usize);
    }
    let Widget::TextArea { view, .. } = widget else { return None };
    let buffer = view.buffer();
    let (start, end) = buffer.selection_bounds().unwrap_or_else(|| {
        let cursor = buffer.iter_at_mark(&buffer.get_insert());
        (cursor, cursor)
    });
    Some(start.offset() as usize..end.offset() as usize)
}
