//! Focus: reporting it, the Tab order, and focus going back to a window.

use std::cell::Cell;
use std::collections::HashMap;

use mitsuami_core::{NodeId, UiEvent};
use windows_core::{IInspectable, Interface};

use super::native_state::elements;
use super::{ElementMap, Events, State, Widget, WinUiBackend, key};
use crate::bindings as w;

/// Reports a focus move, once: moves we make are reported right away (XAML
/// raises `GotFocus` asynchronously), and the later `GotFocus` finds them
/// already reported.
pub(super) fn report_focus(emitter: &Events, focus: &Cell<Option<NodeId>>, now: Option<NodeId>) {
    let before = focus.get();
    if now == before {
        return;
    }
    if let Some(before) = before {
        emitter.emit(before, UiEvent::FocusOut);
    }
    if let Some(now) = now {
        emitter.emit(now, UiEvent::FocusIn);
    }
    focus.set(now);
}

/// Nearest node for a focused element: XAML focuses parts of composite
/// controls (a TextBox's inner editor), so walk up the visual tree.
pub(super) fn resolve(by_element: &ElementMap, element: Option<IInspectable>) -> Option<NodeId> {
    let mut current: Option<w::DependencyObject> = element?.cast().ok();
    let map = by_element.borrow();
    while let Some(object) = current {
        if let Some(id) = map.get(&key(&object)) {
            return Some(*id);
        }
        current = w::VisualTreeHelper::GetParent(&object).ok();
    }
    None
}

impl State {
    /// Focuses a control and reports it right away.
    pub(super) fn focus(&self, id: NodeId, how: w::FocusState) -> bool {
        let Some(node) = self.nodes.get(&id) else { return false };
        let focused = node.focus_target().is_some_and(|e| e.Focus(how).unwrap_or(false));
        if focused && let Some(parts) = self.window_of(id) {
            report_focus(&self.emitter, &parts.focus, Some(id));
        }
        focused
    }
}

pub(super) fn is_control(widget: &Widget) -> bool {
    matches!(
        widget,
        Widget::Field(_)
            | Widget::TextArea { .. }
            | Widget::Password(_)
            | Widget::Search(_)
            | Widget::Button(_)
            | Widget::Toggle(_)
            | Widget::MenuButton(_)
            | Widget::Checkbox(_)
            | Widget::Switch(_)
            | Widget::Select(_)
            | Widget::RadioGroup(_)
            | Widget::Slider { .. }
            | Widget::Number { .. }
            | Widget::Scroll(_)
            | Widget::List(_)
    )
}

/// A window became the active one: focus goes back to the control that had
/// it, or to the first in the Tab order, when XAML's focus isn't on one.
/// XAML focuses the first focusable element when a Page loads with nothing
/// focused, but the content isn't a Page; and it restores focus on
/// activation itself, but can leave it on its root `ScrollViewer`, as after
/// another app's window (NVIDIA's overlay) took activation for a moment.
pub(super) fn restore_focus(
    root: &w::Grid,
    by_element: &ElementMap,
    focus: &Cell<Option<NodeId>>,
    order: &[NodeId],
    emitter: &Events,
) {
    let focused = root
        .cast::<w::IUIElement>()
        .and_then(|r| r.XamlRoot())
        .and_then(|r| w::FocusManager::GetFocusedElementWithRoot(&r))
        .ok()
        .filter(|e| !e.as_raw().is_null());
    if resolve(by_element, focused).is_some() {
        return;
    }
    let how = w::FocusState::Programmatic;
    let last = focus.get().filter(|id| order.contains(id));
    let now = last
        .and_then(|id| tab(root, by_element, None, &[id], false, how))
        .or_else(|| tab(root, by_element, None, order, false, how));
    report_focus(emitter, focus, now);
}

/// Moves focus along the core's Tab order, skipping controls that can't
/// take focus now, focusing it `how`. Returns the node that took it.
pub(super) fn tab(
    root: &w::Grid,
    by_element: &ElementMap,
    from: Option<NodeId>,
    order: &[NodeId],
    backwards: bool,
    how: w::FocusState,
) -> Option<NodeId> {
    if order.is_empty() {
        return None;
    }
    let elements: HashMap<NodeId, usize> = by_element.borrow().iter().map(|(k, v)| (*v, *k)).collect();
    let start = from.and_then(|f| order.iter().position(|id| *id == f));
    let n = order.len();
    for step in 1..=n {
        let i = match (start, backwards) {
            (Some(s), false) => (s + step) % n,
            (Some(s), true) => (s + n - step) % n,
            (None, false) => step - 1,
            (None, true) => n - step,
        };
        let Some(element) = elements.get(&order[i]).and_then(|k| find_element(root, *k)) else { continue };
        // A navigation view and a selector bar take focus on their
        // selected item.
        let element = match (element.cast::<w::NavigationView>(), crate::tabs::Tabs::bar_in(&element)) {
            (Ok(view), _) => match crate::sidebar::Sidebar::focus_target(&view) {
                Some(item) => item,
                None => continue,
            },
            (_, Some(bar)) => match crate::tabs::Tabs::focus_target(&bar) {
                Some(item) => item,
                None => continue,
            },
            _ => element,
        };
        if element.Focus(how).unwrap_or(false) {
            return Some(order[i]);
        }
    }
    None
}

/// The element with COM identity `key` under `root`.
fn find_element(root: &w::Grid, key_: usize) -> Option<w::IUIElement> {
    fn walk(object: w::DependencyObject, key_: usize, depth: usize) -> Option<w::IUIElement> {
        if key(&object) == key_ {
            return object.cast().ok();
        }
        if depth > 64 {
            return None;
        }
        let panel = object.cast::<w::IPanel>().ok();
        if let Some(children) = panel.and_then(|p| p.Children().ok()) {
            for child in elements(&children) {
                if let Some(found) = child.cast().ok().and_then(|c| walk(c, key_, depth + 1)) {
                    return Some(found);
                }
            }
        }
        let content = object.cast::<w::IContentControl>().ok().and_then(|c| c.Content().ok());
        if let Some(found) = content.and_then(|c| c.cast().ok()).and_then(|c| walk(c, key_, depth + 1)) {
            return Some(found);
        }
        None
    }
    walk(root.cast().ok()?, key_, 0)
}

impl WinUiBackend {
    /// Tab pressed in `id`: move along its window's order.
    pub(super) fn tab_from(&self, id: NodeId) {
        let state = self.state.borrow();
        let mut window = Some(id);
        while let Some(current) = window {
            let node = &state.nodes[&current];
            if let Widget::Window(parts) = &node.widget {
                let order = parts.tab_order.borrow().clone();
                if let Some(next) =
                    tab(&parts.root, &state.by_element, Some(id), &order, false, w::FocusState::Keyboard)
                {
                    report_focus(&state.emitter, &parts.focus, Some(next));
                }
                return;
            }
            window = node.parent;
        }
    }
}
