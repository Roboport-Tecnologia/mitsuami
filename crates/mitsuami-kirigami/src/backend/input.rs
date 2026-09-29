//! Synthesizing the user's input: clicks, keys, scrolls and drags.

use std::time::Duration;

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
use mitsuami_core::{
    KeyCode, Modifiers, NodeId, ScrollAxes, ScrollDelta, SelectionMode, SurfaceInput, UiEvent, WidgetKind,
};

use crate::events::node_key;
use crate::ffi::QmlObject;

use super::{KirigamiBackend, Widget, pump_until};

// Qt key codes (`Qt::Key`).
const KEY_TAB: i32 = 0x0100_0001;
const KEY_BACKSPACE: i32 = 0x0100_0003;
const KEY_RETURN: i32 = 0x0100_0004;
const KEY_ESCAPE: i32 = 0x0100_0000;
const KEY_HOME: i32 = 0x0100_0010;
const KEY_END: i32 = 0x0100_0011;
const KEY_UP: i32 = 0x0100_0013;
const KEY_DOWN: i32 = 0x0100_0015;
const KEY_UNKNOWN: i32 = 0x01ff_ffff;

/// A key pressed in a window, which the user's keys reach once it's the
/// focused window: its shortcuts (a modal window's Escape) only match then.
fn key_in(window: QmlObject, code: i32, text: &str) {
    if !window.bool("mitsuamiFocused") {
        window.invoke("requestActivate");
        pump_until(Duration::from_secs(2), || window.bool("mitsuamiFocused"));
    }
    window.key(code, false, text);
}

impl KirigamiBackend {
    pub(super) fn synthesize_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
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
            SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) => unreachable!(),
            SyntheticInput::Click(point) => {
                // Drawn widgets only: their pointer handling is ours.
                let drawn = matches!(self.state.borrow().nodes.get(&id).map(|n| &n.widget), Some(Widget::Drawn { .. }));
                let window = window.filter(|_| drawn).ok_or(ActionError::Unsupported)?;
                window.click(widget_item.map_to_scene(*point));
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
                (WidgetKind::List, Key::Up | Key::Down | Key::Home | Key::End | Key::Enter) => {
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
                    let (code, text) = match key {
                        Key::Up => (KEY_UP, ""),
                        Key::Down => (KEY_DOWN, ""),
                        Key::Home => (KEY_HOME, ""),
                        Key::End => (KEY_END, ""),
                        _ => (KEY_RETURN, "\r"),
                    };
                    key_in(window, code, text);
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
                    let (code, text) = match key {
                        Key::Char(c) => {
                            let code = if c.is_ascii_alphanumeric() || *c == ' ' {
                                c.to_ascii_uppercase() as i32
                            } else {
                                KEY_UNKNOWN
                            };
                            (code, c.to_string())
                        }
                        Key::Backspace => (KEY_BACKSPACE, String::new()),
                        Key::Enter => (KEY_RETURN, "\r".into()),
                        Key::Tab => (KEY_TAB, "\t".into()),
                        Key::Escape => (KEY_ESCAPE, "\u{1b}".into()),
                        Key::Up => (KEY_UP, String::new()),
                        Key::Down => (KEY_DOWN, String::new()),
                        Key::Home => (KEY_HOME, String::new()),
                        Key::End => (KEY_END, String::new()),
                    };
                    key_in(window, code, &text);
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
                    key_in(window, KEY_ESCAPE, "\u{1b}");
                    Ok(())
                }
                _ => Err(ActionError::Unsupported),
            },
        }
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
                let (qt_key, code, text) = match key {
                    Key::Char(c) => {
                        let qt_key = if c.is_ascii_alphanumeric() || *c == ' ' {
                            c.to_ascii_uppercase() as i32
                        } else {
                            KEY_UNKNOWN
                        };
                        (qt_key, KeyCode::from_us_char(*c), c.to_string())
                    }
                    Key::Enter => (KEY_RETURN, KeyCode::Enter, "\r".to_owned()),
                    Key::Escape => (KEY_ESCAPE, KeyCode::Escape, String::new()),
                    Key::Tab => (KEY_TAB, KeyCode::Tab, "\t".to_owned()),
                    Key::Backspace => (KEY_BACKSPACE, KeyCode::Backspace, String::new()),
                    Key::Up => (KEY_UP, KeyCode::ArrowUp, String::new()),
                    Key::Down => (KEY_DOWN, KeyCode::ArrowDown, String::new()),
                    Key::Home => (KEY_HOME, KeyCode::Home, String::new()),
                    Key::End => (KEY_END, KeyCode::End, String::new()),
                };
                // XKB key codes: evdev's plus 8.
                window.surface_key(qt_key, crate::surface::evdev_code(code) + 8, &text);
            }
            SyntheticInput::Scroll { dx, dy } => {
                let delta = ScrollDelta::Points { x: *dx, y: *dy };
                let modifiers = Modifiers::default();
                events.emit(id, UiEvent::SurfaceInput(SurfaceInput::Scroll { delta, modifiers }));
            }
            SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) => {
                return Err(ActionError::Unsupported);
            }
        }
        Ok(())
    }
}
