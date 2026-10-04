//! An in-memory backend for integration tests.
//!
//! It keeps a mirror of the native tree like a real backend would, validates
//! every command against the protocol (panicking on violations), and measures
//! text with fixed, platform-independent metrics so layouts are deterministic:
//! each character is `0.5em` wide and lines are `1.25em` tall.

mod apply;
mod handle;
mod input;
mod lists;
mod measure;
mod metrics;
mod native_state;
mod perform;
mod services;
mod state;
mod windows;

pub use metrics::metrics;
pub use services::{
    FakeServices, FakeServicesHandle, Pending, PendingAlert, PendingLaunch, PendingOpen, PendingSave, PendingTrash,
};

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, A11yProps, ActionError};
use mitsuami_core::backend::{
    Backend, CaptureError, EventSink, Image, MeasureRequest, NativeState, PlatformMetrics, SyntheticInput,
};
use mitsuami_core::l10n::{DateTimeFormat, LanguageIdentifier, NumberFormat, PlatformLocale};
use mitsuami_core::raw_window_handle::{HandleError, RawDisplayHandle, RawWindowHandle};
use mitsuami_core::{
    AppInfo, Command, Insets, NativeSurface, NodeId, Point, Prop, Rect, RowKey, Size, SurfaceHandle, WidgetKind,
};

struct HeadlessNode {
    kind: WidgetKind,
    props: Vec<Prop>,
    a11y: A11yProps,
    frame: Rect,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    scroll_offset: Point,
    /// Scroll views only: the room their scroll bars take, as a test set it
    /// (`HeadlessHandle::set_viewport_insets`). None otherwise: headless
    /// scroll bars overlay the content.
    viewport_insets: Insets,
    /// Lists only: the rows realised, as reported.
    shown: BTreeSet<RowKey>,
    /// Lists only: where the rows are, and each row's index in it.
    placed: Vec<Placed>,
    placed_index: std::collections::HashMap<RowKey, usize>,
    /// Lists only: the heights of the rows measured so far, kept when
    /// they're let go, as native lists keep them.
    heights: BTreeMap<RowKey, f32>,
    /// Tables only: the widths their columns give their cells, as reported.
    column_widths: Vec<f32>,
    /// GPU surfaces only: what the app got.
    surface: Option<SurfaceHandle>,
    /// Windows in full screen only: their size before, to go back to.
    windowed: Option<Size>,
    /// Maximized windows only: their size before, to go back to.
    restored: Option<Size>,
}

/// A user who reads US English, and numbers and dates written as
/// mitsuami writes them (US English, dates in UTC), whatever the machine's
/// settings: tests ask for other languages with `Ui::set_languages`.
struct HeadlessLocale;

impl PlatformLocale for HeadlessLocale {
    fn languages(&self) -> Vec<String> {
        vec!["en-US".into()]
    }

    fn format_number(&self, _: f64, _: &NumberFormat) -> Option<String> {
        None
    }

    fn format_date_time(&self, _: std::time::SystemTime, _: &DateTimeFormat) -> Option<String> {
        None
    }
}

/// A `GpuSurface` with nothing to present to: the app gets it, and its
/// sizes, but no window handle.
struct HeadlessSurface;

// SAFETY: it returns no handles.
unsafe impl NativeSurface for HeadlessSurface {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError> {
        Err(HandleError::NotSupported)
    }

    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError> {
        Err(HandleError::NotSupported)
    }
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
    /// Whether commands go into `log`: only once a test asks for them
    /// (`HeadlessHandle::record_commands`), so it doesn't grow without end.
    recording: bool,
    log: Vec<Command>,
    focused: Option<NodeId>,
    /// The focused text field's selection, in characters; `None` is the
    /// caret at the end, where focusing puts it.
    selection: Option<std::ops::Range<usize>>,
    focus_orders: BTreeMap<NodeId, Vec<NodeId>>,
    app: AppInfo,
    /// The node files being dragged are over, if it takes them.
    drop_hover: Option<NodeId>,
    /// The app's language and whether it's right to left, as the core set it.
    locale: Option<(LanguageIdentifier, bool)>,
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
                recording: false,
                log: Vec::new(),
                focused: None,
                selection: None,
                drop_hover: None,
                focus_orders: BTreeMap::new(),
                app: AppInfo::default(),
                locale: None,
            })),
        }
    }

    pub fn handle(&self) -> HeadlessHandle {
        HeadlessHandle { state: self.state.clone() }
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
        self.apply_batch(batch);
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        self.measure_node(id, request)
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        self.perform_action(id, action)
    }

    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        self.synthesize_input(id, input)
    }

    fn native_state(&self, id: NodeId) -> Option<NativeState> {
        self.read_native_state(id)
    }

    fn locale(&self) -> Rc<dyn PlatformLocale> {
        Rc::new(HeadlessLocale)
    }

    fn set_locale(&mut self, language: &LanguageIdentifier, right_to_left: bool) {
        self.state.borrow_mut().locale = Some((language.clone(), right_to_left));
    }

    fn capture(&mut self, _id: NodeId, reply: mitsuami_core::services::Reply<Result<Image, CaptureError>>) {
        reply(Err(CaptureError::Unsupported));
    }

    fn services(&self) -> Box<dyn mitsuami_core::services::Services> {
        Box::new(FakeServices::default())
    }

    fn set_app_info(&mut self, info: &AppInfo) {
        self.state.borrow_mut().app = info.clone();
    }
}
