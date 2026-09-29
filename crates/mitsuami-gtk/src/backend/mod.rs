//! The [`Backend`] implementation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;
use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{
    Appearance, Backend, CaptureError, EventSink, Image, MeasureRequest, NativeState, PlatformMetrics, SyntheticInput,
};
use mitsuami_core::services::Reply;
use mitsuami_core::{
    AppInfo, ButtonRole, ButtonStyle, Color, Command, CustomProps, ImageFit, ImageSource, Modality, NodeId, Opaque,
    Orientation, RowKey, Size, TextStyle, WidgetKind,
};

use crate::custom::{DrawnArea, ErasedRender};
use crate::file_drop::FileDropTarget;
use crate::host::{Events, Frames, Host};
use crate::radio::RadioGroup;
use crate::services::{ContextMenu, GtkServices, Menus};
use crate::sidebar::{Sidebar, Split};
use crate::surface::SurfaceArea;
use crate::tabs::Tabs;

mod apply;
mod button;
mod capture;
mod create;
mod dialogs;
mod handle;
mod input;
mod measure;
mod metrics;
mod native_state;
mod perform;
mod props;
mod scroll;
mod slider;
mod text;
mod window;

pub(crate) use dialogs::{dialog_parent, file_filters};
pub(crate) use slider::{STEPS, Steps, update_marks};

use button::ButtonFace;
use metrics::metrics;
use window::{FullScreen, Maximized, MinSize};

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
    /// libadwaita's, which a split view's content page can take: it shows
    /// the page's title and back button there.
    header: adw::HeaderBar,
    /// Measured again when toolbar items change it.
    header_height: i32,
    /// The toolbar items in the header bar, in order.
    items: Vec<(NodeId, gtk::Widget)>,
    /// The app's menu, GNOME style: a menu button at the end of the header bar.
    pub(crate) menu_button: gtk::MenuButton,
    pub(crate) shortcuts: gtk::ShortcutController,
    full_screen: FullScreen,
    maximized: Maximized,
    /// The app's `Resizable`, and its `HeightFollowsContent`: GTK 4 can't
    /// hold one side, so either makes the window not resizable.
    resizable: bool,
    height_locked: bool,
    min_size: MinSize,
    /// The window's content beside its sidebar, while it has one.
    split: Option<Split>,
}

enum Widget {
    Window(WindowParts),
    Host(Host),
    Label(gtk::Label),
    Entry(gtk::Entry),
    Password(gtk::PasswordEntry),
    Search(gtk::SearchEntry),
    Button(gtk::Button),
    /// A menu button, and its own menu (apart from its context menu).
    MenuButton {
        button: gtk::MenuButton,
        menu: ContextMenu,
    },
    Checkbox(gtk::CheckButton),
    Switch(gtk::Switch),
    Select {
        dropdown: gtk::DropDown,
        options: gtk::StringList,
    },
    RadioGroup(RadioGroup),
    /// A scale, and its step and marks, also kept on the scale for
    /// [`crate::show_step_marks`].
    Slider {
        scale: gtk::Scale,
        steps: Rc<Steps>,
    },
    SpinButton(gtk::SpinButton),
    /// A text view in a framed scrolled window, and what GTK has no place
    /// for: a text view has no placeholder, and no number of lines.
    TextArea {
        scrolled: gtk::ScrolledWindow,
        view: gtk::TextView,
        placeholder: Option<String>,
        lines: u32,
    },
    /// A progress bar, and whether it pulses: GTK shows work of unknown
    /// length by `pulse()` calls, which a timer makes while it's set.
    Progress {
        bar: gtk::ProgressBar,
        pulsing: Rc<Cell<bool>>,
    },
    Spinner(gtk::Spinner),
    Separator(gtk::Separator),
    /// A picture, and what it was given: GTK can't give pixels back, and
    /// reads its content fit back whether the app chose one or not.
    Picture {
        picture: gtk::Picture,
        source: Option<ImageSource>,
        fit: Option<ImageFit>,
    },
    /// A themed icon, and the size the app gave: GTK holds whole pixels.
    Icon {
        image: gtk::Image,
        size: Option<f32>,
    },
    GpuSurface(SurfaceArea),
    Scroll {
        scrolled: gtk::ScrolledWindow,
        viewport: gtk::Viewport,
    },
    List(crate::list::List),
    Sidebar(Sidebar),
    Tabs(Tabs),
    Group(crate::group::Group),
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
            Widget::Search(w) => w.upcast_ref(),
            Widget::Button(w) => w.upcast_ref(),
            Widget::MenuButton { button, .. } => button.upcast_ref(),
            Widget::Checkbox(w) => w.upcast_ref(),
            Widget::Switch(w) => w.upcast_ref(),
            Widget::Select { dropdown, .. } => dropdown.upcast_ref(),
            Widget::RadioGroup(group) => group.column.upcast_ref(),
            Widget::Slider { scale, .. } => scale.upcast_ref(),
            Widget::SpinButton(w) => w.upcast_ref(),
            Widget::TextArea { scrolled, .. } => scrolled.upcast_ref(),
            Widget::Progress { bar, .. } => bar.upcast_ref(),
            Widget::Spinner(w) => w.upcast_ref(),
            Widget::Separator(w) => w.upcast_ref(),
            Widget::Picture { picture, .. } => picture.upcast_ref(),
            Widget::Icon { image, .. } => image.upcast_ref(),
            Widget::GpuSurface(surface) => surface.area.upcast_ref(),
            Widget::Scroll { scrolled, .. } => scrolled.upcast_ref(),
            Widget::List(list) => list.scrolled.upcast_ref(),
            Widget::Sidebar(sidebar) => sidebar.scrolled.upcast_ref(),
            Widget::Tabs(tabs) => tabs.root(),
            Widget::Group(group) => group.host.upcast_ref(),
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
                | Widget::Search(_)
                | Widget::Button(_)
                | Widget::MenuButton { .. }
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select { .. }
                | Widget::RadioGroup(_)
                | Widget::Slider { .. }
                | Widget::SpinButton(_)
                | Widget::TextArea { .. }
        )
    }

    /// Measured, never laid out inside: controls and escape hatches.
    fn is_leaf(&self) -> bool {
        !matches!(
            self,
            Widget::Window(_)
                | Widget::Host(_)
                | Widget::Scroll { .. }
                | Widget::List(_)
                | Widget::Sidebar(_)
                | Widget::Tabs(_)
                | Widget::Group(_)
        )
    }

    /// The widget that takes keyboard focus: a list's view, not its
    /// scrolled window.
    fn focus_widget(&self) -> gtk::Widget {
        match self {
            Widget::List(list) => list.view.clone().upcast(),
            Widget::Sidebar(sidebar) => sidebar.list.clone().upcast(),
            Widget::TextArea { view, .. } => view.clone().upcast(),
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
    /// Buttons: what they show, which is remade from all three when one
    /// changes (a label, a `ButtonContent`, or an icon alone).
    button: ButtonFace,
    /// The app's raw settings, run after every other prop.
    tweak: Option<Opaque>,
    /// Switches, selects, sliders and progress bars have no caption, only
    /// an accessible label, which GTK doesn't read back.
    a11y_label: Option<String>,
    /// Windows: modal, and the window they belong to.
    modal: Option<(Option<NodeId>, Modality)>,
    /// Signal handlers on objects that outlive the node.
    settings_handlers: Vec<(glib::Object, glib::SignalHandlerId)>,
    /// The context menu, once the app gave one.
    context_menu: Option<ContextMenu>,
    /// Hosts: what they take when files are dropped on them, once the app
    /// said (`None` inside: nothing).
    file_drop: Option<Option<FileDropTarget>>,
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
            // libadwaita takes the scheme from its style manager, and warns
            // about GTK's setting.
            if adw::is_initialized() {
                adw::StyleManager::default().set_color_scheme(match appearance {
                    Appearance::Dark => adw::ColorScheme::ForceDark,
                    Appearance::Light => adw::ColorScheme::ForceLight,
                });
            } else {
                settings.set_gtk_application_prefer_dark_theme(appearance == Appearance::Dark);
            }
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

impl Backend for GtkBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow().events.set_sink(events);
    }

    fn metrics(&self) -> PlatformMetrics {
        metrics()
    }

    /// A tab bar's pages are elsewhere than navigation tabs': each view
    /// measures its own.
    fn tab_insets(&self, id: NodeId) -> Option<mitsuami_core::Insets> {
        match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::Tabs(tabs)) => Some(tabs.insets()),
            _ => None,
        }
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
        self.capture_node(id, reply);
    }
}
