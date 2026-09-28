//! The contract every platform backend implements.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::a11y::{A11yAction, ActionError};
use crate::app_info::{AppInfo, NativeAppInfo};
use crate::command::{Command, UiEvent};
use crate::geometry::{Insets, Point, Rect, Size};
use crate::units::SpacingScale;
use crate::widget::{NodeId, Prop, TextStyle, WidgetKind};

/// Space available on one axis while measuring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvailableSpace {
    Definite(f32),
    MinContent,
    MaxContent,
}

/// What the layout engine asks when it measures a leaf widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureRequest {
    /// Sizes that are already fixed; measure the other axis given these.
    pub known_width: Option<f32>,
    pub known_height: Option<f32>,
    pub available_width: AvailableSpace,
    pub available_height: AvailableSpace,
}

/// Platform facts the core needs for unit resolution and defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct PlatformMetrics {
    pub scale_factor: f32,
    pub spacing: SpacingScale,
    pub font_sizes: FontSizes,
    pub dark_mode: bool,
    pub high_contrast: bool,
    pub reduced_motion: bool,
    /// Where a `Tabs` shows its pages: this far in from its edges, past
    /// its tab strip and border.
    pub tab_insets: Insets,
}

/// Light or dark. Backends can force one (tests do, so captures don't
/// depend on the system setting).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontSizes {
    pub large_title: f32,
    pub title: f32,
    pub headline: f32,
    pub body: f32,
    pub callout: f32,
    pub caption: f32,
    pub monospace: f32,
}

impl FontSizes {
    pub fn get(&self, style: TextStyle) -> f32 {
        match style {
            TextStyle::LargeTitle => self.large_title,
            TextStyle::Title => self.title,
            TextStyle::Headline => self.headline,
            TextStyle::Body => self.body,
            TextStyle::Callout => self.callout,
            TextStyle::Caption => self.caption,
            TextStyle::Monospace => self.monospace,
        }
    }
}

/// Raw input for [`Backend::synthesize`].
#[derive(Clone, Debug, PartialEq)]
pub enum SyntheticInput {
    Key(Key),
    /// Scroll-wheel / trackpad scroll over a `ScrollView` or `List`, in logical units
    /// (positive = towards the end of the content).
    Scroll {
        dx: f32,
        dy: f32,
    },
    /// A primary-button click (down, then up) at this point, in the node's
    /// coordinates. Backends support it on drawn custom widgets, whose
    /// pointer handling is ours.
    Click(Point),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Up,
    Down,
    Home,
    End,
}

/// What a native widget actually shows, read back from the platform.
/// Tests compare this with the core's state to catch backend desyncs.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeState {
    pub kind: WidgetKind,
    pub props: Vec<Prop>,
    pub frame: Rect,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// Has keyboard focus (for text fields: is being edited).
    pub focused: bool,
    /// `ScrollView`s and `List`s only: the current scroll offset.
    pub scroll_offset: Option<Point>,
}

/// An RGBA8 screenshot in physical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    UnknownNode,
    Unsupported,
    Failed(String),
}

/// How backends report native events. Events are queued; the [`Ui`](crate::Ui)
/// drains them when the backend calls [`Ui::process_events`](crate::Ui::process_events)
/// (or right after a [`Backend::perform`]).
#[derive(Clone, Default)]
pub struct EventSink {
    queue: Rc<RefCell<VecDeque<(NodeId, UiEvent)>>>,
}

impl EventSink {
    pub fn emit(&self, id: NodeId, event: UiEvent) {
        self.queue.borrow_mut().push_back((id, event));
    }

    pub(crate) fn pop(&self) -> Option<(NodeId, UiEvent)> {
        self.queue.borrow_mut().pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.borrow().is_empty()
    }
}

/// What the test harness (`mitsuami-test`) needs from a backend beyond the
/// [`Backend`] contract. Implemented by each backend's shareable handle.
pub trait TestHooks {
    /// Short name used in snapshot and visual baseline paths: `"appkit"`,
    /// `"gtk"`, `"winui"`.
    fn name(&self) -> &'static str;
    /// Resizes a window's content area the way the user would, so the
    /// platform reports it back as `WindowResized`.
    fn resize_window(&self, window: NodeId, size: Size);
    /// Clicks the window's close button, the way the user would, so the
    /// platform reports `WindowCloseRequested`. Whether it closes is the
    /// app's call.
    fn close_window(&self, window: NodeId);
    /// Commands applied since the last call (record them when asked to).
    fn take_command_log(&self) -> Vec<Command>;
    /// Live native nodes, as a leak detector.
    fn node_count(&self) -> usize;
    /// What this window shows of the app's id, name and icon (see
    /// [`AppInfo`]), read from the platform.
    fn app_info(&self, window: NodeId) -> NativeAppInfo;
    /// Called after every settle: lets the platform catch up on work it
    /// does asynchronously (showing windows, allocating, delivering queued
    /// notifications) without waiting. Platforms that do it all
    /// synchronously, like AppKit, need nothing here.
    fn settle(&self) {}
}

pub trait Backend {
    /// Called once when the backend is attached to a [`Ui`](crate::Ui).
    fn init(&mut self, events: EventSink);

    fn metrics(&self) -> PlatformMetrics;

    /// Applies a batch of commands to the native tree.
    fn apply(&mut self, batch: &[Command]);

    /// Intrinsic size of a leaf widget. Called synchronously during layout,
    /// after all commands of the current batch have been applied.
    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size;

    /// Performs an accessibility action on the native control, as assistive
    /// technology would. Resulting events go through the [`EventSink`].
    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError>;

    /// Synthesizes raw user input on a native control, as close to real input
    /// as the platform allows. Used by end-to-end tests.
    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError>;

    fn native_state(&self, id: NodeId) -> Option<NativeState>;

    /// Offscreen screenshot of a window or node. Async because some
    /// platforms render asynchronously (WinUI's `RenderTargetBitmap`);
    /// reply whenever it's ready, right away if possible.
    fn capture(&mut self, id: NodeId, reply: crate::services::Reply<Result<Image, CaptureError>>);

    /// The platform's clipboard, dialogs and menus. Called once, when the
    /// backend is attached; tests may replace the result.
    fn services(&self) -> Box<dyn crate::services::Services>;

    /// The app's id, name and icon, set before the app's first window, and
    /// perhaps again later (tests). Apply what the platform has a place
    /// for, to the windows there are and those to come.
    fn set_app_info(&mut self, info: &AppInfo);
}
