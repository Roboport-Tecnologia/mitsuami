//! What the core tells backends to create, and the properties they carry.

use std::fmt;

use crate::any_value::Opaque;
use crate::custom::CustomProps;
use crate::draw::DisplayList;

/// Stable identity of a node for the lifetime of a [`Ui`](crate::Ui).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    pub fn raw(self) -> u32 {
        self.0
    }

    /// For backends and tools that need to rebuild ids, e.g. when replaying a
    /// command log.
    pub fn from_raw(raw: u32) -> NodeId {
        NodeId(raw)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    Window,
    /// A layout host: a plain native view that we position children in.
    Container,
    /// Core-only grouping used by control flow (`Show`, `For`). Never sent to
    /// backends; its children are spliced into the nearest native ancestor.
    Fragment,
    Text,
    Button,
    TextInput,
    Checkbox,
    Switch,
    /// A native pop-up menu of text options (NSPopUpButton, ComboBox,
    /// gtk::DropDown, QQC2.ComboBox). Its options are [`Prop::Options`].
    Select,
    /// A native slider (NSSlider, Slider, gtk::Scale, QQC2.Slider): a
    /// [`Prop::Number`] in a [`Prop::Range`], in [`Prop::Step`]s.
    Slider,
    /// A native progress bar (NSProgressIndicator, ProgressBar,
    /// gtk::ProgressBar, QQC2.ProgressBar): a [`Prop::Progress`].
    Progress,
    /// A native scroll container. It has exactly one native child, the
    /// content, which the core lays out and may be larger than the viewport.
    ScrollView,
    /// A native list control (NSTableView, ListView, gtk::ListView, QML
    /// ListView). Its data is [`Prop::Rows`]. The platform decides which rows
    /// exist (`RowShown`, `RowHidden`); its native children are the hosts of
    /// the rows the core mounted for those (`Container`s with a
    /// [`Prop::Row`]), in row order. It scrolls like a `ScrollView`.
    List,
    /// A custom widget (see [`CustomWidget`](crate::CustomWidget)), named
    /// after it. Its props travel as [`Prop::Custom`].
    Custom(&'static str),
    /// A raw native view supplied by app code. Its factory and updates
    /// travel as [`Prop::Native`].
    Native,
}

impl WidgetKind {
    pub fn is_native(self) -> bool {
        self != WidgetKind::Fragment
    }

    /// Containers lay out children; everything else is measured by the backend.
    pub fn is_container(self) -> bool {
        matches!(self, WidgetKind::Window | WidgetKind::Container | WidgetKind::ScrollView)
    }

    /// Scrolls its content: `ScrollTo` and `Scrolled` apply.
    pub fn scrolls(self) -> bool {
        matches!(self, WidgetKind::ScrollView | WidgetKind::List)
    }

    pub fn name(self) -> &'static str {
        match self {
            WidgetKind::Window => "Window",
            WidgetKind::Container => "Container",
            WidgetKind::ScrollView => "ScrollView",
            WidgetKind::List => "List",
            WidgetKind::Fragment => "Fragment",
            WidgetKind::Text => "Text",
            WidgetKind::Button => "Button",
            WidgetKind::TextInput => "TextInput",
            WidgetKind::Checkbox => "Checkbox",
            WidgetKind::Switch => "Switch",
            WidgetKind::Select => "Select",
            WidgetKind::Slider => "Slider",
            WidgetKind::Progress => "Progress",
            WidgetKind::Custom(name) => name,
            WidgetKind::Native => "Native",
        }
    }
}

/// Semantic text styles, mapped to each platform's type ramp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextStyle {
    LargeTitle,
    Title,
    Headline,
    #[default]
    Body,
    Callout,
    Caption,
    Monospace,
}

/// What a button does in its window or dialog, which each platform shows
/// and handles its own way. Platforms without an equivalent ignore it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonRole {
    /// An ordinary button.
    #[default]
    Normal,
    /// The action Return confirms: the default button on AppKit (Return
    /// clicks it), the suggested action on GTK, highlighted on Qt, the
    /// accent button on WinUI. One per window or dialog.
    Default,
    /// The action Escape takes: the cancel button on AppKit (Escape clicks
    /// it). The others show it as a normal button.
    Cancel,
    /// An action that destroys data: GTK's destructive action; AppKit marks
    /// it so the system can warn. Qt and WinUI have no such style.
    Destructive,
}

/// How a button is drawn, mapped to each platform's native look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonStyle {
    /// The same as `Bordered`, on every platform.
    #[default]
    Automatic,
    /// The platform's push button, with its bezel or frame.
    Bordered,
    /// No bezel or frame until hovered, where the platform does that: for
    /// toolbars, and buttons that sit among text.
    Borderless,
}

/// Scrolling directions of a `ScrollView`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollAxes {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

impl ScrollAxes {
    pub fn horizontal(self) -> bool {
        matches!(self, ScrollAxes::Horizontal | ScrollAxes::Both)
    }

    pub fn vertical(self) -> bool {
        matches!(self, ScrollAxes::Vertical | ScrollAxes::Both)
    }
}

/// Identity of a row of a `List`, stable while its item's key stays in the
/// data. Assigned by the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowKey(pub u64);

/// How many rows of a `List` can be selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    #[default]
    None,
    Single,
    Multiple,
}

/// How a `List` sits in its surroundings, mapped to each platform's native
/// look. Choosing per platform is up to the app, with `platform!`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ListStyle {
    /// The same as `Plain`, on every platform.
    #[default]
    Automatic,
    /// Edge to edge, on the surface around it: sidebars, a window's main
    /// content.
    Plain,
    /// In a bordered box, on the platform's content background: a list
    /// inside a form or a dialog.
    Framed,
}

impl ListStyle {
    pub fn framed(self) -> bool {
        self == ListStyle::Framed
    }
}

/// A property of a native widget. Which ones apply depends on the kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    /// Window title.
    Title(String),
    /// Text content of a `Text`.
    Text(String),
    /// Caption of a `Button`, `Checkbox` or `Switch`; accessible name of a
    /// `Switch`, `Select`, `Slider` or `Progress`.
    Label(String),
    /// Current text of a `TextInput`.
    Value(String),
    Placeholder(String),
    Checked(bool),
    Enabled(bool),
    TextStyle(TextStyle),
    ButtonRole(ButtonRole),
    ButtonStyle(ButtonStyle),
    /// A `Select`'s options, in order.
    Options(Vec<String>),
    /// Which option of a `Select` is chosen: always one, unless it has no
    /// options.
    SelectedIndex(Option<usize>),
    /// A `Slider`'s value, within its range. The core sends it after the
    /// range, which may have clamped it.
    Number(f64),
    /// The values a `Slider` can take.
    Range {
        min: f64,
        max: f64,
    },
    /// A `Slider`'s step, used as the platform uses one: to snap to, where
    /// its sliders snap, and to move by from the keyboard. `None`: the
    /// platform's default.
    Step(Option<f64>),
    /// How far along a `Progress` is, from 0 to 1. `None`: not known (an
    /// indeterminate, animated bar).
    Progress(Option<f64>),
    /// Which axes a `ScrollView` scrolls.
    ScrollAxes(ScrollAxes),
    /// A `List`'s rows, in order.
    Rows(Vec<RowKey>),
    /// How high a `List`'s rows are likely to be, for platforms that must
    /// size rows before they're shown.
    EstimatedRowHeight(f32),
    /// Which row of its `List` a row host shows.
    Row(RowKey),
    SelectionMode(SelectionMode),
    ListStyle(ListStyle),
    /// The selected rows of a `List`.
    Selected(Vec<RowKey>),
    /// A custom widget's props, with its renders.
    Custom(CustomProps),
    /// What a drawn custom widget shows. Computed by the core after layout,
    /// so it arrives with the frames.
    Drawing(DisplayList),
    /// A `Native` node's factory (on create), then its updates: payloads in
    /// the backend's own form.
    Native(Opaque),
    /// Raw platform settings for a built-in widget (see
    /// [`Tweak`](crate::Tweak)), in the backend's own form. Applied after
    /// the widget's other props, and again whenever they change.
    Tweak(Opaque),
}

impl Prop {
    /// Two props with the same key replace each other.
    pub fn key(&self) -> std::mem::Discriminant<Prop> {
        std::mem::discriminant(self)
    }

    /// Whether changing this prop can change the widget's intrinsic size.
    pub fn affects_measure(&self) -> bool {
        matches!(
            self,
            Prop::Text(_)
                | Prop::Label(_)
                | Prop::Placeholder(_)
                | Prop::Options(_)
                | Prop::SelectedIndex(_)
                | Prop::TextStyle(_)
                | Prop::ButtonRole(_)
                | Prop::ButtonStyle(_)
                | Prop::Custom(_)
                | Prop::Native(_)
                | Prop::Tweak(_)
        )
    }
}

/// Finds a prop by pattern in a prop list.
#[macro_export]
macro_rules! find_prop {
    ($props:expr, $variant:ident) => {
        $props.iter().find_map(|p| match p {
            $crate::Prop::$variant(v) => Some(v.clone()),
            _ => None,
        })
    };
}

macro_rules! static_value {
    ($($t:ty),*) => {$(
        impl mitsuami_reactive::IntoValue<$t> for $t {
            fn into_value(self) -> mitsuami_reactive::Value<$t> {
                mitsuami_reactive::Value::Static(self)
            }
        }
    )*};
}

static_value!(TextStyle, ButtonRole, ButtonStyle, ScrollAxes, SelectionMode, ListStyle);
