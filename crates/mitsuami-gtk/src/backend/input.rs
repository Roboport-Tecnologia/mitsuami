//! Synthesized input: keys, clicks, scrolling and file drags.

use gtk::prelude::*;
use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
use mitsuami_core::{NodeId, SelectionMode, WidgetKind};

use super::text::text_view;
use super::{GtkBackend, Widget, owning_node};

impl GtkBackend {
    pub(super) fn synthesize_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        // Its controllers report what it takes.
        if let Some(Widget::GpuSurface(surface)) = self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
            return surface.synthesize(input);
        }
        // Through what the drop target's handlers do: GTK can't start a drag.
        if let SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) = input {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let Some(Some(target)) = &node.file_drop else { return Err(ActionError::Unsupported) };
            match input {
                SyntheticInput::DragFiles(paths) => target.drag(paths),
                SyntheticInput::DropFiles(paths) => target.drop_files(paths),
                _ => target.drag_leave(),
            }
            return Ok(());
        }
        if let SyntheticInput::Click(point) = input {
            // Drawn widgets only: their pointer handling is ours.
            let state = self.state.borrow();
            return match state.nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Drawn { drawn, .. }) => {
                    drawn.click(*point);
                    Ok(())
                }
                Some(_) => Err(ActionError::Unsupported),
                None => Err(ActionError::UnknownNode),
            };
        }
        if let SyntheticInput::Scroll { dx, dy } = input {
            let scrolled = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Scroll { scrolled, .. }) => scrolled.clone(),
                Some(Widget::List(list)) => {
                    self.state.borrow().lists_dirty.set(true);
                    list.scrolled.clone()
                }
                Some(_) => return Err(ActionError::Unsupported),
                None => return Err(ActionError::UnknownNode),
            };
            let (h_on, v_on) = {
                let (h, v) = scrolled.policy();
                (h != gtk::PolicyType::Never, v != gtk::PolicyType::Never)
            };
            let step = |adjustment: gtk::Adjustment, by: f32, on: bool| {
                if on {
                    let max = (adjustment.upper() - adjustment.page_size()).max(0.0);
                    adjustment.set_value((adjustment.value() + by as f64).clamp(0.0, max));
                }
            };
            step(scrolled.hadjustment(), *dx, h_on);
            step(scrolled.vadjustment(), *dy, v_on);
            return Ok(());
        }
        let SyntheticInput::Key(key) = input else { unreachable!() };
        if *key == Key::Escape {
            return self.escape(id);
        }
        // Lists: GTK 4 can't inject key events, and its list keyboard
        // handling has no signals to emit, so do what it does: arrows,
        // Home and End move the selection and show it; Enter activates.
        {
            let state = self.state.borrow();
            if let Some(Widget::List(list)) = state.nodes.get(&id).map(|n| &n.widget) {
                state.lists_dirty.set(true);
                if list.mode() == SelectionMode::None {
                    return Err(ActionError::Unsupported);
                }
                list.view.grab_focus();
                let rows = list.rows();
                let selected = list.selected();
                let current = selected.first().and_then(|k| rows.iter().position(|r| r == k));
                if *key == Key::Enter {
                    if let Some(row) = selected.first() {
                        list.activate(*row);
                    }
                    return Ok(());
                }
                let last = rows.len().checked_sub(1);
                let next = match (key, current) {
                    (Key::Home, _) | (Key::Down, None) => rows.first().map(|_| 0),
                    (Key::End, _) | (Key::Up, None) => last,
                    (Key::Up, Some(i)) => Some(i.saturating_sub(1)),
                    (Key::Down, Some(i)) => Some((i + 1).min(last.unwrap_or(0))),
                    _ => return Err(ActionError::Unsupported),
                };
                if let Some(row) = next.map(|i| rows[i]) {
                    list.select(row);
                }
                return Ok(());
            }
        }
        let (widget, kind, map) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.widget.is_control() && !node.widget.widget().is_sensitive() {
                return Err(ActionError::Disabled);
            }
            (node.widget.widget().clone(), node.kind, state.by_widget.clone())
        };
        match (kind, key) {
            (
                WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput,
                Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab,
            ) => {
                // GTK 4 can't inject key events. Emit the keybinding signals
                // the keys map to instead, on the widgets that handle them:
                // the entry's inner text widget, and the window for Tab.
                let entry = widget.dynamic_cast_ref::<gtk::Editable>().ok_or(ActionError::Unsupported)?;
                // It would take the keys and ignore them; nothing can be
                // typed into it on any platform.
                if !entry.is_editable() {
                    return Err(ActionError::ReadOnly);
                }
                let focus = widget.root().and_then(|r| r.focus());
                if owning_node(&map, focus) != Some(id) {
                    entry.grab_focus();
                    // Focusing selects everything; typing should append, as
                    // after clicking past the end of the text.
                    entry.set_position(-1);
                }
                let text = entry.delegate().ok_or(ActionError::Unsupported)?;
                match key {
                    Key::Char(c) => text.emit_by_name::<()>("insert-at-cursor", &[&c.to_string()]),
                    Key::Backspace => text.emit_by_name::<()>("backspace", &[]),
                    Key::Enter => text.emit_by_name::<()>("activate", &[]),
                    _ => {
                        let window = widget.root().ok_or(ActionError::Unsupported)?;
                        window.emit_by_name::<()>("move-focus", &[&gtk::DirectionType::TabForward]);
                    }
                }
                Ok(())
            }
            // The text view's keybinding signals, as for a field: Return
            // starts a new line, and Tab inserts a tab unless the view
            // doesn't take tabs.
            (WidgetKind::TextArea, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                let view = text_view(&widget).ok_or(ActionError::Unsupported)?;
                if !view.is_editable() {
                    return Err(ActionError::ReadOnly);
                }
                let focus = widget.root().and_then(|r| r.focus());
                if owning_node(&map, focus) != Some(id) {
                    view.grab_focus();
                    // Typing appends, as after clicking past the end.
                    let buffer = view.buffer();
                    buffer.place_cursor(&buffer.end_iter());
                }
                match key {
                    Key::Char(c) => view.emit_by_name::<()>("insert-at-cursor", &[&c.to_string()]),
                    Key::Backspace => view.emit_by_name::<()>("backspace", &[]),
                    Key::Enter => view.emit_by_name::<()>("insert-at-cursor", &[&"\n"]),
                    _ if view.accepts_tab() => view.emit_by_name::<()>("insert-at-cursor", &[&"\t"]),
                    _ => {
                        let window = widget.root().ok_or(ActionError::Unsupported)?;
                        window.emit_by_name::<()>("move-focus", &[&gtk::DirectionType::TabForward]);
                    }
                }
                Ok(())
            }
            (WidgetKind::Button, Key::Enter | Key::Char(' '))
            | (WidgetKind::ToggleButton | WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => {
                self.perform(id, &A11yAction::Activate)
            }
            _ => Err(ActionError::Unsupported),
        }
    }
}
