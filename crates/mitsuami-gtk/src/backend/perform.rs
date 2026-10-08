//! Performing accessibility actions.

use gtk::prelude::*;
use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::{EventValue, NodeId, SelectionMode, UiEvent, WidgetKind};

use crate::custom::Emitter;
use crate::radio::RadioGroup;
use crate::services::{ContextMenu, choose_context_item};

use super::native_state::option_texts;
use super::text::text_view;
use super::{GtkBackend, Widget};

impl GtkBackend {
    pub(super) fn perform_action(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        // A menu button's own menu, without opening it.
        if let A11yAction::MenuItem(item) = action {
            let actions = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                let Widget::MenuButton { button, menu } = &node.widget else { return Err(ActionError::Unsupported) };
                if !button.is_sensitive() {
                    return Err(ActionError::Disabled);
                }
                menu.chooser()
            };
            return choose_context_item(&actions, *item);
        }
        // Any node's, list rows' included. A disabled widget shows none.
        if let A11yAction::ContextMenuItem(item) = action {
            let actions = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                if !node.widget.focus_widget().is_sensitive() {
                    return Err(ActionError::Disabled);
                }
                // A sidebar's items each have a menu of their own.
                let sidebar = match &node.widget {
                    Widget::Sidebar(sidebar) => sidebar.chooser(*item),
                    _ => None,
                };
                node.context_menu.as_ref().map(ContextMenu::chooser).or(sidebar).ok_or(ActionError::Unsupported)?
            };
            return choose_context_item(&actions, *item);
        }
        // A list's rows: what GTK's own row actions do. Handlers only
        // touch the list's data and emit.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                state.lists_dirty.set(true);
                match action {
                    A11yAction::Select if list.mode() != SelectionMode::None => list.select(row),
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
            if let (A11yAction::PressHeader(column), Widget::List(list)) = (action, &node.widget) {
                state.lists_dirty.set(true);
                return if list.press_header(*column) { Ok(()) } else { Err(ActionError::Unsupported) };
            }
            if let (A11yAction::Focus, Widget::List(list)) = (action, &node.widget) {
                return if list.view.grab_focus() { Ok(()) } else { Err(ActionError::Unsupported) };
            }
            // A sidebar's item, as the user clicks it: the list reports it,
            // from its own data, never our state.
            if let Widget::Sidebar(sidebar) = &node.widget {
                return match action {
                    A11yAction::SetValue(title) if sidebar.choose(title) => Ok(()),
                    A11yAction::Focus if sidebar.list.grab_focus() => Ok(()),
                    A11yAction::Activate if sidebar.activate() => Ok(()),
                    A11yAction::ScrollIntoView => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
            // A radio button, as the user clicks it: its toggle reports it.
            if let Widget::RadioGroup(group) = &node.widget {
                if !group.column.is_sensitive() && !matches!(action, A11yAction::ScrollIntoView) {
                    return Err(ActionError::Disabled);
                }
                let group = RadioGroup::clone(group);
                drop(state);
                return match action {
                    A11yAction::SetValue(option) if group.has(option) => {
                        group.choose(option);
                        Ok(())
                    }
                    A11yAction::Focus if group.focus() => Ok(()),
                    A11yAction::ScrollIntoView => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
            // A tab, as the user clicks it: the view reports it.
            if let Widget::Tabs(tabs) = &node.widget {
                return match action {
                    A11yAction::SetValue(title) if tabs.choose(title) => Ok(()),
                    A11yAction::Focus if tabs.focus() => Ok(()),
                    A11yAction::ScrollIntoView => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
        }
        let (widget, kind, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.widget.is_control() && !node.widget.widget().is_sensitive() {
                return Err(ActionError::Disabled);
            }
            let custom = match &node.widget {
                Widget::Custom { render, props, .. } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            (node.widget.widget().clone(), node.kind, state.events.clone(), custom)
        };
        if let Some((render, props)) = custom {
            return render.perform(&widget, &props, action, &Emitter::new(events, id));
        }
        // No state borrow below: GTK calls back into our signal handlers.
        match (action, kind) {
            // What GTK's accessibility actions do. `gtk_widget_activate`
            // would click a button only after its press animation.
            (A11yAction::Activate, WidgetKind::Button) => {
                widget.downcast_ref::<gtk::Button>().ok_or(ActionError::Unsupported)?.emit_clicked()
            }
            (A11yAction::Activate, WidgetKind::ToggleButton) => {
                let toggle = widget.downcast_ref::<gtk::ToggleButton>().ok_or(ActionError::Unsupported)?;
                toggle.set_active(!toggle.is_active());
            }
            (A11yAction::Activate, WidgetKind::Checkbox) => {
                let check = widget.downcast_ref::<gtk::CheckButton>().ok_or(ActionError::Unsupported)?;
                check.set_active(!check.is_active());
            }
            (A11yAction::Activate, WidgetKind::Switch) => {
                let switch = widget.downcast_ref::<gtk::Switch>().ok_or(ActionError::Unsupported)?;
                switch.set_active(!switch.is_active());
            }
            // What GTK's accessible increment and decrement do: a step.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Slider) => {
                let adjustment = widget.downcast_ref::<gtk::Range>().ok_or(ActionError::Unsupported)?.adjustment();
                let step = adjustment.step_increment();
                adjustment.set_value(adjustment.value() + if *action == A11yAction::Increment { step } else { -step });
            }
            // What GTK's accessible increment and decrement do on a spin
            // button: a step, stopping at the ends.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::NumberInput) => {
                let spin = widget.downcast_ref::<gtk::SpinButton>().ok_or(ActionError::Unsupported)?;
                let up = *action == A11yAction::Increment;
                spin.spin(if up { gtk::SpinType::StepForward } else { gtk::SpinType::StepBackward }, 0.0);
            }
            // As if typed and committed: whole numbers, clamped to the range.
            (A11yAction::SetValue(text), WidgetKind::NumberInput) => {
                let spin = widget.downcast_ref::<gtk::SpinButton>().ok_or(ActionError::Unsupported)?;
                let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                spin.set_value(value.round());
            }
            // As if dragged there: `change-value`, which snaps to the step.
            (A11yAction::SetValue(text), WidgetKind::Slider) => {
                let range = widget.downcast_ref::<gtk::Range>().ok_or(ActionError::Unsupported)?;
                let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                range.emit_by_name::<bool>("change-value", &[&gtk::ScrollType::Jump, &value]);
            }
            // What choosing from the pop-up does.
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let dropdown = widget.downcast_ref::<gtk::DropDown>().ok_or(ActionError::Unsupported)?;
                let options = dropdown.model().and_downcast::<gtk::StringList>().ok_or(ActionError::Unsupported)?;
                let index = option_texts(&options).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                dropdown.set_selected(index as u32);
            }
            (
                A11yAction::SetValue(text),
                WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput,
            ) => {
                let entry = widget.dynamic_cast_ref::<gtk::Editable>().ok_or(ActionError::Unsupported)?;
                if !entry.is_editable() {
                    return Err(ActionError::ReadOnly);
                }
                // One edit, one event (`set_text` may report the deletion
                // and the insertion separately).
                events.muted(|| entry.set_text(text));
                // The caret ends up after the new text, as if it was typed.
                entry.set_position(-1);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                // An edit searches, once: the muted `set_text` isn't
                // searched for when GTK's timer fires.
                if kind == WidgetKind::SearchInput {
                    events.emit(id, UiEvent::Search(text.clone()));
                }
            }
            (A11yAction::SetValue(text), WidgetKind::TextArea) => {
                let view = text_view(&widget).ok_or(ActionError::Unsupported)?;
                if !view.is_editable() {
                    return Err(ActionError::ReadOnly);
                }
                // One edit, one event, the caret after it, as for a field.
                let buffer = view.buffer();
                events.muted(|| buffer.set_text(text));
                buffer.place_cursor(&buffer.end_iter());
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            // Native views: what GTK's accessibility actions do for the
            // widget (activate it, step a range or spin button).
            (A11yAction::Activate, WidgetKind::Native) => {
                if !widget.activate() {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Native) => {
                let up = *action == A11yAction::Increment;
                if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
                    spin.spin(if up { gtk::SpinType::StepForward } else { gtk::SpinType::StepBackward }, 0.0);
                } else if let Some(range) = widget.downcast_ref::<gtk::Range>() {
                    let adjustment = range.adjustment();
                    let step = adjustment.step_increment();
                    adjustment.set_value(adjustment.value() + if up { step } else { -step });
                } else {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Focus, _) => {
                // The window's focus observer reports the change.
                if !widget.grab_focus() {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }
}
