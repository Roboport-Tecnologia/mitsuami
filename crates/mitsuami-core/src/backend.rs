//! The contract every platform backend implements.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::a11y::{A11yAction, ActionError};
use crate::app_info::{AppInfo, NativeAppInfo};
use crate::command::{Command, UiEvent};
use crate::geometry::{Insets, Point, Rect, Size};
use crate::services::Shortcut;
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
    /// Where a `Group` shows its content: this far in from its edges,
    /// past its border and the platform's margins; `titled_group_insets`
    /// with a heading, which is inside the box on some platforms and above
    /// it on others.
    pub group_insets: Insets,
    pub titled_group_insets: Insets,
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
    /// A key pressed and released, on its own.
    Key(Key),
    /// A key pressed and released with modifiers held: what a node's keys
    /// (`Prop::Keys`) take, where the focused control doesn't use it.
    Shortcut(Shortcut),
    /// Scroll-wheel / trackpad scroll over a `ScrollView` or `List`, in logical units
    /// (positive = towards the end of the content).
    Scroll { dx: f32, dy: f32 },
    /// A primary-button click (down, then up) at this point, in the node's
    /// coordinates. Backends support it on drawn custom widgets, whose
    /// pointer handling is ours.
    Click(Point),
    /// Files dragged from the file manager, over a node with a
    /// `Prop::FileDrop`: what the platform's drag handling does when they
    /// enter it, through the same path.
    DragFiles(Vec<std::path::PathBuf>),
    /// The drag leaves the node without dropping.
    DragLeave,
    /// The files are dropped on the node (having entered it first, as a
    /// real drop does).
    DropFiles(Vec<std::path::PathBuf>),
}

/// A key, by what it types or does: a character (`Char(' ')` is the
/// space bar), or a key that types none. For shortcuts, key bindings and
/// synthesized input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    /// The key that deletes backwards: Delete (⌫) on Apple keyboards.
    Backspace,
    /// The key that deletes forwards: ⌦ on Apple keyboards.
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// A function key, from F1.
    F(u8),
}

impl From<char> for Key {
    fn from(c: char) -> Key {
        Key::Char(c)
    }
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
    /// A focused text field's or text area's selection, in characters (an
    /// empty one is the caret); `None` for anything else.
    pub selection: Option<std::ops::Range<usize>>,
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
    /// `"gtk"`, `"winui"`, `"kirigami"`.
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
    /// The files dragging this row out of its `List` or `Table` carries
    /// (`Prop::RowFiles`), as the platform's drag source gives them: the
    /// selected rows' when the row is selected, else its own. `node` is the
    /// row's host (a table's: any of its cells'). `None` if it doesn't drag.
    /// Nothing here can start a real drag; this goes through the backend's
    /// own drag handling up to where the platform takes the files.
    fn dragged_files(&self, node: NodeId) -> Option<Vec<std::path::PathBuf>>;
    /// Called after every settle: lets the platform catch up on work it
    /// does asynchronously (showing windows, allocating, delivering queued
    /// notifications) without waiting. Every backend has some: AppKit lays
    /// out tables and toolbars, which make their views in a layout pass.
    fn settle(&self) {}
}

pub trait Backend {
    /// Called once when the backend is attached to a [`Ui`](crate::Ui).
    fn init(&mut self, events: EventSink);

    fn metrics(&self) -> PlatformMetrics;

    /// Where this `Group` puts its content, if not where the metrics'
    /// `group_insets` or `titled_group_insets` say: a tweak can move its
    /// heading or change its border. Asked when the group is measured.
    fn group_insets(&self, _id: NodeId) -> Option<Insets> {
        None
    }

    /// Where this `Tabs` puts its pages, if not where the metrics'
    /// `tab_insets` say: its style can give it another strip. Asked when
    /// its strip is measured.
    fn tab_insets(&self, _id: NodeId) -> Option<Insets> {
        None
    }

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

    /// The user's languages, and how the user's region writes numbers
    /// and dates. Called once, when the backend is attached.
    fn locale(&self) -> std::rc::Rc<dyn crate::l10n::PlatformLocale>;

    /// The app's language, which the core chose from the user's, and
    /// whether it's written right to left: set before the app's first
    /// window, and again when the app changes it. Make the toolkit's own
    /// chrome follow it where the platform lets an app choose (the
    /// direction of title bars, menus and dialogs). Native widgets get
    /// their direction as `Prop::LayoutDirection`.
    fn set_locale(&mut self, language: &crate::l10n::LanguageIdentifier, right_to_left: bool);
}
