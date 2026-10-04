//! The update pipeline: dispatching events, committing, and the run-loop
//! turn.

use crate::a11y::{A11yAction, ActionError};

use crate::command::{Command, EventValue, UiEvent};
use crate::widget::{NodeId, Prop, WidgetKind};

use super::{Fit, Inner, Ui};

impl Ui {
    /// Drains native events and dispatches them to handlers. Handlers run in
    /// one reactive batch per event.
    pub fn process_events(&self) {
        crate::task::with_current(self, || self.dispatch_events());
    }

    fn dispatch_events(&self) {
        loop {
            let chosen = {
                let mut inner = self.inner.borrow_mut();
                let id = inner.menu_queue.pop_front();
                id.and_then(|id| inner.menu_handlers.values().find_map(|handlers| handlers.get(&id)).cloned())
            };
            if let Some(handler) = chosen {
                mitsuami_reactive::batch(|| handler());
                continue;
            }
            let next = self.inner.borrow().events.pop();
            let Some((id, event)) = next else { break };
            let (handlers, meaning) = {
                let mut inner = self.inner.borrow_mut();
                inner.absorb(id, &event);
                let node = inner.nodes.get(&id);
                // A drawn widget decides what a pointer event means.
                let meaning = match (&event, node.and_then(|n| n.drawn().map(|c| (c, n.frame.size)))) {
                    (UiEvent::Pointer(pointer), Some((custom, size))) => custom.pointer(size, pointer),
                    _ => None,
                };
                (node.map(|n| n.handlers.clone()).unwrap_or_default(), meaning)
            };
            self.changed();
            for event in std::iter::once(event).chain(meaning.map(UiEvent::Custom)) {
                mitsuami_reactive::batch(|| {
                    for handler in &handlers {
                        handler(&event);
                    }
                });
            }
        }
    }

    /// Brings the native tree up to date: styles, structure, props, layout.
    pub fn commit(&self) {
        let language = self.l10n.language();
        let mut inner = self.inner.borrow_mut();
        inner.commit_scheduled = false;
        if inner.language.as_ref() != Some(&language) {
            let rtl = crate::l10n::is_rtl(&language);
            inner.backend.set_locale(&language, rtl);
            inner.language = Some(language);
            if inner.rtl != rtl {
                inner.rtl = rtl;
                inner.styles_dirty = true;
            }
        }
        let refocus = std::mem::take(&mut inner.focus_dirty) || inner.styles_dirty || !inner.resync.is_empty();
        if inner.styles_dirty {
            inner.resolve_styles();
        }
        inner.resync_all();
        if refocus {
            inner.sync_focus_orders();
        }
        for id in std::mem::take(&mut inner.pending_focus) {
            if inner.nodes.contains_key(&id) {
                inner.pending.push(Command::Focus { id });
            }
        }
        for (id, range) in std::mem::take(&mut inner.pending_selections) {
            let Some(node) = inner.nodes.get(&id) else { continue };
            // Graphemes of the text the field has now, in its `char`s.
            let text = crate::find_prop!(node.props, Value).unwrap_or_default();
            let range = crate::graphemes::chars_of(&text, range);
            inner.pending.push(Command::SelectText { id, range });
        }
        let batch = std::mem::take(&mut inner.pending);
        inner.created.clear();
        if !batch.is_empty() {
            inner.backend.apply(&batch);
        }
        inner.layout();
        let frames = std::mem::take(&mut inner.pending);
        if !frames.is_empty() {
            inner.backend.apply(&frames);
        }
    }

    /// One run-loop turn: dispatch events and commit, repeating while the
    /// commit itself produced new events (e.g. a window resize) or new sizes
    /// views watch, until idle. Backends call this from their run loop,
    /// before it goes to sleep.
    pub fn tick(&self) {
        const MAX_TURNS: usize = 64;
        // Views that change with their size can change it again. Past this,
        // sizes wait for the next turn, as browsers' `ResizeObserver` does,
        // so a view that never settles doesn't hold the run loop.
        const MAX_SIZE_REPORTS: usize = 8;
        let mut reports = 0;
        for turn in 0..MAX_TURNS {
            // Tasks other threads woke run on the first turn only; one they
            // wake again meanwhile waits for the next tick (`task.rs`).
            self.executor.run_ready(self, turn == 0);
            self.process_events();
            self.commit();
            // Views change with their sizes before anything is shown.
            if reports < MAX_SIZE_REPORTS && self.report_sizes() {
                reports += 1;
                continue;
            }
            let idle = {
                let inner = self.inner.borrow();
                inner.events.is_empty() && inner.menu_queue.is_empty()
            };
            if idle && !self.executor.has_ready() {
                return;
            }
        }
    }

    /// Asks the backend to perform an accessibility action, then dispatches
    /// the events it produced.
    ///
    /// Custom widgets the backend can't act on get the event their shared
    /// definition gives the action (e.g. `Increment` → a new value).
    pub fn perform(&self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        let result = {
            let mut inner = self.inner.borrow_mut();
            let result = inner.backend.perform(id, action);
            match (result, inner.nodes.get(&id).and_then(|n| n.custom())) {
                (Err(ActionError::Unsupported), Some(custom)) => match custom.action(action) {
                    Some(event) => {
                        inner.events.emit(id, UiEvent::Custom(event));
                        Ok(())
                    }
                    None => Err(ActionError::Unsupported),
                },
                (result, _) => result,
            }
        };
        self.process_events();
        result
    }

    /// Synthesizes raw input on a native control, then dispatches the events
    /// it produced.
    pub fn synthesize(&self, id: NodeId, input: &crate::backend::SyntheticInput) -> Result<(), ActionError> {
        let result = self.inner.borrow_mut().backend.synthesize(id, input);
        self.process_events();
        result
    }
}

impl Inner {
    /// Updates core state from events the native side already reflects, so
    /// echoing the value back is a no-op.
    fn absorb(&mut self, id: NodeId, event: &UiEvent) {
        match event {
            UiEvent::Changed(value) => {
                let Some(node) = self.nodes.get_mut(&id) else { return };
                let prop = match (node.kind, value) {
                    (
                        WidgetKind::TextInput
                        | WidgetKind::PasswordInput
                        | WidgetKind::SearchInput
                        | WidgetKind::TextArea,
                        EventValue::Text(text),
                    ) => Prop::Value(text.clone()),
                    (WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::ToggleButton, EventValue::Bool(b)) => {
                        Prop::Checked(*b)
                    }
                    (WidgetKind::List | WidgetKind::Table, EventValue::Rows(rows)) => Prop::Selected(rows.clone()),
                    (WidgetKind::Table, EventValue::Sort(sort)) => Prop::Sort(Some(*sort)),
                    (
                        WidgetKind::Select | WidgetKind::RadioGroup | WidgetKind::Sidebar | WidgetKind::Tabs,
                        EventValue::Index(index),
                    ) => Prop::SelectedIndex(Some(*index)),
                    (WidgetKind::Slider | WidgetKind::NumberInput, EventValue::Number(number)) => Prop::Number(*number),
                    _ => return,
                };
                // A click takes a checkbox out of the mixed state, on every
                // platform.
                if node.kind == WidgetKind::Checkbox && crate::find_prop!(node.props, Mixed) == Some(true) {
                    node.props.retain(|p| !matches!(p, Prop::Mixed(_)));
                    node.props.push(Prop::Mixed(false));
                }
                // A tab view's page shown is in the Tab order.
                self.focus_dirty |= matches!(prop, Prop::SelectedIndex(_));
                node.props.retain(|p| p.key() != prop.key());
                node.props.push(prop);
            }
            UiEvent::PointerLockEnded
            | UiEvent::KeyboardGrabEnded
            | UiEvent::FullScreenChanged(_)
            | UiEvent::MaximizedChanged(_)
            | UiEvent::SidebarShownChanged(_) => {
                let Some(node) = self.nodes.get_mut(&id) else { return };
                let prop = match event {
                    UiEvent::PointerLockEnded => Prop::PointerLock(false),
                    UiEvent::KeyboardGrabEnded => Prop::KeyboardGrab(false),
                    UiEvent::FullScreenChanged(on) => Prop::FullScreen(*on),
                    UiEvent::MaximizedChanged(on) => Prop::Maximized(*on),
                    UiEvent::SidebarShownChanged(shown) => Prop::SidebarShown(*shown),
                    _ => unreachable!(),
                };
                node.props.retain(|p| p.key() != prop.key());
                node.props.push(prop);
            }
            UiEvent::FocusIn => {
                if let Some(window) = self.window_of(id) {
                    self.focused.insert(window, id);
                }
            }
            UiEvent::FocusOut => self.focused.retain(|_, focused| *focused != id),
            UiEvent::Scrolled(offset) => {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.scroll_offset = *offset;
                }
            }
            UiEvent::RowWidth(width) => {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.row_width = Some(*width);
                }
            }
            UiEvent::ViewportInsets(insets) => {
                if let Some(node) = self.nodes.get_mut(&id)
                    && node.viewport_insets != *insets
                {
                    node.viewport_insets = *insets;
                    // The style's padding takes them.
                    self.styles_dirty = true;
                }
            }
            UiEvent::ColumnWidths(widths) => {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.column_widths = Some(widths.clone());
                }
            }
            UiEvent::WindowResized(size) => {
                if let Some(node) = self.nodes.get_mut(&id) {
                    // A following window's height is one the core asked for
                    // (grown to the minimum, as platforms grow it), or the
                    // user's, which ends following if it's to end.
                    if let Fit::Follow { until_resized } = node.fit
                        && !node.in_full_screen()
                        && !node.heights.is_empty()
                    {
                        let min = crate::find_prop!(node.props, MinSize).map_or(0.0, |m| m.height);
                        match node.heights.iter().rposition(|h| (h.max(min) - size.height).abs() < 1.0) {
                            Some(reported) => drop(node.heights.drain(..reported)),
                            None if until_resized => {
                                node.fit = Fit::None;
                                node.heights.clear();
                            }
                            None => {}
                        }
                    }
                    node.window_size = *size;
                    self.styles_dirty = true;
                }
            }
            UiEvent::Remeasure => {
                if let Some(t) = self.nodes.get(&id).and_then(|n| n.taffy) {
                    let _ = self.taffy.mark_dirty(t);
                }
            }
            UiEvent::MetricsChanged => {
                self.metrics = self.backend.metrics();
                self.styles_dirty = true;
                for node in self.nodes.values_mut() {
                    node.drawn_at = None;
                    if let Some(t) = node.taffy {
                        let _ = self.taffy.mark_dirty(t);
                    }
                }
            }
            _ => {}
        }
    }
}
