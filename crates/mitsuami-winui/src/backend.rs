//! The [`Backend`] implementation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mitsuami_core::a11y::{A11yAction, A11yProps, ActionError};
use mitsuami_core::backend::{
    Appearance, AvailableSpace, Backend, CaptureError, EventSink, FontSizes, Image, Key, MeasureRequest, NativeState,
    PlatformMetrics, SyntheticInput,
};
use mitsuami_core::services::{MenuBarData, MenuEntry, Reply};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    AnyValue, ButtonVariant, Command, CustomProps, EventValue, NodeId, Opaque, Point, Prop, Rect, RowKey, ScrollAxes,
    SelectionMode, Size, TextStyle, UiEvent, WidgetKind, find_prop,
};
use windows_core::{EventRevoker, HSTRING, IInspectable, IUnknown, Interface};

use crate::bindings as w;
use crate::custom::{DrawnView, ErasedRender, Measure, NativePayload, WinUiCx};
use crate::runtime;

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
}

impl Default for BackendOptions {
    fn default() -> Self {
        BackendOptions { show_windows: true, record_commands: false, appearance: None, private_clipboard: false }
    }
}

/// The parts of a window: XAML's `Window`, a root grid with a menu bar row
/// and the content host (a `Canvas`) below it.
pub(crate) struct WindowParts {
    pub(crate) window: w::Window,
    root: w::Grid,
    host: w::Canvas,
    title_bar: w::TitleBar,
    app_window: w::AppWindow,
    pub(crate) hwnd: w::HWND,
    pub(crate) id: w::WindowId,
    menu_bar: Option<w::MenuBar>,
    menu_revokers: Vec<EventRevoker>,
    /// The content size the app asked for, re-applied when the menu bar
    /// changes height.
    requested: Option<Size>,
    /// The window's node and how to report its events.
    node: NodeId,
    emitter: Events,
    /// The content size, as last reported.
    size: Rc<Cell<Option<Size>>>,
    /// The node that has keyboard focus, as last reported.
    focus: Rc<Cell<Option<NodeId>>>,
    /// The Tab order sent by the core.
    tab_order: Rc<RefCell<Vec<NodeId>>>,
}

impl WindowParts {
    /// Reports a content size, once per change: we report sizes we set
    /// right away, and XAML's `SizeChanged` echoes them later.
    fn report_size(&self, size: Size) {
        report_size(&self.emitter, self.node, &self.size, size);
    }
}

fn report_size(emitter: &Events, window: NodeId, last: &Cell<Option<Size>>, size: Size) {
    if last.replace(Some(size)) != Some(size) {
        emitter.emit(window, UiEvent::WindowResized(size));
    }
}

/// Clips the content host to its size. A `Canvas` doesn't clip its
/// children, so content past the window's edge would still be rendered, in
/// captures too.
fn clip_to_size(host: &w::IUIElement, size: w::Size) -> windows_core::Result<()> {
    let clip = w::RectangleGeometry::new()?;
    clip.cast::<w::IRectangleGeometry>()?.SetRect(w::Rect {
        x: 0.0,
        y: 0.0,
        width: size.width,
        height: size.height,
    })?;
    host.SetClip(&clip)
}

/// Reports a focus move, once: moves we make are reported right away (XAML
/// raises `GotFocus` asynchronously), and the later `GotFocus` finds them
/// already reported.
fn report_focus(emitter: &Events, focus: &Cell<Option<NodeId>>, now: Option<NodeId>) {
    let before = focus.get();
    if now == before {
        return;
    }
    if let Some(before) = before {
        emitter.emit(before, UiEvent::FocusOut);
    }
    if let Some(now) = now {
        emitter.emit(now, UiEvent::FocusIn);
    }
    focus.set(now);
}

/// Reports a scroll offset once per change, for the same reason.
fn report_offset(emitter: &Events, id: NodeId, last: &Cell<Point>, scroll: &w::IScrollViewer) {
    let offset =
        Point::new(scroll.HorizontalOffset().unwrap_or(0.0) as f32, scroll.VerticalOffset().unwrap_or(0.0) as f32);
    if last.replace(offset) != offset {
        emitter.emit(id, UiEvent::Scrolled(offset));
    }
}

/// Scrolls without animation and applies it now, so the offset (and the
/// `Scrolled` report) doesn't wait for XAML's next layout pass.
fn scroll_now(emitter: &Events, id: NodeId, last: &Cell<Point>, scroll: &w::IScrollViewer, to: Point) -> R<()> {
    // The content's size may be new too: lay out first so the scrollable
    // extent is current, or XAML clamps the offset to the old one.
    let element: w::IUIElement = scroll.cast()?;
    element.UpdateLayout()?;
    scroll.ChangeViewWithOptionalAnimation(Some(to.x as f64), Some(to.y as f64), None, true)?;
    element.UpdateLayout()?;
    report_offset(emitter, id, last, scroll);
    Ok(())
}

enum Widget {
    Window(Box<WindowParts>),
    Host(w::Canvas),
    Label(w::TextBlock),
    Field(w::TextBox),
    Button(w::Button),
    Checkbox(w::CheckBox),
    Switch(w::ToggleSwitch),
    Select(w::ComboBox),
    /// A slider, and the step it was given (XAML reads back its own
    /// default without one).
    Slider {
        slider: w::Slider,
        step: Option<f64>,
    },
    Progress(w::ProgressBar),
    Scroll(w::ScrollViewer),
    List(crate::list::List),
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
}

struct Node {
    kind: WidgetKind,
    widget: Widget,
    /// What the parent holds and our frames size: the control itself, a
    /// window's host, or the `Border` around a native render or view.
    element: w::UIElement,
    /// Inside that `Border`: the render's or view's own control. Many XAML
    /// controls size themselves (`RatingControl` sets its own `Width`), so
    /// they don't get our frame directly.
    inner: Option<w::UIElement>,
    parent: Option<NodeId>,
    /// Row hosts: which row of their list they show.
    row: Option<RowKey>,
    revokers: Vec<EventRevoker>,
    /// The value the native widget is known to show, set by the core or
    /// reported to it. Change events that match it are programmatic.
    shown_text: Rc<RefCell<String>>,
    shown_checked: Rc<Cell<bool>>,
    /// Selects: the chosen index, or -1.
    shown_index: Rc<Cell<i32>>,
    /// Sliders: the value.
    shown_number: Rc<Cell<f64>>,
    /// ScrollViews: the offset last reported.
    offset: Rc<Cell<Point>>,
    /// Props XAML can't report back faithfully.
    text_style: Option<TextStyle>,
    variant: Option<ButtonVariant>,
    /// Switches, selects, sliders and progress bars: their label, which is
    /// only their accessible name.
    a11y_label: Option<String>,
}

type Callback = Rc<dyn Fn()>;
/// The app's menus and how to report a choice.
type Menus = (MenuBarData, Rc<dyn Fn(u32)>);

/// Native element (by COM identity) → node, shared with focus handlers.
type ElementMap = Rc<RefCell<HashMap<usize, NodeId>>>;

/// Emits events and wakes the run loop so they get handled soon, even from
/// modal loops (live resizing) that our own loop doesn't see.
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
    menu: Option<Menus>,
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

pub(crate) fn boxed(text: &str) -> IInspectable {
    ok(w::PropertyValue::CreateString(text), "boxing a string")
}

fn unboxed(value: R<IInspectable>) -> Option<String> {
    value.ok()?.cast::<w::IPropertyValue>().ok()?.GetString().ok()
}

/// A resource of the app's merged dictionaries (Fluent styles and brushes).
fn resource<T: Interface>(name: &str) -> Option<T> {
    let resources = w::Application::Current().ok()?.cast::<w::IApplication>().ok()?.Resources().ok()?;
    let map = resources.cast::<windows_collections::IMap<IInspectable, IInspectable>>().ok()?;
    map.Lookup(&windows_reference::IReference::from(HSTRING::from(name))).ok()?.cast().ok()
}

fn style(name: &str) -> w::Style {
    resource(name).unwrap_or_else(|| panic!("winui backend: missing XAML style {name}"))
}

const NAN_SIZE: f64 = f64::NAN;

/// The texts of a combo box's items.
fn option_texts(combo: &w::ComboBox) -> Vec<String> {
    let Ok(items) = combo.cast::<w::IItemsControl>().and_then(|c| c.Items()) else { return Vec::new() };
    (0..items.Size().unwrap_or(0))
        .filter_map(|i| unboxed(items.GetAt(i).ok()?.cast::<w::IContentControl>().ok()?.Content()))
        .collect()
}

/// Measures with the frame size we imposed lifted: XAML's `Measure` honours
/// an explicit `Width`/`Height`, which would hide the content's own size.
fn measure_element(element: &w::UIElement, available: w::Size) -> w::Size {
    let fe: w::IFrameworkElement = ok(element.cast(), "cast to FrameworkElement");
    let (width, height) = (fe.Width().unwrap_or(NAN_SIZE), fe.Height().unwrap_or(NAN_SIZE));
    _ = fe.SetWidth(NAN_SIZE);
    _ = fe.SetHeight(NAN_SIZE);
    let ui: w::IUIElement = ok(element.cast(), "cast to UIElement");
    _ = ui.Measure(available);
    let desired = ui.DesiredSize().unwrap_or_default();
    _ = fe.SetWidth(width);
    _ = fe.SetHeight(height);
    desired
}

/// Fluent's type ramp (Segoe UI Variable): Caption 12, Body 14, Subtitle 20,
/// Title 28, Title Large 40.
fn text_style_resource(style: TextStyle) -> &'static str {
    match style {
        TextStyle::LargeTitle => "TitleLargeTextBlockStyle",
        TextStyle::Title => "TitleTextBlockStyle",
        TextStyle::Headline => "SubtitleTextBlockStyle",
        TextStyle::Body | TextStyle::Callout | TextStyle::Monospace => "BodyTextBlockStyle",
        TextStyle::Caption => "CaptionTextBlockStyle",
    }
}

fn font_sizes() -> FontSizes {
    FontSizes {
        large_title: 40.0,
        title: 28.0,
        headline: 20.0,
        body: 14.0,
        callout: 14.0,
        caption: 12.0,
        monospace: 14.0,
    }
}

fn font_size(style: TextStyle) -> f64 {
    font_sizes().get(style) as f64
}

fn font_weight(style: TextStyle) -> u16 {
    match style {
        TextStyle::LargeTitle | TextStyle::Title | TextStyle::Headline => 600,
        _ => 400,
    }
}

const MONOSPACE: &str = "Cascadia Mono, Consolas";

/// A window's content: the title bar (content extends into it, the Windows
/// 11 way), a row for the menu bar, then the content host. The root and the
/// host carry the window background (window captures render the host).
const WINDOW_ROOT: &str = r#"
<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      Background="{ThemeResource SolidBackgroundFillColorBaseBrush}">
  <Grid.RowDefinitions>
    <RowDefinition Height="Auto"/>
    <RowDefinition Height="Auto"/>
    <RowDefinition Height="*"/>
  </Grid.RowDefinitions>
  <TitleBar Grid.Row="0"/>
  <Canvas Grid.Row="2" Background="{ThemeResource SolidBackgroundFillColorBaseBrush}"/>
</Grid>"#;

/// Where the menu bar goes in `WINDOW_ROOT`.
const MENU_ROW: i32 = 1;

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
                menu: None,
            })),
        }
    }

    pub fn handle(&self) -> WinUiHandle {
        WinUiHandle { state: self.state.clone() }
    }
}

impl WinUiHandle {
    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    /// Number of live native nodes.
    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }

    /// Reports focus moves XAML made on its own (a focused control became
    /// disabled or went away) whose `GotFocus` hasn't arrived yet.
    pub(crate) fn sync_focus(&self) {
        let state = self.state.borrow();
        let focused: Vec<NodeId> = state
            .nodes
            .iter()
            .filter(|(_, n)| !matches!(n.widget, Widget::Window(_)))
            .filter(|(_, n)| {
                n.control()
                    .cast::<w::IUIElement>()
                    .and_then(|e| e.FocusState())
                    .is_ok_and(|f| f != w::FocusState::Unfocused)
            })
            .map(|(id, _)| *id)
            .collect();
        for node in state.nodes.values() {
            let Widget::Window(parts) = &node.widget else { continue };
            let now = focused.iter().copied().find(|id| state.window_of(*id).is_some_and(|p| p.node == parts.node));
            report_focus(&state.emitter, &parts.focus, now);
        }
    }

    /// Called after every event the platform reports, so the run loop turns.
    pub(crate) fn set_wake(&self, wake: impl Fn() + 'static) {
        *self.state.borrow().emitter.wake.borrow_mut() = Some(Rc::new(wake));
    }

    /// Resizes a window's content like the user would; the platform reports
    /// it back as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let state = self.state.borrow();
        if let Some(Widget::Window(parts)) = state.nodes.get(&window).map(|n| &n.widget) {
            resize_client(parts, size);
        }
    }

    /// Makes visible the windows whose first layout has been applied.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        let state = self.state.borrow();
        for id in pending {
            if let Some(Widget::Window(parts)) = state.nodes.get(&id).map(|n| &n.widget) {
                set_transparent(parts.hwnd, false);
                unsafe { _ = w::SetForegroundWindow(parts.hwnd) };
            }
        }
    }

    /// Escape hatch: the XAML window of a window node.
    pub fn xaml_window(&self, id: NodeId) -> Option<w::Window> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window(parts) => Some(parts.window.clone()),
            _ => None,
        }
    }

    /// The window ids and XAML roots of live windows, for services.
    pub(crate) fn window_parts<T>(&self, id: Option<NodeId>, f: impl FnOnce(&WindowParts) -> T) -> Option<T> {
        let state = self.state.borrow();
        let parts = match id {
            Some(id) => match &state.nodes.get(&id)?.widget {
                Widget::Window(parts) => Some(&**parts),
                _ => None,
            },
            None => None,
        };
        // No (or an unknown) parent: the focused window, else any window.
        let parts = parts.or_else(|| {
            let windows = state.nodes.values().filter_map(|n| match &n.widget {
                Widget::Window(parts) => Some(&**parts),
                _ => None,
            });
            let mut fallback = None;
            for parts in windows {
                if parts.focus.get().is_some() {
                    return Some(parts);
                }
                fallback.get_or_insert(parts);
            }
            fallback
        })?;
        Some(f(parts))
    }

    pub(crate) fn xaml_root(&self, id: Option<NodeId>) -> Option<w::XamlRoot> {
        self.window_parts(id, |parts| parts.host.cast::<w::IUIElement>().ok()?.XamlRoot().ok()).flatten()
    }

    /// Installs the app's menus in every window (and windows created later).
    pub(crate) fn set_menu(&self, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let mut state = self.state.borrow_mut();
        state.menu = Some((menu.clone(), activate.clone()));
        for node in state.nodes.values_mut() {
            if let Widget::Window(parts) = &mut node.widget {
                install_menu(parts, menu, &activate);
            }
        }
    }
}

impl mitsuami_core::TestHooks for WinUiHandle {
    fn name(&self) -> &'static str {
        "winui"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        WinUiHandle::resize_window(self, window, size);
    }

    fn take_command_log(&self) -> Vec<Command> {
        WinUiHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        WinUiHandle::node_count(self)
    }

    /// XAML reports some changes (text edits, scrolling, focus) after the
    /// call that caused them: dispatch them.
    fn settle(&self) {
        runtime::pump();
        self.state.borrow().layout_lists();
        self.sync_focus();
        // MITSUAMI_SHOW_WINDOWS=1: tests have no run loop to show them.
        self.show_pending_windows();
    }
}

// ---------------------------------------------------------------- windows

/// Layered, click-through and almost fully transparent: the window is alive
/// (XAML lays out, renders and takes focus) but can't be seen or clicked.
/// Alpha 1, not 0: the compositor skips fully transparent windows, and
/// XAML's rendering (captures included) stalls with it.
fn set_transparent(hwnd: w::HWND, transparent: bool) {
    let flags = w::WS_EX_LAYERED | w::WS_EX_TRANSPARENT;
    unsafe {
        let style = w::GetWindowLongW(hwnd, w::GWL_EXSTYLE);
        if transparent {
            w::SetWindowLongW(hwnd, w::GWL_EXSTYLE, style | flags);
            _ = w::SetLayeredWindowAttributes(hwnd, 0, 1, w::LWA_ALPHA as u32);
        } else {
            w::SetWindowLongW(hwnd, w::GWL_EXSTYLE, style & !flags);
        }
    }
}

fn scale_of(parts: &WindowParts) -> f64 {
    parts
        .host
        .cast::<w::IUIElement>()
        .and_then(|e| e.XamlRoot())
        .and_then(|r| r.RasterizationScale())
        .unwrap_or_else(|_| unsafe { w::GetDpiForWindow(parts.hwnd) } as f64 / 96.0)
}

/// Height of what sits above the content: the title bar and the menu bar.
fn chrome_height(parts: &WindowParts) -> f64 {
    let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
    // As laid out, which can differ from the desired size (the title bar's
    // row is 32.67 at 150%, for a desired 32); measured if not laid out yet.
    let height = |element: w::UIElement| {
        let actual = element.cast::<w::IFrameworkElement>().and_then(|e| e.ActualHeight()).unwrap_or(0.0);
        if actual > 0.0 { actual } else { measure_element(&element, infinite).height as f64 }
    };
    let title = height(ok(parts.title_bar.cast(), "title bar element"));
    title + parts.menu_bar.as_ref().map_or(0.0, |m| height(ok(m.cast(), "menu bar element")))
}

/// Sets the content area (the host, below the title and menu bars) to
/// `size` logical units, and reports the size it got right away, as AppKit
/// does; XAML's own report comes after its next layout pass.
///
/// With the content extended into the title bar, `ResizeClient` sizes the
/// area below the caption strip while `ClientSize` (and XAML's root) include
/// it, so aim, look at what we got, and correct once.
fn resize_client(parts: &WindowParts, size: Size) {
    let Ok(app_window) = parts.app_window.cast::<w::IAppWindow2>() else { return };
    let scale = scale_of(parts);
    let chrome = chrome_height(parts);
    // Windows keeps a resize border inside the client area of windows with
    // extended title bars (1 px along the top): measure it off the live root.
    let inset = |client: i32, root: R<f64>| match root {
        Ok(root) if root > 0.0 => (client - (root * scale).round() as i32).clamp(0, 8),
        _ => 0,
    };
    let (inset_w, inset_h) = match (app_window.ClientSize(), parts.root.cast::<w::IFrameworkElement>()) {
        (Ok(client), Ok(root)) => (inset(client.width, root.ActualWidth()), inset(client.height, root.ActualHeight())),
        _ => (0, 0),
    };
    let want = w::SizeInt32 {
        width: (size.width as f64 * scale).round() as i32 + inset_w,
        height: ((size.height as f64 + chrome) * scale).round() as i32 + inset_h,
    };
    let mut ask = want;
    for _ in 0..2 {
        if app_window.ResizeClient(ask).is_err() {
            return;
        }
        let Ok(got) = app_window.ClientSize() else { return };
        if got == want {
            break;
        }
        ask.width -= got.width - want.width;
        ask.height -= got.height - want.height;
    }
    // What the window actually got (it may refuse), in logical units.
    if let Ok(got) = app_window.ClientSize() {
        let width = ((got.width - inset_w) as f64 / scale) as f32;
        let height = ((got.height - inset_h) as f64 / scale - chrome).max(0.0) as f32;
        parts.report_size(Size::new(width, height));
    }
}

fn install_menu(parts: &mut WindowParts, menu: &MenuBarData, activate: &Rc<dyn Fn(u32)>) {
    let children = ok(parts.root.cast::<w::IPanel>().and_then(|p| p.Children()), "root children");
    if let Some(old) = parts.menu_bar.take() {
        let old: w::UIElement = ok(old.cast(), "menu bar element");
        let mut index = 0;
        if children.IndexOf(&old, &mut index).unwrap_or(false) {
            _ = children.RemoveAt(index);
        }
    }
    parts.menu_revokers.clear();
    if !menu.menus.is_empty() {
        let menu_bar = ok(build_menu_bar(menu, activate, &mut parts.menu_revokers), "building the menu bar");
        let element: w::UIElement = ok(menu_bar.cast(), "menu bar element");
        _ = w::Grid::SetRow(&ok(element.cast::<w::FrameworkElement>(), "menu bar element"), MENU_ROW);
        _ = children.Append(&element);
        parts.menu_bar = Some(menu_bar);
    }
    if let Some(size) = parts.requested {
        resize_client(parts, size);
    }
}

fn build_menu_bar(menu: &MenuBarData, activate: &Rc<dyn Fn(u32)>, revokers: &mut Vec<EventRevoker>) -> R<w::MenuBar> {
    let menu_bar = w::MenuBar::new()?;
    let menus = menu_bar.cast::<w::IMenuBar>()?.Items()?;
    for data in &menu.menus {
        let item = w::MenuBarItem::new()?;
        let item_iface: w::IMenuBarItem = item.cast()?;
        item_iface.SetTitle(&data.title)?;
        let entries = item_iface.Items()?;
        for entry in &data.entries {
            match entry {
                MenuEntry::Item { id, title, shortcut, enabled } => {
                    let flyout_item = w::MenuFlyoutItem::new()?;
                    let iface: w::IMenuFlyoutItem = flyout_item.cast()?;
                    iface.SetText(title)?;
                    flyout_item.cast::<w::IControl>()?.SetIsEnabled(*enabled)?;
                    if let Some(shortcut) = shortcut
                        && let Some(key) = virtual_key(shortcut.key)
                    {
                        let accelerator = w::KeyboardAccelerator::new()?;
                        let accel: w::IKeyboardAccelerator = accelerator.cast()?;
                        accel.SetKey(key)?;
                        let mut modifiers = w::VirtualKeyModifiers::None;
                        if shortcut.primary {
                            modifiers |= w::VirtualKeyModifiers::Control;
                        }
                        if shortcut.shift {
                            modifiers |= w::VirtualKeyModifiers::Shift;
                        }
                        if shortcut.alt {
                            modifiers |= w::VirtualKeyModifiers::Menu;
                        }
                        accel.SetModifiers(modifiers)?;
                        flyout_item.cast::<w::IUIElement>()?.KeyboardAccelerators()?.Append(&accelerator)?;
                    }
                    let (id, activate) = (*id, activate.clone());
                    revokers.push(iface.Click(move |_, _| activate(id))?);
                    entries.Append(&flyout_item.cast::<w::MenuFlyoutItemBase>()?)?;
                }
                MenuEntry::Separator => {
                    entries.Append(&w::MenuFlyoutSeparator::new()?.cast::<w::MenuFlyoutItemBase>()?)?;
                }
            }
        }
        menus.Append(&item)?;
    }
    Ok(menu_bar)
}

fn virtual_key(c: char) -> Option<w::VirtualKey> {
    let c = c.to_ascii_uppercase();
    (c.is_ascii_uppercase() || c.is_ascii_digit()).then_some(w::VirtualKey(c as i32))
}

/// Nearest node for a focused element: XAML focuses parts of composite
/// controls (a TextBox's inner editor), so walk up the visual tree.
fn resolve(by_element: &ElementMap, element: Option<IInspectable>) -> Option<NodeId> {
    let mut current: Option<w::DependencyObject> = element?.cast().ok();
    let map = by_element.borrow();
    while let Some(object) = current {
        if let Some(id) = map.get(&key(&object)) {
            return Some(*id);
        }
        current = w::VisualTreeHelper::GetParent(&object).ok();
    }
    None
}

impl State {
    fn emitter(&self) -> Events {
        self.emitter.clone()
    }

    fn create_window(&mut self, id: NodeId) -> R<(Widget, w::UIElement, Vec<EventRevoker>)> {
        let window = w::Window::new()?;
        // Markup, for the theme resources: they follow the element's theme
        // (tests force light) and switch live with the system's.
        let root: w::Grid = w::XamlReader::Load(WINDOW_ROOT)?.cast()?;
        let children = root.cast::<w::IPanel>()?.Children()?;
        let title_bar: w::TitleBar = children.GetAt(0)?.cast()?;
        let host: w::Canvas = children.GetAt(1)?.cast()?;
        let host_element: w::UIElement = host.cast()?;
        if let Some(appearance) = self.options.appearance {
            let theme = match appearance {
                Appearance::Light => w::ElementTheme::Light,
                Appearance::Dark => w::ElementTheme::Dark,
            };
            root.cast::<w::IFrameworkElement>()?.SetRequestedTheme(theme)?;
        }
        let iwindow: w::IWindow = window.cast()?;
        iwindow.SetContent(&root.cast::<w::UIElement>()?)?;
        // Fluent's title bar instead of the Win32 caption, which ignores the
        // app's theme. The system still draws the caption buttons: make them
        // tall enough for the TitleBar control and follow the theme.
        iwindow.SetExtendsContentIntoTitleBar(true)?;
        iwindow.SetTitleBar(&title_bar.cast::<w::UIElement>()?)?;

        let app_window = window.cast::<w::IWindow2>()?.AppWindow()?;
        let caption = app_window.cast::<w::IAppWindow>()?.TitleBar()?;
        caption.cast::<w::IAppWindowTitleBar2>()?.SetPreferredHeightOption(w::TitleBarHeightOption::Standard)?;
        caption.cast::<w::IAppWindowTitleBar3>()?.SetPreferredTheme(match self.options.appearance {
            Some(Appearance::Light) => w::TitleBarTheme::Light,
            Some(Appearance::Dark) => w::TitleBarTheme::Dark,
            None => w::TitleBarTheme::UseDefaultAppMode,
        })?;
        let window_id = app_window.cast::<w::IAppWindow>()?.Id()?;
        let hwnd = window_id.value as usize as w::HWND;
        // Invisible until the first layout is applied (or for good, in
        // tests). XAML only measures elements in a live tree, so the window
        // must be activated before anything is measured.
        set_transparent(hwnd, true);
        if !self.options.show_windows {
            // Not moved offscreen: XAML stops rendering windows it considers
            // hidden, and capture needs rendering.
            app_window.cast::<w::IAppWindow>()?.SetIsShownInSwitchers(false)?;
        }

        let loaded = Rc::new(Cell::new(false));
        let mut revokers = Vec::new();
        revokers.push(host.cast::<w::IFrameworkElement>()?.Loaded({
            let loaded = loaded.clone();
            move |_, _| loaded.set(true)
        })?);
        window.cast::<w::IWindow>()?.Activate()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while !loaded.get() {
            assert!(Instant::now() < deadline, "winui backend: a window's content never loaded");
            runtime::pump_nested();
            if !loaded.get() {
                runtime::wait(Some(Duration::from_millis(5)));
            }
        }

        let emitter = self.emitter();
        revokers.push(app_window.cast::<w::IAppWindow>()?.Closing({
            let emitter = emitter.clone();
            move |_, args| {
                // The app decides; the core sends Destroy if it agrees.
                if let Some(args) = args.as_ref() {
                    _ = args.cast::<w::IAppWindowClosingEventArgs>().and_then(|a| a.SetCancel(true));
                }
                emitter.emit(id, UiEvent::WindowCloseRequested);
            }
        })?);
        let size = Rc::new(Cell::new(None::<Size>));
        revokers.push(host.cast::<w::IFrameworkElement>()?.SizeChanged({
            let (emitter, last) = (emitter.clone(), size.clone());
            move |sender, args| {
                let Some(new) = args.as_ref().and_then(|a| a.cast::<w::ISizeChangedEventArgs>().ok()?.NewSize().ok())
                else {
                    return;
                };
                if let Some(host) = sender.as_ref().and_then(|s| s.cast::<w::IUIElement>().ok()) {
                    _ = clip_to_size(&host, new);
                }
                report_size(&emitter, id, &last, Size::new(new.width, new.height));
            }
        })?);
        let focus = Rc::new(Cell::new(None));
        let root_element: w::IUIElement = root.cast()?;
        revokers.push(root_element.GotFocus({
            let (emitter, by_element, focus) = (emitter.clone(), self.by_element.clone(), focus.clone());
            move |_, args| {
                let source = args.as_ref().and_then(|a| a.cast::<w::IRoutedEventArgs>().ok()?.OriginalSource().ok());
                report_focus(&emitter, &focus, resolve(&by_element, source));
            }
        })?);
        let tab_order = Rc::new(RefCell::new(Vec::new()));
        revokers.push(root_element.PreviewKeyDown({
            let (emitter, by_element, focus, tab_order, root) =
                (emitter.clone(), self.by_element.clone(), focus.clone(), tab_order.clone(), root.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
                if args.Key().ok() != Some(w::VirtualKey::Tab) {
                    return;
                }
                let backwards = unsafe { w::GetKeyState(w::VK_SHIFT) } < 0;
                if let Some(next) = tab(&root, &by_element, focus.get(), &tab_order.borrow(), backwards) {
                    report_focus(&emitter, &focus, Some(next));
                    _ = args.SetHandled(true);
                }
            }
        })?);

        if self.options.show_windows {
            self.pending_show.push(id);
        }
        let mut parts = WindowParts {
            window,
            root,
            host,
            title_bar,
            app_window,
            hwnd,
            id: window_id,
            menu_bar: None,
            menu_revokers: Vec::new(),
            requested: None,
            node: id,
            emitter,
            size,
            focus,
            tab_order,
        };
        if let Some((menu, activate)) = &self.menu {
            install_menu(&mut parts, menu, activate);
        }
        Ok((Widget::Window(Box::new(parts)), host_element, revokers))
    }

    fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) -> R<()> {
        let emitter = self.emitter();
        let shown_text = Rc::new(RefCell::new(String::new()));
        let shown_checked = Rc::new(Cell::new(false));
        let shown_index = Rc::new(Cell::new(-1));
        let shown_number = Rc::new(Cell::new(0.0));
        let offset = Rc::new(Cell::new(Point::ZERO));
        let mut revokers = Vec::new();
        let mut inner = None;
        let (widget, element) = match kind {
            WidgetKind::Window => {
                let (widget, element, window_revokers) = self.create_window(id)?;
                revokers = window_revokers;
                (widget, element)
            }
            WidgetKind::Container => {
                let canvas = w::Canvas::new()?;
                let element = canvas.cast()?;
                (Widget::Host(canvas), element)
            }
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not a WinUI one")
                        };
                        let mut cx = WinUiCx::new(emitter.clone(), id);
                        let control = emitter.muted(|| render.create(custom.props(), &mut cx))?;
                        revokers.extend(cx.into_revokers());
                        let element = wrap(&control)?;
                        inner = Some(control);
                        (Widget::Custom { render, props: custom }, element)
                    }
                    None => {
                        let view = DrawnView::new(emitter.clone(), id, &mut revokers)?;
                        let element = view.canvas.cast()?;
                        (Widget::Drawn { view, props: custom }, element)
                    }
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not a WinUI one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let mut cx = WinUiCx::new(emitter.clone(), id);
                let control = emitter.muted(|| -> R<w::UIElement> {
                    let control = create(&mut cx)?;
                    payload.apply(&control)?;
                    Ok(control)
                })?;
                revokers.extend(cx.into_revokers());
                let element = wrap(&control)?;
                inner = Some(control);
                (Widget::Native { measure: payload.spec.measure.clone(), last: opaque }, element)
            }
            WidgetKind::Text => {
                let label = w::TextBlock::new()?;
                label.cast::<w::ITextBlock>()?.SetTextWrapping(w::TextWrapping::Wrap)?;
                let element = label.cast()?;
                (Widget::Label(label), element)
            }
            WidgetKind::Button => {
                let button = w::Button::new()?;
                let emitter = emitter.clone();
                revokers.push(button.cast::<w::IButtonBase>()?.Click(move |_, _| emitter.emit(id, UiEvent::Click))?);
                let element = button.cast()?;
                (Widget::Button(button), element)
            }
            WidgetKind::Checkbox => {
                let checkbox = w::CheckBox::new()?;
                let toggle: w::IToggleButton = checkbox.cast()?;
                for checked in [true, false] {
                    let (emitter, shown) = (emitter.clone(), shown_checked.clone());
                    let handler = move |sender: windows_core::Ref<IInspectable>,
                                        _: windows_core::Ref<w::RoutedEventArgs>| {
                        let Some(value) =
                            sender.as_ref().and_then(|s| s.cast::<w::IToggleButton>().ok()?.IsChecked().ok())
                        else {
                            return;
                        };
                        if shown.replace(value) != value {
                            emitter.emit(id, UiEvent::Changed(EventValue::Bool(value)));
                        }
                    };
                    revokers.push(if checked { toggle.Checked(handler)? } else { toggle.Unchecked(handler)? });
                }
                let element = checkbox.cast()?;
                (Widget::Checkbox(checkbox), element)
            }
            WidgetKind::Switch => {
                let switch = w::ToggleSwitch::new()?;
                // Just the track: its label is its own node. By default it
                // shows "On"/"Off" beside the track and is at least 154 wide.
                let iface: w::IToggleSwitch = switch.cast()?;
                iface.SetOnContent(None::<&IInspectable>)?;
                iface.SetOffContent(None::<&IInspectable>)?;
                switch.cast::<w::IFrameworkElement>()?.SetMinWidth(0.0)?;
                let (emitter, shown) = (emitter.clone(), shown_checked.clone());
                revokers.push(switch.cast::<w::IToggleSwitch>()?.Toggled(move |sender, _| {
                    let Some(value) = sender.as_ref().and_then(|s| s.cast::<w::IToggleSwitch>().ok()?.IsOn().ok())
                    else {
                        return;
                    };
                    if shown.replace(value) != value {
                        emitter.emit(id, UiEvent::Changed(EventValue::Bool(value)));
                    }
                })?);
                let element = switch.cast()?;
                (Widget::Switch(switch), element)
            }
            WidgetKind::Select => {
                let combo = w::ComboBox::new()?;
                // Setting the index or the items reports it too: only an
                // index the core doesn't know about is the user's choice.
                let (emitter, shown) = (emitter.clone(), shown_index.clone());
                revokers.push(combo.cast::<w::ISelector>()?.SelectionChanged(move |sender, _| {
                    let Some(index) = sender.as_ref().and_then(|s| s.cast::<w::ISelector>().ok()?.SelectedIndex().ok())
                    else {
                        return;
                    };
                    if index >= 0 && shown.replace(index) != index {
                        emitter.emit(id, UiEvent::Changed(EventValue::Index(index as usize)));
                    }
                })?);
                let element = combo.cast()?;
                (Widget::Select(combo), element)
            }
            WidgetKind::Slider => {
                let slider = w::Slider::new()?;
                // Setting the value or the range reports it too: only a
                // value the core doesn't know about is the user's.
                let (emitter, shown) = (emitter.clone(), shown_number.clone());
                revokers.push(slider.cast::<w::IRangeBase>()?.ValueChanged(move |sender, _| {
                    let Some(value) = sender.as_ref().and_then(|s| s.cast::<w::IRangeBase>().ok()?.Value().ok()) else {
                        return;
                    };
                    if shown.replace(value) != value {
                        emitter.emit(id, UiEvent::Changed(EventValue::Number(value)));
                    }
                })?);
                let element = slider.cast()?;
                (Widget::Slider { slider, step: None }, element)
            }
            WidgetKind::Progress => {
                let progress = w::ProgressBar::new()?;
                let range: w::IRangeBase = progress.cast()?;
                range.SetMinimum(0.0)?;
                range.SetMaximum(1.0)?;
                let element = progress.cast()?;
                (Widget::Progress(progress), element)
            }
            WidgetKind::TextInput => {
                let field = w::TextBox::new()?;
                let iface: w::ITextBox = field.cast()?;
                // TextChanged also fires (later) for programmatic sets: only
                // text the core doesn't know about is a user edit.
                revokers.push(iface.TextChanged({
                    let (emitter, shown) = (emitter.clone(), shown_text.clone());
                    move |sender, _| {
                        let Some(text) = sender.as_ref().and_then(|s| s.cast::<w::ITextBox>().ok()?.Text().ok()) else {
                            return;
                        };
                        if *shown.borrow() != text {
                            *shown.borrow_mut() = text.clone();
                            emitter.emit(id, UiEvent::Changed(EventValue::Text(text)));
                        }
                    }
                })?);
                revokers.push(field.cast::<w::IUIElement>()?.KeyDown({
                    let emitter = emitter.clone();
                    move |_, args| {
                        let enter = args
                            .as_ref()
                            .and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()?.Key().ok())
                            .is_some_and(|k| k == w::VirtualKey::Enter);
                        if enter {
                            emitter.emit(id, UiEvent::Submit);
                        }
                    }
                })?);
                let element = field.cast()?;
                (Widget::Field(field), element)
            }
            WidgetKind::ScrollView => {
                let scroll = w::ScrollViewer::new()?;
                let iface: w::IScrollViewer = scroll.cast()?;
                set_scroll_axes(&iface, ScrollAxes::default())?;
                revokers.push(iface.ViewChanged({
                    let (emitter, last) = (emitter.clone(), offset.clone());
                    move |sender, _| {
                        if let Some(scroll) = sender.as_ref().and_then(|s| s.cast::<w::IScrollViewer>().ok()) {
                            report_offset(&emitter, id, &last, &scroll);
                        }
                    }
                })?);
                let element = scroll.cast()?;
                (Widget::Scroll(scroll), element)
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
            WidgetKind::List => {
                let list = crate::list::List::new(id, emitter.clone())?;
                let element = list.view.cast()?;
                (Widget::List(list), element)
            }
        };
        // The core assumes new nodes start with a zero frame and only sends
        // frames that differ.
        if !matches!(widget, Widget::Window(_)) {
            let fe: w::IFrameworkElement = element.cast()?;
            fe.SetWidth(0.0)?;
            fe.SetHeight(0.0)?;
        }
        self.by_element.borrow_mut().insert(key(&element), id);
        if let Widget::Window(parts) = &widget {
            // Focus on the window's own parts resolves to no node.
            self.by_element.borrow_mut().remove(&key(&parts.host));
        }
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                element,
                inner,
                parent: None,
                row: None,
                revokers,
                shown_text,
                shown_checked,
                shown_index,
                shown_number,
                offset,
                text_style: None,
                variant: None,
                a11y_label: None,
            },
        );
        Ok(())
    }

    fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) -> R<()> {
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        // Custom widgets and native views keep what they were last given.
        match (prop, &mut node.widget) {
            (Prop::Custom(new), Widget::Custom { render, props }) => {
                if props != new {
                    let element = node.inner.clone().unwrap_or_else(|| node.element.clone());
                    let (render, events) = (render.clone(), self.emitter.clone());
                    events.muted(|| render.update(&element, props.props(), new.props()))?;
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(drawing), Widget::Drawn { view, .. }) => view.set_drawing(drawing)?,
            (Prop::Step(new), Widget::Slider { step, .. }) => *step = *new,
            (Prop::Native(opaque), Widget::Native { last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    let control = node.inner.clone().unwrap_or_else(|| node.element.clone());
                    self.emitter.muted(|| payload.apply(&control))?;
                }
                *last = opaque.clone();
            }
            _ => {}
        }
        match (prop, &node.widget) {
            (Prop::Title(t), Widget::Window(parts)) => {
                // The window's own title still names it in the taskbar and Alt+Tab.
                parts.window.cast::<w::IWindow>()?.SetTitle(t)?;
                parts.title_bar.cast::<w::ITitleBar>()?.SetTitle(t)?;
            }
            (Prop::Text(t), Widget::Label(l)) => l.cast::<w::ITextBlock>()?.SetText(t)?,
            (Prop::Label(t), Widget::Button(_) | Widget::Checkbox(_)) => {
                node.element.cast::<w::IContentControl>()?.SetContent(&boxed(t))?
            }
            (Prop::Label(t), Widget::Switch(_) | Widget::Select(_) | Widget::Slider { .. } | Widget::Progress(_)) => {
                w::AutomationProperties::SetName(&node.element, t)?;
                node.a11y_label = Some(t.clone());
            }
            (Prop::Options(options), Widget::Select(combo)) => {
                // Items are `ComboBoxItem`s, so options with the same text
                // stay apart. Replacing them moves the selection; the chosen
                // index stays if it can, else the first option is chosen,
                // as the core does. It sends the index when that changes it.
                let selector: w::ISelector = combo.cast()?;
                let chosen = selector.SelectedIndex()?;
                let count = options.len() as i32;
                let index = if (0..count).contains(&chosen) {
                    chosen
                } else if count > 0 {
                    0
                } else {
                    -1
                };
                node.shown_index.set(index);
                let items = combo.cast::<w::IItemsControl>()?.Items()?;
                items.Clear()?;
                for option in options {
                    let item = w::ComboBoxItem::new()?;
                    item.cast::<w::IContentControl>()?.SetContent(&boxed(option))?;
                    items.Append(&item.cast::<IInspectable>()?)?;
                }
                selector.SetSelectedIndex(index)?;
            }
            (Prop::Range { min, max }, Widget::Slider { slider, .. }) => {
                let range: w::IRangeBase = slider.cast()?;
                // Widen first, so the value isn't clamped on the way.
                if *min < range.Maximum()? {
                    range.SetMinimum(*min)?;
                    range.SetMaximum(*max)?;
                } else {
                    range.SetMaximum(*max)?;
                    range.SetMinimum(*min)?;
                }
                // A value the new range clamped isn't the user's; the core
                // sends it next.
                node.shown_number.set(range.Value()?);
            }
            (Prop::Step(new), Widget::Slider { slider, .. }) => {
                // XAML snaps to `StepFrequency` and steps by `SmallChange`;
                // both are 1 unless set.
                let value = new.unwrap_or(1.0);
                slider.cast::<w::ISlider>()?.SetStepFrequency(value)?;
                slider.cast::<w::IRangeBase>()?.SetSmallChange(value)?;
            }
            (Prop::Number(n), Widget::Slider { slider, .. }) => {
                node.shown_number.set(*n);
                slider.cast::<w::IRangeBase>()?.SetValue(*n)?;
                // XAML snaps what it's given to its step.
                node.shown_number.set(slider.cast::<w::IRangeBase>()?.Value()?);
            }
            (Prop::Progress(progress), Widget::Progress(p)) => {
                p.cast::<w::IProgressBar>()?.SetIsIndeterminate(progress.is_none())?;
                if let Some(fraction) = progress {
                    p.cast::<w::IRangeBase>()?.SetValue(*fraction)?;
                }
            }
            (Prop::SelectedIndex(index), Widget::Select(combo)) => {
                let index = index.map_or(-1, |i| i as i32);
                node.shown_index.set(index);
                combo.cast::<w::ISelector>()?.SetSelectedIndex(index)?;
            }
            (Prop::Value(t), Widget::Field(f)) => {
                let field: w::ITextBox = f.cast()?;
                // Don't disturb the caret when the field already shows it.
                if field.Text()? != *t {
                    *node.shown_text.borrow_mut() = t.clone();
                    field.SetText(t)?;
                }
            }
            (Prop::Placeholder(t), Widget::Field(f)) => f.cast::<w::ITextBox>()?.SetPlaceholderText(t)?,
            (Prop::Checked(c), Widget::Checkbox(b)) => {
                node.shown_checked.set(*c);
                b.cast::<w::IToggleButton>()?.SetIsChecked(Some(*c))?;
            }
            (Prop::Checked(c), Widget::Switch(s)) => {
                node.shown_checked.set(*c);
                s.cast::<w::IToggleSwitch>()?.SetIsOn(*c)?;
            }
            (Prop::Enabled(e), _) if is_control(&node.widget) => {
                node.element.cast::<w::IControl>()?.SetIsEnabled(*e)?
            }
            (Prop::TextStyle(text_style), Widget::Label(l)) => {
                l.cast::<w::IFrameworkElement>()?.SetStyle(&style(text_style_resource(*text_style)))?;
                if *text_style == TextStyle::Monospace {
                    l.cast::<w::ITextBlock>()?.SetFontFamily(&w::FontFamily::CreateInstanceWithName(MONOSPACE)?)?;
                }
                node.text_style = Some(*text_style);
            }
            (Prop::TextStyle(text_style), _) if is_control(&node.widget) => {
                let control: w::IControl = node.element.cast()?;
                control.SetFontSize(font_size(*text_style))?;
                control.SetFontWeight(w::FontWeight { weight: font_weight(*text_style) })?;
                if *text_style == TextStyle::Monospace {
                    control.SetFontFamily(&w::FontFamily::CreateInstanceWithName(MONOSPACE)?)?;
                }
                node.text_style = Some(*text_style);
            }
            (Prop::ScrollAxes(axes), Widget::Scroll(s)) => set_scroll_axes(&s.cast()?, *axes)?,
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone())?,
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode)?,
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows)?,
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::Variant(variant), Widget::Button(b)) => {
                let name = match variant {
                    ButtonVariant::Primary => "AccentButtonStyle",
                    ButtonVariant::Plain => "SubtleButtonStyle",
                    // Fluent has no destructive button style.
                    ButtonVariant::Default | ButtonVariant::Destructive => "DefaultButtonStyle",
                };
                b.cast::<w::IFrameworkElement>()?.SetStyle(&style(name))?;
                node.variant = Some(*variant);
            }
            _ => {}
        }
        Ok(())
    }

    fn element(&self, id: NodeId, command: &Command) -> w::UIElement {
        match self.nodes.get(&id) {
            Some(node) => node.element.clone(),
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    fn apply(&mut self, command: &Command) -> R<()> {
        match command {
            Command::Create { id, kind, props } => {
                if self.nodes.contains_key(id) {
                    violation(command, "node already exists");
                }
                self.create(*id, *kind, command)?;
                for prop in props {
                    self.set_prop(*id, prop, command)?;
                }
            }
            Command::SetProp { id, prop } => self.set_prop(*id, prop, command)?,
            Command::Insert { parent, child, index } => {
                let child_element = self.element(*child, command);
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                match &self.nodes.get(parent).map(|n| &n.widget) {
                    Some(Widget::List(list)) => {
                        let Some(row) = self.nodes[child].row else {
                            violation(command, "a List's children are row hosts (Containers with a Prop::Row)")
                        };
                        list.insert(row, *child, child_element.clone());
                    }
                    Some(Widget::Scroll(scroll)) => {
                        let content = scroll.cast::<w::IContentControl>()?;
                        if content.Content().is_ok_and(|c| !c.as_raw().is_null()) {
                            violation(command, "a ScrollView has a single native child (its content)");
                        }
                        // The content has a fixed size: at the default Stretch,
                        // XAML centers it when it's smaller than the viewport.
                        let fe: w::IFrameworkElement = child_element.cast()?;
                        fe.SetHorizontalAlignment(w::HorizontalAlignment::Left)?;
                        fe.SetVerticalAlignment(w::VerticalAlignment::Top)?;
                        content.SetContent(&child_element)?;
                    }
                    Some(_) => {
                        let children = self.children(*parent, command)?;
                        let index = (*index as u32).min(children.Size()?);
                        children.InsertAt(index, &child_element)?;
                    }
                    None => violation(command, "parent does not exist"),
                }
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                let child_element = self.element(*child, command);
                match &self.nodes[parent].widget {
                    Widget::List(list) => list.remove(self.nodes[child].row.expect("inserted with a row")),
                    Widget::Scroll(scroll) => scroll.cast::<w::IContentControl>()?.SetContent(None::<&IInspectable>)?,
                    _ => {
                        let children = self.children(*parent, command)?;
                        let mut index = 0;
                        if children.IndexOf(&child_element, &mut index)? {
                            children.RemoveAt(index)?;
                        }
                    }
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                if let Some(parts) = self.window_of(*id)
                    && parts.focus.get() == Some(*id)
                {
                    parts.focus.set(None);
                }
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                self.by_element.borrow_mut().remove(&key(&node.element));
                self.pending_show.retain(|w| w != id);
                drop(node.revokers);
                if let Widget::Window(parts) = node.widget {
                    let WindowParts { window, menu_revokers, .. } = *parts;
                    drop(menu_revokers);
                    window.cast::<w::IWindow>()?.Close()?;
                }
            }
            Command::SetFrame { id, frame } => {
                let element = self.element(*id, command);
                w::Canvas::SetLeft(&element, frame.x() as f64)?;
                w::Canvas::SetTop(&element, frame.y() as f64)?;
                let fe: w::IFrameworkElement = element.cast()?;
                fe.SetWidth(frame.width() as f64)?;
                fe.SetHeight(frame.height() as f64)?;
                if let (Some(row), Some(Widget::List(list))) =
                    (self.nodes[id].row, self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget))
                {
                    list.set_row_height(row, frame.height());
                }
            }
            Command::SetA11y { id, a11y } => {
                self.element(*id, command);
                // On the control itself, not the Border a native render sits in.
                let element = self.nodes[id].control().clone();
                let A11yProps { label, description, hidden, .. } = a11y;
                // An empty name means "derive it from the content".
                let named_by_label = matches!(
                    self.nodes[id].widget,
                    Widget::Switch(_) | Widget::Select(_) | Widget::Slider { .. } | Widget::Progress(_)
                );
                if !named_by_label || label.is_some() {
                    w::AutomationProperties::SetName(&element, label.as_deref().unwrap_or(""))?;
                }
                w::AutomationProperties::SetHelpText(&element, description.as_deref().unwrap_or(""))?;
                w::AutomationProperties::SetAccessibilityView(
                    &element,
                    if *hidden { w::AccessibilityView::Raw } else { w::AccessibilityView::Content },
                )?;
            }
            Command::SetWindowSize { id, size } => match self.nodes.get_mut(id).map(|n| &mut n.widget) {
                Some(Widget::Window(parts)) => {
                    parts.requested = Some(*size);
                    resize_client(parts, *size);
                }
                _ => violation(command, "not a window"),
            },
            Command::SetFocusOrder { window, order } => match self.nodes.get(window).map(|n| &n.widget) {
                // XAML scopes TabIndex to each container, so it can't express
                // a window-wide order across nested hosts: Tab is handled on
                // the window's root instead (see `tab`).
                Some(Widget::Window(parts)) => *parts.tab_order.borrow_mut() = order.clone(),
                _ => violation(command, "not a window"),
            },
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll(scroll)) => {
                    let node = &self.nodes[id];
                    scroll_now(&self.emitter, *id, &node.offset, &scroll.cast()?, *offset)?;
                }
                Some(Widget::List(list)) => list.scroll_to(*offset)?,
                _ => violation(command, "not a ScrollView or List"),
            },
            Command::Focus { id } => {
                self.element(*id, command);
                self.focus(*id, w::FocusState::Programmatic);
            }
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row)?,
                _ => violation(command, "not a List"),
            },
        }
        Ok(())
    }

    /// Lets list views lay out now rather than at XAML's next layout pass:
    /// they realise the containers of the rows in view, and the rows they
    /// report are built in the same run-loop turn. Their handlers only
    /// touch their own data and emit.
    fn layout_lists(&self) {
        for node in self.nodes.values() {
            if let Widget::List(list) = &node.widget {
                list.layout();
            }
        }
    }

    /// The window `id` is in (or is).
    fn window_of(&self, id: NodeId) -> Option<&WindowParts> {
        let mut current = Some(id);
        while let Some(id) = current {
            let node = self.nodes.get(&id)?;
            if let Widget::Window(parts) = &node.widget {
                return Some(parts);
            }
            current = node.parent;
        }
        None
    }

    /// Reports a value the user changed through us (a toggle, typing) right
    /// away; XAML's change events arrive later and find it reported.
    fn report_value(&self, id: NodeId) {
        let Some(node) = self.nodes.get(&id) else { return };
        let changed = match &node.widget {
            Widget::Field(f) => f.cast::<w::ITextBox>().and_then(|f| f.Text()).ok().and_then(|text| {
                let mut shown = node.shown_text.borrow_mut();
                (*shown != text).then(|| {
                    *shown = text.clone();
                    EventValue::Text(text)
                })
            }),
            Widget::Slider { slider, .. } => slider
                .cast::<w::IRangeBase>()
                .and_then(|r| r.Value())
                .ok()
                .and_then(|v| (node.shown_number.replace(v) != v).then_some(EventValue::Number(v))),
            Widget::Select(combo) => combo
                .cast::<w::ISelector>()
                .and_then(|s| s.SelectedIndex())
                .ok()
                .and_then(|i| (i >= 0 && node.shown_index.replace(i) != i).then_some(EventValue::Index(i as usize))),
            Widget::Checkbox(_) | Widget::Switch(_) => {
                let value = match &node.widget {
                    Widget::Checkbox(b) => b.cast::<w::IToggleButton>().and_then(|b| b.IsChecked()).ok(),
                    Widget::Switch(s) => s.cast::<w::IToggleSwitch>().and_then(|s| s.IsOn()).ok(),
                    _ => None,
                };
                value.and_then(|v| (node.shown_checked.replace(v) != v).then_some(EventValue::Bool(v)))
            }
            _ => None,
        };
        if let Some(value) = changed {
            self.emitter.emit(id, UiEvent::Changed(value));
        }
    }

    /// Focuses a control and reports it right away.
    fn focus(&self, id: NodeId, how: w::FocusState) -> bool {
        let Some(node) = self.nodes.get(&id) else { return false };
        let focused = node.control().cast::<w::IUIElement>().and_then(|e| e.Focus(how)).unwrap_or(false);
        if focused && let Some(parts) = self.window_of(id) {
            report_focus(&self.emitter, &parts.focus, Some(id));
        }
        focused
    }

    fn children(&self, parent: NodeId, command: &Command) -> R<w::UIElementCollection> {
        match &self.nodes.get(&parent).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => parts.host.cast::<w::IPanel>()?.Children(),
            Some(Widget::Host(canvas)) => canvas.cast::<w::IPanel>()?.Children(),
            Some(_) => violation(command, "not a container"),
            None => violation(command, "node does not exist"),
        }
    }
}

/// The `Border` a native render or view sits in: it carries our frame, and
/// the control inside sizes itself.
fn wrap(control: &w::UIElement) -> R<w::UIElement> {
    let border = w::Border::new()?;
    border.cast::<w::IBorder>()?.SetChild(control)?;
    border.cast()
}

fn is_control(widget: &Widget) -> bool {
    matches!(
        widget,
        Widget::Field(_)
            | Widget::Button(_)
            | Widget::Checkbox(_)
            | Widget::Switch(_)
            | Widget::Select(_)
            | Widget::Slider { .. }
            | Widget::Scroll(_)
            | Widget::List(_)
    )
}

fn set_scroll_axes(scroll: &w::IScrollViewer, axes: ScrollAxes) -> R<()> {
    let (visible, hidden) = (w::ScrollBarVisibility::Auto, w::ScrollBarVisibility::Disabled);
    let (on, off) = (w::ScrollMode::Enabled, w::ScrollMode::Disabled);
    scroll.SetHorizontalScrollBarVisibility(if axes.horizontal() { visible } else { hidden })?;
    scroll.SetVerticalScrollBarVisibility(if axes.vertical() { visible } else { hidden })?;
    scroll.SetHorizontalScrollMode(if axes.horizontal() { on } else { off })?;
    scroll.SetVerticalScrollMode(if axes.vertical() { on } else { off })
}

/// Moves focus along the core's Tab order, skipping controls that can't
/// take focus now. Returns the node that took it.
fn tab(
    root: &w::Grid,
    by_element: &ElementMap,
    from: Option<NodeId>,
    order: &[NodeId],
    backwards: bool,
) -> Option<NodeId> {
    if order.is_empty() {
        return None;
    }
    let elements: HashMap<NodeId, usize> = by_element.borrow().iter().map(|(k, v)| (*v, *k)).collect();
    let start = from.and_then(|f| order.iter().position(|id| *id == f));
    let n = order.len();
    for step in 1..=n {
        let i = match (start, backwards) {
            (Some(s), false) => (s + step) % n,
            (Some(s), true) => (s + n - step) % n,
            (None, false) => step - 1,
            (None, true) => n - step,
        };
        let Some(element) = elements.get(&order[i]).and_then(|k| find_element(root, *k)) else { continue };
        if element.Focus(w::FocusState::Keyboard).unwrap_or(false) {
            return Some(order[i]);
        }
    }
    None
}

/// The element with COM identity `key` under `root`.
fn find_element(root: &w::Grid, key_: usize) -> Option<w::IUIElement> {
    fn walk(object: w::DependencyObject, key_: usize, depth: usize) -> Option<w::IUIElement> {
        if key(&object) == key_ {
            return object.cast().ok();
        }
        if depth > 64 {
            return None;
        }
        let panel = object.cast::<w::IPanel>().ok();
        if let Some(children) = panel.and_then(|p| p.Children().ok()) {
            for child in elements(&children) {
                if let Some(found) = child.cast().ok().and_then(|c| walk(c, key_, depth + 1)) {
                    return Some(found);
                }
            }
        }
        let content = object.cast::<w::IContentControl>().ok().and_then(|c| c.Content().ok());
        if let Some(found) = content.and_then(|c| c.cast().ok()).and_then(|c| walk(c, key_, depth + 1)) {
            return Some(found);
        }
        None
    }
    walk(root.cast().ok()?, key_, 0)
}

fn ceil(size: w::Size) -> Size {
    Size::new(size.width.ceil(), size.height.ceil())
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
        PlatformMetrics {
            scale_factor: unsafe { w::GetDpiForSystem() } as f32 / 96.0,
            // Fluent's spacing ramp: 4, 8, 12, 16, 24 epx.
            spacing: SpacingScale { xs: 4.0, sm: 8.0, md: 12.0, lg: 16.0, xl: 24.0 },
            font_sizes: font_sizes(),
            dark_mode: dark,
            high_contrast: w::AccessibilitySettings::new()
                .and_then(|s| s.cast::<w::IAccessibilitySettings>()?.HighContrast())
                .unwrap_or(false),
            reduced_motion: settings
                .and_then(|s| s.cast::<w::IUISettings>().ok()?.AnimationsEnabled().ok())
                .is_some_and(|enabled| !enabled),
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
        state.layout_lists();
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
        let natural = match &node.widget {
            Widget::Label(_) => {
                // TODO: min-content (longest word). XAML wraps per character
                // at width 0, so min-content uses max-content for now.
                let width = request.known_width.or(match request.available_width {
                    AvailableSpace::Definite(w) => Some(w),
                    AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                });
                ceil(measure_element(&node.element, w::Size { width: width.unwrap_or(f32::INFINITY), ..infinite }))
            }
            Widget::Field(_) => {
                // Text boxes have no useful intrinsic width.
                let size = ceil(measure_element(&node.element, infinite));
                Size::new(size.width.max(200.0), size.height)
            }
            Widget::Button(_)
            | Widget::Checkbox(_)
            | Widget::Switch(_)
            | Widget::Select(_)
            | Widget::Slider { .. }
            | Widget::Progress(_) => ceil(measure_element(&node.element, infinite)),
            Widget::Custom { render, props } => render
                .measure(node.control(), props.props(), &request)
                .unwrap_or_else(|| ceil(measure_element(&node.element, infinite))),
            Widget::Native { measure: Some(measure), .. } => measure(node.control(), &request),
            Widget::Native { measure: None, .. } => ceil(measure_element(&node.element, infinite)),
            // Measured by the core.
            Widget::Drawn { .. } | Widget::Window(_) | Widget::Host(_) | Widget::Scroll(_) | Widget::List(_) => {
                Size::ZERO
            }
        };
        Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        // A list's rows: select or activate them, as clicking or double
        // clicking their container does.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                match action {
                    A11yAction::Select if list.mode() != SelectionMode::None => {
                        list.select(row).map_err(|_| ActionError::Unsupported)?
                    }
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
        }
        let (element, kind, enabled, shown_text, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let enabled = is_control(&node.widget)
                .then(|| node.element.cast::<w::IControl>().and_then(|c| c.IsEnabled()).unwrap_or(true));
            let custom = match &node.widget {
                Widget::Custom { render, props } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            (node.control().clone(), node.kind, enabled, node.shown_text.clone(), state.emitter(), custom)
        };
        if enabled == Some(false) {
            return Err(ActionError::Disabled);
        }
        if let Some((render, props)) = custom {
            return render.perform(&element, &props, action, &crate::custom::Emitter::new(events, id));
        }
        // No state borrow below: XAML may call back into our handlers.
        let peer =
            || w::FrameworkElementAutomationPeer::CreatePeerForElement(&element).map_err(|_| ActionError::Unsupported);
        match (action, kind) {
            (A11yAction::Activate, WidgetKind::Button) => {
                // What assistive technology does: the UIA Invoke pattern.
                let invoke: w::IInvokeProvider = peer()?
                    .GetPattern(w::PatternInterface::Invoke)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                invoke.Invoke().map_err(|_| ActionError::Unsupported)?;
            }
            (A11yAction::Activate, WidgetKind::Checkbox | WidgetKind::Switch) => {
                let toggle: w::IToggleProvider = peer()?
                    .GetPattern(w::PatternInterface::Toggle)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                toggle.Toggle().map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            // What a screen reader does: the RangeValue pattern.
            (A11yAction::Increment | A11yAction::Decrement | A11yAction::SetValue(_), WidgetKind::Slider) => {
                let range: w::IRangeValueProvider = peer()?
                    .GetPattern(w::PatternInterface::RangeValue)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                let value = match action {
                    A11yAction::SetValue(text) => text.trim().parse().map_err(|_| ActionError::Unsupported)?,
                    _ => {
                        let step = range.SmallChange().unwrap_or(1.0);
                        let step = if *action == A11yAction::Increment { step } else { -step };
                        let (min, max) = (range.Minimum().unwrap_or(f64::MIN), range.Maximum().unwrap_or(f64::MAX));
                        (range.Value().unwrap_or(0.0) + step).clamp(min, max)
                    }
                };
                range.SetValue(value).map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                // As if the option were picked from the open drop-down.
                let combo: w::ComboBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                let index = option_texts(&combo).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                combo
                    .cast::<w::ISelector>()
                    .and_then(|s| s.SetSelectedIndex(index as i32))
                    .map_err(|_| ActionError::Unsupported)?;
                self.state.borrow().report_value(id);
            }
            (A11yAction::SetValue(text), WidgetKind::TextInput) => {
                // The Value pattern where XAML offers it, else the property.
                let value = peer()?.GetPattern(w::PatternInterface::Value).and_then(|p| p.cast::<w::IValueProvider>());
                let set = match value {
                    Ok(value) => value.SetValue(text),
                    Err(_) => element.cast::<w::ITextBox>().and_then(|f| f.SetText(text)),
                };
                set.map_err(|_| ActionError::Unsupported)?;
                // An assistive technology edit is a user edit; report it now
                // rather than when XAML's (asynchronous) TextChanged arrives.
                *shown_text.borrow_mut() = text.clone();
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            (A11yAction::Focus, _) => {
                if !self.state.borrow().focus(id, w::FocusState::Programmatic) {
                    return Err(ActionError::Unsupported);
                }
            }
            // Native views: what a screen reader does, through the element's
            // UI Automation patterns. The control's own events report back.
            (A11yAction::Activate, WidgetKind::Native) => {
                let peer = peer()?;
                if let Ok(invoke) =
                    peer.GetPattern(w::PatternInterface::Invoke).and_then(|p| p.cast::<w::IInvokeProvider>())
                {
                    invoke.Invoke().map_err(|_| ActionError::Unsupported)?;
                } else {
                    let toggle: w::IToggleProvider = peer
                        .GetPattern(w::PatternInterface::Toggle)
                        .and_then(|p| p.cast())
                        .map_err(|_| ActionError::Unsupported)?;
                    toggle.Toggle().map_err(|_| ActionError::Unsupported)?;
                }
            }
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Native) => {
                let range: w::IRangeValueProvider = peer()?
                    .GetPattern(w::PatternInterface::RangeValue)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                let step = range.SmallChange().unwrap_or(1.0);
                let step = if *action == A11yAction::Increment { step } else { -step };
                let (min, max) = (range.Minimum().unwrap_or(f64::MIN), range.Maximum().unwrap_or(f64::MAX));
                let value = (range.Value().unwrap_or(0.0) + step).clamp(min, max);
                range.SetValue(value).map_err(|_| ActionError::Unsupported)?;
            }
            (A11yAction::SetValue(text), WidgetKind::Native) => {
                let peer = peer()?;
                if let Ok(value) =
                    peer.GetPattern(w::PatternInterface::Value).and_then(|p| p.cast::<w::IValueProvider>())
                {
                    value.SetValue(text).map_err(|_| ActionError::Unsupported)?;
                } else {
                    let range: w::IRangeValueProvider = peer
                        .GetPattern(w::PatternInterface::RangeValue)
                        .and_then(|p| p.cast())
                        .map_err(|_| ActionError::Unsupported)?;
                    let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                    range.SetValue(value).map_err(|_| ActionError::Unsupported)?;
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
                Some(Widget::Drawn { view, .. }) => {
                    view.click(*point);
                    Ok(())
                }
                Some(_) => Err(ActionError::Unsupported),
                None => Err(ActionError::UnknownNode),
            };
        }
        let (widget_kind, element, enabled) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let enabled = is_control(&node.widget)
                .then(|| node.element.cast::<w::IControl>().and_then(|c| c.IsEnabled()).unwrap_or(true));
            (node.kind, node.element.clone(), enabled)
        };
        {
            let state = self.state.borrow();
            if let Some(Widget::List(list)) = state.nodes.get(&id).map(|n| &n.widget) {
                return match input {
                    SyntheticInput::Scroll { dy, .. } => {
                        let scroll = list.scroll_viewer().ok_or(ActionError::Unsupported)?;
                        let max = scroll.ScrollableHeight().unwrap_or(0.0).max(0.0);
                        let y = (scroll.VerticalOffset().unwrap_or(0.0) + *dy as f64).clamp(0.0, max);
                        list.scroll_to(Point::new(0.0, y as f32)).map_err(|_| ActionError::Unsupported)
                    }
                    SyntheticInput::Key(key) if list.mode() != SelectionMode::None => {
                        _ = list.view.cast::<w::IUIElement>().and_then(|e| e.Focus(w::FocusState::Keyboard));
                        match list.key(*key) {
                            Ok(true) => Ok(()),
                            _ => Err(ActionError::Unsupported),
                        }
                    }
                    _ => Err(ActionError::Unsupported),
                };
            }
        }
        if let SyntheticInput::Scroll { dx, dy } = input {
            let scroll: w::IScrollViewer = element.cast().map_err(|_| ActionError::Unsupported)?;
            _ = element.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
            let clamp = |v: f64, max: f64| v.clamp(0.0, max.max(0.0));
            let x =
                clamp(scroll.HorizontalOffset().unwrap_or(0.0) + *dx as f64, scroll.ScrollableWidth().unwrap_or(0.0));
            let y =
                clamp(scroll.VerticalOffset().unwrap_or(0.0) + *dy as f64, scroll.ScrollableHeight().unwrap_or(0.0));
            let state = self.state.borrow();
            scroll_now(&state.emitter, id, &state.nodes[&id].offset, &scroll, Point::new(x as f32, y as f32))
                .map_err(|_| ActionError::Unsupported)?;
            return Ok(());
        }
        if enabled == Some(false) {
            return Err(ActionError::Disabled);
        }
        let SyntheticInput::Key(key) = input else { unreachable!() };
        match (widget_kind, key) {
            (WidgetKind::TextInput, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                let field: w::ITextBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                let ui: w::IUIElement = element.cast().map_err(|_| ActionError::Unsupported)?;
                if ui.FocusState().unwrap_or(w::FocusState::Unfocused) == w::FocusState::Unfocused {
                    self.state.borrow().focus(id, w::FocusState::Keyboard);
                    // Focusing may select everything; typing should append,
                    // as after clicking past the end of the text.
                    let end = field.Text().map_or(0, |t| t.encode_utf16().count() as i32);
                    _ = field.SetSelectionStart(end);
                    _ = field.SetSelectionLength(0);
                }
                // Edits go through the selection, like typing does, so
                // TextChanged reports them.
                let edit = |text: &str| -> windows_core::Result<()> {
                    let start = field.SelectionStart()?;
                    field.SetSelectedText(text)?;
                    field.SetSelectionStart(start + text.encode_utf16().count() as i32)?;
                    field.SetSelectionLength(0)
                };
                let result = match key {
                    Key::Char(c) => edit(&c.to_string()),
                    Key::Backspace => (|| {
                        if field.SelectionLength()? == 0 {
                            let start = field.SelectionStart()?;
                            if start == 0 {
                                return Ok(());
                            }
                            field.SetSelectionStart(start - 1)?;
                            field.SetSelectionLength(1)?;
                        }
                        edit("")
                    })(),
                    Key::Enter => {
                        self.state.borrow().emitter().emit(id, UiEvent::Submit);
                        Ok(())
                    }
                    _ => {
                        self.tab_from(id);
                        Ok(())
                    }
                };
                self.state.borrow().report_value(id);
                result.map_err(|_| ActionError::Unsupported)
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
        match &node.widget {
            Widget::Window(parts) => props.push(Prop::Title(parts.window.cast::<w::IWindow>().ok()?.Title().ok()?)),
            Widget::Label(l) => props.push(Prop::Text(l.cast::<w::ITextBlock>().ok()?.Text().ok()?)),
            Widget::Field(f) => {
                let field: w::ITextBox = f.cast().ok()?;
                props.push(Prop::Value(field.Text().ok()?));
                let placeholder = field.PlaceholderText().ok()?;
                if !placeholder.is_empty() {
                    props.push(Prop::Placeholder(placeholder));
                }
            }
            Widget::Button(_) => {
                props.extend(unboxed(node.element.cast::<w::IContentControl>().ok()?.Content()).map(Prop::Label))
            }
            Widget::Checkbox(b) => {
                props.extend(unboxed(node.element.cast::<w::IContentControl>().ok()?.Content()).map(Prop::Label));
                props.push(Prop::Checked(b.cast::<w::IToggleButton>().ok()?.IsChecked().unwrap_or(false)));
            }
            Widget::Switch(s) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.push(Prop::Checked(s.cast::<w::IToggleSwitch>().ok()?.IsOn().ok()?));
            }
            Widget::Slider { slider, step } => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let range: w::IRangeBase = slider.cast().ok()?;
                props.push(Prop::Range { min: range.Minimum().ok()?, max: range.Maximum().ok()? });
                props.push(Prop::Step(*step));
                props.push(Prop::Number(range.Value().ok()?));
            }
            Widget::Progress(p) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let indeterminate = p.cast::<w::IProgressBar>().ok()?.IsIndeterminate().ok()?;
                let value = p.cast::<w::IRangeBase>().ok()?.Value().ok()?;
                props.push(Prop::Progress((!indeterminate).then_some(value)));
            }
            Widget::Select(combo) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.push(Prop::Options(option_texts(combo)));
                let index = combo.cast::<w::ISelector>().ok()?.SelectedIndex().ok()?;
                props.push(Prop::SelectedIndex(usize::try_from(index).ok()));
            }
            Widget::Scroll(s) => {
                let scroll: w::IScrollViewer = s.cast().ok()?;
                let shows = |v: w::ScrollBarVisibility| v != w::ScrollBarVisibility::Disabled;
                let h = scroll.HorizontalScrollBarVisibility().is_ok_and(shows);
                let v = scroll.VerticalScrollBarVisibility().is_ok_and(shows);
                props.push(Prop::ScrollAxes(match (h, v) {
                    (true, true) => ScrollAxes::Both,
                    (true, false) => ScrollAxes::Horizontal,
                    _ => ScrollAxes::Vertical,
                }));
            }
            Widget::Custom { render, props: last } => {
                props.push(Prop::Custom(last.with_props(render.read(node.control(), last.props()))))
            }
            Widget::Drawn { view, props: last } => {
                props.push(Prop::Custom(last.clone()));
                props.push(Prop::Drawing(view.drawing()));
            }
            Widget::Native { last, .. } => props.push(Prop::Native(last.clone())),
            Widget::List(list) => {
                props.push(Prop::Rows(list.rows()));
                props.extend(list.estimate().map(Prop::EstimatedRowHeight));
                props.push(Prop::SelectionMode(list.mode()));
                props.push(Prop::Selected(list.selected()));
            }
            Widget::Host(_) => props.extend(node.row.map(Prop::Row)),
        }
        if is_control(&node.widget) && !matches!(node.widget, Widget::Scroll(_) | Widget::List(_)) {
            props.push(Prop::Enabled(node.element.cast::<w::IControl>().ok()?.IsEnabled().ok()?));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.variant.map(Prop::Variant));

        let frame = match &node.widget {
            Widget::Window(_) => Rect::ZERO,
            _ => {
                let fe: w::IFrameworkElement = node.element.cast().ok()?;
                let finite = |v: f64| if v.is_nan() { 0.0 } else { v as f32 };
                Rect::new(
                    finite(w::Canvas::GetLeft(&node.element).unwrap_or(0.0)),
                    finite(w::Canvas::GetTop(&node.element).unwrap_or(0.0)),
                    finite(fe.Width().unwrap_or(0.0)),
                    finite(fe.Height().unwrap_or(0.0)),
                )
            }
        };
        // A row is where the list view put it.
        let frame = match node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget) {
            Some(Widget::List(list)) => list.row_rect(&node.element, frame),
            _ => frame,
        };
        let by_element = state.by_element.borrow();
        let known = |element: &IInspectable| by_element.get(&key(element)).copied();
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => (list.children(), Some(list.scroll_offset())),
            Widget::Scroll(s) => {
                let scroll: w::IScrollViewer = s.cast().ok()?;
                let content = node.element.cast::<w::IContentControl>().ok()?.Content().ok();
                (
                    content.as_ref().and_then(known).into_iter().collect(),
                    Some(Point::new(
                        scroll.HorizontalOffset().unwrap_or(0.0) as f32,
                        scroll.VerticalOffset().unwrap_or(0.0) as f32,
                    )),
                )
            }
            Widget::Window(parts) => (panel_children(&parts.host, &known), None),
            Widget::Host(canvas) => (panel_children(canvas, &known), None),
            _ => (Vec::new(), None),
        };
        // A list's focus goes to a row's container: the list has it when
        // the window's focus tracking (which walks up to it) says so.
        let list_focused = matches!(node.widget, Widget::List(_))
            && state.window_of(id).is_some_and(|parts| parts.focus.get() == Some(id));
        let focused = list_focused
            || !matches!(node.widget, Widget::Window(_))
                && node
                    .control()
                    .cast::<w::IUIElement>()
                    .and_then(|e| e.FocusState())
                    .is_ok_and(|f| f != w::FocusState::Unfocused);
        Some(NativeState { kind: node.kind, props, frame, parent: node.parent, children, focused, scroll_offset })
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

fn panel_children(panel: &w::Canvas, known: &dyn Fn(&IInspectable) -> Option<NodeId>) -> Vec<NodeId> {
    let Ok(children) = panel.cast::<w::IPanel>().and_then(|p| p.Children()) else { return Vec::new() };
    elements(&children).into_iter().filter_map(|c| c.cast::<IInspectable>().ok()).filter_map(|c| known(&c)).collect()
}

fn elements(collection: &w::UIElementCollection) -> Vec<w::UIElement> {
    let size = collection.Size().unwrap_or(0);
    (0..size).filter_map(|i| collection.GetAt(i).ok()).collect()
}

impl WinUiBackend {
    /// Tab pressed in `id`: move along its window's order.
    fn tab_from(&self, id: NodeId) {
        let state = self.state.borrow();
        let mut window = Some(id);
        while let Some(current) = window {
            let node = &state.nodes[&current];
            if let Widget::Window(parts) = &node.widget {
                let order = parts.tab_order.borrow().clone();
                if let Some(next) = tab(&parts.root, &state.by_element, Some(id), &order, false) {
                    report_focus(&state.emitter, &parts.focus, Some(next));
                }
                return;
            }
            window = node.parent;
        }
    }
}
