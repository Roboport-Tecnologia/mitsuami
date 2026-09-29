//! Synthesized input: keys, pointer, wheel and dragged files.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
use mitsuami_core::services::Shortcut;
use mitsuami_core::{NodeId, Point, SelectionMode, UiEvent, WidgetKind};
use windows_core::Interface;

use super::fields::inner_text_box;
use super::focus::is_control;
use super::new_window::request_close;
use super::scroll::scroll_now;
use super::{Node, Widget, WinUiBackend};
use crate::bindings as w;
use crate::runtime;

impl WinUiBackend {
    pub(super) fn synthesize_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        if let SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) = input {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let target = node.file_drop.as_ref().ok_or(ActionError::Unsupported)?;
            match input {
                SyntheticInput::DragFiles(paths) => target.drag(paths),
                SyntheticInput::DropFiles(paths) => target.drop_files(paths),
                _ => target.leave(),
            }
            return Ok(());
        }
        {
            // What XAML's events would report; a click focuses it.
            let state = self.state.borrow();
            if let Some(Widget::GpuSurface(surface)) = state.nodes.get(&id).map(|n| &n.widget) {
                // Keys with modifiers aren't simulated on a surface.
                if !surface.takes_input() || matches!(input, SyntheticInput::Shortcut(_)) {
                    return Err(ActionError::Unsupported);
                }
                if let SyntheticInput::Click(_) = input {
                    state.focus(id, w::FocusState::Pointer);
                }
                surface.synthesize(input);
                return Ok(());
            }
        }
        if let SyntheticInput::Click(point) = input {
            // Drawn widgets only: their pointer handling is ours.
            let state = self.state.borrow();
            return match state.nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Drawn { view, .. }) => {
                    view.click(*point);
                    Ok(())
                }
                Some(_) => Err(ActionError::Unsupported),
                None => Err(ActionError::UnknownNode),
            };
        }
        let (widget_kind, element, enabled) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let enabled = is_control(&node.widget)
                .then(|| node.element.cast::<w::IControl>().and_then(|c| c.IsEnabled()).unwrap_or(true));
            (node.kind, node.element.clone(), enabled)
        };
        {
            let state = self.state.borrow();
            if let Some(Widget::List(list)) = state.nodes.get(&id).map(|n| &n.widget) {
                return match input {
                    // A table's rows also scroll sideways.
                    SyntheticInput::Scroll { dx, dy } => {
                        let scroll = list.scroll_viewer().ok_or(ActionError::Unsupported)?;
                        let max = scroll.ScrollableHeight().unwrap_or(0.0).max(0.0);
                        let y = (scroll.VerticalOffset().unwrap_or(0.0) + *dy as f64).clamp(0.0, max);
                        let x = if list.is_table() {
                            let max = scroll.ScrollableWidth().unwrap_or(0.0).max(0.0);
                            (scroll.HorizontalOffset().unwrap_or(0.0) + *dx as f64).clamp(0.0, max)
                        } else {
                            0.0
                        };
                        list.scroll_to(Point::new(x as f32, y as f32)).map_err(|_| ActionError::Unsupported)
                    }
                    // The list view's own keys: those simulated here, and
                    // the rest (the page keys, with modifiers) aren't.
                    SyntheticInput::Key(key) if list.uses(Shortcut::new(*key)) => {
                        if list.mode() == SelectionMode::None {
                            return Err(ActionError::Unsupported);
                        }
                        _ = list.view.cast::<w::IUIElement>().and_then(|e| e.Focus(w::FocusState::Keyboard));
                        match list.key(*key) {
                            Ok(true) => Ok(()),
                            _ => Err(ActionError::Unsupported),
                        }
                    }
                    SyntheticInput::Shortcut(shortcut) if list.uses(*shortcut) => Err(ActionError::Unsupported),
                    SyntheticInput::Key(key) => {
                        drop(state);
                        self.send_key(id, Shortcut::new(*key))
                    }
                    SyntheticInput::Shortcut(shortcut) => {
                        drop(state);
                        self.send_key(id, *shortcut)
                    }
                    _ => Err(ActionError::Unsupported),
                };
            }
        }
        if let SyntheticInput::Scroll { dx, dy } = input {
            let scroll: w::IScrollViewer = element.cast().map_err(|_| ActionError::Unsupported)?;
            _ = element.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
            let clamp = |v: f64, max: f64| v.clamp(0.0, max.max(0.0));
            let x =
                clamp(scroll.HorizontalOffset().unwrap_or(0.0) + *dx as f64, scroll.ScrollableWidth().unwrap_or(0.0));
            let y =
                clamp(scroll.VerticalOffset().unwrap_or(0.0) + *dy as f64, scroll.ScrollableHeight().unwrap_or(0.0));
            let state = self.state.borrow();
            scroll_now(&state.emitter, id, &state.nodes[&id].offset, &scroll, Point::new(x as f32, y as f32))
                .map_err(|_| ActionError::Unsupported)?;
            return Ok(());
        }
        if enabled == Some(false) {
            return Err(ActionError::Disabled);
        }
        let key = match input {
            SyntheticInput::Key(key) => key,
            SyntheticInput::Shortcut(shortcut) => {
                // Text fields take keys with modifiers as editing
                // commands, which aren't simulated.
                if matches!(
                    widget_kind,
                    WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea
                ) || self.uses(id, *shortcut)
                {
                    return Err(ActionError::Unsupported);
                }
                return self.send_key(id, *shortcut);
            }
            _ => unreachable!(),
        };
        // A node's keys come first: its window's accelerator only sees
        // the keys nothing on the way up handled.
        if *key == Key::Escape && self.send_key(id, Shortcut::new(*key)).is_ok() {
            return Ok(());
        }
        if *key == Key::Escape {
            // What the accelerator does, in the node's window if it's a
            // dialog; a plain window ignores Escape. Keys are synthesized
            // by driving controls here, not by sending input.
            let hwnd = {
                let state = self.state.borrow();
                let mut current = Some(id);
                loop {
                    match current.and_then(|c| state.nodes.get(&c)) {
                        Some(Node { widget: Widget::Window(parts), .. }) => {
                            break parts.escape.is_some().then_some(parts.hwnd);
                        }
                        Some(node) => current = node.parent,
                        None => break None,
                    }
                }
            };
            if let Some(hwnd) = hwnd {
                request_close(hwnd);
                runtime::pump();
            }
            return Ok(());
        }
        match (widget_kind, key) {
            // A text area takes Return as a new line; Tab moves on from
            // all three, as XAML's text boxes take no tabs.
            (
                WidgetKind::TextInput | WidgetKind::TextArea | WidgetKind::SearchInput,
                Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab,
            ) => {
                // A search box is edited through the text box in its template.
                let field: w::ITextBox = match widget_kind {
                    WidgetKind::SearchInput => inner_text_box(&element).ok_or(ActionError::Unsupported)?,
                    _ => element.cast().map_err(|_| ActionError::Unsupported)?,
                };
                // It would take the keys and ignore them (our edits go
                // around that); nothing can be typed into it on any platform.
                if field.IsReadOnly().unwrap_or(false) {
                    return Err(ActionError::ReadOnly);
                }
                let ui: w::IUIElement = field.cast().map_err(|_| ActionError::Unsupported)?;
                if ui.FocusState().unwrap_or(w::FocusState::Unfocused) == w::FocusState::Unfocused {
                    self.state.borrow().focus(id, w::FocusState::Keyboard);
                    // Focusing may select everything; typing should append,
                    // as after clicking past the end of the text.
                    let end = field.Text().map_or(0, |t| t.encode_utf16().count() as i32);
                    _ = field.SetSelectionStart(end);
                    _ = field.SetSelectionLength(0);
                }
                // Edits go through the selection, like typing does, so
                // TextChanged reports them.
                let edit = |text: &str| -> windows_core::Result<()> {
                    let start = field.SelectionStart()?;
                    field.SetSelectedText(text)?;
                    field.SetSelectionStart(start + text.encode_utf16().count() as i32)?;
                    field.SetSelectionLength(0)
                };
                let result = match key {
                    Key::Char(c) => edit(&c.to_string()),
                    Key::Backspace => (|| {
                        if field.SelectionLength()? == 0 {
                            let start = field.SelectionStart()?;
                            if start == 0 {
                                return Ok(());
                            }
                            field.SetSelectionStart(start - 1)?;
                            field.SetSelectionLength(1)?;
                        }
                        edit("")
                    })(),
                    Key::Enter if widget_kind == WidgetKind::TextArea => edit("\r"),
                    // What its QuerySubmitted reports for a real Return.
                    Key::Enter if widget_kind == WidgetKind::SearchInput => field.Text().map(|text| {
                        self.state.borrow().emitter().emit(id, UiEvent::Search(text));
                    }),
                    Key::Enter => {
                        self.state.borrow().emitter().emit(id, UiEvent::Submit);
                        Ok(())
                    }
                    _ => {
                        self.tab_from(id);
                        Ok(())
                    }
                };
                self.state.borrow().report_value(id);
                result.map_err(|_| ActionError::Unsupported)
            }
            (WidgetKind::PasswordInput, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                let field: w::IPasswordBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                let ui: w::IUIElement = element.cast().map_err(|_| ActionError::Unsupported)?;
                let all = &self.state.borrow().nodes[&id].password_all;
                if ui.FocusState().unwrap_or(w::FocusState::Unfocused) == w::FocusState::Unfocused {
                    self.state.borrow().focus(id, w::FocusState::Keyboard);
                    all.set(false);
                }
                // Password boxes have no caret or selection to edit through:
                // edits go to the end, where typing into a focused box puts
                // them, or replace all of it if it's selected, and
                // PasswordChanged reports them.
                let edit = |edit: &dyn Fn(&mut String)| -> windows_core::Result<()> {
                    let mut text = if all.replace(false) { String::new() } else { field.Password()? };
                    edit(&mut text);
                    field.SetPassword(&text)
                };
                let result = match key {
                    Key::Char(c) => edit(&|t| t.push(*c)),
                    Key::Backspace => edit(&|t| {
                        t.pop();
                    }),
                    Key::Enter => {
                        self.state.borrow().emitter().emit(id, UiEvent::Submit);
                        Ok(())
                    }
                    _ => {
                        self.tab_from(id);
                        Ok(())
                    }
                };
                self.state.borrow().report_value(id);
                result.map_err(|_| ActionError::Unsupported)
            }
            (WidgetKind::Button, Key::Enter | Key::Char(' '))
            | (WidgetKind::ToggleButton | WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => {
                self.perform(id, &A11yAction::Activate)
            }
            // Keys the control uses that aren't simulated.
            _ if self.uses(id, Shortcut::new(*key)) => Err(ActionError::Unsupported),
            _ => self.send_key(id, Shortcut::new(*key)),
        }
    }

    /// A key the control doesn't use, as XAML routes it: `KeyDown` goes up
    /// from the focused control, and the nearest node that takes the key
    /// handles it (`keys.rs`). The control is focused first, if it takes
    /// focus. `Unsupported` if nothing takes it.
    fn send_key(&self, id: NodeId, shortcut: Shortcut) -> Result<(), ActionError> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
        if is_control(&node.widget) {
            state.focus(id, w::FocusState::Keyboard);
        }
        let mut current = Some(id);
        while let Some(node) = current.and_then(|c| state.nodes.get(&c)) {
            if node.keys.as_ref().is_some_and(|keys| keys.take(shortcut)) {
                return Ok(());
            }
            current = node.parent;
        }
        Err(ActionError::Unsupported)
    }

    /// Whether the control handles a key itself on `KeyDown`, so it never
    /// goes up to a node's keys: as each XAML control does. Tab is the
    /// window's (its `PreviewKeyDown` moves focus). Custom renders and
    /// native views could use any key.
    fn uses(&self, id: NodeId, shortcut: Shortcut) -> bool {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return false };
        let Shortcut { key, primary, alt, .. } = shortcut;
        let plain = !primary && !alt;
        let arrow = matches!(key, Key::Up | Key::Down | Key::Left | Key::Right);
        let paging = matches!(key, Key::Home | Key::End | Key::PageUp | Key::PageDown);
        key == Key::Tab
            || match &node.widget {
                // Text boxes: typing, and moving and selecting in the text
                // (with Control by words, and its own editing commands).
                Widget::Field(_) | Widget::Password(_) => {
                    !alt && matches!(key, Key::Char(_) | Key::Backspace | Key::Delete | Key::Left | Key::Right)
                        || matches!(key, Key::Home | Key::End)
                }
                // Up and Down go through its suggestions.
                Widget::Search(_) => {
                    !alt && (matches!(key, Key::Char(_) | Key::Backspace | Key::Delete) || arrow)
                        || matches!(key, Key::Home | Key::End)
                }
                // A text area also takes Return and moves by lines; a
                // number box steps with its arrows and page keys.
                Widget::TextArea { .. } | Widget::Number { .. } => {
                    !alt && matches!(key, Key::Char(_) | Key::Backspace | Key::Delete | Key::Enter) || arrow || paging
                }
                Widget::Button(_) | Widget::Toggle(_) | Widget::Checkbox(_) | Widget::MenuButton(_) => {
                    plain && matches!(key, Key::Enter | Key::Char(' '))
                }
                Widget::Switch(_) => plain && key == Key::Char(' '),
                // Typing picks an option; Alt+Up and Alt+Down open it.
                Widget::Select(_) => {
                    !primary && (arrow || paging) || plain && matches!(key, Key::Char(_) | Key::Enter | Key::F(4))
                }
                Widget::RadioGroup(_) => plain && (arrow || key == Key::Char(' ')),
                Widget::Slider { .. } | Widget::Scroll(_) => plain && (arrow || paging),
                Widget::Sidebar(_) | Widget::Tabs(_) => {
                    plain && (arrow || matches!(key, Key::Home | Key::End | Key::Enter | Key::Char(' ')))
                }
                Widget::List(list) => list.uses(shortcut),
                Widget::Custom { .. } | Widget::Native { .. } => true,
                _ => false,
            }
    }
}
