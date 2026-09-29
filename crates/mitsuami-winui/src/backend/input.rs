//! Synthesized input: keys, pointer, wheel and dragged files.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
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
                if !surface.takes_input() {
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
                    SyntheticInput::Scroll { dy, .. } => {
                        let scroll = list.scroll_viewer().ok_or(ActionError::Unsupported)?;
                        let max = scroll.ScrollableHeight().unwrap_or(0.0).max(0.0);
                        let y = (scroll.VerticalOffset().unwrap_or(0.0) + *dy as f64).clamp(0.0, max);
                        list.scroll_to(Point::new(0.0, y as f32)).map_err(|_| ActionError::Unsupported)
                    }
                    SyntheticInput::Key(key) if list.mode() != SelectionMode::None => {
                        _ = list.view.cast::<w::IUIElement>().and_then(|e| e.Focus(w::FocusState::Keyboard));
                        match list.key(*key) {
                            Ok(true) => Ok(()),
                            _ => Err(ActionError::Unsupported),
                        }
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
        let SyntheticInput::Key(key) = input else { unreachable!() };
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
                if ui.FocusState().unwrap_or(w::FocusState::Unfocused) == w::FocusState::Unfocused {
                    self.state.borrow().focus(id, w::FocusState::Keyboard);
                }
                // Password boxes have no caret or selection to edit through:
                // edits go to the end, where typing into a focused box puts
                // them, and PasswordChanged reports them.
                let edit = |edit: &dyn Fn(&mut String)| -> windows_core::Result<()> {
                    let mut text = field.Password()?;
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
            _ => Err(ActionError::Unsupported),
        }
    }
}
