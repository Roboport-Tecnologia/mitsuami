//! Synthetic input: keys, clicks, scrolls and dragged files, and GPU surfaces' input.

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{Backend, Key, SyntheticInput};
use mitsuami_core::services::Shortcut;
use mitsuami_core::{
    EventValue, KeyCode, Modifiers, MouseButton, NodeId, Point, PointerEvent, PointerKind, Prop, RowKey, ScrollDelta,
    SelectionMode, Size, SurfaceInput, UiEvent, WidgetKind, find_prop,
};

use super::{HeadlessBackend, State};

impl HeadlessBackend {
    pub(super) fn synthesize_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        let mut state = self.state.borrow_mut();
        let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
        if let SyntheticInput::PointerEnter | SyntheticInput::PointerLeave = input {
            if find_prop!(node.props, Hover) != Some(true) {
                return Err(ActionError::Unsupported);
            }
            state.emit(id, UiEvent::Hover(*input == SyntheticInput::PointerEnter));
            return Ok(());
        }
        if *input == SyntheticInput::DoubleClick {
            if find_prop!(node.props, DoubleClick) != Some(true) {
                return Err(ActionError::Unsupported);
            }
            state.emit(id, UiEvent::DoubleClick);
            return Ok(());
        }
        if *input == SyntheticInput::SingleClick {
            if find_prop!(node.props, Click) != Some(true) {
                return Err(ActionError::Unsupported);
            }
            state.emit(id, UiEvent::Click);
            return Ok(());
        }
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
        // As a platform's list does: rows dropped where `RowMoves` takes
        // them, the rows in the list's order.
        if let SyntheticInput::MoveRows { rows, to } = input {
            let moves = find_prop!(node.props, RowMoves).ok_or(ActionError::Unsupported)?;
            let order = find_prop!(node.props, Rows).unwrap_or_default();
            let rows: Vec<RowKey> = order.iter().filter(|row| rows.contains(row)).copied().collect();
            if let Some(to) = moves.target(&rows, *to)
                && !rows.is_empty()
            {
                state.emit(id, UiEvent::RowsMoved { rows, to });
            }
            return Ok(());
        }
        if let SyntheticInput::DragLeave = input {
            if state.drop_hover.take_if(|n| *n == id).is_some() {
                state.emit(id, UiEvent::DropHover(false));
            }
            return Ok(());
        }
        // A text field's own keys come before its own use of them.
        if matches!(kind, WidgetKind::TextInput | WidgetKind::SearchInput) {
            let shortcut = match input {
                SyntheticInput::Key(key) => Some(Shortcut::new(*key)),
                SyntheticInput::Shortcut(shortcut) => Some(*shortcut),
                _ => None,
            };
            if let Some(shortcut) = shortcut
                && find_prop!(node.props, Keys).is_some_and(|keys| keys.contains(&shortcut))
            {
                state.focus(id);
                state.emit(id, UiEvent::Key(shortcut));
                return Ok(());
            }
        }
        let key = match input {
            SyntheticInput::Key(key) => key,
            // Text fields take keys with modifiers as editing commands,
            // which aren't simulated; everywhere else they go up to a node
            // that takes them.
            SyntheticInput::Shortcut(shortcut) => {
                if matches!(
                    kind,
                    WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea
                ) {
                    return Err(ActionError::Unsupported);
                }
                return state.bubble_key(id, *shortcut);
            }
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
            // A text area scrolls its text, up and down.
            SyntheticInput::Scroll { dy, .. } if kind == WidgetKind::TextArea => {
                let end = state.text_area_end(id);
                let node = state.nodes.get_mut(&id).unwrap();
                node.scroll_offset.y = (node.scroll_offset.y + dy).clamp(0.0, end);
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
                    // Below its header, and as wide as its columns.
                    WidgetKind::Table => (
                        mitsuami_core::ScrollAxes::Both,
                        Size::new(
                            node.column_widths.iter().map(|w| w + super::metrics::COLUMN_SPACING).sum(),
                            state.rows(id).iter().map(|r| r.height).sum::<f32>() + super::metrics::TABLE_HEADER_HEIGHT,
                        ),
                    ),
                    _ => (
                        find_prop!(node.props, ScrollAxes).unwrap_or_default(),
                        node.children.first().map_or(Size::ZERO, |c| state.nodes[c].frame.size),
                    ),
                };
                let viewport = node.frame.inset(node.viewport_insets).size;
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
            // Only a surface takes an input method's text.
            SyntheticInput::Text(_) | SyntheticInput::Compose(_) => return Err(ActionError::Unsupported),
            SyntheticInput::DragFiles(_)
            | SyntheticInput::DragLeave
            | SyntheticInput::DropFiles(_)
            | SyntheticInput::MoveRows { .. }
            | SyntheticInput::PointerEnter
            | SyntheticInput::PointerLeave
            | SyntheticInput::DoubleClick
            | SyntheticInput::SingleClick => unreachable!(),
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
                // Typing replaces the selection, or goes in at the caret.
                let mut chars: Vec<char> = find_prop!(state.nodes[&id].props, Value).unwrap_or_default().chars().collect();
                let selection = state.selection.clone().unwrap_or(chars.len()..chars.len());
                let caret = match key {
                    Key::Backspace if selection.is_empty() => {
                        let start = selection.start.saturating_sub(1);
                        chars.drain(start..selection.end);
                        start
                    }
                    Key::Backspace => {
                        chars.drain(selection.clone());
                        selection.start
                    }
                    key => {
                        let typed = match key {
                            Key::Char(c) => *c,
                            Key::Enter => '\n',
                            _ => '\t',
                        };
                        chars.splice(selection.clone(), [typed]);
                        selection.start + 1
                    }
                };
                let text: String = chars.into_iter().collect();
                // At the end, the caret is where focusing puts it.
                state.selection = (caret < text.chars().count()).then_some(caret..caret);
                state.set_prop(id, Prop::Value(text.clone()));
                state.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                // Searches as it's typed, with no pause, as WinUI does,
                // unless only on Return; clearing always searches.
                let on_return = find_prop!(state.nodes[&id].props, SearchOnReturn) == Some(true);
                if kind == WidgetKind::SearchInput && (!on_return || text.is_empty()) {
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
            (WidgetKind::Button | WidgetKind::SplitButton, Key::Enter | Key::Char(' ')) => state.emit(id, UiEvent::Click),
            (WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::ToggleButton, Key::Char(' ')) => {
                drop(state);
                return self.perform(id, &A11yAction::Activate);
            }
            // Arrows, Home and End move the selection (from the first
            // selected row) and show it; Enter activates it.
            (WidgetKind::List | WidgetKind::Table, Key::Up | Key::Down | Key::Home | Key::End | Key::Enter) => {
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
            // A node round the control that takes Escape gets it first, as
            // any key the control doesn't use; then a dialog (a modal
            // window) asks to close, as every platform's do, and a plain
            // window ignores it.
            (_, Key::Escape) => {
                if state.bubble_key(id, Shortcut::new(Key::Escape)).is_ok() {
                    return Ok(());
                }
                // A popover around the control is dismissed, as every
                // platform's is.
                let mut node = Some(id);
                while let Some(n) = node {
                    let popover = &state.nodes[&n];
                    if popover.kind == WidgetKind::Popover
                        && popover.props.iter().any(|p| matches!(p, Prop::PopoverShown(true)))
                    {
                        state.dismiss_popover(n);
                        return Ok(());
                    }
                    node = popover.parent;
                }
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
            // A key the control doesn't use goes up to a node that takes it.
            _ => return state.bubble_key(id, Shortcut::new(*key)),
        }
        Ok(())
    }
}

impl State {
    /// A key the focused control `id` doesn't use, as platforms send it
    /// on: up the tree to the nearest node that takes it (`Prop::Keys`).
    /// Nothing takes it: `Unsupported`, where a platform would beep.
    fn bubble_key(&mut self, id: NodeId, shortcut: Shortcut) -> Result<(), ActionError> {
        if self.focus_orders.values().any(|order| order.contains(&id)) {
            self.focus(id);
        }
        let mut node = Some(id);
        while let Some(at) = node {
            if find_prop!(self.nodes[&at].props, Keys).is_some_and(|keys| keys.contains(&shortcut)) {
                self.emit(at, UiEvent::Key(shortcut));
                return Ok(());
            }
            node = self.nodes[&at].parent;
        }
        Err(ActionError::Unsupported)
    }

    /// Input on a GPU surface that takes it: a click focuses it and
    /// reports the primary button, keys (Tab too) go down and up, typing
    /// what a US keyboard types when it takes text, scrolls are in points.
    pub(super) fn surface_input(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        if find_prop!(self.nodes[&id].props, TakesInput) != Some(true) {
            return Err(ActionError::Unsupported);
        }
        let takes_text = find_prop!(self.nodes[&id].props, TextInput) == Some(true);
        // Input methods are on while it takes text, unless the app turned them off.
        let composes = takes_text && find_prop!(self.nodes[&id].props, InputMethod) != Some(false);
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
                let code = KeyCode::from_key(*key);
                if self.focused != Some(id) {
                    return Err(ActionError::Unsupported);
                }
                let unmodified = code.us_char();
                for pressed in [true, false] {
                    // The character asked for, as its key types it.
                    let text = match key {
                        Key::Char(c) if pressed && takes_text && !c.is_control() => Some(c.to_string()),
                        _ => None,
                    };
                    let key =
                        SurfaceInput::Key { code, native: 0, pressed, repeat: false, modifiers, text, unmodified };
                    self.emit(id, UiEvent::SurfaceInput(key));
                }
            }
            SyntheticInput::Text(text) => {
                if !composes || self.focused != Some(id) {
                    return Err(ActionError::Unsupported);
                }
                // A composition going on ends as it commits.
                if std::mem::take(&mut self.composing) {
                    let preedit = SurfaceInput::Preedit { text: String::new(), cursor: None };
                    self.emit(id, UiEvent::SurfaceInput(preedit));
                }
                self.emit(id, UiEvent::SurfaceInput(SurfaceInput::Text(text.clone())));
            }
            SyntheticInput::Compose(text) => {
                if !composes || self.focused != Some(id) {
                    return Err(ActionError::Unsupported);
                }
                self.composing = !text.is_empty();
                let cursor = self.composing.then_some(text.len());
                let preedit = SurfaceInput::Preedit { text: text.clone(), cursor };
                self.emit(id, UiEvent::SurfaceInput(preedit));
            }
            SyntheticInput::Scroll { dx, dy } => {
                let delta = ScrollDelta::Points { x: *dx, y: *dy };
                self.emit(id, UiEvent::SurfaceInput(SurfaceInput::Scroll { delta, modifiers }));
            }
            // Keys with modifiers aren't simulated on a surface.
            SyntheticInput::Shortcut(_)
            | SyntheticInput::DragFiles(_)
            | SyntheticInput::DragLeave
            | SyntheticInput::DropFiles(_)
            | SyntheticInput::MoveRows { .. }
            | SyntheticInput::PointerEnter
            | SyntheticInput::PointerLeave
            | SyntheticInput::DoubleClick
            | SyntheticInput::SingleClick => {
                return Err(ActionError::Unsupported);
            }
        }
        Ok(())
    }
}
