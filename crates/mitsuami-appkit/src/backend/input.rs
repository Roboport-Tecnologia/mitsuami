//! Synthesizing user input: clicks, scrolling, file drops and keys.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
use mitsuami_core::services::Shortcut;
use mitsuami_core::{NodeId, ScrollAxes, WidgetKind};
use objc2::{Message, msg_send, sel};
use objc2_app_kit::{
    NSEvent, NSEventModifierFlags, NSEventType, NSStandardKeyBindingResponding, NSTextField, NSTextView, NSView,
};
use objc2_foundation::{NSPoint, NSRange, NSSize};

use super::scrolling::scroll_to;
use super::{AppKitBackend, Widget, ns};

impl AppKitBackend {
    pub(super) fn synthesize_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        // Through the tracker's own methods, with the event AppKit sends.
        if let SyntheticInput::PointerEnter | SyntheticInput::PointerLeave = input {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let (tracker, _) = node.hover.as_ref().ok_or(ActionError::Unsupported)?;
            let (tracker, view) = (tracker.clone(), node.widget.view().retain());
            drop(state);
            return tracker.send(&view, *input == SyntheticInput::PointerEnter).ok_or(ActionError::Unsupported);
        }
        // What the recognizer's target does once it counted two clicks.
        if *input == SyntheticInput::DoubleClick {
            let clicker = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                node.double_click.as_ref().map(|(clicker, _)| clicker.clone()).ok_or(ActionError::Unsupported)?
            };
            clicker.fire();
            return Ok(());
        }
        let surface = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::GpuSurface(view)) => Some(view.clone()),
            _ => None,
        };
        if let Some(view) = surface {
            return crate::surface::synthesize(&view, input);
        }
        // Through the host's own dragging methods, with the paths the
        // pasteboard would give them.
        if let SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) = input {
            let host = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Host(host) | Widget::Group { host, .. }) if host.file_drop().is_some() => host.clone(),
                Some(_) => return Err(ActionError::Unsupported),
                None => return Err(ActionError::UnknownNode),
            };
            match input {
                SyntheticInput::DragFiles(paths) => {
                    host.drag_files(paths);
                }
                SyntheticInput::DropFiles(paths) => {
                    host.drop_files(paths);
                }
                _ => host.drag_leave(),
            }
            return Ok(());
        }
        if let SyntheticInput::Click(point) = input {
            // Drawn widgets only: native controls track the mouse in a
            // loop of their own, waiting for real events.
            let view = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Drawn { view, .. }) => view.clone(),
                Some(_) => return Err(ActionError::Unsupported),
                None => return Err(ActionError::UnknownNode),
            };
            let window = view.window().ok_or(ActionError::Unsupported)?;
            let location = view.convertPoint_toView(NSPoint::new(point.x as f64, point.y as f64), None);
            for (kind, up) in [(NSEventType::LeftMouseDown, false), (NSEventType::LeftMouseUp, true)] {
                let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                    kind,
                    location,
                    NSEventModifierFlags::empty(),
                    0.0,
                    window.windowNumber(),
                    None,
                    0,
                    1,
                    if up { 0.0 } else { 1.0 },
                )
                .ok_or(ActionError::Unsupported)?;
                if up { view.mouseUp(&event) } else { view.mouseDown(&event) }
            }
            return Ok(());
        }
        if let SyntheticInput::Scroll { dx, dy } = input {
            let (scroll, axes) = match self.state.borrow().nodes.get(&id).map(|n| (&n.widget, n.scroll_axes)) {
                Some((Widget::Scroll(scroll), axes)) => (scroll.clone(), axes),
                Some((Widget::List(list), _)) => {
                    let axes =
                        if list.scroll.hasHorizontalScroller() { ScrollAxes::Both } else { ScrollAxes::Vertical };
                    (list.scroll.clone(), axes)
                }
                Some(_) => return Err(ActionError::Unsupported),
                None => return Err(ActionError::UnknownNode),
            };
            let clip = scroll.contentView();
            // What shows below a table's header.
            let (start, insets) = (crate::classes::scrolled(&clip), clip.contentInsets());
            let visible = NSSize::new(clip.bounds().size.width - insets.left, clip.bounds().size.height - insets.top);
            let content = scroll.documentView().map_or(visible, |d| d.frame().size);
            let clamp = |v: f64, content: f64, visible: f64, on: bool| {
                if on { v.clamp(0.0, (content - visible).max(0.0)) } else { 0.0 }
            };
            let origin = NSPoint::new(
                clamp(start.x + *dx as f64, content.width, visible.width, axes.horizontal()),
                clamp(start.y + *dy as f64, content.height, visible.height, axes.vertical()),
            );
            scroll_to(&scroll, origin);
            return Ok(());
        }
        let key = match input {
            SyntheticInput::Key(key) => key,
            SyntheticInput::Shortcut(shortcut) => {
                // Text fields take keys with modifiers as editing
                // commands, which aren't simulated.
                let kind = self.state.borrow().nodes.get(&id).ok_or(ActionError::UnknownNode)?.kind;
                if matches!(
                    kind,
                    WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea
                ) {
                    return Err(ActionError::Unsupported);
                }
                return self.send_key(id, *shortcut);
            }
            _ => unreachable!(),
        };
        if *key == Key::Escape {
            return self.escape(id);
        }
        let table = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::List(list)) => Some(list.table.clone()),
            _ => None,
        };
        if let Some(table) = table {
            // Real key events, through the table's own key handling: arrows
            // move the selection and scroll to it, Home and End scroll, and
            // Return activates (ours).
            let (code, character) = match key {
                Key::Up => (126, '\u{f700}'),
                Key::Down => (125, '\u{f701}'),
                Key::Home => (115, '\u{f729}'),
                Key::End => (119, '\u{f72b}'),
                Key::Enter => (36, '\r'),
                _ => return self.send_key(id, Shortcut::new(*key)),
            };
            let window = table.window().ok_or(ActionError::Unsupported)?;
            window.makeFirstResponder(Some(&table));
            let characters = ns(&character.to_string());
            let flags = if *key == Key::Enter {
                NSEventModifierFlags::empty()
            } else {
                NSEventModifierFlags::Function | NSEventModifierFlags::NumericPad
            };
            let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                NSEventType::KeyDown,
                NSPoint::new(0.0, 0.0),
                flags,
                0.0,
                window.windowNumber(),
                None,
                &characters,
                &characters,
                false,
                code,
            )
            .ok_or(ActionError::Unsupported)?;
            table.keyDown(&event);
            return Ok(());
        }
        let (view, kind) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.widget.enabled() == Some(false) {
                return Err(ActionError::Disabled);
            }
            (node.widget.key_view(), node.kind)
        };
        match (kind, key) {
            (
                WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput,
                Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab,
            ) => {
                // Drive the field editor, the object that receives real
                // keystrokes: text goes through `insertText:`, and editing
                // keys through `doCommandBySelector:`, which is what
                // `interpretKeyEvents:` does, delegate hooks included.
                let window = view.window().ok_or(ActionError::Unsupported)?;
                let field: &NSTextField = view.downcast_ref().ok_or(ActionError::Unsupported)?;
                if !field.isEditable() {
                    return Err(ActionError::ReadOnly);
                }
                let just_focused = field.currentEditor().is_none();
                if just_focused {
                    window.makeFirstResponder(Some(&view));
                }
                let editor = field.currentEditor().ok_or(ActionError::Unsupported)?;
                if just_focused {
                    // Focusing selects everything; typing should append, as
                    // after clicking past the end of the text.
                    let end = editor.string().length();
                    editor.setSelectedRange(NSRange::new(end, 0));
                }
                let command = match key {
                    Key::Char(c) => {
                        let text = ns(&c.to_string());
                        let _: () = unsafe { msg_send![&*editor, insertText: &*text] };
                        return Ok(());
                    }
                    Key::Backspace => sel!(deleteBackward:),
                    Key::Enter => sel!(insertNewline:),
                    _ => sel!(insertTab:),
                };
                unsafe { editor.doCommandBySelector(command) };
                Ok(())
            }
            // The text view takes the keys itself, as it takes real ones:
            // Return starts a new line and Tab inserts a tab.
            (WidgetKind::TextArea, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                let window = view.window().ok_or(ActionError::Unsupported)?;
                let text: &NSTextView = view.downcast_ref().ok_or(ActionError::Unsupported)?;
                if !text.isEditable() {
                    return Err(ActionError::ReadOnly);
                }
                let first = window.firstResponder();
                if !first.is_some_and(|r| std::ptr::eq(&*r as *const _ as *const NSView, &*view as *const NSView)) {
                    window.makeFirstResponder(Some(&view));
                    // Typing should append, as after clicking past the end.
                    text.setSelectedRange(NSRange::new(text.string().length(), 0));
                }
                let command = match key {
                    Key::Char(c) => {
                        let typed = ns(&c.to_string());
                        let _: () = unsafe { msg_send![text, insertText: &*typed] };
                        return Ok(());
                    }
                    Key::Backspace => sel!(deleteBackward:),
                    Key::Enter => sel!(insertNewline:),
                    _ => sel!(insertTab:),
                };
                unsafe { text.doCommandBySelector(command) };
                Ok(())
            }
            (WidgetKind::Button, Key::Enter | Key::Char(' '))
            | (WidgetKind::ToggleButton | WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => {
                self.perform(id, &A11yAction::Activate)
            }
            _ => self.send_key(id, Shortcut::new(*key)),
        }
    }

    /// A key the control doesn't use, as the keyboard sends it: a real key
    /// event to its window, the control focused, which goes up the
    /// responder chain to a host or table that takes it. `Unsupported` if
    /// none around does: AppKit would beep.
    fn send_key(&self, id: NodeId, shortcut: Shortcut) -> Result<(), ActionError> {
        let view = self.state.borrow().nodes.get(&id).ok_or(ActionError::UnknownNode)?.widget.key_view();
        if !crate::keys::taken_around(&view, shortcut) {
            return Err(ActionError::Unsupported);
        }
        let window = view.window().ok_or(ActionError::Unsupported)?;
        if view.acceptsFirstResponder() {
            window.makeFirstResponder(Some(&view));
        }
        let event = crate::keys::key_event(&window, shortcut).ok_or(ActionError::Unsupported)?;
        window.sendEvent(&event);
        Ok(())
    }

    /// Escape, as the keyboard sends it to the focused view's window: a key
    /// equivalent first (a Cancel button's), then to the first responder,
    /// whose unhandled `cancelOperation:` reaches the window's delegate.
    fn escape(&self, id: NodeId) -> Result<(), ActionError> {
        let view = self.state.borrow().nodes.get(&id).ok_or(ActionError::UnknownNode)?.widget.key_view();
        let window = view.window().ok_or(ActionError::Unsupported)?;
        if view.acceptsFirstResponder() {
            window.makeFirstResponder(Some(&view));
        }
        let escape = ns("\u{1b}");
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags::empty(),
            0.0,
            window.windowNumber(),
            None,
            &escape,
            &escape,
            false,
            53,
        )
        .ok_or(ActionError::Unsupported)?;
        if !window.performKeyEquivalent(&event) {
            window.sendEvent(&event);
        }
        Ok(())
    }
}
