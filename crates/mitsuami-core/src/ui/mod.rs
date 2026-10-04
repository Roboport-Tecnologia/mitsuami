//! The retained node tree and the update pipeline.
//!
//! The core tree is the source of truth. Backends mirror its *native* part:
//! every node except fragments, which are spliced into their nearest native
//! ancestor. Changes queue [`Command`]s; [`Ui::commit`] resolves styles, syncs
//! native children, applies the batch, runs layout and sends the new frames.

mod accessibility;
mod build;
mod focus;
mod layout;
mod pipeline;
mod queries;
mod scroll;
mod services;
mod styles;
mod tree;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

use taffy::TaffyTree;

use crate::a11y::A11yProps;
use crate::app_info::AppInfo;
use crate::backend::{Backend, EventSink, PlatformMetrics};
use crate::command::{Command, UiEvent};
use crate::custom::CustomProps;
use crate::geometry::{Insets, Point, Rect, Size};
use crate::services::Services;
use crate::style::Style;
use crate::task::{Clock, Executor, Sleep, TaskHandle};
use crate::widget::{NodeId, Prop, WidgetKind};

/// A window's toolbar items and sidebar are the platform's to place, around
/// its content: they aren't in the window's layout box.
fn in_chrome(kind: WidgetKind) -> bool {
    matches!(kind, WidgetKind::ToolbarItem | WidgetKind::Sidebar)
}

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
    /// Windows only: how the height follows the content.
    fit: Fit,
    /// Windows following their content only: the heights the core gave the
    /// window, oldest first: the last the platform reported, and the ones
    /// since that it hasn't yet.
    heights: Vec<f32>,
    /// ScrollViews, Lists and Tables only: how far the content is scrolled.
    scroll_offset: Point,
    /// Lists only: the width the platform gives their rows, if not their own.
    row_width: Option<f32>,
    /// ScrollViews only: where the platform's viewport is in the frame.
    viewport_insets: Insets,
    /// Tables only: the widths the platform gives their columns' cells.
    column_widths: Option<Vec<f32>>,
    /// Tabs and groups only: the size of their tab strip, or heading, and
    /// border, with no content.
    strip: Size,
    /// Groups only: where the backend puts their content, if not where
    /// the metrics say.
    insets: Option<crate::Insets>,
    /// Drawn custom widgets only: the size their drawing was made at, until
    /// their props or the metrics change.
    drawn_at: Option<Size>,
}

/// How a window's height follows its content (`WindowSize`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fit {
    None,
    /// At the first layout.
    Once,
    Follow {
        until_resized: bool,
    },
}

impl Node {
    fn in_full_screen(&self) -> bool {
        crate::find_prop!(self.props, FullScreen).unwrap_or(false)
    }

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
    /// The nodes whose `Create` is in `pending`, so props set on the others
    /// don't look for theirs.
    created: std::collections::HashSet<NodeId>,
    /// Focus requests, sent after the structure of the batch: a node focused
    /// right after it's built isn't in a window yet.
    pending_focus: Vec<NodeId>,
    /// Text selections asked for, sent after the focus requests.
    pending_selections: Vec<(NodeId, std::ops::Range<usize>)>,
    windows: Vec<NodeId>,
    styles_dirty: bool,
    resync: BTreeSet<NodeId>,
    /// Each menu bar's item handlers: the app's (`None`) and windows'.
    menu_handlers: BTreeMap<Option<NodeId>, std::collections::HashMap<u32, Handler0>>,
    /// The effect keeping each menu bar up to date.
    menu_effects: BTreeMap<Option<NodeId>, mitsuami_reactive::Effect>,
    /// Menu item ids are unique across every bar.
    next_menu_id: u32,
    menu_queue: std::collections::VecDeque<u32>,
    /// The app's Quit item in each menu bar (the app's, `None`, or a
    /// window's), if it has one that's enabled.
    quit_items: BTreeMap<Option<NodeId>, u32>,
    /// Last focus order sent, per window.
    focus_orders: BTreeMap<NodeId, Vec<NodeId>>,
    /// Whether a tab index, or what a page shown or a surface taking input
    /// changed since. The Tab order changes only with them, the tree and
    /// styles, so other commits don't walk every window for it.
    focus_dirty: bool,
    /// The focused control of each window, as reported by the backend.
    focused: BTreeMap<NodeId, NodeId>,
    commit_scheduler: Option<Rc<dyn Fn()>>,
    commit_scheduled: bool,
    /// The app's language last given to the backend.
    language: Option<crate::l10n::LanguageIdentifier>,
    /// Its direction, which windows' content inherits.
    rtl: bool,
    /// Sizes views watch (`use_size`, `use_viewport`).
    observers: BTreeMap<u64, Observer>,
    next_observer: u64,
}

/// A size a view watches: the node's it targets now, and the last reported.
struct Observer {
    target: Box<dyn Fn() -> Option<NodeId>>,
    size: mitsuami_reactive::Signal<Size>,
    last: Size,
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
    /// Kept apart too: messages are formatted while the tree is borrowed
    /// (backends title menus in `apply`).
    l10n: Rc<crate::l10n::Localization>,
}

/// Non-owning [`Ui`] handle, for closures stored inside the tree itself.
#[derive(Clone)]
pub struct WeakUi {
    inner: Weak<RefCell<Inner>>,
    executor: Weak<Executor>,
    services: Weak<RefCell<Box<dyn Services>>>,
    l10n: Weak<crate::l10n::Localization>,
}

impl WeakUi {
    pub fn upgrade(&self) -> Option<Ui> {
        Some(Ui {
            inner: self.inner.upgrade()?,
            executor: self.executor.upgrade()?,
            services: self.services.upgrade()?,
            l10n: self.l10n.upgrade()?,
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
        let l10n = crate::l10n::Localization::new(backend.locale());
        let ui = Ui {
            executor: Rc::default(),
            services,
            l10n,
            inner: Rc::new(RefCell::new(Inner {
                backend,
                events,
                metrics,
                nodes: BTreeMap::new(),
                taffy: TaffyTree::new(),
                next_id: 1,
                pending: Vec::new(),
                created: Default::default(),
                pending_focus: Vec::new(),
                pending_selections: Vec::new(),
                windows: Vec::new(),
                styles_dirty: true,
                resync: BTreeSet::new(),
                focus_orders: BTreeMap::new(),
                focus_dirty: true,
                focused: BTreeMap::new(),
                menu_handlers: BTreeMap::new(),
                menu_effects: BTreeMap::new(),
                next_menu_id: 1,
                menu_queue: Default::default(),
                quit_items: BTreeMap::new(),
                commit_scheduler: None,
                commit_scheduled: false,
                language: None,
                rtl: false,
                observers: BTreeMap::new(),
                next_observer: 1,
            })),
        };
        // The next commit gives the backend the new language.
        let weak = ui.downgrade();
        ui.l10n.set_on_change(move |_, _| {
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        ui
    }

    pub fn downgrade(&self) -> WeakUi {
        WeakUi {
            inner: Rc::downgrade(&self.inner),
            executor: Rc::downgrade(&self.executor),
            services: Rc::downgrade(&self.services),
            l10n: Rc::downgrade(&self.l10n),
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

    /// The app's id, name and icon, for the platform to show. Set it
    /// before the app's first window; `App` does.
    pub fn set_app_info(&self, info: AppInfo) {
        self.inner.borrow_mut().backend.set_app_info(&info);
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

    // --------------------------------------------------------- localization

    /// The app's translations (`locales!`). Set them before the app's
    /// first window; `App` does. The language is chosen from them again.
    pub fn set_locales(&self, locales: crate::l10n::Locales) {
        self.l10n.set_locales(locales);
    }

    /// The languages to choose the app's from, most wanted first, in place
    /// of the user's: an app's own language setting, or a test's. `None`:
    /// the user's, as the platform lists them.
    pub fn set_languages(&self, languages: Option<Vec<String>>) {
        self.l10n.set_requested(languages);
    }

    /// The app's language: the first of the user's it has translations
    /// for, else its fallback. Tracked.
    pub fn language(&self) -> crate::l10n::LanguageIdentifier {
        self.l10n.current()
    }

    /// Messages missing from every language, and messages that failed to
    /// format, since the last call. Tests fail on them.
    pub fn take_l10n_errors(&self) -> Vec<String> {
        self.l10n.take_errors()
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
}
