//! mitsuami core: the retained node tree, styles and units, layout,
//! accessibility model, control flow and the backend contract.
//!
//! See `docs/ARCHITECTURE.md` for the design.

pub mod a11y;
mod any_value;
pub mod backend;
pub mod command;
mod custom;
pub mod draw;
mod element;
mod flow;
pub mod geometry;
mod input;
mod list;
mod resource;
pub mod services;
mod store;
pub mod style;
mod surface;
pub mod task;
mod tweak;
mod ui;
pub mod units;
mod view;
mod widget;

pub use a11y::{A11yAction, A11yNode, A11yProps, ActionError, Role};
pub use any_value::{AnyValue, Opaque};
pub use backend::{
    Appearance, AvailableSpace, Backend, EventSink, Key, MeasureRequest, NativeState, PlatformMetrics, SyntheticInput,
    TestHooks,
};
pub use command::{Command, EventValue, PointerEvent, PointerKind, UiEvent};
pub use custom::{Composed, Custom, CustomProps, CustomView, CustomWidget, Drawn, Render, Renderer};
pub use draw::{Canvas, Color, DisplayList, Path, Shape};
pub use element::{Element, ElementBuilder};
pub use flow::{For, Show};
pub use geometry::{Point, Rect, Size, WindowSize};
pub use input::{KeyCode, Modifiers, MouseButton, ScrollDelta, SurfaceInput};
pub use list::{List, ListHandle, RowRender};
pub use resource::{Action, Resource, action, resource, resource_on};
pub use store::{Store, provide_stores, use_store};
pub use style::{Align, Display, FlexDirection, GridPlacement, Justify, Style, TextDirection, Track, repeat};
pub use surface::{NativeSurface, SurfaceHandle, SurfaceSize};
pub use tweak::Tweak;
pub use ui::{NodeInfo, Ui, WeakUi};
pub use units::{Length, LengthExt, Spacing};
pub use view::{AnyView, Callback, Children, Slot, View};
pub use widget::{
    ButtonRole, ButtonStyle, CurrentWindow, ImageFit, ImageSource, ListStyle, Modality, NodeId, Orientation, Pixels,
    Prop, RowKey, ScrollAxes, SelectionMode, TextStyle, WidgetKind,
};

pub use mitsuami_reactive as reactive;
pub use raw_window_handle;
