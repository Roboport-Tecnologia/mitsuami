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
use mitsuami_core::services::{MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData, Reply, Shortcut};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    AnyValue, AppIcon, AppInfo, ButtonRole, ButtonStyle, Command, CustomProps, EventValue, ImageFit, ImageSource,
    Insets, Modality, NativeAppInfo, NativeIcon, NodeId, Opaque, Orientation, Pixels, Point, Prop, Rect, RowKey,
    ScrollAxes, SelectionMode, Size, TextStyle, UiEvent, WidgetKind, find_prop,
};
use mitsuami_core::{Color, FontWeight, HorizontalAlign};
use windows_core::{EventRevoker, HSTRING, IInspectable, IUnknown, Interface};

use crate::bindings as w;
use crate::custom::{DrawnView, ErasedRender, Measure, NativePayload, WinUiCx};
use crate::runtime;
use crate::surface::SurfaceHost;

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
    /// The node that has keyboard focus, as last reported.
    focus: Rc<Cell<Option<NodeId>>>,
    /// The Tab order sent by the core.
    tab_order: Rc<RefCell<Vec<NodeId>>>,
    /// Modal, and the window it belongs to: acted on when it's shown.
    modal: Option<(Option<NodeId>, Modality)>,
    /// The windows it disabled while it's open (application-modal), to
    /// enable again when it closes.
    disabled: Vec<w::HWND>,
    /// Dialogs (modal windows): the Escape accelerator's handler.
    escape: Option<EventRevoker>,
    /// The toolbar, made when its first item arrives: a `CommandBar` whose
    /// primary commands hold the items' hosts, in order.
    toolbar: Option<w::CommandBar>,
    toolbar_items: Vec<(NodeId, w::AppBarElementContainer)>,
    /// Its sidebar's node and navigation view, while it has one: the
    /// view's content is the host.
    sidebar: Option<(NodeId, w::NavigationView)>,
    /// The handlers that move its menu button to the title bar.
    sidebar_revokers: Vec<EventRevoker>,
    /// Full screen as the app wants it, and the user (who changes it
    /// too): what the presenter is compared with when it changes.
    full_screen: Rc<Cell<bool>>,
    /// Made visible: full screen waits for it, as a hidden window would
    /// fill the screen unseen.
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
    Password(w::PasswordBox),
    Button(w::Button),
    /// A button whose `Flyout` is its menu, which it opens on a click.
    MenuButton(w::DropDownButton),
    Checkbox(w::CheckBox),
    Switch(w::ToggleSwitch),
    Select(w::ComboBox),
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
    GpuSurface(SurfaceHost),
    Scroll(w::ScrollViewer),
    List(crate::list::List),
    Sidebar(crate::sidebar::Sidebar),
    Tabs(crate::tabs::Tabs),
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
    /// Checkboxes: whether they show the mixed state (`IsChecked` null).
    /// Leaving it is a change, whatever the value lands on.
    shown_mixed: Rc<Cell<bool>>,
    /// Selects: the chosen index, or -1.
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
    /// Sliders: whether the app gave an `Orientation`.
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

impl ContextMenu {
    fn new(activate: Rc<dyn Fn(u32)>, own: Option<w::FlyoutBase>) -> ContextMenu {
        ContextMenu { sent: Vec::new(), flyout: None, items: MenuItems::default(), revokers: Vec::new(), activate, own }
    }

    fn has_items(menu: &Option<ContextMenu>) -> bool {
        menu.as_ref().is_some_and(|menu| menu.flyout.is_some())
    }

    /// Shows `entries`: in place when only enabled and checked states
    /// changed, so an open menu stays open, as the menu bar does; else a
    /// new flyout (none for no entries). Whether the flyout was replaced.
    fn update(&mut self, entries: &[MenuEntry], scope: String) -> R<bool> {
        let replaced = if self.flyout.is_some() && as_menu_bar(entries).same_structure(&as_menu_bar(&self.sent)) {
            update_items(&self.items, &as_menu_bar(entries));
            false
        } else if self.flyout.is_some() || !entries.is_empty() {
            self.revokers.clear();
            self.items.borrow_mut().clear();
            self.flyout = None;
            if !entries.is_empty() {
                let mut built =
                    MenuBuild { activate: &self.activate, revokers: &mut self.revokers, items: &self.items, scope };
                self.flyout = Some(built.flyout(entries)?);
            }
            true
        } else {
            false
        };
        self.sent = entries.to_vec();
        Ok(replaced)
    }
}

/// Entries as one menu, to compare and update them as menu bars are.
fn as_menu_bar(entries: &[MenuEntry]) -> MenuBarData {
    MenuBarData { menus: vec![MenuData { title: String::new(), entries: entries.to_vec() }] }
}

/// The app's menus, each window's own, and how to report a choice.
#[derive(Default)]
struct Menus {
    app: MenuBarData,
    windows: HashMap<NodeId, MenuBarData>,
    activate: Option<Rc<dyn Fn(u32)>>,
}

impl Menus {
    /// What a window's bar shows: the app's menus, and its own. A dialog
    /// shows only its own: Windows dialogs have no menu bar.
    fn shown(&self, window: NodeId, modal: bool) -> MenuBarData {
        self.app.for_window(self.windows.get(&window), modal)
    }
}

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

/// A button's content: its caption; with an icon, a `FontIcon` before it
/// at Fluent's spacing (8), as WinUI's gallery lays them out; icon only,
/// the `FontIcon`, named by the caption.
fn set_button_content(node: &Node) -> R<()> {
    let control = node.element.cast::<w::IContentControl>()?;
    if node.icon.is_empty() {
        w::AutomationProperties::SetName(&node.element, "")?;
        return control.SetContent(&boxed(&node.caption));
    }
    let icon = w::FontIcon::new()?;
    icon.cast::<w::IFontIcon>()?.SetGlyph(&node.icon)?;
    // UIA reads a panel's or icon's content as nothing: the caption names
    // the button either way.
    w::AutomationProperties::SetName(&node.element, &node.caption)?;
    if node.icon_only == Some(true) {
        return control.SetContent(&icon.cast::<IInspectable>()?);
    }
    let panel = w::StackPanel::new()?;
    let stack = panel.cast::<w::IStackPanel>()?;
    stack.SetOrientation(w::Orientation::Horizontal)?;
    stack.SetSpacing(8.0)?;
    let caption = w::TextBlock::new()?;
    caption.cast::<w::ITextBlock>()?.SetText(&node.caption)?;
    let children = panel.cast::<w::IPanel>()?.Children()?;
    children.Append(&icon.cast::<w::UIElement>()?)?;
    children.Append(&caption.cast::<w::UIElement>()?)?;
    control.SetContent(&panel.cast::<IInspectable>()?)
}

/// What a button's content shows: its caption (the panel's text, or the
/// name of an icon shown alone), its icon, and whether that's alone.
fn button_content(node: &Node) -> Vec<Prop> {
    let Some(content) = node.element.cast::<w::IContentControl>().ok().and_then(|c| c.Content().ok()) else {
        return Vec::new();
    };
    let glyph = |icon: &w::FontIcon| icon.cast::<w::IFontIcon>().ok().and_then(|i| i.Glyph().ok());
    let mut props = Vec::new();
    if let Ok(icon) = content.cast::<w::FontIcon>() {
        props.extend(w::AutomationProperties::GetName(&node.element).ok().map(Prop::Label));
        props.extend(glyph(&icon).map(Prop::Icon));
        props.push(Prop::IconOnly(true));
    } else if let Ok(panel) = content.cast::<w::StackPanel>() {
        let children = panel.cast::<w::IPanel>().ok().and_then(|p| p.Children().ok());
        let children = children.map(|c| elements(&c)).unwrap_or_default();
        let icon = children.first().and_then(|c| c.cast::<w::FontIcon>().ok());
        let caption = children.get(1).and_then(|c| c.cast::<w::ITextBlock>().ok());
        props.extend(caption.and_then(|c| c.Text().ok()).map(Prop::Label));
        props.extend(icon.as_ref().and_then(glyph).map(Prop::Icon));
        if node.icon_only.is_some() {
            props.push(Prop::IconOnly(false));
        }
    } else {
        props.extend(unboxed(Ok(content)).map(Prop::Label));
        // No icon: shown as its caption, whether or not it's icon only.
        props.push(Prop::Icon(String::new()));
        props.extend(node.icon_only.map(Prop::IconOnly));
    }
    props
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

/// A menu button's look. Fluent has no subtle `DropDownButton`, and
/// `SubtleButtonStyle` would replace its template (and its chevron), so
/// borderless is a style over its own that only clears the fill and the
/// border, as a subtle button's are at rest; its template still shows a
/// fill on hover. An explicit style sets only what it says: the theme's
/// template stays.
fn set_menu_button_style(button: &w::DropDownButton, button_style: ButtonStyle) -> R<()> {
    let element = button.cast::<w::IFrameworkElement>()?;
    if button_style != ButtonStyle::Borderless {
        return element.SetStyle(None::<&w::Style>);
    }
    let markup = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="DropDownButton"><Setter Property="Background" Value="{ThemeResource SubtleFillColorTransparentBrush}"/><Setter Property="BorderBrush" Value="{ThemeResource SubtleFillColorTransparentBrush}"/></Style>"#;
    element.SetStyle(&w::XamlReader::Load(markup)?.cast::<w::Style>()?)
}

/// A button's XAML style, from its role and style: one style has both.
/// Borderless wins, since it's how the button is drawn. Fluent has no
/// cancel or destructive style.
fn set_button_style(button: &w::Button, role: Option<ButtonRole>, button_style: Option<ButtonStyle>) -> R<()> {
    let name = match (role.unwrap_or_default(), button_style.unwrap_or_default()) {
        (_, ButtonStyle::Borderless) => "SubtleButtonStyle",
        (ButtonRole::Default, _) => "AccentButtonStyle",
        _ => "DefaultButtonStyle",
    };
    button.cast::<w::IFrameworkElement>()?.SetStyle(&style(name))
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

/// A label's style: its text style's, with its colour on top. The colour
/// is a setter, so a theme brush follows the theme live, as
/// `{ThemeResource}` does in a style.
fn set_label_style(label: &w::TextBlock, text_style: Option<TextStyle>, color: Option<Color>) -> R<()> {
    let base = text_style.map(|s| style(text_style_resource(s)));
    let style = match (color, base) {
        (None, Some(base)) => base,
        (None, None) => return Ok(()),
        (Some(color), base) => {
            let colored = foreground_style("TextBlock", color)?;
            if let Some(base) = base {
                colored.cast::<w::IStyle>()?.SetBasedOn(&base)?;
            }
            colored
        }
    };
    label.cast::<w::IFrameworkElement>()?.SetStyle(&style)
}

/// A style that sets a `target`'s foreground to a colour, as markup, so a
/// theme resource keeps following the theme once set.
fn foreground_style(target: &str, color: Color) -> R<w::Style> {
    let markup = format!(
        r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="{target}"><Setter Property="Foreground" Value="{}"/></Style>"#,
        crate::custom::text_brush(color)
    );
    w::XamlReader::Load(&markup)?.cast()
}

/// Fluent's weights (Segoe UI Variable has each of them).
fn weight_value(weight: FontWeight) -> u16 {
    match weight {
        FontWeight::Regular => 400,
        FontWeight::Medium => 500,
        FontWeight::Semibold => 600,
        FontWeight::Bold => 700,
    }
}

/// The nearest of our weights to a font's.
fn weight_of(weight: u16) -> FontWeight {
    match weight {
        0..450 => FontWeight::Regular,
        450..550 => FontWeight::Medium,
        550..650 => FontWeight::Semibold,
        _ => FontWeight::Bold,
    }
}

fn font_weight(style: TextStyle) -> u16 {
    match style {
        TextStyle::LargeTitle | TextStyle::Title | TextStyle::Headline => 600,
        _ => 400,
    }
}

const MONOSPACE: &str = "Cascadia Mono, Consolas";

/// A window's content: the title bar (content extends into it, the Windows
/// 11 way), a row for the menu bar, one for the toolbar, then the content
/// host. The root and the host carry the window background (window captures
/// render the host).
const WINDOW_ROOT: &str = r#"
<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      Background="{ThemeResource SolidBackgroundFillColorBaseBrush}">
  <Grid.RowDefinitions>
    <RowDefinition Height="Auto"/>
    <RowDefinition Height="Auto"/>
    <RowDefinition Height="Auto"/>
    <RowDefinition Height="*"/>
  </Grid.RowDefinitions>
  <TitleBar Grid.Row="0" IsTabStop="False"/>
  <Canvas Grid.Row="3" Background="{ThemeResource SolidBackgroundFillColorBaseBrush}"/>
</Grid>"#;

/// Where the menu bar goes in `WINDOW_ROOT`.
const MENU_ROW: i32 = 1;

/// Where the content host goes in `WINDOW_ROOT`, or the sidebar's
/// navigation view holding it.
const CONTENT_ROW: i32 = 3;

/// Where the toolbar goes in `WINDOW_ROOT`: under the menu bar, as Windows
/// apps put their command bars.
const TOOLBAR_ROW: i32 = 2;

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
    /// disabled or went away) whose `GotFocus` hasn't arrived yet. XAML
    /// focuses parts of composite controls (a list's row container, a
    /// number box's text box): the node is the nearest one up from there.
    pub(crate) fn sync_focus(&self) {
        let state = self.state.borrow();
        for node in state.nodes.values() {
            let Widget::Window(parts) = &node.widget else { continue };
            let element = parts
                .host
                .cast::<w::IUIElement>()
                .and_then(|h| h.XamlRoot())
                .and_then(|root| w::FocusManager::GetFocusedElementWithRoot(&root))
                .ok()
                .filter(|e| !e.as_raw().is_null());
            let now = resolve(&state.by_element, element)
                .filter(|id| !matches!(state.nodes.get(id).map(|n| &n.widget), Some(Widget::Window(_))));
            report_focus(&state.emitter, &parts.focus, now);
        }
    }

    /// Called after every event the platform reports, so the run loop turns.
    pub(crate) fn set_wake(&self, wake: impl Fn() + 'static) {
        *self.state.borrow().emitter.wake.borrow_mut() = Some(Rc::new(wake));
    }

    /// Resizes a window's content like the user would, keeping a locked
    /// height; the platform reports it back as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let state = self.state.borrow();
        if let Some(Widget::Window(parts)) = state.nodes.get(&window).map(|n| &n.widget) {
            let height = parts.size.get().filter(|_| parts.height_locked).map_or(size.height, |s| s.height);
            resize_client(parts, Size::new(size.width, height));
        }
    }

    /// Makes visible the windows whose first layout has been applied,
    /// modal ones first made modal.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            self.make_modal(id);
            let mut state = self.state.borrow_mut();
            let (by_element, emitter) = (state.by_element.clone(), state.emitter.clone());
            if let Some(Widget::Window(parts)) = state.nodes.get_mut(&id).map(|n| &mut n.widget) {
                set_transparent(parts.hwnd, false);
                unsafe { _ = w::SetForegroundWindow(parts.hwnd) };
                parts.shown = true;
                // Full screen asked for before it was shown.
                apply_full_screen(parts);
                // It was activated when made, before its content: its
                // controls can take focus now.
                restore_focus(&parts.root, &by_element, &parts.focus, &parts.tab_order.borrow(), &emitter);
            }
        }
        // Windows moved to another display since.
        let state = self.state.borrow();
        for node in state.nodes.values() {
            if let Widget::Window(parts) = &node.widget
                && parts.moved.replace(false)
            {
                apply_min_size(parts);
            }
        }
    }

    /// The Windows App SDK's way: owned by its window (`GWLP_HWNDPARENT`,
    /// how a WinUI 3 window gets an owner), centred on it, and `IsModal`,
    /// which disables the owner while it's shown; a dialog's presenter
    /// can't be minimized or maximized. Application-modal (or without an
    /// owner, where `IsModal` can't apply), it also disables the app's
    /// other windows, as Win32 apps do, until it closes.
    fn make_modal(&self, id: NodeId) {
        let mut state = self.state.borrow_mut();
        let Some(Widget::Window(parts)) = state.nodes.get(&id).map(|n| &n.widget) else { return };
        let Some((owner, modality)) = parts.modal else { return };
        let (hwnd, app_window) = (parts.hwnd, parts.app_window.clone());
        let owner = owner.and_then(|o| match state.nodes.get(&o).map(|n| &n.widget) {
            Some(Widget::Window(owner)) => Some((owner.hwnd, owner.app_window.clone())),
            _ => None,
        });
        let others: Vec<w::HWND> = state
            .nodes
            .values()
            .filter_map(|n| match &n.widget {
                Widget::Window(other) if other.hwnd != hwnd => Some(other.hwnd),
                _ => None,
            })
            .collect();
        let app = app_window.cast::<w::IAppWindow>().ok();
        if let Some((owner_hwnd, owner_window)) = &owner {
            unsafe { w::SetWindowLongPtrW(hwnd, w::GWLP_HWNDPARENT, *owner_hwnd as isize) };
            // Centred on its owner, as dialogs open.
            let owner_window = owner_window.cast::<w::IAppWindow>().ok();
            if let (Some(app), Some(owner_window)) = (&app, owner_window)
                && let (Ok(at), Ok(size), Ok(own)) = (owner_window.Position(), owner_window.Size(), app.Size())
            {
                _ = app.Move(w::PointInt32 {
                    x: at.x + (size.width - own.width) / 2,
                    y: at.y + (size.height - own.height) / 2,
                });
            }
        }
        if let Some(presenter) =
            app.and_then(|a| a.Presenter().ok()).and_then(|p| p.cast::<w::IOverlappedPresenter>().ok())
        {
            _ = presenter.SetIsMinimizable(false);
            _ = presenter.SetIsMaximizable(false);
            if owner.is_some() {
                _ = presenter.SetIsModal(true);
            }
        }
        let mut disabled = Vec::new();
        if modality == Modality::Application || owner.is_none() {
            for other in others {
                // Already disabled (by another modal window): not ours to enable.
                if unsafe { w::IsWindowEnabled(other) }.as_bool() {
                    unsafe { _ = w::EnableWindow(other, false.into()) };
                    disabled.push(other);
                }
            }
        }
        if let Some(Widget::Window(parts)) = state.nodes.get_mut(&id).map(|n| &mut n.widget) {
            parts.disabled = disabled;
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
        // No (or an unknown) parent: the active window, else one a modal
        // window doesn't block, focused if one is, else any window. Every
        // window keeps its focused control while inactive, so focus alone
        // picked a modal window's owner, which it blocks.
        let parts = parts.or_else(|| {
            let windows: Vec<&WindowParts> = state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::Window(parts) => Some(&**parts),
                    _ => None,
                })
                .collect();
            let active = unsafe { w::GetActiveWindow() };
            let enabled = |p: &&WindowParts| unsafe { w::IsWindowEnabled(p.hwnd) }.as_bool();
            windows
                .iter()
                .find(|p| p.hwnd == active)
                .or_else(|| windows.iter().find(|p| enabled(p) && p.focus.get().is_some()))
                .or_else(|| windows.iter().find(|p| enabled(p)))
                .or(windows.first())
                .copied()
        })?;
        Some(f(parts))
    }

    pub(crate) fn xaml_root(&self, id: Option<NodeId>) -> Option<w::XamlRoot> {
        self.window_parts(id, |parts| parts.host.cast::<w::IUIElement>().ok()?.XamlRoot().ok()).flatten()
    }

    /// Installs the app's menus (`None`), shown in every window, or a
    /// window's own, shown in it with the app's. A window's menus may come
    /// before the window does: it gets them when it's created.
    pub(crate) fn set_menu(&self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let mut state = self.state.borrow_mut();
        let State { nodes, menus, .. } = &mut *state;
        menus.activate = Some(activate);
        match window {
            None => menus.app = menu.clone(),
            Some(window) if menu.menus.is_empty() => _ = menus.windows.remove(&window),
            Some(window) => _ = menus.windows.insert(window, menu.clone()),
        }
        for (id, node) in nodes.iter_mut() {
            if let Widget::Window(parts) = &mut node.widget
                && window.is_none_or(|w| w == *id)
            {
                refresh_menu(parts, menus);
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

    /// `WM_CLOSE`, which the close button and Alt+F4 end in: the app
    /// window raises `Closing`. `Window.Close()` wouldn't: it closes
    /// without asking.
    fn close_window(&self, window: NodeId) {
        let hwnd = match self.state.borrow().nodes.get(&window).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => parts.hwnd,
            _ => return,
        };
        request_close(hwnd);
        runtime::pump();
    }

    fn take_command_log(&self) -> Vec<Command> {
        WinUiHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        WinUiHandle::node_count(self)
    }

    /// The process's AppUserModelID and the icon the window's title bar
    /// and taskbar button show. Windows keeps no name for the app.
    fn app_info(&self, window: NodeId) -> NativeAppInfo {
        let mut id = windows_core::PWSTR::null();
        let id = unsafe { w::GetCurrentProcessExplicitAppUserModelID(&mut id) }.is_ok().then(|| unsafe {
            let text = id.to_string().ok();
            w::CoTaskMemFree(id.0.cast());
            text
        });
        let hwnd = match self.state.borrow().nodes.get(&window).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => Some(parts.hwnd),
            _ => None,
        };
        let icon = hwnd
            .map(|hwnd| unsafe { w::SendMessageW(hwnd, w::WM_GETICON as u32, w::ICON_BIG as usize, 0) })
            .filter(|icon| *icon != 0)
            .and_then(|icon| icon_size(icon as w::HICON));
        NativeAppInfo {
            id: id.flatten(),
            name: None,
            icon: icon.map(|(width, height)| NativeIcon::Image { width, height }),
        }
    }

    /// XAML reports some changes (text edits, scrolling, focus) after the
    /// call that caused them: dispatch them.
    fn settle(&self) {
        runtime::pump();
        // Spinners have no size until XAML loads them, at its next frame,
        // and images from files until XAML has decoded them, in the
        // background: wait for both, so tests see the size the app gets.
        // A window's content too is laid out at its new size only at
        // XAML's next frame (a new window's at the size it opened at, wider
        // than the one set), and the toolbar places items from its edge.
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.state.borrow().nodes.values().any(|n| match &n.widget {
            Widget::Spinner(ring) => !ring.cast::<w::IFrameworkElement>().and_then(|f| f.IsLoaded()).unwrap_or(true),
            Widget::Image { bitmap: Some(_), failed, opened, .. } => !failed.get() && !opened.get(),
            Widget::Window(parts) => parts.size.get().is_some_and(|size| {
                let Ok(host) = parts.host.cast::<w::IFrameworkElement>() else { return false };
                let (width, height) = (host.ActualWidth().unwrap_or(0.0), host.ActualHeight().unwrap_or(0.0));
                (width - size.width as f64).abs() > 1.0 || (height - size.height as f64).abs() > 1.0
            }),
            _ => false,
        }) && Instant::now() < deadline
        {
            runtime::wait(Some(Duration::from_millis(5)));
            runtime::pump();
        }
        self.state.borrow().layout_lists();
        // GPU surfaces follow their frames at XAML's next frame, which a
        // settle doesn't wait for: place them now, so the app has the size.
        for node in self.state.borrow().nodes.values() {
            if let Widget::GpuSurface(surface) = &node.widget {
                surface.place_now();
            }
        }
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
    let menu = parts.menu_bar.as_ref().map_or(0.0, |m| height(ok(m.cast(), "menu bar element")));
    // A collapsed toolbar has no height, but may still have a desired one.
    let shown = |bar: &&w::CommandBar| {
        bar.cast::<w::IUIElement>().and_then(|e| e.Visibility()).is_ok_and(|v| v == w::Visibility::Visible)
    };
    let toolbar = parts.toolbar.as_ref().filter(shown).map_or(0.0, |t| height(ok(t.cast(), "toolbar element")));
    title + menu + toolbar
}

/// Adds an item's host to the window's toolbar at `index` among its items,
/// making the toolbar if it's the first.
fn insert_toolbar_item(parts: &mut WindowParts, id: NodeId, host: &w::UIElement, index: usize) -> R<()> {
    if parts.toolbar.is_none() {
        let bar = w::CommandBar::new()?;
        let element: w::UIElement = bar.cast()?;
        w::Grid::SetRow(&element.cast::<w::FrameworkElement>()?, TOOLBAR_ROW)?;
        // The bar ends at the window's edge: its "More" button, last,
        // spaces itself from it, and with no secondary commands it doesn't
        // show. Inset the items as far from that edge as the title bar
        // insets the title from the other (2 + 14). The bar has no
        // background, and its padding only reaches its content area.
        let margin = w::Thickness { left: 0.0, top: 0.0, right: 16.0, bottom: 0.0 };
        bar.cast::<w::IFrameworkElement>()?.SetMargin(margin)?;
        // Hidden until an item has something to show.
        element.cast::<w::IUIElement>()?.SetVisibility(w::Visibility::Collapsed)?;
        parts.root.cast::<w::IPanel>()?.Children()?.Append(&element)?;
        parts.toolbar = Some(bar);
    }
    let bar: w::IUIElement = parts.toolbar.as_ref().expect("made above").cast()?;
    let container = w::AppBarElementContainer::new()?;
    container.cast::<w::IContentControl>()?.SetContent(host)?;
    // Centred in the bar, as the bar's own buttons are: the container is
    // the bar's height, and puts its content at the top by default.
    container.cast::<w::IControl>()?.SetVerticalContentAlignment(w::VerticalAlignment::Center)?;
    let index = index.min(parts.toolbar_items.len());
    let commands = parts.toolbar.as_ref().expect("made above").PrimaryCommands()?;
    commands.InsertAt(index as u32, &container.cast::<w::ICommandBarElement>()?)?;
    parts.toolbar_items.insert(index, (id, container.clone()));
    // XAML measures only what's in a live tree, and a collapsed bar keeps
    // its items out of it: lay them out once, shown, so the core can
    // measure what's in them (a button measures nothing out of the tree).
    let shown = bar.Visibility()?;
    bar.SetVisibility(w::Visibility::Visible)?;
    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
    bar.SetVisibility(shown)?;
    // Empty until its first frame.
    container.cast::<w::IUIElement>()?.SetVisibility(w::Visibility::Collapsed)?;
    Ok(())
}

/// Takes the window's sidebar away: the host goes back in the view's
/// place, and keeps its size.
fn remove_sidebar(parts: &mut WindowParts) -> R<()> {
    let Some((_, view)) = parts.sidebar.take() else { return Ok(()) };
    parts.sidebar_revokers.clear();
    crate::sidebar::Sidebar::leave_title_bar(&parts.title_bar)?;
    let children = parts.root.cast::<w::IPanel>()?.Children()?;
    let mut at = 0;
    if children.IndexOf(&view.cast::<w::UIElement>()?, &mut at)? {
        children.RemoveAt(at)?;
    }
    view.cast::<w::IContentControl>()?.SetContent(None::<&IInspectable>)?;
    children.Append(&parts.host.cast::<w::UIElement>()?)?;
    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
    if let Some(size) = parts.requested.or(parts.size.get()) {
        resize_client(parts, size);
    }
    Ok(())
}

/// Where the sidebar's pane is, in the content host's coordinates (beside
/// it, so at negative x): open, icons only, or closed (none).
fn sidebar_frame(parts: &WindowParts) -> Option<Rect> {
    let (_, view) = parts.sidebar.as_ref()?;
    let width = crate::sidebar::Sidebar::pane_width(view);
    if width <= 0.0 {
        return Some(Rect::ZERO);
    }
    let transform = view.cast::<w::IUIElement>().ok()?.TransformToVisual(&parts.host).ok()?;
    let origin = transform.cast::<w::IGeneralTransform>().ok()?.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
    let height = view.cast::<w::IFrameworkElement>().ok()?.ActualHeight().ok()? as f32;
    Some(Rect::new(origin.x, origin.y, width, height))
}

fn remove_toolbar_item(parts: &mut WindowParts, id: NodeId) -> R<()> {
    let Some(index) = parts.toolbar_items.iter().position(|(item, _)| *item == id) else { return Ok(()) };
    let (_, container) = parts.toolbar_items.remove(index);
    if let Some(bar) = &parts.toolbar {
        bar.PrimaryCommands()?.RemoveAt(index as u32)?;
    }
    container.cast::<w::IContentControl>()?.SetContent(None::<&IInspectable>)?;
    update_toolbar(parts)
}

/// Shows an item while it has a size (and the toolbar while any item
/// does); the content keeps the size the app asked for, below it.
fn update_toolbar(parts: &mut WindowParts) -> R<()> {
    let Some(bar) = &parts.toolbar else { return Ok(()) };
    let visible =
        |element: R<w::IUIElement>| element.and_then(|e| e.Visibility()).is_ok_and(|v| v == w::Visibility::Visible);
    let any = parts.toolbar_items.iter().any(|(_, c)| visible(c.cast()));
    let wanted = if any { w::Visibility::Visible } else { w::Visibility::Collapsed };
    let element: w::IUIElement = bar.cast()?;
    let before = chrome_height(parts);
    if element.Visibility()? != wanted {
        element.SetVisibility(wanted)?;
    }
    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
    if chrome_height(parts) != before {
        apply_min_size(parts);
        if let Some(size) = parts.requested {
            resize_client(parts, size);
        }
    }
    Ok(())
}

/// Where the toolbar shows an item's host, in the content host's
/// coordinates (above it, so at negative y); zero while it's collapsed.
fn toolbar_item_frame(parts: &WindowParts, id: NodeId, element: &w::UIElement) -> Option<Rect> {
    let (_, container) = parts.toolbar_items.iter().find(|(item, _)| *item == id)?;
    let shown = container.cast::<w::IUIElement>().ok()?.Visibility().ok()? == w::Visibility::Visible;
    // An item that didn't fit is in the bar's overflow menu: not shown.
    let overflowed = container.cast::<w::ICommandBarElement>().ok()?.IsInOverflow().ok()?;
    if !shown || overflowed {
        return Some(Rect::ZERO);
    }
    let transform = element.cast::<w::IUIElement>().ok()?.TransformToVisual(&parts.host).ok()?;
    let origin = transform.cast::<w::IGeneralTransform>().ok()?.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
    let fe: w::IFrameworkElement = element.cast().ok()?;
    Some(Rect::new(origin.x, origin.y, fe.Width().ok()? as f32, fe.Height().ok()? as f32))
}

/// Sets the content area (the host, below the title and menu bars) to
/// `size` logical units, and reports the size it got right away, as AppKit
/// does; XAML's own report comes after its next layout pass.
///
/// With the content extended into the title bar, `ResizeClient` sizes the
/// area below the caption strip while `ClientSize` (and XAML's root) include
/// it, so aim, look at what we got, and correct once.
fn resize_client(parts: &WindowParts, size: Size) {
    resize_client_with(parts, size, client_insets(parts, scale_of(parts)));
}

/// `resize_client` with the client insets measured before: they're
/// measured off the root as XAML last laid it out, which lags a window
/// Windows has just resized itself.
fn resize_client_with(parts: &WindowParts, size: Size, (inset_w, inset_h): (i32, i32)) {
    // A window in full screen keeps the screen's size.
    if in_full_screen(&parts.app_window) {
        return;
    }
    let Ok(app_window) = parts.app_window.cast::<w::IAppWindow2>() else { return };
    // No smaller than its minimum, as a drag goes.
    let min = content_min(parts).unwrap_or(Size::ZERO);
    let size = Size::new(size.width.max(min.width), size.height.max(min.height));
    let scale = scale_of(parts);
    let chrome = chrome_height(parts);
    // Beside a sidebar, the window is wider by its pane.
    let side = parts.sidebar.as_ref().map_or(0.0, |(_, view)| crate::sidebar::Sidebar::extra_width(view, size.width));
    let want = w::SizeInt32 {
        width: ((size.width + side) as f64 * scale).round() as i32 + inset_w,
        height: ((size.height as f64 + chrome) * scale).round() as i32 + inset_h,
    };
    if let Some(before) = parts.size.get() {
        parts.aimed.set(Some((size, before)));
    }
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
        let width = ((got.width - inset_w) as f64 / scale) as f32 - side;
        let height = ((got.height - inset_h) as f64 / scale - chrome).max(0.0) as f32;
        parts.report_size(Size::new(width, height));
    }
}

/// Windows keeps a resize border inside the client area of windows with
/// extended title bars (1 px along the top): measured off the live root.
fn client_insets(parts: &WindowParts, scale: f64) -> (i32, i32) {
    let inset = |client: i32, root: R<f64>| match root {
        Ok(root) if root > 0.0 => (client - (root * scale).round() as i32).clamp(0, 8),
        _ => 0,
    };
    let client = parts.app_window.cast::<w::IAppWindow2>().and_then(|a| a.ClientSize());
    match (client, parts.root.cast::<w::IFrameworkElement>()) {
        (Ok(client), Ok(root)) => (inset(client.width, root.ActualWidth()), inset(client.height, root.ActualHeight())),
        _ => (0, 0),
    }
}

fn in_full_screen(app_window: &w::AppWindow) -> bool {
    app_window
        .cast::<w::IAppWindow>()
        .and_then(|a| a.Presenter())
        .and_then(|p| p.cast::<w::IAppWindowPresenter>()?.Kind())
        .is_ok_and(|kind| kind == w::AppWindowPresenterKind::FullScreen)
}

/// Puts the window in full screen, or back in its own presenter, as the
/// app wants, once it's shown. `FullScreenPresenter` has no caption, so
/// the title bar goes too.
fn apply_full_screen(parts: &mut WindowParts) {
    if !parts.shown {
        return;
    }
    let on = parts.full_screen.get();
    if on == in_full_screen(&parts.app_window) {
        return;
    }
    let Ok(app) = parts.app_window.cast::<w::IAppWindow>() else { return };
    let done = if on {
        parts.overlapped = app.Presenter().ok();
        app.SetPresenterByKind(w::AppWindowPresenterKind::FullScreen)
    } else {
        match parts.overlapped.take() {
            Some(presenter) => app.SetPresenter(&presenter),
            None => app.SetPresenterByKind(w::AppWindowPresenterKind::Overlapped),
        }
    };
    let now = in_full_screen(&parts.app_window);
    show_title_bar(parts, !now);
    if !on {
        apply_min_size(parts);
    }
    // Refused: the window stays as it is, and the app hears so.
    if done.is_err() || now != on {
        parts.full_screen.set(now);
        parts.emitter.emit(parts.node, UiEvent::FullScreenChanged(now));
    }
}

fn show_title_bar(parts: &WindowParts, shown: bool) {
    let visibility = if shown { w::Visibility::Visible } else { w::Visibility::Collapsed };
    _ = parts.title_bar.cast::<w::IUIElement>().and_then(|e| e.SetVisibility(visibility));
}

/// Pixels the window adds around its content: the frame (`Size` less
/// `ClientSize`) and the resize border inside the client area.
fn frame_pixels(parts: &WindowParts, scale: f64) -> (i32, i32) {
    let (inset_w, inset_h) = client_insets(parts, scale);
    let app = parts.app_window.cast::<w::IAppWindow>();
    let client = parts.app_window.cast::<w::IAppWindow2>().and_then(|a| a.ClientSize());
    match (app.and_then(|a| a.Size()), client) {
        (Ok(outer), Ok(client)) => (outer.width - client.width + inset_w, outer.height - client.height + inset_h),
        _ => (inset_w, inset_h),
    }
}

/// The app's minimum, no larger than the content of a window filling its
/// display's work area: a machine's mode can be larger than a laptop's
/// screen, and Windows would make a window as large as its minimum.
fn content_min(parts: &WindowParts) -> Option<Size> {
    let min = parts.min_size?;
    let monitor = unsafe { w::MonitorFromWindow(parts.hwnd, w::MONITOR_DEFAULTTONEAREST as u32) };
    let mut info = w::MONITORINFO { cbSize: std::mem::size_of::<w::MONITORINFO>() as u32, ..Default::default() };
    if monitor.is_null() || !unsafe { w::GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Some(min);
    }
    let scale = scale_of(parts);
    let (frame_w, frame_h) = frame_pixels(parts, scale);
    let work = info.rcWork;
    let most_w = ((work.right - work.left - frame_w).max(0) as f64 / scale) as f32;
    let most_h = (((work.bottom - work.top - frame_h).max(0) as f64 / scale) - chrome_height(parts)).max(0.0) as f32;
    Some(Size::new(min.width.min(most_w), min.height.min(most_h)))
}

/// The content's minimum size as the window's: the presenter's preferred
/// minimum is the whole window's, in pixels, so it takes the title bar,
/// the menu bar, the toolbar and the frame. A window already smaller
/// grows to it. Applied again whenever what's above the content changes,
/// and when the app or the content sets a locked height: a minimum and a
/// maximum at the height asked for, so the user resizes only the width.
fn apply_min_size(parts: &WindowParts) {
    let min = content_min(parts);
    if in_full_screen(&parts.app_window) {
        return;
    }
    let presenter = parts
        .app_window
        .cast::<w::IAppWindow>()
        .and_then(|a| a.Presenter())
        .and_then(|p| p.cast::<w::IOverlappedPresenter3>());
    let Ok(presenter) = presenter else { return };
    let locked = parts.height_locked.then(|| {
        let height = parts.requested.or(parts.size.get()).map_or(0.0, |s| s.height);
        height.max(min.map_or(0.0, |m| m.height))
    });
    // Nothing to set, or to take back.
    if min.is_none() && locked.is_none() && presenter.PreferredMaximumHeight().is_err() {
        return;
    }
    let scale = scale_of(parts);
    // Measured before Windows grows the window: the resize that follows
    // uses them (see `resize_client_with`).
    let insets = client_insets(parts, scale);
    let (frame_w, frame_h) = frame_pixels(parts, scale);
    let chrome = chrome_height(parts);
    let outer_height = |height: f32| ((height as f64 + chrome) * scale).round() as i32 + frame_h;
    _ = presenter.SetPreferredMinimumWidth(min.map(|m| (m.width as f64 * scale).round() as i32 + frame_w));
    _ = presenter.SetPreferredMinimumHeight(locked.or(min.map(|m| m.height)).map(outer_height));
    _ = presenter.SetPreferredMaximumHeight(locked.map(outer_height));
    // Windows grows a smaller window to the new minimum itself, before
    // XAML lays it out again.
    if let (Some(min), Some(size)) = (min, parts.size.get())
        && (size.width < min.width || size.height < min.height)
    {
        resize_client_with(parts, size, insets);
    }
}

/// Whether the user can't resize the height, as the presenter has it.
fn height_locked(parts: &WindowParts) -> bool {
    let presenter = parts
        .app_window
        .cast::<w::IAppWindow>()
        .and_then(|a| a.Presenter())
        .and_then(|p| p.cast::<w::IOverlappedPresenter3>());
    presenter.and_then(|p| p.PreferredMaximumHeight()).is_ok()
}

/// Corrects a window once XAML has laid its content out at the size
/// `resize_client` gave it: that aimed with the title bar's height and the
/// resize border as XAML last laid them out, at the window's old size, and
/// they differ by a pixel at the new one. Only small misses: a window that
/// refused the size keeps what it got.
fn correct_client(app_window: &w::AppWindow, host: Option<&w::IUIElement>, want: Size, got: Size) {
    let Ok(app_window) = app_window.cast::<w::IAppWindow2>() else { return };
    let scale = host.and_then(|h| h.XamlRoot().ok()).and_then(|r| r.RasterizationScale().ok()).unwrap_or(1.0);
    let miss = |want: f32, got: f32| ((want - got) as f64 * scale).round() as i32;
    let (width, height) = (miss(want.width, got.width), miss(want.height, got.height));
    if (width, height) == (0, 0) || width.abs() > 2 || height.abs() > 2 {
        return;
    }
    if let Ok(client) = app_window.ClientSize() {
        _ = app_window.ResizeClient(w::SizeInt32 { width: client.width + width, height: client.height + height });
    }
}

/// Brings a window's bar up to date with the menus it shows: in place
/// when only enabled and checked states changed, so an open menu stays
/// open, and rebuilt otherwise.
fn refresh_menu(parts: &mut WindowParts, menus: &Menus) {
    let shown = menus.shown(parts.node, parts.modal.is_some());
    if parts.menu_bar.is_some() && shown.same_structure(&parts.menu_shown) {
        update_menu(parts, &shown);
    } else if shown != parts.menu_shown {
        let activate = menus.activate.clone().unwrap_or_else(|| Rc::new(|_| {}));
        install_menu(parts, &shown, &activate);
    }
    parts.menu_shown = shown;
}

fn update_menu(parts: &WindowParts, menu: &MenuBarData) {
    update_items(&parts.menu_items, menu);
}

/// Shows new enabled and checked states on built items.
fn update_items(items: &MenuItems, menu: &MenuBarData) {
    let mut items = items.borrow_mut();
    for data in menu.items() {
        let Some((item, check)) = items.get_mut(&data.id) else { continue };
        _ = item.cast::<w::IControl>().and_then(|c| c.SetIsEnabled(data.enabled));
        *check = data.check;
        show_check(item, data.check);
    }
}

/// Sets what a toggle or radio item shows. Only `Click` reports a choice,
/// so this never does.
fn show_check(item: &w::MenuFlyoutItemBase, check: MenuCheck) {
    match check {
        MenuCheck::None => {}
        MenuCheck::Check(on) => _ = item.cast::<w::IToggleMenuFlyoutItem>().and_then(|t| t.SetIsChecked(on)),
        MenuCheck::Radio(on) => _ = item.cast::<w::IRadioMenuFlyoutItem>().and_then(|r| r.SetIsChecked(on)),
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
    parts.menu_items.borrow_mut().clear();
    if !menu.menus.is_empty() {
        let mut built = MenuBuild {
            activate,
            revokers: &mut parts.menu_revokers,
            items: &parts.menu_items,
            scope: format!("window-{}", parts.node),
        };
        let menu_bar = ok(built.menu_bar(menu), "building the menu bar");
        let element: w::UIElement = ok(menu_bar.cast(), "menu bar element");
        _ = w::Grid::SetRow(&ok(element.cast::<w::FrameworkElement>(), "menu bar element"), MENU_ROW);
        _ = children.Append(&element);
        parts.menu_bar = Some(menu_bar);
    }
    apply_min_size(parts);
    if let Some(size) = parts.requested {
        resize_client(parts, size);
    }
}

/// `WM_CLOSE`, which the close button and Alt+F4 end in: the app window
/// raises `Closing`, which asks the app.
/// Whether the app runs from a package (MSIX), which has its own id and
/// icon.
fn packaged() -> bool {
    let mut length = 0u32;
    unsafe { w::GetCurrentPackageFullName(&mut length, windows_core::PWSTR::null()) != w::APPMODEL_ERROR_NO_PACKAGE }
}

/// An `.ico` file as it is, or else the image (PNG, which icons may hold)
/// as one icon of its own size.
fn window_icon(icon: &AppIcon) -> Option<WindowIcon> {
    if let AppIcon::File(path) = icon
        && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ico"))
    {
        return Some(WindowIcon::File(path.to_string_lossy().into_owned()));
    }
    let bytes = icon.read()?;
    let icon = unsafe {
        w::CreateIconFromResourceEx(
            bytes.as_ptr(),
            bytes.len() as u32,
            true.into(),
            0x0003_0000,
            0,
            0,
            w::LR_DEFAULTCOLOR as u32,
        )
    };
    (!icon.is_null()).then_some(WindowIcon::Handle(icon))
}

fn set_icon(app_window: &w::AppWindow, icon: &WindowIcon) -> R<()> {
    let app_window = app_window.cast::<w::IAppWindow>()?;
    match icon {
        WindowIcon::File(path) => app_window.SetIcon(path),
        // An `IconId` is the icon's handle, as a `WindowId` is a window's.
        WindowIcon::Handle(icon) => app_window.SetIconWithIconId(w::IconId { value: *icon as u64 }),
    }
}

/// An icon's size in pixels. A monochrome icon's mask holds its image
/// and its mask, one above the other.
fn icon_size(icon: w::HICON) -> Option<(u32, u32)> {
    let mut info = w::ICONINFO::default();
    if !unsafe { w::GetIconInfo(icon, &mut info) }.as_bool() {
        return None;
    }
    let colour = !info.hbmColor.is_null();
    let mut bitmap = w::BITMAP::default();
    let read = unsafe {
        w::GetObjectW(
            if colour { info.hbmColor } else { info.hbmMask },
            size_of::<w::BITMAP>() as i32,
            (&raw mut bitmap).cast(),
        )
    };
    unsafe {
        _ = w::DeleteObject(info.hbmColor);
        _ = w::DeleteObject(info.hbmMask);
    }
    let height = if colour { bitmap.bmHeight } else { bitmap.bmHeight / 2 };
    (read != 0).then_some((bitmap.bmWidth.max(0) as u32, height.max(0) as u32))
}

fn request_close(hwnd: w::HWND) {
    unsafe { _ = w::PostMessageW(hwnd, w::WM_CLOSE as u32, 0, 0) };
}

/// What a Win32 dialog does with Escape (`IDCANCEL`): it asks to close, as
/// the close button does. An accelerator only fires when the focused
/// control didn't use the key, so an open drop-down still closes itself.
fn escape_closes(root: &w::Grid, hwnd: w::HWND) -> R<EventRevoker> {
    let accelerator = w::KeyboardAccelerator::new()?;
    let accel: w::IKeyboardAccelerator = accelerator.cast()?;
    accel.SetKey(w::VirtualKey::Escape)?;
    accel.SetModifiers(w::VirtualKeyModifiers::None)?;
    let revoker = accel.Invoked(move |_, args| {
        if let Some(args) = args.as_ref() {
            _ = args.cast::<w::IKeyboardAcceleratorInvokedEventArgs>().and_then(|a| a.SetHandled(true));
        }
        request_close(hwnd);
    })?;
    root.cast::<w::IUIElement>()?.KeyboardAccelerators()?.Append(&accelerator)?;
    Ok(revoker)
}

/// Builds a window's `MenuBar`, or a context menu's `MenuFlyout`: the
/// same items. Items with a role stay where the app put them: Windows has
/// no standard place of its own for About, Settings or Exit, and no
/// standard titles or shortcuts for them.
struct MenuBuild<'a> {
    activate: &'a Rc<dyn Fn(u32)>,
    revokers: &'a mut Vec<EventRevoker>,
    items: &'a MenuItems,
    /// Where its radio groups' names are unique: XAML's are the thread's,
    /// and ids only the menu's.
    scope: String,
}

impl MenuBuild<'_> {
    fn menu_bar(&mut self, menu: &MenuBarData) -> R<w::MenuBar> {
        let menu_bar = w::MenuBar::new()?;
        let menus = menu_bar.cast::<w::IMenuBar>()?.Items()?;
        for data in &menu.menus {
            let item = w::MenuBarItem::new()?;
            let item_iface: w::IMenuBarItem = item.cast()?;
            item_iface.SetTitle(&data.title)?;
            self.entries(&item_iface.Items()?, data)?;
            menus.Append(&item)?;
        }
        Ok(menu_bar)
    }

    fn flyout(&mut self, entries: &[MenuEntry]) -> R<w::MenuFlyout> {
        let flyout = w::MenuFlyout::new()?;
        let menu = MenuData { title: String::new(), entries: entries.to_vec() };
        self.entries(&flyout.cast::<w::IMenuFlyout>()?.Items()?, &menu)?;
        Ok(flyout)
    }

    fn entries(&mut self, entries: &windows_collections::IVector<w::MenuFlyoutItemBase>, menu: &MenuData) -> R<()> {
        for (entry, group) in menu.entries.iter().zip(menu.radio_groups()) {
            let element = match entry {
                MenuEntry::Item(data) => self.item(data, group)?,
                MenuEntry::Submenu(submenu) => {
                    let sub = w::MenuFlyoutSubItem::new()?;
                    let iface: w::IMenuFlyoutSubItem = sub.cast()?;
                    iface.SetText(&submenu.title)?;
                    self.entries(&iface.Items()?, submenu)?;
                    sub.cast()?
                }
                MenuEntry::Separator => w::MenuFlyoutSeparator::new()?.cast()?,
            };
            entries.Append(&element)?;
        }
        Ok(())
    }

    /// A `MenuFlyoutItem`, or for a check mark a `ToggleMenuFlyoutItem`, or
    /// a `RadioMenuFlyoutItem` grouped with the radio items next to it.
    fn item(&mut self, data: &MenuItemData, group: Option<u32>) -> R<w::MenuFlyoutItemBase> {
        let element: w::MenuFlyoutItemBase = match data.check {
            MenuCheck::None => w::MenuFlyoutItem::new()?.cast()?,
            MenuCheck::Check(_) => w::ToggleMenuFlyoutItem::new()?.cast()?,
            MenuCheck::Radio(_) => {
                let radio = w::RadioMenuFlyoutItem::new()?;
                let name = format!("mitsuami-{}-{}", self.scope, group.unwrap_or(data.id));
                radio.cast::<w::IRadioMenuFlyoutItem>()?.SetGroupName(&name)?;
                radio.cast()?
            }
        };
        show_check(&element, data.check);
        let iface: w::IMenuFlyoutItem = element.cast()?;
        iface.SetText(&data.title)?;
        element.cast::<w::IControl>()?.SetIsEnabled(data.enabled)?;
        if let Some(shortcut) = data.shortcut
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
            element.cast::<w::IUIElement>()?.KeyboardAccelerators()?.Append(&accelerator)?;
        }
        // XAML flips a toggle or radio item as it's clicked. What it shows
        // is the app's state, so put it back: the app's choice comes back
        // from the core.
        let (id, activate, items) = (data.id, self.activate.clone(), Rc::downgrade(self.items));
        self.revokers.push(iface.Click(move |_, _| {
            if let Some(items) = items.upgrade() {
                show_checks(&items);
            }
            activate(id);
        })?);
        self.items.borrow_mut().insert(data.id, (element.clone(), data.check));
        Ok(element)
    }
}

/// What a context menu shows, read back from its items. XAML can't keep
/// ids or roles: those come from what was sent.
fn read_menu(items: &windows_collections::IVector<w::MenuFlyoutItemBase>, menu: &ContextMenu) -> Vec<MenuEntry> {
    let ids: HashMap<usize, u32> = menu.items.borrow().iter().map(|(id, (item, _))| (key(item), *id)).collect();
    let read_item = |item: &w::MenuFlyoutItemBase| -> Option<MenuItemData> {
        let id = *ids.get(&key(item))?;
        let sent = mitsuami_core::services::menu_item_by_id(&menu.sent, id);
        let check = if let Ok(radio) = item.cast::<w::IRadioMenuFlyoutItem>() {
            MenuCheck::Radio(radio.IsChecked().ok()?)
        } else if let Ok(toggle) = item.cast::<w::IToggleMenuFlyoutItem>() {
            MenuCheck::Check(toggle.IsChecked().ok()?)
        } else {
            MenuCheck::None
        };
        let accelerators = item.cast::<w::IUIElement>().ok()?.KeyboardAccelerators().ok()?;
        let shortcut = (accelerators.Size().ok()? > 0)
            .then(|| accelerators.GetAt(0).ok()?.cast::<w::IKeyboardAccelerator>().ok())
            .flatten()
            .and_then(|accel| {
                let key = char::from_u32(accel.Key().ok()?.0 as u32)?;
                let modifiers = accel.Modifiers().ok()?;
                Some(Shortcut {
                    // XAML's keys are upper case: the case sent, if it's this key.
                    key: sent
                        .and_then(|s| s.shortcut)
                        .map(|s| s.key)
                        .filter(|k| k.eq_ignore_ascii_case(&key))
                        .unwrap_or(key.to_ascii_lowercase()),
                    primary: modifiers.contains(w::VirtualKeyModifiers::Control),
                    shift: modifiers.contains(w::VirtualKeyModifiers::Shift),
                    alt: modifiers.contains(w::VirtualKeyModifiers::Menu),
                })
            });
        Some(MenuItemData {
            id,
            title: item.cast::<w::IMenuFlyoutItem>().ok()?.Text().ok()?.to_string(),
            shortcut,
            enabled: item.cast::<w::IControl>().ok()?.IsEnabled().ok()?,
            check,
            role: sent.map(|s| s.role).unwrap_or_default(),
        })
    };
    fn read(
        items: &windows_collections::IVector<w::MenuFlyoutItemBase>,
        read_item: &dyn Fn(&w::MenuFlyoutItemBase) -> Option<MenuItemData>,
    ) -> Vec<MenuEntry> {
        (0..items.Size().unwrap_or(0))
            .filter_map(|i| items.GetAt(i).ok())
            .filter_map(|item| {
                if item.cast::<w::MenuFlyoutSeparator>().is_ok() {
                    Some(MenuEntry::Separator)
                } else if let Ok(sub) = item.cast::<w::IMenuFlyoutSubItem>() {
                    Some(MenuEntry::Submenu(MenuData {
                        title: sub.Text().ok()?.to_string(),
                        entries: read(&sub.Items().ok()?, read_item),
                    }))
                } else {
                    read_item(&item).map(MenuEntry::Item)
                }
            })
            .collect()
    }
    read(items, &read_item)
}

/// Puts back the app's checked states on items XAML flipped.
fn show_checks(items: &MenuItems) {
    for (item, check) in items.borrow().values() {
        show_check(item, *check);
    }
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

    fn create_window(
        &mut self,
        id: NodeId,
        modal: Option<(Option<NodeId>, Modality)>,
    ) -> R<(Widget, w::UIElement, Vec<EventRevoker>)> {
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
        if let Some(icon) = &self.icon {
            _ = set_icon(&app_window, icon);
        }
        let window_id = app_window.cast::<w::IAppWindow>()?.Id()?;
        let hwnd = window_id.value as usize as w::HWND;
        crate::session::watch(hwnd);
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
        let aimed = Rc::new(Cell::new(None::<(Size, Size)>));
        revokers.push(host.cast::<w::IFrameworkElement>()?.SizeChanged({
            let (emitter, last, aimed, app_window) = (emitter.clone(), size.clone(), aimed.clone(), app_window.clone());
            move |sender, args| {
                let Some(new) = args.as_ref().and_then(|a| a.cast::<w::ISizeChangedEventArgs>().ok()?.NewSize().ok())
                else {
                    return;
                };
                let host = sender.as_ref().and_then(|s| s.cast::<w::IUIElement>().ok());
                if let Some(host) = &host {
                    _ = clip_to_size(host, new);
                }
                let new = Size::new(new.width, new.height);
                if let Some((want, before)) = aimed.get()
                    && new != before
                {
                    aimed.set(None);
                    correct_client(&app_window, host.as_ref(), want, new);
                }
                report_size(&emitter, id, &last, new);
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
        // A press on the title bar would reach XAML's root ScrollViewer,
        // which takes focus from the focused control; Windows' own title
        // bars leave focus where it is.
        revokers.push(title_bar.cast::<w::IUIElement>()?.PointerPressed(|_, args| {
            if let Some(args) = args.as_ref() {
                _ = args.cast::<w::IPointerRoutedEventArgs>().and_then(|a| a.SetHandled(true));
            }
        })?);
        // Full screen changed elsewhere (another part of the process): the
        // app hears of it. Our own changes match what it asked for.
        let full_screen = Rc::new(Cell::new(false));
        let monitor = unsafe { w::MonitorFromWindow(hwnd, w::MONITOR_DEFAULTTONEAREST as u32) } as isize;
        let (monitor, moved) = (Rc::new(Cell::new(monitor)), Rc::new(Cell::new(false)));
        revokers.push(app_window.cast::<w::IAppWindow>()?.Changed({
            let (emitter, full_screen, title_bar) = (emitter.clone(), full_screen.clone(), title_bar.clone());
            let (monitor, moved) = (monitor.clone(), moved.clone());
            move |sender, args| {
                let args = args.as_ref().and_then(|a| a.cast::<w::IAppWindowChangedEventArgs>().ok());
                // Onto another display: its minimum is applied again after
                // the next tick, where the backend's state is at hand.
                if args.as_ref().and_then(|a| a.DidPositionChange().ok()) == Some(true) {
                    let now = unsafe { w::MonitorFromWindow(hwnd, w::MONITOR_DEFAULTTONEAREST as u32) } as isize;
                    if monitor.replace(now) != now {
                        moved.set(true);
                        emitter.wake();
                    }
                }
                let changed = args.and_then(|a| a.DidPresenterChange().ok());
                let Some(app_window) = sender.as_ref().filter(|_| changed == Some(true)) else { return };
                let on = in_full_screen(app_window);
                let visibility = if on { w::Visibility::Collapsed } else { w::Visibility::Visible };
                _ = title_bar.cast::<w::IUIElement>().and_then(|e| e.SetVisibility(visibility));
                if full_screen.replace(on) != on {
                    emitter.emit(id, UiEvent::FullScreenChanged(on));
                }
            }
        })?);
        let tab_order = Rc::new(RefCell::new(Vec::new()));
        revokers.push(iwindow.Activated({
            let (emitter, by_element, focus, tab_order, root) =
                (emitter.clone(), self.by_element.clone(), focus.clone(), tab_order.clone(), root.clone());
            move |_, args| {
                let state = args
                    .as_ref()
                    .and_then(|a| a.cast::<w::IWindowActivatedEventArgs>().ok()?.WindowActivationState().ok());
                if state == Some(w::WindowActivationState::Deactivated) {
                    // GPU surfaces' pointer locks and keyboard grabs end
                    // when the window stops being the active one.
                    crate::surface::window_deactivated(hwnd);
                } else {
                    let (emitter, by_element, focus, tab_order, root) =
                        (emitter.clone(), by_element.clone(), focus.clone(), tab_order.clone(), root.clone());
                    let restore: Box<dyn FnOnce()> =
                        Box::new(move || restore_focus(&root, &by_element, &focus, &tab_order.borrow(), &emitter));
                    // After XAML's own restore, which follows `Activated`.
                    let ticket = crate::later::park(restore);
                    if let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() {
                        crate::later::on_ui(&queue, move || {
                            if let Some(restore) = crate::later::take::<Box<dyn FnOnce()>>(ticket) {
                                restore();
                            }
                        });
                    }
                }
            }
        })?);
        revokers.push(root_element.PreviewKeyDown({
            let (emitter, by_element, focus, tab_order, root) =
                (emitter.clone(), self.by_element.clone(), focus.clone(), tab_order.clone(), root.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
                if args.Key().ok() != Some(w::VirtualKey::Tab) {
                    return;
                }
                // A GPU surface that takes input takes Tab; Control+Tab
                // still moves on.
                if crate::surface::takes_tab(focus.get()) && unsafe { w::GetKeyState(w::VK_CONTROL) } >= 0 {
                    return;
                }
                let backwards = unsafe { w::GetKeyState(w::VK_SHIFT) } < 0;
                if let Some(next) =
                    tab(&root, &by_element, focus.get(), &tab_order.borrow(), backwards, w::FocusState::Keyboard)
                {
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
            menu_shown: MenuBarData::default(),
            menu_items: MenuItems::default(),
            requested: None,
            node: id,
            emitter,
            size,
            aimed,
            focus,
            tab_order,
            // Known from its Create, so a dialog never gets the app's bar.
            modal,
            disabled: Vec::new(),
            escape: None,
            toolbar: None,
            toolbar_items: Vec::new(),
            sidebar: None,
            sidebar_revokers: Vec::new(),
            full_screen,
            shown: false,
            overlapped: None,
            min_size: None,
            height_locked: false,
            moved,
        };
        refresh_menu(&mut parts, &self.menus);
        Ok((Widget::Window(Box::new(parts)), host_element, revokers))
    }

    fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) -> R<()> {
        let emitter = self.emitter();
        let shown_text = Rc::new(RefCell::new(String::new()));
        let shown_checked = Rc::new(Cell::new(false));
        let shown_mixed = Rc::new(Cell::new(false));
        let shown_index = Rc::new(Cell::new(-1));
        let shown_number = Rc::new(Cell::new(0.0));
        let offset = Rc::new(Cell::new(Point::ZERO));
        let mut revokers = Vec::new();
        let mut inner = None;
        let (widget, element) = match kind {
            WidgetKind::Window => {
                let modal = match command {
                    Command::Create { props, .. } => props.iter().find_map(|p| match p {
                        Prop::Modal { owner, modality } => Some((*owner, *modality)),
                        _ => None,
                    }),
                    _ => None,
                };
                let (widget, element, window_revokers) = self.create_window(id, modal)?;
                revokers = window_revokers;
                (widget, element)
            }
            WidgetKind::Container | WidgetKind::ToolbarItem => {
                let canvas = w::Canvas::new()?;
                let element = canvas.cast()?;
                (Widget::Host(canvas), element)
            }
            WidgetKind::GpuSurface => {
                let surface = SurfaceHost::new(id, emitter.clone())?;
                let element = surface.canvas.cast()?;
                (Widget::GpuSurface(surface), element)
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
            // No click of its own: a click opens its flyout.
            WidgetKind::MenuButton => {
                let button = w::DropDownButton::new()?;
                let element = button.cast()?;
                (Widget::MenuButton(button), element)
            }
            WidgetKind::Icon => {
                let icon = w::FontIcon::new()?;
                let element = icon.cast()?;
                (Widget::Icon(icon), element)
            }
            WidgetKind::Checkbox => {
                let checkbox = w::CheckBox::new()?;
                let toggle: w::IToggleButton = checkbox.cast()?;
                for checked in [true, false] {
                    let (emitter, shown, mixed) = (emitter.clone(), shown_checked.clone(), shown_mixed.clone());
                    let handler = move |sender: windows_core::Ref<IInspectable>,
                                        _: windows_core::Ref<w::RoutedEventArgs>| {
                        let Some(value) =
                            sender.as_ref().and_then(|s| s.cast::<w::IToggleButton>().ok()?.IsChecked().ok())
                        else {
                            return;
                        };
                        let was_mixed = mixed.replace(false);
                        if shown.replace(value) != value || was_mixed {
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
            WidgetKind::NumberInput => {
                let number = w::NumberBox::new()?;
                let iface: w::INumberBox = number.cast()?;
                // XAML's default hides the spin buttons; `Inline` is its
                // spin box. A tweak can pick `Compact` or `Hidden`.
                iface.SetSpinButtonPlacementMode(w::NumberBoxSpinButtonPlacementMode::Inline)?;
                // Typing commits on Return or leaving the field, the buttons
                // and arrow keys at once; setting the value or the range
                // reports it too, so only a value the core doesn't know
                // about is the user's. NumberBox takes decimals and shows an
                // emptied field as NaN: keep whole numbers, and put back the
                // last one for NaN. It ignores sets from inside its own
                // `ValueChanged`, so those wait until the handler returns;
                // `shown` already holds what they set, so they report
                // nothing.
                let (emitter, shown) = (emitter.clone(), shown_number.clone());
                revokers.push(iface.ValueChanged(move |sender, _| {
                    let Some(number) = sender.as_ref().and_then(|s| s.cast::<w::INumberBox>().ok()) else { return };
                    let Ok(value) = number.Value() else { return };
                    let whole = if value.is_nan() { shown.get() } else { value.round() };
                    if whole.to_bits() != value.to_bits() {
                        set_later(&number, value, whole);
                    }
                    if !value.is_nan() && shown.replace(whole) != whole {
                        emitter.emit(id, UiEvent::Changed(EventValue::Number(whole)));
                    }
                })?);
                let element = number.cast()?;
                (Widget::Number { number, step: None }, element)
            }
            WidgetKind::Progress => {
                let progress = w::ProgressBar::new()?;
                let range: w::IRangeBase = progress.cast()?;
                range.SetMinimum(0.0)?;
                range.SetMaximum(1.0)?;
                let element = progress.cast()?;
                (Widget::Progress(progress), element)
            }
            // Indeterminate by default; inactive, it shows nothing.
            WidgetKind::Spinner => {
                let ring = w::ProgressRing::new()?;
                ring.cast::<w::IProgressRing>()?.SetIsActive(false)?;
                // It has no size until XAML loads it and applies its
                // template: measure it again then.
                revokers.push(ring.cast::<w::IFrameworkElement>()?.Loaded({
                    let emitter = emitter.clone();
                    move |_, _| emitter.emit(id, UiEvent::Remeasure)
                })?);
                let element = ring.cast()?;
                (Widget::Spinner(ring), element)
            }
            // XAML decodes files in the background: once it has, the image
            // has a size, and the core measures it again. A file it can't
            // read shows nothing.
            WidgetKind::Image => {
                let image = w::Image::new()?;
                let (failed, opened) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
                revokers.push(image.ImageOpened({
                    let (emitter, opened) = (emitter.clone(), opened.clone());
                    move |_, _| {
                        opened.set(true);
                        emitter.emit(id, UiEvent::Remeasure);
                    }
                })?);
                revokers.push(image.ImageFailed({
                    let (emitter, failed) = (emitter.clone(), failed.clone());
                    move |_, _| {
                        failed.set(true);
                        emitter.emit(id, UiEvent::Remeasure);
                    }
                })?);
                let element = image.cast()?;
                (Widget::Image { image, source: None, fit: None, bitmap: None, failed, opened }, element)
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
                revokers.push(submit_on_enter(&field.cast()?, &emitter, id)?);
                let element = field.cast()?;
                (Widget::Field(field), element)
            }
            // With XAML's default reveal button, shown while there's text.
            WidgetKind::PasswordInput => {
                let field = w::PasswordBox::new()?;
                // PasswordChanged also fires for programmatic sets: only
                // text the core doesn't know about is a user edit.
                revokers.push(field.cast::<w::IPasswordBox>()?.PasswordChanged({
                    let (emitter, shown) = (emitter.clone(), shown_text.clone());
                    move |sender, _| {
                        let Some(text) =
                            sender.as_ref().and_then(|s| s.cast::<w::IPasswordBox>().ok()?.Password().ok())
                        else {
                            return;
                        };
                        if *shown.borrow() != text {
                            *shown.borrow_mut() = text.clone();
                            emitter.emit(id, UiEvent::Changed(EventValue::Text(text)));
                        }
                    }
                })?);
                revokers.push(submit_on_enter(&field.cast()?, &emitter, id)?);
                let element = field.cast()?;
                (Widget::Password(field), element)
            }
            WidgetKind::ScrollView => {
                let scroll = w::ScrollViewer::new()?;
                let iface: w::IScrollViewer = scroll.cast()?;
                set_scrolling(&iface, ScrollAxes::default(), true)?;
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
            WidgetKind::Sidebar => {
                let sidebar = crate::sidebar::Sidebar::new(id, emitter.clone())?;
                let element = sidebar.view.cast()?;
                (Widget::Sidebar(sidebar), element)
            }
            WidgetKind::Tabs => {
                let tabs = crate::tabs::Tabs::new(id, emitter.clone(), self.tab_bar.clone())?;
                let element = tabs.canvas.cast()?;
                (Widget::Tabs(tabs), element)
            }
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
                shown_mixed,
                shown_index,
                shown_number,
                offset,
                shift_wheel: None,
                scroll_content: false,
                text_style: None,
                text_color: None,
                role: None,
                button_style: None,
                orientation: None,
                mixed: None,
                tweak: None,
                a11y_label: None,
                description: None,
                tooltip: String::new(),
                context_menu: None,
                button_menu: None,
                caption: String::new(),
                icon: String::new(),
                icon_only: None,
                icon_size: false,
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
            (Prop::Step(new), Widget::Slider { step, .. } | Widget::Number { step, .. }) => *step = *new,
            // A dialog doesn't take full screen (its presenter is modal,
            // over its owner), as a sheet can't on macOS.
            (Prop::FullScreen(on), Widget::Window(parts)) => {
                if *on && parts.modal.is_some() {
                    parts.full_screen.set(false);
                    parts.emitter.emit(parts.node, UiEvent::FullScreenChanged(false));
                } else {
                    parts.full_screen.set(*on);
                    apply_full_screen(parts);
                }
            }
            (Prop::MinSize(min), Widget::Window(parts)) => {
                parts.min_size = Some(*min);
                apply_min_size(parts);
            }
            (Prop::HeightFollowsContent(on), Widget::Window(parts)) => {
                parts.height_locked = *on;
                apply_min_size(parts);
            }
            // Acted on when it's shown.
            (Prop::Modal { owner, modality }, Widget::Window(parts)) => {
                let was_modal = parts.modal.replace((*owner, *modality)).is_some();
                if parts.escape.is_none() {
                    parts.escape = Some(escape_closes(&parts.root, parts.hwnd)?);
                }
                if !was_modal {
                    refresh_menu(parts, &self.menus);
                }
            }
            (Prop::Image(new), Widget::Image { image, source, bitmap, failed, opened, .. }) => {
                failed.set(false);
                opened.set(false);
                *bitmap = None;
                match new {
                    ImageSource::Pixels(pixels) => {
                        image.SetSource(&writeable_bitmap(pixels)?.cast::<w::ImageSource>()?)?
                    }
                    ImageSource::File(path) => {
                        let decoded = w::BitmapImage::CreateInstanceWithUriSource(&file_uri(path)?)?;
                        image.SetSource(&decoded.cast::<w::ImageSource>()?)?;
                        *bitmap = Some(decoded);
                    }
                }
                *source = Some(new.clone());
            }
            (Prop::ImageFit(new), Widget::Image { image, fit, .. }) => {
                image.SetStretch(match new {
                    ImageFit::Contain => w::Stretch::Uniform,
                    ImageFit::Stretch => w::Stretch::Fill,
                })?;
                *fit = Some(*new);
            }
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
            // 0 is XAML's "no limit"; trimming puts an ellipsis at the end
            // of the last line shown.
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                let text: w::ITextBlock = l.cast()?;
                text.SetMaxLines(lines.map_or(0, |n| n as i32))?;
                text.SetTextTrimming(if lines.is_some() {
                    w::TextTrimming::CharacterEllipsis
                } else {
                    w::TextTrimming::None
                })?;
            }
            (Prop::Label(t), Widget::Button(_) | Widget::MenuButton(_)) => {
                node.caption = t.clone();
                set_button_content(node)?;
            }
            (Prop::Icon(name), Widget::Button(_) | Widget::MenuButton(_)) => {
                node.icon = name.clone();
                set_button_content(node)?;
            }
            (Prop::IconOnly(only), Widget::Button(_) | Widget::MenuButton(_)) => {
                node.icon_only = Some(*only);
                set_button_content(node)?;
            }
            (Prop::Label(t), Widget::Checkbox(_)) => {
                node.element.cast::<w::IContentControl>()?.SetContent(&boxed(t))?
            }
            (Prop::Icon(name), Widget::Icon(icon)) => icon.cast::<w::IFontIcon>()?.SetGlyph(name)?,
            // A glyph is text: the brushes text takes, the accent's own
            // for text included.
            (Prop::TextColor(color), Widget::Icon(icon)) => {
                node.text_color = Some(*color);
                icon.cast::<w::IFrameworkElement>()?.SetStyle(&foreground_style("FontIcon", *color)?)?;
            }
            (Prop::IconSize(points), Widget::Icon(icon)) => {
                icon.cast::<w::IFontIcon>()?.SetFontSize(f64::from(*points))?;
                node.icon_size = true;
            }
            (
                Prop::Label(t),
                Widget::Switch(_)
                | Widget::Select(_)
                | Widget::Slider { .. }
                | Widget::Number { .. }
                | Widget::Progress(_)
                | Widget::Spinner(_)
                | Widget::Image { .. }
                | Widget::Icon(_)
                | Widget::GpuSurface(_),
            ) => {
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
                // A value the new range clamps isn't the user's, and XAML
                // reports it while the range is set: expect it first.
                node.shown_number.set(range.Value()?.max(*min).min(*max));
                // Widen first, so the value is clamped once, to the new range.
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
            (Prop::Orientation(o), Widget::Slider { slider, .. }) => {
                let orientation = if o.vertical() { w::Orientation::Vertical } else { w::Orientation::Horizontal };
                slider.cast::<w::ISlider>()?.SetOrientation(orientation)?;
                node.orientation = Some(*o);
            }
            (Prop::Number(n), Widget::Slider { slider, .. }) => {
                node.shown_number.set(*n);
                slider.cast::<w::IRangeBase>()?.SetValue(*n)?;
                // XAML clamps what it's given to its range.
                node.shown_number.set(slider.cast::<w::IRangeBase>()?.Value()?);
            }
            (Prop::Range { min, max }, Widget::Number { number, .. }) => {
                let iface: w::INumberBox = number.cast()?;
                // A value the new range clamps isn't the user's, and XAML
                // reports it while the range is set: expect it first.
                node.shown_number.set(iface.Value()?.max(*min).min(*max));
                // Widen first, so the value is clamped once, to the new range.
                if *min < iface.Maximum()? {
                    iface.SetMinimum(*min)?;
                    iface.SetMaximum(*max)?;
                } else {
                    iface.SetMaximum(*max)?;
                    iface.SetMinimum(*min)?;
                }
                // A value the new range clamped isn't the user's; the core
                // sends it next.
                node.shown_number.set(iface.Value()?);
            }
            // What the spin buttons and arrow keys add; 1 unless set.
            (Prop::Step(new), Widget::Number { number, .. }) => {
                number.cast::<w::INumberBox>()?.SetSmallChange(new.unwrap_or(1.0))?
            }
            (Prop::Number(n), Widget::Number { number, .. }) => {
                let iface: w::INumberBox = number.cast()?;
                node.shown_number.set(*n);
                iface.SetValue(*n)?;
                // It clamps what it's given to its range.
                node.shown_number.set(iface.Value()?);
            }
            (Prop::Running(r), Widget::Spinner(ring)) => ring.cast::<w::IProgressRing>()?.SetIsActive(*r)?,
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
            (Prop::Sections(sections), Widget::Sidebar(sidebar)) => sidebar.set_sections(sections.clone())?,
            (Prop::SelectedIndex(index), Widget::Sidebar(sidebar)) => sidebar.set_selected(*index)?,
            (Prop::TabTitles(titles), Widget::Tabs(tabs)) => tabs.set_titles(titles)?,
            (Prop::SelectedIndex(index), Widget::Tabs(tabs)) => tabs.set_selected(*index)?,
            // Its tabs; its pages are the app's.
            (Prop::Enabled(e), Widget::Tabs(tabs)) => tabs.bar.cast::<w::IControl>()?.SetIsEnabled(*e)?,
            (Prop::Value(t), Widget::Field(f)) => {
                let field: w::ITextBox = f.cast()?;
                // Don't disturb the caret when the field already shows it.
                if field.Text()? != *t {
                    *node.shown_text.borrow_mut() = t.clone();
                    field.SetText(t)?;
                }
            }
            (Prop::Placeholder(t), Widget::Field(f)) => f.cast::<w::ITextBox>()?.SetPlaceholderText(t)?,
            // Still focusable and selectable, so its text can be copied.
            (Prop::ReadOnly(r), Widget::Field(f)) => f.cast::<w::ITextBox>()?.SetIsReadOnly(*r)?,
            (Prop::Value(t), Widget::Password(f)) => {
                let field: w::IPasswordBox = f.cast()?;
                if field.Password()? != *t {
                    *node.shown_text.borrow_mut() = t.clone();
                    field.SetPassword(t)?;
                }
            }
            (Prop::Placeholder(t), Widget::Password(f)) => f.cast::<w::IPasswordBox>()?.SetPlaceholderText(t)?,
            (Prop::Checked(c), Widget::Checkbox(b)) => {
                node.shown_checked.set(*c);
                // The mixed state shows over it.
                if !node.shown_mixed.get() {
                    b.cast::<w::IToggleButton>()?.SetIsChecked(Some(*c))?;
                }
            }
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                node.mixed = Some(*m);
                node.shown_mixed.set(*m);
                let checked = node.shown_checked.get();
                b.cast::<w::IToggleButton>()?.SetIsChecked(if *m { None } else { Some(checked) })?;
            }
            (Prop::Checked(c), Widget::Switch(s)) => {
                node.shown_checked.set(*c);
                s.cast::<w::IToggleSwitch>()?.SetIsOn(*c)?;
            }
            (Prop::Enabled(e), _) if is_control(&node.widget) => {
                node.element.cast::<w::IControl>()?.SetIsEnabled(*e)?
            }
            (Prop::TextStyle(text_style), Widget::Label(l)) => {
                node.text_style = Some(*text_style);
                set_label_style(l, node.text_style, node.text_color)?;
                if *text_style == TextStyle::Monospace {
                    l.cast::<w::ITextBlock>()?.SetFontFamily(&w::FontFamily::CreateInstanceWithName(MONOSPACE)?)?;
                }
            }
            (Prop::TextColor(color), Widget::Label(l)) => {
                node.text_color = Some(*color);
                set_label_style(l, node.text_style, node.text_color)?;
            }
            // Local values, so they win over the text style's setters.
            (Prop::FontWeight(weight), Widget::Label(l)) => {
                l.cast::<w::ITextBlock>()?.SetFontWeight(w::FontWeight { weight: weight_value(*weight) })?
            }
            (Prop::Italic(italic), Widget::Label(l)) => l.cast::<w::ITextBlock>()?.SetFontStyle(if *italic {
                w::FontStyle::Italic
            } else {
                w::FontStyle::Normal
            })?,
            (Prop::TextAlign(align), Widget::Label(l)) => l.cast::<w::ITextBlock>()?.SetTextAlignment(match align {
                HorizontalAlign::Left => w::TextAlignment::Left,
                HorizontalAlign::Center => w::TextAlignment::Center,
                HorizontalAlign::Right => w::TextAlignment::Right,
            })?,
            (Prop::TextStyle(text_style), _) if is_control(&node.widget) => {
                let control: w::IControl = node.element.cast()?;
                control.SetFontSize(font_size(*text_style))?;
                control.SetFontWeight(w::FontWeight { weight: font_weight(*text_style) })?;
                if *text_style == TextStyle::Monospace {
                    control.SetFontFamily(&w::FontFamily::CreateInstanceWithName(MONOSPACE)?)?;
                }
                node.text_style = Some(*text_style);
            }
            (Prop::ScrollAxes(axes), Widget::Scroll(s)) => {
                let scroll = s.cast()?;
                set_scrolling(&scroll, *axes, scroll_bars(&scroll)?)?
            }
            (Prop::ScrollBars(show), Widget::Scroll(s)) => {
                let scroll = s.cast()?;
                set_scrolling(&scroll, scroll_axes(&scroll)?, *show)?
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone())?,
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode)?,
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style)?,
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows)?,
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                node.role = Some(*role);
                set_button_style(b, node.role, node.button_style)?;
            }
            (Prop::ButtonStyle(button_style), Widget::Button(b)) => {
                node.button_style = Some(*button_style);
                set_button_style(b, node.role, node.button_style)?;
            }
            (Prop::ButtonStyle(button_style), Widget::MenuButton(b)) => {
                node.button_style = Some(*button_style);
                set_menu_button_style(b, *button_style)?;
            }
            (Prop::TakesInput(on), Widget::GpuSurface(surface)) => surface.set_takes_input(*on)?,
            (Prop::PointerLock(on), Widget::GpuSurface(surface)) => surface.set_pointer_lock(*on),
            (Prop::KeyboardGrab(on), Widget::GpuSurface(surface)) => surface.set_keyboard_grab(*on),
            (Prop::Cursor(cursor), Widget::GpuSurface(surface)) => surface.set_cursor(cursor),
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            (Prop::Tooltip(text), _) => {
                // On the control itself, not the Border a native render sits in.
                let control = node.inner.as_ref().unwrap_or(&node.element);
                let value = (!text.is_empty()).then(|| boxed(text));
                w::ToolTipService::SetToolTip(control, value.as_ref())?;
                node.tooltip = text.clone();
                set_hit_testable(node)?;
                set_help_text(node)?;
            }
            (Prop::ContextMenu(entries), _) => {
                // On the control the pointer is on, as the tooltip. XAML
                // shows it on a right-click, a press and hold, Shift+F10
                // and the Menu key, and `ContextRequested` bubbles: a child
                // without one shows its container's.
                let control: w::IUIElement = node.control().cast()?;
                let events = self.emitter.clone();
                let menu = node.context_menu.get_or_insert_with(|| {
                    ContextMenu::new(
                        Rc::new(move |item| events.emit(id, UiEvent::ContextMenuItem(item))),
                        control.ContextFlyout().ok(),
                    )
                });
                if menu.update(entries, format!("node-{id}"))? {
                    match &menu.flyout {
                        Some(flyout) => control.SetContextFlyout(flyout)?,
                        None => control.SetContextFlyout(menu.own.as_ref())?,
                    }
                }
                set_hit_testable(node)?;
            }
            // The button's own flyout, which XAML opens on a click, Enter,
            // Space or UIA's Expand. Radio groups are named apart from its
            // context menu's.
            (Prop::Menu(entries), Widget::MenuButton(b)) => {
                let events = self.emitter.clone();
                let menu = node.button_menu.get_or_insert_with(|| {
                    ContextMenu::new(Rc::new(move |item| events.emit(id, UiEvent::MenuItem(item))), None)
                });
                if menu.update(entries, format!("button-{id}"))? {
                    let button = b.cast::<w::IButton>()?;
                    match &menu.flyout {
                        Some(flyout) => button.SetFlyout(flyout)?,
                        None => button.SetFlyout(None::<&w::FlyoutBase>)?,
                    }
                }
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

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) -> R<()> {
        let node = &self.nodes[&id];
        match node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            Some(run) => run(&node.element),
            None => Ok(()),
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
                self.run_tweak(*id)?;
            }
            Command::SetProp { id, prop } => {
                self.set_prop(*id, prop, command)?;
                self.run_tweak(*id)?;
            }
            Command::Insert { parent, child, index } => {
                let child_element = self.element(*child, command);
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if let Widget::Sidebar(sidebar) = &self.nodes[child].widget {
                    let view = sidebar.view.clone();
                    let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget) else {
                        violation(command, "a sidebar goes in a window")
                    };
                    if parts.sidebar.is_some() {
                        violation(command, "a window has one sidebar");
                    }
                    // The view takes the host's place, with the host as
                    // its content, and fills it (not our zero frame).
                    let children = parts.root.cast::<w::IPanel>()?.Children()?;
                    let host: w::UIElement = parts.host.cast()?;
                    let mut at = 0;
                    if children.IndexOf(&host, &mut at)? {
                        children.RemoveAt(at)?;
                    }
                    let fe: w::IFrameworkElement = view.cast()?;
                    fe.SetWidth(f64::NAN)?;
                    fe.SetHeight(f64::NAN)?;
                    w::Grid::SetRow(&view.cast::<w::FrameworkElement>()?, CONTENT_ROW)?;
                    view.cast::<w::IContentControl>()?.SetContent(&host)?;
                    children.Append(&view.cast::<w::UIElement>()?)?;
                    parts.sidebar_revokers = crate::sidebar::Sidebar::follow_title_bar(&view, &parts.title_bar)?;
                    parts.sidebar = Some((*child, view));
                    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
                    // The content keeps its size: the window grows by the
                    // pane.
                    if let Some(size) = parts.requested.or(parts.size.get()) {
                        resize_client(parts, size);
                    }
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return Ok(());
                }
                if self.nodes[child].kind == WidgetKind::ToolbarItem {
                    // Items come after the window's content.
                    let content = self
                        .nodes
                        .values()
                        .filter(|n| n.parent == Some(*parent) && n.kind != WidgetKind::ToolbarItem)
                        .count();
                    let Some(index) = index.checked_sub(content) else {
                        violation(command, "toolbar items go after the window's content")
                    };
                    let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget) else {
                        violation(command, "toolbar items go in windows")
                    };
                    insert_toolbar_item(parts, *child, &child_element, index)?;
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return Ok(());
                }
                match &self.nodes.get(parent).map(|n| &n.widget) {
                    Some(Widget::Tabs(tabs)) => {
                        if self.nodes[child].kind != WidgetKind::Container {
                            violation(command, "a Tabs' children are page hosts (Containers)");
                        }
                        tabs.insert(*index, &child_element)?;
                    }
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
                        let wheel = shift_wheel(scroll, &child_element)?;
                        self.nodes.get_mut(parent).unwrap().shift_wheel = Some(wheel);
                        let child_node = self.nodes.get_mut(child).unwrap();
                        child_node.scroll_content = true;
                        set_hit_testable(child_node)?;
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
                if self.nodes[child].kind == WidgetKind::ToolbarItem
                    && let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget)
                {
                    remove_toolbar_item(parts, *child)?;
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return Ok(());
                }
                if self.nodes[child].kind == WidgetKind::Sidebar
                    && let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget)
                {
                    remove_sidebar(parts)?;
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return Ok(());
                }
                match &self.nodes[parent].widget {
                    Widget::Tabs(tabs) => tabs.remove(&child_element)?,
                    Widget::List(list) => list.remove(self.nodes[child].row.expect("inserted with a row")),
                    Widget::Scroll(scroll) => {
                        scroll.cast::<w::IContentControl>()?.SetContent(None::<&IInspectable>)?;
                        self.nodes.get_mut(parent).unwrap().shift_wheel = None;
                        let child_node = self.nodes.get_mut(child).unwrap();
                        child_node.scroll_content = false;
                        set_hit_testable(child_node)?;
                    }
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
                self.menus.windows.remove(id);
                drop(node.revokers);
                // The app's handle may keep its child window: it just stops
                // showing.
                if let Widget::GpuSurface(surface) = &node.widget {
                    surface.detach();
                }
                if let Widget::Window(parts) = node.widget {
                    let WindowParts { window, menu_revokers, modal, disabled, .. } = *parts;
                    drop(menu_revokers);
                    // What it disabled comes back before it closes, so its
                    // owner is the window that becomes active.
                    for other in disabled {
                        unsafe { _ = w::EnableWindow(other, true.into()) };
                    }
                    window.cast::<w::IWindow>()?.Close()?;
                    if let Some(Some(Widget::Window(owner))) =
                        modal.and_then(|(owner, _)| owner).map(|o| self.nodes.get(&o).map(|n| &n.widget))
                    {
                        unsafe { _ = w::SetForegroundWindow(owner.hwnd) };
                    }
                }
            }
            Command::SetFrame { id, frame } => {
                let element = self.element(*id, command);
                // A toolbar item is where the toolbar puts it, at this size;
                // an empty one is collapsed.
                if self.nodes[id].kind == WidgetKind::ToolbarItem
                    && let Some(window) = self.nodes[id].parent
                    && let Some(Widget::Window(parts)) = self.nodes.get_mut(&window).map(|n| &mut n.widget)
                {
                    let fe: w::IFrameworkElement = element.cast()?;
                    fe.SetWidth(frame.width() as f64)?;
                    fe.SetHeight(frame.height() as f64)?;
                    if let Some((_, container)) = parts.toolbar_items.iter().find(|(item, _)| item == id) {
                        let empty = frame.size.is_empty();
                        let visibility = if empty { w::Visibility::Collapsed } else { w::Visibility::Visible };
                        container.cast::<w::IUIElement>()?.SetVisibility(visibility)?;
                    }
                    update_toolbar(parts)?;
                    return Ok(());
                }
                // A page goes where its tab view shows pages, at this size.
                let (x, y) = match self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget) {
                    Some(Widget::Tabs(tabs)) => tabs.page_origin(),
                    _ => (frame.x() as f64, frame.y() as f64),
                };
                w::Canvas::SetLeft(&element, x)?;
                w::Canvas::SetTop(&element, y)?;
                let fe: w::IFrameworkElement = element.cast()?;
                fe.SetWidth(frame.width() as f64)?;
                fe.SetHeight(frame.height() as f64)?;
                if let Widget::List(list) = &self.nodes[id].widget {
                    list.set_width(frame.width());
                }
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
                    Widget::Switch(_)
                        | Widget::Select(_)
                        | Widget::Slider { .. }
                        | Widget::Number { .. }
                        | Widget::Progress(_)
                        | Widget::Spinner(_)
                );
                if !named_by_label || label.is_some() {
                    w::AutomationProperties::SetName(&element, label.as_deref().unwrap_or(""))?;
                }
                let node = self.nodes.get_mut(id).unwrap();
                node.description = description.clone();
                set_help_text(node)?;
                w::AutomationProperties::SetAccessibilityView(
                    &element,
                    if *hidden { w::AccessibilityView::Raw } else { w::AccessibilityView::Content },
                )?;
            }
            Command::SetWindowSize { id, size } => match self.nodes.get_mut(id).map(|n| &mut n.widget) {
                Some(Widget::Window(parts)) => {
                    if !in_full_screen(&parts.app_window) {
                        parts.requested = Some(*size);
                        // A locked height moves first, which Windows may
                        // apply itself: the resize uses the insets from
                        // before (see `apply_min_size`).
                        let insets = client_insets(parts, scale_of(parts));
                        if parts.height_locked {
                            apply_min_size(parts);
                        }
                        resize_client_with(parts, *size, insets);
                    }
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
    /// Brings scroll views' content into XAML's live tree. A new
    /// `ScrollViewer` shows its content only once a layout pass has applied
    /// its template, and XAML measures only elements in the live tree: until
    /// then, controls in it measured as if untemplated (buttons 0 wide).
    /// Scroll views inside scroll views connect one level per pass.
    fn connect_scroll_content(&self) {
        let live = |element: &w::UIElement| {
            element.cast::<w::IUIElement>().and_then(|e| e.XamlRoot()).is_ok_and(|r| !r.as_raw().is_null())
        };
        loop {
            let waiting = self.nodes.values().find_map(|node| {
                let Widget::Scroll(scroll) = &node.widget else { return None };
                let content = scroll.cast::<w::IContentControl>().ok()?.Content().ok()?;
                let content: w::UIElement = content.cast().ok()?;
                (live(&node.element) && !live(&content)).then(|| node.element.clone())
            });
            let Some(scroll) = waiting else { break };
            _ = scroll.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
            let content = scroll.cast::<w::IContentControl>().and_then(|c| c.Content()).and_then(|c| c.cast());
            if !content.is_ok_and(|c: w::UIElement| live(&c)) {
                // Nothing more to do this batch; don't spin.
                break;
            }
        }
    }

    fn layout_lists(&self) {
        for node in self.nodes.values() {
            if let Widget::List(list) = &node.widget {
                list.layout();
            }
        }
    }

    /// The window `id` is in (or is).
    /// Gives GPU surfaces that are now in a window their child windows.
    fn attach_surfaces(&self) {
        for (id, node) in &self.nodes {
            if let Widget::GpuSurface(surface) = &node.widget
                && !surface.is_attached()
                && let Some(parts) = self.window_of(*id)
                && let Err(error) = surface.attach(parts.hwnd)
            {
                panic!("winui backend: a GpuSurface's child window: {error}");
            }
        }
    }

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
            Widget::Password(f) => f.cast::<w::IPasswordBox>().and_then(|f| f.Password()).ok().and_then(|text| {
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
            Widget::Number { number, .. } => number
                .cast::<w::INumberBox>()
                .and_then(|n| n.Value())
                .ok()
                .filter(|v| !v.is_nan())
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
                value.and_then(|v| {
                    let was_mixed = node.shown_mixed.replace(false);
                    (node.shown_checked.replace(v) != v || was_mixed).then_some(EventValue::Bool(v))
                })
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
        let focused = node.focus_target().is_some_and(|e| e.Focus(how).unwrap_or(false));
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

/// Sets a number box to `to` once its `ValueChanged` handler has returned,
/// unless something else has changed it from `from` by then.
fn set_later(number: &w::INumberBox, from: f64, to: f64) {
    let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() else { return };
    let ticket = crate::later::park(number.clone());
    crate::later::on_ui(&queue, move || {
        if let Some(number) = crate::later::take::<w::INumberBox>(ticket)
            && number.Value().is_ok_and(|v| v.to_bits() == from.to_bits())
        {
            let _ = number.SetValue(to);
        }
    });
}

/// Return submits a text or password box; leaving it doesn't.
fn submit_on_enter(field: &w::IUIElement, emitter: &Events, id: NodeId) -> R<EventRevoker> {
    let emitter = emitter.clone();
    field.KeyDown(move |_, args| {
        let enter = args
            .as_ref()
            .and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()?.Key().ok())
            .is_some_and(|k| k == w::VirtualKey::Enter);
        if enter {
            emitter.emit(id, UiEvent::Submit);
        }
    })
}

fn is_control(widget: &Widget) -> bool {
    matches!(
        widget,
        Widget::Field(_)
            | Widget::Password(_)
            | Widget::Button(_)
            | Widget::MenuButton(_)
            | Widget::Checkbox(_)
            | Widget::Switch(_)
            | Widget::Select(_)
            | Widget::Slider { .. }
            | Widget::Number { .. }
            | Widget::Scroll(_)
            | Widget::List(_)
    )
}

/// Scrolling on the given axes, with scroll bars (`Auto`: XAML's, which
/// collapse to thin indicators) or without (`Hidden`: it still scrolls, by
/// wheel, touchpad and touch). `Disabled` is for axes that don't scroll.
fn set_scrolling(scroll: &w::IScrollViewer, axes: ScrollAxes, show: bool) -> R<()> {
    let visible = if show { w::ScrollBarVisibility::Auto } else { w::ScrollBarVisibility::Hidden };
    let hidden = w::ScrollBarVisibility::Disabled;
    let (on, off) = (w::ScrollMode::Enabled, w::ScrollMode::Disabled);
    scroll.SetHorizontalScrollBarVisibility(if axes.horizontal() { visible } else { hidden })?;
    scroll.SetVerticalScrollBarVisibility(if axes.vertical() { visible } else { hidden })?;
    scroll.SetHorizontalScrollMode(if axes.horizontal() { on } else { off })?;
    scroll.SetVerticalScrollMode(if axes.vertical() { on } else { off })
}

fn scroll_axes(scroll: &w::IScrollViewer) -> R<ScrollAxes> {
    let shows = |v: w::ScrollBarVisibility| v != w::ScrollBarVisibility::Disabled;
    Ok(match (shows(scroll.HorizontalScrollBarVisibility()?), shows(scroll.VerticalScrollBarVisibility()?)) {
        (true, true) => ScrollAxes::Both,
        (true, false) => ScrollAxes::Horizontal,
        _ => ScrollAxes::Vertical,
    })
}

/// A Canvas without a background isn't hit-testable, so the pointer would
/// never rest on it, a right-click would pass it by, and so would the
/// wheel: a clear one while it has a tooltip (as drawn views have) or a
/// context menu, or is a ScrollView's content.
fn set_hit_testable(node: &Node) -> R<()> {
    let Widget::Host(canvas) = &node.widget else { return Ok(()) };
    let panel = canvas.cast::<w::IPanel>()?;
    if node.tooltip.is_empty() && !node.scroll_content && !ContextMenu::has_items(&node.context_menu) {
        panel.SetBackground(None::<&w::Brush>)
    } else {
        let clear = w::SolidColorBrush::CreateInstanceWithColor(w::Color { a: 0, r: 0, g: 0, b: 0 })?;
        panel.SetBackground(&clear)
    }
}

/// How far a wheel notch scrolls: XAML's 3 lines of 16 px.
const WHEEL_NOTCH: f64 = 48.0;

/// Shift+wheel scrolls sideways in Windows' own apps (Explorer, Edge) and
/// on the other platforms, but not in XAML's `ScrollViewer`
/// (microsoft-ui-xaml#8553, closed as not planned). The content sees the
/// wheel before the scroll viewer does: scroll it there, as far as a notch
/// scrolls down, if it can scroll sideways.
fn shift_wheel(scroll: &w::ScrollViewer, content: &w::UIElement) -> R<EventRevoker> {
    let scroll: w::IScrollViewer = scroll.cast()?;
    // Where the last notch sent the view, and when: XAML applies a scroll
    // at its next layout, so notches that come before it add up.
    let last: Cell<Option<(f64, Instant)>> = Cell::new(None);
    content.cast::<w::IUIElement>()?.PointerWheelChanged(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        let width = scroll.ScrollableWidth().unwrap_or(0.0);
        if unsafe { w::GetKeyState(w::VK_SHIFT) } >= 0 || width <= 0.0 {
            return;
        }
        let Ok(props) = args.GetCurrentPoint(None).and_then(|p| p.cast::<w::IPointerPoint>()?.Properties()) else {
            return;
        };
        let props: w::IPointerPointProperties = ok(props.cast(), "cast to PointerPointProperties");
        if props.IsHorizontalMouseWheel().unwrap_or(true) {
            return;
        }
        let from = match last.get() {
            Some((x, at)) if at.elapsed() < Duration::from_millis(250) => x,
            _ => scroll.HorizontalOffset().unwrap_or(0.0),
        };
        let delta = props.MouseWheelDelta().unwrap_or(0) as f64 / 120.0;
        let x = (from - delta * WHEEL_NOTCH).clamp(0.0, width);
        last.set(Some((x, Instant::now())));
        _ = scroll.ChangeViewWithOptionalAnimation(Some(x), None, None, true);
        _ = args.SetHandled(true);
    })
}

fn scroll_bars(scroll: &w::IScrollViewer) -> R<bool> {
    let hidden = w::ScrollBarVisibility::Hidden;
    Ok(scroll.HorizontalScrollBarVisibility()? != hidden && scroll.VerticalScrollBarVisibility()? != hidden)
}

/// A window became the active one: focus goes back to the control that had
/// it, or to the first in the Tab order, when XAML's focus isn't on one.
/// XAML focuses the first focusable element when a Page loads with nothing
/// focused, but the content isn't a Page; and it restores focus on
/// activation itself, but can leave it on its root `ScrollViewer`, as after
/// another app's window (NVIDIA's overlay) took activation for a moment.
fn restore_focus(
    root: &w::Grid,
    by_element: &ElementMap,
    focus: &Cell<Option<NodeId>>,
    order: &[NodeId],
    emitter: &Events,
) {
    let focused = root
        .cast::<w::IUIElement>()
        .and_then(|r| r.XamlRoot())
        .and_then(|r| w::FocusManager::GetFocusedElementWithRoot(&r))
        .ok()
        .filter(|e| !e.as_raw().is_null());
    if resolve(by_element, focused).is_some() {
        return;
    }
    let how = w::FocusState::Programmatic;
    let last = focus.get().filter(|id| order.contains(id));
    let now = last
        .and_then(|id| tab(root, by_element, None, &[id], false, how))
        .or_else(|| tab(root, by_element, None, order, false, how));
    report_focus(emitter, focus, now);
}

/// Moves focus along the core's Tab order, skipping controls that can't
/// take focus now, focusing it `how`. Returns the node that took it.
fn tab(
    root: &w::Grid,
    by_element: &ElementMap,
    from: Option<NodeId>,
    order: &[NodeId],
    backwards: bool,
    how: w::FocusState,
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
        // A navigation view and a selector bar take focus on their
        // selected item.
        let element = match (element.cast::<w::NavigationView>(), crate::tabs::Tabs::bar_in(&element)) {
            (Ok(view), _) => match crate::sidebar::Sidebar::focus_target(&view) {
                Some(item) => item,
                None => continue,
            },
            (_, Some(bar)) => match crate::tabs::Tabs::focus_target(&bar) {
                Some(item) => item,
                None => continue,
            },
            _ => element,
        };
        if element.Focus(how).unwrap_or(false) {
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
        // A selector bar over the pages, with no border around them.
        let bar = self.state.borrow().tab_bar.get().unwrap_or(crate::tabs::BAR_HEIGHT);
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
            tab_insets: Insets::new(bar, 0.0, 0.0, 0.0),
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
            Widget::Field(_) | Widget::Password(_) => {
                // Text boxes have no useful intrinsic width.
                let size = ceil(measure_element(&node.element, infinite));
                Size::new(size.width.max(200.0), size.height)
            }
            Widget::Button(_)
            | Widget::MenuButton(_)
            | Widget::Checkbox(_)
            | Widget::Switch(_)
            | Widget::Select(_)
            | Widget::Slider { .. }
            | Widget::Number { .. }
            | Widget::Progress(_)
            | Widget::Spinner(_)
            | Widget::Icon(_) => ceil(measure_element(&node.element, infinite)),
            // Pixels over their scale. A file at its pixel count in
            // effective pixels, as XAML shows it; nothing until it's
            // decoded, or if it can't be.
            Widget::Image { source: Some(ImageSource::Pixels(pixels)), .. } => pixels.size(),
            Widget::Image { bitmap: Some(bitmap), failed, .. } if !failed.get() => {
                let bitmap: w::IBitmapSource = ok(bitmap.cast(), "cast to BitmapSource");
                Size::new(
                    bitmap.PixelWidth().unwrap_or(0).max(0) as f32,
                    bitmap.PixelHeight().unwrap_or(0).max(0) as f32,
                )
            }
            Widget::Image { .. } => Size::ZERO,
            // As large as the layout makes it.
            Widget::GpuSurface(_) => Size::ZERO,
            Widget::Custom { render, props } => render
                .measure(node.control(), props.props(), &request)
                .unwrap_or_else(|| ceil(measure_element(&node.element, infinite))),
            Widget::Native { measure: Some(measure), .. } => measure(node.control(), &request),
            Widget::Native { measure: None, .. } => ceil(measure_element(&node.element, infinite)),
            // Its bar: the core adds the pages.
            Widget::Tabs(tabs) => ceil(tabs.strip()),
            // Measured by the core, or never (the sidebar is the window's).
            Widget::Drawn { .. }
            | Widget::Window(_)
            | Widget::Host(_)
            | Widget::Scroll(_)
            | Widget::List(_)
            | Widget::Sidebar(_) => Size::ZERO,
        };
        Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        match action {
            A11yAction::ContextMenuItem(item) => return self.choose_menu_item(id, *item, false),
            A11yAction::MenuItem(item) => return self.choose_menu_item(id, *item, true),
            _ => {}
        }
        // A list's rows: select or activate them, as clicking or double
        // clicking their container does.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            // A sidebar's item, as a click chooses it: the view reports it
            // to a handler that holds its own data, never our state.
            if let (A11yAction::SetValue(title), Widget::Sidebar(sidebar)) = (action, &node.widget) {
                return match sidebar.choose(title) {
                    Ok(true) => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
            // A tab, as a click picks it; the same way.
            if let (A11yAction::SetValue(title), Widget::Tabs(tabs)) = (action, &node.widget) {
                if tabs.bar.cast::<w::IControl>().and_then(|c| c.IsEnabled()).is_ok_and(|on| !on) {
                    return Err(ActionError::Disabled);
                }
                return match tabs.choose(title) {
                    Ok(true) => Ok(()),
                    _ => Err(ActionError::Unsupported),
                };
            }
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
            // What a screen reader does: NumberBox's RangeValue pattern. Its
            // steps stop at the ends, as its own spin buttons' do.
            (A11yAction::Increment | A11yAction::Decrement | A11yAction::SetValue(_), WidgetKind::NumberInput) => {
                let range: w::IRangeValueProvider = peer()?
                    .GetPattern(w::PatternInterface::RangeValue)
                    .and_then(|p| p.cast())
                    .map_err(|_| ActionError::Unsupported)?;
                let (min, max) = (range.Minimum().unwrap_or(f64::MIN), range.Maximum().unwrap_or(f64::MAX));
                let value = match action {
                    A11yAction::SetValue(text) => {
                        text.trim().parse::<f64>().map_err(|_| ActionError::Unsupported)?.round()
                    }
                    _ => {
                        let step = range.SmallChange().unwrap_or(1.0);
                        let step = if *action == A11yAction::Increment { step } else { -step };
                        range.Value().unwrap_or(0.0) + step
                    }
                };
                range.SetValue(value.clamp(min, max)).map_err(|_| ActionError::Unsupported)?;
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
                if element.cast::<w::ITextBox>().and_then(|f| f.IsReadOnly()).unwrap_or(false) {
                    return Err(ActionError::ReadOnly);
                }
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
            // Password boxes don't let UI Automation set their value.
            (A11yAction::SetValue(text), WidgetKind::PasswordInput) => {
                let field: w::IPasswordBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                *shown_text.borrow_mut() = text.clone();
                field.SetPassword(text).map_err(|_| ActionError::Unsupported)?;
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
        {
            // What XAML's events would report; a click focuses it.
            let state = self.state.borrow();
            if let Some(Widget::GpuSurface(surface)) = state.nodes.get(&id).map(|n| &n.widget) {
                if !surface.takes_input() {
                    return Err(ActionError::Unsupported);
                }
                if let SyntheticInput::Click(_) = input {
                    state.focus(id, w::FocusState::Pointer);
                }
                surface.synthesize(input);
                return Ok(());
            }
        }
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
        if *key == Key::Escape {
            // What the accelerator does, in the node's window if it's a
            // dialog; a plain window ignores Escape. Keys are synthesized
            // by driving controls here, not by sending input.
            let hwnd = {
                let state = self.state.borrow();
                let mut current = Some(id);
                loop {
                    match current.and_then(|c| state.nodes.get(&c)) {
                        Some(Node { widget: Widget::Window(parts), .. }) => {
                            break parts.escape.is_some().then_some(parts.hwnd);
                        }
                        Some(node) => current = node.parent,
                        None => break None,
                    }
                }
            };
            if let Some(hwnd) = hwnd {
                request_close(hwnd);
                runtime::pump();
            }
            return Ok(());
        }
        match (widget_kind, key) {
            (WidgetKind::TextInput, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                let field: w::ITextBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                // It would take the keys and ignore them (our edits go
                // around that); nothing can be typed into it on any platform.
                if field.IsReadOnly().unwrap_or(false) {
                    return Err(ActionError::ReadOnly);
                }
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
            (WidgetKind::PasswordInput, Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab) => {
                let field: w::IPasswordBox = element.cast().map_err(|_| ActionError::Unsupported)?;
                let ui: w::IUIElement = element.cast().map_err(|_| ActionError::Unsupported)?;
                if ui.FocusState().unwrap_or(w::FocusState::Unfocused) == w::FocusState::Unfocused {
                    self.state.borrow().focus(id, w::FocusState::Keyboard);
                }
                // Password boxes have no caret or selection to edit through:
                // edits go to the end, where typing into a focused box puts
                // them, and PasswordChanged reports them.
                let edit = |edit: &dyn Fn(&mut String)| -> windows_core::Result<()> {
                    let mut text = field.Password()?;
                    edit(&mut text);
                    field.SetPassword(&text)
                };
                let result = match key {
                    Key::Char(c) => edit(&|t| t.push(*c)),
                    Key::Backspace => edit(&|t| {
                        t.pop();
                    }),
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
            Widget::Window(parts) => {
                props.push(Prop::Title(parts.window.cast::<w::IWindow>().ok()?.Title().ok()?));
                props.extend(parts.modal.map(|(owner, modality)| Prop::Modal { owner, modality }));
                // Until it's shown, what it will show.
                let full = if parts.shown { in_full_screen(&parts.app_window) } else { parts.full_screen.get() };
                props.push(Prop::FullScreen(full));
                props.extend(parts.min_size.map(Prop::MinSize));
                props.push(Prop::HeightFollowsContent(height_locked(parts)));
            }
            Widget::Label(l) => {
                let text: w::ITextBlock = l.cast().ok()?;
                props.push(Prop::Text(text.Text().ok()?));
                let lines = text.MaxLines().ok()?;
                props.push(Prop::MaxLines((lines > 0).then_some(lines as u32)));
                props.push(Prop::FontWeight(weight_of(text.FontWeight().ok()?.weight)));
                props.push(Prop::Italic(text.FontStyle().ok()? == w::FontStyle::Italic));
                props.push(Prop::TextAlign(match text.TextAlignment().ok()? {
                    w::TextAlignment::Center => HorizontalAlign::Center,
                    w::TextAlignment::Right => HorizontalAlign::Right,
                    _ => HorizontalAlign::Left,
                }));
                // Its brush is a theme resource in its style, which can't
                // be told apart from another once resolved.
                props.extend(node.text_color.map(Prop::TextColor));
            }
            Widget::Field(f) => {
                let field: w::ITextBox = f.cast().ok()?;
                props.push(Prop::Value(field.Text().ok()?));
                props.push(Prop::ReadOnly(field.IsReadOnly().ok()?));
                let placeholder = field.PlaceholderText().ok()?;
                if !placeholder.is_empty() {
                    props.push(Prop::Placeholder(placeholder));
                }
            }
            Widget::Password(f) => {
                let field: w::IPasswordBox = f.cast().ok()?;
                props.push(Prop::Value(field.Password().ok()?));
                let placeholder = field.PlaceholderText().ok()?;
                if !placeholder.is_empty() {
                    props.push(Prop::Placeholder(placeholder));
                }
            }
            Widget::Button(_) => props.extend(button_content(node)),
            Widget::MenuButton(b) => {
                props.extend(button_content(node));
                if let Some(menu) = &node.button_menu {
                    let shown = b.cast::<w::IButton>().ok()?.Flyout().ok();
                    let ours =
                        menu.flyout.as_ref().filter(|flyout| shown.as_ref().is_some_and(|s| key(s) == key(*flyout)));
                    props.push(Prop::Menu(match ours {
                        Some(flyout) => read_menu(&flyout.cast::<w::IMenuFlyout>().ok()?.Items().ok()?, menu),
                        None => Vec::new(),
                    }));
                }
            }
            Widget::Icon(icon) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let icon = icon.cast::<w::IFontIcon>().ok()?;
                props.push(Prop::Icon(icon.Glyph().ok()?));
                if node.icon_size {
                    props.push(Prop::IconSize(icon.FontSize().ok()? as f32));
                }
                // A theme resource in its style, as for text.
                props.extend(node.text_color.map(Prop::TextColor));
            }
            Widget::Checkbox(b) => {
                props.extend(unboxed(node.element.cast::<w::IContentControl>().ok()?.Content()).map(Prop::Label));
                // `IsChecked` is null while mixed.
                let shown = b.cast::<w::IToggleButton>().ok()?.IsChecked().ok();
                props.push(Prop::Checked(shown.unwrap_or(node.shown_checked.get())));
                if node.mixed.is_some() {
                    props.push(Prop::Mixed(shown.is_none()));
                }
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
                if node.orientation.is_some() {
                    let vertical = slider.cast::<w::ISlider>().ok()?.Orientation().ok()? == w::Orientation::Vertical;
                    props.push(Prop::Orientation(if vertical {
                        Orientation::Vertical
                    } else {
                        Orientation::Horizontal
                    }));
                }
            }
            Widget::Number { number, step } => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let iface: w::INumberBox = number.cast().ok()?;
                props.push(Prop::Range { min: iface.Minimum().ok()?, max: iface.Maximum().ok()? });
                props.push(Prop::Step(*step));
                props.push(Prop::Number(iface.Value().ok()?));
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
            Widget::Spinner(ring) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.push(Prop::Running(ring.cast::<w::IProgressRing>().ok()?.IsActive().ok()?));
            }
            Widget::GpuSurface(surface) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.extend(surface.props());
            }
            Widget::Image { source, fit, .. } => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.extend(source.clone().map(Prop::Image));
                props.extend(fit.map(Prop::ImageFit));
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
            Widget::Sidebar(sidebar) => {
                props.push(Prop::Sections(sidebar.sections()));
                props.push(Prop::SelectedIndex(sidebar.selected()));
            }
            Widget::Tabs(tabs) => {
                props.push(Prop::TabTitles(tabs.titles()));
                props.push(Prop::SelectedIndex(tabs.selected()));
                props.push(Prop::Enabled(tabs.bar.cast::<w::IControl>().ok()?.IsEnabled().ok()?));
            }
            Widget::Scroll(s) => {
                let scroll: w::IScrollViewer = s.cast().ok()?;
                props.push(Prop::ScrollAxes(scroll_axes(&scroll).ok()?));
                props.push(Prop::ScrollBars(scroll_bars(&scroll).ok()?));
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
                props.extend(list.style().map(Prop::ListStyle));
                props.push(Prop::Selected(list.selected()));
            }
            Widget::Host(_) => props.extend(node.row.map(Prop::Row)),
        }
        if is_control(&node.widget) && !matches!(node.widget, Widget::Scroll(_) | Widget::List(_)) {
            props.push(Prop::Enabled(node.element.cast::<w::IControl>().ok()?.IsEnabled().ok()?));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        // "" when it has none.
        props.push(Prop::Tooltip(unboxed(w::ToolTipService::GetToolTip(node.control())).unwrap_or_default()));
        if let Some(menu) = &node.context_menu {
            let shown = node.control().cast::<w::IUIElement>().ok()?.ContextFlyout().ok();
            // Ours, or the control's own (or none) while the app's is empty.
            let ours = menu.flyout.as_ref().filter(|flyout| shown.as_ref().is_some_and(|s| key(s) == key(*flyout)));
            props.push(Prop::ContextMenu(match ours {
                Some(flyout) => read_menu(&flyout.cast::<w::IMenuFlyout>().ok()?.Items().ok()?, menu),
                None => Vec::new(),
            }));
        }

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
        // A row is where the list view put it, a toolbar item where the
        // toolbar did.
        let frame = match node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget) {
            Some(Widget::List(list)) => list.row_rect(&node.element, frame),
            Some(Widget::Window(parts)) if node.kind == WidgetKind::ToolbarItem => {
                toolbar_item_frame(parts, id, &node.element).unwrap_or(frame)
            }
            Some(Widget::Window(parts)) if node.kind == WidgetKind::Sidebar => {
                sidebar_frame(parts).unwrap_or(Rect::ZERO)
            }
            // A page its tab view doesn't show is collapsed: nowhere.
            Some(Widget::Tabs(_)) if !crate::tabs::Tabs::shows(&node.element) => Rect::ZERO,
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
            Widget::Window(parts) => {
                let mut children = panel_children(&parts.host, &known);
                children.extend(parts.toolbar_items.iter().map(|(item, _)| *item));
                children.extend(parts.sidebar.as_ref().map(|(sidebar, _)| *sidebar));
                (children, None)
            }
            Widget::Host(canvas) => (panel_children(canvas, &known), None),
            // Its pages; the bar isn't a node.
            Widget::Tabs(tabs) => (panel_children(&tabs.canvas, &known), None),
            _ => (Vec::new(), None),
        };
        // Composite controls give focus to a part (a list's row container,
        // a number box's text box): the control has it when the window's
        // focus tracking (which walks up to it) says so.
        let tracked = !matches!(node.widget, Widget::Window(_))
            && state.window_of(id).is_some_and(|parts| parts.focus.get() == Some(id));
        let focused = tracked
            || !matches!(node.widget, Widget::Window(_))
                && node
                    .control()
                    .cast::<w::IUIElement>()
                    .and_then(|e| e.FocusState())
                    .is_ok_and(|f| f != w::FocusState::Unfocused);
        Some(NativeState { kind: node.kind, props, frame, parent: node.parent, children, focused, scroll_offset })
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
    /// What Narrator does once it has shown a control's context menu: the
    /// item's UIA Invoke (Toggle for a check item), which clicks it as the
    /// pointer does, so the item's `Click` handler reports it. A disabled
    /// control gets no input, so shows no menu.
    /// Chooses an item of the node's context menu, or of a menu button's
    /// menu, without opening it.
    fn choose_menu_item(&self, id: NodeId, item: u32, button: bool) -> Result<(), ActionError> {
        let (element, items, activate) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.control().cast::<w::IControl>().and_then(|c| c.IsEnabled()).is_ok_and(|on| !on) {
                return Err(ActionError::Disabled);
            }
            let menu = if button { &node.button_menu } else { &node.context_menu };
            let menu = menu.as_ref().filter(|menu| menu.flyout.is_some()).ok_or(ActionError::Unsupported)?;
            let element = menu.items.borrow().get(&item).map(|(element, _)| element.clone());
            (element.ok_or(ActionError::Unsupported)?, menu.items.clone(), menu.activate.clone())
        };
        if !element.cast::<w::IControl>().and_then(|c| c.IsEnabled()).unwrap_or(false) {
            return Err(ActionError::Disabled);
        }
        // No state borrow: the item's `Click` handler emits the choice.
        let peer = w::FrameworkElementAutomationPeer::CreatePeerForElement(
            &element.cast::<w::UIElement>().map_err(|_| ActionError::Unsupported)?,
        )
        .map_err(|_| ActionError::Unsupported)?;
        let pattern = |which| peer.GetPattern(which).ok();
        if let Some(invoke) = pattern(w::PatternInterface::Invoke).and_then(|p| p.cast::<w::IInvokeProvider>().ok()) {
            invoke.Invoke().map_err(|_| ActionError::Unsupported)
        } else if let Some(toggle) =
            pattern(w::PatternInterface::Toggle).and_then(|p| p.cast::<w::IToggleProvider>().ok())
        {
            toggle.Toggle().map_err(|_| ActionError::Unsupported)
        } else {
            // An item whose peer has neither (a radio item may not): what
            // its `Click` handler does.
            show_checks(&items);
            activate(item);
            Ok(())
        }
    }

    /// Tab pressed in `id`: move along its window's order.
    fn tab_from(&self, id: NodeId) {
        let state = self.state.borrow();
        let mut window = Some(id);
        while let Some(current) = window {
            let node = &state.nodes[&current];
            if let Widget::Window(parts) = &node.widget {
                let order = parts.tab_order.borrow().clone();
                if let Some(next) =
                    tab(&parts.root, &state.by_element, Some(id), &order, false, w::FocusState::Keyboard)
                {
                    report_focus(&state.emitter, &parts.focus, Some(next));
                }
                return;
            }
            window = node.parent;
        }
    }
}

/// A bitmap of the pixels, as XAML takes them: premultiplied BGRA.
fn writeable_bitmap(pixels: &Pixels) -> R<w::WriteableBitmap> {
    let bitmap = w::WriteableBitmap::CreateInstanceWithDimensions(pixels.width() as i32, pixels.height() as i32)?;
    let buffer = bitmap.PixelBuffer()?;
    let length = buffer.Length()? as usize;
    let access: w::IBufferByteAccess = buffer.cast()?;
    // SAFETY: the buffer holds `length` bytes and outlives the slice.
    let bytes = unsafe { std::slice::from_raw_parts_mut(access.Buffer()?, length) };
    let premultiply = |c: u8, a: u8| ((c as u16 * a as u16 + 127) / 255) as u8;
    for (to, from) in bytes.chunks_exact_mut(4).zip(pixels.rgba().chunks_exact(4)) {
        let [r, g, b, a] = [from[0], from[1], from[2], from[3]];
        to.copy_from_slice(&[premultiply(b, a), premultiply(g, a), premultiply(r, a), a]);
    }
    bitmap.Invalidate()?;
    Ok(bitmap)
}

/// A `file:///` URI for a path, made absolute.
fn file_uri(path: &std::path::Path) -> R<w::Uri> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    w::Uri::CreateUri(&format!("file:///{}", path.display().to_string().replace('\\', "/")))
}
