//! Text selections: a text box's (a search box's is in its template).
//! XAML counts in UTF-16 units, the core in characters; a text area's line
//! breaks are one `\r` each, as the `\n` the core has.
//!
//! A password box can only select all of its text, and can't say what's
//! selected: a range over all of it selects it all, and typing replaces
//! it; any other range leaves the caret where focus put it, at the end.

use std::ops::Range;

use windows_core::Interface;

use super::fields::inner_text_box;
use super::{Node, R, Widget};
use crate::bindings as w;

/// What edits the node's text, and selects it.
fn text_box(node: &Node) -> Option<w::ITextBox> {
    match &node.widget {
        Widget::Field(field) | Widget::TextArea { field, .. } => field.cast().ok(),
        Widget::Search(_) => inner_text_box(&node.element),
        _ => None,
    }
}

/// Selects these characters of a focused field's or text area's text.
pub(super) fn select(node: &Node, range: Range<usize>) -> R<()> {
    if let Widget::Password(field) = &node.widget {
        let text = field.Password()?;
        let all = range.start == 0 && range.end == text.chars().count() && !text.is_empty();
        if all {
            field.SelectAll()?;
        }
        node.password_all.set(all);
        return Ok(());
    }
    let Some(field) = text_box(node) else { return Ok(()) };
    let text = field.Text()?;
    let units = |chars: usize| text.chars().take(chars).map(char::len_utf16).sum::<usize>() as i32;
    let start = units(range.start);
    field.Select(start, units(range.end) - start)
}

/// The selection of a field or text area, in characters, while it has
/// focus (`focused`). `None` for a password box, which can't tell.
pub(super) fn selection(node: &Node, focused: bool) -> Option<Range<usize>> {
    let field = text_box(node).filter(|_| focused)?;
    let text = field.Text().ok()?;
    let start = usize::try_from(field.SelectionStart().ok()?).ok()?;
    let length = usize::try_from(field.SelectionLength().ok()?).ok()?;
    let chars = |units: usize| {
        let mut seen = 0;
        text.chars().take_while(|c| (seen + c.len_utf16() <= units).then(|| seen += c.len_utf16()).is_some()).count()
    };
    Some(chars(start)..chars(start + length))
}
