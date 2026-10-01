//! The [`Backend`] implementation.

mod apply;
mod controls;
mod create;
mod fields;
mod focus;
mod handle;
mod input;
mod keys;
mod measure;
mod menus;
mod native_state;
mod new_window;
mod perform;
mod props;
mod scroll;
mod selection;
mod styles;
mod truncate;
mod windows;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::Color;
use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{
    Appearance, Backend, CaptureError, EventSink, Image, MeasureRequest, NativeState, PlatformMetrics, SyntheticInput,
};
use mitsuami_core::services::{MenuBarData, MenuCheck, MenuEntry, Reply};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    AnyValue, AppInfo, ButtonRole, ButtonStyle, Command, CustomProps, HorizontalAlign, ImageFit, ImageSource, Insets,
    LayoutDirection, Modality, NodeId, Opaque, Orientation, Point, RowKey, Size, TabsStyle, TextStyle, Truncation,
    UiEvent, WidgetKind,
};
use windows_core::{EventRevoker, HSTRING, IInspectable, IUnknown, Interface};

use crate::bindings as w;
use crate::custom::{DrawnView, ErasedRender, Measure};
use crate::runtime;
use crate::surface::SurfaceHost;
use fields::inner_text_box;
pub(crate) use measure::measure_element;
pub(crate) use new_window::packaged;
use new_window::{set_icon, window_icon};
use styles::font_sizes;
pub(crate) use styles::style;

/// Fluent's spacing ramp: 4, 8, 12, 16, 24 epx. The spacing tokens, and
/// the space the backend puts in its own chrome.
pub(crate) const SPACING: SpacingScale = SpacingScale { xs: 4.0, sm: 8.0, md: 12.0, lg: 16.0, xl: 24.0 };

/// How the backend behaves; apps and tests want different things.
#[derive(Clone, Debug)]
pub struct BackendOptions {
    /// Make windows visible once their first layout is applied. Tests keep
    /// them invisible: layout, events and capture all work without it.
    pub show_windows: bool,
    /// Keep a log of applied commands (for tests).
    pub record_commands: bool,
    /// Force this theme, so captures are comparable across machines.
    pub appearance: Option<Appearance>,
    /// Keep clipboard text in memory instead of the system clipboard (tests).
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

/// The parts of a window: XAML's `Window`, a root grid with a row for the
/// menu bar and toolbar and the content host (a `Canvas`) below it.
pub(crate) struct WindowParts {
    pub(crate) window: w::Window,
    root: w::Grid,
    /// The menu bar and toolbar's row: the menu bar leading, the toolbar in
    /// the room left.
    bars: w::Grid,
    host: w::Canvas,
    /// An empty text box, collapsed, that text areas measure their lines
    /// by: XAML measures text boxes only in a live tree.
    text_probe: w::TextBox,
    title_bar: w::TitleBar,
    app_window: w::AppWindow,
    pub(crate) hwnd: w::HWND,
    pub(crate) id: w::WindowId,
    menu_bar: Option<w::MenuBar>,
    menu_revokers: Vec<EventRevoker>,
    /// The menus the bar shows (the app's and the window's own), to tell
    /// what changed.
    menu_shown: MenuBarData,
    /// The bar's items by id, and the check mark each should show.
    menu_items: MenuItems,
    /// The content size the app asked for, re-applied when the menu bar
    /// changes height.
    requested: Option<Size>,
    /// The window's node and how to report its events.
    node: NodeId,
    emitter: Events,
    /// The content size, as last reported.
    size: Rc<Cell<Option<Size>>>,
    /// A content size `resize_client` aimed for, and the size before it,
    /// until XAML lays the content out at the window's new size.
    aimed: Rc<Cell<Option<(Size, Size)>>>,
    /// The resize border inside the client area, in pixels, as last
    /// measured (see `client_insets`).
    insets: Cell<(i32, i32)>,
    /// The node that has keyboard focus, as last reported.
    focus: Rc<Cell<Option<NodeId>>>,
    /// The Tab order sent by the core.
    tab_order: Rc<RefCell<Vec<NodeId>>>,
    /// Focus asked for before the control could take it, and the text to
    /// select in it (`focus_wanted`).
    wanted_focus: RefCell<Option<(NodeId, Option<std::ops::Range<usize>>)>>,
    /// Modal, and the window it belongs to: acted on when it's shown.
    modal: Option<(Option<NodeId>, Modality)>,
    /// The windows it disabled while it's open (application-modal), to
    /// enable again when it closes.
    disabled: Vec<w::HWND>,
    /// Dialogs (modal windows): the Escape accelerator's handler.
    escape: Option<EventRevoker>,
    /// The toolbar, made when its first item arrives: a `CommandBar` in the
    /// bars, whose primary commands hold the items' hosts, in order.
    toolbar: Option<w::CommandBar>,
    toolbar_items: Vec<(NodeId, w::AppBarElementContainer)>,
    /// Its sidebar's node and navigation view, while it has one: the
    /// view's content is the host.
    sidebar: Option<(NodeId, w::NavigationView)>,
    /// The handlers that move its menu button to the title bar, and what
    /// places it, which the sidebar holds weakly.
    sidebar_revokers: Vec<EventRevoker>,
    sidebar_place: Option<crate::sidebar::Place>,
    /// Full screen as the app wants it, and the user (who changes it
    /// too): what the presenter is compared with when it changes.
    full_screen: Rc<Cell<bool>>,
    /// Maximized as the app wants it, and the user: what the presenter's
    /// state is compared with when the window's size changes.
    maximized: Rc<Cell<bool>>,
    /// The user can resize it: the presenter's, kept for full screen.
    resizable: bool,
    /// Made visible: full screen and maximizing wait for it, as a hidden
    /// window would fill the screen unseen, or be shown by `Maximize`.
    shown: bool,
    /// The window's own presenter while it's in full screen, to go back
    /// to with its settings (modal, minimum size).
    overlapped: Option<w::AppWindowPresenter>,
    /// The content's minimum size, as the core set it.
    min_size: Option<Size>,
    /// The content sets the height, not the user.
    height_locked: bool,
    /// It moved to another display: its minimum is capped by the
    /// display's work area, so it's applied again.
    moved: Rc<Cell<bool>>,
}

enum Widget {
    Window(Box<WindowParts>),
    Host(w::Canvas),
    Label(w::TextBlock),
    Field(w::TextBox),
    /// A multi-line text box, and how many lines it's tall: XAML has no
    /// number of lines.
    TextArea {
        field: w::TextBox,
        lines: u32,
    },
    Password(w::PasswordBox),
    /// Windows' search box: an auto-suggest box with the find icon.
    Search(w::AutoSuggestBox),
    Button(w::Button),
    /// A toggle button: a button's content, a checkbox's `IsChecked`.
    Toggle(w::ToggleButton),
    /// A button whose `Flyout` is its menu, which it opens on a click.
    MenuButton(w::DropDownButton),
    Checkbox(w::CheckBox),
    Switch(w::ToggleSwitch),
    Select(w::ComboBox),
    RadioGroup(w::RadioButtons),
    /// A slider, and the step it was given (XAML reads back its own
    /// default without one).
    Slider {
        slider: w::Slider,
        step: Option<f64>,
    },
    /// A number box, and the step it was given (XAML reads back its own
    /// default without one).
    Number {
        number: w::NumberBox,
        step: Option<f64>,
    },
    Progress(w::ProgressBar),
    Spinner(w::ProgressRing),
    /// XAML has no separator control: a `Border` in the divider brush.
    Separator(w::Border),
    /// An image view, and what it was given: XAML can't give back a
    /// source's path or pixels, or say whether a fit was chosen.
    Image {
        image: w::Image,
        source: Option<ImageSource>,
        fit: Option<ImageFit>,
        /// Files: the bitmap XAML decodes in the background, and whether
        /// decoding failed, or is done.
        bitmap: Option<w::BitmapImage>,
        failed: Rc<Cell<bool>>,
        opened: Rc<Cell<bool>>,
    },
    /// A glyph of Segoe Fluent Icons (the theme's symbol font).
    Icon(w::FontIcon),
    /// A file's icon in an `Image`, what the app gave (the image has only
    /// pixels), and its loads: the latest one asked for, and the latest
    /// one shown.
    FileIcon {
        image: w::Image,
        file: Option<std::path::PathBuf>,
        thumbnail: Option<bool>,
        size: Option<f32>,
        asked: Rc<Cell<u64>>,
        shown: Rc<Cell<u64>>,
    },
    GpuSurface(SurfaceHost),
    Scroll(w::ScrollViewer),
    List(crate::list::List),
    Sidebar(crate::sidebar::Sidebar),
    Tabs(crate::tabs::Tabs),
    Group(crate::group::Group),
    /// A custom widget with a native render, and the props it shows.
    Custom {
        render: Rc<dyn ErasedRender>,
        props: CustomProps,
    },
    /// A drawn custom widget.
    Drawn {
        view: DrawnView,
        props: CustomProps,
    },
    /// A native view from app code, and the last `Prop::Native` it got.
    Native {
        measure: Option<Measure>,
        last: Opaque,
    },
}

impl Node {
    /// What takes focus, UI Automation and a render's calls.
    fn control(&self) -> &w::UIElement {
        self.inner.as_ref().unwrap_or(&self.element)
    }

    /// What takes keyboard focus: a sidebar's or tab view's selected item
    /// (or first), as Tab reaches a navigation view or selector bar.
    fn focus_target(&self) -> Option<w::IUIElement> {
        match &self.widget {
            Widget::Sidebar(sidebar) => crate::sidebar::Sidebar::focus_target(&sidebar.view),
            Widget::Tabs(tabs) => crate::tabs::Tabs::focus_target(&tabs.bar),
            // Its chosen button, or the first, as Tab reaches the group.
            Widget::RadioGroup(group) => group.ContainerFromIndex(group.SelectedIndex().ok()?.max(0)).ok()?.cast().ok(),
            // The text box in its template.
            Widget::Search(_) => inner_text_box(&self.element).and_then(|f| f.cast().ok()),
            _ => self.control().cast().ok(),
        }
    }
}

struct Node {
    kind: WidgetKind,
    widget: Widget,
    /// What the parent holds and our frames size: the control itself, a
    /// window's host, or the `Border` around a native render or view.
    element: w::UIElement,
    /// Inside that `Border`: the render's or view's own control. Many XAML
    /// controls size themselves (`RatingControl` sets its own `Width`), so
    /// they don't get our frame directly. A table's list view, inside
    /// the grid with its header.
    inner: Option<w::UIElement>,
    parent: Option<NodeId>,
    /// Row hosts: which row of their list they show; cell hosts, which
    /// row of their table.
    row: Option<RowKey>,
    /// Cell hosts: which column of their table's row they show.
    column: Option<usize>,
    revokers: Vec<EventRevoker>,
    /// The value the native widget is known to show, set by the core or
    /// reported to it. Change events that match it are programmatic.
    shown_text: Rc<RefCell<String>>,
    shown_checked: Rc<Cell<bool>>,
    /// Checkboxes: whether they show the mixed state (`IsChecked` null).
    /// Leaving it is a change, whatever the value lands on.
    shown_mixed: Rc<Cell<bool>>,
    /// Selects and radio groups: the chosen index, or -1.
    shown_index: Rc<Cell<i32>>,
    /// Sliders and number boxes: the value.
    shown_number: Rc<Cell<f64>>,
    /// ScrollViews: the offset last reported, and Shift+wheel on their
    /// content.
    offset: Rc<Cell<Point>>,
    shift_wheel: Option<EventRevoker>,
    /// A ScrollView's content: hit-testable everywhere, for the wheel.
    scroll_content: bool,
    /// Props XAML can't report back faithfully.
    text_style: Option<TextStyle>,
    /// Labels: their colour, a theme brush in their style.
    text_color: Option<Color>,
    role: Option<ButtonRole>,
    button_style: Option<ButtonStyle>,
    /// Tabs: the style the app chose, which this platform doesn't have.
    tabs_style: Option<TabsStyle>,
    /// Sliders: whether the app gave an `Orientation`. Separators: which
    /// way they run, which a `Border` doesn't know.
    orientation: Option<Orientation>,
    /// Checkboxes: whether the app gave `Mixed`.
    mixed: Option<bool>,
    /// The app's raw settings, run after every other prop.
    tweak: Option<Opaque>,
    /// Switches, selects, sliders and progress bars: their label, which is
    /// only their accessible name.
    a11y_label: Option<String>,
    /// The app's accessible description, which UIA's help text shows in
    /// place of the tooltip.
    description: Option<String>,
    tooltip: String,
    /// Buttons: their caption, icon (empty: none) and whether the icon
    /// shows alone, which their content is made of.
    caption: String,
    icon: String,
    icon_only: Option<bool>,
    /// Icons: whether the app gave a size (XAML reads back its default).
    icon_size: bool,
    /// Set once the core gave a context menu.
    context_menu: Option<ContextMenu>,
    /// Menu buttons: their menu, set once the core gave one.
    button_menu: Option<ContextMenu>,
    /// Hosts and groups: the files they take and the drag over them, while
    /// they take some, and whether the core ever sent `FileDrop`.
    file_drop: Option<crate::drop::DropTarget>,
    file_drop_sent: bool,
    /// What reports hover, once the core asked for it.
    hover: Option<crate::hover::HoverTracker>,
    /// Containers, groups, lists and tables: the keys they take, once the
    /// core sent some.
    keys: Option<keys::Keys>,
    /// The direction the core gave, for what stays left to right because
    /// XAML would mirror what it holds, whose frames are mirrored already.
    direction: Option<LayoutDirection>,
    /// Labels: where the core aligned the text.
    align: Option<HorizontalAlign>,
    /// Labels: where the app cut them off, which XAML can't (its trimming
    /// is always at the end).
    truncation: Option<Truncation>,
    /// Labels: the text the app gave, which they show cut off at its
    /// start or middle (`truncate`).
    label: truncate::LabelText,
    /// Password boxes: all their text is selected (`SelectText`), which
    /// XAML can't tell, so typing replaces it.
    password_all: Cell<bool>,
}

type Callback = Rc<dyn Fn()>;

type MenuItems = Rc<RefCell<HashMap<u32, (w::MenuFlyoutItemBase, MenuCheck)>>>;

/// A node's context menu, or a menu button's menu: what the core sent, and
/// the `MenuFlyout` showing it as the control's `ContextFlyout` or the
/// button's `Flyout` (none while it's empty).
struct ContextMenu {
    sent: Vec<MenuEntry>,
    flyout: Option<w::MenuFlyout>,
    items: MenuItems,
    revokers: Vec<EventRevoker>,
    /// Reports an item chosen.
    activate: Rc<dyn Fn(u32)>,
    /// The control's own flyout (a text box's Cut, Copy and Paste), back
    /// while the app's menu is empty.
    own: Option<w::FlyoutBase>,
}

/// The app's menus, each window's own, and how to report a choice.
#[derive(Default)]
struct Menus {
    app: MenuBarData,
    windows: HashMap<NodeId, MenuBarData>,
    activate: Option<Rc<dyn Fn(u32)>>,
}

/// Native element (by COM identity) → node, shared with focus handlers.
type ElementMap = Rc<RefCell<HashMap<usize, NodeId>>>;

/// Emits events and schedules a tick so they get handled soon, even from
/// modal loops (live resizing).
#[derive(Clone)]
pub(crate) struct Events {
    sink: EventSink,
    wake: Rc<RefCell<Option<Callback>>>,
    /// Set while the backend applies a custom widget's or native view's
    /// props: what their controls report then is programmatic.
    muted: Rc<Cell<bool>>,
}

impl Events {
    pub(crate) fn emit(&self, id: NodeId, event: UiEvent) {
        self.sink.emit(id, event);
        let wake = self.wake.borrow().clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    /// Makes the run loop turn, with nothing to report.
    pub(crate) fn wake(&self) {
        let wake = self.wake.borrow().clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    /// An event of a custom widget or native view, unless muted.
    pub(crate) fn emit_custom(&self, id: NodeId, event: AnyValue) {
        if !self.muted.get() {
            self.emit(id, UiEvent::Custom(event));
        }
    }

    /// Runs `f` with custom events muted.
    fn muted<T>(&self, f: impl FnOnce() -> T) -> T {
        let before = self.muted.replace(true);
        let result = f();
        self.muted.set(before);
        result
    }
}

pub(crate) struct State {
    pub(crate) options: BackendOptions,
    nodes: HashMap<NodeId, Node>,
    by_element: ElementMap,
    emitter: Events,
    log: Vec<Command>,
    pending_show: Vec<NodeId>,
    menus: Menus,
    /// The app's icon, which every window gets.
    icon: Option<WindowIcon>,
    /// A tab view's bar height, once one is measured (`tab_insets`).
    tab_bar: crate::tabs::BarHeight,
    /// A group's heading height, once one is measured (`titled_group_insets`).
    group_heading: crate::group::HeadingHeight,
    /// The app's language is right to left: windows' own rows (the title
    /// bar, menu bar and toolbar) are mirrored.
    right_to_left: bool,
}

/// The app's icon, as windows take it.
enum WindowIcon {
    /// An `.ico` file, with its sizes.
    File(String),
    /// Kept for as long as windows show it.
    Handle(w::HICON),
}

pub struct WinUiBackend {
    state: Rc<RefCell<State>>,
}

/// Shared access to a [`WinUiBackend`] after it was moved into a `Ui`.
#[derive(Clone)]
pub struct WinUiHandle {
    state: Rc<RefCell<State>>,
}

type R<T> = windows_core::Result<T>;

fn ok<T>(result: R<T>, what: &str) -> T {
    result.unwrap_or_else(|e| panic!("winui backend: {what} failed: {e}"))
}

/// COM identity of an element.
fn key(element: &impl Interface) -> usize {
    element.cast::<IUnknown>().map_or(0, |u| u.as_raw() as usize)
}

/// UIA's help text: the app's description, else the tooltip, as the core's
/// accessibility tree has it.
fn set_help_text(node: &Node) -> R<()> {
    let help = node.description.as_deref().unwrap_or(&node.tooltip);
    w::AutomationProperties::SetHelpText(node.control(), help)
}

pub(crate) fn boxed(text: &str) -> IInspectable {
    ok(w::PropertyValue::CreateString(text), "boxing a string")
}

fn unboxed(value: R<IInspectable>) -> Option<String> {
    value.ok()?.cast::<w::IPropertyValue>().ok()?.GetString().ok()
}

fn violation(command: &Command, problem: &str) -> ! {
    panic!("winui backend: protocol violation in {command:?}: {problem}")
}

impl WinUiBackend {
    /// Starts the Windows App Runtime and XAML on this thread if needed.
    pub fn new(options: BackendOptions) -> WinUiBackend {
        runtime::init();
        WinUiBackend {
            state: Rc::new(RefCell::new(State {
                options,
                nodes: HashMap::new(),
                by_element: ElementMap::default(),
                emitter: Events { sink: EventSink::default(), wake: Rc::default(), muted: Rc::default() },
                log: Vec::new(),
                pending_show: Vec::new(),
                menus: Menus::default(),
                icon: None,
                tab_bar: Rc::default(),
                group_heading: Rc::default(),
                right_to_left: false,
            })),
        }
    }

    pub fn handle(&self) -> WinUiHandle {
        WinUiHandle { state: self.state.clone() }
    }
}

impl State {
    fn emitter(&self) -> Events {
        self.emitter.clone()
    }
}

/// A Canvas without a background isn't hit-testable, so the pointer would
/// never rest on it, a right-click would pass it by, and so would the
/// wheel and dragged files: a clear one while it has a tooltip (as drawn
/// views have), a context menu or a drop target, or is a ScrollView's
/// content. A group's canvas only while it takes files.
fn set_hit_testable(node: &Node) -> R<()> {
    let panel = match &node.widget {
        Widget::Host(canvas) => canvas.cast::<w::IPanel>()?,
        Widget::Group(group) if node.file_drop_sent || node.hover.is_some() => group.canvas.cast::<w::IPanel>()?,
        _ => return Ok(()),
    };
    if node.tooltip.is_empty()
        && !node.scroll_content
        && !ContextMenu::has_items(&node.context_menu)
        && node.file_drop.is_none()
        && node.hover.is_none()
    {
        panel.SetBackground(None::<&w::Brush>)
    } else {
        let clear = w::SolidColorBrush::CreateInstanceWithColor(w::Color { a: 0, r: 0, g: 0, b: 0 })?;
        panel.SetBackground(&clear)
    }
}

impl Backend for WinUiBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow_mut().emitter.sink = events;
    }

    fn metrics(&self) -> PlatformMetrics {
        let dark = match self.state.borrow().options.appearance {
            Some(appearance) => appearance == Appearance::Dark,
            None => w::Application::Current()
                .and_then(|a| a.cast::<w::IApplication>()?.RequestedTheme())
                .is_ok_and(|t| t == w::ApplicationTheme::Dark),
        };
        let settings = w::UISettings::new().ok();
        // A selector bar over the pages, with no border around them.
        let bar = self.state.borrow().tab_bar.get().unwrap_or(crate::tabs::BAR_HEIGHT);
        // A card under a heading, as Settings groups settings.
        let heading = self.state.borrow().group_heading.get().unwrap_or(crate::group::HEADING_HEIGHT);
        let (group_insets, titled_group_insets) = crate::group::insets(heading);
        PlatformMetrics {
            scale_factor: unsafe { w::GetDpiForSystem() } as f32 / 96.0,
            spacing: SPACING,
            font_sizes: font_sizes(),
            dark_mode: dark,
            high_contrast: w::AccessibilitySettings::new()
                .and_then(|s| s.cast::<w::IAccessibilitySettings>()?.HighContrast())
                .unwrap_or(false),
            reduced_motion: settings
                .and_then(|s| s.cast::<w::IUISettings>().ok()?.AnimationsEnabled().ok())
                .is_some_and(|enabled| !enabled),
            tab_insets: Insets::new(bar, 0.0, 0.0, 0.0),
            group_insets,
            titled_group_insets,
        }
    }

    fn apply(&mut self, batch: &[Command]) {
        let mut state = self.state.borrow_mut();
        for command in batch {
            if state.options.record_commands {
                state.log.push(command.clone());
            }
            if let Err(error) = state.apply(command) {
                panic!("winui backend: {command:?} failed: {error}");
            }
        }
        state.attach_surfaces();
        state.connect_scroll_content();
        state.layout_lists();
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

    /// The AppUserModelID groups the app's windows on the taskbar; only an
    /// unpackaged app sets it, as a package has its own. The name is the
    /// executable's (its version resource) or its shortcut's, so it's left.
    /// Each window gets the icon, as a Win32 app's get its class's.
    fn set_app_info(&mut self, info: &AppInfo) {
        if let Some(id) = &info.id
            && !packaged()
        {
            let id = HSTRING::from(id.as_str());
            _ = unsafe { w::SetCurrentProcessExplicitAppUserModelID(windows_core::PCWSTR(id.as_ptr())) };
        }
        let Some(icon) = info.icon.as_ref().and_then(window_icon) else { return };
        let mut state = self.state.borrow_mut();
        for node in state.nodes.values() {
            if let Widget::Window(parts) = &node.widget {
                _ = set_icon(&parts.app_window, &icon);
            }
        }
        if let Some(WindowIcon::Handle(old)) = state.icon.replace(icon) {
            unsafe { _ = w::DestroyIcon(old) };
        }
    }

    fn locale(&self) -> Rc<dyn mitsuami_core::l10n::PlatformLocale> {
        Rc::new(crate::locale::WinLocale::new(self.state.borrow().options.locale.as_deref()))
    }

    /// Windows' own rows mirror; the content host doesn't, since the core
    /// mirrored the frames.
    fn set_locale(&mut self, _language: &mitsuami_core::l10n::LanguageIdentifier, right_to_left: bool) {
        let mut state = self.state.borrow_mut();
        state.right_to_left = right_to_left;
        for node in state.nodes.values() {
            if let Widget::Window(parts) = &node.widget {
                _ = windows::set_window_direction(&parts.root, &parts.host, right_to_left);
            }
        }
    }

    fn services(&self) -> Box<dyn mitsuami_core::services::Services> {
        let private = self.state.borrow().options.private_clipboard;
        Box::new(crate::services::WinUiServices::new(self.handle(), private))
    }

    fn capture(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        let element = {
            let state = self.state.borrow();
            match state.nodes.get(&id) {
                // A window's capture is its content area, like its size.
                Some(Node { widget: Widget::Window(parts), .. }) => parts.host.cast::<w::UIElement>().ok(),
                Some(node) => Some(node.element.clone()),
                None => None,
            }
        };
        let Some(element) = element else { return reply(Err(CaptureError::UnknownNode)) };
        crate::capture::capture(element, reply);
    }
}
