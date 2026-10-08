//! Performing accessibility actions, as assistive technology does.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::{EventValue, NodeId, SelectionMode, UiEvent, WidgetKind};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{Message, msg_send};
use objc2_app_kit::{NSPopUpButton, NSScrollView, NSSlider, NSTextField, NSTextView, NSView};
use objc2_foundation::NSArray;

use crate::custom::Emitter;
use crate::number_field::NumberField;

use super::{AppKitBackend, Widget, ns};

impl AppKitBackend {
    pub(super) fn perform_action(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        // A sidebar's items each have a menu of their own.
        if let A11yAction::ContextMenuItem(_) | A11yAction::Activate = action {
            let state = self.state.borrow();
            if let Some(Widget::Sidebar(sidebar)) = state.nodes.get(&id).map(|n| &n.widget) {
                return match action {
                    A11yAction::ContextMenuItem(item) => sidebar.choose_menu_item(*item),
                    _ if sidebar.activate() => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
        }
        if let A11yAction::ContextMenuItem(item) = action {
            return self.choose_context_menu_item(id, *item);
        }
        if let A11yAction::MenuItem(item) = action {
            return self.choose_pull_down_item(id, *item);
        }
        // A list's rows: select or activate them in the table. The table
        // only calls back into its own data, never into our state.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                match action {
                    // A table's row, through any of its cells.
                    A11yAction::Select if list.mode() != SelectionMode::None => {
                        list.set_selected(&[row]);
                        list.report_selection();
                    }
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
        }
        if let A11yAction::PressHeader(column) = action {
            let state = self.state.borrow();
            return match state.nodes.get(&id).map(|n| (n.kind, &n.widget)) {
                Some((WidgetKind::Table, Widget::List(list))) if list.press_header(*column) => Ok(()),
                Some(_) => Err(ActionError::Unsupported),
                None => Err(ActionError::UnknownNode),
            };
        }
        let (widget_view, control_enabled, kind, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let custom = match &node.widget {
                Widget::Custom { render, props, .. } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            let enabled = node.widget.enabled();
            (node.widget.view().retain(), enabled, node.kind, state.events.clone(), custom)
        };
        if control_enabled == Some(false) {
            return Err(ActionError::Disabled);
        }
        if let Some((render, props)) = custom {
            return render.perform(&widget_view, &props, action, &Emitter::new(events, id));
        }
        // No state borrow below: AppKit calls back into our targets.
        match (action, kind) {
            (
                A11yAction::Activate,
                WidgetKind::Button | WidgetKind::ToggleButton | WidgetKind::Checkbox | WidgetKind::Switch,
            ) => {
                // The press action assistive technology uses. Its return
                // value is unreliable for windows that aren't on screen (it
                // reports NO after pressing), so it is ignored.
                let _: bool = unsafe { msg_send![&*widget_view, accessibilityPerformPress] };
            }
            // Native views: whatever their accessibility element does.
            // Return values are ignored for the same reason as above.
            (A11yAction::Activate, WidgetKind::Native) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformPress] };
            }
            (A11yAction::Increment, WidgetKind::Native) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformIncrement] };
            }
            (A11yAction::Decrement, WidgetKind::Native) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformDecrement] };
            }
            // What VoiceOver's increment and decrement do.
            (A11yAction::Increment, WidgetKind::Slider) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformIncrement] };
            }
            (A11yAction::Decrement, WidgetKind::Slider) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformDecrement] };
            }
            (A11yAction::SetValue(text), WidgetKind::Slider) => {
                let slider: &NSSlider = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                // As if dragged there: the slider sends its action.
                slider.setDoubleValue(value);
                unsafe { slider.sendAction_to(slider.action(), slider.target().as_deref()) };
            }
            // What VoiceOver does to the stepper; it sends the stepper's action.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::NumberInput) => {
                let field: &NumberField = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                let stepper = a11y_element(field.stepper());
                let _: bool = if *action == A11yAction::Increment {
                    unsafe { msg_send![&*stepper, accessibilityPerformIncrement] }
                } else {
                    unsafe { msg_send![&*stepper, accessibilityPerformDecrement] }
                };
            }
            // As if typed into the field and committed.
            (A11yAction::SetValue(text), WidgetKind::NumberInput) => {
                let field: &NumberField = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                field.field().setStringValue(&ns(text));
                unsafe { field.field().sendAction_to(field.field().action(), field.field().target().as_deref()) };
            }
            // As if the user clicked the item: the table reports it, from
            // the sidebar's own data, never our state.
            (A11yAction::SetValue(title), WidgetKind::Sidebar) => {
                let state = self.state.borrow();
                let Some(Widget::Sidebar(sidebar)) = state.nodes.get(&id).map(|n| &n.widget) else {
                    return Err(ActionError::Unsupported);
                };
                if !sidebar.choose(title) {
                    return Err(ActionError::Unsupported);
                }
            }
            // As if the user clicked the button: its action reports it.
            (A11yAction::SetValue(option), WidgetKind::RadioGroup) => {
                let state = self.state.borrow();
                let Some(Widget::RadioGroup(group)) = state.nodes.get(&id).map(|n| &n.widget) else {
                    return Err(ActionError::Unsupported);
                };
                if !group.is_enabled() {
                    return Err(ActionError::Disabled);
                }
                if !group.choose(option) {
                    return Err(ActionError::Unsupported);
                }
            }
            // As if the user clicked the tab: the delegate reports it.
            (A11yAction::SetValue(title), WidgetKind::Tabs) => {
                let state = self.state.borrow();
                let Some(Widget::Tabs(tabs)) = state.nodes.get(&id).map(|n| &n.widget) else {
                    return Err(ActionError::Unsupported);
                };
                if !tabs.choose(title) {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                // As if the item were picked from the open menu: the pop-up
                // button chooses it and sends its action.
                let popup: &NSPopUpButton = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                let index = popup.itemTitles().iter().position(|t| t.to_string() == *text);
                let (Some(index), Some(menu)) = (index, popup.menu()) else { return Err(ActionError::Unsupported) };
                menu.performActionForItemAtIndex(index as isize);
            }
            (
                A11yAction::SetValue(text),
                WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput,
            ) => {
                let field: &NSTextField = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                if !field.isEditable() {
                    return Err(ActionError::ReadOnly);
                }
                field.setStringValue(&ns(text));
                // Programmatic edits don't notify the delegate; assistive
                // technology edits are user edits, so report one, and the
                // search a search field's edits ask for.
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                if kind == WidgetKind::SearchInput {
                    events.emit(id, UiEvent::Search(text.clone()));
                }
            }
            (A11yAction::SetValue(text), WidgetKind::TextArea) => {
                let scroll: &NSScrollView = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                let view = scroll.documentView().and_then(|v| v.downcast::<NSTextView>().ok());
                let view = view.ok_or(ActionError::Unsupported)?;
                if !view.isEditable() {
                    return Err(ActionError::ReadOnly);
                }
                // `setString:` doesn't notify the delegate either.
                view.setString(&ns(text));
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            (A11yAction::Focus, _) => {
                let view = self.state.borrow().nodes.get(&id).map(|n| n.widget.key_view()).unwrap_or(widget_view);
                let window = view.window().ok_or(ActionError::Unsupported)?;
                if !window.makeFirstResponder(Some(&view)) {
                    return Err(ActionError::Unsupported);
                }
                // The window delegate reports the focus change.
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }
}

/// The object assistive technology acts on for a view: the view itself, or
/// for views that aren't accessibility elements (an `NSStepper`), their
/// single accessibility child (its cell), as VoiceOver finds it.
fn a11y_element(view: &NSView) -> Retained<AnyObject> {
    let mut element: Retained<AnyObject> = view.retain().into();
    loop {
        let is_element: bool = unsafe { msg_send![&*element, isAccessibilityElement] };
        let children: Option<Retained<NSArray<AnyObject>>> = unsafe { msg_send![&*element, accessibilityChildren] };
        match children {
            Some(children) if !is_element && children.len() == 1 => element = children.objectAtIndex(0),
            _ => return element,
        }
    }
}
