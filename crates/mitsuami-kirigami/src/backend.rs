//! The [`Backend`] implementation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use mitsuami_core::a11y::{A11yAction, A11yProps, ActionError};
use mitsuami_core::backend::{
    Appearance, AvailableSpace, Backend, CaptureError, EventSink, Image, Key, MeasureRequest, NativeState,
    PlatformMetrics, SyntheticInput,
};
use mitsuami_core::services::{MenuBarData, Reply};
use mitsuami_core::{
    AppInfo, ButtonRole, ButtonStyle, Color, Command, CustomProps, DisplayList, EventValue, HorizontalAlign, ImageFit,
    ImageSource, InputPurpose, KeyCode, Modality, Modifiers, NativeAppInfo, NativeIcon, NodeId, Opaque, Orientation,
    Point, PointerEvent, Prop, Rect, RowKey, ScrollAxes, ScrollDelta, SelectionMode, SidebarSectionData, Size,
    SurfaceInput, TabsStyle, TextStyle, UiEvent, WidgetKind, find_prop,
};

use crate::custom::{Emitter, ErasedRender, KirigamiCx, NativePayload, flatten};
use crate::events::{Events, node_from_key, node_key};
use crate::ffi::{self, Callback, QmlObject};
use crate::qml;
use crate::services::{ContextMenu, KirigamiServices, Menus, drawer_qml};
use crate::surface::SurfaceItem;
use crate::theme;

/// How the backend behaves; apps and tests want different things.
#[derive(Clone, Debug, Default)]
pub struct BackendOptions {
    /// Keep a log of applied commands (for tests).
    pub record_commands: bool,
    /// Force this appearance (Breeze Light or Breeze Dark), so captures are
    /// comparable across machines whatever the desktop's color scheme. The
    /// scheme is the application's, so it applies to every window.
    pub appearance: Option<Appearance>,
}

// Qt key codes (`Qt::Key`).
const KEY_TAB: i32 = 0x0100_0001;
const KEY_BACKSPACE: i32 = 0x0100_0003;
const KEY_RETURN: i32 = 0x0100_0004;
const KEY_ESCAPE: i32 = 0x0100_0000;
const KEY_HOME: i32 = 0x0100_0010;
const KEY_END: i32 = 0x0100_0011;
const KEY_UP: i32 = 0x0100_0013;
const KEY_DOWN: i32 = 0x0100_0015;
const KEY_UNKNOWN: i32 = 0x01ff_ffff;

// `Text.elide` values (`Qt::TextElideMode`).
const ELIDE_RIGHT: i32 = 1;
const ELIDE_NONE: i32 = 3;
/// `Qt::AlignLeft`, `Qt::AlignRight` and `Qt::AlignHCenter`.
const ALIGN_LEFT: i32 = 1;
const ALIGN_RIGHT: i32 = 2;
const ALIGN_H_CENTER: i32 = 4;

/// A window's content host and what it reports.
pub(crate) struct WindowRoot {
    id: NodeId,
    pub(crate) window: QmlObject,
    host: QmlObject,
    events: Events,
    /// Content size the core knows about (requested or last reported).
    size: Cell<Size>,
    /// A size the core asked for that the host doesn't have yet: not
    /// reported back.
    requested: Cell<Option<Size>>,
    /// The height of Kirigami's toolbar above the content, once known.
    header: Cell<Option<f64>>,
    /// Its menus, a Kirigami global drawer.
    pub(crate) drawer: Cell<Option<QmlObject>>,
    /// What the drawer shows: the app's menus and the window's own.
    pub(crate) menu: RefCell<MenuBarData>,
    /// Its toolbar items, in order.
    toolbar: RefCell<Vec<NodeId>>,
    /// Whether its first control was given focus, which happens once.
    focused_first: Cell<bool>,
    /// Full screen as the app wants it, and the user (who changes it too).
    full_screen: Cell<bool>,
    /// Maximized as the app wants it, and the user.
    maximized: Cell<bool>,
    /// The user can resize it: without, its minimum and maximum are its
    /// size, which moves with every size the app gives it.
    resizable: Cell<bool>,
    /// The smallest content size, if the app set one.
    min: Cell<Option<Size>>,
    /// The content sets the height, not the user.
    height_locked: Cell<bool>,
    /// Its sidebar's node and page, while it has one.
    sidebar: Cell<Option<(NodeId, QmlObject)>>,
}

/// `Qt::WindowFullScreen`.
const FULL_SCREEN: i32 = 0x4;
/// `Qt::WindowMaximized`.
const MAXIMIZED: i32 = 0x2;

/// `TextEdit.NoWrap` and `TextEdit.Wrap` (at word boundaries, or anywhere).
const TEXT_EDIT_NO_WRAP: i32 = 0;
const TEXT_EDIT_WRAP: i32 = 4;

/// `Qt::ImhDialableCharactersOnly`, `Qt::ImhEmailCharactersOnly` and
/// `Qt::ImhUrlCharactersOnly`: what a field is for, to an input method.
const PHONE_HINT: i32 = 0x0010_0000;
const EMAIL_HINT: i32 = 0x0020_0000;
const URL_HINT: i32 = 0x0040_0000;
const PURPOSE_HINTS: i32 = PHONE_HINT | EMAIL_HINT | URL_HINT;

fn purpose_hint(purpose: InputPurpose) -> i32 {
    match purpose {
        InputPurpose::Text => 0,
        InputPurpose::Email => EMAIL_HINT,
        InputPurpose::Url => URL_HINT,
        InputPurpose::Phone => PHONE_HINT,
    }
}

/// `QWINDOWSIZE_MAX`, a window's largest side and its default maximum.
const WINDOW_SIZE_MAX: i32 = 16_777_215;

impl WindowRoot {
    fn host_size(&self) -> Size {
        size_of(self.host)
    }

    /// The toolbar's height. Kirigami lays out its page stack when it
    /// polishes, before a frame, so the window's items are polished first.
    fn header(&self) -> f64 {
        if let Some(header) = self.header.get() {
            return header;
        }
        self.window.polish_items();
        let header = self.window.real("height") - self.host.real("height");
        header.max(0.0)
    }

    /// How much wider the window is than its content: by the sidebar's
    /// column, where the page row shows it beside the content.
    fn side(&self) -> f64 {
        if self.sidebar.get().is_none() {
            return 0.0;
        }
        self.window.polish_items();
        self.window.real("mitsuamiSidebarWidth").max(0.0)
    }

    /// Sizes the window so its content area is `size`. Qt sizes windows in
    /// whole pixels.
    fn place(&self, size: Size) {
        let header = self.header();
        let height = (size.height as f64 + header).round();
        let width = (size.width as f64 + self.side()).round();
        // A locked height moves with it, and a fixed size: let go first,
        // so neither bound is past the other on the way.
        if !self.resizable.get() {
            self.window.set_int("minimumWidth", 0);
            self.window.set_int("maximumWidth", WINDOW_SIZE_MAX);
            self.window.set_int("maximumWidth", width as i32);
            self.window.set_int("minimumWidth", width as i32);
        }
        if self.height_locked.get() || !self.resizable.get() {
            self.window.set_int("minimumHeight", 0);
            self.window.set_int("maximumHeight", WINDOW_SIZE_MAX);
            self.window.set_int("maximumHeight", height as i32);
            self.window.set_int("minimumHeight", height as i32);
        }
        self.window.set_real("width", width);
        self.window.set_real("height", height);
    }

    /// The user can't resize it: Qt has no such flag on Linux, and KWin
    /// holds a window whose minimum is its maximum, as KDE's fixed-size
    /// dialogs are.
    fn set_resizable(&self, on: bool) {
        self.resizable.set(on);
        self.apply_min();
    }

    fn width_fixed(&self) -> bool {
        self.window.int("maximumWidth") < WINDOW_SIZE_MAX
    }

    fn is_maximized(&self) -> bool {
        self.window.window_states() & MAXIMIZED != 0
    }

    /// As KDE's maximize action does: full screen stays as it is.
    fn set_maximized(&self, on: bool) {
        self.maximized.set(on);
        let states = self.window.window_states();
        let wanted = if on { states | MAXIMIZED } else { states & !MAXIMIZED };
        if wanted != states {
            self.window.set_window_states(wanted);
        }
    }

    /// The content sets the height: a minimum and a maximum at the height
    /// it has, so the user resizes only the width, as 2ksbox's launcher
    /// holds its content-sized dialogs.
    fn set_height_locked(&self, locked: bool) {
        self.height_locked.set(locked);
        self.apply_min();
    }

    /// A fixed size holds the height too: then the app's.
    fn height_locked(&self) -> bool {
        if self.resizable.get() { self.window.int("maximumHeight") < WINDOW_SIZE_MAX } else { self.height_locked.get() }
    }

    /// Full screen as Qt has it: what it asked the platform for, until the
    /// platform says otherwise.
    fn in_full_screen(&self) -> bool {
        self.window.window_states() & FULL_SCREEN != 0
    }

    /// As KDE's full screen action does: the other states (maximized)
    /// stay, for when it comes back.
    fn set_full_screen(&self, on: bool) {
        self.full_screen.set(on);
        let states = self.window.window_states();
        let wanted = if on { states | FULL_SCREEN } else { states & !FULL_SCREEN };
        if wanted != states {
            self.window.set_window_states(wanted);
        }
    }

    /// Qt applied a state: on Wayland once the compositor has, and when
    /// the user changed it (the window manager's key). Only what the app
    /// didn't ask for is reported, a refusal too.
    fn states_changed(&self) {
        let now = self.in_full_screen();
        if self.full_screen.replace(now) != now {
            self.events.emit(self.id, UiEvent::FullScreenChanged(now));
        }
        let now = self.is_maximized();
        if self.maximized.replace(now) != now {
            self.events.emit(self.id, UiEvent::MaximizedChanged(now));
        }
    }

    /// The window's minimum is the content's and Kirigami's toolbar above
    /// it, in whole points, no larger than its screen takes.
    fn min_window_size(&self, min: Size) -> (i32, i32) {
        let min = self.capped(min);
        ((min.width as f64 + self.side()).ceil() as i32, (min.height as f64 + self.header()).ceil() as i32)
    }

    /// A minimum no larger than the content of a window filling its
    /// screen's available area (a machine's mode can be larger than a
    /// laptop's screen), in whole points.
    fn capped(&self, min: Size) -> Size {
        let Some((width, height)) = self.window.available_size() else { return min };
        let most =
            Size::new((width - self.side()).floor().max(0.0) as f32, (height - self.header()).floor().max(0.0) as f32);
        Size::new(min.width.min(most.width), min.height.min(most.height))
    }

    /// Another screen, another cap on the minimum.
    fn screen_changed(&self) {
        self.apply_min();
    }

    fn apply_min(&self) {
        let min = self.min.get().map(|min| self.min_window_size(min));
        let fixed = !self.resizable.get();
        let current = |side: &str| self.window.real(side).round() as i32;
        let locked = (self.height_locked.get() || fixed).then(|| current("height"));
        // Nothing to set, or to take back.
        let held = self.window.int("maximumHeight") < WINDOW_SIZE_MAX || self.width_fixed();
        if min.is_none() && locked.is_none() && !held {
            return;
        }
        let (width, height) = min.unwrap_or((0, 0));
        let fixed_width = fixed.then(|| current("width"));
        self.window.set_int("maximumWidth", fixed_width.unwrap_or(WINDOW_SIZE_MAX));
        self.window.set_int("minimumWidth", fixed_width.unwrap_or(width));
        self.window.set_int("maximumHeight", locked.unwrap_or(WINDOW_SIZE_MAX));
        self.window.set_int("minimumHeight", locked.unwrap_or(height));
    }

    /// The minimum as Qt has it: the app's, if Qt has what it was given
    /// (capped by the screen). A locked height hides the minimum's.
    fn min_size(&self) -> Size {
        // A fixed size hides the minimum: the app's.
        if !self.resizable.get() {
            return self.min.get().unwrap_or(Size::ZERO);
        }
        let (width, height) = (self.window.int("minimumWidth"), self.window.int("minimumHeight"));
        let locked = self.height_locked();
        match self.min.get() {
            Some(min) if self.min_window_size(min).0 == width && (locked || self.min_window_size(min).1 == height) => {
                min
            }
            _ => {
                Size::new((width as f64 - self.side()).max(0.0) as f32, (height as f64 - self.header()).max(0.0) as f32)
            }
        }
    }

    /// A size no smaller than the minimum.
    fn at_least_min(&self, size: Size) -> Size {
        let min = self.min.get().map_or(Size::ZERO, |min| self.capped(min));
        Size::new(size.width.max(min.width), size.height.max(min.height))
    }

    /// Whether the window has drawn a frame: it's shown and laid out.
    pub(crate) fn has_rendered(&self) -> bool {
        self.header.get().is_some()
    }

    /// The first frame is laid out for real: the toolbar's height is known.
    fn rendered(&self) {
        if self.header.get().is_some() {
            return;
        }
        let header = self.window.real("height") - self.host.real("height");
        self.header.set(Some(header.max(0.0)));
        self.apply_min();
        if let Some(requested) = self.requested.get() {
            self.place(requested);
        }
        self.host_resized();
    }

    /// The host's height changed while the window's didn't: the toolbar
    /// did (a toolbar item taller than it, or one gone). The content keeps
    /// the size the core has; the window grows or shrinks instead. Only on
    /// the host's height: its width changes first when both do, while its
    /// height still lags the window's.
    fn toolbar_resized(&self) {
        let Some(header) = self.header.get() else { return };
        let now = (self.window.real("height") - self.host.real("height")).max(0.0);
        if (now - header).abs() < 0.5 {
            return;
        }
        self.header.set(Some(now));
        self.apply_min();
        match self.requested.get() {
            Some(size) => self.place(size),
            None => self.request(self.size.get()),
        }
    }

    /// The host changed size: report it, unless it's on its way to a size
    /// the core asked for.
    fn host_resized(&self) {
        let size = self.host_size();
        if let Some(requested) = self.requested.get() {
            let close = (size.width - requested.width).abs() < 1.0 && (size.height - requested.height).abs() < 1.0;
            if !close || self.header.get().is_none() {
                return;
            }
            self.requested.set(None);
        }
        if self.size.replace(size) != size {
            self.events.emit(self.id, UiEvent::WindowResized(size));
        }
    }

    fn request(&self, size: Size) {
        self.size.set(size);
        self.requested.set(Some(size));
        self.place(size);
    }

    /// Resizes it to what the app asked for, no smaller than its minimum,
    /// and reports the size it gets. Before it's shown, the core's size
    /// is what it asked for: a minimum that grows it is reported too.
    fn resize_to(&self, asked: Size) {
        if !self.has_rendered() {
            self.size.set(asked);
        }
        let size = self.at_least_min(asked);
        self.requested.set(Some(size));
        self.place(size);
    }
}

enum Widget {
    Window {
        root: Rc<WindowRoot>,
    },
    Host(QmlObject),
    /// A toolbar item's host, and the page action that shows it.
    ToolbarItem {
        host: QmlObject,
        action: QmlObject,
    },
    Label(QmlObject),
    Button(QmlObject),
    MenuButton(QmlObject),
    Field(QmlObject),
    /// A scroll view, and the text area in it.
    TextArea {
        root: QmlObject,
        area: QmlObject,
    },
    Checkbox(QmlObject),
    Switch(QmlObject),
    Select(QmlObject),
    RadioGroup(QmlObject),
    Slider(QmlObject),
    NumberInput(QmlObject),
    Progress(QmlObject),
    Spinner(QmlObject),
    Separator(QmlObject),
    Icon(QmlObject),
    /// An image, what it shows (Qt can't give pixels or the source back as
    /// given), and the pixels it hands QML's provider, if any.
    Image {
        item: QmlObject,
        source: Option<ImageSource>,
        fit: Option<ImageFit>,
        pixels: Option<ffi::ProvidedPixels>,
    },
    GpuSurface(SurfaceItem),
    Scroll {
        view: QmlObject,
        flickable: QmlObject,
    },
    List(crate::list::List),
    /// A tab view, and the item its page hosts are in. Its strip, the
    /// bar shown, is `strip`.
    Tabs {
        root: QmlObject,
        pages: QmlObject,
    },
    /// A group: its host, the `QQC2.GroupBox` drawn behind its content,
    /// and the item its children are in.
    Group {
        root: QmlObject,
        group: QmlObject,
        content: QmlObject,
    },
    /// A window's sidebar page, and the sections it was given.
    Sidebar {
        page: QmlObject,
        sections: Vec<SidebarSectionData>,
    },
    /// A custom widget with a KDE render, and the props it last got.
    Custom {
        item: QmlObject,
        render: Rc<dyn ErasedRender>,
        props: CustomProps,
    },
    /// A drawn custom widget, its props and what it last drew.
    Drawn {
        item: QmlObject,
        props: CustomProps,
        drawing: DisplayList,
    },
    /// A native view from app code, and the last `Prop::Native` it got.
    Native {
        item: QmlObject,
        measure: Option<NativeMeasure>,
        last: Opaque,
    },
}

type NativeMeasure = Rc<dyn Fn(QmlObject, &MeasureRequest) -> Size>;

impl Widget {
    /// The item that stands for the node: a window's content host.
    fn item(&self) -> QmlObject {
        match self {
            Widget::Window { root } => root.host,
            Widget::ToolbarItem { host: i, .. }
            | Widget::Host(i)
            | Widget::Label(i)
            | Widget::Button(i)
            | Widget::MenuButton(i)
            | Widget::Field(i)
            | Widget::Checkbox(i)
            | Widget::Switch(i)
            | Widget::Select(i)
            | Widget::RadioGroup(i)
            | Widget::Slider(i)
            | Widget::NumberInput(i)
            | Widget::Progress(i)
            | Widget::Spinner(i)
            | Widget::Separator(i)
            | Widget::Icon(i)
            | Widget::Image { item: i, .. }
            | Widget::Scroll { view: i, .. }
            | Widget::Custom { item: i, .. }
            | Widget::Drawn { item: i, .. }
            | Widget::Native { item: i, .. }
            | Widget::Tabs { root: i, .. }
            | Widget::Group { root: i, .. }
            | Widget::TextArea { root: i, .. }
            | Widget::Sidebar { page: i, .. } => *i,
            Widget::GpuSurface(surface) => surface.item,
            Widget::List(list) => list.root,
        }
    }

    /// Made from our templates, which show a tooltip (`qml::a11y`). A
    /// window's host isn't: a window takes no tooltip.
    fn has_tooltip(&self) -> bool {
        !matches!(
            self,
            Widget::Window { .. }
                | Widget::Custom { .. }
                | Widget::Drawn { .. }
                | Widget::Native { .. }
                | Widget::Sidebar { .. }
        )
    }

    /// Made from our templates, which show a context menu
    /// (`qml::CONTEXT_MENU`), except text fields and areas: they keep KDE's
    /// own, with Cut, Copy and Paste, as a field's own menu wins on every
    /// platform.
    fn shows_context_menu(&self) -> bool {
        self.has_tooltip() && !matches!(self, Widget::Field(_) | Widget::TextArea { .. })
    }

    /// The item that takes keyboard focus and input: a list's list view.
    fn input_item(&self) -> QmlObject {
        match self {
            Widget::Scroll { flickable, .. } => *flickable,
            Widget::List(list) => list.view,
            Widget::GpuSurface(surface) => surface.input,
            Widget::Sidebar { page, .. } => page.child("mitsuamiSidebarList").unwrap_or(*page),
            Widget::TextArea { area, .. } => *area,
            // Its bar, which hands focus to its selected tab.
            Widget::Tabs { root, .. } => strip(*root),
            widget => widget.item(),
        }
    }

    /// Where children go: the host, the scrolled content, or a tab view's
    /// page area.
    fn content(&self) -> QmlObject {
        match self {
            Widget::Scroll { flickable, .. } => flickable.object("contentItem").expect("flickables have content"),
            Widget::Tabs { pages, .. } => *pages,
            Widget::Group { content, .. } => *content,
            widget => widget.item(),
        }
    }

    /// Built-in controls: they take `Enabled`.
    fn is_control(&self) -> bool {
        matches!(
            self,
            Widget::Label(_)
                | Widget::Button(_)
                | Widget::MenuButton(_)
                | Widget::Field(_)
                | Widget::TextArea { .. }
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select(_)
                | Widget::RadioGroup(_)
                | Widget::Slider(_)
                | Widget::NumberInput(_)
        )
    }

    /// Controls that can take keyboard focus.
    fn is_focusable(&self) -> bool {
        matches!(
            self,
            Widget::Button(_)
                | Widget::MenuButton(_)
                | Widget::Field(_)
                | Widget::TextArea { .. }
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select(_)
                | Widget::RadioGroup(_)
                | Widget::Slider(_)
                | Widget::NumberInput(_)
                | Widget::Custom { .. }
                | Widget::Native { .. }
                | Widget::List(_)
                | Widget::Sidebar { .. }
                | Widget::Tabs { .. }
        ) || matches!(self, Widget::GpuSurface(surface) if surface.takes_input())
    }

    /// Measured, never laid out inside: controls and escape hatches.
    fn is_leaf(&self) -> bool {
        !matches!(
            self,
            Widget::Window { .. }
                | Widget::Host(_)
                | Widget::ToolbarItem { .. }
                | Widget::Scroll { .. }
                | Widget::List(_)
                | Widget::Sidebar { .. }
                | Widget::Tabs { .. }
                | Widget::Group { .. }
        )
    }
}

/// `Image.Status`: loaded, or failed.
const IMAGE_READY: i32 = 1;
const IMAGE_ERROR: i32 = 3;

/// `Image.FillMode`.
const FILL_STRETCH: i32 = 0;
const FILL_PRESERVE_ASPECT_FIT: i32 = 1;

/// `Qt::Orientation`, a `Slider`'s `orientation`.
const QT_HORIZONTAL: i32 = 1;
const QT_VERTICAL: i32 = 2;

/// `Qt::CheckState`, a `CheckBox`'s `checkState`.
const UNCHECKED: i32 = 0;
const PARTIALLY_CHECKED: i32 = 1;
const CHECKED: i32 = 2;

struct Node {
    kind: WidgetKind,
    widget: Widget,
    parent: Option<NodeId>,
    /// Row hosts: which row of their list they show.
    row: Option<RowKey>,
    /// Props Qt can't report back faithfully.
    text_style: Option<TextStyle>,
    role: Option<ButtonRole>,
    button_style: Option<ButtonStyle>,
    /// Tabs: the style the app chose, `Automatic` apart.
    tabs_style: Option<TabsStyle>,
    /// Sliders: whether the app gave an `Orientation`. Separators: which
    /// way they run, which Kirigami's don't know.
    orientation: Option<Orientation>,
    /// Checkboxes: whether the app gave `Mixed`, and the `Checked` the box
    /// shows when it isn't mixed.
    mixed: Option<bool>,
    checked: bool,
    /// Icons and buttons: whether the app gave a size, or said a button
    /// shows only its icon.
    icon_size: bool,
    icon_only: bool,
    /// The app's raw settings, run after every other prop.
    tweak: Option<Opaque>,
    scroll_axes: Option<ScrollAxes>,
    /// Switches, selects, sliders and progress bars show no caption; the
    /// label is their accessible name.
    a11y_label: Option<String>,
    /// The tooltip, for items made elsewhere (custom renders, drawn and
    /// native items), which have no `mitsuamiTooltip` to hold it.
    tooltip: String,
    /// Windows: modal, and the window they belong to (Qt reads back the
    /// modality, but not which node the transient parent is).
    modal: Option<(Option<NodeId>, Modality)>,
    /// The context menu, once the app gave one.
    context_menu: Option<ContextMenu>,
    /// Menu buttons: their menu, once the app gave one.
    button_menu: Option<ContextMenu>,
    /// Hosts: the drop area over them while they take files, and whether
    /// the app gave `FileDrop` at all.
    file_drop: Option<crate::file_drop::FileDropArea>,
    file_drop_given: bool,
}

pub(crate) struct State {
    options: BackendOptions,
    nodes: HashMap<NodeId, Node>,
    events: Events,
    log: Vec<Command>,
    pending_show: Vec<NodeId>,
    /// The app's menus and windows' own, for windows current and future.
    pub(crate) menus: Menus,
}

impl Drop for State {
    /// A backend's windows go with it, at the next turn of the event loop.
    /// A backend can be dropped with the thread-locals that hold it as the
    /// process exits, when destroying Qt objects isn't safe any more (KDE's
    /// icon loader may be gone): posting their deletion touches nothing,
    /// and at exit the posted deletions are dropped (see `cpp/shim.cpp`).
    fn drop(&mut self) {
        if ffi::is_exiting() {
            return;
        }
        for node in self.nodes.values() {
            if let Widget::Window { root } = &node.widget {
                if let Some(drawer) = root.drawer.take() {
                    drawer.delete_later();
                }
                root.window.delete_later();
            }
            if let Some(menu) = &node.context_menu {
                menu.delete_later();
            }
            if let Some(menu) = &node.button_menu {
                menu.delete_later();
            }
        }
    }
}

pub struct KirigamiBackend {
    state: Rc<RefCell<State>>,
}

/// Shared access to a [`KirigamiBackend`] after it was moved into a `Ui`.
#[derive(Clone)]
pub struct KirigamiHandle {
    state: Rc<RefCell<State>>,
}

fn violation(command: &Command, problem: &str) -> ! {
    panic!("kirigami backend: protocol violation in {command:?}: {problem}")
}

/// A key pressed in a window, which the user's keys reach once it's the
/// focused window: its shortcuts (a modal window's Escape) only match then.
fn key_in(window: QmlObject, code: i32, text: &str) {
    if !window.bool("mitsuamiFocused") {
        window.invoke("requestActivate");
        pump_until(Duration::from_secs(2), || window.bool("mitsuamiFocused"));
    }
    window.key(code, false, text);
}

/// Runs Qt's event loop until `done` holds or `timeout` passes.
fn pump_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    loop {
        ffi::process_events();
        if done() {
            return true;
        }
        if started.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn size_of(item: QmlObject) -> Size {
    Size::new(item.real("width") as f32, item.real("height") as f32)
}

fn frame_of(item: QmlObject) -> Rect {
    Rect::new(item.real("x") as f32, item.real("y") as f32, item.real("width") as f32, item.real("height") as f32)
}

impl KirigamiBackend {
    /// Qt must be initialized on this thread ([`crate::run`] and
    /// [`crate::init_for_tests`] do it).
    pub fn new(options: BackendOptions) -> KirigamiBackend {
        assert!(ffi::is_initialized(), "mitsuami: initialize Qt on the main thread first");
        // Windows of backends dropped before are deleted "later" (see
        // `State::drop`): now. Their controls would still hold Alt-key
        // mnemonics, for one.
        ffi::process_events();
        if let Some(appearance) = options.appearance {
            theme::force(appearance);
        }
        let state = Rc::new(RefCell::new(State {
            options,
            nodes: HashMap::new(),
            events: Events::default(),
            log: Vec::new(),
            pending_show: Vec::new(),
            menus: Menus::default(),
        }));
        // Theme and font changes are metrics changes, for every window.
        let weak = Rc::downgrade(&state);
        for signal in ["backgroundColorChanged()", "textColorChanged()", "defaultFontChanged()"] {
            let weak = weak.clone();
            theme::theme().connect(signal, move || {
                if let Some(handle) = KirigamiHandle::from_weak(&weak) {
                    handle.emit_to_windows(UiEvent::MetricsChanged);
                }
            });
        }
        KirigamiBackend { state }
    }

    pub fn handle(&self) -> KirigamiHandle {
        KirigamiHandle { state: self.state.clone() }
    }
}

impl KirigamiHandle {
    pub fn command_log(&self) -> Vec<Command> {
        self.state.borrow().log.clone()
    }

    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    /// Number of live native nodes.
    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }

    /// Called after every native event, so the run loop ticks.
    pub fn set_wake(&self, wake: impl Fn() + 'static) {
        self.state.borrow().events.set_wake(wake);
    }

    /// Resizes a window's content like the user would, and waits until Qt
    /// has laid it out; the content host reports it as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let Some(root) = self.window_root(window) else { return };
        // The user can't resize it.
        if !root.resizable.get() {
            return;
        }
        // A drag goes no smaller than the minimum, and keeps a locked height.
        let mut size = root.at_least_min(size);
        if root.height_locked() {
            size.height = root.size.get().height;
        }
        root.place(size);
        pump_until(Duration::from_secs(2), || root.host_size() == size);
    }

    /// Shows windows whose first layout has been applied.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            if let Some(root) = self.window_root(id) {
                root.window.set_bool("visible", true);
                root.window.invoke("requestActivate");
            }
        }
    }

    /// Dispatches whatever Qt has ready, without waiting.
    pub fn pump(&self) {
        ffi::process_events();
    }

    /// Escape hatch: the `Kirigami.ApplicationWindow` of a window node.
    pub fn qml_window(&self, id: NodeId) -> Option<QmlObject> {
        self.window_root(id).map(|root| root.window)
    }

    /// Escape hatch: the item of any node (a window's content host).
    pub fn qml_item(&self, id: NodeId) -> Option<QmlObject> {
        self.state.borrow().nodes.get(&id).map(|n| n.widget.item())
    }

    fn window_root(&self, id: NodeId) -> Option<Rc<WindowRoot>> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window { root } => Some(root.clone()),
            _ => None,
        }
    }

    /// Every window, in creation order, for services (menus, dialog parents).
    pub(crate) fn windows(&self) -> Vec<(NodeId, Rc<WindowRoot>)> {
        let state = self.state.borrow();
        let mut windows: Vec<(NodeId, Rc<WindowRoot>)> = state
            .nodes
            .iter()
            .filter_map(|(id, n)| match &n.widget {
                Widget::Window { root } => Some((*id, root.clone())),
                _ => None,
            })
            .collect();
        windows.sort_by_key(|(id, _)| *id);
        windows
    }

    fn emit_to_windows(&self, event: UiEvent) {
        let Ok(state) = self.state.try_borrow() else { return };
        let events = state.events.clone();
        drop(state);
        for (id, _) in self.windows() {
            events.emit(id, event.clone());
        }
    }

    /// Asks the app to close every window (the Quit command).
    pub(crate) fn request_quit(&self) {
        self.emit_to_windows(UiEvent::WindowCloseRequested);
    }

    pub(crate) fn weak(&self) -> Weak<RefCell<State>> {
        Rc::downgrade(&self.state)
    }

    pub(crate) fn from_weak(state: &Weak<RefCell<State>>) -> Option<KirigamiHandle> {
        state.upgrade().map(|state| KirigamiHandle { state })
    }

    pub(crate) fn with_menus<R>(&self, f: impl FnOnce(&mut Menus) -> R) -> R {
        f(&mut self.state.borrow_mut().menus)
    }
}

impl mitsuami_core::TestHooks for KirigamiHandle {
    fn name(&self) -> &'static str {
        "kirigami"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        KirigamiHandle::resize_window(self, window, size);
    }

    /// `QWindow::close()` asks the platform to close the window, which
    /// sends the close event the close button sends; the window's close
    /// filter reports it.
    fn close_window(&self, window: NodeId) {
        if let Some(root) = self.window_root(window) {
            root.window.invoke("close");
        }
    }

    fn take_command_log(&self) -> Vec<Command> {
        KirigamiHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        KirigamiHandle::node_count(self)
    }

    fn app_info(&self, window: NodeId) -> NativeAppInfo {
        let (id, name) = ffi::app_id_and_name();
        let icon = self.window_root(window).and_then(|root| root.window.window_icon());
        NativeAppInfo {
            id: (!id.is_empty()).then_some(id),
            name: (!name.is_empty()).then_some(name),
            icon: icon.map(|icon| match icon {
                ffi::WindowIcon::Named(name) => NativeIcon::Named(name),
                ffi::WindowIcon::Image { width, height } => NativeIcon::Image { width, height },
            }),
        }
    }

    /// Windows are shown once laid out, and Qt delivers what it queued
    /// (polishing, rendering, focus).
    fn settle(&self) {
        self.show_pending_windows();
        self.pump();
        // List views place and create delegates when they polish, before
        // a frame: have it now, so rows are where the view says.
        for (_, root) in self.windows() {
            if root.has_rendered() {
                root.window.polish_items();
            }
        }
        self.pump();
        // A size the app or a minimum asked for is reported once Qt lays
        // the window out; a platform that gives another is waited for no
        // longer than a user's resize is.
        for (_, root) in self.windows() {
            if root.has_rendered() && root.requested.get().is_some() {
                pump_until(Duration::from_secs(2), || root.requested.get().is_none());
            }
        }
    }
}

/// Keeps a scroll view's content size in step with the frames the core
/// sent: `ScrollTo` in the same commit needs the new range.
fn sync_scroll(flickable: QmlObject) {
    let content = flickable.object("contentItem").expect("flickables have content");
    let size = content.child_items().first().map(|c| size_of(*c)).unwrap_or_default();
    flickable.set_real("contentWidth", size.width as f64);
    flickable.set_real("contentHeight", size.height as f64);
    // Flickables don't clamp by themselves when the content shrinks.
    for (offset, content, view) in [("contentX", "contentWidth", "width"), ("contentY", "contentHeight", "height")] {
        let max = (flickable.real(content) - flickable.real(view)).max(0.0);
        if flickable.real(offset) > max {
            flickable.set_real(offset, max);
        }
    }
}

fn scroll_offset(flickable: QmlObject) -> Point {
    Point::new(flickable.real("contentX") as f32, flickable.real("contentY") as f32)
}

impl State {
    fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) {
        let events = self.events.clone();
        let widget = match kind {
            WidgetKind::Window => self.create_window(id, command),
            WidgetKind::Container => Widget::Host(QmlObject::load(&qml::container())),
            WidgetKind::ToolbarItem => {
                let host = QmlObject::load(&qml::container());
                let action = QmlObject::load(&qml::toolbar_action());
                action.set_object("mitsuamiItem", Some(host));
                Widget::ToolbarItem { host, action }
            }
            WidgetKind::Sidebar => {
                let page = QmlObject::load(&qml::sidebar());
                // The user's choice only: the app's doesn't emit it.
                page.connect("mitsuamiChosen()", move || {
                    if let Ok(index) = usize::try_from(page.int("mitsuamiSelected")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)));
                    }
                });
                Widget::Sidebar { page, sections: Vec::new() }
            }
            WidgetKind::Tabs => {
                let root = QmlObject::load(&qml::tabs());
                let pages = root.child("mitsuamiPages").expect("tab views have a page area");
                // The user's choice only: the app's doesn't emit it.
                root.connect("mitsuamiChosen()", move || {
                    if let Ok(index) = usize::try_from(root.int("mitsuamiSelected")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)));
                    }
                });
                Widget::Tabs { root, pages }
            }
            WidgetKind::Group => {
                let root = QmlObject::load(&qml::group());
                let group = root.child("mitsuamiGroupBox").expect("groups have a group box");
                let content = root.child("mitsuamiGroupContent").expect("groups have a content item");
                Widget::Group { root, group, content }
            }
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not a Kirigami one")
                        };
                        let item = render.create(custom.props(), &mut KirigamiCx::new(events.clone(), id));
                        Widget::Custom { item, render, props: custom }
                    }
                    None => {
                        let key = ffi::register(move |callback| {
                            if let Callback::Pointer(kind, position) = callback {
                                events.emit(id, UiEvent::Pointer(PointerEvent { kind, position }));
                            }
                        });
                        Widget::Drawn { item: QmlObject::drawn(key), props: custom, drawing: DisplayList::default() }
                    }
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not a Kirigami one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let measure = payload.spec.measure.clone();
                let item = create(&mut KirigamiCx::new(events.clone(), id));
                payload.apply(item);
                Widget::Native { item, measure, last: opaque }
            }
            // Selectable text is another item: chosen once, when it's made.
            WidgetKind::Text => {
                let selectable = matches!(command, Command::Create { props, .. }
                    if find_prop!(props, Selectable) == Some(true));
                Widget::Label(QmlObject::load(&if selectable { qml::selectable_label() } else { qml::label() }))
            }
            // `toggled` is the user's; `checkedChanged` fires for ours too.
            WidgetKind::ToggleButton => {
                let button = QmlObject::load(&qml::toggle_button());
                button.connect("toggled()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Bool(button.bool("checked"))))
                });
                Widget::Button(button)
            }
            WidgetKind::Button => {
                let button = QmlObject::load(&qml::button());
                button.connect("clicked()", move || events.emit(id, UiEvent::Click));
                Widget::Button(button)
            }
            // A click opens its menu: it reports only the item chosen.
            WidgetKind::MenuButton => Widget::MenuButton(QmlObject::load(&qml::menu_button())),
            WidgetKind::Checkbox | WidgetKind::Switch => {
                let switch = kind == WidgetKind::Switch;
                let toggle = QmlObject::load(&if switch { qml::switch() } else { qml::checkbox() });
                // `toggled` is the user's; `checkedChanged` fires for ours too.
                toggle.connect("toggled()", move || {
                    // Setting `checkState` to partly checked makes a box
                    // tristate; after a click, clicks mustn't cycle back.
                    if !switch {
                        toggle.set_bool("tristate", false);
                    }
                    events.emit(id, UiEvent::Changed(EventValue::Bool(toggle.bool("checked"))))
                });
                if switch { Widget::Switch(toggle) } else { Widget::Checkbox(toggle) }
            }
            WidgetKind::Select => {
                let select = QmlObject::load(&qml::select());
                // `activated` is the user's; `currentIndexChanged` fires for
                // ours too.
                select.connect("activated(int)", move || {
                    if let Ok(index) = usize::try_from(select.int("currentIndex")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)))
                    }
                });
                Widget::Select(select)
            }
            WidgetKind::RadioGroup => {
                let group = QmlObject::load(&qml::radio_group());
                group.connect("mitsuamiChosen()", move || {
                    if let Ok(index) = usize::try_from(group.int("mitsuamiSelected")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)))
                    }
                });
                Widget::RadioGroup(group)
            }
            WidgetKind::Slider => {
                let slider = QmlObject::load(&qml::slider());
                // `moved` is the user's; `valueChanged` fires for ours too.
                slider.connect("moved()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Number(slider.real("value"))))
                });
                Widget::Slider(slider)
            }
            WidgetKind::NumberInput => {
                let spin = QmlObject::load(&qml::number_input());
                // `valueModified` is the user's (buttons, arrow keys, a typed
                // number once it's committed); `valueChanged` fires for ours
                // too.
                spin.connect("valueModified()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Number(spin.int("value").into())))
                });
                Widget::NumberInput(spin)
            }
            WidgetKind::Progress => Widget::Progress(QmlObject::load(&qml::progress())),
            WidgetKind::Spinner => Widget::Spinner(QmlObject::load(&qml::spinner())),
            WidgetKind::Separator => Widget::Separator(QmlObject::load(&qml::separator())),
            WidgetKind::Icon => Widget::Icon(QmlObject::load(&qml::icon())),
            WidgetKind::GpuSurface => Widget::GpuSurface(SurfaceItem::new(id, events.clone())),
            WidgetKind::Image => {
                let item = QmlObject::load(&qml::image());
                // Loading is synchronous, but should an image finish (or
                // fail) later, its size changed.
                item.connect("statusChanged()", move || {
                    if matches!(item.int("status"), IMAGE_READY | IMAGE_ERROR) {
                        events.emit(id, UiEvent::Remeasure);
                    }
                });
                Widget::Image { item, source: None, fit: None, pixels: None }
            }
            WidgetKind::TextInput | WidgetKind::PasswordInput => {
                let qml = if kind == WidgetKind::TextInput { qml::text_field() } else { qml::password_field() };
                let field = QmlObject::load(&qml);
                let e = events.clone();
                // `textEdited` is the user's; `textChanged` fires for ours too.
                field
                    .connect("textEdited()", move || e.emit(id, UiEvent::Changed(EventValue::Text(field.str("text")))));
                // Return and Enter only; leaving the field doesn't submit.
                field.connect("accepted()", move || events.emit(id, UiEvent::Submit));
                Widget::Field(field)
            }
            WidgetKind::SearchInput => {
                let field = QmlObject::load(&qml::search_field());
                let e = events.clone();
                // The user's edits, the clear button's too; not ours.
                field.connect("mitsuamiEdited()", move || {
                    e.emit(id, UiEvent::Changed(EventValue::Text(field.str("text"))))
                });
                field.connect("mitsuamiSearched()", move || events.emit(id, UiEvent::Search(field.str("text"))));
                Widget::Field(field)
            }
            WidgetKind::TextArea => {
                let root = QmlObject::load(&qml::text_area());
                let area = root.child("mitsuamiTextArea").expect("text areas have their area");
                // `textChanged` fires for ours too; our sets are marked.
                root.connect("mitsuamiEdited()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Text(root.str("text"))))
                });
                Widget::TextArea { root, area }
            }
            WidgetKind::ScrollView => {
                let view = QmlObject::load(&qml::scroll_view());
                let flickable = view.child("mitsuamiFlickable").expect("scroll views have a flickable");
                for signal in ["contentXChanged()", "contentYChanged()"] {
                    let events = events.clone();
                    flickable.connect(signal, move || events.emit(id, UiEvent::Scrolled(scroll_offset(flickable))));
                }
                Widget::Scroll { view, flickable }
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
            WidgetKind::List => Widget::List(crate::list::List::new(id, events.clone())),
        };
        let item = widget.item();
        item.set_node(node_key(id));
        // A zero frame: the core only sends frames that differ from the
        // last one, and an item's size follows its implicit size until set.
        if kind != WidgetKind::Window {
            item.set_geometry(0.0, 0.0, 0.0, 0.0);
        }
        if widget.is_leaf() {
            // Not shown until it has a frame.
            item.set_bool("visible", false);
        }
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                parent: None,
                row: None,
                text_style: None,
                role: None,
                button_style: None,
                tabs_style: None,
                orientation: None,
                mixed: None,
                checked: false,
                icon_size: false,
                icon_only: false,
                tweak: None,
                tooltip: String::new(),
                modal: None,
                scroll_axes: None,
                a11y_label: None,
                context_menu: None,
                button_menu: None,
                file_drop: None,
                file_drop_given: false,
            },
        );
    }

    fn create_window(&mut self, id: NodeId, command: &Command) -> Widget {
        let events = self.events.clone();
        // A dialog's drawer holds only its own menus. Its modality comes
        // with its Create, so the drawer can come with the window.
        if let Command::Create { props, .. } = command
            && props.iter().any(|p| matches!(p, Prop::Modal { .. }))
        {
            self.menus.modal.insert(id);
        }
        let menu = self.menus.of(id);
        let drawer = drawer_qml(&menu, self.menus.modal.contains(&id));
        let window = QmlObject::load(&qml::window(drawer.as_deref()));
        let host = window.child("mitsuamiHost").expect("windows have a content host");
        let root = Rc::new(WindowRoot {
            id,
            window,
            host,
            events: events.clone(),
            size: Cell::new(Size::ZERO),
            requested: Cell::new(None),
            header: Cell::new(None),
            drawer: Cell::new(None),
            menu: RefCell::new(menu),
            toolbar: RefCell::new(Vec::new()),
            focused_first: Cell::new(false),
            full_screen: Cell::new(false),
            maximized: Cell::new(false),
            resizable: Cell::new(true),
            min: Cell::new(None),
            height_locked: Cell::new(false),
            sidebar: Cell::new(None),
        });
        let weak = Rc::downgrade(&root);
        window.connect("windowStateChanged(Qt::WindowState)", move || {
            if let Some(root) = weak.upgrade() {
                root.states_changed();
            }
        });
        let weak = Rc::downgrade(&root);
        window.connect("screenChanged(QScreen*)", move || {
            if let Some(root) = weak.upgrade() {
                root.screen_changed();
            }
        });
        for signal in ["widthChanged()", "heightChanged()"] {
            let root = Rc::downgrade(&root);
            let height = signal == "heightChanged()";
            host.connect(signal, move || {
                if let Some(root) = root.upgrade() {
                    if height {
                        root.toolbar_resized();
                    }
                    root.host_resized();
                }
            });
        }
        let weak = Rc::downgrade(&root);
        window.connect("frameSwapped()", move || {
            if let Some(root) = weak.upgrade() {
                root.rendered();
            }
        });
        // The app decides whether a window closes (e.g. to ask about
        // unsaved changes); the core destroys it if so.
        let e = events.clone();
        window.watch_close(move || e.emit(id, UiEvent::WindowCloseRequested));
        // One observer for every focus change: clicks, Tab, code.
        let (e, focused) = (events.clone(), Cell::new(None));
        window.connect("activeFocusItemChanged()", move || {
            let now = window.focus_item().and_then(|item| item.node()).map(node_from_key);
            let before = focused.replace(now);
            if before != now {
                if let Some(old) = before {
                    e.emit(old, UiEvent::FocusOut);
                }
                if let Some(new) = now {
                    e.emit(new, UiEvent::FocusIn);
                }
            }
        });
        let e = events.clone();
        window.connect("devicePixelRatioChanged()", move || e.emit(id, UiEvent::MetricsChanged));
        self.pending_show.push(id);
        if drawer.is_some()
            && let Some(wiring) = &self.menus.wiring
        {
            // Its drawer came with the window, if it has menus.
            root.drawer.set(window.object("globalDrawer"));
            wiring.connect(&root);
        }
        Widget::Window { root }
    }

    fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) {
        // A dialog over the window it belongs to, as 2ksbox's are: set
        // before the window is first shown, which is when Qt applies them.
        if let Prop::Modal { owner, modality } = prop {
            let parent = owner.and_then(|o| match self.nodes.get(&o).map(|n| &n.widget) {
                Some(Widget::Window { root }) => Some(root.window),
                _ => None,
            });
            let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
            let Widget::Window { root } = &node.widget else { violation(command, "only windows are modal") };
            root.window.set_object("transientParent", parent);
            // Qt::Dialog (which includes Qt::Window).
            root.window.set_int("flags", root.window.int("flags") | 0x3);
            // Escape asks it to close.
            root.window.set_bool("mitsuamiModal", true);
            root.window.set_int(
                "modality",
                match modality {
                    Modality::Window => 1,
                    Modality::Application => 2,
                },
            );
            node.modal = Some((*owner, *modality));
            return;
        }
        let events = self.events.clone();
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window { root }) => {
                root.window.set_str("title", t);
                // The page's title is what Kirigami shows in its toolbar.
                // Beside a sidebar, the sidebar's page has it, and the
                // content's is the item chosen's.
                match root.sidebar.get() {
                    Some((_, sidebar)) => sidebar.set_str("title", t),
                    None => {
                        if let Some(page) = root.window.child("mitsuamiPage") {
                            page.set_str("title", t);
                        }
                    }
                }
            }
            (Prop::Sections(new), Widget::Sidebar { page, sections }) => {
                page.set_str("mitsuamiSections", &sections_json(new));
                *sections = new.clone();
            }
            (Prop::SelectedIndex(index), Widget::Sidebar { page, .. }) => {
                page.set_int("mitsuamiSelected", index.map_or(-1, |i| i as i32));
            }
            // Its window takes the page out of its row, or puts it back.
            (Prop::SidebarShown(shown), Widget::Sidebar { page, .. }) => page.set_bool("mitsuamiShown", *shown),
            // Titles and pages come in either order: each shows again.
            (Prop::Title(title), Widget::Group { root, .. }) => root.set_str("mitsuamiTitle", title),
            (Prop::TabTitles(titles), Widget::Tabs { root, .. }) => {
                root.set_str_list("mitsuamiTitles", titles);
                root.invoke("mitsuamiShow");
            }
            (Prop::TabIcons(icons), Widget::Tabs { root, .. }) => root.set_str_list("mitsuamiIcons", icons),
            // Kirigami's navigation bar unless the app asks for Qt's tab bar.
            (Prop::TabsStyle(style), Widget::Tabs { root, .. }) => {
                root.set_bool("mitsuamiNavigation", *style != TabsStyle::TabBar);
                root.invoke("mitsuamiShow");
                node.tabs_style = Some(*style);
            }
            (Prop::SelectedIndex(index), Widget::Tabs { root, .. }) => {
                root.set_int("mitsuamiSelected", index.map_or(-1, |i| i as i32));
                root.invoke("mitsuamiShow");
            }
            (Prop::FullScreen(on), Widget::Window { root }) => root.set_full_screen(*on),
            (Prop::HeightFollowsContent(on), Widget::Window { root }) => root.set_height_locked(*on),
            (Prop::Maximized(on), Widget::Window { root }) => root.set_maximized(*on),
            (Prop::Resizable(on), Widget::Window { root }) => root.set_resizable(*on),
            // Qt keeps the user from making it smaller; a window smaller
            // already grows, as on the other platforms.
            (Prop::MinSize(min), Widget::Window { root }) => {
                root.min.set(Some(*min));
                root.apply_min();
                let size = root.requested.get().unwrap_or(root.size.get());
                if root.at_least_min(size) != size && !root.in_full_screen() {
                    root.resize_to(size);
                }
            }
            (Prop::Text(t), Widget::Label(l)) => l.set_str("text", t),
            // Qt elides the last line it shows.
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                l.set_int("maximumLineCount", lines.map_or(i32::MAX, |n| n as i32));
                l.set_int("elide", if lines.is_some() { ELIDE_RIGHT } else { ELIDE_NONE });
            }
            (Prop::TextColor(color), Widget::Label(l) | Widget::Icon(l)) => {
                if let Color::Rgba(r, g, b, a) = *color {
                    l.set_int("mitsuamiRgba", u32::from_be_bytes([r, g, b, a]) as i32);
                }
                l.set_int("mitsuamiColor", qml::color(*color));
            }
            (Prop::FontWeight(weight), Widget::Label(l)) => l.set_int("mitsuamiWeight", qml::font_weight(*weight)),
            (Prop::Italic(italic), Widget::Label(l)) => l.set_bool("mitsuamiItalic", *italic),
            // Set, it's what shows: Qt mirrors it only under
            // `LayoutMirroring`, which nothing enables, and aligns by the
            // text's own direction only while it's unset.
            (Prop::TextAlign(align), Widget::Label(l)) => l.set_int(
                "horizontalAlignment",
                match align {
                    HorizontalAlign::Left => ALIGN_LEFT,
                    HorizontalAlign::Center => ALIGN_H_CENTER,
                    HorizontalAlign::Right => ALIGN_RIGHT,
                },
            ),
            (Prop::Label(t), Widget::Button(b) | Widget::MenuButton(b) | Widget::Checkbox(b)) => b.set_str("text", t),
            (
                Prop::Label(t),
                Widget::Switch(s)
                | Widget::Select(s)
                | Widget::RadioGroup(s)
                | Widget::Slider(s)
                | Widget::NumberInput(s)
                | Widget::Progress(s)
                | Widget::Spinner(s)
                | Widget::Icon(s)
                | Widget::Image { item: s, .. },
            ) => {
                s.set_str("mitsuamiA11yName", t);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::GpuSurface(surface)) => {
                surface.item.set_str("mitsuamiA11yName", t);
                node.a11y_label = Some(t.clone());
            }
            (Prop::TakesInput(takes), Widget::GpuSurface(surface)) => surface.set_takes_input(*takes),
            (Prop::PointerLock(on), Widget::GpuSurface(surface)) => surface.set_pointer_lock(*on),
            (Prop::KeyboardGrab(on), Widget::GpuSurface(surface)) => surface.set_keyboard_grab(*on),
            (Prop::Cursor(cursor), Widget::GpuSurface(surface)) => surface.set_cursor(cursor),
            (Prop::Options(options), Widget::Select(s)) => {
                // A new model resets the chosen index; it stays if it can,
                // else the first option is chosen, as the core does. It
                // sends the index when that changes it.
                let chosen = s.int("currentIndex");
                s.set_str_list("mitsuamiOptions", options);
                let count = options.len() as i32;
                s.set_int(
                    "currentIndex",
                    if (0..count).contains(&chosen) {
                        chosen
                    } else if count > 0 {
                        0
                    } else {
                        -1
                    },
                );
            }
            (Prop::Range { min, max }, Widget::Slider(s)) => {
                s.set_real("from", *min);
                s.set_real("to", *max);
            }
            (Prop::Step(step), Widget::Slider(s)) => s.set_real("stepSize", step.unwrap_or(0.0)),
            (Prop::Number(n), Widget::Slider(s)) => s.set_real("value", *n),
            // Whole numbers: the core only sends those.
            (Prop::Range { min, max }, Widget::NumberInput(s)) => {
                s.set_int("from", *min as i32);
                s.set_int("to", *max as i32);
            }
            (Prop::Step(step), Widget::NumberInput(s)) => s.set_int("stepSize", step.map_or(1, |s| s as i32)),
            (Prop::Number(n), Widget::NumberInput(s)) => s.set_int("value", *n as i32),
            (Prop::WrapAround(on), Widget::NumberInput(s)) => s.set_bool("wrap", *on),
            (Prop::Orientation(o), Widget::Slider(s)) => {
                s.set_int("orientation", if o.vertical() { QT_VERTICAL } else { QT_HORIZONTAL });
                node.orientation = Some(*o);
            }
            // Kirigami's has no orientation: its frame says which way it
            // runs.
            (Prop::Orientation(o), Widget::Separator(_)) => node.orientation = Some(*o),
            // Stopped, a busy indicator fades out.
            (Prop::Running(r), Widget::Spinner(s)) => s.set_bool("running", *r),
            (Prop::Image(new), Widget::Image { item, source, pixels, .. }) => {
                match new {
                    ImageSource::File(path) => {
                        item.set_url("source", path);
                        *pixels = None;
                    }
                    ImageSource::Pixels(p) => {
                        // The new pixels first, then the url that shows them;
                        // the old ones go once nothing shows them.
                        let provided = ffi::ProvidedPixels::new(p);
                        item.set_url_str("source", &provided.url());
                        *pixels = Some(provided);
                    }
                }
                *source = Some(new.clone());
            }
            (Prop::ImageFit(new), Widget::Image { item, fit, .. }) => {
                item.set_int(
                    "fillMode",
                    match new {
                        ImageFit::Contain => FILL_PRESERVE_ASPECT_FIT,
                        ImageFit::Stretch => FILL_STRETCH,
                    },
                );
                *fit = Some(*new);
            }
            (Prop::Progress(progress), Widget::Progress(p)) => {
                p.set_bool("indeterminate", progress.is_none());
                if let Some(fraction) = progress {
                    p.set_real("value", *fraction);
                }
            }
            // New buttons, chosen as before if they can be, as the core
            // does: it only sends the index when that changes it.
            (Prop::Options(options), Widget::RadioGroup(g)) => {
                g.invoke("mitsuamiReadShown");
                let shown = g.int("mitsuamiShown");
                g.set_int("mitsuamiSelected", if shown < options.len() as i32 { shown } else { -1 });
                g.set_str_list("mitsuamiOptions", options);
            }
            (Prop::SelectedIndex(index), Widget::RadioGroup(g)) => {
                g.set_int("mitsuamiSelected", index.map_or(-1, |i| i as i32));
                // Set to what it was, it doesn't show it again.
                g.invoke("mitsuamiShow");
            }
            (Prop::SelectedIndex(index), Widget::Select(s)) => {
                s.set_int("currentIndex", index.map_or(-1, |i| i as i32))
            }
            (Prop::Value(t), Widget::Field(f)) => {
                // Don't disturb the caret when the field already shows it.
                if f.str("text") != *t {
                    // Kirigami's search for it isn't reported.
                    if node.kind == WidgetKind::SearchInput {
                        f.set_str("mitsuamiShown", t);
                    }
                    f.set_str("text", t);
                }
            }
            (Prop::Placeholder(t), Widget::Field(f)) => f.set_str("placeholderText", t),
            // Still focusable and selectable, so its text can be copied.
            (Prop::ReadOnly(r), Widget::Field(f)) => f.set_bool("readOnly", *r),
            // For the on-screen keyboard and input methods.
            (Prop::InputPurpose(purpose), Widget::Field(f)) => {
                let hints = f.int("inputMethodHints") & !PURPOSE_HINTS;
                f.set_int("inputMethodHints", hints | purpose_hint(*purpose));
            }
            (Prop::Value(t), Widget::TextArea { root, .. }) => {
                if root.str("text") != *t {
                    set_area_text(*root, t);
                }
            }
            (Prop::Placeholder(t), Widget::TextArea { root, .. }) => root.set_str("placeholderText", t),
            (Prop::ReadOnly(r), Widget::TextArea { root, .. }) => root.set_bool("readOnly", *r),
            (Prop::Lines(n), Widget::TextArea { root, .. }) => root.set_int("mitsuamiLines", *n as i32),
            // Unwrapped, long lines scroll sideways in the scroll view.
            (Prop::LineWrap(on), Widget::TextArea { area, .. }) => {
                area.set_int("wrapMode", if *on { TEXT_EDIT_WRAP } else { TEXT_EDIT_NO_WRAP })
            }
            (Prop::Checked(c), Widget::Checkbox(b)) => {
                node.checked = *c;
                // The mixed state shows over it.
                if b.int("checkState") != PARTIALLY_CHECKED {
                    b.set_bool("checked", *c);
                }
            }
            (Prop::Checked(c), Widget::Switch(b)) => b.set_bool("checked", *c),
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                node.mixed = Some(*m);
                if *m {
                    b.set_int("checkState", PARTIALLY_CHECKED);
                } else {
                    b.set_bool("tristate", false);
                    b.set_int("checkState", if node.checked { CHECKED } else { UNCHECKED });
                }
            }
            // Disabled, a text area shows no selection.
            (Prop::Enabled(e), Widget::TextArea { root, area }) => {
                root.set_bool("enabled", *e);
                if !e {
                    area.invoke("deselect");
                }
            }
            (Prop::Enabled(e), w) if w.is_control() => w.item().set_bool("enabled", *e),
            (Prop::TextStyle(style), w) if w.is_control() => {
                w.item().set_int("mitsuamiTextStyle", qml::text_style(*style));
                node.text_style = Some(*style);
            }
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                // Breeze tints the default button. There is no cancel or
                // destructive style.
                b.set_bool("mitsuamiDefault", *role == ButtonRole::Default);
                node.role = Some(*role);
            }
            (Prop::Icon(name), Widget::Button(b) | Widget::MenuButton(b)) => b.set_str("mitsuamiIcon", name),
            (Prop::IconOnly(only), Widget::Button(b) | Widget::MenuButton(b)) => {
                b.set_bool("mitsuamiIconOnly", *only);
                node.icon_only = true;
            }
            (Prop::Icon(name), Widget::Icon(i)) => i.set_str("mitsuamiName", name),
            (Prop::IconSize(points), Widget::Icon(i)) => {
                i.set_real("mitsuamiSize", *points as f64);
                node.icon_size = true;
            }
            (Prop::FileDrop(drop), Widget::Host(_) | Widget::Group { .. }) => {
                node.file_drop_given = true;
                match (drop, &node.file_drop) {
                    (Some(drop), Some(area)) => area.set(drop.clone()),
                    (Some(drop), None) => {
                        let host = node.widget.item();
                        node.file_drop = Some(crate::file_drop::FileDropArea::new(host, id, events, drop.clone()));
                    }
                    (None, _) => {
                        if let Some(area) = node.file_drop.take() {
                            area.remove();
                        }
                    }
                }
            }
            (Prop::Menu(entries), Widget::MenuButton(b)) => {
                let b = *b;
                node.button_menu
                    .get_or_insert_with(|| {
                        ContextMenu::for_button(move |chosen| events.emit(id, UiEvent::MenuItem(chosen)))
                    })
                    .set(Some(b), entries);
            }
            (Prop::ButtonStyle(style), Widget::Button(b) | Widget::MenuButton(b)) => {
                // Flat buttons have no frame until hovered.
                b.set_bool("flat", *style == ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Checked(c), Widget::Button(b)) => b.set_bool("checked", *c),
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            (Prop::Tooltip(text), widget) => {
                if widget.has_tooltip() {
                    widget.item().set_str("mitsuamiTooltip", text);
                }
                node.tooltip = text.clone();
            }
            // Custom renders, drawn and native items and text fields keep
            // it on the node without showing it.
            (Prop::ContextMenu(entries), widget) => {
                let item = widget.shows_context_menu().then(|| widget.item());
                node.context_menu
                    .get_or_insert_with(|| {
                        ContextMenu::new(move |chosen| events.emit(id, UiEvent::ContextMenuItem(chosen)))
                    })
                    .set(item, entries);
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone()),
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode),
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style),
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows),
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::ScrollAxes(axes), Widget::Scroll { view, .. }) => {
                let bits = match axes {
                    ScrollAxes::Horizontal => 1,
                    ScrollAxes::Vertical => 2,
                    ScrollAxes::Both => 3,
                };
                view.set_int("mitsuamiAxes", bits);
                node.scroll_axes = Some(*axes);
            }
            (Prop::ScrollBars(show), Widget::Scroll { view, .. }) => view.set_bool("mitsuamiBars", *show),
            (Prop::Custom(new), Widget::Custom { item, render, props }) => {
                if props != new {
                    render.update(*item, props.props(), new.props());
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(new), Widget::Drawn { item, drawing, .. }) => {
                item.set_drawn_ops(&flatten(new));
                *drawing = new.clone();
            }
            (Prop::Native(opaque), Widget::Native { item, last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    payload.apply(*item);
                }
                *last = opaque.clone();
            }
            _ => {}
        }
        // A window's sidebar leaves its row or comes back, and the content
        // keeps its size: the window shrinks or grows by the column.
        if let Prop::SidebarShown(_) = prop
            && let Some(Widget::Window { root }) = node.parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget)
        {
            root.window.invoke("mitsuamiApplySidebarShown");
            let size = root.size.get();
            if !size.is_empty() {
                root.resize_to(size);
            }
        }
    }

    fn widget(&self, id: NodeId, command: &Command) -> &Widget {
        match self.nodes.get(&id) {
            Some(node) => &node.widget,
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    fn window_root(&self, id: NodeId, command: &Command) -> Rc<WindowRoot> {
        match self.nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::Window { root }) => root.clone(),
            _ => violation(command, "not a window"),
        }
    }

    /// The flickable of the scroll view a node is the content of.
    fn scroll_parent(&self, id: NodeId) -> Option<QmlObject> {
        match &self.nodes.get(&self.nodes.get(&id)?.parent?)?.widget {
            Widget::Scroll { flickable, .. } => Some(*flickable),
            _ => None,
        }
    }

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) {
        let node = &self.nodes[&id];
        if let Some(run) = node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            match &node.widget {
                // The list view or text area, not the scroll view around it.
                Widget::List(list) => run(list.view),
                Widget::TextArea { area, .. } => run(*area),
                Widget::Group { group, .. } => run(*group),
                widget => run(widget.item()),
            }
        }
    }

    fn apply(&mut self, command: &Command) {
        match command {
            Command::Create { id, kind, props } => {
                if self.nodes.contains_key(id) {
                    violation(command, "node already exists");
                }
                self.create(*id, *kind, command);
                for prop in props {
                    self.set_prop(*id, prop, command);
                }
                self.run_tweak(*id);
            }
            Command::SetProp { id, prop } => {
                self.set_prop(*id, prop, command);
                self.run_tweak(*id);
            }
            Command::Insert { parent, child, index } => {
                let item = self.widget(*child, command).item();
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if let Widget::Sidebar { page, .. } = self.nodes[child].widget {
                    let Widget::Window { root } = &self.nodes[parent].widget else {
                        violation(command, "a sidebar goes in a window")
                    };
                    if root.sidebar.get().is_some() {
                        violation(command, "a window has one sidebar");
                    }
                    let content = root.window.child("mitsuamiPage").expect("windows have a page");
                    page.set_str("title", &root.window.str("title"));
                    page.set_object("mitsuamiContent", Some(content));
                    root.window.set_object("mitsuamiSidebar", Some(page));
                    root.window.invoke("mitsuamiShowSidebar");
                    root.sidebar.set(Some((*child, page)));
                    // The content keeps its size: the window grows by the
                    // sidebar's column.
                    let size = root.size.get();
                    if !size.is_empty() {
                        root.resize_to(size);
                    }
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if let Widget::ToolbarItem { action, .. } = self.nodes[child].widget {
                    // Items come after the window's content, and before its sidebar.
                    let content = self
                        .nodes
                        .values()
                        .filter(|n| {
                            n.parent == Some(*parent)
                                && !matches!(n.kind, WidgetKind::ToolbarItem | WidgetKind::Sidebar)
                        })
                        .count();
                    let Some(index) = index.checked_sub(content) else {
                        violation(command, "toolbar items go after the window's content")
                    };
                    let Widget::Window { root } = &self.nodes[parent].widget else {
                        violation(command, "toolbar items go in windows")
                    };
                    let page = root.window.child("mitsuamiPage").expect("windows have a page");
                    page.set_object("mitsuamiAction", Some(action));
                    page.set_int("mitsuamiIndex", index as i32);
                    page.invoke("mitsuamiInsert");
                    root.toolbar.borrow_mut().insert(index, *child);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if let Widget::List(list) = &self.nodes[parent].widget {
                    let Some(row) = self.nodes[child].row else {
                        violation(command, "a List's children are row hosts (Containers with a Prop::Row)")
                    };
                    list.insert(row, *child, item);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                let parent_widget = self.widget(*parent, command);
                let content = parent_widget.content();
                if matches!(parent_widget, Widget::Scroll { .. }) && !content.child_items().is_empty() {
                    violation(command, "a ScrollView has a single native child (its content)");
                }
                if matches!(parent_widget, Widget::Tabs { .. }) && !matches!(self.nodes[child].widget, Widget::Host(_))
                {
                    violation(command, "a Tabs' children are page hosts (Containers)");
                }
                item.set_parent_item(Some(content), *index);
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                match &self.nodes[parent].widget {
                    Widget::Scroll { flickable, .. } => sync_scroll(*flickable),
                    // Shown if it's the page chosen, hidden if not.
                    Widget::Tabs { root, .. } => {
                        root.invoke("mitsuamiShow");
                    }
                    _ => {}
                }
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                match (&self.nodes[parent].widget, &self.nodes[child].widget) {
                    (Widget::List(list), _) => list.remove(self.nodes[child].row.expect("inserted with a row")),
                    (Widget::Window { root }, Widget::Sidebar { .. }) => hide_sidebar(root),
                    // Out of the page area, shown again wherever it goes.
                    (Widget::Tabs { root, .. }, child) => {
                        let item = child.item();
                        item.set_parent_item(None, 0);
                        item.set_bool("visible", true);
                        root.invoke("mitsuamiShow");
                    }
                    (Widget::Window { root }, Widget::ToolbarItem { host, action }) => {
                        // Out of the toolbar's item first, which goes with
                        // the action.
                        host.set_parent_item(None, 0);
                        let page = root.window.child("mitsuamiPage").expect("windows have a page");
                        page.set_object("mitsuamiAction", Some(*action));
                        page.invoke("mitsuamiRemove");
                        page.set_object("mitsuamiAction", None);
                        root.toolbar.borrow_mut().retain(|i| i != child);
                    }
                    _ => self.widget(*child, command).item().set_parent_item(None, 0),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                self.pending_show.retain(|w| w != id);
                self.menus.forget(*id);
                // Not the item's child: a popup only has it as its parent.
                if let Some(menu) = &node.context_menu {
                    menu.delete_later();
                }
                if let Some(menu) = &node.button_menu {
                    menu.delete_later();
                }
                match &node.widget {
                    Widget::Window { root } => {
                        // The menu drawer isn't the window's child: it
                        // would outlive it, bound to a parent that's gone.
                        if let Some(drawer) = root.drawer.take() {
                            drawer.destroy();
                        }
                        root.window.destroy()
                    }
                    Widget::ToolbarItem { host, action } => {
                        host.destroy();
                        action.destroy();
                    }
                    // Out of its window's page row first, if it's still in
                    // it (the window goes too).
                    Widget::Sidebar { page, .. } => {
                        if let Some(Widget::Window { root }) =
                            node.parent.and_then(|p| self.nodes.get(&p)).map(|n| &n.widget)
                        {
                            hide_sidebar(root);
                        }
                        page.destroy();
                    }
                    // The app's handle may keep its surface: it just stops
                    // showing.
                    Widget::GpuSurface(surface) => {
                        surface.detach();
                        surface.item.destroy();
                    }
                    widget => widget.item().destroy(),
                }
            }
            Command::SetFrame { id, frame } => {
                // A toolbar item: its size; the toolbar places it, and
                // doesn't show it while it's empty.
                if let Widget::ToolbarItem { host, action } = self.widget(*id, command) {
                    host.set_geometry(0.0, 0.0, frame.width() as f64, frame.height() as f64);
                    action.set_bool("visible", !frame.size.is_empty());
                    return;
                }
                let widget = self.widget(*id, command);
                let item = widget.item();
                let parent = self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget);
                // A tab view's page: its size; it's at the top-left of the
                // page area.
                let at = match parent {
                    Some(Widget::Tabs { .. }) => Point::ZERO,
                    _ => frame.origin,
                };
                item.set_geometry(at.x as f64, at.y as f64, frame.width() as f64, frame.height() as f64);
                // Leaves with an empty frame (hidden, or not laid out yet)
                // aren't shown: controls draw their frames regardless of size.
                if widget.is_leaf() {
                    item.set_bool("visible", !frame.size.is_empty());
                }
                if let Some(Widget::List(list)) = parent {
                    list.row_measured(frame.height());
                }
                let flickable = match widget {
                    Widget::Scroll { flickable, .. } => Some(*flickable),
                    _ => self.scroll_parent(*id),
                };
                if let Some(flickable) = flickable {
                    sync_scroll(flickable);
                }
            }
            Command::SetA11y { id, a11y } => {
                let item = self.widget(*id, command).item();
                let A11yProps { label, description, hidden, .. } = a11y;
                let node = &self.nodes[id];
                // A switch's or select's accessible name is its label prop.
                let named_by_label = matches!(
                    node.widget,
                    Widget::Switch(_)
                        | Widget::Select(_)
                        | Widget::RadioGroup(_)
                        | Widget::Slider(_)
                        | Widget::NumberInput(_)
                        | Widget::Progress(_)
                        | Widget::Spinner(_)
                        | Widget::Icon(_)
                        | Widget::Image { .. }
                        | Widget::GpuSurface(_)
                );
                if !named_by_label || label.is_some() {
                    item.set_str("mitsuamiA11yName", label.as_deref().unwrap_or_default());
                }
                item.set_str("mitsuamiA11yDescription", description.as_deref().unwrap_or_default());
                item.set_bool("mitsuamiA11yHidden", *hidden);
            }
            // A window in full screen keeps the screen's size.
            Command::SetWindowSize { id, size } => {
                let root = self.window_root(*id, command);
                if !root.in_full_screen() {
                    root.resize_to(*size);
                }
            }
            Command::SetFocusOrder { window, order } => {
                // A tab view's bar, not its pages, whose controls follow it.
                let items: Vec<QmlObject> = order
                    .iter()
                    .map(|id| match self.widget(*id, command) {
                        widget @ Widget::Tabs { .. } => widget.input_item(),
                        widget => widget.item(),
                    })
                    .collect();
                let root = self.window_root(*window, command);
                root.window.set_tab_order(&items);
                // Qt Quick focuses nothing in a new window, so keys went
                // nowhere until a click or Tab. AppKit focuses its initial
                // first responder and GTK its first control; so does this.
                if !root.focused_first.get() {
                    let first = order
                        .iter()
                        .map(|id| self.widget(*id, command))
                        .find(|w| w.is_focusable() && (!w.is_control() || w.item().bool("enabled")));
                    if root.window.focus_item().and_then(|f| f.node()).is_some() {
                        root.focused_first.set(true);
                    } else if let Some(widget) = first {
                        widget.input_item().force_focus();
                        root.focused_first.set(true);
                    }
                }
            }
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll { flickable, .. }) => {
                    // Qt reports both through `contentX/YChanged`.
                    flickable.set_real("contentX", offset.x as f64);
                    flickable.set_real("contentY", offset.y as f64);
                }
                Some(Widget::List(list)) => list.scroll_to(*offset),
                _ => violation(command, "not a ScrollView or List"),
            },
            Command::Focus { id } => {
                let widget = self.widget(*id, command);
                if widget.is_focusable() {
                    widget.input_item().force_focus();
                }
            }
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row),
                _ => violation(command, "not a List"),
            },
        }
    }
}

/// Takes a window's sidebar out of its page row: the content's page is
/// titled after the window again, and the content keeps its size.
fn hide_sidebar(root: &WindowRoot) {
    let Some((_, page)) = root.sidebar.take() else { return };
    page.set_object("mitsuamiContent", None);
    root.window.invoke("mitsuamiHideSidebar");
    if let Some(content) = root.window.child("mitsuamiPage") {
        content.set_str("title", &root.window.str("title"));
    }
    let size = root.size.get();
    if !size.is_empty() {
        root.resize_to(size);
    }
}

/// A sidebar's sections as the JSON its page reads (`qml::sidebar`).
fn sections_json(sections: &[SidebarSectionData]) -> String {
    fn string(s: &str) -> String {
        let mut out = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    let optional = |s: &Option<String>| s.as_deref().map_or("null".to_owned(), string);
    let sections: Vec<String> = sections
        .iter()
        .map(|section| {
            let items: Vec<String> = section
                .items
                .iter()
                .map(|item| format!(r#"{{"title":{},"icon":{}}}"#, string(&item.title), optional(&item.icon)))
                .collect();
            format!(r#"{{"title":{},"items":[{}]}}"#, optional(&section.title), items.join(","))
        })
        .collect();
    format!("[{}]", sections.join(","))
}

/// A tab view's strip: the bar it shows.
fn strip(tabs: QmlObject) -> QmlObject {
    tabs.object("mitsuamiStrip").expect("tab views have a strip")
}

/// A tab view's titles, as its tabs show them.
fn tab_titles(tabs: QmlObject) -> Vec<String> {
    if tabs.int("mitsuamiCount") == 0 {
        return Vec::new();
    }
    tabs.str("mitsuamiShownTitles").split('\u{1f}').map(str::to_owned).collect()
}

/// The options a radio group has buttons for.
fn radio_options(group: QmlObject) -> Vec<String> {
    if group.int("mitsuamiCount") == 0 {
        return Vec::new();
    }
    group.str("mitsuamiOptionTexts").split('\u{1f}').map(str::to_owned).collect()
}

/// Sets a text area's text as the backend: its `textChanged` doesn't report
/// an edit.
fn set_area_text(root: QmlObject, text: &str) {
    root.set_bool("mitsuamiSetting", true);
    root.set_str("text", text);
    root.set_bool("mitsuamiSetting", false);
}

/// A select's options, as it shows them.
fn option_texts(select: QmlObject) -> Vec<String> {
    if select.int("count") == 0 {
        return Vec::new();
    }
    select.str("mitsuamiOptionTexts").split('\u{1f}').map(str::to_owned).collect()
}

/// Implicit sizes, except text: it wraps to the space it's offered, down to
/// its longest word.
fn measure_item(item: QmlObject, wraps: bool, request: MeasureRequest) -> Size {
    let natural = Size::new(item.real("implicitWidth") as f32, item.real("implicitHeight") as f32);
    if !wraps {
        return Size::new(
            request.known_width.unwrap_or(natural.width.ceil()),
            request.known_height.unwrap_or(natural.height.ceil()),
        );
    }
    // Word-wrapped at width 1, a label is as wide as its longest word.
    let frame_width = item.real("width");
    let min_content = || {
        item.set_real("width", 1.0);
        item.real("contentWidth").ceil() as f32
    };
    let width = match (request.known_width, request.available_width) {
        (Some(known), _) => known,
        (None, AvailableSpace::MaxContent) => natural.width.ceil(),
        (None, AvailableSpace::MinContent) => min_content(),
        (None, AvailableSpace::Definite(available)) => {
            let natural = natural.width.ceil();
            if natural <= available { natural } else { available.floor().max(min_content()) }
        }
    };
    // An eliding label also drops the lines past its height, and its
    // frame's may be less than it needs (0 before its first layout).
    let frame_height = item.real("height");
    item.set_real("height", f32::MAX as f64);
    item.set_real("width", width as f64);
    let height = item.real("implicitHeight").ceil() as f32;
    item.set_real("width", frame_width);
    item.set_real("height", frame_height);
    Size::new(width, request.known_height.unwrap_or(height))
}

impl Backend for KirigamiBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow().events.set_sink(events);
    }

    fn metrics(&self) -> PlatformMetrics {
        theme::metrics()
    }

    /// Below its strip, which is as high as the bar it shows: the
    /// metrics' are Qt's tab bar's.
    fn tab_insets(&self, id: NodeId) -> Option<mitsuami_core::Insets> {
        let state = self.state.borrow();
        let Some(Widget::Tabs { root, .. }) = state.nodes.get(&id).map(|n| &n.widget) else { return None };
        let bar = strip(*root);
        root.invoke("mitsuamiPolishStrip");
        Some(mitsuami_core::Insets::new(bar.real("implicitHeight").ceil() as f32, 0.0, 0.0, 0.0))
    }

    fn apply(&mut self, batch: &[Command]) {
        let mut state = self.state.borrow_mut();
        let events = state.events.clone();
        events.muted(|| {
            for command in batch {
                if state.options.record_commands {
                    state.log.push(command.clone());
                }
                state.apply(command);
            }
        });
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        match &node.widget {
            Widget::Custom { item, render, props } => {
                render.measure(*item, props.props(), &request).unwrap_or_else(|| measure_item(*item, false, request))
            }
            Widget::Native { item, measure: Some(measure), .. } => measure(*item, &request),
            // Pixels are as large as they say, over their scale; files as
            // Qt reads them (nothing when missing or unreadable).
            Widget::Image { source: Some(ImageSource::Pixels(pixels)), .. } => {
                let natural = pixels.size();
                Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
            }
            // As large as the layout makes it.
            Widget::GpuSurface(_) => Size::new(request.known_width.unwrap_or(0.0), request.known_height.unwrap_or(0.0)),
            // With no page: its bar's size. Qt sizes a bar's tabs when it
            // polishes it, before a frame.
            Widget::Tabs { root, .. } => {
                let bar = strip(*root);
                root.invoke("mitsuamiPolishStrip");
                Size::new(
                    request.known_width.unwrap_or(root.real("mitsuamiStripWidth").ceil() as f32),
                    request.known_height.unwrap_or(bar.real("implicitHeight").ceil() as f32),
                )
            }
            // Its buttons, down the column: a layout sizes itself when
            // it's polished, before a frame.
            Widget::RadioGroup(group) => {
                group.invoke("ensurePolished");
                measure_item(*group, false, request)
            }
            // Empty, with its title: the box's own implicit size, which is
            // at least its title's width and its paddings.
            Widget::Group { group, .. } => {
                group.invoke("ensurePolished");
                Size::new(
                    request.known_width.unwrap_or(group.real("implicitWidth").ceil() as f32),
                    request.known_height.unwrap_or(group.real("implicitHeight").ceil() as f32),
                )
            }
            // Measured by the core, or never (the sidebar is the window's).
            Widget::Drawn { .. }
            | Widget::Window { .. }
            | Widget::Host(_)
            | Widget::Scroll { .. }
            | Widget::List(_)
            | Widget::Sidebar { .. } => Size::ZERO,
            widget => measure_item(widget.item(), matches!(widget, Widget::Label(_)), request),
        }
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        match action {
            A11yAction::ContextMenuItem(item) => return self.choose_menu_item(id, *item, false),
            A11yAction::MenuItem(item) => return self.choose_menu_item(id, *item, true),
            _ => {}
        }
        // A list's rows: select or activate them, as a click or a double
        // click on their delegate does.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                match action {
                    A11yAction::Select if list.mode() != SelectionMode::None => list.select(row),
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
        }
        let (item, input, kind, focusable, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let item = node.widget.item();
            // Qt's controls accept actions while disabled and do nothing.
            if node.widget.is_control() && !item.bool("enabled") {
                return Err(ActionError::Disabled);
            }
            let custom = match &node.widget {
                Widget::Custom { render, props, .. } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            (item, node.widget.input_item(), node.kind, node.widget.is_focusable(), state.events.clone(), custom)
        };
        if let Some((render, props)) = custom {
            return render.perform(item, &props, action, &Emitter::new(events, id));
        }
        // No state borrow below: Qt calls back into our signal handlers.
        match (action, kind) {
            // What a screen reader does, through the items' accessible
            // actions. Like a click, they focus the control.
            (A11yAction::Activate, WidgetKind::Button) => {
                if !item.accessible_action("Press") {
                    return Err(ActionError::Unsupported);
                }
            }
            // A checkable button's action is Toggle, or Press where Qt
            // gives it only that.
            (A11yAction::Activate, WidgetKind::ToggleButton) => {
                if !item.accessible_action("Toggle") && !item.accessible_action("Press") {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Activate, WidgetKind::Checkbox | WidgetKind::Switch) => {
                if !item.accessible_action("Toggle") {
                    return Err(ActionError::Unsupported);
                }
            }
            // What the arrow keys do.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Slider | WidgetKind::NumberInput) => {
                item.set_int("mitsuamiStepBy", if *action == A11yAction::Increment { 1 } else { -1 });
            }
            (A11yAction::SetValue(text), WidgetKind::Slider | WidgetKind::NumberInput) => {
                item.set_real("mitsuamiMoveTo", text.trim().parse().map_err(|_| ActionError::Unsupported)?);
            }
            // As if the item were clicked.
            (A11yAction::SetValue(title), WidgetKind::Sidebar) => {
                let index = {
                    let state = self.state.borrow();
                    match state.nodes.get(&id).map(|n| &n.widget) {
                        Some(Widget::Sidebar { sections, .. }) => {
                            sections.iter().flat_map(|s| &s.items).position(|i| i.title == *title)
                        }
                        _ => None,
                    }
                };
                item.set_int("mitsuamiChoice", index.ok_or(ActionError::Unsupported)? as i32);
            }
            // As if its tab were clicked.
            (A11yAction::SetValue(title), WidgetKind::Tabs) => {
                let index = tab_titles(item).iter().position(|t| t == title).ok_or(ActionError::Unsupported)?;
                item.set_int("mitsuamiChoice", index as i32);
            }
            // As if its button were clicked.
            (A11yAction::SetValue(text), WidgetKind::RadioGroup) => {
                let index = radio_options(item).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                item.set_int("mitsuamiChoice", index as i32);
            }
            // As if the option were picked from the pop-up.
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let index = option_texts(item).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                item.set_int("mitsuamiChoice", index as i32);
            }
            (A11yAction::SetValue(text), WidgetKind::TextInput | WidgetKind::PasswordInput) => {
                if item.bool("readOnly") {
                    return Err(ActionError::ReadOnly);
                }
                item.set_str("text", text);
                // The caret ends up after the new text, as if it was typed.
                item.set_int("cursorPosition", text.chars().count() as i32);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            // Searched for at once, as a user's edit that's done; Kirigami's
            // own search for it isn't reported.
            (A11yAction::SetValue(text), WidgetKind::SearchInput) => {
                item.set_str("mitsuamiShown", text);
                item.set_str("text", text);
                item.set_int("cursorPosition", text.chars().count() as i32);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                events.emit(id, UiEvent::Search(text.clone()));
            }
            (A11yAction::SetValue(text), WidgetKind::TextArea) => {
                if item.bool("readOnly") {
                    return Err(ActionError::ReadOnly);
                }
                // One edit, reported once, the caret after it.
                set_area_text(item, text);
                input.set_int("cursorPosition", text.chars().count() as i32);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            // Native views: the item's own accessible actions.
            (A11yAction::Activate, WidgetKind::Native) => {
                if !item.accessible_action("Press") && !item.accessible_action("Toggle") {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Native) => {
                let up = *action == A11yAction::Increment;
                if !item.accessible_action(if up { "Increase" } else { "Decrease" }) {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Focus, _) => {
                if !focusable {
                    return Err(ActionError::Unsupported);
                }
                // The window's focus observer reports the change.
                input.force_focus();
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }

    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        let (widget_item, kind, window) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let mut window_id = id;
            while let Some(parent) = state.nodes.get(&window_id).and_then(|n| n.parent) {
                window_id = parent;
            }
            let window = match state.nodes.get(&window_id).map(|n| &n.widget) {
                Some(Widget::Window { root }) => Some(root.window),
                _ => None,
            };
            if node.widget.is_control() && !node.widget.item().bool("enabled") {
                return Err(ActionError::Disabled);
            }
            (node.widget.input_item(), node.kind, window)
        };
        if kind == WidgetKind::GpuSurface {
            return self.surface_input(id, widget_item, window, input);
        }
        // The drop area's own handling, with the state let go: its reports
        // may wake the run loop.
        if let SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) = input {
            let drop = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                node.file_drop.as_ref().map(|area| area.input()).ok_or(ActionError::Unsupported)?
            };
            match input {
                SyntheticInput::DragFiles(paths) => drop.enter(paths),
                SyntheticInput::DragLeave => drop.leave(),
                SyntheticInput::DropFiles(paths) => drop.dropped(paths),
                _ => unreachable!(),
            }
            return Ok(());
        }
        match input {
            // Handled above.
            SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) => unreachable!(),
            SyntheticInput::Click(point) => {
                // Drawn widgets only: their pointer handling is ours.
                let drawn = matches!(self.state.borrow().nodes.get(&id).map(|n| &n.widget), Some(Widget::Drawn { .. }));
                let window = window.filter(|_| drawn).ok_or(ActionError::Unsupported)?;
                window.click(widget_item.map_to_scene(*point));
                Ok(())
            }
            SyntheticInput::Scroll { dy, .. } if kind == WidgetKind::List => {
                // `ListView`'s content starts at `originY`.
                let view = widget_item;
                let (origin, max) = (view.real("originY"), (view.real("contentHeight") - view.real("height")).max(0.0));
                let offset = (view.real("contentY") - origin + *dy as f64).clamp(0.0, max);
                if offset >= max && *dy > 0.0 {
                    // Rows not shown yet are estimates: Qt goes to the real end.
                    view.invoke("positionViewAtEnd");
                } else {
                    view.set_real("contentY", offset + origin);
                }
                Ok(())
            }
            SyntheticInput::Scroll { dx, dy } => {
                if kind != WidgetKind::ScrollView {
                    return Err(ActionError::Unsupported);
                }
                let axes = self.state.borrow().nodes[&id].scroll_axes.unwrap_or(ScrollAxes::Vertical);
                let flickable = widget_item;
                let step = |offset: &str, content: &str, view: &str, by: f32, on: bool| {
                    if on {
                        let max = (flickable.real(content) - flickable.real(view)).max(0.0);
                        flickable.set_real(offset, (flickable.real(offset) + by as f64).clamp(0.0, max));
                    }
                };
                step("contentX", "contentWidth", "width", *dx, axes.horizontal());
                step("contentY", "contentHeight", "height", *dy, axes.vertical());
                Ok(())
            }
            SyntheticInput::Key(key) => match (kind, key) {
                // Real key events, through the list view's own keyboard
                // navigation (and ours for Home, End and Return).
                (WidgetKind::List, Key::Up | Key::Down | Key::Home | Key::End | Key::Enter) => {
                    let window = window.ok_or(ActionError::Unsupported)?;
                    if self
                        .state
                        .borrow()
                        .nodes
                        .get(&id)
                        .map(|n| &n.widget)
                        .is_some_and(|w| matches!(w, Widget::List(list) if list.mode() == SelectionMode::None))
                    {
                        return Err(ActionError::Unsupported);
                    }
                    widget_item.force_focus();
                    let (code, text) = match key {
                        Key::Up => (KEY_UP, ""),
                        Key::Down => (KEY_DOWN, ""),
                        Key::Home => (KEY_HOME, ""),
                        Key::End => (KEY_END, ""),
                        _ => (KEY_RETURN, "\r"),
                    };
                    key_in(window, code, text);
                    Ok(())
                }
                // Qt's text area takes Return as a new line and Tab as a
                // tab, as real keys.
                (
                    WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea,
                    _,
                ) => {
                    // It would take the keys and ignore them; nothing can be
                    // typed into it on any platform.
                    if widget_item.bool("readOnly") {
                        return Err(ActionError::ReadOnly);
                    }
                    // Real key events, through Qt's text editing.
                    let window = window.ok_or(ActionError::Unsupported)?;
                    if window.focus_item().and_then(|f| f.node()) != Some(node_key(id)) {
                        widget_item.force_focus();
                        // Typing appends, as after clicking past the end.
                        widget_item.set_int("cursorPosition", widget_item.str("text").chars().count() as i32);
                    }
                    let (code, text) = match key {
                        Key::Char(c) => {
                            let code = if c.is_ascii_alphanumeric() || *c == ' ' {
                                c.to_ascii_uppercase() as i32
                            } else {
                                KEY_UNKNOWN
                            };
                            (code, c.to_string())
                        }
                        Key::Backspace => (KEY_BACKSPACE, String::new()),
                        Key::Enter => (KEY_RETURN, "\r".into()),
                        Key::Tab => (KEY_TAB, "\t".into()),
                        Key::Escape => (KEY_ESCAPE, "\u{1b}".into()),
                        Key::Up => (KEY_UP, String::new()),
                        Key::Down => (KEY_DOWN, String::new()),
                        Key::Home => (KEY_HOME, String::new()),
                        Key::End => (KEY_END, String::new()),
                    };
                    key_in(window, code, &text);
                    Ok(())
                }
                (WidgetKind::Button, Key::Enter | Key::Char(' '))
                | (WidgetKind::ToggleButton | WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => {
                    self.perform(id, &A11yAction::Activate)
                }
                // A real Escape, from the node if it takes focus: a modal
                // window's shortcut asks it to close.
                (_, Key::Escape) => {
                    let window = window.ok_or(ActionError::Unsupported)?;
                    if self.state.borrow().nodes.get(&id).is_some_and(|n| n.widget.is_control()) {
                        widget_item.force_focus();
                    }
                    key_in(window, KEY_ESCAPE, "\u{1b}");
                    Ok(())
                }
                _ => Err(ActionError::Unsupported),
            },
        }
    }

    fn native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        let mut props = Vec::new();
        let item = node.widget.item();
        match &node.widget {
            Widget::Window { root } => {
                props.push(Prop::Title(root.window.str("title")));
                props.push(Prop::FullScreen(root.in_full_screen()));
                props.push(Prop::MinSize(root.min_size()));
                props.push(Prop::HeightFollowsContent(root.height_locked()));
                props.push(Prop::Maximized(root.is_maximized()));
                props.push(Prop::Resizable(!root.width_fixed()));
                // The modality as Qt has it; the owner as the node has it.
                if let Some((owner, modality)) = node.modal {
                    let modality = match root.window.int("modality") {
                        1 => Modality::Window,
                        2 => Modality::Application,
                        _ => modality,
                    };
                    props.push(Prop::Modal { owner, modality });
                }
            }
            Widget::Label(l) => {
                props.push(Prop::Text(l.str("text")));
                props.push(Prop::Selectable(l.bool("mitsuamiSelectable")));
                let lines = l.int("maximumLineCount");
                props.push(Prop::MaxLines((lines != i32::MAX).then_some(lines as u32)));
                props
                    .extend(qml::color_from(l.int("mitsuamiColor"), l.int("mitsuamiRgba") as u32).map(Prop::TextColor));
                props.push(Prop::FontWeight(qml::font_weight_from(l.int("mitsuamiShownWeight"))));
                props.push(Prop::Italic(l.bool("mitsuamiShownItalic")));
                let align = l.int("effectiveHorizontalAlignment");
                props.push(Prop::TextAlign(match align {
                    ALIGN_RIGHT => HorizontalAlign::Right,
                    ALIGN_H_CENTER => HorizontalAlign::Center,
                    _ => HorizontalAlign::Left,
                }));
            }
            Widget::Field(f) | Widget::TextArea { root: f, .. } => {
                props.push(Prop::Value(f.str("text")));
                props.push(Prop::Placeholder(f.str("placeholderText")));
                props.push(Prop::ReadOnly(f.bool("readOnly")));
                if let Widget::TextArea { root, area } = &node.widget {
                    props.push(Prop::Lines(root.int("mitsuamiLines") as u32));
                    props.push(Prop::LineWrap(area.int("wrapMode") != TEXT_EDIT_NO_WRAP));
                } else if node.kind == WidgetKind::TextInput {
                    let hints = f.int("inputMethodHints");
                    let shown = [InputPurpose::Phone, InputPurpose::Email, InputPurpose::Url]
                        .into_iter()
                        .find(|p| hints & purpose_hint(*p) != 0);
                    props.push(Prop::InputPurpose(shown.unwrap_or_default()));
                }
            }
            Widget::Button(b) | Widget::MenuButton(b) => {
                props.push(Prop::Label(b.str("text")));
                if node.kind == WidgetKind::ToggleButton {
                    props.push(Prop::Checked(b.bool("checked")));
                }
                props.extend(node.button_menu.as_ref().map(|menu| Prop::Menu(menu.shown())));
                props.push(Prop::Icon(b.str("mitsuamiShownIcon")));
                if node.icon_only {
                    // Only with an icon: without, it shows its text.
                    let icon = !b.str("mitsuamiShownIcon").is_empty();
                    props
                        .push(Prop::IconOnly(b.bool("mitsuamiShownIconOnly") || (!icon && b.bool("mitsuamiIconOnly"))));
                }
            }
            Widget::Checkbox(c) => {
                props.push(Prop::Label(c.str("text")));
                let mixed = c.int("checkState") == PARTIALLY_CHECKED;
                props.push(Prop::Checked(if mixed { node.checked } else { c.bool("checked") }));
                if node.mixed.is_some() {
                    props.push(Prop::Mixed(mixed));
                }
            }
            Widget::Switch(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Checked(s.bool("checked")));
            }
            Widget::Slider(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Range { min: s.real("from"), max: s.real("to") });
                let step = s.real("stepSize");
                props.push(Prop::Step((step > 0.0).then_some(step)));
                props.push(Prop::Number(s.real("value")));
                if node.orientation.is_some() {
                    props.push(Prop::Orientation(if s.int("orientation") == QT_VERTICAL {
                        Orientation::Vertical
                    } else {
                        Orientation::Horizontal
                    }));
                }
            }
            Widget::NumberInput(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Range { min: s.int("from").into(), max: s.int("to").into() });
                props.push(Prop::Step(Some(s.int("stepSize").into())));
                props.push(Prop::WrapAround(s.bool("wrap")));
                props.push(Prop::Number(s.int("value").into()));
            }
            Widget::Progress(p) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Progress((!p.bool("indeterminate")).then(|| p.real("value"))));
            }
            Widget::Spinner(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Running(s.bool("running")));
            }
            Widget::Separator(_) => props.extend(node.orientation.map(Prop::Orientation)),
            Widget::Icon(i) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Icon(i.str("mitsuamiShownName")));
                props
                    .extend(qml::color_from(i.int("mitsuamiColor"), i.int("mitsuamiRgba") as u32).map(Prop::TextColor));
                // Its size, which it takes no room at without a name.
                if node.icon_size {
                    let shown = if i.str("mitsuamiName").is_empty() { "mitsuamiSize" } else { "implicitWidth" };
                    props.push(Prop::IconSize(i.real(shown) as f32));
                }
            }
            Widget::Image { source, fit, .. } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(source.clone().map(Prop::Image));
                props.extend(fit.map(Prop::ImageFit));
            }
            Widget::GpuSurface(surface) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let (takes, locked, grabbed, cursor) = surface.props();
                props.extend(takes.map(Prop::TakesInput));
                props.extend(locked.map(Prop::PointerLock));
                props.extend(grabbed.map(Prop::KeyboardGrab));
                props.extend(cursor.map(Prop::Cursor));
            }
            Widget::RadioGroup(g) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Options(radio_options(*g)));
                g.invoke("mitsuamiReadShown");
                props.push(Prop::SelectedIndex(usize::try_from(g.int("mitsuamiShown")).ok()));
            }
            Widget::Select(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Options(option_texts(*s)));
                props.push(Prop::SelectedIndex(usize::try_from(s.int("currentIndex")).ok()));
            }
            Widget::Scroll { view, .. } => {
                props.extend(node.scroll_axes.map(Prop::ScrollAxes));
                props.push(Prop::ScrollBars(view.bool("mitsuamiBars")));
            }
            Widget::Custom { item, render, props: last } => {
                props.push(Prop::Custom(last.with_props(render.read(*item, last.props()))))
            }
            Widget::Drawn { props: last, drawing, .. } => {
                props.push(Prop::Custom(last.clone()));
                props.push(Prop::Drawing(drawing.clone()));
            }
            Widget::Native { last, .. } => props.push(Prop::Native(last.clone())),
            Widget::List(list) => {
                props.push(Prop::Rows(list.rows()));
                props.extend(list.estimate().map(Prop::EstimatedRowHeight));
                props.push(Prop::SelectionMode(list.mode()));
                props.extend(list.style().map(Prop::ListStyle));
                props.push(Prop::Selected(list.selected()));
            }
            Widget::Host(_) => props.extend(node.row.map(Prop::Row)),
            Widget::ToolbarItem { .. } => {}
            Widget::Sidebar { page, sections } => {
                props.push(Prop::Sections(sections.clone()));
                props.push(Prop::SelectedIndex(usize::try_from(page.int("mitsuamiSelected")).ok()));
                // As its window's row has it, or the app wants it before.
                let root = node.parent.and_then(|p| state.nodes.get(&p)).and_then(|p| match &p.widget {
                    Widget::Window { root } => Some(root.window),
                    _ => None,
                });
                props.push(Prop::SidebarShown(match root {
                    Some(window) => window.bool("mitsuamiSidebarIn"),
                    None => page.bool("mitsuamiShown"),
                }));
            }
            Widget::Group { group, .. } => props.push(Prop::Title(group.str("title"))),
            Widget::Tabs { root, .. } => {
                props.push(Prop::TabTitles(tab_titles(*root)));
                props.push(Prop::TabIcons(root.str("mitsuamiShownIcons").split('\u{1f}').map(str::to_owned).collect()));
                props.push(Prop::SelectedIndex(usize::try_from(strip(*root).int("currentIndex")).ok()));
                // Which strip it shows; `Automatic` is the navigation bar.
                let navigation = root.bool("mitsuamiNavigation");
                props.extend(
                    node.tabs_style
                        .map(|chosen| match (chosen, navigation) {
                            (TabsStyle::TabBar, false) | (TabsStyle::Automatic | TabsStyle::Navigation, true) => chosen,
                            (_, true) => TabsStyle::Navigation,
                            (_, false) => TabsStyle::TabBar,
                        })
                        .map(Prop::TabsStyle),
                );
            }
        }
        if node.widget.is_control() {
            props.push(Prop::Enabled(item.bool("enabled")));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        if node.file_drop_given {
            props.push(Prop::FileDrop(node.file_drop.as_ref().map(|area| area.drop_value())));
        }
        props.push(Prop::Tooltip(if node.widget.has_tooltip() {
            node.widget.item().str("mitsuamiTooltip")
        } else {
            node.tooltip.clone()
        }));
        props.extend(node.context_menu.as_ref().map(|menu| Prop::ContextMenu(menu.shown())));
        let frame = match &node.widget {
            Widget::Window { root } => {
                let size = root.size.get();
                Rect::new(0.0, 0.0, size.width, size.height)
            }
            _ => frame_of(item),
        };
        // A row is where the list view put it, a toolbar item where the
        // toolbar did: above the content, in its coordinates.
        let frame = match (node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget), &node.widget) {
            (Some(Widget::List(list)), _) => list.row_rect(item, frame),
            (Some(Widget::Window { root }), Widget::ToolbarItem { host, action }) => {
                if !action.bool("visible") || host.object("parent").is_none() {
                    Rect::ZERO
                } else {
                    let (at, origin) = (host.map_to_scene(Point::ZERO), root.host.map_to_scene(Point::ZERO));
                    Rect::new(at.x - origin.x, at.y - origin.y, frame.width(), frame.height())
                }
            }
            // A sidebar's page is beside the content (at negative x), where
            // the page row shows it.
            (Some(Widget::Window { root }), Widget::Sidebar { page, .. }) => {
                if !root.window.bool("mitsuamiSidebarIn") || !page.bool("visible") || page.real("width") <= 0.0 {
                    Rect::ZERO
                } else {
                    let (at, origin) = (page.map_to_scene(Point::ZERO), root.host.map_to_scene(Point::ZERO));
                    Rect::new(at.x - origin.x, at.y - origin.y, frame.width(), frame.height())
                }
            }
            // A tab view's page is in its page area, below the bar, while
            // it's the one shown.
            (Some(Widget::Tabs { root, .. }), _) => {
                if !item.bool("visible") {
                    Rect::ZERO
                } else {
                    let (at, origin) = (item.map_to_scene(Point::ZERO), root.map_to_scene(Point::ZERO));
                    Rect::new(at.x - origin.x, at.y - origin.y, frame.width(), frame.height())
                }
            }
            _ => frame,
        };
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => (Vec::new(), Some(list.scroll_offset())),
            Widget::Scroll { flickable, .. } => (node.widget.content().child_items(), Some(scroll_offset(*flickable))),
            Widget::Window { .. } | Widget::Host(_) | Widget::ToolbarItem { .. } => (item.child_items(), None),
            Widget::Tabs { pages, .. } => (pages.child_items(), None),
            Widget::Group { content, .. } => (content.child_items(), None),
            _ => (Vec::new(), None),
        };
        // Items that stand for nodes themselves: `node()` walks up the tree.
        let own = node_key(id);
        let mut children: Vec<NodeId> = match &node.widget {
            Widget::List(list) => list.children(),
            _ => children.iter().filter_map(|c| c.node()).filter(|key| *key != own).map(node_from_key).collect(),
        };
        // A window's toolbar items come after its content, and its sidebar
        // after them.
        if let Widget::Window { root } = &node.widget {
            children.extend(root.toolbar.borrow().iter().copied());
            children.extend(root.sidebar.get().map(|(id, _)| id));
        }
        let window = {
            let mut top = id;
            while let Some(parent) = state.nodes.get(&top).and_then(|n| n.parent) {
                top = parent;
            }
            match state.nodes.get(&top).map(|n| &n.widget) {
                Some(Widget::Window { root }) => Some(root.window),
                _ => None,
            }
        };
        let focused = !matches!(node.widget, Widget::Window { .. })
            && window.and_then(|w| w.focus_item()).and_then(|f| f.node()) == Some(node_key(id));
        Some(NativeState { kind: node.kind, props, frame, parent: node.parent, children, focused, scroll_offset })
    }

    fn services(&self) -> Box<dyn mitsuami_core::services::Services> {
        Box::new(KirigamiServices::new(self.handle()))
    }

    /// Qt's own: the desktop file name, the display name (after each
    /// window's title, as KDE apps show theirs) and the window icon.
    fn set_app_info(&mut self, info: &AppInfo) {
        let icon = info.icon.as_ref().and_then(|icon| icon.read());
        ffi::set_app_info(info.id.as_deref(), info.name.as_deref(), icon.as_deref());
    }

    /// Renders the window's scene right away, and crops it to the node.
    fn capture(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        let target = {
            let state = self.state.borrow();
            let Some(node) = state.nodes.get(&id) else { return reply(Err(CaptureError::UnknownNode)) };
            let mut top = id;
            while let Some(parent) = state.nodes.get(&top).and_then(|n| n.parent) {
                top = parent;
            }
            let window = match state.nodes.get(&top).map(|n| &n.widget) {
                Some(Widget::Window { root }) => root.window,
                _ => return reply(Err(CaptureError::Failed("the node is not in a window".into()))),
            };
            let item = node.widget.item();
            let size = match &node.widget {
                Widget::Window { root } => root.size.get(),
                _ => size_of(item),
            };
            let origin = item.map_to_scene(Point::new(0.0, 0.0));
            (window, Rect::new(origin.x, origin.y, size.width, size.height))
        };
        let (window, rect) = target;
        reply(match window.grab(Some(rect)) {
            Some((rgba, width, height, scale_factor)) => Ok(Image { width, height, scale_factor, rgba }),
            None => Err(CaptureError::Failed("Qt rendered nothing".into())),
        });
    }
}

impl KirigamiBackend {
    /// Input on a GPU surface that takes it, through Qt's own event path:
    /// a click (which focuses it) and keys with their native scan codes
    /// go to the window. A scroll is reported as it would be, in points.
    fn surface_input(
        &self,
        id: NodeId,
        input_item: QmlObject,
        window: Option<QmlObject>,
        input: &SyntheticInput,
    ) -> Result<(), ActionError> {
        let events = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            match &node.widget {
                Widget::GpuSurface(surface) if surface.takes_input() => state.events.clone(),
                _ => return Err(ActionError::Unsupported),
            }
        };
        let window = window.ok_or(ActionError::Unsupported)?;
        match input {
            SyntheticInput::Click(point) => window.click(input_item.map_to_scene(*point)),
            SyntheticInput::Key(key) => {
                if window.focus_item().and_then(|f| f.node()) != Some(node_key(id)) {
                    return Err(ActionError::Unsupported);
                }
                let (qt_key, code, text) = match key {
                    Key::Char(c) => {
                        let qt_key = if c.is_ascii_alphanumeric() || *c == ' ' {
                            c.to_ascii_uppercase() as i32
                        } else {
                            KEY_UNKNOWN
                        };
                        (qt_key, KeyCode::from_us_char(*c), c.to_string())
                    }
                    Key::Enter => (KEY_RETURN, KeyCode::Enter, "\r".to_owned()),
                    Key::Escape => (KEY_ESCAPE, KeyCode::Escape, String::new()),
                    Key::Tab => (KEY_TAB, KeyCode::Tab, "\t".to_owned()),
                    Key::Backspace => (KEY_BACKSPACE, KeyCode::Backspace, String::new()),
                    Key::Up => (KEY_UP, KeyCode::ArrowUp, String::new()),
                    Key::Down => (KEY_DOWN, KeyCode::ArrowDown, String::new()),
                    Key::Home => (KEY_HOME, KeyCode::Home, String::new()),
                    Key::End => (KEY_END, KeyCode::End, String::new()),
                };
                // XKB key codes: evdev's plus 8.
                window.surface_key(qt_key, crate::surface::evdev_code(code) + 8, &text);
            }
            SyntheticInput::Scroll { dx, dy } => {
                let delta = ScrollDelta::Points { x: *dx, y: *dy };
                let modifiers = Modifiers::default();
                events.emit(id, UiEvent::SurfaceInput(SurfaceInput::Scroll { delta, modifiers }));
            }
            SyntheticInput::DragFiles(_) | SyntheticInput::DragLeave | SyntheticInput::DropFiles(_) => {
                return Err(ActionError::Unsupported);
            }
        }
        Ok(())
    }

    /// What assistive technology does once it has shown the menu (the
    /// node's context menu, or a menu button's own): trigger the item's
    /// action, as clicking it does. A disabled item (or one in a disabled
    /// container) gets no input, so it shows no menu.
    fn choose_menu_item(&self, id: NodeId, item: u32, button: bool) -> Result<(), ActionError> {
        let action = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if !node.widget.item().bool("enabled") {
                return Err(ActionError::Disabled);
            }
            let menu = if button { &node.button_menu } else { &node.context_menu };
            menu.as_ref().and_then(|menu| menu.action(item)).ok_or(ActionError::Unsupported)?
        };
        if !action.bool("enabled") {
            return Err(ActionError::Disabled);
        }
        // No state borrow: the action's handler reports the choice.
        action.invoke("trigger");
        Ok(())
    }
}

/// Opens dialogs on this window, or the active one.
pub(crate) fn dialog_parent(handle: &KirigamiHandle, parent: Option<NodeId>) -> Option<Rc<WindowRoot>> {
    let windows = handle.windows();
    // Not by `active`: a modal window's owner, which it blocks, reports it
    // too, and so do the owner's other dialogs.
    parent
        .and_then(|id| windows.iter().find(|(w, _)| *w == id).map(|(_, root)| root.clone()))
        .or_else(|| windows.iter().find(|(_, root)| root.window.bool("mitsuamiFocused")).map(|(_, root)| root.clone()))
        .or_else(|| windows.first().map(|(_, root)| root.clone()))
}
