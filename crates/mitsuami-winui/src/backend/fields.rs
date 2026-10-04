//! Text fields, text areas and number boxes.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::{EventValue, NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use super::measure::measure_element;
use super::{Events, R};
use crate::bindings as w;

/// Sets a number box to `to` once its `ValueChanged` handler has returned,
/// unless something else has changed it from `from` by then.
pub(super) fn set_later(number: &w::INumberBox, from: f64, to: f64) {
    let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() else { return };
    let ticket = crate::later::park_until(&queue, number.clone());
    crate::later::on_ui_take(&queue, ticket, move |number: w::INumberBox| {
        if number.Value().is_ok_and(|v| v.to_bits() == from.to_bits()) {
            let _ = number.SetValue(to);
        }
    });
}

/// A text box made as text areas are, to measure their lines by: out of
/// sight, out of the Tab order and of the accessibility tree.
pub(super) fn text_area_probe() -> R<w::TextBox> {
    let probe = w::TextBox::new()?;
    let iface: w::ITextBox = probe.cast()?;
    iface.SetAcceptsReturn(true)?;
    iface.SetTextWrapping(w::TextWrapping::Wrap)?;
    let element: w::IUIElement = probe.cast()?;
    element.SetIsTabStop(false)?;
    element.SetVisibility(w::Visibility::Collapsed)?;
    w::AutomationProperties::SetAccessibilityView(&probe.cast::<w::DependencyObject>()?, w::AccessibilityView::Raw)?;
    Ok(probe)
}

/// The size of a text area of this many lines in `field`'s font: the
/// probe's, shown only while it's measured.
pub(super) fn measure_lines(field: &w::TextBox, probe: &w::TextBox, lines: u32) -> R<w::Size> {
    let (font, empty): (w::IControl, w::IControl) = (field.cast()?, probe.cast()?);
    empty.SetFontSize(font.FontSize()?)?;
    empty.SetFontFamily(&font.FontFamily()?)?;
    probe.cast::<w::ITextBox>()?.SetText(&vec![""; lines.max(1) as usize].join("\r"))?;
    let element: w::IUIElement = probe.cast()?;
    element.SetVisibility(w::Visibility::Visible)?;
    let size = measure_element(&probe.cast()?, w::Size { width: f32::INFINITY, height: f32::INFINITY });
    element.SetVisibility(w::Visibility::Collapsed)?;
    Ok(size)
}

/// A text box's text with its lines ending in `\n`: XAML ends them in
/// `\r`, whatever they were set with. In one pass, and none without a
/// line break: it runs on every keystroke, over the whole text.
pub(super) fn box_text(field: &w::ITextBox) -> windows_core::Result<String> {
    let text = field.Text()?;
    if !text.contains('\r') {
        return Ok(text);
    }
    let mut lines = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            chars.next_if_eq(&'\n');
            lines.push('\n');
        } else {
            lines.push(c);
        }
    }
    Ok(lines)
}

/// Reports a text box's user edits. TextChanged also fires (later) for
/// programmatic sets: only text the core doesn't know about is a user edit.
pub(super) fn report_text_changes(
    field: &w::TextBox,
    emitter: &Events,
    shown: &Rc<RefCell<String>>,
    id: NodeId,
) -> R<EventRevoker> {
    let (emitter, shown) = (emitter.clone(), shown.clone());
    field.cast::<w::ITextBox>()?.TextChanged(move |sender, _| {
        let Some(text) = sender.as_ref().and_then(|s| box_text(&s.cast::<w::ITextBox>().ok()?).ok()) else {
            return;
        };
        if *shown.borrow() != text {
            *shown.borrow_mut() = text.clone();
            emitter.emit(id, UiEvent::Changed(EventValue::Text(text)));
        }
    })
}

/// What a search box shows: the text box in its template has it at once,
/// the search box itself only once its TextChanged comes, later (not
/// before a test's settle is over).
pub(super) fn search_text(search: &w::AutoSuggestBox) -> Option<String> {
    match search.cast::<w::UIElement>().ok().and_then(|s| inner_text_box(&s)) {
        Some(field) => field.Text().ok(),
        None => search.cast::<w::IAutoSuggestBox>().and_then(|s| s.Text()).ok(),
    }
}

/// What a search box shows, as `search_text`, with the text box in its
/// template kept in `field`: its edits come a keystroke at a time, and
/// finding it walks the template. Found again once it leaves the tree (a
/// new template).
pub(super) fn search_text_with(search: &w::AutoSuggestBox, field: &RefCell<Option<w::ITextBox>>) -> Option<String> {
    let loaded = |f: &w::ITextBox| f.cast::<w::IFrameworkElement>().and_then(|f| f.IsLoaded()).unwrap_or(false);
    let kept = field.borrow().clone().filter(loaded);
    let found = kept.or_else(|| search.cast::<w::UIElement>().ok().and_then(|s| inner_text_box(&s)));
    *field.borrow_mut() = found.clone();
    match found {
        Some(found) => found.Text().ok(),
        None => search.cast::<w::IAutoSuggestBox>().and_then(|s| s.Text()).ok(),
    }
}

/// The text box in a search box's template, once it's applied.
pub(super) fn inner_text_box(search: &w::UIElement) -> Option<w::ITextBox> {
    let mut queue = std::collections::VecDeque::from([search.cast::<w::DependencyObject>().ok()?]);
    while let Some(node) = queue.pop_front() {
        if let Ok(field) = node.cast::<w::ITextBox>() {
            return Some(field);
        }
        for i in 0..w::VisualTreeHelper::GetChildrenCount(&node).unwrap_or(0) {
            if let Ok(child) = w::VisualTreeHelper::GetChild(&node, i) {
                queue.push_back(child);
            }
        }
    }
    None
}

/// Return submits a text or password box; leaving it doesn't.
pub(super) fn submit_on_enter(field: &w::IUIElement, emitter: &Events, id: NodeId) -> R<EventRevoker> {
    let emitter = emitter.clone();
    field.KeyDown(move |_, args| {
        let enter = args
            .as_ref()
            .and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()?.Key().ok())
            .is_some_and(|k| k == w::VirtualKey::Enter);
        if enter {
            emitter.emit(id, UiEvent::Submit);
        }
    })
}
