//! Performing accessibility actions, as assistive technology does.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::services::menu_item_by_id;
use mitsuami_core::{EventValue, NodeId, Prop, SelectionMode, UiEvent, WidgetKind, find_prop};

use super::HeadlessBackend;

impl HeadlessBackend {
    pub(super) fn perform_action(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        let mut state = self.state.borrow_mut();
        let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
        if find_prop!(node.props, Enabled) == Some(false) {
            return Err(ActionError::Disabled);
        }
        let kind = node.kind;
        // A menu button's own menu, or any node's context menu.
        let chosen = match action {
            A11yAction::ContextMenuItem(item) => {
                Some((find_prop!(node.props, ContextMenu), *item, UiEvent::ContextMenuItem(*item)))
            }
            A11yAction::MenuItem(item) if kind == WidgetKind::MenuButton => {
                Some((find_prop!(node.props, Menu), *item, UiEvent::MenuItem(*item)))
            }
            _ => None,
        };
        if let Some((menu, item, event)) = chosen {
            let menu = menu.unwrap_or_default();
            return match menu_item_by_id(&menu, item).map(|item| item.enabled) {
                Some(true) => {
                    state.emit(id, event);
                    Ok(())
                }
                Some(false) => Err(ActionError::Disabled),
                None => Err(ActionError::Unsupported),
            };
        }
        match (action, kind) {
            (A11yAction::Activate, WidgetKind::Button) => {
                state.focus(id);
                state.emit(id, UiEvent::Click);
            }
            (A11yAction::Activate, WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::ToggleButton) => {
                // Out of the mixed state, a click checks the box, as on
                // AppKit and Qt.
                let props = &state.nodes[&id].props;
                let checked = find_prop!(props, Mixed) == Some(true) || !find_prop!(props, Checked).unwrap_or(false);
                if find_prop!(props, Mixed) == Some(true) {
                    state.set_prop(id, Prop::Mixed(false));
                }
                state.set_prop(id, Prop::Checked(checked));
                state.focus(id);
                state.emit(id, UiEvent::Changed(EventValue::Bool(checked)));
            }
            (
                A11yAction::SetValue(text),
                WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea,
            ) => {
                if find_prop!(state.nodes[&id].props, ReadOnly) == Some(true) {
                    return Err(ActionError::ReadOnly);
                }
                state.set_prop(id, Prop::Value(text.clone()));
                state.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                if kind == WidgetKind::SearchInput {
                    state.emit(id, UiEvent::Search(text.clone()));
                }
            }
            (A11yAction::SetValue(_) | A11yAction::Increment | A11yAction::Decrement, WidgetKind::Slider) => {
                let props = &state.nodes[&id].props;
                let (min, max) = props
                    .iter()
                    .find_map(|p| match p {
                        Prop::Range { min, max } => Some((*min, *max)),
                        _ => None,
                    })
                    .unwrap_or((0.0, 1.0));
                let value = find_prop!(props, Number).unwrap_or(min);
                // Steps by the step, or a tenth of the range without one.
                let step = find_prop!(props, Step).flatten().unwrap_or((max - min) / 10.0);
                let value = match action {
                    A11yAction::SetValue(text) => text.trim().parse().map_err(|_| ActionError::Unsupported)?,
                    A11yAction::Increment => value + step,
                    _ => value - step,
                };
                let value = value.clamp(min, max);
                state.set_prop(id, Prop::Number(value));
                state.emit(id, UiEvent::Changed(EventValue::Number(value)));
            }
            (A11yAction::SetValue(_) | A11yAction::Increment | A11yAction::Decrement, WidgetKind::NumberInput) => {
                let props = &state.nodes[&id].props;
                let (min, max) = props
                    .iter()
                    .find_map(|p| match p {
                        Prop::Range { min, max } => Some((*min, *max)),
                        _ => None,
                    })
                    .unwrap_or((0.0, 100.0));
                let value = find_prop!(props, Number).unwrap_or(min);
                let step = find_prop!(props, Step).flatten().unwrap_or(1.0);
                let wraps = find_prop!(props, WrapAround) == Some(true);
                // Whole numbers, clamped at the ends, as GTK, Qt and WinUI
                // do it; stepped past one end of a range that wraps, the
                // other end, as every platform's wrapping does it.
                let value = match action {
                    A11yAction::SetValue(text) => {
                        text.trim().parse::<f64>().map_err(|_| ActionError::Unsupported)?.round()
                    }
                    A11yAction::Increment if wraps && value + step > max => min,
                    A11yAction::Decrement if wraps && value - step < min => max,
                    A11yAction::Increment => value + step,
                    _ => value - step,
                };
                let value = value.clamp(min, max);
                state.set_prop(id, Prop::Number(value));
                state.emit(id, UiEvent::Changed(EventValue::Number(value)));
            }
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let options = find_prop!(state.nodes[&id].props, Options).unwrap_or_default();
                let index = options.iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                state.set_prop(id, Prop::SelectedIndex(Some(index)));
                state.emit(id, UiEvent::Changed(EventValue::Index(index)));
            }
            // A radio button, by its option, as a screen reader presses one.
            (A11yAction::SetValue(text), WidgetKind::RadioGroup) => {
                let options = find_prop!(state.nodes[&id].props, Options).unwrap_or_default();
                let index = options.iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                if find_prop!(state.nodes[&id].props, SelectedIndex).flatten() != Some(index) {
                    state.set_prop(id, Prop::SelectedIndex(Some(index)));
                    state.emit(id, UiEvent::Changed(EventValue::Index(index)));
                }
            }
            // An item, by its title, as a screen reader selects one.
            (A11yAction::SetValue(title), WidgetKind::Sidebar) => {
                let sections = find_prop!(state.nodes[&id].props, Sections).unwrap_or_default();
                let mut titles = sections.iter().flat_map(|s| s.items.iter().map(|i| &i.title));
                let index = titles.position(|t| t == title).ok_or(ActionError::Unsupported)?;
                if find_prop!(state.nodes[&id].props, SelectedIndex).flatten() != Some(index) {
                    state.set_prop(id, Prop::SelectedIndex(Some(index)));
                    state.emit(id, UiEvent::Changed(EventValue::Index(index)));
                }
            }
            // A tab, by its title, as a screen reader picks one.
            (A11yAction::SetValue(title), WidgetKind::Tabs) => {
                let titles = find_prop!(state.nodes[&id].props, TabTitles).unwrap_or_default();
                let index = titles.iter().position(|t| t == title).ok_or(ActionError::Unsupported)?;
                if find_prop!(state.nodes[&id].props, SelectedIndex).flatten() != Some(index) {
                    state.set_prop(id, Prop::SelectedIndex(Some(index)));
                    state.emit(id, UiEvent::Changed(EventValue::Index(index)));
                }
            }
            (
                A11yAction::Focus,
                WidgetKind::Button
                | WidgetKind::ToggleButton
                | WidgetKind::TextInput
                | WidgetKind::PasswordInput
                | WidgetKind::SearchInput
                | WidgetKind::TextArea
                | WidgetKind::Checkbox
                | WidgetKind::Switch
                | WidgetKind::Select
                | WidgetKind::RadioGroup
                | WidgetKind::Slider
                | WidgetKind::NumberInput
                | WidgetKind::List
                | WidgetKind::Sidebar
                | WidgetKind::Tabs,
            ) => state.focus(id),
            (A11yAction::Select | A11yAction::Activate, WidgetKind::Container) => {
                let row = find_prop!(state.nodes[&id].props, Row);
                let list = state.nodes[&id].parent.filter(|p| state.nodes[p].kind == WidgetKind::List);
                let (Some(row), Some(list)) = (row, list) else { return Err(ActionError::Unsupported) };
                if *action == A11yAction::Activate {
                    state.emit(list, UiEvent::RowActivated(row));
                } else if find_prop!(state.nodes[&list].props, SelectionMode).unwrap_or_default() == SelectionMode::None
                {
                    return Err(ActionError::Unsupported);
                } else {
                    state.select(list, vec![row]);
                }
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }
}
