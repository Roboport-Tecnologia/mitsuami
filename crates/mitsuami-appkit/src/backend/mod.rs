//! The [`Backend`] implementation.

mod apply;
mod capture;
mod create;
mod fonts;
mod group;
mod handle;
mod images;
mod input;
mod measure;
mod menus;
mod native_state;
mod perform;
mod props;
mod scrolling;
mod selection;
mod windows;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{
    Appearance, Backend, CaptureError, EventSink, Image, MeasureRequest, NativeState, PlatformMetrics, SyntheticInput,
};
use mitsuami_core::services::MenuEntry;
use mitsuami_core::{
    AppIcon, AppInfo, ButtonRole, ButtonStyle, Color, Command, CustomProps, FontWeight, ImageFit, ImageSource,
    Modality, NodeId, Opaque, Orientation, RowKey, ScrollAxes, Size, TabsStyle, TextStyle, Truncation, WidgetKind,
};
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, Message};
use objc2_app_kit::{
    NSApplication, NSBox, NSButton, NSControl, NSImage, NSImageView, NSPopUpButton, NSProgressIndicator, NSScrollView,
    NSSlider, NSSwitch, NSTextField, NSView, NSWindow,
};
use objc2_foundation::{NSBundle, NSData, NSString};

use crate::classes::{ActionTarget, ClosureTarget, DrawnView, HostView, ViewMap, WindowDelegate};
use crate::custom::ErasedRender;
use crate::number_field::NumberField;
use crate::radio::RadioGroup;
use crate::sidebar::{Sidebar, Split};
use crate::surface::SurfaceView;
use crate::tabs::Tabs;
use crate::toolbar::Toolbar;

use fonts::metrics;
use group::group_probe_like;
pub(crate) use images::ns_image;

/// How the backend behaves; apps and tests want different things.
#[derive(Clone, Debug)]
pub struct BackendOptions {
    /// Order windows front once their first layout is applied. Tests keep
    /// windows offscreen: layout, events and capture all work without it.
    pub show_windows: bool,
    /// Keep a log of applied commands (for tests).
    pub record_commands: bool,
    /// Force this appearance, so captures are comparable across machines
    /// regardless of system settings.
    pub appearance: Option<Appearance>,
    /// Use a private pasteboard instead of the system clipboard (tests).
    pub private_clipboard: bool,
    /// Take this as the user's language, and write numbers and dates as
    /// its region does, in UTC, whatever the system's settings (tests).
    pub locale: Option<String>,
}

impl Default for BackendOptions {
    fn default() -> Self {
        BackendOptions {
            show_windows: true,
            record_commands: false,
            appearance: None,
            private_clipboard: false,
            locale: None,
        }
    }
}

enum Widget {
    Window {
        window: Retained<NSWindow>,
        host: Retained<HostView>,
        _delegate: Retained<WindowDelegate>,
        /// Made when the first toolbar item arrives, or the sidebar.
        toolbar: Option<Toolbar>,
        /// The window's content beside its sidebar, while it has one.
        split: Option<Split>,
    },
    Host(Retained<HostView>),
    /// A layout host with an `NSBox` behind its children, the box's size.
    Group {
        host: Retained<HostView>,
        frame: Retained<NSBox>,
    },
    Sidebar(Sidebar),
    Tabs(Tabs),
    Label(Retained<NSTextField>),
    Field(Retained<NSTextField>),
    TextArea(crate::text_area::TextArea),
    Button(Retained<NSButton>),
    Checkbox(Retained<NSButton>),
    Switch(Retained<NSSwitch>),
    Select(Retained<NSPopUpButton>),
    RadioGroup(RadioGroup),
    /// A pull-down: its first item is the title it shows, then the app's
    /// menu, whose items call the target.
    MenuButton {
        popup: Retained<NSPopUpButton>,
        target: Retained<ClosureTarget>,
        sent: Vec<MenuEntry>,
    },
    /// A slider, and the step it was given: AppKit steps by tick marks,
    /// which only approximate one that doesn't divide the range.
    Slider {
        slider: Retained<NSSlider>,
        step: Option<f64>,
    },
    NumberInput(Retained<NumberField>),
    Progress(Retained<NSProgressIndicator>),
    /// A spinning indicator, and whether it's animating: AppKit can't say.
    Spinner {
        indicator: Retained<NSProgressIndicator>,
        running: bool,
    },
    Separator(Retained<NSBox>),
    Image(Retained<NSImageView>),
    Icon(Retained<NSImageView>),
    FileIcon(Retained<NSImageView>),
    GpuSurface(Retained<SurfaceView>),
    Scroll(Retained<NSScrollView>),
    List(crate::list::List),
    /// A custom widget with an AppKit render, and the props it last got.
    Custom {
        view: Retained<NSView>,
        render: Rc<dyn ErasedRender>,
        props: CustomProps,
    },
    /// A drawn custom widget.
    Drawn {
        view: Retained<DrawnView>,
        props: CustomProps,
    },
    /// A native view from app code, and the last `Prop::Native` it got.
    Native {
        view: Retained<NSView>,
        measure: Option<NativeMeasure>,
        last: Opaque,
    },
}

type NativeMeasure = Rc<dyn Fn(&NSView, &MeasureRequest) -> Size>;

impl Widget {
    fn view(&self) -> &NSView {
        match self {
            Widget::Window { host, .. } => host,
            Widget::Host(v) | Widget::Group { host: v, .. } => v,
            Widget::Label(v) | Widget::Field(v) => v,
            Widget::TextArea(area) => &area.scroll,
            Widget::Button(v) | Widget::Checkbox(v) => v,
            Widget::Switch(v) => v,
            Widget::Select(v) | Widget::MenuButton { popup: v, .. } => v,
            Widget::RadioGroup(group) => &group.stack,
            Widget::Slider { slider, .. } => slider,
            Widget::NumberInput(v) => v,
            Widget::Progress(v) => v,
            Widget::Spinner { indicator, .. } => indicator,
            Widget::Separator(v) => v,
            Widget::Image(v) | Widget::Icon(v) | Widget::FileIcon(v) => v,
            Widget::GpuSurface(v) => v,
            Widget::Scroll(v) => v,
            Widget::List(list) => &list.scroll,
            Widget::Sidebar(sidebar) => &sidebar.scroll,
            Widget::Tabs(tabs) => &tabs.view,
            Widget::Custom { view, .. } | Widget::Native { view, .. } => view,
            Widget::Drawn { view, .. } => view,
        }
    }

    fn control(&self) -> Option<&NSControl> {
        match self {
            Widget::Label(v) | Widget::Field(v) => Some(v),
            Widget::Button(v) | Widget::Checkbox(v) => Some(v),
            Widget::Switch(v) => Some(v),
            Widget::Select(v) | Widget::MenuButton { popup: v, .. } => Some(v),
            Widget::Slider { slider, .. } => Some(slider),
            // Its field: what's focused, and what text styles apply to.
            Widget::NumberInput(n) => Some(n.field()),
            // Its buttons each: see `RadioGroup`.
            // A text view isn't a control: see `enabled`.
            Widget::Window { .. }
            | Widget::RadioGroup(_)
            | Widget::TextArea(_)
            | Widget::Progress(_)
            | Widget::Spinner { .. }
            | Widget::Separator(_)
            | Widget::Image(_)
            | Widget::Icon(_)
            | Widget::FileIcon(_)
            | Widget::GpuSurface(_)
            | Widget::Host(_)
            | Widget::Group { .. }
            | Widget::Scroll(_)
            | Widget::List(_)
            | Widget::Sidebar(_)
            | Widget::Tabs(_)
            | Widget::Custom { .. }
            | Widget::Drawn { .. }
            | Widget::Native { .. } => None,
        }
    }

    /// The view that takes keyboard focus: a list's table, not its scroll
    /// view.
    fn key_view(&self) -> Retained<NSView> {
        match self {
            Widget::List(list) => Retained::into_super(Retained::into_super(Retained::into_super(list.table.clone()))),
            Widget::Sidebar(sidebar) => Retained::into_super(Retained::into_super(sidebar.table.clone())),
            Widget::NumberInput(n) => Retained::into_super(Retained::into_super(n.field().retain())),
            Widget::RadioGroup(group) => group.key_view(),
            Widget::TextArea(area) => Retained::into_super(Retained::into_super(area.text.clone())),
            widget => widget.view().retain(),
        }
    }

    /// Whether a control or text area takes input; `None` for the rest.
    fn enabled(&self) -> Option<bool> {
        match self {
            Widget::TextArea(area) => Some(area.enabled()),
            widget => widget.control().map(|c| c.isEnabled()),
        }
    }
}

struct Node {
    kind: WidgetKind,
    widget: Widget,
    /// Controls only hold weak references to their target and delegate.
    _target: Option<Retained<ActionTarget>>,
    /// Action targets of native renders and native views.
    _targets: Vec<Retained<ClosureTarget>>,
    parent: Option<NodeId>,
    /// Row hosts: which row of their list they show.
    row: Option<RowKey>,
    /// Cell hosts: which column of their table's row they show.
    column: Option<usize>,
    /// Props AppKit can't report back faithfully.
    text_style: Option<TextStyle>,
    /// Labels: what the app gave, which the font and colour are made of
    /// (read back from the label, but only once given).
    weight: Option<FontWeight>,
    italic: Option<bool>,
    text_color: Option<Color>,
    align: bool,
    /// Labels: where the app cut them off, which a label of several lines
    /// doesn't show.
    truncation: Option<Truncation>,
    role: Option<ButtonRole>,
    button_style: Option<ButtonStyle>,
    /// Tabs: the style the app chose, which this platform doesn't have,
    /// and the icons it gave, which an `NSTabView` doesn't draw.
    tabs_style: Option<TabsStyle>,
    tab_icons: Option<Vec<String>>,
    /// Sliders: whether the app gave an `Orientation`. Separators: which
    /// way they run, which an `NSBox` takes from its frame's shape.
    orientation: Option<Orientation>,
    /// Checkboxes: whether the app gave `Mixed`, and the `Checked` the box
    /// shows when it isn't mixed.
    mixed: Option<bool>,
    checked: bool,
    /// Scroll views: the axes they scroll, and whether they show scroll
    /// bars. Without scrollers, AppKit can't tell which axes scroll.
    scroll_axes: ScrollAxes,
    scroll_bars: bool,
    /// Images: what they show and how they fit, which AppKit can't give
    /// back (the fit only if the app chose one).
    image: Option<ImageSource>,
    fit: Option<ImageFit>,
    /// Icons and buttons: the icon's name and size, which AppKit can't
    /// give back (a symbol image has no name), and whether the app gave
    /// `IconOnly`.
    icon: Option<String>,
    icon_size: Option<f32>,
    icon_only: Option<bool>,
    /// File icons: the file, and whether the app asked for a thumbnail,
    /// which an image view can't give back.
    file: Option<std::path::PathBuf>,
    thumbnail: Option<bool>,
    /// Windows: modal, and the window they belong to.
    modal: Option<(Option<NodeId>, Modality)>,
    /// The app's raw settings, run after every other prop.
    tweak: Option<Opaque>,
    /// Hosts: whether the core set a `FileDrop` (the host has the rest).
    file_drop: bool,
    /// The context menu the app gave, if it gave one (AppKit can't tell
    /// radio items from check items, nor keep roles), and the target of
    /// its items, which only hold weak references to it.
    context_menu: Option<(Vec<MenuEntry>, Retained<ClosureTarget>)>,
    /// The owner of the tracking area that reports hover, and the area,
    /// once the core asked for it.
    hover: Option<(Retained<crate::classes::HoverTracker>, Retained<objc2_app_kit::NSTrackingArea>)>,
    /// The target of the recognizer that reports double clicks, and the
    /// recognizer, once the core asked for it.
    double_click: Option<(Retained<crate::classes::DoubleClicker>, Retained<objc2_app_kit::NSClickGestureRecognizer>)>,
}

struct State {
    mtm: MainThreadMarker,
    options: BackendOptions,
    nodes: HashMap<NodeId, Node>,
    /// Native view (by address) → node. Shared with window delegates, which
    /// resolve the first responder; only ever borrowed briefly.
    by_view: ViewMap,
    events: EventSink,
    log: Vec<Command>,
    pending_show: Vec<NodeId>,
    /// The key view chain last built for each window.
    focus_orders: HashMap<NodeId, Vec<NodeId>>,
    /// The app's name, for the app menu.
    app_name: Option<String>,
    /// Where tab views put their pages, from the last metrics: measuring
    /// one would otherwise make a tab view to probe each time.
    tab_insets: std::cell::Cell<Option<mitsuami_core::Insets>>,
    /// Where group boxes put their content, by how they're set up, from
    /// probes made since the last metrics (see `group_probe_like`).
    group_probes: RefCell<HashMap<group::ProbeKey, (mitsuami_core::Insets, f32)>>,
    /// Each node's children, in no particular order (`Node::parent` the
    /// other way).
    children: HashMap<NodeId, Vec<NodeId>>,
    /// The lists and windows the batch being applied touched, which are
    /// laid out at its end (see `layout_touched`), and whether it touched
    /// something that can bring any list into view (a tab view's page, a
    /// scroll view's offset).
    touched_lists: HashSet<NodeId>,
    touched_windows: HashSet<NodeId>,
    touched_all_lists: bool,
}

pub struct AppKitBackend {
    state: Rc<RefCell<State>>,
}

/// Shared access to an [`AppKitBackend`] after it was moved into a `Ui`.
#[derive(Clone)]
pub struct AppKitHandle {
    state: Rc<RefCell<State>>,
}

/// Where a checkbox or radio button puts its box: on the reading side,
/// which AppKit takes from the app's direction, not the button's.
pub(crate) fn toggle_image_position(
    direction: objc2_app_kit::NSUserInterfaceLayoutDirection,
) -> objc2_app_kit::NSCellImagePosition {
    match direction {
        objc2_app_kit::NSUserInterfaceLayoutDirection::RightToLeft => objc2_app_kit::NSCellImagePosition::ImageRight,
        _ => objc2_app_kit::NSCellImagePosition::ImageLeft,
    }
}

/// A field's text starts on its reading side; natural alignment would
/// take the app's direction, not the field's.
pub(crate) fn field_alignment(
    direction: objc2_app_kit::NSUserInterfaceLayoutDirection,
) -> objc2_app_kit::NSTextAlignment {
    match direction {
        objc2_app_kit::NSUserInterfaceLayoutDirection::RightToLeft => objc2_app_kit::NSTextAlignment::Right,
        _ => objc2_app_kit::NSTextAlignment::Left,
    }
}

pub(crate) fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn key(view: &NSView) -> usize {
    view as *const NSView as usize
}

fn violation(command: &Command, problem: &str) -> ! {
    panic!("appkit backend: protocol violation in {command:?}: {problem}")
}

impl AppKitBackend {
    pub fn new(mtm: MainThreadMarker, options: BackendOptions) -> AppKitBackend {
        AppKitBackend {
            state: Rc::new(RefCell::new(State {
                mtm,
                options,
                nodes: HashMap::new(),
                by_view: ViewMap::default(),
                events: EventSink::default(),
                log: Vec::new(),
                pending_show: Vec::new(),
                focus_orders: HashMap::new(),
                app_name: None,
                tab_insets: std::cell::Cell::new(None),
                group_probes: RefCell::default(),
                children: HashMap::new(),
                touched_lists: HashSet::new(),
                touched_windows: HashSet::new(),
                touched_all_lists: false,
            })),
        }
    }

    pub fn handle(&self) -> AppKitHandle {
        AppKitHandle { state: self.state.clone() }
    }
}

impl Backend for AppKitBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow_mut().events = events;
    }

    /// The box's own, which a tweak may have changed (a title at the
    /// bottom, other margins).
    fn group_insets(&self, id: NodeId) -> Option<mitsuami_core::Insets> {
        let state = self.state.borrow();
        let Widget::Group { frame, .. } = &state.nodes.get(&id)?.widget else { return None };
        Some(group_probe_like(&state.group_probes, frame).0)
    }

    fn metrics(&self) -> PlatformMetrics {
        let state = self.state.borrow();
        let metrics = metrics(state.mtm, state.options.appearance);
        state.tab_insets.set(Some(metrics.tab_insets));
        // Fonts, and so titles, may have changed.
        state.group_probes.borrow_mut().clear();
        metrics
    }

    fn apply(&mut self, batch: &[Command]) {
        let mut state = self.state.borrow_mut();
        for command in batch {
            if state.options.record_commands {
                state.log.push(command.clone());
            }
            state.apply(command);
            state.note_touched(command);
        }
        state.layout_touched();
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        measure::measure(&self.state.borrow(), id, request)
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        self.perform_action(id, action)
    }

    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        self.synthesize_input(id, input)
    }

    fn native_state(&self, id: NodeId) -> Option<NativeState> {
        native_state::native_state(&self.state.borrow(), id)
    }

    /// The id is the bundle's, and so is the icon if the bundle has one: a
    /// bundle's icon has the sizes and variants (dark, tinted) that one
    /// image doesn't. The name goes in the app menu; the menu bar's title
    /// is the bundle's or the process's, which AppKit alone sets.
    fn set_app_info(&mut self, info: &AppInfo) {
        let mut state = self.state.borrow_mut();
        state.app_name = info.name.clone();
        let bundle = NSBundle::mainBundle();
        let has_icon = ["CFBundleIconFile", "CFBundleIconName"]
            .iter()
            .any(|key| bundle.objectForInfoDictionaryKey(&ns(key)).is_some());
        let image = info.icon.as_ref().filter(|_| !has_icon).and_then(|icon| match icon {
            AppIcon::File(path) => NSImage::initWithContentsOfFile(NSImage::alloc(), &ns(&path.to_string_lossy())),
            AppIcon::Bytes(bytes) => NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes)),
        });
        if let Some(image) = image {
            unsafe { NSApplication::sharedApplication(state.mtm).setApplicationIconImage(Some(&image)) };
        }
    }

    fn locale(&self) -> std::rc::Rc<dyn mitsuami_core::l10n::PlatformLocale> {
        std::rc::Rc::new(crate::locale::AppKitLocale::new(self.state.borrow().options.locale.as_deref()))
    }

    fn set_locale(&mut self, language: &mitsuami_core::l10n::LanguageIdentifier, right_to_left: bool) {
        crate::locale::set_app_locale(language, right_to_left);
    }

    fn services(&self) -> Box<dyn mitsuami_core::services::Services> {
        let state = self.state.borrow();
        Box::new(crate::services::AppKitServices::new(state.mtm, self.handle(), state.options.private_clipboard))
    }

    fn capture(&mut self, id: NodeId, reply: mitsuami_core::services::Reply<Result<Image, CaptureError>>) {
        // Drawing into a bitmap is synchronous on AppKit: answer right away.
        reply(self.capture_now(id));
    }
}
