//! The [`Backend`] implementation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{
    Appearance, Backend, CaptureError, EventSink, Image, MeasureRequest, NativeState, PlatformMetrics, SyntheticInput,
};
use mitsuami_core::services::{MenuBarData, Reply};
use mitsuami_core::{
    AppInfo, ButtonRole, ButtonStyle, Command, CustomProps, DisplayList, HorizontalAlign, ImageFit, ImageSource,
    InputPurpose, LayoutDirection, Modality, NodeId, Opaque, Orientation, Point, Rect, RowKey, ScrollAxes,
    SidebarSectionData, Size, TabsStyle, TextStyle, Truncation, UiEvent, WidgetKind,
};

use crate::custom::ErasedRender;
use crate::events::Events;
use crate::ffi::{self, QmlObject};
use crate::services::{ContextMenu, KirigamiServices, Menus};
use crate::surface::SurfaceItem;
use crate::theme;

mod apply;
mod capture;
mod create;
mod handle;
mod input;
mod measure;
mod native_state;
mod perform;
mod props;
mod selection;
mod widget;
mod window;

pub(crate) use input::qt_key;
pub(crate) use window::dialog_parent;

/// How the backend behaves; apps and tests want different things.
#[derive(Clone, Debug, Default)]
pub struct BackendOptions {
    /// Keep a log of applied commands (for tests).
    pub record_commands: bool,
    /// Force this appearance (Breeze Light or Breeze Dark), so captures are
    /// comparable across machines whatever the desktop's color scheme. The
    /// scheme is the application's, so it applies to every window.
    pub appearance: Option<Appearance>,
    /// Take this as the user's language, and write numbers and dates as
    /// its region does, in UTC, whatever the system's settings (tests).
    pub locale: Option<String>,
}

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
    FileIcon(QmlObject),
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
    /// Row hosts: which row of their list they show; cell hosts, of their
    /// table.
    row: Option<RowKey>,
    /// Cell hosts: which column of their table's row they show.
    column: Option<usize>,
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
    /// The handler that reports hover, once the core asked for it.
    hover: Option<crate::hover::Hover>,
    /// The handler that reports double clicks, once the core asked for it.
    double_click: Option<crate::double_click::DoubleClick>,
    /// Menu buttons: their menu, once the app gave one.
    button_menu: Option<ContextMenu>,
    /// Hosts: the drop area over them while they take files, and whether
    /// the app gave `FileDrop` at all.
    file_drop: Option<crate::file_drop::FileDropArea>,
    file_drop_given: bool,
    /// Hosts, groups, lists and tables: the keys they take, once the app
    /// gave them.
    keys: Option<crate::keys::NodeKeys>,
    /// Labels: where the app aligned the text, which Qt mirrors in a
    /// mirrored label.
    align: Option<HorizontalAlign>,
    /// Labels: where the app cut them off, which Qt shows only on a single
    /// line.
    truncation: Option<Truncation>,
    /// The direction the core gave an item made without QML (a drawn
    /// item), which has no `LayoutMirroring` to hold it.
    direction: Option<LayoutDirection>,
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

    fn locale(&self) -> std::rc::Rc<dyn mitsuami_core::l10n::PlatformLocale> {
        std::rc::Rc::new(crate::locale::QtLocale::new(self.state.borrow().options.locale.as_deref()))
    }

    fn set_locale(&mut self, language: &mitsuami_core::l10n::LanguageIdentifier, right_to_left: bool) {
        crate::locale::set_app_locale(language, right_to_left);
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

    fn capture(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        self.capture_node(id, reply)
    }
}
