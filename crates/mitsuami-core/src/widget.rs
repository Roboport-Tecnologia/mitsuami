//! What the core tells backends to create, and the properties they carry.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::any_value::Opaque;
use crate::custom::CustomProps;
use crate::draw::{Color, DisplayList};
use crate::geometry::{Point, Size};
use crate::services::MenuEntry;

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
    /// A host for one item of its window's toolbar, the bar across the top
    /// that shows the window's title (the unified NSToolbar, GTK's header
    /// bar, a CommandBar on WinUI, Kirigami's page toolbar). A native child
    /// of the window, after its content. The core lays out what's in it at
    /// its natural size; the platform places it, at the bar's trailing end.
    ToolbarItem,
    /// A window's sidebar: the list down its leading side that picks what
    /// the window shows (a source list in an NSSplitViewController, a
    /// libadwaita navigation split view, NavigationView's pane on WinUI, a
    /// first column in Kirigami's page row). A native child of the window,
    /// after its content and toolbar items. Its items are data
    /// ([`Prop::Sections`]), the chosen one [`Prop::SelectedIndex`]; the
    /// platform draws, places and sizes it, and the window's content is
    /// what's beside it.
    Sidebar,
    /// Core-only grouping used by control flow (`Show`, `For`). Never sent to
    /// backends; its children are spliced into the nearest native ancestor.
    Fragment,
    Text,
    Button,
    /// A native button that opens a menu of actions (a pull-down
    /// NSPopUpButton, gtk::MenuButton, DropDownButton, a QQC2.Button that
    /// opens a QQC2.Menu). Its caption, icon and style are a `Button`'s;
    /// its menu is [`Prop::Menu`], and the item chosen comes as
    /// [`UiEvent::MenuItem`](crate::UiEvent::MenuItem).
    MenuButton,
    TextInput,
    /// A native password field (NSSecureTextField, PasswordBox,
    /// gtk::PasswordEntry, Kirigami.PasswordField): a `TextInput` whose
    /// text is hidden, as each platform hides it.
    PasswordInput,
    /// A native field for text over many lines (an NSTextView in an
    /// NSScrollView, a TextBox that accepts Return, a gtk::TextView in a
    /// gtk::ScrolledWindow, a QQC2.TextArea in a QQC2.ScrollView). Its
    /// lines wrap, Return starts a new one, and it scrolls past
    /// [`Prop::Lines`], its natural height.
    TextArea,
    Checkbox,
    Switch,
    /// A native pop-up menu of text options (NSPopUpButton, ComboBox,
    /// gtk::DropDown, QQC2.ComboBox). Its options are [`Prop::Options`].
    Select,
    /// Native radio buttons, one for each of [`Prop::Options`], of which
    /// one at most is chosen, [`Prop::SelectedIndex`] (NSButtons of the
    /// radio type, RadioButtons, gtk::CheckButtons in a group,
    /// QQC2.RadioButtons). Down a column, as each platform stacks them.
    RadioGroup,
    /// A native slider (NSSlider, Slider, gtk::Scale, QQC2.Slider): a
    /// [`Prop::Number`] in a [`Prop::Range`], in [`Prop::Step`]s.
    Slider,
    /// A native field for a whole number, with buttons that step it up and
    /// down (NumberBox, gtk::SpinButton, QQC2.SpinBox; on AppKit an
    /// NSTextField with an NSStepper beside it, as AppKit apps pair them):
    /// a [`Prop::Number`] in a [`Prop::Range`], stepped by [`Prop::Step`].
    /// Whole numbers that fit an `i32`, because Qt's spin box holds an
    /// `int`.
    NumberInput,
    /// A native progress bar (NSProgressIndicator, ProgressBar,
    /// gtk::ProgressBar, QQC2.ProgressBar): a [`Prop::Progress`].
    Progress,
    /// A native spinner for work of unknown length (a spinning
    /// NSProgressIndicator, gtk::Spinner, ProgressRing,
    /// QQC2.BusyIndicator). It spins while [`Prop::Running`], and shows
    /// nothing otherwise.
    Spinner,
    /// A native separator line between groups of content (a separator
    /// NSBox, gtk::Separator, Kirigami.Separator; WinUI has none, so a
    /// `Border` in the divider brush, as Fluent apps draw one). It runs as
    /// [`Prop::Orientation`] says and is as thick as the platform draws
    /// it; the layout gives it its length.
    Separator,
    /// A native image view (NSImageView, Image, gtk::Picture, QML Image):
    /// a picture from [`Prop::Image`], at its own size unless the layout
    /// sizes it, fitted as [`Prop::ImageFit`] says.
    Image,
    /// An icon from the platform's own set, by its name there: an SF
    /// Symbol in an NSImageView, a themed icon in a gtk::Image or a
    /// Kirigami.Icon, a Segoe Fluent Icons glyph in a FontIcon on WinUI.
    /// [`Prop::Icon`] names it, at the platform's own size unless
    /// [`Prop::IconSize`] says otherwise.
    Icon,
    /// A native surface the app presents to with its own GPU API, off the
    /// UI thread (an `NSView` backed by a `CAMetalLayer`, a Wayland
    /// subsurface, a child window). The backend reports it as
    /// `SurfaceReady`, then its size in pixels as `SurfaceResized`; with
    /// [`Prop::TakesInput`], its keys and pointer as `SurfaceInput`.
    GpuSurface,
    /// A native scroll container. It has exactly one native child, the
    /// content, which the core lays out and may be larger than the viewport.
    ScrollView,
    /// A native list control (NSTableView, ListView, gtk::ListView, QML
    /// ListView). Its data is [`Prop::Rows`]. The platform decides which rows
    /// exist (`RowShown`, `RowHidden`); its native children are the hosts of
    /// the rows the core mounted for those (`Container`s with a
    /// [`Prop::Row`]), in row order. It scrolls like a `ScrollView`.
    List,
    /// A native tab view: pages, one shown at a time, and a strip of their
    /// titles to pick one (NSTabView, gtk::Notebook, a SelectorBar over
    /// its pages on WinUI, QQC2.TabBar over a StackLayout). Its native
    /// children are its pages' hosts (`Container`s), in order, titled by
    /// [`Prop::TabTitles`]; the shown one is [`Prop::SelectedIndex`].
    /// Every page stays mounted. The core lays out each page at the size
    /// inside the tab strip and border ([`PlatformMetrics::tab_insets`](crate::PlatformMetrics)),
    /// all in the same place, so the view is as big as its biggest page;
    /// the platform places them and shows the chosen one.
    Tabs,
    /// A native box around related content, under an optional heading
    /// ([`Prop::Title`]): an NSBox, libadwaita's card under a heading, a
    /// Fluent card under a heading on WinUI, a QQC2.GroupBox. The core lays
    /// out its children as a container's, inside
    /// [`PlatformMetrics::group_insets`](crate::PlatformMetrics) (or the
    /// titled ones), and it's at least as wide as its heading, which
    /// backends measure. Backends draw the box behind the children, which
    /// are its native children.
    Group,
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
        matches!(
            self,
            WidgetKind::Window
                | WidgetKind::Container
                | WidgetKind::ToolbarItem
                | WidgetKind::ScrollView
                | WidgetKind::Tabs
                | WidgetKind::Group
        )
    }

    /// Scrolls its content: `ScrollTo` and `Scrolled` apply.
    pub fn scrolls(self) -> bool {
        matches!(self, WidgetKind::ScrollView | WidgetKind::List)
    }

    pub fn name(self) -> &'static str {
        match self {
            WidgetKind::Window => "Window",
            WidgetKind::Container => "Container",
            WidgetKind::ToolbarItem => "ToolbarItem",
            WidgetKind::Sidebar => "Sidebar",
            WidgetKind::ScrollView => "ScrollView",
            WidgetKind::List => "List",
            WidgetKind::Tabs => "Tabs",
            WidgetKind::Group => "Group",
            WidgetKind::Fragment => "Fragment",
            WidgetKind::Text => "Text",
            WidgetKind::Button => "Button",
            WidgetKind::MenuButton => "MenuButton",
            WidgetKind::TextInput => "TextInput",
            WidgetKind::PasswordInput => "PasswordInput",
            WidgetKind::TextArea => "TextArea",
            WidgetKind::Checkbox => "Checkbox",
            WidgetKind::Switch => "Switch",
            WidgetKind::Select => "Select",
            WidgetKind::RadioGroup => "RadioGroup",
            WidgetKind::Slider => "Slider",
            WidgetKind::NumberInput => "NumberInput",
            WidgetKind::Progress => "Progress",
            WidgetKind::Spinner => "Spinner",
            WidgetKind::Separator => "Separator",
            WidgetKind::Image => "Image",
            WidgetKind::Icon => "Icon",
            WidgetKind::GpuSurface => "GpuSurface",
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

/// How heavy a `Text`'s font is, in place of its text style's weight.
/// A platform whose font lacks one uses the nearest it has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontWeight {
    #[default]
    Regular,
    Medium,
    Semibold,
    Bold,
}

/// Where a `Text`'s lines go across its frame, in its reading direction:
/// `Start` is the left in left-to-right text, the right in right-to-left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// A [`TextAlign`] the core has resolved against the text's direction, as
/// backends get it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HorizontalAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// What a button does in its window or dialog, which each platform shows
/// and handles its own way. Platforms without an equivalent ignore it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonRole {
    /// An ordinary button.
    #[default]
    Normal,
    /// The action Return confirms: the default button on AppKit (Return
    /// clicks it), the suggested action on GTK, the default button on Qt, the
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

/// What an `Image` shows.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageSource {
    /// An image file, decoded by the platform: PNG and JPEG everywhere,
    /// other formats where the platform reads them. It's read when the
    /// source is set; a file that changes under the same path isn't read
    /// again.
    File(PathBuf),
    /// Pixels the app has in memory.
    Pixels(Pixels),
}

/// An image in memory: rows of straight (not premultiplied) RGBA8, top
/// row first, drawn at `scale` pixels to a point, so an image made at the
/// window's scale factor is shown pixel for pixel.
#[derive(Clone, PartialEq)]
pub struct Pixels {
    width: u32,
    height: u32,
    scale: f32,
    rgba: Arc<[u8]>,
}

/// Its size and scale, not its bytes.
impl fmt::Debug for Pixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pixels({}×{} @{}x)", self.width, self.height, self.scale)
    }
}

impl Pixels {
    /// Panics unless `rgba` holds `width × height` pixels.
    pub fn new(width: u32, height: u32, rgba: impl Into<Arc<[u8]>>) -> Pixels {
        let rgba = rgba.into();
        assert_eq!(rgba.len(), width as usize * height as usize * 4, "{width}×{height} RGBA8 pixels");
        Pixels { width, height, scale: 1.0, rgba }
    }

    /// Pixels to a point; 1 by default.
    pub fn scale(mut self, scale: f32) -> Pixels {
        assert!(scale > 0.0, "a scale above 0");
        self.scale = scale;
        self
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Its size in points.
    pub fn size(&self) -> crate::Size {
        crate::Size::new(self.width as f32 / self.scale, self.height as f32 / self.scale)
    }
}

/// How an `Image` fills a frame the layout makes a different size from
/// it. Only the fits every platform's image view has; without one, each
/// fits as it does by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageFit {
    /// As large as fits, keeping its proportions (NSImageView
    /// proportionally up or down, GTK `Contain`, Qt `PreserveAspectFit`,
    /// XAML `Uniform`).
    Contain,
    /// The whole frame, proportions or not (axes independently, `Fill`,
    /// `Stretch`, `Fill`).
    Stretch,
}

/// The pointer's cursor over a `GpuSurface`, while it isn't locked.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Cursor {
    /// The platform's own: the arrow.
    #[default]
    Default,
    /// None: the pointer still moves, unseen (a machine that draws its own).
    Hidden,
    /// An image of the app's, e.g. the one a machine gives its pointer,
    /// shown at its scale. `hotspot`: the point that points, in points from
    /// its top left.
    Image { pixels: Pixels, hotspot: Point },
}

/// What a modal `Window` blocks while it's open, shown each platform's way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modality {
    /// The window it belongs to (the one it's declared in): a sheet on
    /// that window on macOS, Qt's `WindowModal`, an owned `IsModal` window
    /// on WinUI. GTK's modal windows block the whole app.
    Window,
    /// Every other window of the app: a window in AppKit's modal loop,
    /// Qt's `ApplicationModal`, a modal window on GTK, and on WinUI an
    /// owned `IsModal` window that also disables the app's other windows.
    Application,
}

/// The window whose content is being built: provided in each window's
/// scope, so views can find the window they're in (a modal `Window`
/// belongs to it, a `Toolbar`'s items go in its toolbar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentWindow(pub NodeId);

/// Which files a node takes when they're dropped on it: files with these
/// extensions (any file without a list), and folders if `folders`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FileDrop {
    /// Without the dot, compared ignoring case. `None`: any file.
    pub extensions: Option<Vec<String>>,
    pub folders: bool,
}

impl FileDrop {
    /// Any file, no folders.
    pub fn files() -> FileDrop {
        FileDrop { extensions: None, folders: false }
    }

    /// Files with these extensions (`"iso"`, not `".iso"`), no folders.
    pub fn extensions<S: Into<String>>(extensions: impl IntoIterator<Item = S>) -> FileDrop {
        FileDrop { extensions: Some(extensions.into_iter().map(Into::into).collect()), folders: false }
    }

    /// Folders only.
    pub fn folders() -> FileDrop {
        FileDrop { extensions: Some(Vec::new()), folders: true }
    }

    /// Folders too.
    pub fn and_folders(mut self) -> FileDrop {
        self.folders = true;
        self
    }

    /// Whether it takes this file or folder, as it is on disk.
    pub fn accepts(&self, path: &std::path::Path) -> bool {
        if path.is_dir() {
            return self.folders;
        }
        match &self.extensions {
            None => true,
            Some(extensions) => path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e))),
        }
    }

    /// What it takes of these, in order.
    pub fn accepted(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        paths.iter().filter(|p| self.accepts(p)).cloned().collect()
    }
}

/// Which way a `Slider` or a `Separator` runs. Every platform has vertical
/// sliders; larger values are up on all of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

impl Orientation {
    pub fn vertical(self) -> bool {
        self == Orientation::Vertical
    }
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

/// A group of a `Sidebar`'s items, under a heading if it has a title.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SidebarSectionData {
    pub title: Option<String>,
    pub items: Vec<SidebarItemData>,
}

/// One item of a `Sidebar`: its title, and the name of its icon in the
/// platform's own set (an SF Symbol, a symbolic theme icon, a Segoe
/// Fluent Icons glyph), which platforms without it ignore.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SidebarItemData {
    pub title: String,
    pub icon: Option<String>,
}

/// A property of a native widget. Which ones apply depends on the kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    /// A window's title, or a `Group`'s heading (empty: none).
    Title(String),
    /// A window fills its screen, the platform's own way (a Space of its
    /// own on macOS). The user can change it too (the title bar's button,
    /// the window manager's key): the backend reports `FullScreenChanged`,
    /// which the core absorbs.
    FullScreen(bool),
    /// The smallest content size the user can make a window, in points.
    /// A window smaller when it's set grows to it.
    MinSize(Size),
    /// The content sets a window's height, not the user, who resizes only
    /// its width; where the platform can't hold one side (GTK), neither.
    /// The core sets it for a `WindowSize::FollowHeight` window.
    HeightFollowsContent(bool),
    /// Text content of a `Text`.
    Text(String),
    /// How many lines a `Text` shows at most, the last one cut off with an
    /// ellipsis, as the platform draws one. `None`: all of them.
    MaxLines(Option<u32>),
    /// A `Text`'s or `Icon`'s colour. Semantic colours follow the
    /// appearance (dark mode, high contrast, the accent colour); `Rgba` is
    /// fixed. Icons in full colour keep their own; symbolic ones take it.
    /// Sent only if the app chose.
    TextColor(Color),
    /// A `Text`'s weight, in place of its text style's.
    FontWeight(FontWeight),
    /// A `Text` in italics.
    Italic(bool),
    /// Where a `Text`'s lines go across its frame. The core resolves the
    /// app's `TextAlign` against the text's direction, so it's left or
    /// right here.
    TextAlign(HorizontalAlign),
    /// Caption of a `Button`, `MenuButton`, `Checkbox` or `Switch`; accessible name of a
    /// `Switch`, `Select`, `RadioGroup`, `Slider`, `NumberInput`, `Progress`, `Image`,
    /// `Icon` or `GpuSurface`, and of a `Button` that shows only its icon.
    Label(String),
    /// Current text of a `TextInput`, `PasswordInput` or `TextArea`; a
    /// text area's lines end in `\n`.
    Value(String),
    /// What a text field shows while it's empty. Text areas show it where
    /// the platform's have one (Qt, WinUI).
    Placeholder(String),
    /// A `TextInput` or `TextArea` shows its text, which can be selected
    /// and copied, but not edited: by typing, or by assistive technology.
    ReadOnly(bool),
    /// How many lines of text a `TextArea` is tall at its natural size, in
    /// the platform's font and line spacing; it scrolls past them. The
    /// layout can still make it taller or shorter.
    Lines(u32),
    Checked(bool),
    /// A `Checkbox` shows the mixed state (some of what it stands for is
    /// checked), whatever `Checked` says. A click leaves it: where it lands
    /// is the platform's call, and the core absorbs it as `Mixed(false)`.
    Mixed(bool),
    Enabled(bool),
    TextStyle(TextStyle),
    ButtonRole(ButtonRole),
    ButtonStyle(ButtonStyle),
    /// A `Select`'s or `RadioGroup`'s options, in order.
    Options(Vec<String>),
    /// Which option of a `Select` is chosen: always one, unless it has no
    /// options. Which option of a `RadioGroup` is: `None` for none. Which item of a `Sidebar` is, counting across its sections:
    /// `None` for none. Which page of a `Tabs` is shown: always one, unless
    /// it has no pages.
    SelectedIndex(Option<usize>),
    /// A `Slider`'s or `NumberInput`'s value, within its range. The core
    /// sends it after the range, which may have clamped it.
    Number(f64),
    /// The values a `Slider` or `NumberInput` can take.
    Range {
        min: f64,
        max: f64,
    },
    /// A `Slider`'s step, used as the platform uses one: to snap to, where
    /// its sliders snap, and to move by from the keyboard. What a
    /// `NumberInput`'s buttons and arrow keys add or take away. `None`: the
    /// platform's default.
    Step(Option<f64>),
    /// Which way a `Slider` or a `Separator` runs.
    Orientation(Orientation),
    /// Text the platform shows when the pointer rests on the widget, as
    /// its tooltip, and assistive technology reads as its description.
    /// Any widget or container can have one; empty: none.
    Tooltip(String),
    /// The menu the platform shows its own way on a right-click, a long
    /// press or the menu key: items, separators and submenus. Any widget
    /// or container can have one; empty: none. The node reports the item
    /// chosen as [`UiEvent::ContextMenuItem`](crate::UiEvent::ContextMenuItem).
    ContextMenu(Vec<MenuEntry>),
    /// A `MenuButton`'s menu: items, separators and submenus, built as a
    /// context menu's. The node reports the item chosen as
    /// [`UiEvent::MenuItem`](crate::UiEvent::MenuItem).
    Menu(Vec<MenuEntry>),
    /// A window is modal, belonging to `owner` (the window it was declared
    /// in, if any). Sent once, before the window is shown.
    Modal {
        owner: Option<NodeId>,
        modality: Modality,
    },
    /// What an `Image` shows.
    Image(ImageSource),
    /// How an `Image` fills its frame; sent only if the app chose.
    ImageFit(ImageFit),
    /// What an `Icon` shows, or the icon a `Button` or `MenuButton` shows before its
    /// caption: a name in the platform's own set (an SF Symbol, a themed
    /// icon's name, a Segoe Fluent Icons glyph). Empty: none. A name the
    /// set doesn't have shows as the platform shows one: nothing, or its
    /// missing-icon icon.
    Icon(String),
    /// How big an `Icon` is, in points, used as the platform sizes icons:
    /// an SF Symbol's point size (as a font's), the side of a square
    /// elsewhere. Sent only if the app chose.
    IconSize(f32),
    /// A `Button` or `MenuButton` shows its icon without its caption, which stays its
    /// accessible name. Without an icon it shows its caption.
    IconOnly(bool),
    /// Whether a `Spinner` spins. Stopped, it shows nothing but keeps its
    /// place.
    Running(bool),
    /// How far along a `Progress` is, from 0 to 1. `None`: not known (an
    /// indeterminate, animated bar).
    Progress(Option<f64>),
    /// Which axes a `ScrollView` scrolls.
    ScrollAxes(ScrollAxes),
    /// Whether a `ScrollView` shows scroll bars, as the platform shows them.
    /// Without, it still scrolls, by wheel, trackpad and touch.
    ScrollBars(bool),
    /// A `Tabs`' page titles, in page order.
    TabTitles(Vec<String>),
    /// A `Sidebar`'s items, in sections, in order.
    Sections(Vec<SidebarSectionData>),
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
    /// A `GpuSurface` takes keys and the pointer, and reports them as
    /// `SurfaceInput`: it takes focus from a click and from Tab.
    TakesInput(bool),
    /// A `GpuSurface` holds the pointer: the cursor is hidden and stays
    /// put, and its moves are reported as `SurfaceInput::Motion`. Only
    /// while its window is the active one; the backend reports
    /// `PointerLockEnded` when that stops, or it can't lock.
    PointerLock(bool),
    /// A `GpuSurface` takes every key while it has focus, the system's
    /// shortcuts and the app's own too, as far as the platform lets an
    /// app. Setting it focuses the surface; the backend reports
    /// `KeyboardGrabEnded` when the surface or its window loses focus, or
    /// it can't grab.
    KeyboardGrab(bool),
    /// The pointer's cursor over a `GpuSurface`.
    Cursor(Cursor),
    /// A `Container` or `Group` takes the files and folders this says
    /// when they're dragged from the platform's file manager and dropped
    /// on it; `None`: none. While acceptable files are over it, the
    /// platform shows it'll copy them, and the node reports
    /// `DropHover(true)`, then `DropHover(false)`; a drop reports the ones
    /// it takes as `FilesDropped`.
    FileDrop(Option<FileDrop>),
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

    /// Whether changing this prop can change the intrinsic size of a widget
    /// of this kind.
    pub fn affects_measure(&self, kind: WidgetKind) -> bool {
        // A spin box can be as wide as its range's longest number (GTK) or
        // its text (Qt).
        if kind == WidgetKind::NumberInput && matches!(self, Prop::Range { .. } | Prop::Number(_)) {
            return true;
        }
        // A pull-down can be as wide as its widest item (AppKit).
        if kind == WidgetKind::MenuButton && matches!(self, Prop::Menu(_)) {
            return true;
        }
        if kind == WidgetKind::TextArea && matches!(self, Prop::Lines(_)) {
            return true;
        }
        matches!(
            self,
            Prop::Text(_)
                | Prop::MaxLines(_)
                | Prop::FontWeight(_)
                | Prop::Italic(_)
                | Prop::Label(_)
                | Prop::Placeholder(_)
                | Prop::Options(_)
                | Prop::SelectedIndex(_)
                | Prop::TextStyle(_)
                | Prop::ButtonRole(_)
                | Prop::ButtonStyle(_)
                | Prop::Orientation(_)
                | Prop::Image(_)
                | Prop::Icon(_)
                | Prop::IconSize(_)
                | Prop::IconOnly(_)
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

static_value!(
    FileDrop,
    TextStyle,
    FontWeight,
    TextAlign,
    crate::draw::Color,
    ButtonRole,
    ButtonStyle,
    Orientation,
    ScrollAxes,
    SelectionMode,
    ListStyle,
    ImageSource,
    Pixels,
    ImageFit,
    Modality,
    Cursor,
    Size
);
