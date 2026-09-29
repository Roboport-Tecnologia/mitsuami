//! Synthetic input: keys, clicks, scrolls and dragged files, and GPU surfaces' input.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
use mitsuami_core::{
    EventValue, KeyCode, Modifiers, MouseButton, NodeId, Point, PointerEvent, PointerKind, Prop, RowKey, ScrollDelta,
    SelectionMode, Size, SurfaceInput, UiEvent, WidgetKind, find_prop,
};

use super::{HeadlessBackend, State};

impl HeadlessBackend {
    pub(super) fn synthesize_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        let mut state = self.state.borrow_mut();
        let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
        if find_prop!(node.props, Enabled) == Some(false) {
            return Err(ActionError::Disabled);
        }
        let kind = node.kind;
        if kind == WidgetKind::GpuSurface {
            return state.surface_input(id, input);
        }
        // As a platform does: the files it takes show it'll copy them, and
        // a drop gives it those.
        if let SyntheticInput::DragFiles(paths) | SyntheticInput::DropFiles(paths) = input {
            let drop = find_prop!(node.props, FileDrop).flatten().ok_or(ActionError::Unsupported)?;
            let accepted = drop.accepted(paths);
            if !accepted.is_empty() && state.drop_hover != Some(id) {
                state.drop_hover = Some(id);
                state.emit(id, UiEvent::DropHover(true));
            }
            if matches!(input, SyntheticInput::DropFiles(_)) {
                if !accepted.is_empty() {
                    state.emit(id, UiEvent::FilesDropped(accepted));
                }
                if state.drop_hover.take_if(|n| *n == id).is_some() {
                    state.emit(id, UiEvent::DropHover(false));
                }
            }
            return Ok(());
        }
        if let SyntheticInput::DragLeave = input {
            if state.drop_hover.take_if(|n| *n == id).is_some() {
                state.emit(id, UiEvent::DropHover(false));
            }
            return Ok(());
        }
        let key = match input {
            SyntheticInput::Key(key) => key,
            SyntheticInput::Click(position) => {
                // Only drawn widgets handle pointers themselves.
                let drawn = find_prop!(node.props, Custom).is_some_and(|c| c.is_drawn());
                if !drawn {
                    return Err(ActionError::Unsupported);
                }
                for kind in [PointerKind::Down, PointerKind::Up] {
                    state.emit(id, UiEvent::Pointer(PointerEvent { kind, position: *position }));
                }
                return Ok(());
            }
            SyntheticInput::Scroll { dx, dy } => {
                if !kind.scrolls() {
                    return Err(ActionError::Unsupported);
                }
                let node = &state.nodes[&id];
                let (axes, content) = match kind {
                    WidgetKind::List => (
                        mitsuami_core::ScrollAxes::Vertical,
                        Size::new(node.frame.width(), state.rows(id).iter().map(|r| r.height).sum()),
                    ),
                    _ => (
                        find_prop!(node.props, ScrollAxes).unwrap_or_default(),
                        node.children.first().map_or(Size::ZERO, |c| state.nodes[c].frame.size),
                    ),
                };
                let viewport = node.frame.size;
                let clamp = |v: f32, content: f32, viewport: f32, on: bool| {
                    if on { v.clamp(0.0, (content - viewport).max(0.0)) } else { 0.0 }
                };
                let offset = Point::new(
                    clamp(node.scroll_offset.x + dx, content.width, viewport.width, axes.horizontal()),
                    clamp(node.scroll_offset.y + dy, content.height, viewport.height, axes.vertical()),
                );
                state.scroll(id, offset);
                return Ok(());
            }
            // Handled above.
            SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) => unreachable!(),
        };
        // Nothing can be typed into a read-only field (AppKit's can't even
        // take focus from the keyboard).
        if matches!(kind, WidgetKind::TextInput | WidgetKind::TextArea)
            && find_prop!(node.props, ReadOnly) == Some(true)
        {
            return Err(ActionError::ReadOnly);
        }
        match (kind, key) {
            (WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput, Key::Char(_) | Key::Backspace)
            // Return starts a new line, and Tab inserts a tab, as AppKit's,
            // GTK's and Qt's text areas take it.
            | (WidgetKind::TextArea, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                state.focus(id);
                let mut text = find_prop!(state.nodes[&id].props, Value).unwrap_or_default();
                match key {
                    Key::Char(c) => text.push(*c),
                    Key::Enter => text.push('\n'),
                    Key::Tab => text.push('\t'),
                    _ => {
                        text.pop();
                    }
                }
                state.set_prop(id, Prop::Value(text.clone()));
                state.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                // Searches as it's typed, with no pause, as WinUI does.
                if kind == WidgetKind::SearchInput {
                    state.emit(id, UiEvent::Search(text));
                }
            }
            (WidgetKind::TextInput | WidgetKind::PasswordInput, Key::Enter) => state.emit(id, UiEvent::Submit),
            (WidgetKind::SearchInput, Key::Enter) => {
                let text = find_prop!(state.nodes[&id].props, Value).unwrap_or_default();
                state.emit(id, UiEvent::Search(text));
            }
            // Moves focus on; the field keeps its text and does not submit.
            (WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput, Key::Tab) => {
                if let Some(next) = state.next_focusable(id) {
                    state.focus(next);
                }
            }
            (WidgetKind::Button, Key::Enter | Key::Char(' ')) => state.emit(id, UiEvent::Click),
            (WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::ToggleButton, Key::Char(' ')) => {
                drop(state);
                return self.perform(id, &A11yAction::Activate);
            }
            // Arrows, Home and End move the selection (from the first
            // selected row) and show it; Enter activates it.
            (WidgetKind::List, Key::Up | Key::Down | Key::Home | Key::End | Key::Enter) => {
                if find_prop!(state.nodes[&id].props, SelectionMode).unwrap_or_default() == SelectionMode::None {
                    return Err(ActionError::Unsupported);
                }
                state.focus(id);
                let rows: Vec<RowKey> = state.rows(id).iter().map(|r| r.key).collect();
                let selected = find_prop!(state.nodes[&id].props, Selected).unwrap_or_default();
                let current = selected.first().and_then(|k| rows.iter().position(|r| r == k));
                if *key == Key::Enter {
                    if let Some(row) = selected.first() {
                        state.emit(id, UiEvent::RowActivated(*row));
                    }
                    return Ok(());
                }
                let last = rows.len().checked_sub(1);
                let next = match (key, current) {
                    (Key::Home, _) | (Key::Down, None) => rows.first().map(|_| 0),
                    (Key::End, _) | (Key::Up, None) => last,
                    (Key::Up, Some(i)) => Some(i.saturating_sub(1)),
                    (_, Some(i)) => Some((i + 1).min(last.unwrap_or(0))),
                    (_, None) => None,
                };
                if let Some(row) = next.map(|i| rows[i]) {
                    state.select(id, vec![row]);
                    state.reveal(id, row);
                }
            }
            // A dialog (a modal window) asks to close, as every platform's
            // do; a plain window ignores it.
            (_, Key::Escape) => {
                let mut window = Some(id);
                while let Some(w) = window.filter(|w| state.nodes[w].kind != WidgetKind::Window) {
                    window = state.nodes[&w].parent;
                }
                if let Some(window) = window
                    && state.nodes[&window].props.iter().any(|p| matches!(p, Prop::Modal { .. }))
                {
                    state.emit(window, UiEvent::WindowCloseRequested);
                }
            }
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }
}

impl State {
    /// Input on a GPU surface that takes it: a click focuses it and
    /// reports the primary button, keys (Tab too) go down and up, scrolls
    /// are in points.
    pub(super) fn surface_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        if find_prop!(self.nodes[&id].props, TakesInput) != Some(true) {
            return Err(ActionError::Unsupported);
        }
        let modifiers = Modifiers::default();
        match input {
            SyntheticInput::Click(position) => {
                self.focus(id);
                for pressed in [true, false] {
                    let button = MouseButton::Primary;
                    let position = *position;
                    self.emit(id, UiEvent::SurfaceInput(SurfaceInput::Button { button, pressed, position, modifiers }));
                }
            }
            SyntheticInput::Key(key) => {
                let code = match key {
                    Key::Char(c) => KeyCode::from_us_char(*c),
                    Key::Enter => KeyCode::Enter,
                    Key::Escape => KeyCode::Escape,
                    Key::Tab => KeyCode::Tab,
                    Key::Backspace => KeyCode::Backspace,
                    Key::Up => KeyCode::ArrowUp,
                    Key::Down => KeyCode::ArrowDown,
                    Key::Home => KeyCode::Home,
                    Key::End => KeyCode::End,
                };
                if self.focused != Some(id) {
                    return Err(ActionError::Unsupported);
                }
                for pressed in [true, false] {
                    let key = SurfaceInput::Key { code, native: 0, pressed, repeat: false, modifiers };
                    self.emit(id, UiEvent::SurfaceInput(key));
                }
            }
            SyntheticInput::Scroll { dx, dy } => {
                let delta = ScrollDelta::Points { x: *dx, y: *dy };
                self.emit(id, UiEvent::SurfaceInput(SurfaceInput::Scroll { delta, modifiers }));
            }
            SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) => {
                return Err(ActionError::Unsupported);
            }
        }
        Ok(())
    }
}
