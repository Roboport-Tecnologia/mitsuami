//! Performing what assistive technology does.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::{EventValue, NodeId, SelectionMode, UiEvent, WidgetKind};

use crate::custom::Emitter;

use super::{KirigamiBackend, Widget, option_texts, radio_options, set_area_text, tab_titles};

impl KirigamiBackend {
    pub(super) fn perform_action(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        match action {
            A11yAction::ContextMenuItem(item) => return self.choose_menu_item(id, *item, false),
            A11yAction::MenuItem(item) => return self.choose_menu_item(id, *item, true),
            _ => {}
        }
        // A table's header: pressed as a click on it does, which sorts.
        if let A11yAction::PressHeader(column) = action {
            let state = self.state.borrow();
            return match state.nodes.get(&id).map(|n| (n.kind, &n.widget)) {
                Some((WidgetKind::Table, Widget::List(list))) if list.press_header(*column) => Ok(()),
                Some(_) => Err(ActionError::Unsupported),
                None => Err(ActionError::UnknownNode),
            };
        }
        // A list's rows (a table's, through their cells): select or
        // activate them, as a click or a double click on their delegate
        // does.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                match action {
                    A11yAction::Select if list.mode() != SelectionMode::None => list.select(row),
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
        }
        let (item, input, kind, focusable, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let item = node.widget.item();
            // Qt's controls accept actions while disabled and do nothing.
            if node.widget.is_control() && !item.bool("enabled") {
                return Err(ActionError::Disabled);
            }
            let custom = match &node.widget {
                Widget::Custom { render, props, .. } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            (item, node.widget.input_item(), node.kind, node.widget.is_focusable(), state.events.clone(), custom)
        };
        if let Some((render, props)) = custom {
            return render.perform(item, &props, action, &Emitter::new(events, id));
        }
        // No state borrow below: Qt calls back into our signal handlers.
        match (action, kind) {
            // What a screen reader does, through the items' accessible
            // actions. Like a click, they focus the control.
            (A11yAction::Activate, WidgetKind::Button) => {
                if !item.accessible_action("Press") {
                    return Err(ActionError::Unsupported);
                }
            }
            // A checkable button's action is Toggle, or Press where Qt
            // gives it only that.
            (A11yAction::Activate, WidgetKind::ToggleButton) => {
                if !item.accessible_action("Toggle") && !item.accessible_action("Press") {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Activate, WidgetKind::Checkbox | WidgetKind::Switch) => {
                if !item.accessible_action("Toggle") {
                    return Err(ActionError::Unsupported);
                }
            }
            // What the arrow keys do.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Slider | WidgetKind::NumberInput) => {
                item.set_int("mitsuamiStepBy", if *action == A11yAction::Increment { 1 } else { -1 });
            }
            (A11yAction::SetValue(text), WidgetKind::Slider | WidgetKind::NumberInput) => {
                item.set_real("mitsuamiMoveTo", text.trim().parse().map_err(|_| ActionError::Unsupported)?);
            }
            // As if the item were clicked.
            (A11yAction::SetValue(title), WidgetKind::Sidebar) => {
                let index = {
                    let state = self.state.borrow();
                    match state.nodes.get(&id).map(|n| &n.widget) {
                        Some(Widget::Sidebar { sections, .. }) => {
                            sections.iter().flat_map(|s| &s.items).position(|i| i.title == *title)
                        }
                        _ => None,
                    }
                };
                item.set_int("mitsuamiChoice", index.ok_or(ActionError::Unsupported)? as i32);
            }
            // As a double-click on the chosen item.
            (A11yAction::Activate, WidgetKind::Sidebar) => {
                let index = item.int("mitsuamiSelected");
                if index < 0 {
                    return Err(ActionError::Unsupported);
                }
                item.set_int("mitsuamiActivation", index);
            }
            // As if its tab were clicked.
            (A11yAction::SetValue(title), WidgetKind::Tabs) => {
                let index = tab_titles(item).iter().position(|t| t == title).ok_or(ActionError::Unsupported)?;
                item.set_int("mitsuamiChoice", index as i32);
            }
            // As if its button were clicked.
            (A11yAction::SetValue(text), WidgetKind::RadioGroup) => {
                let index = radio_options(item).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                item.set_int("mitsuamiChoice", index as i32);
            }
            // As if the option were picked from the pop-up.
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let index = option_texts(item).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                item.set_int("mitsuamiChoice", index as i32);
            }
            (A11yAction::SetValue(text), WidgetKind::TextInput | WidgetKind::PasswordInput) => {
                if item.bool("readOnly") {
                    return Err(ActionError::ReadOnly);
                }
                item.set_str("text", text);
                // The caret ends up after the new text, as if it was typed.
                item.set_int("cursorPosition", text.chars().count() as i32);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            // Searched for at once, as a user's edit that's done; Kirigami's
            // own search for it isn't reported.
            (A11yAction::SetValue(text), WidgetKind::SearchInput) => {
                item.set_str("mitsuamiShown", text);
                item.set_str("text", text);
                item.set_int("cursorPosition", text.chars().count() as i32);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                events.emit(id, UiEvent::Search(text.clone()));
            }
            (A11yAction::SetValue(text), WidgetKind::TextArea) => {
                if item.bool("readOnly") {
                    return Err(ActionError::ReadOnly);
                }
                // One edit, reported once, the caret after it.
                set_area_text(item, text);
                input.set_int("cursorPosition", text.chars().count() as i32);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            // Native views: the item's own accessible actions.
            (A11yAction::Activate, WidgetKind::Native) => {
                if !item.accessible_action("Press") && !item.accessible_action("Toggle") {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Native) => {
                let up = *action == A11yAction::Increment;
                if !item.accessible_action(if up { "Increase" } else { "Decrease" }) {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Focus, _) => {
                if !focusable {
                    return Err(ActionError::Unsupported);
                }
                // The window's focus observer reports the change.
                input.force_focus();
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }

    /// What assistive technology does once it has shown the menu (the
    /// node's context menu, or a menu button's own): trigger the item's
    /// action, as clicking it does. A disabled item (or one in a disabled
    /// container) gets no input, so it shows no menu.
    fn choose_menu_item(&self, id: NodeId, item: u32, button: bool) -> Result<(), ActionError> {
        let action = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if !node.widget.item().bool("enabled") {
                return Err(ActionError::Disabled);
            }
            match (&node.widget, button) {
                // A sidebar's items each have a menu of their own.
                (Widget::Sidebar { page, menus, .. }, false) => menus.action(*page, item),
                _ => {
                    let menu = if button { &node.button_menu } else { &node.context_menu };
                    menu.as_ref().and_then(|menu| menu.action(item))
                }
            }
            .ok_or(ActionError::Unsupported)?
        };
        if !action.bool("enabled") {
            return Err(ActionError::Disabled);
        }
        // No state borrow: the action's handler reports the choice.
        action.invoke("trigger");
        Ok(())
    }
}
