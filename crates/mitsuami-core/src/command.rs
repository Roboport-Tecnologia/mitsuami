//! The data flowing between the core and a backend.

use crate::a11y::A11yProps;
use crate::any_value::AnyValue;
use crate::geometry::{Point, Rect, Size};
use crate::widget::{NodeId, Prop, RowKey, WidgetKind};

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
    /// Window content size requested by the app. The platform may refuse it.
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventValue {
    Text(String),
    Bool(bool),
    Number(f64),
    /// A `List`'s selected rows.
    Rows(Vec<RowKey>),
}

/// Something that happened in the native UI.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    Click,
    /// The user changed a control's value. The native widget already shows it.
    Changed(EventValue),
    Submit,
    FocusIn,
    FocusOut,
    WindowResized(Size),
    WindowCloseRequested,
    /// Platform metrics changed (text size, color scheme, …).
    MetricsChanged,
    /// A `ScrollView`'s or `List`'s scroll offset changed (by the user or
    /// by `ScrollTo`).
    Scrolled(Point),
    /// A `List`'s row was activated: double-clicked, or Enter pressed on it.
    RowActivated(RowKey),
    /// The width a `List` gives its rows, when it isn't the list's own
    /// width (scroll bars that take room from the rows, list insets).
    RowWidth(f32),
    /// A pointer event on a drawn custom widget, in its coordinates.
    Pointer(PointerEvent),
    /// An event of a custom widget or native view, of its own type.
    Custom(AnyValue),
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
