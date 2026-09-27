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
use mitsuami_core::services::Reply;
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    ButtonRole, ButtonStyle, Command, CustomProps, EventValue, NodeId, Opaque, Point, Prop, Rect, RowKey, ScrollAxes,
    SelectionMode, Size, TextStyle, UiEvent, WidgetKind, find_prop,
};

use crate::custom::{DrawnArea, Emitter, ErasedRender, GtkCx, NativePayload};
use crate::host::{Events, Frames, Host, WindowRoot};
use crate::services::{GtkServices, MenuParts};

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
    header_height: i32,
    /// The app's menu, GNOME style: a menu button at the end of the header bar.
    pub(crate) menu_button: gtk::MenuButton,
    pub(crate) shortcuts: gtk::ShortcutController,
}

enum Widget {
    Window(WindowParts),
    Host(Host),
    Label(gtk::Label),
    Entry(gtk::Entry),
    Button(gtk::Button),
    Checkbox(gtk::CheckButton),
    Switch(gtk::Switch),
    Select {
        dropdown: gtk::DropDown,
        options: gtk::StringList,
    },
    /// A scale, and the step it was given (GTK needs one to move by).
    Slider {
        scale: gtk::Scale,
        step: Option<f64>,
    },
    /// A progress bar, and whether it pulses: GTK shows work of unknown
    /// length by `pulse()` calls, which a timer makes while it's set.
    Progress {
        bar: gtk::ProgressBar,
        pulsing: Rc<Cell<bool>>,
    },
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
            Widget::Button(w) => w.upcast_ref(),
            Widget::Checkbox(w) => w.upcast_ref(),
            Widget::Switch(w) => w.upcast_ref(),
            Widget::Select { dropdown, .. } => dropdown.upcast_ref(),
            Widget::Slider { scale, .. } => scale.upcast_ref(),
            Widget::Progress { bar, .. } => bar.upcast_ref(),
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
                | Widget::Button(_)
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select { .. }
                | Widget::Slider { .. }
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
    role: Option<ButtonRole>,
    button_style: Option<ButtonStyle>,
    /// Checkboxes: whether the app gave `Mixed`.
    mixed: Option<bool>,
    /// The app's raw settings, run after every other prop.
    tweak: Option<Opaque>,
    /// Switches, selects, sliders and progress bars have no caption, only
    /// an accessible label, which GTK doesn't read back.
    a11y_label: Option<String>,
    /// Signal handlers on objects that outlive the node.
    settings_handlers: Vec<glib::SignalHandlerId>,
}

pub(crate) struct State {
    options: BackendOptions,
    nodes: HashMap<NodeId, Node>,
    by_widget: WidgetMap,
    frames: Frames,
    events: Events,
    log: Vec<Command>,
    pending_show: Vec<NodeId>,
    /// The app's menu, installed in every window, current and future.
    pub(crate) menu: Option<MenuParts>,
    /// A list changed (its rows, a row's size, its scroll position): the
    /// list view must lay out again before its rows are known.
    lists_dirty: Cell<bool>,
}

impl Drop for State {
    /// GTK keeps toplevels alive until they're destroyed; a backend's
    /// windows go with it.
    fn drop(&mut self) {
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
        GtkBackend {
            state: Rc::new(RefCell::new(State {
                options,
                nodes: HashMap::new(),
                by_widget: WidgetMap::default(),
                frames: Frames::default(),
                events: Events::default(),
                log: Vec::new(),
                pending_show: Vec::new(),
                menu: None,
                lists_dirty: Cell::new(false),
            })),
        }
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

    /// Resizes a window's content like the user would, and waits until GTK
    /// has allocated it; the content host reports it as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let Some((gtk_window, host, header_height)) = self.window_parts(window) else { return };
        gtk_window.set_default_size(size.width as i32, size.height as i32 + header_height);
        // Some backends (Broadway before GTK 4.16) only size a toplevel when
        // its surface is presented, so a mapped window ignores a new default
        // size until then. `GtkWindow::present` would only focus it.
        if gtk_window.is_mapped()
            && let Some(toplevel) = gtk_window.surface().and_downcast::<gdk::Toplevel>()
        {
            let layout = gdk::ToplevelLayout::new();
            layout.set_resizable(gtk_window.is_resizable());
            toplevel.present(&layout);
        }
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

    /// Dispatches whatever the GTK main context has ready, without waiting.
    pub fn pump(&self) {
        let context = glib::MainContext::default();
        while context.iteration(false) {}
    }

    /// Escape hatch: the native window of a window node.
    pub fn gtk_window(&self, id: NodeId) -> Option<gtk::Window> {
        self.window_parts(id).map(|(window, _, _)| window)
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

    pub(crate) fn weak(&self) -> Weak<RefCell<State>> {
        Rc::downgrade(&self.state)
    }

    pub(crate) fn from_weak(state: &Weak<RefCell<State>>) -> Option<GtkHandle> {
        state.upgrade().map(|state| GtkHandle { state })
    }

    pub(crate) fn with_menu<R>(&self, f: impl FnOnce(&MenuParts) -> R) -> Option<R> {
        self.state.borrow().menu.as_ref().map(f)
    }

    /// Installs (or replaces) the app menu in every window.
    pub(crate) fn set_menu(&self, menu: MenuParts) {
        let mut state = self.state.borrow_mut();
        for node in state.nodes.values() {
            if let Widget::Window(parts) = &node.widget {
                menu.install(parts);
            }
        }
        state.menu = Some(menu);
    }
}

impl mitsuami_core::TestHooks for GtkHandle {
    fn name(&self) -> &'static str {
        "gtk"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        GtkHandle::resize_window(self, window, size);
    }

    fn take_command_log(&self) -> Vec<Command> {
        GtkHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        GtkHandle::node_count(self)
    }

    /// Windows are shown once laid out, and GTK delivers what it queued
    /// (allocations, scroll adjustments, focus).
    fn settle(&self) {
        self.show_pending_windows();
        self.pump();
        self.layout_lists();
    }
}

/// Keeps a scroll view's adjustments in step with the frames the core sent,
/// without waiting for GTK to allocate: `ScrollTo` in the same commit
/// needs the new range.
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

impl State {
    fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) {
        let events = self.events.clone();
        let mut settings_handlers = Vec::new();
        let widget = match kind {
            WidgetKind::Window => Widget::Window(self.create_window(id, &mut settings_handlers)),
            WidgetKind::Container => Widget::Host(Host::new(self.frames.clone(), None)),
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
                Widget::Slider { scale, step: None }
            }
            WidgetKind::Progress => Widget::Progress { bar: gtk::ProgressBar::new(), pulsing: Rc::default() },
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
                role: None,
                button_style: None,
                mixed: None,
                tweak: None,
                a11y_label: None,
                settings_handlers,
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
        if let Some(settings) = gtk::Settings::default() {
            for property in ["gtk-font-name", "gtk-theme-name", "gtk-application-prefer-dark-theme"] {
                let e = events.clone();
                settings_handlers.push(
                    settings.connect_notify_local(Some(property), move |_, _| e.emit(id, UiEvent::MetricsChanged)),
                );
            }
        }
        self.pending_show.push(id);
        let parts = WindowParts { window, host, header_height, menu_button, shortcuts };
        if let Some(menu) = &self.menu {
            menu.install(&parts);
        }
        parts
    }

    fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) {
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window(parts)) => parts.window.set_title(Some(t)),
            (Prop::Text(t), Widget::Label(l)) => l.set_text(t),
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
            (Prop::Range { min, max }, Widget::Slider { scale, step }) => {
                scale.set_range(*min, *max);
                set_increments(scale, *step);
            }
            (Prop::Step(new), Widget::Slider { scale, step }) => {
                *step = *new;
                set_increments(scale, *new);
            }
            (Prop::Number(n), Widget::Slider { scale, .. }) => scale.set_value(*n),
            (Prop::Label(t), Widget::Progress { bar, .. }) => {
                bar.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
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
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone()),
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode),
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style),
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows),
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::ScrollAxes(axes), Widget::Scroll { scrolled, .. }) => {
                let policy = |on: bool| if on { gtk::PolicyType::Automatic } else { gtk::PolicyType::Never };
                scrolled.set_policy(policy(axes.horizontal()), policy(axes.vertical()));
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
            run(node.widget.widget());
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
                match &self.nodes[parent].widget {
                    Widget::Scroll { viewport, .. } => viewport.set_child(None::<&gtk::Widget>),
                    Widget::List(list) => list.remove(self.nodes[child].row.expect("inserted with a row")),
                    _ => self.widget(*child, command).unparent(),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                let widget = node.widget.widget().clone();
                self.by_widget.borrow_mut().remove(&widget);
                self.frames.borrow_mut().remove(&widget);
                self.pending_show.retain(|w| w != id);
                if let Some(settings) = gtk::Settings::default() {
                    for handler in node.settings_handlers {
                        settings.disconnect(handler);
                    }
                }
                match &node.widget {
                    Widget::Window(parts) => parts.window.destroy(),
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
                self.update_child_visible(*id);
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
            Command::SetWindowSize { id, size } => {
                let (parts, root) = self.window_root(*id, command);
                root.size.set(*size);
                parts.window.set_default_size(size.width as i32, size.height as i32 + parts.header_height);
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

/// GTK's scales move by their step from the keyboard and snap to nothing.
/// Without a step they need one anyway: a tenth of the range, with pages
/// of ten steps as `gtk::Scale::with_range` makes them.
fn set_increments(scale: &gtk::Scale, step: Option<f64>) {
    let adjustment = scale.adjustment();
    let step = step.unwrap_or((adjustment.upper() - adjustment.lower()) / 10.0);
    scale.set_increments(step, step * 10.0);
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
            // Measured by the core.
            Widget::Drawn { .. } | Widget::Window(_) | Widget::Host(_) | Widget::Scroll { .. } | Widget::List(_) => {
                Size::ZERO
            }
            widget => measure_widget(widget.widget(), matches!(widget, Widget::Label(_)), request),
        }
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
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
            (A11yAction::SetValue(text), WidgetKind::Slider) => {
                let range = widget.downcast_ref::<gtk::Range>().ok_or(ActionError::Unsupported)?;
                range.set_value(text.trim().parse().map_err(|_| ActionError::Unsupported)?);
            }
            // What choosing from the pop-up does.
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let dropdown = widget.downcast_ref::<gtk::DropDown>().ok_or(ActionError::Unsupported)?;
                let options = dropdown.model().and_downcast::<gtk::StringList>().ok_or(ActionError::Unsupported)?;
                let index = option_texts(&options).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                dropdown.set_selected(index as u32);
            }
            (A11yAction::SetValue(text), WidgetKind::TextInput) => {
                let entry = widget.downcast_ref::<gtk::Entry>().ok_or(ActionError::Unsupported)?;
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
            (WidgetKind::TextInput, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                // GTK 4 can't inject key events. Emit the keybinding signals
                // the keys map to instead, on the widgets that handle them:
                // the entry's inner text widget, and the window for Tab.
                let entry = widget.downcast_ref::<gtk::Entry>().ok_or(ActionError::Unsupported)?;
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
            Widget::Window(parts) => props.push(Prop::Title(text(parts.window.title()))),
            Widget::Label(l) => props.push(Prop::Text(l.text().to_string())),
            Widget::Entry(e) => {
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
            Widget::Slider { scale, step } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let adjustment = scale.adjustment();
                props.push(Prop::Range { min: adjustment.lower(), max: adjustment.upper() });
                props.push(Prop::Step(*step));
                props.push(Prop::Number(adjustment.value()));
            }
            Widget::Progress { bar, pulsing } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Progress((!pulsing.get()).then(|| bar.fraction())));
            }
            Widget::Scroll { scrolled, .. } => {
                let (h, v) = scrolled.policy();
                props.push(Prop::ScrollAxes(match (h != gtk::PolicyType::Never, v != gtk::PolicyType::Never) {
                    (true, true) => ScrollAxes::Both,
                    (true, false) => ScrollAxes::Horizontal,
                    _ => ScrollAxes::Vertical,
                }));
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
        store.append(&gtk_filter);
    }
    Some(store)
}
