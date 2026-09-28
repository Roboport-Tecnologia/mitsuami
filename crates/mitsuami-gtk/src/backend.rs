//! The [`Backend`] implementation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, gio, glib, graphene, gsk, pango};
use mitsuami_core::a11y::{A11yAction, A11yProps, ActionError};
use mitsuami_core::backend::{
    Appearance, AvailableSpace, Backend, CaptureError, EventSink, FontSizes, Image, Key, MeasureRequest, NativeState,
    PlatformMetrics, SyntheticInput,
};
use mitsuami_core::services::{MenuBarData, Reply};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    AppInfo, ButtonRole, ButtonStyle, Color, Command, CustomProps, EventValue, FontWeight, HorizontalAlign, ImageFit,
    ImageSource, Modality, NativeAppInfo, NativeIcon, NodeId, Opaque, Orientation, Point, Prop, Rect, RowKey,
    ScrollAxes, SelectionMode, Size, TextStyle, UiEvent, WidgetKind, find_prop,
};

use crate::custom::{DrawnArea, Emitter, ErasedRender, GtkCx, NativePayload};
use crate::host::{Events, Frames, Host, WindowRoot};
use crate::services::{ContextMenu, GtkServices, Menus, choose_context_item};
use crate::surface::SurfaceArea;

/// How the backend behaves; apps and tests want different things.
#[derive(Clone, Debug, Default)]
pub struct BackendOptions {
    /// Keep a log of applied commands (for tests).
    pub record_commands: bool,
    /// Force this appearance, so captures are comparable across machines
    /// regardless of system settings. GTK's settings are per display, so
    /// this applies to every window on it.
    pub appearance: Option<Appearance>,
}

/// Native widget → node. Shared with the windows' focus observers.
type WidgetMap = Rc<RefCell<HashMap<gtk::Widget, NodeId>>>;

pub(crate) struct WindowParts {
    pub(crate) window: gtk::Window,
    host: Host,
    header: gtk::HeaderBar,
    /// Measured again when toolbar items change it.
    header_height: i32,
    /// The toolbar items in the header bar, in order.
    items: Vec<(NodeId, gtk::Widget)>,
    /// The app's menu, GNOME style: a menu button at the end of the header bar.
    pub(crate) menu_button: gtk::MenuButton,
    pub(crate) shortcuts: gtk::ShortcutController,
    full_screen: FullScreen,
    min_size: MinSize,
}

/// The app's minimum content size, no larger than the window's monitor less
/// the header bar: a machine's mode can be larger than a laptop's screen,
/// and GTK would make the window as large as asked. GTK 4 has no work area
/// (the panels' room) on Wayland, so it's the monitor's whole geometry.
/// Applied again when the window goes to another monitor, or the header
/// bar's height changes.
#[derive(Clone, Default)]
struct MinSize {
    app: Rc<Cell<Option<Size>>>,
    header_height: Rc<Cell<i32>>,
}

impl MinSize {
    /// The minimum GTK is given, in whole points.
    fn capped(&self, window: &gtk::Window) -> Option<(i32, i32)> {
        let min = self.app.get()?;
        let (mut width, mut height) = (min.width.ceil() as i32, min.height.ceil() as i32);
        if let Some(monitor) = monitor_of(window) {
            let area = monitor.geometry();
            width = width.min(area.width());
            height = height.min((area.height() - self.header_height.get()).max(0));
        }
        Some((width, height))
    }

    /// Asked by the content, which the window's minimum follows (the header
    /// bar's above it); a window smaller grows to it, as GTK allocates no
    /// less.
    fn apply(&self, window: &gtk::Window, host: &Host) {
        let Some((width, height)) = self.capped(window) else { return };
        host.set_size_request(width, height);
        if let Some(root) = host.window_root() {
            let size = root.resizing.get().unwrap_or(root.size.get());
            let grown = Size::new(size.width.max(width as f32), size.height.max(height as f32));
            if grown != size {
                root.resizing.set(Some(grown));
            }
        }
    }

    /// The minimum as GTK has it: the app's, if GTK holds it as capped.
    fn shown(&self, window: &gtk::Window, host: &Host) -> Size {
        let (width, height) = host.size_request();
        match self.app.get() {
            Some(min) if self.capped(window) == Some((width, height)) => min,
            _ => Size::new(requested(width), requested(height)),
        }
    }
}

/// The monitor the window is on, or before it has a surface, the first.
fn monitor_of(window: &gtk::Window) -> Option<gdk::Monitor> {
    let display = WidgetExt::display(window);
    match window.surface() {
        Some(surface) => display.monitor_at_surface(&surface),
        None => display.monitors().item(0).and_downcast(),
    }
}

/// Full screen as the app wants it. GTK reports its own changes
/// (`notify::fullscreened`) like the user's, and later: the compositor
/// applies them when it configures the window. A change it didn't ask
/// for, or ended somewhere else, is the user's or the platform's.
#[derive(Clone, Default)]
struct FullScreen {
    wanted: Rc<Cell<bool>>,
    /// Asked for, and not in effect yet.
    pending: Rc<Cell<bool>>,
}

impl FullScreen {
    fn set(&self, window: &gtk::Window, on: bool) {
        self.wanted.set(on);
        self.pending.set(window.is_fullscreen() != on);
        if on { window.fullscreen() } else { window.unfullscreen() }
    }

    /// What the window shows, or while a request is pending (or it isn't
    /// shown yet), what it will.
    fn shown(&self, window: &gtk::Window) -> bool {
        if self.pending.get() { self.wanted.get() } else { window.is_fullscreen() }
    }

    fn in_effect(&self, window: &gtk::Window) -> bool {
        window.is_fullscreen() || (self.pending.get() && self.wanted.get())
    }
}

/// Gives a window a new default size. Some backends (Broadway before GTK
/// 4.16) only size a toplevel when its surface is presented, so a mapped
/// window ignores a new default size until then. `GtkWindow::present`
/// would only focus it.
fn resize(window: &gtk::Window, width: i32, height: i32) {
    window.set_default_size(width, height);
    if window.is_mapped()
        && let Some(toplevel) = window.surface().and_downcast::<gdk::Toplevel>()
    {
        let layout = gdk::ToplevelLayout::new();
        layout.set_resizable(window.is_resizable());
        toplevel.present(&layout);
    }
}

/// A size request's side: none (-1) is 0.
fn requested(side: i32) -> f32 {
    side.max(0) as f32
}

enum Widget {
    Window(WindowParts),
    Host(Host),
    Label(gtk::Label),
    Entry(gtk::Entry),
    Password(gtk::PasswordEntry),
    Button(gtk::Button),
    Checkbox(gtk::CheckButton),
    Switch(gtk::Switch),
    Select {
        dropdown: gtk::DropDown,
        options: gtk::StringList,
    },
    /// A scale, and its step and marks, also kept on the scale for
    /// [`crate::show_step_marks`].
    Slider {
        scale: gtk::Scale,
        steps: Rc<Steps>,
    },
    SpinButton(gtk::SpinButton),
    /// A progress bar, and whether it pulses: GTK shows work of unknown
    /// length by `pulse()` calls, which a timer makes while it's set.
    Progress {
        bar: gtk::ProgressBar,
        pulsing: Rc<Cell<bool>>,
    },
    Spinner(gtk::Spinner),
    /// A picture, and what it was given: GTK can't give pixels back, and
    /// reads its content fit back whether the app chose one or not.
    Picture {
        picture: gtk::Picture,
        source: Option<ImageSource>,
        fit: Option<ImageFit>,
    },
    GpuSurface(SurfaceArea),
    Scroll {
        scrolled: gtk::ScrolledWindow,
        viewport: gtk::Viewport,
    },
    List(crate::list::List),
    /// A custom widget with a GTK render, and the props it last got.
    Custom {
        widget: gtk::Widget,
        render: Rc<dyn ErasedRender>,
        props: CustomProps,
    },
    /// A drawn custom widget.
    Drawn {
        drawn: DrawnArea,
        props: CustomProps,
    },
    /// A native view from app code, and the last `Prop::Native` it got.
    Native {
        widget: gtk::Widget,
        measure: Option<NativeMeasure>,
        last: Opaque,
    },
}

type NativeMeasure = Rc<dyn Fn(&gtk::Widget, &MeasureRequest) -> Size>;

impl Widget {
    /// The widget that stands for the node: a window's content host.
    fn widget(&self) -> &gtk::Widget {
        match self {
            Widget::Window(WindowParts { host, .. }) | Widget::Host(host) => host.upcast_ref(),
            Widget::Label(w) => w.upcast_ref(),
            Widget::Entry(w) => w.upcast_ref(),
            Widget::Password(w) => w.upcast_ref(),
            Widget::Button(w) => w.upcast_ref(),
            Widget::Checkbox(w) => w.upcast_ref(),
            Widget::Switch(w) => w.upcast_ref(),
            Widget::Select { dropdown, .. } => dropdown.upcast_ref(),
            Widget::Slider { scale, .. } => scale.upcast_ref(),
            Widget::SpinButton(w) => w.upcast_ref(),
            Widget::Progress { bar, .. } => bar.upcast_ref(),
            Widget::Spinner(w) => w.upcast_ref(),
            Widget::Picture { picture, .. } => picture.upcast_ref(),
            Widget::GpuSurface(surface) => surface.area.upcast_ref(),
            Widget::Scroll { scrolled, .. } => scrolled.upcast_ref(),
            Widget::List(list) => list.scrolled.upcast_ref(),
            Widget::Custom { widget, .. } | Widget::Native { widget, .. } => widget,
            Widget::Drawn { drawn, .. } => drawn.area.upcast_ref(),
        }
    }

    /// Built-in controls: they take `Enabled`.
    fn is_control(&self) -> bool {
        matches!(
            self,
            Widget::Label(_)
                | Widget::Entry(_)
                | Widget::Password(_)
                | Widget::Button(_)
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select { .. }
                | Widget::Slider { .. }
                | Widget::SpinButton(_)
        )
    }

    /// Measured, never laid out inside: controls and escape hatches.
    fn is_leaf(&self) -> bool {
        !matches!(self, Widget::Window(_) | Widget::Host(_) | Widget::Scroll { .. } | Widget::List(_))
    }

    /// The widget that takes keyboard focus: a list's view, not its
    /// scrolled window.
    fn focus_widget(&self) -> gtk::Widget {
        match self {
            Widget::List(list) => list.view.clone().upcast(),
            widget => widget.widget().clone(),
        }
    }
}

struct Node {
    kind: WidgetKind,
    widget: Widget,
    parent: Option<NodeId>,
    /// Row hosts: which row of their list they show.
    row: Option<RowKey>,
    /// Props GTK can't report back faithfully.
    text_style: Option<TextStyle>,
    /// Labels: the colour the app gave, for the theme colours GTK has no
    /// class for.
    text_color: Option<Color>,
    role: Option<ButtonRole>,
    button_style: Option<ButtonStyle>,
    /// Sliders: whether the app gave an `Orientation`.
    orientation: Option<Orientation>,
    /// Checkboxes: whether the app gave `Mixed`.
    mixed: Option<bool>,
    /// The app's raw settings, run after every other prop.
    tweak: Option<Opaque>,
    /// Switches, selects, sliders and progress bars have no caption, only
    /// an accessible label, which GTK doesn't read back.
    a11y_label: Option<String>,
    /// Windows: modal, and the window they belong to.
    modal: Option<(Option<NodeId>, Modality)>,
    /// Signal handlers on objects that outlive the node.
    settings_handlers: Vec<glib::SignalHandlerId>,
    /// The context menu, once the app gave one.
    context_menu: Option<ContextMenu>,
}

pub(crate) struct State {
    options: BackendOptions,
    nodes: HashMap<NodeId, Node>,
    by_widget: WidgetMap,
    frames: Frames,
    events: Events,
    log: Vec<Command>,
    pending_show: Vec<NodeId>,
    /// The app's menus and windows' own, and each window's primary menu.
    pub(crate) menus: Menus,
    /// A list changed (its rows, a row's size, its scroll position): the
    /// list view must lay out again before its rows are known.
    lists_dirty: Cell<bool>,
}

impl Drop for State {
    /// GTK keeps toplevels alive until they're destroyed; a backend's
    /// windows go with it.
    fn drop(&mut self) {
        for menu in self.nodes.values().filter_map(|node| node.context_menu.as_ref()) {
            menu.close();
        }
        for node in self.nodes.values() {
            if let Widget::Window(parts) = &node.widget {
                parts.window.destroy();
            }
        }
    }
}

pub struct GtkBackend {
    state: Rc<RefCell<State>>,
}

/// Shared access to a [`GtkBackend`] after it was moved into a `Ui`.
#[derive(Clone)]
pub struct GtkHandle {
    state: Rc<RefCell<State>>,
}

/// GNOME's type scale, as style classes of the theme. There is no callout
/// size in GNOME, so callouts use the body size.
fn text_style_class(style: TextStyle) -> Option<&'static str> {
    match style {
        TextStyle::LargeTitle => Some("title-1"),
        TextStyle::Title => Some("title-2"),
        TextStyle::Headline => Some("heading"),
        TextStyle::Body | TextStyle::Callout => None,
        TextStyle::Caption => Some("caption"),
        TextStyle::Monospace => Some("monospace"),
    }
}

const TEXT_STYLE_CLASSES: [&str; 5] = ["title-1", "title-2", "heading", "caption", "monospace"];

/// Text colours the theme has a class for, so they follow it (light, dark,
/// high contrast, the accent). The rest are Pango attributes.
fn color_class(color: Color) -> Option<&'static str> {
    match color {
        Color::SecondaryLabel => Some("dim-label"),
        Color::Accent => Some("accent"),
        Color::Error => Some("error"),
        Color::Warning => Some("warning"),
        Color::Success => Some("success"),
        _ => None,
    }
}

const COLOR_CLASSES: [(&str, Color); 5] = [
    ("dim-label", Color::SecondaryLabel),
    ("accent", Color::Accent),
    ("error", Color::Error),
    ("warning", Color::Warning),
    ("success", Color::Success),
];

/// Replaces a label's Pango attributes of these types with `new`, keeping
/// the others: colour, weight and slant are set one at a time.
fn replace_attrs(label: &gtk::Label, types: &[pango::AttrType], new: Vec<pango::Attribute>) {
    let list = pango::AttrList::new();
    for attr in label.attributes().map(|l| l.attributes()).unwrap_or_default() {
        if !types.contains(&attr.type_()) {
            list.insert(attr);
        }
    }
    for attr in new {
        list.insert(attr);
    }
    label.set_attributes(Some(&list));
}

fn find_attr(label: &gtk::Label, type_: pango::AttrType) -> Option<pango::Attribute> {
    label.attributes()?.attributes().into_iter().find(|a| a.type_() == type_)
}

fn pango_weight(weight: FontWeight) -> pango::Weight {
    match weight {
        FontWeight::Regular => pango::Weight::Normal,
        FontWeight::Medium => pango::Weight::Medium,
        FontWeight::Semibold => pango::Weight::Semibold,
        FontWeight::Bold => pango::Weight::Bold,
    }
}

fn font_weight(weight: i32) -> FontWeight {
    match weight {
        ..450 => FontWeight::Regular,
        450..550 => FontWeight::Medium,
        550..650 => FontWeight::Semibold,
        _ => FontWeight::Bold,
    }
}

/// A label's colour as its classes and attributes show it. Theme colours
/// without a class are attributes that can't be told from `Rgba`, so
/// those come from what the app set.
fn label_color(label: &gtk::Label, set: Option<Color>) -> Option<Color> {
    if let Some((_, color)) = COLOR_CLASSES.iter().find(|(class, _)| label.has_css_class(class)) {
        return Some(*color);
    }
    let Some(fg) = find_attr(label, pango::AttrType::Foreground) else { return set.map(|_| Color::Label) };
    if let Some(c @ (Color::Separator | Color::ControlBackground | Color::WindowBackground)) = set {
        return Some(c);
    }
    let fg = fg.downcast_ref::<pango::AttrColor>()?.color();
    let alpha = find_attr(label, pango::AttrType::ForegroundAlpha)
        .and_then(|a| a.downcast_ref::<pango::AttrInt>().map(|a| a.value()))
        .unwrap_or(65535);
    let byte = |v: u16| (v / 257) as u8;
    Some(Color::Rgba(byte(fg.red()), byte(fg.green()), byte(fg.blue()), byte(alpha as u16)))
}

/// GNOME has no cancel style: cancel buttons are normal buttons.
fn role_class(role: ButtonRole) -> Option<&'static str> {
    match role {
        ButtonRole::Normal | ButtonRole::Cancel => None,
        ButtonRole::Default => Some("suggested-action"),
        ButtonRole::Destructive => Some("destructive-action"),
    }
}

const ROLE_CLASSES: [&str; 2] = ["suggested-action", "destructive-action"];

/// The font size the theme gives a text style, in logical px.
fn font_size(style: TextStyle) -> f32 {
    let label = gtk::Label::new(None);
    if let Some(class) = text_style_class(style) {
        label.add_css_class(class);
    }
    let Some(font) = label.pango_context().font_description() else { return 16.0 };
    let size = font.size() as f32 / pango::SCALE as f32;
    // Point sizes are CSS points: 4/3 px.
    if font.is_size_absolute() { size } else { size * 4.0 / 3.0 }
}

fn metrics() -> PlatformMetrics {
    let settings = gtk::Settings::default();
    let theme = settings.as_ref().and_then(|s| s.gtk_theme_name()).unwrap_or_default().to_lowercase();
    let scale = gdk::Display::default()
        .and_then(|d| d.monitors().item(0))
        .and_then(|m| m.downcast::<gdk::Monitor>().ok())
        .map_or(1.0, |m| m.scale_factor() as f32);
    PlatformMetrics {
        scale_factor: scale,
        // GNOME spaces in multiples of 6px; 24px is a generous window margin.
        spacing: SpacingScale { xs: 3.0, sm: 6.0, md: 12.0, lg: 18.0, xl: 24.0 },
        font_sizes: FontSizes {
            large_title: font_size(TextStyle::LargeTitle),
            title: font_size(TextStyle::Title),
            headline: font_size(TextStyle::Headline),
            body: font_size(TextStyle::Body),
            callout: font_size(TextStyle::Callout),
            caption: font_size(TextStyle::Caption),
            monospace: font_size(TextStyle::Monospace),
        },
        dark_mode: settings.as_ref().is_some_and(|s| s.is_gtk_application_prefer_dark_theme())
            || theme.contains("dark"),
        high_contrast: theme.contains("highcontrast"),
        reduced_motion: settings.as_ref().is_some_and(|s| !s.is_gtk_enable_animations()),
    }
}

fn violation(command: &Command, problem: &str) -> ! {
    panic!("gtk backend: protocol violation in {command:?}: {problem}")
}

/// Runs the GTK main context until `done` holds or `timeout` passes.
fn pump_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let context = glib::MainContext::default();
    let started = Instant::now();
    loop {
        while context.iteration(false) {}
        if done() {
            return true;
        }
        if started.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The node a widget belongs to: the nearest known widget among it and its
/// ancestors. Composite widgets (an entry's text, a scroll view's
/// viewport) put focus on children we didn't create.
fn owning_node(map: &WidgetMap, widget: Option<gtk::Widget>) -> Option<NodeId> {
    let map = map.borrow();
    let mut current = widget;
    while let Some(widget) = current {
        if let Some(id) = map.get(&widget) {
            return Some(*id);
        }
        current = widget.parent();
    }
    None
}

impl GtkBackend {
    /// GTK must be initialized (`gtk::init`) on this thread.
    pub fn new(options: BackendOptions) -> GtkBackend {
        assert!(gtk::is_initialized_main_thread(), "mitsuami: initialize GTK on the main thread first");
        if let Some(appearance) = options.appearance
            && let Some(settings) = gtk::Settings::default()
        {
            settings.set_gtk_application_prefer_dark_theme(appearance == Appearance::Dark);
            settings.set_gtk_theme_name(Some("Adwaita"));
        }
        let state = Rc::new(RefCell::new(State {
            options,
            nodes: HashMap::new(),
            by_widget: WidgetMap::default(),
            frames: Frames::default(),
            events: Events::default(),
            log: Vec::new(),
            pending_show: Vec::new(),
            menus: Menus::default(),
            lists_dirty: Cell::new(false),
        }));
        state.borrow_mut().menus.backend = Rc::downgrade(&state);
        GtkBackend { state }
    }

    pub fn handle(&self) -> GtkHandle {
        GtkHandle { state: self.state.clone() }
    }
}

impl GtkHandle {
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

    /// Resizes a window's content like the user would, no smaller than its
    /// minimum (GTK allocates no less), and waits until GTK has allocated
    /// it; the content host reports it as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let Some((gtk_window, host, header_height)) = self.window_parts(window) else { return };
        // The user can't resize it.
        if !gtk_window.is_resizable() {
            return;
        }
        let (min_width, min_height) = host.size_request();
        let size = Size::new(size.width.max(requested(min_width)), size.height.max(requested(min_height)));
        resize(&gtk_window, size.width as i32, size.height as i32 + header_height);
        let target = (size.width as i32, size.height as i32);
        pump_until(Duration::from_secs(2), || (WidgetExt::width(&host), WidgetExt::height(&host)) == target);
    }

    /// Presents windows whose first layout has been applied.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            if let Some(window) = self.gtk_window(id) {
                window.present();
            }
        }
    }

    /// Lets list views that changed lay out now, rather than at the next
    /// frame: they bind rows (and place them) when allocated, and on a
    /// display nobody watches, frames stall. Each list's scrolled window is
    /// allocated again at its frame, as its host does; then the rows the
    /// list views report are delivered.
    fn layout_lists(&self) {
        let lists: Vec<(gtk::ScrolledWindow, Rect)> = {
            let state = self.state.borrow();
            if !state.lists_dirty.replace(false) {
                return;
            }
            let frames = state.frames.borrow();
            state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::List(list) if list.scrolled.is_mapped() => {
                        let frame = frames.get(list.scrolled.upcast_ref::<gtk::Widget>()).copied()?;
                        Some((list.scrolled.clone(), frame))
                    }
                    _ => None,
                })
                .collect()
        };
        for (scrolled, frame) in lists {
            scrolled.measure(gtk::Orientation::Horizontal, -1);
            let transform = gsk::Transform::new().translate(&graphene::Point::new(frame.x(), frame.y()));
            scrolled.allocate(frame.width().round() as i32, frame.height().round() as i32, -1, Some(transform));
        }
        self.pump();
    }

    /// Waits for mapped windows to reach the size the app or their minimum
    /// asked for, which GTK allocates (and the host reports) at the next
    /// frame; a platform that gives another size is waited for no longer
    /// than a user's resize is.
    fn wait_for_resizes(&self) {
        let ids: Vec<NodeId> = self.windows().into_iter().map(|(id, _)| id).collect();
        for (window, host, _) in ids.into_iter().filter_map(|id| self.window_parts(id)) {
            let Some(root) = host.window_root() else { continue };
            let Some(size) = root.resizing.get() else { continue };
            if !window.is_mapped() {
                continue;
            }
            let target = (size.width as i32, size.height as i32);
            pump_until(Duration::from_secs(2), || (WidgetExt::width(&host), WidgetExt::height(&host)) == target);
            root.resizing.set(None);
        }
    }

    /// Lets GPU surfaces whose frame changed take it now, rather than at
    /// the next frame, as lists do: each reports its new size when
    /// allocated, and the app draws at it.
    fn layout_surfaces(&self) {
        let surfaces: Vec<(gtk::DrawingArea, Rect)> = {
            let state = self.state.borrow();
            let frames = state.frames.borrow();
            state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::GpuSurface(surface) if surface.area.is_mapped() => {
                        let frame = frames.get(surface.area.upcast_ref::<gtk::Widget>()).copied()?;
                        let size = (frame.width().round() as i32, frame.height().round() as i32);
                        let allocated = (WidgetExt::width(&surface.area), WidgetExt::height(&surface.area));
                        (size != allocated).then(|| (surface.area.clone(), frame))
                    }
                    _ => None,
                })
                .collect()
        };
        for (area, frame) in surfaces {
            area.measure(gtk::Orientation::Horizontal, -1);
            let transform = gsk::Transform::new().translate(&graphene::Point::new(frame.x(), frame.y()));
            area.allocate(frame.width().round() as i32, frame.height().round() as i32, -1, Some(transform));
        }
        self.pump();
    }

    /// Lets header bars whose items changed place them now, rather than at
    /// the next frame, as lists do: each is allocated again where it is.
    fn layout_headers(&self) {
        let headers: Vec<gtk::HeaderBar> = {
            let state = self.state.borrow();
            state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::Window(parts) if parts.header.is_mapped() && parts.header.should_layout() => {
                        Some(parts.header.clone())
                    }
                    _ => None,
                })
                .collect()
        };
        for header in headers {
            let Some(bounds) = header.parent().and_then(|p| header.compute_bounds(&p)) else { continue };
            header.measure(gtk::Orientation::Horizontal, -1);
            let transform = gsk::Transform::new().translate(&graphene::Point::new(bounds.x(), bounds.y()));
            header.allocate(bounds.width().round() as i32, bounds.height().round() as i32, -1, Some(transform));
        }
        self.pump();
    }

    /// Dispatches whatever the GTK main context has ready, without waiting.
    pub fn pump(&self) {
        let context = glib::MainContext::default();
        while context.iteration(false) {}
    }

    /// Escape hatch: the native window of a window node.
    pub fn gtk_window(&self, id: NodeId) -> Option<gtk::Window> {
        self.window_parts(id).map(|(window, _, _)| window)
    }

    /// The actions of a window's primary menu, for tests: GTK has no getter
    /// for a widget's action groups.
    #[doc(hidden)]
    pub fn menu_actions(&self, window: NodeId) -> Option<gtk::gio::ActionGroup> {
        self.state.borrow().menus.actions(window)
    }

    /// Escape hatch: the native widget of any node (a window's content host).
    pub fn gtk_widget(&self, id: NodeId) -> Option<gtk::Widget> {
        self.state.borrow().nodes.get(&id).map(|n| n.widget.widget().clone())
    }

    fn window_parts(&self, id: NodeId) -> Option<(gtk::Window, Host, i32)> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window(parts) => Some((parts.window.clone(), parts.host.clone(), parts.header_height)),
            _ => None,
        }
    }

    /// Every window, for services (menus, dialog parents).
    pub(crate) fn windows(&self) -> Vec<(NodeId, gtk::Window)> {
        let state = self.state.borrow();
        let mut windows: Vec<(NodeId, gtk::Window)> = state
            .nodes
            .iter()
            .filter_map(|(id, n)| match &n.widget {
                Widget::Window(parts) => Some((*id, parts.window.clone())),
                _ => None,
            })
            .collect();
        windows.sort_by_key(|(id, _)| *id);
        windows
    }

    /// Asks the app to close every window (the Quit command).
    pub(crate) fn request_quit(&self) {
        let events = self.state.borrow().events.clone();
        for (id, _) in self.windows() {
            events.emit(id, UiEvent::WindowCloseRequested);
        }
    }

    pub(crate) fn from_weak(state: &Weak<RefCell<State>>) -> Option<GtkHandle> {
        state.upgrade().map(|state| GtkHandle { state })
    }

    /// Takes the app's menus (`window` is `None`) or a window's own, and
    /// brings the primary menus showing them up to date.
    pub(crate) fn set_menu(&self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let mut state = self.state.borrow_mut();
        let State { nodes, menus, .. } = &mut *state;
        menus.set(window, menu, activate);
        for (id, node) in nodes.iter() {
            if let Widget::Window(parts) = &node.widget
                && window.is_none_or(|w| w == *id)
            {
                menus.show_in(*id, parts);
            }
        }
    }
}

impl mitsuami_core::TestHooks for GtkHandle {
    fn name(&self) -> &'static str {
        "gtk"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        GtkHandle::resize_window(self, window, size);
    }

    /// As the close button does: GTK emits `close-request`.
    fn close_window(&self, window: NodeId) {
        if let Some((gtk_window, _, _)) = self.window_parts(window) {
            gtk_window.close();
        }
    }

    fn take_command_log(&self) -> Vec<Command> {
        GtkHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        GtkHandle::node_count(self)
    }

    fn app_info(&self, window: NodeId) -> NativeAppInfo {
        let icon = self
            .window_parts(window)
            .and_then(|(window, _, _)| window.icon_name())
            .or_else(gtk::Window::default_icon_name);
        NativeAppInfo {
            id: glib::prgname().map(Into::into),
            name: glib::application_name().map(Into::into),
            icon: icon.map(|name| NativeIcon::Named(name.into())),
        }
    }

    /// Windows are shown once laid out, and GTK delivers what it queued
    /// (allocations, scroll adjustments, focus).
    fn settle(&self) {
        self.show_pending_windows();
        self.pump();
        self.layout_lists();
        self.layout_headers();
        self.layout_surfaces();
        self.wait_for_resizes();
    }
}

/// Keeps a scroll view's adjustments in step with the frames the core sent,
/// without waiting for GTK to allocate: `ScrollTo` in the same commit
/// needs the new range.
/// Packs a window's toolbar items at the end of its header bar, in order:
/// `pack_end` packs from the end inwards, so the last item goes first. The
/// main menu button, packed when the window was made, stays at the very
/// end, as GNOME's primary menu is.
fn pack_items(parts: &WindowParts) {
    for (_, widget) in &parts.items {
        if widget.parent().is_some() {
            parts.header.remove(widget);
        }
    }
    for (_, widget) in parts.items.iter().rev() {
        parts.header.pack_end(widget);
    }
}

/// An item taller than the header bar makes it taller; the window grows
/// with it, so the content keeps the size the core asked for.
fn keep_content_size(parts: &mut WindowParts) {
    let height = parts.header.measure(gtk::Orientation::Vertical, -1).1;
    if height == parts.header_height {
        return;
    }
    parts.header_height = height;
    parts.min_size.header_height.set(height);
    parts.min_size.apply(&parts.window, &parts.host);
    let size = parts.host.window_root().expect("window hosts have a root").size.get();
    parts.window.set_default_size(size.width as i32, size.height as i32 + height);
}

fn sync_scroll(frames: &Frames, scrolled: &gtk::ScrolledWindow, viewport: &gtk::Viewport) {
    let frames = frames.borrow();
    let view = frames.get(scrolled.upcast_ref::<gtk::Widget>()).map(|f| f.size).unwrap_or_default();
    let content = viewport.child().and_then(|c| frames.get(&c).map(|f| f.size)).unwrap_or(view);
    for (adjustment, content, view) in
        [(scrolled.hadjustment(), content.width, view.width), (scrolled.vadjustment(), content.height, view.height)]
    {
        let (content, view) = (content as f64, view as f64);
        let upper = content.max(view);
        let value = adjustment.value().clamp(0.0, upper - view);
        adjustment.configure(value, 0.0, upper, view * 0.1, view * 0.9, view);
    }
}

/// Scrolls like the user would: GTK reports it through the adjustments.
fn scroll_to(scrolled: &gtk::ScrolledWindow, offset: Point) {
    scrolled.hadjustment().set_value(offset.x as f64);
    scrolled.vadjustment().set_value(offset.y as f64);
}

fn scroll_offset(scrolled: &gtk::ScrolledWindow) -> Point {
    Point::new(scrolled.hadjustment().value() as f32, scrolled.vadjustment().value() as f32)
}

/// Scroll bars (as the theme draws them) on the axes that scroll; without,
/// `External` still scrolls them by wheel, touchpad and touch.
fn set_scroll_policy(scrolled: &gtk::ScrolledWindow, axes: ScrollAxes, show: bool) {
    let on = if show { gtk::PolicyType::Automatic } else { gtk::PolicyType::External };
    let policy = |scrolls: bool| if scrolls { on } else { gtk::PolicyType::Never };
    scrolled.set_policy(policy(axes.horizontal()), policy(axes.vertical()));
}

fn scroll_axes(scrolled: &gtk::ScrolledWindow) -> ScrollAxes {
    match scrolled.policy() {
        (gtk::PolicyType::Never, _) => ScrollAxes::Vertical,
        (_, gtk::PolicyType::Never) => ScrollAxes::Horizontal,
        _ => ScrollAxes::Both,
    }
}

fn scroll_bars(scrolled: &gtk::ScrolledWindow) -> bool {
    let (h, v) = scrolled.policy();
    h != gtk::PolicyType::External && v != gtk::PolicyType::External
}

impl State {
    fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) {
        let events = self.events.clone();
        let mut settings_handlers = Vec::new();
        let widget = match kind {
            WidgetKind::Window => {
                // A dialog's menu holds only its own menus.
                if let Command::Create { props, .. } = command
                    && props.iter().any(|p| matches!(p, Prop::Modal { .. }))
                {
                    self.menus.set_modal(id);
                }
                Widget::Window(self.create_window(id, &mut settings_handlers))
            }
            WidgetKind::Container | WidgetKind::ToolbarItem => Widget::Host(Host::new(self.frames.clone(), None)),
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not a GTK one")
                        };
                        let widget = render.create(custom.props(), &mut GtkCx::new(events.clone(), id));
                        Widget::Custom { widget, render, props: custom }
                    }
                    None => Widget::Drawn { drawn: DrawnArea::new(events.clone(), id), props: custom },
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not a GTK one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let measure = payload.spec.measure.clone();
                let widget = create(&mut GtkCx::new(events.clone(), id));
                payload.apply(&widget);
                Widget::Native { widget, measure, last: opaque }
            }
            WidgetKind::Text => {
                let label = gtk::Label::new(None);
                label.set_wrap(true);
                label.set_wrap_mode(pango::WrapMode::Word);
                // Start-aligned and top-aligned in frames larger than the
                // text (GTK mirrors xalign for right-to-left text).
                label.set_xalign(0.0);
                label.set_yalign(0.0);
                Widget::Label(label)
            }
            WidgetKind::Button => {
                let button = gtk::Button::new();
                button.connect_clicked(move |_| events.emit(id, UiEvent::Click));
                Widget::Button(button)
            }
            WidgetKind::Checkbox => {
                let check = gtk::CheckButton::new();
                check.connect_toggled(move |c| {
                    // GTK leaves `inconsistent` to the app, and apps clear it
                    // when the user toggles the box.
                    if !events.is_muted() {
                        c.set_inconsistent(false);
                    }
                    events.emit(id, UiEvent::Changed(EventValue::Bool(c.is_active())))
                });
                Widget::Checkbox(check)
            }
            WidgetKind::Switch => {
                let switch = gtk::Switch::new();
                switch
                    .connect_active_notify(move |s| events.emit(id, UiEvent::Changed(EventValue::Bool(s.is_active()))));
                Widget::Switch(switch)
            }
            WidgetKind::Select => {
                let options = gtk::StringList::new(&[]);
                let dropdown = gtk::DropDown::new(Some(options.clone()), None::<gtk::Expression>);
                dropdown.connect_selected_notify(move |d| {
                    if d.selected() != gtk::INVALID_LIST_POSITION {
                        events.emit(id, UiEvent::Changed(EventValue::Index(d.selected() as usize)))
                    }
                });
                Widget::Select { dropdown, options }
            }
            WidgetKind::Slider => {
                let scale = gtk::Scale::new(gtk::Orientation::Horizontal, None::<&gtk::Adjustment>);
                scale.connect_value_changed(move |s| events.emit(id, UiEvent::Changed(EventValue::Number(s.value()))));
                let steps = Rc::new(Steps::default());
                // SAFETY: the key only ever holds an `Rc<Steps>`.
                unsafe { scale.set_data(STEPS, steps.clone()) };
                // GTK's scales have no stepped mode: with a step, the user's
                // moves stop only on steps, as on AppKit and WinUI.
                let s = steps.clone();
                scale.connect_change_value(move |scale, _, value| match s.step.get() {
                    Some(step) if step > 0.0 => {
                        scale.set_value(snap(&scale.adjustment(), value, step));
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                });
                Widget::Slider { scale, steps }
            }
            WidgetKind::NumberInput => {
                // Steps of 1 and pages of 10, as `gtk_spin_button_new_with_range`
                // makes them, until the app gives a step.
                let spin = gtk::SpinButton::with_range(0.0, 100.0, 1.0);
                spin.set_digits(0);
                spin.set_numeric(true);
                // Typing is reported when GTK commits it: on Return or
                // when the field loses focus.
                spin.connect_value_changed(move |s| events.emit(id, UiEvent::Changed(EventValue::Number(s.value()))));
                Widget::SpinButton(spin)
            }
            WidgetKind::Progress => Widget::Progress { bar: gtk::ProgressBar::new(), pulsing: Rc::default() },
            WidgetKind::Spinner => Widget::Spinner(gtk::Spinner::new()),
            WidgetKind::Image => Widget::Picture { picture: gtk::Picture::new(), source: None, fit: None },
            WidgetKind::GpuSurface => Widget::GpuSurface(SurfaceArea::new(id, events.clone())),
            WidgetKind::TextInput => {
                let entry = gtk::Entry::new();
                let e = events.clone();
                entry.connect_changed(move |entry| {
                    e.emit(id, UiEvent::Changed(EventValue::Text(entry.text().to_string())))
                });
                // Only Return activates; leaving the field doesn't submit.
                entry.connect_activate(move |_| events.emit(id, UiEvent::Submit));
                Widget::Entry(entry)
            }
            // Without the peek icon, GTK's default: GNOME apps add it where
            // they want it.
            WidgetKind::PasswordInput => {
                let entry = gtk::PasswordEntry::new();
                let e = events.clone();
                entry.connect_changed(move |entry| {
                    e.emit(id, UiEvent::Changed(EventValue::Text(entry.text().to_string())))
                });
                // Only Return activates; leaving the field doesn't submit.
                entry.connect_activate(move |_| events.emit(id, UiEvent::Submit));
                Widget::Password(entry)
            }
            WidgetKind::ScrollView => {
                let scrolled = gtk::ScrolledWindow::new();
                scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                let viewport = gtk::Viewport::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
                scrolled.set_child(Some(&viewport));
                let (h, v) = (scrolled.hadjustment(), scrolled.vadjustment());
                for adjustment in [&h, &v] {
                    let (events, h, v) = (events.clone(), h.clone(), v.clone());
                    adjustment.connect_value_changed(move |_| {
                        events.emit(id, UiEvent::Scrolled(Point::new(h.value() as f32, v.value() as f32)))
                    });
                }
                Widget::Scroll { scrolled, viewport }
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
            WidgetKind::List => Widget::List(crate::list::List::new(id, events.clone())),
        };
        self.by_widget.borrow_mut().insert(widget.widget().clone(), id);
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                parent: None,
                row: None,
                text_style: None,
                text_color: None,
                role: None,
                button_style: None,
                orientation: None,
                mixed: None,
                tweak: None,
                a11y_label: None,
                modal: None,
                settings_handlers,
                context_menu: None,
            },
        );
    }

    fn create_window(&mut self, id: NodeId, settings_handlers: &mut Vec<glib::SignalHandlerId>) -> WindowParts {
        let events = self.events.clone();
        let window = gtk::Window::new();
        // An explicit header bar has a known height, so the content gets
        // exactly the size the core asks for.
        let header = gtk::HeaderBar::new();
        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_tooltip_text(Some("Main Menu"));
        menu_button.set_primary(true);
        header.pack_end(&menu_button);
        window.set_titlebar(Some(&header));
        // Measured with the menu button in, so showing it changes nothing.
        let header_height = header.measure(gtk::Orientation::Vertical, -1).1;
        menu_button.set_visible(false);
        let shortcuts = gtk::ShortcutController::new();
        window.add_controller(shortcuts.clone());

        let root = WindowRoot {
            id,
            events: events.clone(),
            size: Cell::new(Size::ZERO),
            resizing: Cell::new(None),
            focus_order: RefCell::new(Vec::new()),
        };
        let host = Host::new(self.frames.clone(), Some(root));
        window.set_child(Some(&host));

        let e = events.clone();
        // The app decides whether a window closes (e.g. to ask about
        // unsaved changes); the core destroys it if so.
        window.connect_close_request(move |_| {
            e.emit(id, UiEvent::WindowCloseRequested);
            glib::Propagation::Stop
        });
        // One observer for every focus change: clicks, Tab, code.
        let (e, map, focused) = (events.clone(), self.by_widget.clone(), Cell::new(None));
        window.connect_focus_widget_notify(move |window| {
            let now = owning_node(&map, GtkWindowExt::focus(window));
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
        window.connect_scale_factor_notify(move |_| e.emit(id, UiEvent::MetricsChanged));
        let full_screen = FullScreen::default();
        let (e, fs) = (events.clone(), full_screen.clone());
        window.connect_fullscreened_notify(move |window| {
            let now = window.is_fullscreen();
            // The app's own request, in effect.
            if fs.pending.replace(false) && now == fs.wanted.get() {
                return;
            }
            if now != fs.wanted.replace(now) {
                e.emit(id, UiEvent::FullScreenChanged(now));
            }
        });
        if let Some(settings) = gtk::Settings::default() {
            for property in ["gtk-font-name", "gtk-theme-name", "gtk-application-prefer-dark-theme"] {
                let e = events.clone();
                settings_handlers.push(
                    settings.connect_notify_local(Some(property), move |_, _| e.emit(id, UiEvent::MetricsChanged)),
                );
            }
        }
        let min_size = MinSize::default();
        min_size.header_height.set(header_height);
        // Another monitor, another cap on the minimum.
        let (min, h) = (min_size.clone(), host.clone());
        window.connect_realize(move |window| {
            let Some(surface) = window.surface() else { return };
            let (min, host, window) = (min.clone(), h.clone(), window.clone());
            surface.connect_enter_monitor(move |_, _| min.apply(&window, &host));
        });
        self.pending_show.push(id);
        let parts = WindowParts {
            window,
            host,
            header,
            header_height,
            items: Vec::new(),
            menu_button,
            shortcuts,
            full_screen,
            min_size,
        };
        self.menus.show_in(id, &parts);
        parts
    }

    fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) {
        // Transient for the window it belongs to, and modal. GTK's modal
        // windows block the whole app, so both modalities are the same
        // here; GNOME attaches modal dialogs to their parent itself. Not
        // `destroy_with_parent`: the core destroys it.
        if let Prop::Modal { owner, modality } = prop {
            let owner = owner.and_then(|o| match self.nodes.get(&o).map(|n| &n.widget) {
                Some(Widget::Window(parts)) => Some(parts.window.clone()),
                _ => None,
            });
            let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
            if let Widget::Window(parts) = &node.widget {
                parts.window.set_transient_for(owner.as_ref());
                parts.window.set_modal(true);
                if node.modal.is_none() {
                    parts.window.add_controller(escape_closes());
                }
                node.modal = Some((prop_owner(prop), *modality));
                if self.menus.set_modal(id) {
                    self.menus.show_in(id, parts);
                }
            }
            return;
        }
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window(parts)) => parts.window.set_title(Some(t)),
            (Prop::FullScreen(on), Widget::Window(parts)) => parts.full_screen.set(&parts.window, *on),
            (Prop::MinSize(min), Widget::Window(parts)) => {
                parts.min_size.app.set(Some(*min));
                parts.min_size.apply(&parts.window, &parts.host);
            }
            // GTK 4 can't hold one side of a window: the content sets its
            // size, which the user can't change, as GNOME's dialogs that
            // fit their content.
            (Prop::HeightFollowsContent(on), Widget::Window(parts)) => parts.window.set_resizable(!*on),
            (Prop::Text(t), Widget::Label(l)) => l.set_text(t),
            // GTK limits the lines of wrapping labels that ellipsize.
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                l.set_lines(lines.map_or(-1, |n| n as i32));
                l.set_ellipsize(if lines.is_some() { pango::EllipsizeMode::End } else { pango::EllipsizeMode::None });
            }
            (Prop::TextColor(color), Widget::Label(l)) => {
                for (class, _) in COLOR_CLASSES {
                    l.remove_css_class(class);
                }
                let mut attrs = Vec::new();
                match color_class(*color) {
                    Some(class) => l.add_css_class(class),
                    // The theme's foreground, as the label has without.
                    None if *color == Color::Label => {}
                    None => {
                        let rgba = crate::custom::rgba(l.upcast_ref(), *color);
                        let channel = |v: f32| (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
                        attrs.push(
                            pango::AttrColor::new_foreground(
                                channel(rgba.red()),
                                channel(rgba.green()),
                                channel(rgba.blue()),
                            )
                            .into(),
                        );
                        attrs.push(pango::AttrInt::new_foreground_alpha(channel(rgba.alpha())).into());
                    }
                }
                replace_attrs(l, &[pango::AttrType::Foreground, pango::AttrType::ForegroundAlpha], attrs);
                node.text_color = Some(*color);
            }
            // Over the text style's class, whose weight it replaces.
            (Prop::FontWeight(weight), Widget::Label(l)) => {
                replace_attrs(
                    l,
                    &[pango::AttrType::Weight],
                    vec![pango::AttrInt::new_weight(pango_weight(*weight)).into()],
                );
            }
            (Prop::Italic(italic), Widget::Label(l)) => {
                let style = if *italic { pango::Style::Italic } else { pango::Style::Normal };
                replace_attrs(l, &[pango::AttrType::Style], vec![pango::AttrInt::new_style(style).into()]);
            }
            // The core resolved the direction, so left is left: GTK mirrors
            // `xalign` and `justify` in right-to-left widgets.
            (Prop::TextAlign(align), Widget::Label(l)) => {
                l.set_direction(gtk::TextDirection::Ltr);
                let (xalign, justify) = match align {
                    HorizontalAlign::Left => (0.0, gtk::Justification::Left),
                    HorizontalAlign::Center => (0.5, gtk::Justification::Center),
                    HorizontalAlign::Right => (1.0, gtk::Justification::Right),
                };
                l.set_xalign(xalign);
                l.set_justify(justify);
            }
            (Prop::Label(t), Widget::Button(b)) => b.set_label(t),
            (Prop::Label(t), Widget::Checkbox(c)) => c.set_label(Some(t)),
            (Prop::Label(t), Widget::Switch(s)) => {
                s.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::Select { dropdown, .. }) => {
                dropdown.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Options(new), Widget::Select { dropdown, options }) => {
                // Replacing the items moves the selection; the chosen index
                // stays if it can, else the first option is chosen, as the
                // core does. It sends the index when that changes it.
                let chosen = dropdown.selected();
                let new: Vec<&str> = new.iter().map(String::as_str).collect();
                options.splice(0, options.n_items(), &new);
                let count = options.n_items();
                if count > 0 {
                    dropdown.set_selected(if chosen < count { chosen } else { 0 });
                }
            }
            (Prop::Label(t), Widget::Slider { scale, .. }) => {
                scale.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Range { min, max }, Widget::Slider { scale, steps }) => {
                scale.set_range(*min, *max);
                set_increments(scale, steps.step.get());
                update_marks(scale, steps);
            }
            (Prop::Step(new), Widget::Slider { scale, steps }) => {
                steps.step.set(*new);
                set_increments(scale, *new);
                update_marks(scale, steps);
            }
            (Prop::Number(n), Widget::Slider { scale, .. }) => scale.set_value(*n),
            (Prop::Orientation(o), Widget::Slider { scale, .. }) => {
                scale.set_orientation(if o.vertical() {
                    gtk::Orientation::Vertical
                } else {
                    gtk::Orientation::Horizontal
                });
                // GTK runs vertical scales top down; apps invert them so
                // that up is more, as on the other platforms.
                scale.set_inverted(o.vertical());
                node.orientation = Some(*o);
            }
            (Prop::Label(t), Widget::SpinButton(spin)) => {
                spin.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            // May clamp the value; the core sends the value after the range.
            (Prop::Range { min, max }, Widget::SpinButton(spin)) => spin.set_range(*min, *max),
            (Prop::Step(step), Widget::SpinButton(spin)) => {
                let step = step.unwrap_or(1.0);
                spin.set_increments(step, step * 10.0);
            }
            (Prop::Number(n), Widget::SpinButton(spin)) => spin.set_value(*n),
            (Prop::Label(t), Widget::Progress { bar, .. }) => {
                bar.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::Spinner(s)) => {
                s.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::GpuSurface(surface)) => {
                surface.area.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (
                Prop::TakesInput(_) | Prop::PointerLock(_) | Prop::KeyboardGrab(_) | Prop::Cursor(_),
                Widget::GpuSurface(surface),
            ) => surface.set_prop(prop),
            (Prop::Label(t), Widget::Picture { picture, .. }) => {
                picture.set_alternative_text(Some(t));
                node.a11y_label = Some(t.clone());
            }
            // GTK decodes files as it's given them; one it can't read
            // shows nothing, and measures nothing.
            (Prop::Image(new), Widget::Picture { picture, source, .. }) => {
                match new {
                    ImageSource::File(path) => picture.set_filename(Some(path)),
                    ImageSource::Pixels(pixels) => {
                        let texture = gdk::MemoryTexture::new(
                            pixels.width() as i32,
                            pixels.height() as i32,
                            gdk::MemoryFormat::R8g8b8a8,
                            &glib::Bytes::from(pixels.rgba()),
                            pixels.width() as usize * 4,
                        );
                        picture.set_paintable(Some(&texture));
                    }
                }
                *source = Some(new.clone());
            }
            (Prop::ImageFit(new), Widget::Picture { picture, fit, .. }) => {
                picture.set_content_fit(match new {
                    ImageFit::Contain => gtk::ContentFit::Contain,
                    ImageFit::Stretch => gtk::ContentFit::Fill,
                });
                *fit = Some(*new);
            }
            // Stopped, a GTK spinner draws nothing.
            (Prop::Running(r), Widget::Spinner(s)) => s.set_spinning(*r),
            (Prop::Progress(progress), Widget::Progress { bar, pulsing }) => match progress {
                Some(fraction) => {
                    pulsing.set(false);
                    bar.set_fraction(*fraction);
                }
                None if !pulsing.replace(true) => {
                    let (bar, pulsing) = (bar.downgrade(), pulsing.clone());
                    glib::timeout_add_local(Duration::from_millis(100), move || match bar.upgrade() {
                        Some(bar) if pulsing.get() => {
                            bar.pulse();
                            glib::ControlFlow::Continue
                        }
                        _ => glib::ControlFlow::Break,
                    });
                }
                None => {}
            },
            // With options, GTK always has one chosen (its selection
            // autoselects), and so does the core.
            (Prop::SelectedIndex(Some(index)), Widget::Select { dropdown, .. }) => dropdown.set_selected(*index as u32),
            (Prop::Value(t), Widget::Entry(e)) => {
                // Don't disturb the caret when the field already shows it.
                if e.text() != t.as_str() {
                    e.set_text(t);
                }
            }
            (Prop::Placeholder(t), Widget::Entry(e)) => e.set_placeholder_text(Some(t)),
            // Still focusable and selectable, so its text can be copied.
            (Prop::ReadOnly(r), Widget::Entry(e)) => e.set_editable(!r),
            (Prop::Value(t), Widget::Password(e)) => {
                if e.text() != t.as_str() {
                    e.set_text(t);
                }
            }
            (Prop::Placeholder(t), Widget::Password(e)) => e.set_placeholder_text(Some(t)),
            (Prop::Checked(c), Widget::Checkbox(b)) => b.set_active(*c),
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                b.set_inconsistent(*m);
                node.mixed = Some(*m);
            }
            (Prop::Checked(c), Widget::Switch(s)) => s.set_active(*c),
            (Prop::Enabled(e), w) if w.is_control() => w.widget().set_sensitive(*e),
            (Prop::TextStyle(style), w) if w.is_control() => {
                let widget = w.widget();
                for class in TEXT_STYLE_CLASSES {
                    widget.remove_css_class(class);
                }
                if let Some(class) = text_style_class(*style) {
                    widget.add_css_class(class);
                }
                node.text_style = Some(*style);
            }
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                for class in ROLE_CLASSES {
                    b.remove_css_class(class);
                }
                if let Some(class) = role_class(*role) {
                    b.add_css_class(class);
                }
                node.role = Some(*role);
            }
            (Prop::ButtonStyle(style), Widget::Button(b)) => {
                b.set_has_frame(*style != ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            // On the widget the pointer rests on: a list's view, not the
            // scrolled window around it. GTK reads it to assistive
            // technology as the description.
            (Prop::Tooltip(t), widget) => {
                widget.focus_widget().set_tooltip_text(Some(t.as_str()).filter(|t| !t.is_empty()))
            }
            // On the widget the pointer rests on, as the tooltip is, and
            // for its children without one of their own.
            (Prop::ContextMenu(entries), widget) => {
                let events = self.events.clone();
                let focus = widget.focus_widget();
                node.context_menu
                    .get_or_insert_with(|| {
                        let activate = move |item| events.emit(id, UiEvent::ContextMenuItem(item));
                        ContextMenu::new(&focus, Rc::new(activate))
                    })
                    .set(&focus, entries);
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone()),
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode),
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style),
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows),
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::ScrollAxes(axes), Widget::Scroll { scrolled, .. }) => {
                set_scroll_policy(scrolled, *axes, scroll_bars(scrolled))
            }
            (Prop::ScrollBars(show), Widget::Scroll { scrolled, .. }) => {
                set_scroll_policy(scrolled, scroll_axes(scrolled), *show)
            }
            (Prop::Custom(new), Widget::Custom { widget, render, props }) => {
                if props != new {
                    render.update(widget, props.props(), new.props());
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(drawing), Widget::Drawn { drawn, .. }) => drawn.set_drawing(drawing.clone()),
            (Prop::Native(opaque), Widget::Native { widget, last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    payload.apply(widget);
                }
                *last = opaque.clone();
            }
            _ => {}
        }
    }

    /// Leaves with an empty frame (hidden ones, or not laid out yet) are
    /// kept out of GTK's allocation: controls can't be allocated smaller
    /// than their padding. Containers stay, since content may overflow them.
    fn update_child_visible(&self, id: NodeId) {
        let Some(node) = self.nodes.get(&id) else { return };
        if node.widget.is_leaf() {
            let widget = node.widget.widget();
            let empty = self.frames.borrow().get(widget).is_none_or(|f| f.size.is_empty());
            widget.set_child_visible(!empty);
        }
    }

    fn widget(&self, id: NodeId, command: &Command) -> gtk::Widget {
        match self.nodes.get(&id) {
            Some(node) => node.widget.widget().clone(),
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    fn window_root(&self, id: NodeId, command: &Command) -> (&WindowParts, &WindowRoot) {
        match self.nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => (parts, parts.host.window_root().expect("window hosts have a root")),
            _ => violation(command, "not a window"),
        }
    }

    /// The scroll view a widget is, or is the content of.
    fn scroll_of(&self, widget: &gtk::Widget) -> Option<(gtk::ScrolledWindow, gtk::Viewport)> {
        let id = owning_node(&self.by_widget, Some(widget.clone()))?;
        let node = self.nodes.get(&id)?;
        let id = match &node.widget {
            Widget::Scroll { .. } => id,
            _ => node.parent?,
        };
        match &self.nodes.get(&id)?.widget {
            Widget::Scroll { scrolled, viewport } => Some((scrolled.clone(), viewport.clone())),
            _ => None,
        }
    }

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) {
        let node = &self.nodes[&id];
        if let Some(run) = node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            match &node.widget {
                // The list view, not the scrolled window around it.
                Widget::List(list) => run(list.view.upcast_ref()),
                widget => run(widget.widget()),
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
                let child_widget = self.widget(*child, command);
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if self.nodes[child].kind == WidgetKind::ToolbarItem {
                    // Items come after the window's content.
                    let content = self
                        .nodes
                        .values()
                        .filter(|n| n.parent == Some(*parent) && n.kind != WidgetKind::ToolbarItem)
                        .count();
                    let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget) else {
                        violation(command, "toolbar items go in windows")
                    };
                    let Some(index) = index.checked_sub(content) else {
                        violation(command, "toolbar items go after the window's content")
                    };
                    // Hidden until it has a size. The header bar stretches
                    // its children to its height; centred, the host keeps
                    // its own, as a label's text is centred in the bar.
                    child_widget.set_visible(false);
                    child_widget.set_valign(gtk::Align::Center);
                    parts.items.insert(index.min(parts.items.len()), (*child, child_widget));
                    pack_items(parts);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                match &self.nodes.get(parent).map(|n| &n.widget) {
                    Some(Widget::Scroll { viewport, .. }) => {
                        if viewport.child().is_some() {
                            violation(command, "a ScrollView has a single native child (its content)");
                        }
                        viewport.set_child(Some(&child_widget));
                    }
                    Some(Widget::List(list)) => {
                        let Some(row) = self.nodes[child].row else {
                            violation(command, "a List's children are row hosts (Containers with a Prop::Row)")
                        };
                        list.insert(row, *child, child_widget);
                    }
                    _ => {
                        let parent_widget = self.widget(*parent, command);
                        let mut before = parent_widget.first_child();
                        for _ in 0..*index {
                            before = before.and_then(|w| w.next_sibling());
                        }
                        child_widget.insert_before(&parent_widget, before.as_ref());
                    }
                }
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                self.update_child_visible(*child);
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                if let Widget::Window(parts) = &mut self.nodes.get_mut(parent).unwrap().widget
                    && let Some(at) = parts.items.iter().position(|(id, _)| id == child)
                {
                    let (_, widget) = parts.items.remove(at);
                    parts.header.remove(&widget);
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return;
                }
                match &self.nodes[parent].widget {
                    Widget::Scroll { viewport, .. } => viewport.set_child(None::<&gtk::Widget>),
                    Widget::List(list) => list.remove(self.nodes[child].row.expect("inserted with a row")),
                    _ => self.widget(*child, command).unparent(),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                if let Some(menu) = &node.context_menu {
                    menu.close();
                }
                let widget = node.widget.widget().clone();
                self.by_widget.borrow_mut().remove(&widget);
                self.frames.borrow_mut().remove(&widget);
                self.pending_show.retain(|w| w != id);
                if let Some(settings) = gtk::Settings::default() {
                    for handler in node.settings_handlers {
                        settings.disconnect(handler);
                    }
                }
                if let Some(Widget::Window(parts)) =
                    node.parent.and_then(|p| self.nodes.get_mut(&p)).map(|n| &mut n.widget)
                    && let Some(at) = parts.items.iter().position(|(item, _)| item == id)
                {
                    parts.items.remove(at);
                    parts.header.remove(&widget);
                }
                // The app's handle may keep its surface: it just stops showing.
                if let Widget::GpuSurface(surface) = &node.widget {
                    surface.detach();
                }
                match &node.widget {
                    Widget::Window(parts) => {
                        self.menus.forget(*id);
                        parts.window.destroy();
                    }
                    _ => {
                        if let Some(viewport) = widget.parent().and_then(|p| p.downcast::<gtk::Viewport>().ok()) {
                            viewport.set_child(None::<&gtk::Widget>);
                        } else if widget.parent().is_some() {
                            widget.unparent();
                        }
                    }
                }
            }
            Command::SetFrame { id, frame } => {
                let widget = self.widget(*id, command);
                self.frames.borrow_mut().insert(widget.clone(), *frame);
                // A toolbar item: the header bar places it, at this size.
                if self.nodes[id].kind == WidgetKind::ToolbarItem {
                    widget.set_visible(!frame.size.is_empty());
                    widget.queue_resize();
                    if let Some(window) = self.nodes[id].parent
                        && let Some(Widget::Window(parts)) = self.nodes.get_mut(&window).map(|n| &mut n.widget)
                    {
                        keep_content_size(parts);
                    }
                    return;
                }
                self.update_child_visible(*id);
                if let Widget::Slider { scale, steps } = &self.nodes[id].widget {
                    let vertical = scale.orientation() == gtk::Orientation::Vertical;
                    set_travel(scale, steps, if vertical { frame.height() } else { frame.width() });
                }
                // Hosts ask for their frame size, so their parents must
                // measure again, not just reallocate.
                widget.queue_resize();
                if let Some(Widget::List(list)) =
                    self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget)
                {
                    list.row_measured(frame.height());
                }
                if let Some((scrolled, viewport)) = self.scroll_of(&widget) {
                    sync_scroll(&self.frames, &scrolled, &viewport);
                }
            }
            Command::SetA11y { id, a11y } => {
                let widget = self.widget(*id, command);
                let A11yProps { label, description, hidden, .. } = a11y;
                use gtk::accessible::{Property, State as A11yState};
                match label {
                    Some(label) => widget.update_property(&[Property::Label(label)]),
                    None => widget.reset_property(gtk::AccessibleProperty::Label),
                }
                match description {
                    Some(description) => widget.update_property(&[Property::Description(description)]),
                    None => widget.reset_property(gtk::AccessibleProperty::Description),
                }
                widget.update_state(&[A11yState::Hidden(*hidden)]);
            }
            // A window in full screen keeps the screen's size (its default
            // size would apply when it leaves); none goes below its minimum.
            Command::SetWindowSize { id, size } => {
                let (parts, root) = self.window_root(*id, command);
                if parts.full_screen.in_effect(&parts.window) {
                    return;
                }
                let (min_width, min_height) = parts.host.size_request();
                let asked = *size;
                let size = Size::new(size.width.max(requested(min_width)), size.height.max(requested(min_height)));
                // Before it's first allocated, it's the size the content
                // has, as the core asked; after, the allocation reports it
                // (the app resizing it, `Ui::set_window_size`, hears only
                // from that), and so does one the minimum grows.
                if !parts.window.is_mapped() {
                    root.size.set(asked);
                }
                root.resizing.set((root.size.get() != size).then_some(size));
                resize(&parts.window, size.width as i32, size.height as i32 + parts.header_height);
            }
            Command::SetFocusOrder { window, order } => {
                let widgets: Vec<gtk::Widget> = order
                    .iter()
                    .map(|id| match self.nodes.get(id) {
                        Some(node) => node.widget.focus_widget(),
                        None => violation(command, &format!("node {id} does not exist")),
                    })
                    .collect();
                let (_, root) = self.window_root(*window, command);
                *root.focus_order.borrow_mut() = widgets;
            }
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll { scrolled, .. }) => scroll_to(scrolled, *offset),
                Some(Widget::List(list)) => scroll_to(&list.scrolled, *offset),
                _ => violation(command, "not a ScrollView or List"),
            },
            Command::Focus { id } => match self.nodes.get(id) {
                Some(node) => {
                    node.widget.focus_widget().grab_focus();
                }
                None => violation(command, "node does not exist"),
            },
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row),
                _ => violation(command, "not a List"),
            },
        }
    }
}

/// GTK's scales move by their step from the keyboard. Without a step they
/// need one anyway: a tenth of the range, with pages of ten steps as
/// `gtk::Scale::with_range` makes them.
fn set_increments(scale: &gtk::Scale, step: Option<f64>) {
    let adjustment = scale.adjustment();
    let step = step.unwrap_or((adjustment.upper() - adjustment.lower()) / 10.0);
    scale.set_increments(step, step * 10.0);
}

/// A slider's step, whether a mark shows at each one, and the length its
/// knob travels, which decides whether the marks fit.
#[derive(Default)]
pub(crate) struct Steps {
    step: Cell<Option<f64>>,
    pub(crate) hidden: Cell<bool>,
    length: Cell<f64>,
    /// The marks on the scale: range and step, if any.
    shown: Cell<Option<(f64, f64, f64)>>,
}

/// Where a scale keeps its [`Steps`].
pub(crate) const STEPS: &str = "mitsuami-steps";

/// How far from its mark GTK holds the knob while dragging
/// (`MARK_SNAP_LENGTH` in `gtkrange.c`). With steps closer than twice that,
/// it holds the knob past the next step, which then jumps several at once.
const MARK_HOLD: f64 = 12.0;

/// A mark at every step, as AppKit draws its tick marks, unless the app
/// turned them off or the steps are too close for GTK's drag.
pub(crate) fn update_marks(scale: &gtk::Scale, steps: &Steps) {
    let adjustment = scale.adjustment();
    let (min, max) = (adjustment.lower(), adjustment.upper());
    let wanted = steps
        .step
        .get()
        .filter(|step| *step > 0.0 && !steps.hidden.get())
        .filter(|step| steps.length.get() * step / (max - min) >= 2.0 * MARK_HOLD)
        .map(|step| (min, max, step));
    if steps.shown.replace(wanted) == wanted {
        return;
    }
    scale.clear_marks();
    if let Some((min, max, step)) = wanted {
        for i in 0..=((max - min) / step + 1e-9).floor() as u32 {
            scale.add_mark(min + i as f64 * step, gtk::PositionType::Bottom, None);
        }
    }
}

/// Sets the length a slider's knob travels in a frame of `length` along it:
/// the frame less the scale's padding, which the knob overhangs.
fn set_travel(scale: &gtk::Scale, steps: &Steps, length: f32) {
    #[allow(deprecated)] // The only way to read a widget's CSS padding.
    let padding = scale.style_context().padding();
    let padding = match scale.orientation() {
        gtk::Orientation::Vertical => padding.top() + padding.bottom(),
        _ => padding.left() + padding.right(),
    };
    steps.length.set((length as f64 - padding as f64).max(0.0));
    update_marks(scale, steps);
}

/// The step nearest `value`, counting from the minimum, within the range.
fn snap(adjustment: &gtk::Adjustment, value: f64, step: f64) -> f64 {
    let min = adjustment.lower();
    (min + ((value - min) / step).round() * step).clamp(min, adjustment.upper())
}

/// A select's options, as it shows them.
fn option_texts(options: &gtk::StringList) -> Vec<String> {
    (0..options.n_items()).filter_map(|i| options.string(i)).map(|s| s.to_string()).collect()
}

/// Natural sizes, except text: it wraps to the space it's offered, down to
/// its longest word.
fn measure_widget(widget: &gtk::Widget, wraps: bool, request: MeasureRequest) -> Size {
    let (min_width, natural_width, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
    let width = match (request.known_width, wraps) {
        (Some(known), _) => known.round() as i32,
        (None, false) => natural_width,
        (None, true) => match request.available_width {
            AvailableSpace::Definite(available) => natural_width.min(available.floor() as i32),
            AvailableSpace::MinContent => min_width,
            AvailableSpace::MaxContent => natural_width,
        },
    };
    // GTK can't measure narrower than the minimum (it warns); the text
    // overflows instead.
    let width = width.max(min_width);
    let height = widget.measure(gtk::Orientation::Vertical, width).1;
    Size::new(request.known_width.unwrap_or(width as f32), request.known_height.unwrap_or(height as f32))
}

/// Premultiplied ARGB in native byte order (what `Texture::download` gives)
/// to straight RGBA.
fn to_rgba(argb: &[u8]) -> Vec<u8> {
    argb.chunks_exact(4)
        .flat_map(|p| {
            let [b, g, r, a] = u32::from_ne_bytes([p[0], p[1], p[2], p[3]]).to_le_bytes();
            let straight = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
            [straight(r), straight(g), straight(b), a]
        })
        .collect()
}

/// Renders a widget GTK has laid out, in software: the same pixels
/// whichever GPU renderer the display uses.
fn render(widget: &gtk::Widget, size: (i32, i32)) -> Result<Image, CaptureError> {
    let scale = widget.scale_factor();
    let (width, height) = (size.0 * scale, size.1 * scale);
    let snapshot = gtk::Snapshot::new();
    snapshot.scale(scale as f32, scale as f32);
    gtk::WidgetPaintable::new(Some(widget)).snapshot(&snapshot, size.0 as f64, size.1 as f64);
    let node = snapshot.to_node().ok_or_else(|| CaptureError::Failed("nothing was drawn".into()))?;
    let renderer = gsk::CairoRenderer::new();
    renderer.realize(None::<&gdk::Surface>).map_err(|e| CaptureError::Failed(e.to_string()))?;
    let texture = renderer.render_texture(node, Some(&graphene::Rect::new(0.0, 0.0, width as f32, height as f32)));
    renderer.unrealize();
    let (width, height) = (texture.width() as usize, texture.height() as usize);
    let mut bytes = vec![0; width * height * 4];
    texture.download(&mut bytes, width * 4);
    Ok(Image { width: width as u32, height: height as u32, scale_factor: scale as f32, rgba: to_rgba(&bytes) })
}

impl Backend for GtkBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow().events.set_sink(events);
    }

    fn metrics(&self) -> PlatformMetrics {
        metrics()
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
        if state.nodes.values().any(|n| matches!(n.widget, Widget::List(_))) {
            state.lists_dirty.set(true);
        }
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        match &node.widget {
            Widget::Custom { widget, render, props } => render
                .measure(widget, props.props(), &request)
                .unwrap_or_else(|| measure_widget(widget, false, request)),
            Widget::Native { widget, measure: Some(measure), .. } => measure(widget, &request),
            // Marks make a scale thicker, and whether they fit depends on
            // its length: decide for the length being measured.
            Widget::Slider { scale, steps } => {
                let length = match scale.orientation() {
                    gtk::Orientation::Vertical => request.known_height,
                    _ => request.known_width,
                };
                if let Some(length) = length {
                    set_travel(scale, steps, length);
                }
                measure_widget(scale.upcast_ref(), false, request)
            }
            // A texture measures at its pixel count; pixels made at a scale
            // take that many fewer points. Files measure as GTK reads them.
            Widget::Picture { source: Some(ImageSource::Pixels(pixels)), .. } => {
                let size = pixels.size();
                Size::new(request.known_width.unwrap_or(size.width), request.known_height.unwrap_or(size.height))
            }
            // As large as the layout makes it.
            Widget::GpuSurface(_) => Size::new(request.known_width.unwrap_or(0.0), request.known_height.unwrap_or(0.0)),
            // Measured by the core.
            Widget::Drawn { .. } | Widget::Window(_) | Widget::Host(_) | Widget::Scroll { .. } | Widget::List(_) => {
                Size::ZERO
            }
            widget => measure_widget(widget.widget(), matches!(widget, Widget::Label(_)), request),
        }
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        // Any node's, list rows' included. A disabled widget shows none.
        if let A11yAction::ContextMenuItem(item) = action {
            let actions = {
                let state = self.state.borrow();
                let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
                if !node.widget.focus_widget().is_sensitive() {
                    return Err(ActionError::Disabled);
                }
                node.context_menu.as_ref().map(ContextMenu::chooser).ok_or(ActionError::Unsupported)?
            };
            return choose_context_item(&actions, *item);
        }
        // A list's rows: what GTK's own row actions do. Handlers only
        // touch the list's data and emit.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                state.lists_dirty.set(true);
                match action {
                    A11yAction::Select if list.mode() != SelectionMode::None => list.select(row),
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
            if let (A11yAction::Focus, Widget::List(list)) = (action, &node.widget) {
                return if list.view.grab_focus() { Ok(()) } else { Err(ActionError::Unsupported) };
            }
        }
        let (widget, kind, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.widget.is_control() && !node.widget.widget().is_sensitive() {
                return Err(ActionError::Disabled);
            }
            let custom = match &node.widget {
                Widget::Custom { render, props, .. } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            (node.widget.widget().clone(), node.kind, state.events.clone(), custom)
        };
        if let Some((render, props)) = custom {
            return render.perform(&widget, &props, action, &Emitter::new(events, id));
        }
        // No state borrow below: GTK calls back into our signal handlers.
        match (action, kind) {
            // What GTK's accessibility actions do. `gtk_widget_activate`
            // would click a button only after its press animation.
            (A11yAction::Activate, WidgetKind::Button) => {
                widget.downcast_ref::<gtk::Button>().ok_or(ActionError::Unsupported)?.emit_clicked()
            }
            (A11yAction::Activate, WidgetKind::Checkbox) => {
                let check = widget.downcast_ref::<gtk::CheckButton>().ok_or(ActionError::Unsupported)?;
                check.set_active(!check.is_active());
            }
            (A11yAction::Activate, WidgetKind::Switch) => {
                let switch = widget.downcast_ref::<gtk::Switch>().ok_or(ActionError::Unsupported)?;
                switch.set_active(!switch.is_active());
            }
            // What GTK's accessible increment and decrement do: a step.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Slider) => {
                let adjustment = widget.downcast_ref::<gtk::Range>().ok_or(ActionError::Unsupported)?.adjustment();
                let step = adjustment.step_increment();
                adjustment.set_value(adjustment.value() + if *action == A11yAction::Increment { step } else { -step });
            }
            // What GTK's accessible increment and decrement do on a spin
            // button: a step, stopping at the ends.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::NumberInput) => {
                let spin = widget.downcast_ref::<gtk::SpinButton>().ok_or(ActionError::Unsupported)?;
                let up = *action == A11yAction::Increment;
                spin.spin(if up { gtk::SpinType::StepForward } else { gtk::SpinType::StepBackward }, 0.0);
            }
            // As if typed and committed: whole numbers, clamped to the range.
            (A11yAction::SetValue(text), WidgetKind::NumberInput) => {
                let spin = widget.downcast_ref::<gtk::SpinButton>().ok_or(ActionError::Unsupported)?;
                let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                spin.set_value(value.round());
            }
            // As if dragged there: `change-value`, which snaps to the step.
            (A11yAction::SetValue(text), WidgetKind::Slider) => {
                let range = widget.downcast_ref::<gtk::Range>().ok_or(ActionError::Unsupported)?;
                let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                range.emit_by_name::<bool>("change-value", &[&gtk::ScrollType::Jump, &value]);
            }
            // What choosing from the pop-up does.
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let dropdown = widget.downcast_ref::<gtk::DropDown>().ok_or(ActionError::Unsupported)?;
                let options = dropdown.model().and_downcast::<gtk::StringList>().ok_or(ActionError::Unsupported)?;
                let index = option_texts(&options).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                dropdown.set_selected(index as u32);
            }
            (A11yAction::SetValue(text), WidgetKind::TextInput | WidgetKind::PasswordInput) => {
                let entry = widget.dynamic_cast_ref::<gtk::Editable>().ok_or(ActionError::Unsupported)?;
                if !entry.is_editable() {
                    return Err(ActionError::ReadOnly);
                }
                // One edit, one event (`set_text` may report the deletion
                // and the insertion separately).
                events.muted(|| entry.set_text(text));
                // The caret ends up after the new text, as if it was typed.
                entry.set_position(-1);
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            // Native views: what GTK's accessibility actions do for the
            // widget (activate it, step a range or spin button).
            (A11yAction::Activate, WidgetKind::Native) => {
                if !widget.activate() {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Native) => {
                let up = *action == A11yAction::Increment;
                if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
                    spin.spin(if up { gtk::SpinType::StepForward } else { gtk::SpinType::StepBackward }, 0.0);
                } else if let Some(range) = widget.downcast_ref::<gtk::Range>() {
                    let adjustment = range.adjustment();
                    let step = adjustment.step_increment();
                    adjustment.set_value(adjustment.value() + if up { step } else { -step });
                } else {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::Focus, _) => {
                // The window's focus observer reports the change.
                if !widget.grab_focus() {
                    return Err(ActionError::Unsupported);
                }
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }

    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        // Its controllers report what it takes.
        if let Some(Widget::GpuSurface(surface)) = self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
            return surface.synthesize(input);
        }
        if let SyntheticInput::Click(point) = input {
            // Drawn widgets only: their pointer handling is ours.
            let state = self.state.borrow();
            return match state.nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Drawn { drawn, .. }) => {
                    drawn.click(*point);
                    Ok(())
                }
                Some(_) => Err(ActionError::Unsupported),
                None => Err(ActionError::UnknownNode),
            };
        }
        if let SyntheticInput::Scroll { dx, dy } = input {
            let scrolled = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Scroll { scrolled, .. }) => scrolled.clone(),
                Some(Widget::List(list)) => {
                    self.state.borrow().lists_dirty.set(true);
                    list.scrolled.clone()
                }
                Some(_) => return Err(ActionError::Unsupported),
                None => return Err(ActionError::UnknownNode),
            };
            let (h_on, v_on) = {
                let (h, v) = scrolled.policy();
                (h != gtk::PolicyType::Never, v != gtk::PolicyType::Never)
            };
            let step = |adjustment: gtk::Adjustment, by: f32, on: bool| {
                if on {
                    let max = (adjustment.upper() - adjustment.page_size()).max(0.0);
                    adjustment.set_value((adjustment.value() + by as f64).clamp(0.0, max));
                }
            };
            step(scrolled.hadjustment(), *dx, h_on);
            step(scrolled.vadjustment(), *dy, v_on);
            return Ok(());
        }
        let SyntheticInput::Key(key) = input else { unreachable!() };
        if *key == Key::Escape {
            return self.escape(id);
        }
        // Lists: GTK 4 can't inject key events, and its list keyboard
        // handling has no signals to emit, so do what it does: arrows,
        // Home and End move the selection and show it; Enter activates.
        {
            let state = self.state.borrow();
            if let Some(Widget::List(list)) = state.nodes.get(&id).map(|n| &n.widget) {
                state.lists_dirty.set(true);
                if list.mode() == SelectionMode::None {
                    return Err(ActionError::Unsupported);
                }
                list.view.grab_focus();
                let rows = list.rows();
                let selected = list.selected();
                let current = selected.first().and_then(|k| rows.iter().position(|r| r == k));
                if *key == Key::Enter {
                    if let Some(row) = selected.first() {
                        list.activate(*row);
                    }
                    return Ok(());
                }
                let last = rows.len().checked_sub(1);
                let next = match (key, current) {
                    (Key::Home, _) | (Key::Down, None) => rows.first().map(|_| 0),
                    (Key::End, _) | (Key::Up, None) => last,
                    (Key::Up, Some(i)) => Some(i.saturating_sub(1)),
                    (Key::Down, Some(i)) => Some((i + 1).min(last.unwrap_or(0))),
                    _ => return Err(ActionError::Unsupported),
                };
                if let Some(row) = next.map(|i| rows[i]) {
                    list.select(row);
                }
                return Ok(());
            }
        }
        let (widget, kind, map) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.widget.is_control() && !node.widget.widget().is_sensitive() {
                return Err(ActionError::Disabled);
            }
            (node.widget.widget().clone(), node.kind, state.by_widget.clone())
        };
        match (kind, key) {
            (
                WidgetKind::TextInput | WidgetKind::PasswordInput,
                Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab,
            ) => {
                // GTK 4 can't inject key events. Emit the keybinding signals
                // the keys map to instead, on the widgets that handle them:
                // the entry's inner text widget, and the window for Tab.
                let entry = widget.dynamic_cast_ref::<gtk::Editable>().ok_or(ActionError::Unsupported)?;
                // It would take the keys and ignore them; nothing can be
                // typed into it on any platform.
                if !entry.is_editable() {
                    return Err(ActionError::ReadOnly);
                }
                let focus = widget.root().and_then(|r| r.focus());
                if owning_node(&map, focus) != Some(id) {
                    entry.grab_focus();
                    // Focusing selects everything; typing should append, as
                    // after clicking past the end of the text.
                    entry.set_position(-1);
                }
                let text = entry.delegate().ok_or(ActionError::Unsupported)?;
                match key {
                    Key::Char(c) => text.emit_by_name::<()>("insert-at-cursor", &[&c.to_string()]),
                    Key::Backspace => text.emit_by_name::<()>("backspace", &[]),
                    Key::Enter => text.emit_by_name::<()>("activate", &[]),
                    _ => {
                        let window = widget.root().ok_or(ActionError::Unsupported)?;
                        window.emit_by_name::<()>("move-focus", &[&gtk::DirectionType::TabForward]);
                    }
                }
                Ok(())
            }
            (WidgetKind::Button, Key::Enter | Key::Char(' '))
            | (WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => self.perform(id, &A11yAction::Activate),
            _ => Err(ActionError::Unsupported),
        }
    }

    fn native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        let mut props = Vec::new();
        let text = |s: Option<glib::GString>| s.map(|s| s.to_string()).unwrap_or_default();
        match &node.widget {
            Widget::Window(parts) => {
                props.push(Prop::Title(text(parts.window.title())));
                props.push(Prop::FullScreen(parts.full_screen.shown(&parts.window)));
                props.push(Prop::MinSize(parts.min_size.shown(&parts.window, &parts.host)));
                props.push(Prop::HeightFollowsContent(!parts.window.is_resizable()));
                props.extend(node.modal.map(|(owner, modality)| Prop::Modal { owner, modality }));
            }
            Widget::Label(l) => {
                props.push(Prop::Text(l.text().to_string()));
                let limited = l.ellipsize() != pango::EllipsizeMode::None && l.lines() > 0;
                props.push(Prop::MaxLines(limited.then(|| l.lines() as u32)));
                props.extend(label_color(l, node.text_color).map(Prop::TextColor));
                let int =
                    |type_| find_attr(l, type_).and_then(|a| a.downcast_ref::<pango::AttrInt>().map(|a| a.value()));
                props.extend(int(pango::AttrType::Weight).map(|w| Prop::FontWeight(font_weight(w))));
                // `PANGO_STYLE_NORMAL` is 0; oblique shows as italics too.
                props.extend(int(pango::AttrType::Style).map(|s| Prop::Italic(s != 0)));
                // Where the text shows: GTK mirrors `xalign` in right-to-left
                // widgets (labels the app hasn't aligned, in such a locale).
                let x = if l.direction() == gtk::TextDirection::Rtl { 1.0 - l.xalign() } else { l.xalign() };
                props.push(Prop::TextAlign(match x {
                    x if x < 0.25 => HorizontalAlign::Left,
                    x if x > 0.75 => HorizontalAlign::Right,
                    _ => HorizontalAlign::Center,
                }));
            }
            Widget::Entry(e) => {
                props.push(Prop::Value(e.text().to_string()));
                if let Some(p) = e.placeholder_text() {
                    props.push(Prop::Placeholder(p.to_string()));
                }
                props.push(Prop::ReadOnly(!e.is_editable()));
            }
            Widget::Password(e) => {
                props.push(Prop::Value(e.text().to_string()));
                if let Some(p) = e.placeholder_text() {
                    props.push(Prop::Placeholder(p.to_string()));
                }
            }
            Widget::Button(b) => props.push(Prop::Label(text(b.label()))),
            Widget::Checkbox(c) => {
                props.push(Prop::Label(text(c.label())));
                props.push(Prop::Checked(c.is_active()));
                if node.mixed.is_some() {
                    props.push(Prop::Mixed(c.is_inconsistent()));
                }
            }
            Widget::Switch(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Checked(s.is_active()));
            }
            Widget::Select { dropdown, options } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Options(option_texts(options)));
                let index = dropdown.selected();
                props.push(Prop::SelectedIndex((index != gtk::INVALID_LIST_POSITION).then_some(index as usize)));
            }
            Widget::Slider { scale, steps } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let adjustment = scale.adjustment();
                props.push(Prop::Range { min: adjustment.lower(), max: adjustment.upper() });
                props.push(Prop::Step(steps.step.get()));
                props.push(Prop::Number(adjustment.value()));
                if node.orientation.is_some() {
                    props.push(Prop::Orientation(match scale.orientation() {
                        gtk::Orientation::Vertical => Orientation::Vertical,
                        _ => Orientation::Horizontal,
                    }));
                }
            }
            Widget::SpinButton(spin) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let adjustment = spin.adjustment();
                props.push(Prop::Range { min: adjustment.lower(), max: adjustment.upper() });
                props.push(Prop::Step(Some(adjustment.step_increment())));
                props.push(Prop::Number(spin.value()));
            }
            Widget::Progress { bar, pulsing } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Progress((!pulsing.get()).then(|| bar.fraction())));
            }
            Widget::Spinner(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Running(s.is_spinning()));
            }
            Widget::Picture { picture, source, fit } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(source.clone().map(Prop::Image));
                // Read back, but only if the app chose one.
                if fit.is_some() {
                    props.push(Prop::ImageFit(match picture.content_fit() {
                        gtk::ContentFit::Fill => ImageFit::Stretch,
                        _ => ImageFit::Contain,
                    }));
                }
            }
            Widget::GpuSurface(surface) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(surface.props());
            }
            Widget::Scroll { scrolled, .. } => {
                props.push(Prop::ScrollAxes(scroll_axes(scrolled)));
                props.push(Prop::ScrollBars(scroll_bars(scrolled)));
            }
            Widget::Custom { widget, render, props: last } => {
                props.push(Prop::Custom(last.with_props(render.read(widget, last.props()))))
            }
            Widget::Drawn { drawn, props: last } => {
                props.push(Prop::Custom(last.clone()));
                props.push(Prop::Drawing(drawn.drawing()));
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
        }
        let widget = node.widget.widget();
        if node.widget.is_control() {
            props.push(Prop::Enabled(widget.is_sensitive()));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        props.push(Prop::Tooltip(text(node.widget.focus_widget().tooltip_text())));
        props.extend(node.context_menu.as_ref().map(|m| Prop::ContextMenu(m.entries(&node.widget.focus_widget()))));
        let frame = match &node.widget {
            Widget::Window(parts) => {
                let size = parts.host.window_root().expect("window hosts have a root").size.get();
                Rect::new(0.0, 0.0, size.width, size.height)
            }
            _ => state.frames.borrow().get(widget).copied().unwrap_or_default(),
        };
        // A row is where the list view put it.
        let frame = match (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget)) {
            (Some(row), Some(Widget::List(list))) => list.row_rect(row, &state.frames).unwrap_or(frame),
            // A toolbar item is where the header bar put it, in the content
            // host's coordinates (above it); a hidden one isn't shown.
            (_, Some(Widget::Window(parts))) if node.kind == WidgetKind::ToolbarItem => {
                match widget.compute_point(&parts.host, &gtk::graphene::Point::new(0.0, 0.0)) {
                    Some(origin) if widget.is_visible() => {
                        Rect::new(origin.x(), origin.y(), frame.width(), frame.height())
                    }
                    _ => Rect::ZERO,
                }
            }
            _ => frame,
        };
        let by_widget = state.by_widget.borrow();
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => (list.children(), Some(scroll_offset(&list.scrolled))),
            Widget::Scroll { scrolled, viewport } => (
                viewport.child().and_then(|c| by_widget.get(&c).copied()).into_iter().collect(),
                Some(scroll_offset(scrolled)),
            ),
            _ => {
                let mut children = Vec::new();
                let mut next = widget.first_child();
                while let Some(child) = next {
                    children.extend(by_widget.get(&child).copied());
                    next = child.next_sibling();
                }
                // Then its toolbar items, in the header bar.
                if let Widget::Window(parts) = &node.widget {
                    children.extend(parts.items.iter().map(|(id, _)| *id));
                }
                (children, None)
            }
        };
        drop(by_widget);
        let focus = widget.root().and_then(|r| r.focus());
        let focused = !matches!(node.widget, Widget::Window(_)) && owning_node(&state.by_widget, focus) == Some(id);
        // A list's focus is on its view, or on one of its rows' item
        // widgets; a control in a row owns its own.
        Some(NativeState { kind: node.kind, props, frame, parent: node.parent, children, focused, scroll_offset })
    }

    fn services(&self) -> Box<dyn mitsuami_core::services::Services> {
        Box::new(GtkServices::new(self.handle()))
    }

    /// Without a `GtkApplication`, GTK takes the Wayland app id and the X11
    /// class from the program name, which `run` also sets before GTK
    /// starts (X11 reads it then). GTK 4 windows show only themed icons,
    /// and GNOME apps install theirs named after their id, so windows show
    /// the icon of that name; the app's image is for the other platforms.
    fn set_app_info(&mut self, info: &AppInfo) {
        if let Some(id) = &info.id {
            glib::set_prgname(Some(id.as_str()));
            gtk::Window::set_default_icon_name(id);
        }
        if let Some(name) = &info.name {
            glib::set_application_name(name);
        }
    }

    /// Only what GTK has drawn can be captured: this replies from the frame
    /// clock, once the widget is shown at its size, after that frame's
    /// layout (so changes made before the call are included).
    fn capture(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        let (widget, size) = {
            let state = self.state.borrow();
            let Some(node) = state.nodes.get(&id) else { return reply(Err(CaptureError::UnknownNode)) };
            let size = match &node.widget {
                Widget::Window(parts) => parts.host.window_root().expect("window hosts have a root").size.get(),
                _ => state.frames.borrow().get(node.widget.widget()).map(|f| f.size).unwrap_or_default(),
            };
            (node.widget.widget().clone(), size)
        };
        let target = (size.width.round() as i32, size.height.round() as i32);
        let reply = Rc::new(Cell::new(Some(reply)));
        // Tick callbacks run before layout; paint comes after it, in the
        // same frame.
        widget.add_tick_callback(move |widget, clock| {
            if !widget.is_mapped() {
                return glib::ControlFlow::Continue;
            }
            let handler: Rc<Cell<Option<glib::SignalHandlerId>>> = Rc::default();
            let (reply, widget, slot) = (reply.clone(), widget.clone(), handler.clone());
            handler.set(Some(clock.connect_after_paint(move |clock| {
                if let Some(reply) = reply.take() {
                    reply(if (widget.width(), widget.height()) == target {
                        render(&widget, target)
                    } else {
                        Err(CaptureError::Failed(format!(
                            "the widget is {}×{}, not {}×{}",
                            widget.width(),
                            widget.height(),
                            target.0,
                            target.1
                        )))
                    });
                }
                if let Some(handler) = slot.take() {
                    clock.disconnect(handler);
                }
            })));
            glib::ControlFlow::Break
        });
    }
}

/// Opens `files` dialogs and alerts on this window, or the active one.
pub(crate) fn dialog_parent(handle: &GtkHandle, parent: Option<NodeId>) -> Option<gtk::Window> {
    let windows = handle.windows();
    parent
        .and_then(|id| windows.iter().find(|(w, _)| *w == id).map(|(_, window)| window.clone()))
        .or_else(|| windows.iter().find(|(_, w)| w.is_active()).map(|(_, w)| w.clone()))
        .or_else(|| windows.first().map(|(_, w)| w.clone()))
}

pub(crate) fn file_filters(filters: &[mitsuami_core::services::FileFilter]) -> Option<gio::ListStore> {
    if filters.is_empty() {
        return None;
    }
    let store = gio::ListStore::new::<gtk::FileFilter>();
    for filter in filters {
        let gtk_filter = gtk::FileFilter::new();
        gtk_filter.set_name(Some(&filter.name));
        for extension in &filter.extensions {
            gtk_filter.add_suffix(extension.trim_start_matches('.'));
        }
        if filter.is_all() {
            gtk_filter.add_pattern("*");
        }
        store.append(&gtk_filter);
    }
    Some(store)
}

fn prop_owner(prop: &Prop) -> Option<NodeId> {
    match prop {
        Prop::Modal { owner, .. } => *owner,
        _ => None,
    }
}

/// The name of a modal window's Escape controller.
const ESCAPE: &str = "mitsuami-escape";

/// What `GtkDialog` does: Escape closes the window, which asks first
/// (`close-request`, which the backend reports and stops). In the bubble
/// phase, so a focused widget that uses Escape itself (an open popover,
/// entry completion) gets it first.
fn escape_closes() -> gtk::ShortcutController {
    let controller = gtk::ShortcutController::new();
    controller.set_name(Some(ESCAPE));
    controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
    let close = gtk::CallbackAction::new(|widget, _| {
        if let Some(window) = widget.downcast_ref::<gtk::Window>() {
            window.close();
        }
        glib::Propagation::Stop
    });
    controller.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string("Escape"), Some(close)));
    controller
}

impl GtkBackend {
    /// Escape on a node. GTK 4 can't inject key events, so after focusing
    /// the node, the window's Escape shortcut runs as a key press would
    /// run it; a plain window has none, and nothing happens, as with a
    /// real Escape.
    fn escape(&mut self, id: NodeId) -> Result<(), ActionError> {
        let widget = {
            let state = self.state.borrow();
            state.nodes.get(&id).ok_or(ActionError::UnknownNode)?.widget.widget().clone()
        };
        if widget.is_focusable() {
            widget.grab_focus();
        }
        let window = widget.root().and_then(|r| r.downcast::<gtk::Window>().ok()).ok_or(ActionError::Unsupported)?;
        let controllers = window.observe_controllers();
        let controller = (0..controllers.n_items())
            .filter_map(|i| controllers.item(i).and_downcast::<gtk::ShortcutController>())
            .find(|c| c.name().as_deref() == Some(ESCAPE));
        if let Some(action) =
            controller.and_then(|c| c.item(0).and_downcast::<gtk::Shortcut>()).and_then(|shortcut| shortcut.action())
        {
            action.activate(gtk::ShortcutActionFlags::empty(), &window, None);
        }
        Ok(())
    }
}
