//! Accessibility actions, done as assistive technology does them.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::{EventValue, NodeId, SelectionMode, UiEvent, WidgetKind};
use windows_core::Interface;

use super::controls::{option_texts, radio_options};
use super::fields::inner_text_box;
use super::focus::is_control;
use super::{Widget, WinUiBackend};
use crate::bindings as w;

impl WinUiBackend {
    pub(super) fn perform_action(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        match action {
            A11yAction::ContextMenuItem(item) => return self.choose_menu_item(id, *item, false),
            A11yAction::MenuItem(item) => return self.choose_menu_item(id, *item, true),
            _ => {}
        }
        // A list's rows: select or activate them, as clicking or double
        // clicking their container does.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            // A sidebar's item, as a click chooses it: the view reports it
            // to a handler that holds its own data, never our state.
            if let (A11yAction::SetValue(title), Widget::Sidebar(sidebar)) = (action, &node.widget) {
                return match sidebar.choose(title) {
                    Ok(true) => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
            // A tab, as a click picks it; the same way.
            if let (A11yAction::SetValue(title), Widget::Tabs(tabs)) = (action, &node.widget) {
                if tabs.bar.cast::<w::IControl>().and_then(|c| c.IsEnabled()).is_ok_and(|on| !on) {
                    return Err(ActionError::Disabled);
                }
                return match tabs.choose(title) {
                    Ok(true) => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
            // A header, as a click presses it.
            if let (A11yAction::PressHeader(column), Widget::List(list)) = (action, &node.widget) {
                return if list.press_header(*column) { Ok(()) } else { Err(ActionError::Unsupported) };
            }
            // A row, through its host or (a table's) any of its cells.
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                match action {
                    A11yAction::Select if list.mode() != SelectionMode::None => {
                        list.select(row).map_err(|_| ActionError::Unsupported)?
                    }
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
        }
        let (element, kind, enabled, shown_text, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let enabled = is_control(&node.widget)
                .then(|| node.element.cast::<w::IControl>().and_then(|c| c.IsEnabled()).unwrap_or(true));
            let custom = match &node.widget {
                Widget::Custom { render, props } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            (node.control().clone(), node.kind, enabled, node.shown_text.clone(), state.emitter(), custom)
        };
        if enabled == Some(false) {
            return Err(ActionError::Disabled);
        }
        if let Some((render, props)) = custom {
            return render.perform(&element, &props, action, &crate::custom::Emitter::new(events, id));
        }
        // No state borrow below: XAML may call back into our handlers.
        let peer =
            || w::FrameworkElementAutomationPeer::CreatePeerForElement(&element).map_err(|_| ActionError::Unsupported);
        match (action, kind) {
            (A11yAction::Activate, WidgetKind::Button) => {
                // What assistive technology does: the UIA Invoke pattern.
                let invoke: w::IInvokeProvider = peer()?
                    .GetPattern(w::PatternInterface::Invoke)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                invoke.Invoke().map_err(|_| ActionError::Unsupported)?;
            }
            (A11yAction::Activate, WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::ToggleButton) => {
                let toggle: w::IToggleProvider = peer()?
                    .GetPattern(w::PatternInterface::Toggle)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                toggle.Toggle().map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            // What a screen reader does: the RangeValue pattern.
            (A11yAction::Increment | A11yAction::Decrement | A11yAction::SetValue(_), WidgetKind::Slider) => {
                let range: w::IRangeValueProvider = peer()?
                    .GetPattern(w::PatternInterface::RangeValue)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                let value = match action {
                    A11yAction::SetValue(text) => text.trim().parse().map_err(|_| ActionError::Unsupported)?,
                    _ => {
                        let step = range.SmallChange().unwrap_or(1.0);
                        let step = if *action == A11yAction::Increment { step } else { -step };
                        let (min, max) = (range.Minimum().unwrap_or(f64::MIN), range.Maximum().unwrap_or(f64::MAX));
                        (range.Value().unwrap_or(0.0) + step).clamp(min, max)
                    }
                };
                range.SetValue(value).map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            // What a screen reader does: NumberBox's RangeValue pattern. Its
            // steps stop at the ends, as its own spin buttons' do.
            (A11yAction::Increment | A11yAction::Decrement | A11yAction::SetValue(_), WidgetKind::NumberInput) => {
                let range: w::IRangeValueProvider = peer()?
                    .GetPattern(w::PatternInterface::RangeValue)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                let (min, max) = (range.Minimum().unwrap_or(f64::MIN), range.Maximum().unwrap_or(f64::MAX));
                let value = match action {
                    A11yAction::SetValue(text) => {
                        text.trim().parse::<f64>().map_err(|_| ActionError::Unsupported)?.round()
                    }
                    _ => {
                        let step = range.SmallChange().unwrap_or(1.0);
                        let step = if *action == A11yAction::Increment { step } else { -step };
                        range.Value().unwrap_or(0.0) + step
                    }
                };
                range.SetValue(value.clamp(min, max)).map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                // As if the option were picked from the open drop-down.
                let combo: w::ComboBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                let index = option_texts(&combo).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                combo
                    .cast::<w::ISelector>()
                    .and_then(|s| s.SetSelectedIndex(index as i32))
                    .map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            (A11yAction::SetValue(text), WidgetKind::RadioGroup) => {
                // As its button's click chooses it.
                let group: w::RadioButtons = element.cast().map_err(|_| ActionError::Unsupported)?;
                let index = radio_options(&group).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                group.SetSelectedIndex(index as i32).map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            (A11yAction::SetValue(text), WidgetKind::TextInput | WidgetKind::TextArea) => {
                if element.cast::<w::ITextBox>().and_then(|f| f.IsReadOnly()).unwrap_or(false) {
                    return Err(ActionError::ReadOnly);
                }
                // The Value pattern where XAML offers it, else the property.
                let value = peer()?.GetPattern(w::PatternInterface::Value).and_then(|p| p.cast::<w::IValueProvider>());
                let set = match value {
                    Ok(value) => value.SetValue(text),
                    Err(_) => element.cast::<w::ITextBox>().and_then(|f| f.SetText(text)),
                };
                set.map_err(|_| ActionError::Unsupported)?;
                // An assistive technology edit is a user edit; report it now
                // rather than when XAML's (asynchronous) TextChanged arrives.
                *shown_text.borrow_mut() = text.clone();
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            // What editing its text box does; the box reports it only
            // later, and as a search at once.
            (A11yAction::SetValue(text), WidgetKind::SearchInput) => {
                *shown_text.borrow_mut() = text.clone();
                let set = match inner_text_box(&element) {
                    Some(field) => field.SetText(text),
                    None => element.cast::<w::IAutoSuggestBox>().and_then(|s| s.SetText(text)),
                };
                set.map_err(|_| ActionError::Unsupported)?;
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                events.emit(id, UiEvent::Search(text.clone()));
            }
            // Password boxes don't let UI Automation set their value.
            (A11yAction::SetValue(text), WidgetKind::PasswordInput) => {
                let field: w::IPasswordBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                *shown_text.borrow_mut() = text.clone();
                field.SetPassword(text).map_err(|_| ActionError::Unsupported)?;
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            (A11yAction::Focus, _) => {
                if !self.state.borrow().focus(id, w::FocusState::Programmatic) {
                    return Err(ActionError::Unsupported);
                }
            }
            // Native views: what a screen reader does, through the element's
            // UI Automation patterns. The control's own events report back.
            (A11yAction::Activate, WidgetKind::Native) => {
                let peer = peer()?;
                if let Ok(invoke) =
                    peer.GetPattern(w::PatternInterface::Invoke).and_then(|p| p.cast::<w::IInvokeProvider>())
                {
                    invoke.Invoke().map_err(|_| ActionError::Unsupported)?;
                } else {
                    let toggle: w::IToggleProvider = peer
                        .GetPattern(w::PatternInterface::Toggle)
                        .and_then(|p| p.cast())
                        .map_err(|_| ActionError::Unsupported)?;
                    toggle.Toggle().map_err(|_| ActionError::Unsupported)?;
                }
            }
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Native) => {
                let range: w::IRangeValueProvider = peer()?
                    .GetPattern(w::PatternInterface::RangeValue)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                let step = range.SmallChange().unwrap_or(1.0);
                let step = if *action == A11yAction::Increment { step } else { -step };
                let (min, max) = (range.Minimum().unwrap_or(f64::MIN), range.Maximum().unwrap_or(f64::MAX));
                let value = (range.Value().unwrap_or(0.0) + step).clamp(min, max);
                range.SetValue(value).map_err(|_| ActionError::Unsupported)?;
            }
            (A11yAction::SetValue(text), WidgetKind::Native) => {
                let peer = peer()?;
                if let Ok(value) =
                    peer.GetPattern(w::PatternInterface::Value).and_then(|p| p.cast::<w::IValueProvider>())
                {
                    value.SetValue(text).map_err(|_| ActionError::Unsupported)?;
                } else {
                    let range: w::IRangeValueProvider = peer
                        .GetPattern(w::PatternInterface::RangeValue)
                        .and_then(|p| p.cast())
                        .map_err(|_| ActionError::Unsupported)?;
                    let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                    range.SetValue(value).map_err(|_| ActionError::Unsupported)?;
                }
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }
}
