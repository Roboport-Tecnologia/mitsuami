//! Synthesizing the user's input: clicks, keys, scrolls and drags.

use std::time::Duration;

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
use mitsuami_core::services::Shortcut;
use mitsuami_core::{
    KeyCode, Modifiers, NodeId, ScrollAxes, ScrollDelta, SelectionMode, SurfaceInput, UiEvent, WidgetKind,
};

use crate::events::node_key;
use crate::ffi::QmlObject;

use super::{KirigamiBackend, Widget, pump_until};

// Qt key codes (`Qt::Key`).
const KEY_ESCAPE: i32 = 0x0100_0000;
const KEY_UNKNOWN: i32 = 0x01ff_ffff;

/// A key's `Qt::Key`, and the text a key event of it carries.
pub(crate) fn qt_key(key: Key) -> (i32, String) {
    let special = |offset: i32| (0x0100_0000 + offset, String::new());
    match key {
        // ASCII keys are their upper case character in Qt.
        Key::Char(c) => {
            let code = if c.is_ascii_graphic() || c == ' ' { c.to_ascii_uppercase() as i32 } else { KEY_UNKNOWN };
            (code, c.to_string())
        }
        Key::Escape => (KEY_ESCAPE, "\u{1b}".into()),
        Key::Tab => (0x0100_0001, "\t".into()),
        Key::Backspace => special(0x03),
        Key::Enter => (0x0100_0004, "\r".into()),
        Key::Delete => special(0x07),
        Key::Home => special(0x10),
        Key::End => special(0x11),
        Key::Left => special(0x12),
        Key::Up => special(0x13),
        Key::Right => special(0x14),
        Key::Down => special(0x15),
        Key::PageUp => special(0x16),
        Key::PageDown => special(0x17),
        Key::F(n) => special(0x2f + i32::from(n)),
    }
}

/// A key pressed in a window, which the user's keys reach once it's the
/// focused window: its shortcuts (a modal window's Escape) only match then.
fn key_in(window: QmlObject, code: i32, modifiers: i32, text: &str) {
    if !window.bool("mitsuamiFocused") {
        window.invoke("requestActivate");
        pump_until(Duration::from_secs(2), || window.bool("mitsuamiFocused"));
    }
    window.key(code, modifiers, text);
}

impl KirigamiBackend {
    pub(super) fn synthesize_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        // What the hover handler's signal runs, with the state let go.
        if let SyntheticInput::PointerEnter | SyntheticInput::PointerLeave = input {
            let report = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                node.hover.as_ref().map(|hover| hover.report()).ok_or(ActionError::Unsupported)?
            };
            report(*input == SyntheticInput::PointerEnter);
            return Ok(());
        }
        if *input == SyntheticInput::DoubleClick {
            let report = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                node.double_click.as_ref().map(|d| d.report()).ok_or(ActionError::Unsupported)?
            };
            report();
            return Ok(());
        }
        let (widget_item, kind, window) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let mut window_id = id;
            while let Some(parent) = state.nodes.get(&window_id).and_then(|n| n.parent) {
                window_id = parent;
            }
            let window = match state.nodes.get(&window_id).map(|n| &n.widget) {
                Some(Widget::Window { root }) => Some(root.window),
                _ => None,
            };
            if node.widget.is_control() && !node.widget.item().bool("enabled") {
                return Err(ActionError::Disabled);
            }
            (node.widget.input_item(), node.kind, window)
        };
        if kind == WidgetKind::GpuSurface {
            return self.surface_input(id, widget_item, window, input);
        }
        // The drop area's own handling, with the state let go: its reports
        // may wake the run loop.
        if let SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) = input {
            let drop = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                node.file_drop.as_ref().map(|area| area.input()).ok_or(ActionError::Unsupported)?
            };
            match input {
                SyntheticInput::DragFiles(paths) => drop.enter(paths),
                SyntheticInput::DragLeave => drop.leave(),
                SyntheticInput::DropFiles(paths) => drop.dropped(paths),
                _ => unreachable!(),
            }
            return Ok(());
        }
        match input {
            // Handled above.
            SyntheticInput::DragFiles(_)
            | SyntheticInput::DragLeave
            | SyntheticInput::DropFiles(_)
            | SyntheticInput::PointerEnter
            | SyntheticInput::PointerLeave
            | SyntheticInput::DoubleClick => unreachable!(),
            // Text fields take keys with modifiers as editing commands,
            // which aren't simulated.
            SyntheticInput::Shortcut(_)
                if matches!(
                    kind,
                    WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea
                ) =>
            {
                Err(ActionError::Unsupported)
            }
            SyntheticInput::Shortcut(shortcut) => self.send_key(id, *shortcut, widget_item, window),
            SyntheticInput::Click(point) => {
                // Drawn widgets only: their pointer handling is ours.
                let drawn = matches!(self.state.borrow().nodes.get(&id).map(|n| &n.widget), Some(Widget::Drawn { .. }));
                let window = window.filter(|_| drawn).ok_or(ActionError::Unsupported)?;
                window.click(widget_item.map_to_scene(*point));
                Ok(())
            }
            // A table's content starts at its origin too, and scrolls
            // sideways.
            SyntheticInput::Scroll { dx, dy } if kind == WidgetKind::Table => {
                let view = widget_item;
                for (offset, origin, content, size, by) in [
                    ("contentX", "originX", "contentWidth", "width", *dx),
                    ("contentY", "originY", "contentHeight", "height", *dy),
                ] {
                    let (origin, max) = (view.real(origin), (view.real(content) - view.real(size)).max(0.0));
                    let at = (view.real(offset) - origin + by as f64).clamp(0.0, max);
                    view.set_real(offset, at + origin);
                }
                Ok(())
            }
            SyntheticInput::Scroll { dy, .. } if kind == WidgetKind::List => {
                // `ListView`'s content starts at `originY`.
                let view = widget_item;
                let (origin, max) = (view.real("originY"), (view.real("contentHeight") - view.real("height")).max(0.0));
                let offset = (view.real("contentY") - origin + *dy as f64).clamp(0.0, max);
                if offset >= max && *dy > 0.0 {
                    // Rows not shown yet are estimates: Qt goes to the real end.
                    view.invoke("positionViewAtEnd");
                } else {
                    view.set_real("contentY", offset + origin);
                }
                Ok(())
            }
            SyntheticInput::Scroll { dx, dy } => {
                if kind != WidgetKind::ScrollView {
                    return Err(ActionError::Unsupported);
                }
                let axes = self.state.borrow().nodes[&id].scroll_axes.unwrap_or(ScrollAxes::Vertical);
                let flickable = widget_item;
                let step = |offset: &str, content: &str, view: &str, by: f32, on: bool| {
                    if on {
                        let max = (flickable.real(content) - flickable.real(view)).max(0.0);
                        flickable.set_real(offset, (flickable.real(offset) + by as f64).clamp(0.0, max));
                    }
                };
                step("contentX", "contentWidth", "width", *dx, axes.horizontal());
                step("contentY", "contentHeight", "height", *dy, axes.vertical());
                Ok(())
            }
            SyntheticInput::Key(key) => match (kind, key) {
                // Real key events, through the list view's own keyboard
                // navigation (and ours for Home, End and Return).
                (WidgetKind::List | WidgetKind::Table, Key::Up | Key::Down | Key::Home | Key::End | Key::Enter) => {
                    let window = window.ok_or(ActionError::Unsupported)?;
                    if self
                        .state
                        .borrow()
                        .nodes
                        .get(&id)
                        .map(|n| &n.widget)
                        .is_some_and(|w| matches!(w, Widget::List(list) if list.mode() == SelectionMode::None))
                    {
                        return Err(ActionError::Unsupported);
                    }
                    widget_item.force_focus();
                    let (code, text) = qt_key(*key);
                    key_in(window, code, 0, &text);
                    Ok(())
                }
                // Qt's text area takes Return as a new line and Tab as a
                // tab, as real keys.
                (
                    WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea,
                    _,
                ) => {
                    // It would take the keys and ignore them; nothing can be
                    // typed into it on any platform.
                    if widget_item.bool("readOnly") {
                        return Err(ActionError::ReadOnly);
                    }
                    // Real key events, through Qt's text editing.
                    let window = window.ok_or(ActionError::Unsupported)?;
                    if window.focus_item().and_then(|f| f.node()) != Some(node_key(id)) {
                        widget_item.force_focus();
                        // Typing appends, as after clicking past the end.
                        widget_item.set_int("cursorPosition", widget_item.str("text").chars().count() as i32);
                    }
                    let (code, text) = qt_key(*key);
                    key_in(window, code, 0, &text);
                    Ok(())
                }
                (WidgetKind::Button, Key::Enter | Key::Char(' '))
                | (WidgetKind::ToggleButton | WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => {
                    self.perform(id, &A11yAction::Activate)
                }
                // A real Escape, from the node if it takes focus: a modal
                // window's shortcut asks it to close.
                (_, Key::Escape) => {
                    let window = window.ok_or(ActionError::Unsupported)?;
                    if self.state.borrow().nodes.get(&id).is_some_and(|n| n.widget.is_control()) {
                        widget_item.force_focus();
                    }
                    key_in(window, KEY_ESCAPE, 0, "\u{1b}");
                    Ok(())
                }
                _ => self.send_key(id, Shortcut::new(*key), widget_item, window),
            },
        }
    }

    /// A key the control doesn't use, as the keyboard sends it: a real key
    /// press to its window, the control focused, which Qt Quick sends on
    /// up its parent items to a node's item that takes it (`crate::keys`).
    /// `Unsupported` if none around does.
    fn send_key(
        &self,
        id: NodeId,
        shortcut: Shortcut,
        input_item: QmlObject,
        window: Option<QmlObject>,
    ) -> Result<(), ActionError> {
        let focusable = {
            let state = self.state.borrow();
            let mut at = Some(id);
            let mut taken = false;
            while let Some(node) = at.and_then(|at| state.nodes.get(&at)) {
                if node.keys.as_ref().is_some_and(|keys| keys.takes(&shortcut)) {
                    taken = true;
                    break;
                }
                at = node.parent;
            }
            if !taken {
                return Err(ActionError::Unsupported);
            }
            state.nodes.get(&id).is_some_and(|n| n.widget.is_focusable())
        };
        let window = window.ok_or(ActionError::Unsupported)?;
        if focusable {
            input_item.force_focus();
        }
        let (code, mut text) = qt_key(shortcut.key);
        if shortcut.shift {
            text = text.to_uppercase();
        }
        key_in(window, code, crate::keys::modifiers(&shortcut), &text);
        Ok(())
    }

    /// Input on a GPU surface that takes it, through Qt's own event path:
    /// a click (which focuses it) and keys with their native scan codes
    /// go to the window. A scroll is reported as it would be, in points.
    fn surface_input(
        &self,
        id: NodeId,
        input_item: QmlObject,
        window: Option<QmlObject>,
        input: &SyntheticInput,
    ) -> Result<(), ActionError> {
        let events = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            match &node.widget {
                Widget::GpuSurface(surface) if surface.takes_input() => state.events.clone(),
                _ => return Err(ActionError::Unsupported),
            }
        };
        let window = window.ok_or(ActionError::Unsupported)?;
        match input {
            SyntheticInput::Click(point) => window.click(input_item.map_to_scene(*point)),
            SyntheticInput::Key(key) => {
                if window.focus_item().and_then(|f| f.node()) != Some(node_key(id)) {
                    return Err(ActionError::Unsupported);
                }
                let ((qt_key, text), code) = (qt_key(*key), KeyCode::from_key(*key));
                // XKB key codes: evdev's plus 8.
                window.surface_key(qt_key, crate::surface::evdev_code(code) + 8, &text);
            }
            SyntheticInput::Scroll { dx, dy } => {
                let delta = ScrollDelta::Points { x: *dx, y: *dy };
                let modifiers = Modifiers::default();
                events.emit(id, UiEvent::SurfaceInput(SurfaceInput::Scroll { delta, modifiers }));
            }
            // Keys with modifiers aren't simulated on a surface.
            SyntheticInput::Shortcut(_)
            | SyntheticInput::DragFiles(_)
            | SyntheticInput::DragLeave
            | SyntheticInput::DropFiles(_)
            | SyntheticInput::PointerEnter
            | SyntheticInput::PointerLeave
            | SyntheticInput::DoubleClick => {
                return Err(ActionError::Unsupported);
            }
        }
        Ok(())
    }
}
