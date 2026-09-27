//! An in-memory backend for integration tests.
//!
//! It keeps a mirror of the native tree like a real backend would, validates
//! every command against the protocol (panicking on violations), and measures
//! text with fixed, platform-independent metrics so layouts are deterministic:
//! each character is `0.5em` wide and lines are `1.25em` tall.

mod services;

pub use services::{FakeServices, FakeServicesHandle, Pending, PendingAlert, PendingOpen, PendingSave};

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, A11yProps, ActionError};
use mitsuami_core::backend::{
    AvailableSpace, Backend, CaptureError, EventSink, FontSizes, Image, Key, MeasureRequest, NativeState,
    PlatformMetrics, SyntheticInput,
};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    Command, EventValue, NodeId, Orientation, Point, PointerEvent, PointerKind, Prop, Rect, RowKey, SelectionMode,
    Size, TextStyle, UiEvent, WidgetKind, find_prop,
};

/// Fixed metrics: 16px body text, 4/8/12/16/24 spacing, scale factor 1.
pub fn metrics() -> PlatformMetrics {
    PlatformMetrics {
        scale_factor: 1.0,
        spacing: SpacingScale { xs: 4.0, sm: 8.0, md: 12.0, lg: 16.0, xl: 24.0 },
        font_sizes: FontSizes {
            large_title: 32.0,
            title: 24.0,
            headline: 18.0,
            body: 16.0,
            callout: 15.0,
            caption: 12.0,
            monospace: 14.0,
        },
        dark_mode: false,
        high_contrast: false,
        reduced_motion: false,
    }
}

struct HeadlessNode {
    kind: WidgetKind,
    props: Vec<Prop>,
    a11y: A11yProps,
    frame: Rect,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    scroll_offset: Point,
    /// Lists only: the rows realised, as reported.
    shown: BTreeSet<RowKey>,
    /// Lists only: where the rows are, and each row's index in it.
    placed: Vec<Placed>,
    placed_index: std::collections::HashMap<RowKey, usize>,
    /// Lists only: the heights of the rows measured so far, kept when
    /// they're let go, as native lists keep them.
    heights: BTreeMap<RowKey, f32>,
}

/// Where a list placed a row.
#[derive(Clone, Copy)]
struct Placed {
    key: RowKey,
    top: f32,
    height: f32,
}

struct State {
    nodes: BTreeMap<NodeId, HeadlessNode>,
    events: Option<EventSink>,
    metrics: PlatformMetrics,
    log: Vec<Command>,
    focused: Option<NodeId>,
    focus_orders: BTreeMap<NodeId, Vec<NodeId>>,
}

impl State {
    fn node(&mut self, id: NodeId, command: &Command) -> &mut HeadlessNode {
        match self.nodes.get_mut(&id) {
            Some(node) => node,
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    fn emit(&self, id: NodeId, event: UiEvent) {
        if let Some(events) = &self.events {
            events.emit(id, event);
        }
    }

    fn set_prop(&mut self, id: NodeId, prop: Prop) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop);
        }
    }

    /// The next enabled control after `id` in its window's focus order (as
    /// sent by the core), wrapping around. Every control takes focus, as with
    /// full keyboard access.
    fn next_focusable(&self, id: NodeId) -> Option<NodeId> {
        let order = self.focus_orders.values().find(|order| order.contains(&id))?;
        let start = order.iter().position(|n| *n == id)?;
        (1..order.len())
            .map(|step| order[(start + step) % order.len()])
            .find(|candidate| self.nodes.get(candidate).is_some_and(|n| find_prop!(n.props, Enabled) != Some(false)))
    }

    /// Moves a scroll view, reporting it like a platform would. Lists then
    /// show the rows that came into view.
    fn scroll(&mut self, id: NodeId, offset: Point) {
        let node = self.nodes.get_mut(&id).unwrap();
        if node.scroll_offset != offset {
            node.scroll_offset = offset;
            let kind = node.kind;
            self.emit(id, UiEvent::Scrolled(offset));
            if kind == WidgetKind::List {
                self.show_rows(id);
            }
        }
    }

    /// A list's rows and where it placed them.
    fn rows(&self, list: NodeId) -> &[Placed] {
        &self.nodes[&list].placed
    }

    fn row(&self, list: NodeId, key: RowKey) -> Option<Placed> {
        let node = &self.nodes[&list];
        node.placed_index.get(&key).map(|i| node.placed[*i])
    }

    /// Places a list's rows one below the other, as native lists do: rows
    /// measured so far as high as their hosts were, the others as high as
    /// the estimate (the app's, or the mean of the rows measured so far, or
    /// two lines of text).
    fn place_rows(&mut self, list: NodeId) {
        let node = &self.nodes[&list];
        let rows: BTreeSet<RowKey> = find_prop!(node.props, Rows).unwrap_or_default().into_iter().collect();
        let measured: Vec<(RowKey, f32)> = node
            .children
            .iter()
            .filter_map(|host| {
                let host = &self.nodes[host];
                Some((find_prop!(host.props, Row)?, host.frame.height()))
            })
            .filter(|(_, height)| *height > 0.0)
            .collect();
        let heights = &mut self.nodes.get_mut(&list).unwrap().heights;
        heights.retain(|key, _| rows.contains(key));
        heights.extend(measured);
        let placed = self.placement(list);
        let node = self.nodes.get_mut(&list).unwrap();
        node.placed_index = placed.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
        node.placed = placed;
    }

    fn placement(&self, list: NodeId) -> Vec<Placed> {
        let node = &self.nodes[&list];
        let heights = &node.heights;
        let estimate = find_prop!(node.props, EstimatedRowHeight).unwrap_or_else(|| match heights.len() {
            0 => (self.metrics.font_sizes.body * 2.0).round(),
            n => (heights.values().sum::<f32>() / n as f32).round(),
        });
        let mut top = 0.0;
        find_prop!(node.props, Rows)
            .unwrap_or_default()
            .into_iter()
            .map(|key| {
                let height = heights.get(&key).copied().unwrap_or(estimate);
                top += height;
                Placed { key, top: top - height, height }
            })
            .collect()
    }

    /// Realises the rows in a list's view and lets go of the others,
    /// reporting both, as native lists do (without prefetching any).
    fn show_rows(&mut self, list: NodeId) {
        let node = &self.nodes[&list];
        let (start, end) = (node.scroll_offset.y, node.scroll_offset.y + node.frame.height());
        let shown: BTreeSet<RowKey> = if end > start {
            self.rows(list).iter().filter(|r| r.top < end && r.top + r.height > start).map(|r| r.key).collect()
        } else {
            BTreeSet::new()
        };
        let before = std::mem::replace(&mut self.nodes.get_mut(&list).unwrap().shown, shown.clone());
        for row in before.difference(&shown) {
            self.emit(list, UiEvent::RowHidden(*row));
        }
        for row in shown.difference(&before) {
            self.emit(list, UiEvent::RowShown(*row));
        }
    }

    /// The largest offset of a list: its rows' height less its own.
    fn clamp_list(&self, list: NodeId, y: f32) -> f32 {
        let total: f32 = self.rows(list).iter().map(|r| r.height).sum();
        y.clamp(0.0, (total - self.nodes[&list].frame.height()).max(0.0))
    }

    /// Selects rows as the user would, reporting it.
    fn select(&mut self, list: NodeId, selection: Vec<RowKey>) {
        let current = find_prop!(self.nodes[&list].props, Selected).unwrap_or_default();
        if current != selection {
            self.set_prop(list, Prop::Selected(selection.clone()));
            self.emit(list, UiEvent::Changed(EventValue::Rows(selection)));
        }
    }

    /// Scrolls a list just enough to show a row, as native lists do when
    /// the keyboard moves the selection.
    fn reveal(&mut self, list: NodeId, key: RowKey) {
        let Some(row) = self.row(list, key) else { return };
        let node = &self.nodes[&list];
        let (offset, visible) = (node.scroll_offset, node.frame.height());
        let y = if row.top < offset.y {
            row.top
        } else if row.top + row.height > offset.y + visible {
            row.top + row.height - visible
        } else {
            offset.y
        };
        let y = self.clamp_list(list, y);
        self.scroll(list, Point::new(offset.x, y));
    }

    /// Checks what a native list needs of its row hosts, once a batch is in.
    fn check_lists(&self, command: &Command) {
        for (id, node) in &self.nodes {
            if node.kind != WidgetKind::List {
                continue;
            }
            let rows = self.rows(*id);
            let mut last = None;
            for child in &node.children {
                let child = &self.nodes[child];
                let Some(key) = find_prop!(child.props, Row).filter(|_| child.kind == WidgetKind::Container) else {
                    violation(
                        command,
                        &format!("list {id} has a child that isn't a row host (a Container with a Prop::Row)"),
                    );
                };
                let Some(index) = rows.iter().position(|r| r.key == key) else {
                    violation(command, &format!("list {id} hosts row {key:?}, which isn't in its Prop::Rows"));
                };
                if last.is_some_and(|last| last >= index) {
                    violation(command, &format!("list {id}'s row hosts aren't in row order"));
                }
                last = Some(index);
            }
        }
    }

    fn focus(&mut self, id: NodeId) {
        if self.focused == Some(id) {
            return;
        }
        if let Some(previous) = self.focused.replace(id) {
            self.emit(previous, UiEvent::FocusOut);
        }
        self.emit(id, UiEvent::FocusIn);
    }
}

fn violation(command: &Command, problem: &str) -> ! {
    panic!("headless backend: protocol violation in {command:?}: {problem}")
}

/// The backend. Hand it to [`Ui::new`](mitsuami_core::Ui::new); keep a
/// [`HeadlessHandle`] to inspect it afterwards.
pub struct HeadlessBackend {
    state: Rc<RefCell<State>>,
}

/// Shared access to a [`HeadlessBackend`] after it was moved into a `Ui`.
#[derive(Clone)]
pub struct HeadlessHandle {
    state: Rc<RefCell<State>>,
}

impl Default for HeadlessBackend {
    fn default() -> Self {
        HeadlessBackend::new()
    }
}

impl HeadlessBackend {
    pub fn new() -> HeadlessBackend {
        HeadlessBackend {
            state: Rc::new(RefCell::new(State {
                nodes: BTreeMap::new(),
                events: None,
                metrics: metrics(),
                log: Vec::new(),
                focused: None,
                focus_orders: BTreeMap::new(),
            })),
        }
    }

    pub fn handle(&self) -> HeadlessHandle {
        HeadlessHandle { state: self.state.clone() }
    }
}

impl HeadlessHandle {
    /// Every command applied so far, in order.
    pub fn command_log(&self) -> Vec<Command> {
        self.state.borrow().log.clone()
    }

    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    /// Simulates the user resizing a window.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let mut state = self.state.borrow_mut();
        if let Some(node) = state.nodes.get_mut(&window) {
            node.frame.size = size;
        }
        state.emit(window, UiEvent::WindowResized(size));
    }

    /// Simulates a change of system settings (text size, dark mode, …).
    pub fn set_metrics(&self, metrics: PlatformMetrics) {
        let mut state = self.state.borrow_mut();
        state.metrics = metrics;
        let windows: Vec<NodeId> =
            state.nodes.iter().filter(|(_, n)| n.kind == WidgetKind::Window).map(|(id, _)| *id).collect();
        for window in windows {
            state.emit(window, UiEvent::MetricsChanged);
        }
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.state.borrow().focused
    }

    pub fn a11y(&self, id: NodeId) -> Option<A11yProps> {
        self.state.borrow().nodes.get(&id).map(|n| n.a11y.clone())
    }

    /// Number of live native nodes: a leak detector for tests.
    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }
}

impl mitsuami_core::TestHooks for HeadlessHandle {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        HeadlessHandle::resize_window(self, window, size);
    }

    fn take_command_log(&self) -> Vec<Command> {
        HeadlessHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        HeadlessHandle::node_count(self)
    }
}

impl Backend for HeadlessBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow_mut().events = Some(events);
    }

    fn metrics(&self) -> PlatformMetrics {
        self.state.borrow().metrics.clone()
    }

    fn apply(&mut self, batch: &[Command]) {
        let mut state = self.state.borrow_mut();
        for command in batch {
            state.log.push(command.clone());
            match command {
                Command::Create { id, kind, props } => {
                    if state.nodes.contains_key(id) {
                        violation(command, "node already exists");
                    }
                    if *kind == WidgetKind::Fragment {
                        violation(command, "fragments are core-only");
                    }
                    if matches!(kind, WidgetKind::Custom(_)) && find_prop!(props, Custom).is_none() {
                        violation(command, "custom widgets are created with their Prop::Custom");
                    }
                    if *kind == WidgetKind::Native && find_prop!(props, Native).is_none() {
                        violation(command, "native views are created with their Prop::Native");
                    }
                    state.nodes.insert(
                        *id,
                        HeadlessNode {
                            kind: *kind,
                            props: props.clone(),
                            a11y: A11yProps::default(),
                            frame: Rect::ZERO,
                            parent: None,
                            children: Vec::new(),
                            scroll_offset: Point::ZERO,
                            shown: BTreeSet::new(),
                            placed: Vec::new(),
                            placed_index: Default::default(),
                            heights: BTreeMap::new(),
                        },
                    );
                }
                Command::SetProp { id, prop } => {
                    state.node(*id, command);
                    state.set_prop(*id, prop.clone());
                    // Like native lists, removing rows deselects them.
                    if let Prop::Rows(rows) = prop {
                        let selected = find_prop!(state.nodes[id].props, Selected).unwrap_or_default();
                        let kept: Vec<RowKey> = selected.iter().copied().filter(|k| rows.contains(k)).collect();
                        if kept != selected {
                            state.select(*id, kept);
                        }
                    }
                }
                Command::Insert { parent, child, index } => {
                    if let Some(p) = state.node(*child, command).parent {
                        violation(command, &format!("child is still attached to {p}"));
                    }
                    let parent_node = state.node(*parent, command);
                    if parent_node.kind == WidgetKind::ScrollView && !parent_node.children.is_empty() {
                        violation(command, "a ScrollView has a single native child (its content)");
                    }
                    let siblings = &mut state.node(*parent, command).children;
                    if *index > siblings.len() {
                        violation(command, &format!("index out of bounds (len {})", siblings.len()));
                    }
                    siblings.insert(*index, *child);
                    state.node(*child, command).parent = Some(*parent);
                }
                Command::Remove { parent, child } => {
                    let siblings = &mut state.node(*parent, command).children;
                    let Some(pos) = siblings.iter().position(|c| c == child) else {
                        violation(command, "not a child of this parent");
                    };
                    siblings.remove(pos);
                    state.node(*child, command).parent = None;
                }
                Command::Destroy { id } => {
                    let node = state.nodes.remove(id).unwrap_or_else(|| violation(command, "node does not exist"));
                    if let Some(parent) = node.parent.and_then(|p| state.nodes.get_mut(&p)) {
                        parent.children.retain(|c| c != id);
                    }
                    for child in node.children {
                        if let Some(child) = state.nodes.get_mut(&child) {
                            child.parent = None;
                        }
                    }
                    if state.focused == Some(*id) {
                        state.focused = None;
                    }
                    state.focus_orders.remove(id);
                }
                Command::SetFrame { id, frame } => {
                    let node = state.node(*id, command);
                    if node.kind == WidgetKind::Window {
                        violation(command, "window frames belong to the platform");
                    }
                    node.frame = *frame;
                }
                Command::SetA11y { id, a11y } => state.node(*id, command).a11y = a11y.clone(),
                Command::SetWindowSize { id, size } => state.node(*id, command).frame.size = *size,
                Command::SetFocusOrder { window, order } => {
                    if state.node(*window, command).kind != WidgetKind::Window {
                        violation(command, "not a window");
                    }
                    for id in order {
                        state.node(*id, command);
                    }
                    state.focus_orders.insert(*window, order.clone());
                }
                Command::ScrollTo { id, offset } => {
                    if !state.node(*id, command).kind.scrolls() {
                        violation(command, "not a ScrollView or List");
                    }
                    state.scroll(*id, *offset);
                }
                Command::Focus { id } => {
                    state.node(*id, command);
                    state.focus(*id);
                }
                Command::ScrollToRow { id, row } => {
                    if state.node(*id, command).kind != WidgetKind::List {
                        violation(command, "not a List");
                    }
                    state.place_rows(*id);
                    state.reveal(*id, *row);
                }
            }
        }
        if let Some(last) = batch.last() {
            // New data, sizes or rows: what's in view may have changed.
            let lists: Vec<NodeId> =
                state.nodes.iter().filter(|(_, n)| n.kind == WidgetKind::List).map(|(id, _)| *id).collect();
            for list in &lists {
                state.place_rows(*list);
            }
            state.check_lists(last);
            for list in lists {
                let offset = state.nodes[&list].scroll_offset;
                let y = state.clamp_list(list, offset.y);
                if y != offset.y {
                    state.scroll(list, Point::new(offset.x, y));
                }
                state.show_rows(list);
            }
        }
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        let fonts = &state.metrics.font_sizes;
        let font = fonts.get(find_prop!(node.props, TextStyle).unwrap_or(TextStyle::Body));
        let line = (font * 1.25).round();
        let label = || find_prop!(node.props, Label).unwrap_or_default();
        let natural = match node.kind {
            WidgetKind::Text => {
                let text = find_prop!(node.props, Text).unwrap_or_default();
                let wrap = request.known_width.or(match request.available_width {
                    AvailableSpace::Definite(w) => Some(w),
                    AvailableSpace::MinContent => Some(0.0),
                    AvailableSpace::MaxContent => None,
                });
                text_size(&text, font, wrap)
            }
            WidgetKind::Button => {
                let text = text_size(&label(), font, None);
                Size::new(text.width + 24.0, (line + 8.0).max(28.0))
            }
            WidgetKind::TextInput => Size::new(200.0, line + 8.0),
            WidgetKind::Checkbox => {
                let text = text_size(&label(), font, None);
                Size::new(16.0 + 6.0 + text.width, line.max(16.0))
            }
            WidgetKind::Switch => Size::new(40.0, 24.0),
            WidgetKind::Slider => match find_prop!(node.props, Orientation).unwrap_or_default() {
                Orientation::Horizontal => Size::new(160.0, 20.0),
                Orientation::Vertical => Size::new(20.0, 160.0),
            },
            WidgetKind::Progress => Size::new(160.0, 8.0),
            // Sized for its chosen option, with room for the arrow.
            WidgetKind::Select => {
                let options = find_prop!(node.props, Options).unwrap_or_default();
                let chosen = find_prop!(node.props, SelectedIndex).flatten().and_then(|i| options.get(i).cloned());
                Size::new(text_size(&chosen.unwrap_or_default(), font, None).width + 32.0, (line + 8.0).max(28.0))
            }
            // Native renders are stood in for by the drawn one, if any.
            // Native views have no stand-in: size them with styles.
            WidgetKind::Custom(_) => find_prop!(node.props, Custom)
                .and_then(|c| c.measure_drawn(&request, &state.metrics))
                .unwrap_or(Size::ZERO),
            _ => Size::ZERO,
        };
        Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        let mut state = self.state.borrow_mut();
        let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
        if find_prop!(node.props, Enabled) == Some(false) {
            return Err(ActionError::Disabled);
        }
        let kind = node.kind;
        match (action, kind) {
            (A11yAction::Activate, WidgetKind::Button) => {
                state.focus(id);
                state.emit(id, UiEvent::Click);
            }
            (A11yAction::Activate, WidgetKind::Checkbox | WidgetKind::Switch) => {
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
            (A11yAction::SetValue(text), WidgetKind::TextInput) => {
                state.set_prop(id, Prop::Value(text.clone()));
                state.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
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
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let options = find_prop!(state.nodes[&id].props, Options).unwrap_or_default();
                let index = options.iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                state.set_prop(id, Prop::SelectedIndex(Some(index)));
                state.emit(id, UiEvent::Changed(EventValue::Index(index)));
            }
            (
                A11yAction::Focus,
                WidgetKind::Button
                | WidgetKind::TextInput
                | WidgetKind::Checkbox
                | WidgetKind::Switch
                | WidgetKind::Select
                | WidgetKind::Slider
                | WidgetKind::List,
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

    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        let mut state = self.state.borrow_mut();
        let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
        if find_prop!(node.props, Enabled) == Some(false) {
            return Err(ActionError::Disabled);
        }
        let kind = node.kind;
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
        };
        match (kind, key) {
            (WidgetKind::TextInput, Key::Char(_) | Key::Backspace) => {
                state.focus(id);
                let mut text = find_prop!(state.nodes[&id].props, Value).unwrap_or_default();
                match key {
                    Key::Char(c) => text.push(*c),
                    _ => {
                        text.pop();
                    }
                }
                state.set_prop(id, Prop::Value(text.clone()));
                state.emit(id, UiEvent::Changed(EventValue::Text(text)));
            }
            (WidgetKind::TextInput, Key::Enter) => state.emit(id, UiEvent::Submit),
            // Moves focus on; the field keeps its text and does not submit.
            (WidgetKind::TextInput, Key::Tab) => {
                if let Some(next) = state.next_focusable(id) {
                    state.focus(next);
                }
            }
            (WidgetKind::Button, Key::Enter | Key::Char(' ')) => state.emit(id, UiEvent::Click),
            (WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => {
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
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }

    fn native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        // A list places its rows itself: the core only gives their sizes.
        let row = find_prop!(node.props, Row)
            .zip(node.parent.filter(|p| state.nodes[p].kind == WidgetKind::List))
            .and_then(|(key, list)| state.row(list, key));
        let frame = match row {
            Some(row) => Rect::new(0.0, row.top, node.frame.width(), node.frame.height()),
            None => node.frame,
        };
        Some(NativeState {
            kind: node.kind,
            props: node.props.clone(),
            frame,
            parent: node.parent,
            children: node.children.clone(),
            focused: state.focused == Some(id),
            scroll_offset: node.kind.scrolls().then_some(node.scroll_offset),
        })
    }

    fn capture(&mut self, _id: NodeId, reply: mitsuami_core::services::Reply<Result<Image, CaptureError>>) {
        reply(Err(CaptureError::Unsupported));
    }

    fn services(&self) -> Box<dyn mitsuami_core::services::Services> {
        Box::new(FakeServices::default())
    }
}

/// Greedy word wrap with fixed-width characters.
fn text_size(text: &str, font: f32, wrap_width: Option<f32>) -> Size {
    let char_width = font * 0.5;
    let line_height = (font * 1.25).round();
    let mut lines: Vec<usize> = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = 0usize;
        for word in paragraph.split_whitespace() {
            let len = word.chars().count();
            let candidate = if current == 0 { len } else { current + 1 + len };
            let fits = wrap_width.is_none_or(|w| candidate as f32 * char_width <= w + 0.01);
            if current == 0 || fits {
                current = candidate;
            } else {
                lines.push(current);
                current = len;
            }
        }
        lines.push(current);
    }
    let widest = lines.iter().copied().max().unwrap_or(0);
    Size::new(widest as f32 * char_width, lines.len().max(1) as f32 * line_height)
}
