//! The data flowing between the core and a backend.

use crate::a11y::A11yProps;
use crate::any_value::AnyValue;
use crate::geometry::{Point, Rect, Size};
use crate::input::SurfaceInput;
use crate::services::Shortcut;
use crate::surface::{SurfaceHandle, SurfaceSize};
use crate::widget::{ColumnSort, NodeId, Prop, RowKey, WidgetKind};

/// A change the backend must apply to the native widget tree.
///
/// Commands arrive in batches. Within a batch, order matters: a node is
/// always created before it is inserted, and a subtree root is removed before
/// it is destroyed.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Native views start with a zero frame; the core only sends frames
    /// that differ from the last one it sent.
    Create {
        id: NodeId,
        kind: WidgetKind,
        props: Vec<Prop>,
    },
    SetProp {
        id: NodeId,
        prop: Prop,
    },
    Insert {
        parent: NodeId,
        child: NodeId,
        index: usize,
    },
    Remove {
        parent: NodeId,
        child: NodeId,
    },
    /// Frees a node. Sent for every native node of a destroyed subtree,
    /// children first. The subtree's root has already been removed from its
    /// parent; nodes inside it may still be attached to each other.
    Destroy {
        id: NodeId,
    },
    /// Parent-relative, logical units. Never sent for windows.
    SetFrame {
        id: NodeId,
        frame: Rect,
    },
    SetA11y {
        id: NodeId,
        a11y: A11yProps,
    },
    /// Window content size requested by the app. The platform may refuse it
    /// (a window in full screen keeps the screen's), or give it no smaller
    /// than the window's `MinSize`; it reports the size it gives as
    /// `WindowResized`.
    SetWindowSize {
        id: NodeId,
        size: Size,
    },
    /// The keyboard (Tab) order of a window's focusable controls. Sent when
    /// it changes. Backends chain focus in this order; which controls can
    /// actually take focus stays a platform decision (e.g. macOS keyboard
    /// navigation settings).
    SetFocusOrder {
        window: NodeId,
        order: Vec<NodeId>,
    },
    /// Scrolls a `ScrollView` or `List` so `offset` (content coordinates) is at its
    /// top-left. Already clamped by the core. Like user scrolling, it makes
    /// the backend report `Scrolled`.
    ScrollTo {
        id: NodeId,
        offset: Point,
    },
    Focus {
        id: NodeId,
    },
    /// Selects these characters of a text field's or text area's text,
    /// counted in Unicode scalar values, within it and on grapheme
    /// boundaries (the app counts graphemes). A `Focus` for the
    /// field comes first in the batch: a selection is the focused field's.
    SelectText {
        id: NodeId,
        range: std::ops::Range<usize>,
    },
    /// Scrolls a `List` just enough to show a row, as the platform's own
    /// "scroll to row" does. The platform then reports `Scrolled`, and the
    /// rows it shows.
    ScrollToRow {
        id: NodeId,
        row: RowKey,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventValue {
    Text(String),
    Bool(bool),
    /// A `Slider`'s or `NumberInput`'s new value.
    Number(f64),
    /// A `List`'s or `Table`'s selected rows.
    Rows(Vec<RowKey>),
    /// The column a `Table` is sorted by now, and which way.
    Sort(ColumnSort),
    /// The option chosen in a `Select` or `RadioGroup`.
    Index(usize),
}

/// Something that happened in the native UI.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    Click,
    /// The user changed a control's value. The native widget already shows it.
    Changed(EventValue),
    Submit,
    /// A `SearchInput` asks for a search for its text, when the platform
    /// does: as the user types (after a pause, where the platform waits for
    /// one), on Return, and when the field is cleared. Not for text the
    /// core set.
    Search(String),
    FocusIn,
    FocusOut,
    WindowResized(Size),
    WindowCloseRequested,
    /// The user put a window in full screen, or took it out (the title
    /// bar's button, the window manager's key), or the platform did. Not
    /// reported for the app's own `FullScreen`. The core absorbs it as
    /// `FullScreen`.
    FullScreenChanged(bool),
    /// The user maximized a window or restored it (the title bar's button,
    /// a double-click on it, the window manager's key), or the platform
    /// did. Not reported for the app's own `Maximized`. The core absorbs
    /// it as `Maximized`.
    MaximizedChanged(bool),
    /// The user showed or hid a `Sidebar` (the platform's own toggle, its
    /// divider dragged away), or the platform did. Not reported for the
    /// app's own `SidebarShown`. The core absorbs it as `SidebarShown`.
    SidebarShownChanged(bool),
    /// Platform metrics changed (text size, color scheme, …).
    MetricsChanged,
    /// A `ScrollView`'s or `List`'s scroll offset changed (by the user or
    /// by `ScrollTo`).
    Scrolled(Point),
    /// A `List` or `Table` realised a row: it's in view, or about to be.
    /// The core mounts the row and sends its host (a table's: its cells').
    RowShown(RowKey),
    /// A `List` or `Table` let go of a row it had shown. The core disposes it.
    RowHidden(RowKey),
    /// A `List`'s row was activated: double-clicked, or Enter pressed on it.
    RowActivated(RowKey),
    /// The width a `List` gives its rows, when it isn't the list's own
    /// width (scroll bars that take room from the rows, list insets). Rows
    /// are laid out at that width.
    RowWidth(f32),
    /// The widths a `Table`'s columns give their cells, in column order:
    /// once they're known, and whenever they change (the user resized a
    /// column, the table was resized). Cells are laid out at them.
    ColumnWidths(Vec<f32>),
    /// A pointer event on a drawn custom widget, in its coordinates.
    Pointer(PointerEvent),
    /// A widget's natural size changed on its own (an image finished
    /// loading); the core measures it again.
    Remeasure,
    /// An event of a custom widget or native view, of its own type.
    Custom(AnyValue),
    /// The item of the node's context menu with this id was chosen.
    ContextMenuItem(u32),
    /// The item of a `MenuButton`'s menu with this id was chosen.
    MenuItem(u32),
    /// A key the node takes (`Prop::Keys`) was pressed while it, or a
    /// control inside it that doesn't use the key, had keyboard focus.
    Key(Shortcut),
    /// Files a node takes (`Prop::FileDrop`) are being dragged over it
    /// (`true`), or no longer are: they left, or were dropped.
    DropHover(bool),
    /// Files and folders were dropped on a node: the ones it takes, in
    /// the order the platform gave them.
    FilesDropped(Vec<std::path::PathBuf>),
    /// A `GpuSurface`'s native surface exists: the app can make its GPU
    /// surface on it. Reported once, before any `SurfaceResized`; its size
    /// may still be empty.
    SurfaceReady(SurfaceHandle),
    /// A `GpuSurface`'s size in pixels, or its scale, changed.
    SurfaceResized(SurfaceSize),
    /// Keys or the pointer on a `GpuSurface` that takes input.
    SurfaceInput(SurfaceInput),
    /// The platform ended a `GpuSurface`'s pointer lock, or couldn't make
    /// it: its window stopped being the active one. The core absorbs it
    /// as `PointerLock(false)`.
    PointerLockEnded,
    /// The platform ended a `GpuSurface`'s keyboard grab, or couldn't make
    /// it: the surface lost focus, or its window stopped being the active
    /// one. The core absorbs it as `KeyboardGrab(false)`.
    KeyboardGrabEnded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Down,
    Up,
}

/// A primary-button pointer event. Position in the node's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub position: Point,
}
