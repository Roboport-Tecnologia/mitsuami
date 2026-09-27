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
use mitsuami_core::services::Reply;
use mitsuami_core::{
    ButtonRole, ButtonStyle, Command, CustomProps, DisplayList, EventValue, ImageFit, ImageSource, Modality, NodeId,
    Opaque, Orientation, Point, PointerEvent, Prop, Rect, RowKey, ScrollAxes, SelectionMode, Size, TextStyle, UiEvent,
    WidgetKind, find_prop,
};

use crate::custom::{Emitter, ErasedRender, KirigamiCx, NativePayload, flatten};
use crate::events::{Events, node_from_key, node_key};
use crate::ffi::{self, Callback, QmlObject};
use crate::qml;
use crate::services::{KirigamiServices, MenuParts};
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
    /// The app's menu, a Kirigami global drawer.
    pub(crate) drawer: Cell<Option<QmlObject>>,
}

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

    /// Sizes the window so its content area is `size`. Qt sizes windows in
    /// whole pixels.
    fn place(&self, size: Size) {
        let header = self.header();
        self.window.set_real("width", size.width.round() as f64);
        self.window.set_real("height", (size.height as f64 + header).round());
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
        if let Some(requested) = self.requested.get() {
            self.place(requested);
        }
        self.host_resized();
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
}

enum Widget {
    Window {
        root: Rc<WindowRoot>,
    },
    Host(QmlObject),
    Label(QmlObject),
    Button(QmlObject),
    Field(QmlObject),
    Checkbox(QmlObject),
    Switch(QmlObject),
    Select(QmlObject),
    Slider(QmlObject),
    NumberInput(QmlObject),
    Progress(QmlObject),
    Spinner(QmlObject),
    /// An image, what it shows (Qt can't give pixels or the source back as
    /// given), and the pixels it hands QML's provider, if any.
    Image {
        item: QmlObject,
        source: Option<ImageSource>,
        fit: Option<ImageFit>,
        pixels: Option<ffi::ProvidedPixels>,
    },
    Scroll {
        view: QmlObject,
        flickable: QmlObject,
    },
    List(crate::list::List),
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
            Widget::Host(i)
            | Widget::Label(i)
            | Widget::Button(i)
            | Widget::Field(i)
            | Widget::Checkbox(i)
            | Widget::Switch(i)
            | Widget::Select(i)
            | Widget::Slider(i)
            | Widget::NumberInput(i)
            | Widget::Progress(i)
            | Widget::Spinner(i)
            | Widget::Image { item: i, .. }
            | Widget::Scroll { view: i, .. }
            | Widget::Custom { item: i, .. }
            | Widget::Drawn { item: i, .. }
            | Widget::Native { item: i, .. } => *i,
            Widget::List(list) => list.root,
        }
    }

    /// Made from our templates, which show a tooltip (`qml::a11y`). A
    /// window's host isn't: a window takes no tooltip.
    fn has_tooltip(&self) -> bool {
        !matches!(self, Widget::Window { .. } | Widget::Custom { .. } | Widget::Drawn { .. } | Widget::Native { .. })
    }

    /// The item that takes keyboard focus and input: a list's list view.
    fn input_item(&self) -> QmlObject {
        match self {
            Widget::Scroll { flickable, .. } => *flickable,
            Widget::List(list) => list.view,
            widget => widget.item(),
        }
    }

    /// Where children go: the host, or the scrolled content.
    fn content(&self) -> QmlObject {
        match self {
            Widget::Scroll { flickable, .. } => flickable.object("contentItem").expect("flickables have content"),
            widget => widget.item(),
        }
    }

    /// Built-in controls: they take `Enabled`.
    fn is_control(&self) -> bool {
        matches!(
            self,
            Widget::Label(_)
                | Widget::Button(_)
                | Widget::Field(_)
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select(_)
                | Widget::Slider(_)
                | Widget::NumberInput(_)
        )
    }

    /// Controls that can take keyboard focus.
    fn is_focusable(&self) -> bool {
        matches!(
            self,
            Widget::Button(_)
                | Widget::Field(_)
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select(_)
                | Widget::Slider(_)
                | Widget::NumberInput(_)
                | Widget::Custom { .. }
                | Widget::Native { .. }
                | Widget::List(_)
        )
    }

    /// Measured, never laid out inside: controls and escape hatches.
    fn is_leaf(&self) -> bool {
        !matches!(self, Widget::Window { .. } | Widget::Host(_) | Widget::Scroll { .. } | Widget::List(_))
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
    /// Sliders: whether the app gave an `Orientation`.
    orientation: Option<Orientation>,
    /// Checkboxes: whether the app gave `Mixed`, and the `Checked` the box
    /// shows when it isn't mixed.
    mixed: Option<bool>,
    checked: bool,
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
}

pub(crate) struct State {
    options: BackendOptions,
    nodes: HashMap<NodeId, Node>,
    events: Events,
    log: Vec<Command>,
    pending_show: Vec<NodeId>,
    /// The app's menu, installed in every window, current and future.
    pub(crate) menu: Option<MenuParts>,
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
            menu: None,
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

    pub(crate) fn with_menu<R>(&self, f: impl FnOnce(&MenuParts) -> R) -> Option<R> {
        self.state.borrow().menu.as_ref().map(f)
    }

    pub(crate) fn with_menu_mut<R>(&self, f: impl FnOnce(&mut MenuParts) -> R) -> Option<R> {
        self.state.borrow_mut().menu.as_mut().map(f)
    }

    /// Installs (or replaces) the app menu in every window.
    pub(crate) fn set_menu(&self, menu: MenuParts) {
        let windows = self.windows();
        for (_, root) in &windows {
            menu.install(root);
        }
        self.state.borrow_mut().menu = Some(menu);
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
            WidgetKind::Window => self.create_window(id),
            WidgetKind::Container => Widget::Host(QmlObject::load(&qml::container())),
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
            WidgetKind::Text => Widget::Label(QmlObject::load(&qml::label())),
            WidgetKind::Button => {
                let button = QmlObject::load(&qml::button());
                button.connect("clicked()", move || events.emit(id, UiEvent::Click));
                Widget::Button(button)
            }
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
                orientation: None,
                mixed: None,
                checked: false,
                tweak: None,
                tooltip: String::new(),
                modal: None,
                scroll_axes: None,
                a11y_label: None,
            },
        );
    }

    fn create_window(&mut self, id: NodeId) -> Widget {
        let events = self.events.clone();
        let window = QmlObject::load(&qml::window(self.menu.as_ref().and_then(|m| m.drawer_qml())));
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
        });
        for signal in ["widthChanged()", "heightChanged()"] {
            let root = Rc::downgrade(&root);
            host.connect(signal, move || {
                if let Some(root) = root.upgrade() {
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
        if let Some(menu) = &self.menu {
            // Its drawer came with the window, if the app has menus.
            root.drawer.set(window.object("globalDrawer"));
            menu.connect(&root);
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
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window { root }) => {
                root.window.set_str("title", t);
                // The page's title is what Kirigami shows in its toolbar.
                if let Some(page) = root.window.child("mitsuamiPage") {
                    page.set_str("title", t);
                }
            }
            (Prop::Text(t), Widget::Label(l)) => l.set_str("text", t),
            // Qt elides the last line it shows.
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                l.set_int("maximumLineCount", lines.map_or(i32::MAX, |n| n as i32));
                l.set_int("elide", if lines.is_some() { ELIDE_RIGHT } else { ELIDE_NONE });
            }
            (Prop::Label(t), Widget::Button(b) | Widget::Checkbox(b)) => b.set_str("text", t),
            (
                Prop::Label(t),
                Widget::Switch(s)
                | Widget::Select(s)
                | Widget::Slider(s)
                | Widget::NumberInput(s)
                | Widget::Progress(s)
                | Widget::Spinner(s)
                | Widget::Image { item: s, .. },
            ) => {
                s.set_str("mitsuamiA11yName", t);
                node.a11y_label = Some(t.clone());
            }
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
            (Prop::Orientation(o), Widget::Slider(s)) => {
                s.set_int("orientation", if o.vertical() { QT_VERTICAL } else { QT_HORIZONTAL });
                node.orientation = Some(*o);
            }
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
            (Prop::SelectedIndex(index), Widget::Select(s)) => {
                s.set_int("currentIndex", index.map_or(-1, |i| i as i32))
            }
            (Prop::Value(t), Widget::Field(f)) => {
                // Don't disturb the caret when the field already shows it.
                if f.str("text") != *t {
                    f.set_str("text", t);
                }
            }
            (Prop::Placeholder(t), Widget::Field(f)) => f.set_str("placeholderText", t),
            // Still focusable and selectable, so its text can be copied.
            (Prop::ReadOnly(r), Widget::Field(f)) => f.set_bool("readOnly", *r),
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
            (Prop::ButtonStyle(style), Widget::Button(b)) => {
                // Flat buttons have no frame until hovered.
                b.set_bool("flat", *style == ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            (Prop::Tooltip(text), widget) => {
                if widget.has_tooltip() {
                    widget.item().set_str("mitsuamiTooltip", text);
                }
                node.tooltip = text.clone();
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
                // The list view, not the scroll view around it.
                Widget::List(list) => run(list.view),
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
                item.set_parent_item(Some(content), *index);
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                if let Widget::Scroll { flickable, .. } = &self.nodes[parent].widget {
                    sync_scroll(*flickable);
                }
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                match &self.nodes[parent].widget {
                    Widget::List(list) => list.remove(self.nodes[child].row.expect("inserted with a row")),
                    _ => self.widget(*child, command).item().set_parent_item(None, 0),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                self.pending_show.retain(|w| w != id);
                match &node.widget {
                    Widget::Window { root } => {
                        // The menu drawer isn't the window's child: it
                        // would outlive it, bound to a parent that's gone.
                        if let Some(drawer) = root.drawer.take() {
                            drawer.destroy();
                        }
                        root.window.destroy()
                    }
                    widget => widget.item().destroy(),
                }
            }
            Command::SetFrame { id, frame } => {
                let widget = self.widget(*id, command);
                let item = widget.item();
                item.set_geometry(frame.x() as f64, frame.y() as f64, frame.width() as f64, frame.height() as f64);
                // Leaves with an empty frame (hidden, or not laid out yet)
                // aren't shown: controls draw their frames regardless of size.
                if widget.is_leaf() {
                    item.set_bool("visible", !frame.size.is_empty());
                }
                if let Some(Widget::List(list)) =
                    self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget)
                {
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
                        | Widget::Slider(_)
                        | Widget::NumberInput(_)
                        | Widget::Progress(_)
                        | Widget::Spinner(_)
                        | Widget::Image { .. }
                );
                if !named_by_label || label.is_some() {
                    item.set_str("mitsuamiA11yName", label.as_deref().unwrap_or_default());
                }
                item.set_str("mitsuamiA11yDescription", description.as_deref().unwrap_or_default());
                item.set_bool("mitsuamiA11yHidden", *hidden);
            }
            Command::SetWindowSize { id, size } => {
                let root = self.window_root(*id, command);
                root.request(*size);
            }
            Command::SetFocusOrder { window, order } => {
                let items: Vec<QmlObject> = order.iter().map(|id| self.widget(*id, command).item()).collect();
                self.window_root(*window, command).window.set_tab_order(&items);
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
    item.set_real("width", width as f64);
    let height = item.real("implicitHeight").ceil() as f32;
    item.set_real("width", frame_width);
    Size::new(width, request.known_height.unwrap_or(height))
}

impl Backend for KirigamiBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow().events.set_sink(events);
    }

    fn metrics(&self) -> PlatformMetrics {
        theme::metrics()
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
            // Measured by the core.
            Widget::Drawn { .. }
            | Widget::Window { .. }
            | Widget::Host(_)
            | Widget::Scroll { .. }
            | Widget::List(_) => Size::ZERO,
            widget => measure_item(widget.item(), matches!(widget, Widget::Label(_)), request),
        }
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
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
        match input {
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
                    window.key(code, false, text);
                    Ok(())
                }
                (WidgetKind::TextInput | WidgetKind::PasswordInput, _) => {
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
                    window.key(code, false, &text);
                    Ok(())
                }
                (WidgetKind::Button, Key::Enter | Key::Char(' '))
                | (WidgetKind::Checkbox | WidgetKind::Switch, Key::Char(' ')) => {
                    self.perform(id, &A11yAction::Activate)
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
                let lines = l.int("maximumLineCount");
                props.push(Prop::MaxLines((lines != i32::MAX).then_some(lines as u32)));
            }
            Widget::Field(f) => {
                props.push(Prop::Value(f.str("text")));
                props.push(Prop::Placeholder(f.str("placeholderText")));
                props.push(Prop::ReadOnly(f.bool("readOnly")));
            }
            Widget::Button(b) => props.push(Prop::Label(b.str("text"))),
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
            Widget::Image { source, fit, .. } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(source.clone().map(Prop::Image));
                props.extend(fit.map(Prop::ImageFit));
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
        }
        if node.widget.is_control() {
            props.push(Prop::Enabled(item.bool("enabled")));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        props.push(Prop::Tooltip(if node.widget.has_tooltip() {
            node.widget.item().str("mitsuamiTooltip")
        } else {
            node.tooltip.clone()
        }));
        let frame = match &node.widget {
            Widget::Window { root } => {
                let size = root.size.get();
                Rect::new(0.0, 0.0, size.width, size.height)
            }
            _ => frame_of(item),
        };
        // A row is where the list view put it.
        let frame = match node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget) {
            Some(Widget::List(list)) => list.row_rect(item, frame),
            _ => frame,
        };
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => (Vec::new(), Some(list.scroll_offset())),
            Widget::Scroll { flickable, .. } => (node.widget.content().child_items(), Some(scroll_offset(*flickable))),
            Widget::Window { .. } | Widget::Host(_) => (item.child_items(), None),
            _ => (Vec::new(), None),
        };
        // Items that stand for nodes themselves: `node()` walks up the tree.
        let own = node_key(id);
        let children = match &node.widget {
            Widget::List(list) => list.children(),
            _ => children.iter().filter_map(|c| c.node()).filter(|key| *key != own).map(node_from_key).collect(),
        };
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

/// Opens dialogs on this window, or the active one.
pub(crate) fn dialog_parent(handle: &KirigamiHandle, parent: Option<NodeId>) -> Option<Rc<WindowRoot>> {
    let windows = handle.windows();
    parent
        .and_then(|id| windows.iter().find(|(w, _)| *w == id).map(|(_, root)| root.clone()))
        .or_else(|| windows.iter().find(|(_, root)| root.window.bool("active")).map(|(_, root)| root.clone()))
        .or_else(|| windows.first().map(|(_, root)| root.clone()))
}
