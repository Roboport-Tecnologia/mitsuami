//! The retained node tree and the update pipeline.
//!
//! The core tree is the source of truth. Backends mirror its *native* part:
//! every node except fragments, which are spliced into their nearest native
//! ancestor. Changes queue [`Command`]s; [`Ui::commit`] resolves styles, syncs
//! native children, applies the batch, runs layout and sends the new frames.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

use taffy::TaffyTree;

use crate::a11y::{A11yAction, A11yNode, A11yProps, ActionError, Role};
use crate::backend::{AvailableSpace, Backend, EventSink, MeasureRequest, NativeState, PlatformMetrics};
use crate::command::{Command, EventValue, UiEvent};
use crate::custom::CustomProps;
use crate::geometry::{Point, Rect, Size, WindowSize};
use crate::services::{Alert, MenuBar, OpenFile, SaveFile, ServiceError, Services, reply_future};
use crate::style::{Align, Display, FlexDirection, Style, TextDirection};
use crate::task::{Clock, Executor, Sleep, TaskHandle};
use crate::units::ResolveContext;
use crate::widget::{NodeId, Prop, RowKey, WidgetKind};

pub(crate) type Handler = Rc<dyn Fn(&UiEvent)>;
type Handler0 = Rc<dyn Fn()>;

struct Node {
    kind: WidgetKind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// Native nodes only: the flattened children last sent to the backend.
    native_children: Vec<NodeId>,
    native_parent: Option<NodeId>,
    props: Vec<Prop>,
    style: Style,
    a11y: A11yProps,
    test_id: Option<String>,
    tab_index: Option<u32>,
    handlers: Vec<Handler>,
    taffy: Option<taffy::NodeId>,
    /// Relative to the native parent.
    frame: Rect,
    /// Windows only: content size.
    window_size: Size,
    /// Windows only: the height is still to be fitted to the content.
    fit_height: bool,
    /// ScrollViews and Lists only: how far the content is scrolled.
    scroll_offset: Point,
    /// Lists only: the width the platform gives their rows, if not their own.
    row_width: Option<f32>,
}

impl Node {
    fn prop(&self, key: &Prop) -> Option<&Prop> {
        self.props.iter().find(|p| p.key() == key.key())
    }

    fn custom(&self) -> Option<&CustomProps> {
        self.props.iter().find_map(|p| match p {
            Prop::Custom(custom) => Some(custom),
            _ => None,
        })
    }

    /// Drawn custom widgets are measured and drawn by the core.
    fn drawn(&self) -> Option<&CustomProps> {
        self.custom().filter(|c| c.is_drawn())
    }
}

struct Inner {
    backend: Box<dyn Backend>,
    events: EventSink,
    metrics: PlatformMetrics,
    nodes: BTreeMap<NodeId, Node>,
    taffy: TaffyTree<NodeId>,
    next_id: u32,
    pending: Vec<Command>,
    /// Focus requests, sent after the structure of the batch: a node focused
    /// right after it's built isn't in a window yet.
    pending_focus: Vec<NodeId>,
    windows: Vec<NodeId>,
    styles_dirty: bool,
    resync: BTreeSet<NodeId>,
    menu_handlers: std::collections::HashMap<u32, Handler0>,
    menu_queue: std::collections::VecDeque<u32>,
    /// Last focus order sent, per window.
    focus_orders: BTreeMap<NodeId, Vec<NodeId>>,
    /// The focused control of each window, as reported by the backend.
    focused: BTreeMap<NodeId, NodeId>,
    commit_scheduler: Option<Rc<dyn Fn()>>,
    commit_scheduled: bool,
}

/// Handle to a UI instance: one backend, its windows and their node trees.
/// Cheap to clone; all clones share the same state.
#[derive(Clone)]
pub struct Ui {
    inner: Rc<RefCell<Inner>>,
    /// Kept apart from `inner`: tasks run while the tree is free to borrow.
    executor: Rc<Executor>,
    /// Kept apart too: replies may arrive at any time.
    services: Rc<RefCell<Box<dyn Services>>>,
}

/// Non-owning [`Ui`] handle, for closures stored inside the tree itself.
#[derive(Clone)]
pub struct WeakUi {
    inner: Weak<RefCell<Inner>>,
    executor: Weak<Executor>,
    services: Weak<RefCell<Box<dyn Services>>>,
}

impl WeakUi {
    pub fn upgrade(&self) -> Option<Ui> {
        Some(Ui {
            inner: self.inner.upgrade()?,
            executor: self.executor.upgrade()?,
            services: self.services.upgrade()?,
        })
    }
}

/// Snapshot of one native node, for inspection tools and tests.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeInfo {
    pub id: NodeId,
    pub kind: WidgetKind,
    pub props: Vec<Prop>,
    pub test_id: Option<String>,
    /// In window coordinates.
    pub frame: Rect,
    pub children: Vec<NodeInfo>,
}

impl Ui {
    pub fn new(backend: impl Backend + 'static) -> Ui {
        let mut backend: Box<dyn Backend> = Box::new(backend);
        let events = EventSink::default();
        backend.init(events.clone());
        let metrics = backend.metrics();
        let services = Rc::new(RefCell::new(backend.services()));
        Ui {
            executor: Rc::default(),
            services,
            inner: Rc::new(RefCell::new(Inner {
                backend,
                events,
                metrics,
                nodes: BTreeMap::new(),
                taffy: TaffyTree::new(),
                next_id: 1,
                pending: Vec::new(),
                pending_focus: Vec::new(),
                windows: Vec::new(),
                styles_dirty: true,
                resync: BTreeSet::new(),
                focus_orders: BTreeMap::new(),
                focused: BTreeMap::new(),
                menu_handlers: Default::default(),
                menu_queue: Default::default(),
                commit_scheduler: None,
                commit_scheduled: false,
            })),
        }
    }

    pub fn downgrade(&self) -> WeakUi {
        WeakUi {
            inner: Rc::downgrade(&self.inner),
            executor: Rc::downgrade(&self.executor),
            services: Rc::downgrade(&self.services),
        }
    }

    // ---------------------------------------------------------------- tasks

    pub(crate) fn executor(&self) -> &Executor {
        &self.executor
    }

    /// Runs `future` on the UI thread, polled from [`Ui::tick`]. Unlike the
    /// free [`spawn_local`](crate::task::spawn_local), it is not tied to a
    /// reactive scope; cancel it with the returned handle.
    pub fn spawn_local(&self, future: impl std::future::Future<Output = ()> + 'static) -> TaskHandle {
        self.spawn_in(future, None)
    }

    pub(crate) fn spawn_in(
        &self,
        future: impl std::future::Future<Output = ()> + 'static,
        owner: Option<mitsuami_reactive::Owner>,
    ) -> TaskHandle {
        let id = self.executor.spawn(Box::pin(future), owner);
        self.changed();
        TaskHandle::new(id, self.downgrade())
    }

    /// A future that completes after `duration` on this `Ui`'s clock.
    pub fn sleep(&self, duration: std::time::Duration) -> Sleep {
        Sleep::new(self.clone(), duration)
    }

    /// Replaces the clock timers use (tests install a [`ManualClock`](crate::task::ManualClock)).
    pub fn set_clock(&self, clock: Rc<dyn Clock>) {
        self.executor.set_clock(clock);
    }

    /// How to make the UI thread's run loop turn from any thread; called
    /// when a task is woken (e.g. by background work finishing).
    pub fn set_waker(&self, wake: std::sync::Arc<dyn Fn() + Send + Sync>) {
        self.executor.set_wake_ui(wake);
    }

    /// Time until the next timer fires (zero if overdue), for run loops to
    /// schedule a wake-up.
    pub fn time_to_next_timer(&self) -> Option<std::time::Duration> {
        let now = self.executor.now();
        self.executor.next_deadline().map(|deadline| deadline.saturating_sub(now))
    }

    // ------------------------------------------------------------- services

    /// Replaces the platform services (tests install a scripted fake).
    pub fn set_services(&self, services: Box<dyn Services>) {
        *self.services.borrow_mut() = services;
    }

    pub fn clipboard_text(&self) -> impl std::future::Future<Output = Option<String>> + use<> {
        let (reply, text) = reply_future();
        self.services.borrow_mut().clipboard_text(reply);
        text
    }

    /// Puts text on the clipboard. The write is issued right away; await
    /// the result to learn whether it worked.
    pub fn set_clipboard_text(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = Result<(), ServiceError>> + use<> {
        let (reply, done) = reply_future();
        self.services.borrow_mut().set_clipboard_text(text, reply);
        done
    }

    /// Shows an alert; resolves to the index of the chosen button.
    pub fn alert(&self, parent: Option<NodeId>, alert: Alert) -> impl std::future::Future<Output = usize> + use<> {
        let (reply, answer) = reply_future();
        self.services.borrow_mut().alert(parent, &alert, reply);
        answer
    }

    /// Resolves to the chosen paths, or `None` if cancelled.
    pub fn open_file(
        &self,
        parent: Option<NodeId>,
        request: OpenFile,
    ) -> impl std::future::Future<Output = Option<Vec<std::path::PathBuf>>> + use<> {
        let (reply, answer) = reply_future();
        self.services.borrow_mut().open_file(parent, &request, reply);
        answer
    }

    pub fn save_file(
        &self,
        parent: Option<NodeId>,
        request: SaveFile,
    ) -> impl std::future::Future<Output = Option<std::path::PathBuf>> + use<> {
        let (reply, answer) = reply_future();
        self.services.borrow_mut().save_file(parent, &request, reply);
        answer
    }

    /// Installs the app's menus. Reactive `enabled` states are re-sent to
    /// the platform when they change, for as long as the current scope lives.
    pub fn set_menu(&self, menu: MenuBar) {
        let (data, handlers) = menu.into_parts();
        self.inner.borrow_mut().menu_handlers = handlers
            .into_iter()
            .map(|(id, handler)| {
                let handler = crate::task::in_current_scope(move |()| handler());
                (id, Rc::new(move || handler(())) as Handler0)
            })
            .collect();
        let weak = self.downgrade();
        let activate: Rc<dyn Fn(u32)> = Rc::new(move |id| {
            if let Some(ui) = weak.upgrade() {
                ui.inner.borrow_mut().menu_queue.push_back(id);
                ui.changed();
            }
        });
        let services = self.services.clone();
        mitsuami_reactive::effect(move || {
            let data = data();
            services.borrow_mut().set_menu(&data, activate.clone());
        });
    }

    /// Tasks that have not finished yet.
    pub fn pending_tasks(&self) -> usize {
        self.executor.task_count()
    }

    pub fn events(&self) -> EventSink {
        self.inner.borrow().events.clone()
    }

    pub fn metrics(&self) -> PlatformMetrics {
        self.inner.borrow().metrics.clone()
    }

    /// Native backends call this to learn when a commit is needed. The
    /// callback should schedule [`Ui::commit`] on the run loop (e.g. before
    /// the next frame). Without a scheduler, callers commit explicitly.
    pub fn set_commit_scheduler(&self, schedule: impl Fn() + 'static) {
        self.inner.borrow_mut().commit_scheduler = Some(Rc::new(schedule));
    }

    fn changed(&self) {
        let schedule = {
            let mut inner = self.inner.borrow_mut();
            if inner.commit_scheduled {
                return;
            }
            inner.commit_scheduled = true;
            inner.commit_scheduler.clone()
        };
        if let Some(schedule) = schedule {
            schedule();
        }
    }

    // ------------------------------------------------------------ building

    pub fn create(&self, kind: WidgetKind, props: Vec<Prop>) -> NodeId {
        let id = {
            let mut inner = self.inner.borrow_mut();
            let id = NodeId(inner.next_id);
            inner.next_id += 1;
            let taffy = if kind.is_native() {
                let t = if kind.is_container() {
                    inner.taffy.new_leaf(taffy::Style::default())
                } else {
                    inner.taffy.new_leaf_with_context(taffy::Style::default(), id)
                };
                Some(t.expect("taffy: failed to create node"))
            } else {
                None
            };
            if kind.is_native() {
                inner.pending.push(Command::Create { id, kind, props: props.clone() });
            }
            inner.nodes.insert(
                id,
                Node {
                    kind,
                    parent: None,
                    children: Vec::new(),
                    native_children: Vec::new(),
                    native_parent: None,
                    props,
                    style: Style::default(),
                    a11y: A11yProps::default(),
                    test_id: None,
                    tab_index: None,
                    handlers: Vec::new(),
                    taffy,
                    frame: Rect::ZERO,
                    window_size: Size::ZERO,
                    fit_height: false,
                    scroll_offset: Point::ZERO,
                    row_width: None,
                },
            );
            inner.styles_dirty = true;
            id
        };
        self.changed();
        id
    }

    /// A window fitting its height is sized at its first layout, so give it
    /// its content before the next commit.
    pub fn create_window(&self, title: impl Into<String>, size: impl Into<WindowSize>) -> NodeId {
        let id = self.create(WidgetKind::Window, vec![Prop::Title(title.into())]);
        let mut inner = self.inner.borrow_mut();
        let node = inner.nodes.get_mut(&id).expect("just created");
        let (size, fit_height) = match size.into() {
            WindowSize::Fixed(size) => (size, false),
            WindowSize::FitHeight(width) => (Size::new(width, 0.0), true),
        };
        node.window_size = size;
        node.fit_height = fit_height;
        node.frame = Rect { origin: Point::ZERO, size };
        inner.windows.push(id);
        // A fitted size is sent with the first frames.
        if !fit_height {
            inner.pending.push(Command::SetWindowSize { id, size });
        }
        id
    }

    /// Sets a prop, sending it to the backend only if it changed.
    pub fn set_prop(&self, id: NodeId, prop: Prop) {
        {
            let mut inner = self.inner.borrow_mut();
            let inner = &mut *inner;
            let Some(node) = inner.nodes.get_mut(&id) else { return };
            if node.prop(&prop) == Some(&prop) {
                return;
            }
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop.clone());
            if prop.affects_measure()
                && let Some(t) = node.taffy
            {
                let _ = inner.taffy.mark_dirty(t);
            }
            if matches!(prop, Prop::TextStyle(_)) {
                inner.styles_dirty = true;
            }
            if node.kind.is_native() {
                inner.queue_prop(id, prop);
            }
        }
        self.changed();
    }

    pub fn set_style(&self, id: NodeId, style: Style) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(node) = inner.nodes.get_mut(&id) else { return };
            if node.style == style {
                return;
            }
            node.style = style;
            inner.styles_dirty = true;
        }
        self.changed();
    }

    /// Edits a node's style in place (used by reactive style bindings).
    pub fn update_style(&self, id: NodeId, f: impl FnOnce(&mut Style)) {
        let style = {
            let inner = self.inner.borrow();
            let Some(node) = inner.nodes.get(&id) else { return };
            let mut style = node.style.clone();
            f(&mut style);
            style
        };
        self.set_style(id, style);
    }

    pub fn set_a11y(&self, id: NodeId, a11y: A11yProps) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(node) = inner.nodes.get_mut(&id) else { return };
            if node.a11y == a11y {
                return;
            }
            node.a11y = a11y.clone();
            if node.kind.is_native() {
                inner.pending.push(Command::SetA11y { id, a11y });
            }
        }
        self.changed();
    }

    /// Moves a control ahead in the Tab order: controls with a tab index
    /// come first, lowest index first, then everything else in tree order.
    pub fn set_tab_index(&self, id: NodeId, index: Option<u32>) {
        if let Some(node) = self.inner.borrow_mut().nodes.get_mut(&id) {
            node.tab_index = index;
        }
        self.changed();
    }

    /// The control that has keyboard focus in `window`, if any.
    pub fn focused(&self, window: NodeId) -> Option<NodeId> {
        self.inner.borrow().focused.get(&window).copied()
    }

    /// The keyboard order of a window's focusable controls, as sent to the
    /// backend.
    pub fn focus_order(&self, window: NodeId) -> Vec<NodeId> {
        self.inner.borrow().focus_order(window)
    }

    pub fn set_test_id(&self, id: NodeId, test_id: impl Into<String>) {
        if let Some(node) = self.inner.borrow_mut().nodes.get_mut(&id) {
            node.test_id = Some(test_id.into());
        }
    }

    /// Adds an event handler. It runs in the reactive scope that is current
    /// now (usually the component being built), so `inject`, `spawn_local`
    /// and friends work inside it.
    pub fn on_event(&self, id: NodeId, handler: impl Fn(&UiEvent) + 'static) {
        let owner = mitsuami_reactive::Owner::current();
        let handler = move |event: &UiEvent| match owner {
            Some(owner) if owner.is_alive() => owner.with(|| handler(event)),
            _ => handler(event),
        };
        if let Some(node) = self.inner.borrow_mut().nodes.get_mut(&id) {
            node.handlers.push(Rc::new(handler));
        }
    }

    pub fn focus(&self, id: NodeId) {
        self.inner.borrow_mut().pending_focus.push(id);
        self.changed();
    }

    // ------------------------------------------------------- tree structure

    /// Inserts `child` under `parent` at `index` (or at the end), detaching it
    /// from its previous parent first.
    pub fn insert_child(&self, parent: NodeId, child: NodeId, index: Option<usize>) {
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.nodes.contains_key(&parent) || !inner.nodes.contains_key(&child) {
                return;
            }
            inner.detach(child);
            let siblings = &mut inner.nodes.get_mut(&parent).unwrap().children;
            let index = index.unwrap_or(siblings.len()).min(siblings.len());
            siblings.insert(index, child);
            inner.nodes.get_mut(&child).unwrap().parent = Some(parent);
            inner.mark_resync(parent);
            inner.styles_dirty = true;
        }
        self.changed();
    }

    pub fn append_child(&self, parent: NodeId, child: NodeId) {
        self.insert_child(parent, child, None);
    }

    /// Replaces the children of `parent`, keeping nodes that stay.
    pub fn set_children(&self, parent: NodeId, children: Vec<NodeId>) {
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.nodes.contains_key(&parent) {
                return;
            }
            let old = inner.nodes[&parent].children.clone();
            for child in old.iter().filter(|c| !children.contains(c)) {
                inner.detach(*child);
            }
            for child in &children {
                if inner.nodes.get(child).and_then(|n| n.parent) != Some(parent) {
                    inner.detach(*child);
                }
                if let Some(node) = inner.nodes.get_mut(child) {
                    node.parent = Some(parent);
                }
            }
            inner.nodes.get_mut(&parent).unwrap().children = children;
            inner.mark_resync(parent);
            inner.styles_dirty = true;
        }
        self.changed();
    }

    /// Removes a node and its whole subtree, natively too.
    pub fn destroy(&self, id: NodeId) {
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.nodes.contains_key(&id) {
                return;
            }
            inner.detach(id);
            // Remove the subtree's native roots from their native parent now,
            // so the backend sees Remove before Destroy.
            let roots = inner.native_roots(id);
            for root in &roots {
                inner.detach_native(*root);
            }
            let mut subtree = Vec::new();
            inner.collect_subtree(id, &mut subtree);
            for node_id in subtree {
                let node = inner.nodes.remove(&node_id).expect("in subtree");
                if let Some(t) = node.taffy {
                    let _ = inner.taffy.remove(t);
                }
                inner.resync.remove(&node_id);
                inner.windows.retain(|w| *w != node_id);
                inner.focus_orders.remove(&node_id);
                inner.focused.retain(|window, focused| *window != node_id && *focused != node_id);
                if node.kind.is_native() {
                    inner.pending.push(Command::Destroy { id: node_id });
                }
            }
        }
        self.changed();
    }

    // ------------------------------------------------------------- pipeline

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
                id.and_then(|id| inner.menu_handlers.get(&id).cloned())
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
        let mut inner = self.inner.borrow_mut();
        inner.commit_scheduled = false;
        if inner.styles_dirty {
            inner.resolve_styles();
        }
        inner.resync_all();
        inner.sync_focus_orders();
        for id in std::mem::take(&mut inner.pending_focus) {
            if inner.nodes.contains_key(&id) {
                inner.pending.push(Command::Focus { id });
            }
        }
        let batch = std::mem::take(&mut inner.pending);
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
    /// commit itself produced new events (e.g. a window resize), until idle.
    /// Backends call this from their run loop, before it goes to sleep.
    pub fn tick(&self) {
        const MAX_TURNS: usize = 64;
        for _ in 0..MAX_TURNS {
            self.executor.run_ready(self);
            self.process_events();
            self.commit();
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

    // ------------------------------------------------------------- queries

    pub fn windows(&self) -> Vec<NodeId> {
        self.inner.borrow().windows.clone()
    }

    pub fn exists(&self, id: NodeId) -> bool {
        self.inner.borrow().nodes.contains_key(&id)
    }

    pub fn kind(&self, id: NodeId) -> Option<WidgetKind> {
        self.inner.borrow().nodes.get(&id).map(|n| n.kind)
    }

    pub fn props(&self, id: NodeId) -> Vec<Prop> {
        self.inner.borrow().nodes.get(&id).map(|n| n.props.clone()).unwrap_or_default()
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.inner.borrow().nodes.get(&id).and_then(|n| n.parent)
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.inner.borrow().nodes.get(&id).map(|n| n.children.clone()).unwrap_or_default()
    }

    pub fn native_children(&self, id: NodeId) -> Vec<NodeId> {
        self.inner.borrow().nodes.get(&id).map(|n| n.native_children.clone()).unwrap_or_default()
    }

    /// Frame relative to the native parent, as last sent to the backend.
    /// A list's rows are where the platform put them, at the size sent.
    pub fn frame(&self, id: NodeId) -> Option<Rect> {
        let inner = self.inner.borrow();
        inner.nodes.contains_key(&id).then(|| inner.placed_frame(id))
    }

    /// Frame in the coordinates of the node's window.
    pub fn window_frame(&self, id: NodeId) -> Option<Rect> {
        let inner = self.inner.borrow();
        let node = inner.nodes.get(&id)?;
        if node.kind == WidgetKind::Window {
            return Some(node.frame);
        }
        Some(inner.placed_frame(id).offset(inner.window_origin(id)?))
    }

    /// The part of a node that can be seen: its window frame, clipped by
    /// every enclosing scroll view and by the window. `None` if nothing is.
    pub fn visible_rect(&self, id: NodeId) -> Option<Rect> {
        let window = self.window_of(id)?;
        let size = self.window_size(window)?;
        let mut visible = self.window_frame(id)?.intersection(&Rect::new(0.0, 0.0, size.width, size.height))?;
        let mut current = self.inner.borrow().nodes.get(&id)?.native_parent;
        while let Some(ancestor) = current {
            if self.kind(ancestor).is_some_and(WidgetKind::scrolls) {
                visible = visible.intersection(&self.window_frame(ancestor)?)?;
            }
            current = self.inner.borrow().nodes.get(&ancestor)?.native_parent;
        }
        Some(visible)
    }

    /// Current scroll offset of a `ScrollView` or `List`.
    pub fn scroll_offset(&self, id: NodeId) -> Option<Point> {
        let inner = self.inner.borrow();
        let node = inner.nodes.get(&id)?;
        node.kind.scrolls().then_some(node.scroll_offset)
    }

    /// Scrolls a `ScrollView` or `List`, clamping to its content (a list
    /// clamps it itself: its content is the platform's).
    pub fn scroll_to(&self, id: NodeId, offset: Point) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(offset) = inner.clamp_scroll(id, offset) else { return };
            let node = inner.nodes.get_mut(&id).unwrap();
            if node.scroll_offset == offset {
                return;
            }
            node.scroll_offset = offset;
            inner.pending.push(Command::ScrollTo { id, offset });
        }
        self.changed();
    }

    /// Scrolls every enclosing `ScrollView` and `List` just enough to show
    /// the node.
    pub fn scroll_into_view(&self, id: NodeId) {
        let mut target = id;
        loop {
            let scroll_view = {
                let inner = self.inner.borrow();
                let mut current = inner.nodes.get(&target).and_then(|n| n.native_parent);
                while let Some(ancestor) = current {
                    if inner.nodes[&ancestor].kind.scrolls() {
                        break;
                    }
                    current = inner.nodes[&ancestor].native_parent;
                }
                current
            };
            let Some(scroll_view) = scroll_view else { return };
            // In a list, show the target's row: where rows are is the
            // platform's business.
            let row = {
                let inner = self.inner.borrow();
                let mut current = Some(target);
                while let Some(node) = current.and_then(|c| inner.nodes.get(&c)) {
                    if node.native_parent == Some(scroll_view) {
                        break;
                    }
                    current = node.native_parent;
                }
                (inner.nodes[&scroll_view].kind == WidgetKind::List)
                    .then(|| current.and_then(|host| crate::find_prop!(inner.nodes[&host].props, Row)))
            };
            if let Some(row) = row {
                if let Some(row) = row {
                    self.scroll_to_row(scroll_view, row);
                }
                target = scroll_view;
                continue;
            }
            let (Some(rect), Some(viewport)) = (self.window_frame(target), self.window_frame(scroll_view)) else {
                return;
            };
            let offset = self.scroll_offset(scroll_view).unwrap_or_default();
            // The target in content coordinates, and what currently shows.
            let x = rect.x() - viewport.x() + offset.x;
            let y = rect.y() - viewport.y() + offset.y;
            let next = Point::new(
                fit(x, rect.width(), offset.x, viewport.width()),
                fit(y, rect.height(), offset.y, viewport.height()),
            );
            self.scroll_to(scroll_view, next);
            target = scroll_view;
        }
    }

    // ---------------------------------------------------------------- lists

    /// Scrolls a `List` just enough to show a row, mounted or not. The
    /// platform scrolls and reports it.
    pub(crate) fn scroll_to_row(&self, id: NodeId, row: RowKey) {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.nodes.get(&id).is_none_or(|n| n.kind != WidgetKind::List) {
                return;
            }
            inner.pending.push(Command::ScrollToRow { id, row });
        }
        self.changed();
    }

    /// The window a node is attached to, if any.
    pub fn window_of(&self, id: NodeId) -> Option<NodeId> {
        self.inner.borrow().window_of(id)
    }

    pub fn window_size(&self, window: NodeId) -> Option<Size> {
        self.inner.borrow().nodes.get(&window).map(|n| n.window_size)
    }

    pub fn native_state(&self, id: NodeId) -> Option<NativeState> {
        self.inner.borrow().backend.native_state(id)
    }

    /// An offscreen screenshot of a window or node.
    pub fn capture(
        &self,
        id: NodeId,
    ) -> impl std::future::Future<Output = Result<crate::backend::Image, crate::backend::CaptureError>> + use<> {
        let (reply, image) = reply_future();
        self.inner.borrow_mut().backend.capture(id, reply);
        image
    }

    /// The native tree under `id`, with window-coordinate frames.
    pub fn inspect(&self, id: NodeId) -> Option<NodeInfo> {
        let inner = self.inner.borrow();
        let origin = inner.window_origin(id)?;
        Some(inner.inspect(id, origin))
    }

    /// The computed accessibility tree of a window.
    pub fn a11y_tree(&self, window: NodeId) -> Option<A11yNode> {
        let inner = self.inner.borrow();
        let mut nodes = inner.a11y(window, Point::ZERO);
        (nodes.len() == 1).then(|| nodes.remove(0))
    }
}

impl Inner {
    /// Sends a prop. Until the node's `Create` has gone out, the prop joins
    /// it, so backends create every widget with its initial props (reactive
    /// ones included): custom widgets and native views can't be created
    /// without theirs.
    fn queue_prop(&mut self, id: NodeId, prop: Prop) {
        // A new range may clamp a slider's value: the value follows it.
        let then = match prop {
            Prop::Range { .. } => {
                self.nodes.get(&id).and_then(|n| n.props.iter().find(|p| matches!(p, Prop::Number(_)))).cloned()
            }
            _ => None,
        };
        self.queue_prop_now(id, prop);
        if let Some(number) = then {
            self.queue_prop_now(id, number);
        }
    }

    fn queue_prop_now(&mut self, id: NodeId, prop: Prop) {
        let create = self.pending.iter_mut().rev().find_map(|command| match command {
            Command::Create { id: created, props, .. } if *created == id => Some(props),
            _ => None,
        });
        match create {
            Some(props) => {
                props.retain(|p| p.key() != prop.key());
                props.push(prop);
            }
            None => self.pending.push(Command::SetProp { id, prop }),
        }
    }

    fn mark_resync(&mut self, id: NodeId) {
        if let Some(native) = self.native_ancestor_or_self(id) {
            self.resync.insert(native);
        }
    }

    fn native_ancestor_or_self(&self, id: NodeId) -> Option<NodeId> {
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.nodes.get(&node_id)?;
            if node.kind.is_native() {
                return Some(node_id);
            }
            current = node.parent;
        }
        None
    }

    fn detach(&mut self, child: NodeId) {
        let Some(parent) = self.nodes.get(&child).and_then(|n| n.parent) else { return };
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.children.retain(|c| *c != child);
        }
        self.nodes.get_mut(&child).unwrap().parent = None;
        self.mark_resync(parent);
    }

    /// Removes a native node from its native parent, immediately.
    fn detach_native(&mut self, child: NodeId) {
        let Some(parent) = self.nodes.get(&child).and_then(|n| n.native_parent) else { return };
        self.pending.push(Command::Remove { parent, child });
        let child_taffy = self.nodes[&child].taffy;
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.native_children.retain(|c| *c != child);
            // A list's rows aren't in its layout box (`layout_list`).
            if let (Some(p), Some(c), false) = (node.taffy, child_taffy, node.kind == WidgetKind::List) {
                let _ = self.taffy.remove_child(p, c);
            }
        }
        self.nodes.get_mut(&child).unwrap().native_parent = None;
    }

    /// The native nodes a subtree contributes to its native parent.
    fn native_roots(&self, id: NodeId) -> Vec<NodeId> {
        let node = &self.nodes[&id];
        if node.kind.is_native() {
            return vec![id];
        }
        node.children.iter().flat_map(|c| self.native_roots(*c)).collect()
    }

    fn flattened_children(&self, id: NodeId) -> Vec<NodeId> {
        self.nodes[&id].children.iter().flat_map(|c| self.native_roots(*c)).collect()
    }

    fn collect_subtree(&self, id: NodeId, out: &mut Vec<NodeId>) {
        for child in &self.nodes[&id].children {
            self.collect_subtree(*child, out);
        }
        out.push(id);
    }

    fn resync_all(&mut self) {
        let dirty = std::mem::take(&mut self.resync);
        for id in dirty {
            if self.nodes.contains_key(&id) {
                self.resync_node(id);
            }
        }
    }

    /// Makes the backend's children of `parent` match the core tree.
    fn resync_node(&mut self, parent: NodeId) {
        let desired = self.flattened_children(parent);
        let current = self.nodes[&parent].native_children.clone();
        for child in current.iter().filter(|c| !desired.contains(c)) {
            self.detach_native(*child);
        }
        let mut working: Vec<NodeId> = self.nodes[&parent].native_children.clone();
        for (index, child) in desired.iter().enumerate() {
            if working.get(index) == Some(child) {
                continue;
            }
            if let Some(pos) = working.iter().position(|c| c == child) {
                working.remove(pos);
                self.pending.push(Command::Remove { parent, child: *child });
            } else if let Some(other) = self.nodes[child].native_parent
                && other != parent
            {
                self.detach_native(*child);
            }
            working.insert(index, *child);
            self.pending.push(Command::Insert { parent, child: *child, index });
        }
        for child in &desired {
            self.nodes.get_mut(child).unwrap().native_parent = Some(parent);
        }
        let taffy_children: Vec<_> = desired.iter().filter_map(|c| self.nodes[c].taffy).collect();
        // A list's rows are laid out on their own (`layout_list`).
        if let Some(t) = self.nodes[&parent].taffy.filter(|_| self.nodes[&parent].kind != WidgetKind::List) {
            let _ = self.taffy.set_children(t, &taffy_children);
        }
        self.nodes.get_mut(&parent).unwrap().native_children = desired;
    }

    /// Focusable controls in reading (tree) order, explicit tab indices first.
    fn focus_order(&self, window: NodeId) -> Vec<NodeId> {
        fn walk(inner: &Inner, id: NodeId, out: &mut Vec<(Option<u32>, NodeId)>) {
            let node = &inner.nodes[&id];
            if node.style.is_hidden() {
                return;
            }
            if matches!(
                node.kind,
                WidgetKind::Button
                    | WidgetKind::TextInput
                    | WidgetKind::Checkbox
                    | WidgetKind::Switch
                    | WidgetKind::Select
                    | WidgetKind::Slider
                    | WidgetKind::List
            ) {
                out.push((node.tab_index, id));
            }
            for child in &node.native_children {
                walk(inner, *child, out);
            }
        }
        let mut entries = Vec::new();
        if self.nodes.contains_key(&window) {
            walk(self, window, &mut entries);
        }
        // Stable sort: explicit indices first (ascending), tree order otherwise.
        entries.sort_by_key(|(index, _)| index.unwrap_or(u32::MAX));
        entries.into_iter().map(|(_, id)| id).collect()
    }

    fn sync_focus_orders(&mut self) {
        for window in self.windows.clone() {
            let order = self.focus_order(window);
            if self.focus_orders.get(&window) != Some(&order) {
                self.focus_orders.insert(window, order.clone());
                self.pending.push(Command::SetFocusOrder { window, order });
            }
        }
    }

    fn resolve_styles(&mut self) {
        self.styles_dirty = false;
        let body = self.metrics.font_sizes.body;
        for window in self.windows.clone() {
            let viewport = self.nodes[&window].window_size;
            self.resolve_node(window, body, false, viewport, false);
        }
    }

    /// `in_stretching_column`: the layout parent is a flex column that
    /// stretches its children across (the default).
    fn resolve_node(
        &mut self,
        id: NodeId,
        inherited_font: f32,
        inherited_rtl: bool,
        viewport: Size,
        in_stretching_column: bool,
    ) {
        let node = &self.nodes[&id];
        let font_size = match crate::find_prop!(node.props, TextStyle) {
            Some(style) => self.metrics.font_sizes.get(style),
            None => inherited_font,
        };
        let rtl = match node.style.direction {
            TextDirection::Inherit => inherited_rtl,
            TextDirection::Ltr => false,
            TextDirection::Rtl => true,
        };
        if let Some(t) = node.taffy {
            let cx = ResolveContext {
                font_size,
                root_font_size: self.metrics.font_sizes.body,
                viewport,
                spacing: &self.metrics.spacing,
            };
            let mut style = node.style.to_taffy(&cx, rtl);
            // Toggles have a fixed natural size, like CSS replaced elements:
            // stretched, some platforms draw them centered in the extra
            // space (`NSSwitch`) and all of them take clicks there.
            if matches!(node.kind, WidgetKind::Checkbox | WidgetKind::Switch) {
                style.justify_self.get_or_insert(taffy::AlignSelf::START);
                if in_stretching_column {
                    style.align_self.get_or_insert(taffy::AlignSelf::START);
                }
            }
            if node.kind == WidgetKind::Window {
                style.size = taffy::Size {
                    width: taffy::Dimension::length(node.window_size.width),
                    height: if node.fit_height {
                        taffy::Dimension::auto()
                    } else {
                        taffy::Dimension::length(node.window_size.height)
                    },
                };
            }
            if self.taffy.style(t).ok() != Some(&style) {
                let _ = self.taffy.set_style(t, style);
            }
        }
        // Nodes without a layout box pass their parent's context through.
        let in_stretching_column = match node.taffy {
            Some(_) => {
                let s = &node.style;
                s.display == Display::Flex
                    && matches!(s.flex_direction, FlexDirection::Column | FlexDirection::ColumnReverse)
                    && matches!(s.align_items, None | Some(Align::Stretch))
            }
            None => in_stretching_column,
        };
        for child in self.nodes[&id].children.clone() {
            self.resolve_node(child, font_size, rtl, viewport, in_stretching_column);
        }
    }

    fn layout(&mut self) {
        for window in self.windows.clone() {
            let node = &self.nodes[&window];
            let Some(root) = node.taffy else { continue };
            let (mut size, fit_height) = (node.window_size, node.fit_height);
            let available = taffy::Size {
                width: taffy::AvailableSpace::Definite(size.width),
                height: if fit_height {
                    taffy::AvailableSpace::MaxContent
                } else {
                    taffy::AvailableSpace::Definite(size.height)
                },
            };
            self.compute_layout(root, available);
            if fit_height {
                size.height = self.taffy.layout(root).map_or(0.0, |l| l.size.height).ceil();
                let node = self.nodes.get_mut(&window).unwrap();
                node.window_size = size;
                node.fit_height = false;
                // The window's style takes the fitted height, and `vh`
                // re-resolves against it.
                self.styles_dirty = true;
                self.pending.push(Command::SetWindowSize { id: window, size });
            }
            self.nodes.get_mut(&window).unwrap().frame = Rect { origin: Point::ZERO, size };
            self.collect_frames(window);
        }
        self.update_drawings();
    }

    /// Lays out the tree under `root`, measuring leaves natively (or, for
    /// drawn custom widgets, with their shared measure).
    fn compute_layout(&mut self, root: taffy::NodeId, available: taffy::Size<taffy::AvailableSpace>) {
        let Inner { taffy, backend, nodes, metrics, .. } = self;
        let _ = taffy.compute_layout_with_measure(root, available, |input, _, context, style| {
            taffy::compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, available| match context {
                    Some(id) => {
                        let request = MeasureRequest {
                            known_width: known.width,
                            known_height: known.height,
                            available_width: space(available.width),
                            available_height: space(available.height),
                        };
                        let node = nodes.get(id);
                        let size = match node.and_then(|n| n.drawn()) {
                            Some(custom) => {
                                let size = custom.measure_drawn(&request, metrics).unwrap_or(Size::ZERO);
                                Size::new(known.width.unwrap_or(size.width), known.height.unwrap_or(size.height))
                            }
                            // Like a scroll view, a list has no natural size
                            // of its own: it's as big as its style makes it.
                            None if node.is_some_and(|n| n.kind == WidgetKind::List) => {
                                Size::new(known.width.unwrap_or(0.0), known.height.unwrap_or(0.0))
                            }
                            None => backend.measure(*id, request),
                        };
                        taffy::Size { width: size.width, height: size.height }
                    }
                    None => taffy::Size::ZERO,
                },
            )
        });
    }

    /// Lays out a `List`'s mounted rows, each on its own at the width the
    /// list gives its rows. Their sizes are sent; where they go is the
    /// platform's to decide.
    fn layout_list(&mut self, list: NodeId) {
        let node = &self.nodes[&list];
        let width = node.row_width.unwrap_or(node.frame.width());
        for host in node.native_children.clone() {
            let Some(t) = self.nodes[&host].taffy else { continue };
            if let Ok(style) = self.taffy.style(t)
                && style.size.width != taffy::Dimension::length(width)
            {
                let mut style = style.clone();
                style.size.width = taffy::Dimension::length(width);
                let _ = self.taffy.set_style(t, style);
            }
            let available = taffy::Size {
                width: taffy::AvailableSpace::Definite(width),
                height: taffy::AvailableSpace::MaxContent,
            };
            self.compute_layout(t, available);
            let height = self.taffy.layout(t).map_or(0.0, |l| l.size.height);
            let frame = Rect::new(0.0, 0.0, width, height);
            if self.nodes[&host].frame != frame {
                self.nodes.get_mut(&host).unwrap().frame = frame;
                self.pending.push(Command::SetFrame { id: host, frame });
            }
            self.collect_frames(host);
        }
    }

    /// Redraws drawn custom widgets, sending the display lists that changed
    /// (new props, a new size, new metrics) along with the frames.
    fn update_drawings(&mut self) {
        let mut changed = Vec::new();
        for (id, node) in &self.nodes {
            let Some(drawing) = node.drawn().and_then(|c| c.draw(node.frame.size, &self.metrics)) else { continue };
            let drawing = Prop::Drawing(drawing);
            if node.prop(&drawing) != Some(&drawing) {
                changed.push((*id, drawing));
            }
        }
        for (id, prop) in changed {
            let node = self.nodes.get_mut(&id).unwrap();
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop.clone());
            self.pending.push(Command::SetProp { id, prop });
        }
    }

    fn collect_frames(&mut self, parent: NodeId) {
        if self.nodes[&parent].kind == WidgetKind::List {
            return self.layout_list(parent);
        }
        for child in self.nodes[&parent].native_children.clone() {
            let node = &self.nodes[&child];
            if let Some(t) = node.taffy
                && let Ok(layout) = self.taffy.layout(t)
            {
                let frame = Rect::new(layout.location.x, layout.location.y, layout.size.width, layout.size.height);
                if frame != node.frame {
                    self.nodes.get_mut(&child).unwrap().frame = frame;
                    self.pending.push(Command::SetFrame { id: child, frame });
                }
            }
            self.collect_frames(child);
        }
    }

    /// Updates core state from events the native side already reflects, so
    /// echoing the value back is a no-op.
    fn absorb(&mut self, id: NodeId, event: &UiEvent) {
        match event {
            UiEvent::Changed(value) => {
                let Some(node) = self.nodes.get_mut(&id) else { return };
                let prop = match (node.kind, value) {
                    (WidgetKind::TextInput, EventValue::Text(text)) => Prop::Value(text.clone()),
                    (WidgetKind::Checkbox | WidgetKind::Switch, EventValue::Bool(b)) => Prop::Checked(*b),
                    (WidgetKind::List, EventValue::Rows(rows)) => Prop::Selected(rows.clone()),
                    (WidgetKind::Select, EventValue::Index(index)) => Prop::SelectedIndex(Some(*index)),
                    (WidgetKind::Slider, EventValue::Number(number)) => Prop::Number(*number),
                    _ => return,
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
            UiEvent::WindowResized(size) => {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.window_size = *size;
                    self.styles_dirty = true;
                }
            }
            UiEvent::MetricsChanged => {
                self.metrics = self.backend.metrics();
                self.styles_dirty = true;
                for node in self.nodes.values() {
                    if let Some(t) = node.taffy {
                        let _ = self.taffy.mark_dirty(t);
                    }
                }
            }
            _ => {}
        }
    }

    fn window_of(&self, id: NodeId) -> Option<NodeId> {
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.nodes.get(&node_id)?;
            if node.kind == WidgetKind::Window {
                return Some(node_id);
            }
            current = node.parent;
        }
        None
    }

    /// Window coordinates of the point `id`'s children are positioned
    /// from: its own top-left, moved by its scroll offset.
    fn content_origin(&self, id: NodeId) -> Point {
        let node = &self.nodes[&id];
        if node.kind == WidgetKind::Window {
            return Point::ZERO;
        }
        let parent = node.native_parent.map_or(Point::ZERO, |p| self.content_origin(p));
        let origin = self.placed_frame(id).origin;
        Point::new(parent.x + origin.x - node.scroll_offset.x, parent.y + origin.y - node.scroll_offset.y)
    }

    /// Window coordinates of the origin `id`'s frame is relative to.
    fn window_origin(&self, id: NodeId) -> Option<Point> {
        Some(self.nodes.get(&id)?.native_parent.map_or(Point::ZERO, |p| self.content_origin(p)))
    }

    /// A node's frame in its native parent. A list's rows are where the
    /// platform placed them (as its `native_state` says), at the size the
    /// core sent.
    fn placed_frame(&self, id: NodeId) -> Rect {
        let node = &self.nodes[&id];
        match node.native_parent.map(|p| self.nodes[&p].kind) {
            Some(WidgetKind::List) => {
                let origin = self.backend.native_state(id).map_or(node.frame.origin, |s| s.frame.origin);
                Rect { origin, size: node.frame.size }
            }
            _ => node.frame,
        }
    }

    fn clamp_scroll(&self, id: NodeId, offset: Point) -> Option<Point> {
        let node = self.nodes.get(&id)?;
        let (axes, content) = match node.kind {
            WidgetKind::ScrollView => (
                crate::find_prop!(node.props, ScrollAxes).unwrap_or_default(),
                node.native_children.first().map_or(Size::ZERO, |c| self.nodes[c].frame.size),
            ),
            WidgetKind::List => (crate::ScrollAxes::Vertical, Size::new(node.frame.width(), f32::INFINITY)),
            _ => return None,
        };
        let viewport = node.frame.size;
        let clamp = |v: f32, content: f32, viewport: f32, enabled: bool| {
            if enabled { v.clamp(0.0, (content - viewport).max(0.0)) } else { 0.0 }
        };
        Some(Point::new(
            clamp(offset.x, content.width, viewport.width, axes.horizontal()),
            clamp(offset.y, content.height, viewport.height, axes.vertical()),
        ))
    }

    fn child_origin(node: &Node, frame: Rect) -> Point {
        match node.kind {
            WidgetKind::Window => Point::ZERO,
            _ => Point::new(frame.origin.x - node.scroll_offset.x, frame.origin.y - node.scroll_offset.y),
        }
    }

    fn inspect(&self, id: NodeId, parent_origin: Point) -> NodeInfo {
        let node = &self.nodes[&id];
        let frame =
            if node.kind == WidgetKind::Window { node.frame } else { self.placed_frame(id).offset(parent_origin) };
        let origin = Inner::child_origin(node, frame);
        NodeInfo {
            id,
            kind: node.kind,
            props: node.props.clone(),
            test_id: node.test_id.clone(),
            frame,
            children: node.native_children.iter().map(|c| self.inspect(*c, origin)).collect(),
        }
    }

    fn text_of(&self, id: NodeId) -> Option<String> {
        let node = self.nodes.get(&id)?;
        crate::find_prop!(node.props, Text).or_else(|| crate::find_prop!(node.props, Label))
    }

    fn a11y(&self, id: NodeId, parent_origin: Point) -> Vec<A11yNode> {
        let node = &self.nodes[&id];
        // Custom widgets bring their own semantics; the app's overrides win.
        let a11y = match node.custom() {
            Some(custom) => custom.a11y().overridden_by(&node.a11y),
            None => node.a11y.clone(),
        };
        if a11y.hidden || node.style.is_hidden() {
            return Vec::new();
        }
        let frame =
            if node.kind == WidgetKind::Window { node.frame } else { self.placed_frame(id).offset(parent_origin) };
        let origin = Inner::child_origin(node, frame);
        let children: Vec<A11yNode> = node.native_children.iter().flat_map(|c| self.a11y(*c, origin)).collect();

        let labelled = a11y.label.is_some() || a11y.labelled_by.is_some();
        let row = crate::find_prop!(node.props, Row);
        let role = a11y.role.unwrap_or(match node.kind {
            WidgetKind::Window => Role::Window,
            WidgetKind::Container if row.is_some() => Role::ListItem,
            WidgetKind::Container if labelled => Role::Group,
            WidgetKind::Container | WidgetKind::Fragment => Role::None,
            WidgetKind::ScrollView => Role::ScrollArea,
            WidgetKind::List => Role::List,
            WidgetKind::Text => Role::StaticText,
            WidgetKind::Button => Role::Button,
            WidgetKind::TextInput => Role::TextField,
            WidgetKind::Checkbox => Role::Checkbox,
            WidgetKind::Switch => Role::Switch,
            WidgetKind::Select => Role::ComboBox,
            WidgetKind::Slider => Role::Slider,
            WidgetKind::Progress => Role::ProgressBar,
            WidgetKind::Custom(_) | WidgetKind::Native => Role::Group,
        });
        if role == Role::None {
            return children;
        }
        let props = &node.props;
        let name =
            a11y.label.clone().or_else(|| a11y.labelled_by.and_then(|l| self.text_of(l))).or_else(|| match node.kind {
                WidgetKind::Window => crate::find_prop!(props, Title),
                WidgetKind::Text => crate::find_prop!(props, Text),
                WidgetKind::Button
                | WidgetKind::Checkbox
                | WidgetKind::Switch
                | WidgetKind::Select
                | WidgetKind::Slider
                | WidgetKind::Progress => crate::find_prop!(props, Label),
                WidgetKind::TextInput => crate::find_prop!(props, Placeholder),
                // Rows read as their text, as screen readers read native rows.
                WidgetKind::Container if row.is_some() => {
                    let texts: Vec<&str> = children
                        .iter()
                        .flat_map(|c| c.walk())
                        .filter(|n| n.role == Role::StaticText)
                        .filter_map(|n| n.name.as_deref())
                        .collect();
                    (!texts.is_empty()).then(|| texts.join(" "))
                }
                _ => None,
            });
        let selected = row.map(|key| {
            let list = node.native_parent.and_then(|p| self.nodes.get(&p));
            list.and_then(|l| crate::find_prop!(l.props, Selected)).is_some_and(|s| s.contains(&key))
        });
        vec![A11yNode {
            id,
            role,
            name,
            description: a11y.description,
            value: match node.kind {
                WidgetKind::TextInput => Some(crate::find_prop!(props, Value).unwrap_or_default()),
                // The chosen option; empty without options.
                WidgetKind::Select => {
                    let options = crate::find_prop!(props, Options).unwrap_or_default();
                    let chosen = crate::find_prop!(props, SelectedIndex).flatten();
                    Some(chosen.and_then(|i| options.get(i).cloned()).unwrap_or_default())
                }
                WidgetKind::Slider => crate::find_prop!(props, Number).map(|n| n.to_string()),
                // As screen readers read progress bars.
                WidgetKind::Progress => {
                    crate::find_prop!(props, Progress).flatten().map(|f| format!("{}%", (f * 100.0).round()))
                }
                _ => a11y.value,
            },
            checked: match node.kind {
                WidgetKind::Checkbox | WidgetKind::Switch => Some(crate::find_prop!(props, Checked).unwrap_or(false)),
                _ => None,
            },
            selected,
            enabled: crate::find_prop!(props, Enabled).unwrap_or(true),
            test_id: node.test_id.clone(),
            frame,
            children,
        }]
    }
}

/// Where to scroll along one axis so `len` at `start` shows, moving as
/// little as possible from `current` with `visible` in view.
fn fit(start: f32, len: f32, current: f32, visible: f32) -> f32 {
    if start < current {
        start
    } else if start + len > current + visible {
        (start + len - visible).min(start)
    } else {
        current
    }
}

fn space(s: taffy::AvailableSpace) -> AvailableSpace {
    match s {
        taffy::AvailableSpace::Definite(v) => AvailableSpace::Definite(v),
        taffy::AvailableSpace::MinContent => AvailableSpace::MinContent,
        taffy::AvailableSpace::MaxContent => AvailableSpace::MaxContent,
    }
}
