//! The [`Backend`] implementation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, A11yProps, ActionError};
use mitsuami_core::backend::{
    Appearance, AvailableSpace, Backend, CaptureError, EventSink, FontSizes, Image, Key, MeasureRequest, NativeState,
    PlatformMetrics, SyntheticInput,
};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    ButtonRole, ButtonStyle, Command, CustomProps, EventValue, NodeId, Opaque, Orientation, Point, Prop, Rect, RowKey,
    ScrollAxes, SelectionMode, Size, TextStyle, UiEvent, WidgetKind, find_prop,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, Message, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSBackingStoreType, NSBitmapFormat, NSButton, NSControl, NSControlStateValueMixed,
    NSControlStateValueOff, NSControlStateValueOn, NSEvent, NSEventModifierFlags, NSEventType, NSFont, NSFontTextStyle,
    NSFontTextStyleBody, NSFontTextStyleCallout, NSFontTextStyleCaption1, NSFontTextStyleHeadline,
    NSFontTextStyleLargeTitle, NSFontTextStyleTitle1, NSFontWeightRegular, NSMenuItem, NSPopUpButton,
    NSProgressIndicator, NSProgressIndicatorStyle, NSScreen, NSScrollView, NSSecureTextField, NSSlider,
    NSStandardKeyBindingResponding, NSSwitch, NSTextField, NSView, NSViewBoundsDidChangeNotification, NSWindow,
    NSWindowOrderingMode, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{NSArray, NSDictionary, NSNotificationCenter, NSPoint, NSRange, NSRect, NSSize, NSString};

use crate::classes::{ActionTarget, ClosureTarget, DrawnView, HostView, ViewMap, WindowDelegate};
use crate::custom::{AppKitCx, Emitter, ErasedRender, NativePayload};

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
}

impl Default for BackendOptions {
    fn default() -> Self {
        BackendOptions { show_windows: true, record_commands: false, appearance: None, private_clipboard: false }
    }
}

enum Widget {
    Window {
        window: Retained<NSWindow>,
        host: Retained<HostView>,
        _delegate: Retained<WindowDelegate>,
    },
    Host(Retained<HostView>),
    Label(Retained<NSTextField>),
    Field(Retained<NSTextField>),
    Button(Retained<NSButton>),
    Checkbox(Retained<NSButton>),
    Switch(Retained<NSSwitch>),
    Select(Retained<NSPopUpButton>),
    /// A slider, and the step it was given: AppKit steps by tick marks,
    /// which only approximate one that doesn't divide the range.
    Slider {
        slider: Retained<NSSlider>,
        step: Option<f64>,
    },
    Progress(Retained<NSProgressIndicator>),
    /// A spinning indicator, and whether it's animating: AppKit can't say.
    Spinner {
        indicator: Retained<NSProgressIndicator>,
        running: bool,
    },
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
            Widget::Host(v) => v,
            Widget::Label(v) | Widget::Field(v) => v,
            Widget::Button(v) | Widget::Checkbox(v) => v,
            Widget::Switch(v) => v,
            Widget::Select(v) => v,
            Widget::Slider { slider, .. } => slider,
            Widget::Progress(v) => v,
            Widget::Spinner { indicator, .. } => indicator,
            Widget::Scroll(v) => v,
            Widget::List(list) => &list.scroll,
            Widget::Custom { view, .. } | Widget::Native { view, .. } => view,
            Widget::Drawn { view, .. } => view,
        }
    }

    fn control(&self) -> Option<&NSControl> {
        match self {
            Widget::Label(v) | Widget::Field(v) => Some(v),
            Widget::Button(v) | Widget::Checkbox(v) => Some(v),
            Widget::Switch(v) => Some(v),
            Widget::Select(v) => Some(v),
            Widget::Slider { slider, .. } => Some(slider),
            Widget::Window { .. }
            | Widget::Progress(_)
            | Widget::Spinner { .. }
            | Widget::Host(_)
            | Widget::Scroll(_)
            | Widget::List(_)
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
            widget => widget.view().retain(),
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
    /// Props AppKit can't report back faithfully.
    text_style: Option<TextStyle>,
    role: Option<ButtonRole>,
    button_style: Option<ButtonStyle>,
    /// Sliders: whether the app gave an `Orientation`.
    orientation: Option<Orientation>,
    /// Checkboxes: whether the app gave `Mixed`, and the `Checked` the box
    /// shows when it isn't mixed.
    mixed: Option<bool>,
    checked: bool,
    /// Scroll views: the axes they scroll, and whether they show scroll
    /// bars. Without scrollers, AppKit can't tell which axes scroll.
    scroll_axes: ScrollAxes,
    scroll_bars: bool,
    /// The app's raw settings, run after every other prop.
    tweak: Option<Opaque>,
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
}

pub struct AppKitBackend {
    state: Rc<RefCell<State>>,
}

/// Shared access to an [`AppKitBackend`] after it was moved into a `Ui`.
#[derive(Clone)]
pub struct AppKitHandle {
    state: Rc<RefCell<State>>,
}

fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn key(view: &NSView) -> usize {
    view as *const NSView as usize
}

fn font(style: TextStyle) -> Retained<NSFont> {
    let text_style: &NSFontTextStyle = unsafe {
        match style {
            TextStyle::LargeTitle => NSFontTextStyleLargeTitle,
            TextStyle::Title => NSFontTextStyleTitle1,
            TextStyle::Headline => NSFontTextStyleHeadline,
            TextStyle::Body => NSFontTextStyleBody,
            TextStyle::Callout => NSFontTextStyleCallout,
            TextStyle::Caption => NSFontTextStyleCaption1,
            TextStyle::Monospace => {
                let size = font(TextStyle::Body).pointSize();
                return NSFont::monospacedSystemFontOfSize_weight(size, NSFontWeightRegular);
            }
        }
    };
    unsafe { NSFont::preferredFontForTextStyle_options(text_style, &NSDictionary::new()) }
}

fn metrics(mtm: MainThreadMarker, forced: Option<Appearance>) -> PlatformMetrics {
    let size = |style| font(style).pointSize() as f32;
    let dark = match forced {
        Some(appearance) => appearance == Appearance::Dark,
        None => {
            let appearance = NSApplication::sharedApplication(mtm).effectiveAppearance();
            let candidates = unsafe { [NSAppearanceNameAqua, NSAppearanceNameDarkAqua] };
            let names = NSArray::from_slice(&candidates);
            appearance
                .bestMatchFromAppearancesWithNames(&names)
                .is_some_and(|name| &*name == unsafe { NSAppearanceNameDarkAqua })
        }
    };
    let workspace = NSWorkspace::sharedWorkspace();
    PlatformMetrics {
        scale_factor: NSScreen::mainScreen(mtm).map_or(1.0, |s| s.backingScaleFactor() as f32),
        // The macOS HIG favours tight spacing; 20pt is the standard window margin.
        spacing: SpacingScale { xs: 4.0, sm: 6.0, md: 8.0, lg: 12.0, xl: 20.0 },
        font_sizes: FontSizes {
            large_title: size(TextStyle::LargeTitle),
            title: size(TextStyle::Title),
            headline: size(TextStyle::Headline),
            body: size(TextStyle::Body),
            callout: size(TextStyle::Callout),
            caption: size(TextStyle::Caption),
            monospace: size(TextStyle::Monospace),
        },
        dark_mode: dark,
        high_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
        reduced_motion: workspace.accessibilityDisplayShouldReduceMotion(),
    }
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
            })),
        }
    }

    pub fn handle(&self) -> AppKitHandle {
        AppKitHandle { state: self.state.clone() }
    }
}

impl AppKitHandle {
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

    /// Resizes a window's content like the user would; the window delegate
    /// reports it back as a `WindowResized` event.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let window = self.ns_window(window);
        if let Some(window) = window {
            window.setContentSize(NSSize::new(size.width as f64, size.height as f64));
        }
    }

    /// Orders front windows whose first layout has been applied.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            if let Some(window) = self.ns_window(id) {
                window.center();
                window.makeKeyAndOrderFront(None);
            }
        }
    }

    /// Escape hatch: the native window of a window node.
    pub fn ns_window(&self, id: NodeId) -> Option<Retained<NSWindow>> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window { window, .. } => Some(window.clone()),
            _ => None,
        }
    }

    /// Escape hatch: the native view of any node (a window's content view).
    pub fn ns_view(&self, id: NodeId) -> Option<Retained<NSView>> {
        let state = self.state.borrow();
        let view = state.nodes.get(&id)?.widget.view();
        Some(view.retain())
    }
}

impl mitsuami_core::TestHooks for AppKitHandle {
    fn name(&self) -> &'static str {
        "appkit"
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        AppKitHandle::resize_window(self, window, size);
    }

    fn take_command_log(&self) -> Vec<Command> {
        AppKitHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        AppKitHandle::node_count(self)
    }

    /// Offscreen windows get no display cycle, where tables add the rows
    /// scrolling brought into view.
    fn settle(&self) {
        self.state.borrow().layout_lists();
    }
}

impl State {
    /// Lets tables lay out their rows now rather than at the next display:
    /// they report the rows they show, and the core builds those in the
    /// same run-loop turn, before anything is drawn. Their callbacks only
    /// emit.
    fn layout_lists(&self) {
        for node in self.nodes.values() {
            if let Widget::List(list) = &node.widget {
                // Scrolling doesn't mark the table as needing layout; its
                // rows follow at the next display.
                list.table.setNeedsLayout(true);
                list.scroll.layoutSubtreeIfNeeded();
            }
        }
    }

    fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) {
        let mtm = self.mtm;
        let target = matches!(
            kind,
            WidgetKind::Button
                | WidgetKind::Checkbox
                | WidgetKind::Switch
                | WidgetKind::Select
                | WidgetKind::Slider
                | WidgetKind::TextInput
                | WidgetKind::PasswordInput
                | WidgetKind::ScrollView
                | WidgetKind::List
        )
        .then(|| ActionTarget::new(mtm, id, kind, self.events.clone()));
        let action = Some(sel!(fire:));
        let mut targets = Vec::new();
        let target_obj: Option<&AnyObject> = target.as_deref().map(|t| t.as_ref());
        let widget = match kind {
            WidgetKind::Window => {
                let style = NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable;
                let window = unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(
                        NSWindow::alloc(mtm),
                        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 300.0)),
                        style,
                        NSBackingStoreType::Buffered,
                        false,
                    )
                };
                unsafe { window.setReleasedWhenClosed(false) };
                // The core owns the Tab order (reading order, not geometry)
                // and sends it with SetFocusOrder.
                window.setAutorecalculatesKeyViewLoop(false);
                let host = HostView::new(mtm, true);
                window.setContentView(Some(&host));
                let delegate = WindowDelegate::new(mtm, id, self.events.clone(), self.by_view.clone());
                window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
                delegate.observe_focus(&window);
                if let Some(appearance) = self.options.appearance {
                    let name = match appearance {
                        Appearance::Light => unsafe { NSAppearanceNameAqua },
                        Appearance::Dark => unsafe { NSAppearanceNameDarkAqua },
                    };
                    window.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
                }
                if self.options.show_windows {
                    self.pending_show.push(id);
                }
                Widget::Window { window, host, _delegate: delegate }
            }
            WidgetKind::Container => Widget::Host(HostView::new(mtm, false)),
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not an AppKit one")
                        };
                        let mut cx = AppKitCx::new(mtm, id, self.events.clone(), &mut targets);
                        let view = render.create(custom.props(), &mut cx);
                        Widget::Custom { view, render, props: custom }
                    }
                    None => Widget::Drawn { view: DrawnView::new(mtm, id, self.events.clone()), props: custom },
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not an AppKit one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let measure = payload.spec.measure.clone();
                let mut cx = AppKitCx::new(mtm, id, self.events.clone(), &mut targets);
                let view = create(&mut cx);
                payload.apply(&view);
                Widget::Native { view, measure, last: opaque }
            }
            WidgetKind::Text => {
                let label = NSTextField::wrappingLabelWithString(&ns(""), mtm);
                label.setSelectable(false);
                Widget::Label(label)
            }
            WidgetKind::Button => {
                Widget::Button(unsafe { NSButton::buttonWithTitle_target_action(&ns(""), target_obj, action, mtm) })
            }
            WidgetKind::Checkbox => {
                Widget::Checkbox(unsafe { NSButton::checkboxWithTitle_target_action(&ns(""), target_obj, action, mtm) })
            }
            WidgetKind::Switch => {
                let switch = NSSwitch::new(mtm);
                unsafe {
                    switch.setTarget(target_obj);
                    switch.setAction(action);
                }
                Widget::Switch(switch)
            }
            WidgetKind::Select => {
                let popup = NSPopUpButton::initWithFrame_pullsDown(
                    NSPopUpButton::alloc(mtm),
                    crate::classes::zero_rect(),
                    false,
                );
                unsafe {
                    popup.setTarget(target_obj);
                    popup.setAction(action);
                }
                Widget::Select(popup)
            }
            WidgetKind::Slider => {
                let slider = NSSlider::new(mtm);
                unsafe {
                    slider.setTarget(target_obj);
                    slider.setAction(action);
                }
                Widget::Slider { slider, step: None }
            }
            WidgetKind::Progress => {
                let progress = NSProgressIndicator::new(mtm);
                progress.setMinValue(0.0);
                progress.setMaxValue(1.0);
                Widget::Progress(progress)
            }
            WidgetKind::Spinner => {
                let indicator = NSProgressIndicator::new(mtm);
                indicator.setStyle(NSProgressIndicatorStyle::Spinning);
                indicator.setIndeterminate(true);
                // Stopped, it shows nothing, as the other platforms' do.
                indicator.setDisplayedWhenStopped(false);
                Widget::Spinner { indicator, running: false }
            }
            WidgetKind::TextInput => {
                // No target-action: submit comes from the delegate (Return
                // only), edits from `controlTextDidChange:`.
                let field = NSTextField::textFieldWithString(&ns(""), mtm);
                if let Some(target) = &target {
                    // SAFETY: the node keeps the target alive as long as the field.
                    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&**target))) };
                }
                Widget::Field(field)
            }
            WidgetKind::PasswordInput => {
                // A text field whose cell and field editor hide the text;
                // the rest is a text field's, delegate included.
                // `textFieldWithString:` is inherited: called on the secure
                // class, it makes a secure field set up as a text field.
                // SAFETY: the class method returns an autoreleased instance
                // of the class it's called on.
                let field: Retained<NSSecureTextField> = unsafe {
                    msg_send![<NSSecureTextField as objc2::ClassType>::class(), textFieldWithString: &*ns("")]
                };
                let field = field.into_super();
                if let Some(target) = &target {
                    // SAFETY: the node keeps the target alive as long as the field.
                    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&**target))) };
                }
                Widget::Field(field)
            }
            WidgetKind::ScrollView => {
                let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), crate::classes::zero_rect());
                scroll.setDrawsBackground(false);
                scroll.setAutohidesScrollers(true);
                // The core places the content; AppKit mustn't inset it for
                // the title bar on top of that.
                scroll.setAutomaticallyAdjustsContentInsets(false);
                observe_scrolling(&scroll, target.as_deref());
                Widget::Scroll(scroll)
            }
            WidgetKind::List => {
                let list = crate::list::List::new(mtm, id, self.events.clone());
                observe_scrolling(&list.scroll, target.as_deref());
                Widget::List(list)
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
        };
        // The core assumes new nodes start with a zero frame and only sends
        // frames that differ; AppKit controls come with their own.
        if !matches!(widget, Widget::Window { .. }) {
            widget.view().setFrame(crate::classes::zero_rect());
        }
        self.by_view.borrow_mut().insert(key(widget.view()), id);
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                _target: target,
                _targets: targets,
                parent: None,
                row: None,
                text_style: None,
                role: None,
                button_style: None,
                orientation: None,
                scroll_axes: ScrollAxes::default(),
                scroll_bars: true,
                mixed: None,
                checked: false,
                tweak: None,
            },
        );
    }

    fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) {
        let mtm = self.mtm;
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window { window, .. }) => window.setTitle(&ns(t)),
            (Prop::Text(t), Widget::Label(l)) => l.setStringValue(&ns(t)),
            // 0 is AppKit's "no limit"; the cell puts an ellipsis at the
            // end of the last line it shows.
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                l.setMaximumNumberOfLines(lines.map_or(0, |n| n as isize));
                if let Some(cell) = l.cell() {
                    cell.setTruncatesLastVisibleLine(lines.is_some());
                }
            }
            (Prop::Label(t), Widget::Button(b) | Widget::Checkbox(b)) => b.setTitle(&ns(t)),
            (Prop::Label(t), Widget::Switch(s)) => s.setAccessibilityLabel(Some(&ns(t))),
            (Prop::Label(t), Widget::Select(p)) => p.setAccessibilityLabel(Some(&ns(t))),
            (Prop::Options(options), Widget::Select(p)) => {
                // Items go straight into the menu: `addItemWithTitle:`
                // drops earlier items with the same title. The chosen index
                // stays if it can, else the first option is chosen, as the
                // core does; it sends the index when that changes it.
                let chosen = p.indexOfSelectedItem();
                p.removeAllItems();
                if let Some(menu) = p.menu() {
                    for option in options {
                        let item = unsafe {
                            NSMenuItem::initWithTitle_action_keyEquivalent(
                                NSMenuItem::alloc(mtm),
                                &ns(option),
                                None,
                                &ns(""),
                            )
                        };
                        menu.addItem(&item);
                    }
                }
                let count = p.numberOfItems();
                p.selectItemAtIndex(if (0..count).contains(&chosen) {
                    chosen
                } else if count > 0 {
                    0
                } else {
                    -1
                });
            }
            (Prop::SelectedIndex(index), Widget::Select(p)) => {
                p.selectItemAtIndex(index.map_or(-1, |i| i as isize));
            }
            (Prop::Label(t), Widget::Slider { slider, .. }) => slider.setAccessibilityLabel(Some(&ns(t))),
            (Prop::Range { min, max }, Widget::Slider { slider, step }) => {
                slider.setMinValue(*min);
                slider.setMaxValue(*max);
                set_ticks(slider, *step);
            }
            (Prop::Step(new), Widget::Slider { slider, step }) => {
                *step = *new;
                set_ticks(slider, *new);
            }
            (Prop::Number(n), Widget::Slider { slider, .. }) => slider.setDoubleValue(*n),
            (Prop::Orientation(o), Widget::Slider { slider, .. }) => {
                slider.setVertical(o.vertical());
                node.orientation = Some(*o);
            }
            (Prop::Label(t), Widget::Progress(p) | Widget::Spinner { indicator: p, .. }) => {
                p.setAccessibilityLabel(Some(&ns(t)))
            }
            (Prop::Running(r), Widget::Spinner { indicator, running }) => {
                if *r {
                    unsafe { indicator.startAnimation(None) };
                } else {
                    unsafe { indicator.stopAnimation(None) };
                }
                *running = *r;
            }
            (Prop::Progress(progress), Widget::Progress(p)) => match progress {
                Some(fraction) => {
                    if p.isIndeterminate() {
                        unsafe { p.stopAnimation(None) };
                        p.setIndeterminate(false);
                        // Once animated, macOS 26's bar layer keeps drawing
                        // the indeterminate animation after it stops, whatever
                        // the value; setting the style again rebuilds it.
                        p.setStyle(NSProgressIndicatorStyle::Spinning);
                        p.setStyle(NSProgressIndicatorStyle::Bar);
                    }
                    p.setDoubleValue(*fraction);
                }
                None => {
                    p.setIndeterminate(true);
                    unsafe { p.startAnimation(None) };
                }
            },
            (Prop::Value(t), Widget::Field(f)) => {
                // Don't disturb the caret when the field already shows it.
                if f.stringValue().to_string() != *t {
                    f.setStringValue(&ns(t));
                }
            }
            (Prop::Placeholder(t), Widget::Field(f)) => f.setPlaceholderString(Some(&ns(t))),
            // Still selectable, so its text can be copied; it takes focus
            // from a click, and from the keyboard only with Full Keyboard
            // Access.
            (Prop::ReadOnly(r), Widget::Field(f)) => f.setEditable(!r),
            (Prop::Checked(c), Widget::Checkbox(b)) => {
                node.checked = *c;
                // The mixed state shows over it.
                if b.state() != NSControlStateValueMixed {
                    b.setState(if *c { NSControlStateValueOn } else { NSControlStateValueOff })
                }
            }
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                node.mixed = Some(*m);
                // Allowed only while shown: clicks would cycle through it.
                b.setAllowsMixedState(*m);
                b.setState(match (*m, node.checked) {
                    (true, _) => NSControlStateValueMixed,
                    (false, true) => NSControlStateValueOn,
                    (false, false) => NSControlStateValueOff,
                });
            }
            (Prop::Checked(c), Widget::Switch(s)) => {
                s.setState(if *c { NSControlStateValueOn } else { NSControlStateValueOff })
            }
            (Prop::Enabled(e), w) if w.control().is_some() => w.control().unwrap().setEnabled(*e),
            (Prop::TextStyle(style), w) if w.control().is_some() => {
                w.control().unwrap().setFont(Some(&font(*style)));
                node.text_style = Some(*style);
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone()),
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode),
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style),
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::ScrollAxes(axes), Widget::Scroll(scroll)) => {
                node.scroll_axes = *axes;
                set_scrollers(scroll, node.scroll_axes, node.scroll_bars);
            }
            // A scroll view without scrollers still scrolls by wheel and
            // trackpad.
            (Prop::ScrollBars(show), Widget::Scroll(scroll)) => {
                node.scroll_bars = *show;
                set_scrollers(scroll, node.scroll_axes, node.scroll_bars);
            }
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                // Return clicks the default button, Escape the cancel button.
                b.setKeyEquivalent(&ns(match role {
                    ButtonRole::Default => "\r",
                    ButtonRole::Cancel => "\u{1b}",
                    ButtonRole::Normal | ButtonRole::Destructive => "",
                }));
                b.setHasDestructiveAction(*role == ButtonRole::Destructive);
                node.role = Some(*role);
            }
            (Prop::ButtonStyle(style), Widget::Button(b)) => {
                b.setBordered(*style != ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            (Prop::Custom(new), Widget::Custom { view, render, props }) => {
                if props != new {
                    render.update(view, props.props(), new.props());
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(drawing), Widget::Drawn { view, .. }) => view.set_drawing(drawing.clone()),
            (Prop::Native(opaque), Widget::Native { view, last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    payload.apply(view);
                }
                *last = opaque.clone();
            }
            _ => {}
        }
    }

    fn view(&self, id: NodeId, command: &Command) -> Retained<NSView> {
        match self.nodes.get(&id) {
            Some(node) => node.widget.view().retain(),
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) {
        let node = &self.nodes[&id];
        if let Some(run) = node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            match &node.widget {
                // The table, not the scroll view around it.
                Widget::List(list) => run(&list.table),
                widget => run(widget.view()),
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
                let parent_view = self.view(*parent, command);
                let child_view = self.view(*child, command);
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if let Widget::Scroll(scroll) = &self.nodes[parent].widget {
                    if scroll.documentView().is_some() {
                        violation(command, "a ScrollView has a single native child (its content)");
                    }
                    scroll.setDocumentView(Some(&child_view));
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if let Widget::List(list) = &self.nodes[parent].widget {
                    let Some(row) = self.nodes[child].row else {
                        violation(command, "a List's children are row hosts (Containers with a Prop::Row)")
                    };
                    list.insert(row, *child, child_view);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                let siblings = parent_view.subviews();
                if *index >= siblings.len() {
                    parent_view.addSubview(&child_view);
                } else {
                    let before = siblings.objectAtIndex(*index);
                    parent_view.addSubview_positioned_relativeTo(
                        &child_view,
                        NSWindowOrderingMode::Below,
                        Some(&before),
                    );
                }
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                match &self.nodes[parent].widget {
                    Widget::Scroll(scroll) => scroll.setDocumentView(None),
                    Widget::List(list) => list.remove(self.nodes[child].row.expect("inserted with a row")),
                    _ => self.view(*child, command).removeFromSuperview(),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                self.by_view.borrow_mut().remove(&key(node.widget.view()));
                self.pending_show.retain(|w| w != id);
                self.focus_orders.remove(id);
                if let Some(target) = &node._target {
                    unsafe { NSNotificationCenter::defaultCenter().removeObserver(target) };
                }
                match &node.widget {
                    Widget::Window { window, _delegate, .. } => {
                        _delegate.stop_observing_focus(window);
                        window.setDelegate(None);
                        window.close();
                    }
                    Widget::List(list) => {
                        list.detach();
                        list.scroll.removeFromSuperview();
                    }
                    widget => widget.view().removeFromSuperview(),
                }
            }
            Command::SetFrame { id, frame } => {
                let rect = NSRect::new(
                    NSPoint::new(frame.x() as f64, frame.y() as f64),
                    NSSize::new(frame.width() as f64, frame.height() as f64),
                );
                if let Some(Widget::List(list)) = self.nodes.get(id).map(|n| &n.widget) {
                    list.set_frame(rect);
                    return;
                }
                // A row fills its cell, and the table makes the row as high.
                let parent = self.nodes.get(id).and_then(|n| n.parent).map(|p| &self.nodes[&p].widget);
                if let Some(Widget::List(list)) = parent {
                    self.view(*id, command).setFrame(rect);
                    if let Some(row) = self.nodes[id].row {
                        list.set_row_height(row, frame.height());
                    }
                    return;
                }
                let view = self.view(*id, command);
                // Layout places what the user sees, the alignment rect, as
                // Auto Layout does; controls draw their bezels inset from
                // their frames (a push button by 7pt a side before macOS 26).
                view.setFrame(view.frameForAlignmentRect(NSRect::new(
                    NSPoint::new(frame.x() as f64, frame.y() as f64),
                    NSSize::new(frame.width() as f64, frame.height() as f64),
                )));
            }
            Command::SetA11y { id, a11y } => {
                let view = self.view(*id, command);
                let A11yProps { label, description, hidden, .. } = a11y;
                view.setAccessibilityLabel(label.as_deref().map(ns).as_deref());
                view.setAccessibilityHelp(description.as_deref().map(ns).as_deref());
                if *hidden {
                    view.setAccessibilityElement(false);
                }
            }
            Command::SetWindowSize { id, size } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Window { window, .. }) => {
                    window.setContentSize(NSSize::new(size.width as f64, size.height as f64))
                }
                _ => violation(command, "not a window"),
            },
            Command::SetFocusOrder { window, order } => {
                let ns_window = match self.nodes.get(window).map(|n| &n.widget) {
                    Some(Widget::Window { window, .. }) => window.clone(),
                    _ => violation(command, "not a window"),
                };
                // Unlink the previous chain, then link the new one as a loop.
                // AppKit still skips views that can't take focus right now
                // (disabled, or not reachable under the user's keyboard
                // navigation setting).
                for old in self.focus_orders.remove(window).unwrap_or_default() {
                    if let Some(node) = self.nodes.get(&old) {
                        unsafe { node.widget.key_view().setNextKeyView(None) };
                    }
                }
                let views: Vec<Retained<NSView>> = order
                    .iter()
                    .map(|id| match self.nodes.get(id) {
                        Some(node) => node.widget.key_view(),
                        None => violation(command, &format!("node {id} does not exist")),
                    })
                    .collect();
                for (i, view) in views.iter().enumerate() {
                    let next = &views[(i + 1) % views.len()];
                    unsafe { view.setNextKeyView(Some(next)) };
                }
                ns_window.setInitialFirstResponder(views.first().map(|v| &**v));
                self.focus_orders.insert(*window, order.clone());
            }
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll(scroll)) => scroll_to(scroll, NSPoint::new(offset.x as f64, offset.y as f64)),
                Some(Widget::List(list)) => scroll_to(&list.scroll, NSPoint::new(offset.x as f64, offset.y as f64)),
                _ => violation(command, "not a ScrollView or List"),
            },
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row),
                _ => violation(command, "not a List"),
            },
            Command::Focus { id } => {
                let Some(node) = self.nodes.get(id) else { violation(command, "node does not exist") };
                let view = node.widget.key_view();
                if let Some(window) = view.window() {
                    window.makeFirstResponder(Some(&view));
                }
            }
        }
    }
}

/// Reports a scroll view's clip view moving, through the node's target.
fn observe_scrolling(scroll: &NSScrollView, target: Option<&ActionTarget>) {
    let clip = scroll.contentView();
    clip.setPostsBoundsChangedNotifications(true);
    if let Some(target) = target {
        // SAFETY: the target is removed as an observer when the node is destroyed.
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                target,
                sel!(scrolled:),
                Some(NSViewBoundsDidChangeNotification),
                Some(&clip),
            );
        }
    }
}

/// Scrolls like the user would, so the clip view reports the change.
fn scroll_to(scroll: &NSScrollView, origin: NSPoint) {
    let clip = scroll.contentView();
    clip.scrollToPoint(origin);
    scroll.reflectScrolledClipView(&clip);
}

/// Scrollers for the axes that scroll, if the scroll view shows any.
fn set_scrollers(scroll: &NSScrollView, axes: ScrollAxes, show: bool) {
    scroll.setHasVerticalScroller(show && axes.vertical());
    scroll.setHasHorizontalScroller(show && axes.horizontal());
}

fn focused(widget: &Widget) -> bool {
    let view = widget.key_view();
    let Some(responder) = view.window().and_then(|w| w.firstResponder()) else { return false };
    match widget {
        // While editing, the window's field editor is first responder.
        Widget::Field(field) => field.currentEditor().is_some(),
        _ => std::ptr::eq(&*responder as *const _ as *const NSView, &*view as *const NSView),
    }
}

fn ceil_size(size: NSSize) -> Size {
    Size::new(size.width.ceil() as f32, size.height.ceil() as f32)
}

/// The object assistive technology acts on for a view: the view itself, or
/// for views that aren't accessibility elements (an `NSStepper`), their
/// single accessibility child (its cell), as VoiceOver finds it.
fn a11y_element(view: &NSView) -> Retained<AnyObject> {
    let mut element: Retained<AnyObject> = view.retain().into();
    loop {
        let is_element: bool = unsafe { msg_send![&*element, isAccessibilityElement] };
        let children: Option<Retained<NSArray<AnyObject>>> = unsafe { msg_send![&*element, accessibilityChildren] };
        match children {
            Some(children) if !is_element && children.len() == 1 => element = children.objectAtIndex(0),
            _ => return element,
        }
    }
}

/// `intrinsicContentSize`, with "no intrinsic size" (-1) as zero.
/// AppKit's steps are tick marks, which the knob then only stops at.
fn set_ticks(slider: &NSSlider, step: Option<f64>) {
    let range = slider.maxValue() - slider.minValue();
    match step.filter(|s| *s > 0.0 && range > 0.0) {
        Some(step) => {
            slider.setNumberOfTickMarks((range / step).round() as isize + 1);
            slider.setAllowsTickMarkValuesOnly(true);
        }
        None => {
            slider.setNumberOfTickMarks(0);
            slider.setAllowsTickMarkValuesOnly(false);
        }
    }
}

fn intrinsic(view: &NSView) -> Size {
    let size = view.intrinsicContentSize();
    ceil_size(NSSize::new(size.width.max(0.0), size.height.max(0.0)))
}

impl Backend for AppKitBackend {
    fn init(&mut self, events: EventSink) {
        self.state.borrow_mut().events = events;
    }

    fn metrics(&self) -> PlatformMetrics {
        let state = self.state.borrow();
        metrics(state.mtm, state.options.appearance)
    }

    fn apply(&mut self, batch: &[Command]) {
        let mut state = self.state.borrow_mut();
        for command in batch {
            if state.options.record_commands {
                state.log.push(command.clone());
            }
            state.apply(command);
        }
        state.layout_lists();
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return Size::ZERO };
        let natural = match &node.widget {
            Widget::Label(label) => {
                // TODO: min-content (longest word) — until then text never
                // shrinks below its single-line width in flex rows.
                let width = request.known_width.map(f64::from).or(match request.available_width {
                    AvailableSpace::Definite(w) => Some(w as f64),
                    AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                });
                let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width.unwrap_or(1.0e7), 1.0e7));
                match label.cell() {
                    Some(cell) => ceil_size(cell.cellSizeForBounds(bounds)),
                    None => Size::ZERO,
                }
            }
            Widget::Field(field) => {
                let intrinsic = field.intrinsicContentSize();
                Size::new(
                    if intrinsic.width > 0.0 { intrinsic.width.ceil() as f32 } else { 200.0 },
                    intrinsic.height.ceil() as f32,
                )
            }
            Widget::Button(v) | Widget::Checkbox(v) => ceil_size(v.intrinsicContentSize()),
            Widget::Switch(v) => ceil_size(v.intrinsicContentSize()),
            // AppKit sizes pop-up buttons for their widest item.
            Widget::Select(v) => ceil_size(v.intrinsicContentSize()),
            // No natural width: they're as wide as the layout makes them.
            Widget::Slider { slider, .. } => intrinsic(slider),
            Widget::Progress(p) => intrinsic(p),
            Widget::Spinner { indicator, .. } => intrinsic(indicator),
            Widget::Custom { view, render, props } => {
                render.measure(view, props.props(), &request).unwrap_or_else(|| intrinsic(view))
            }
            Widget::Native { view, measure, .. } => match measure {
                Some(measure) => measure(view, &request),
                None => intrinsic(view),
            },
            // Measured by the core.
            Widget::Drawn { .. } | Widget::Window { .. } | Widget::Host(_) | Widget::Scroll(_) | Widget::List(_) => {
                Size::ZERO
            }
        };
        Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        // A list's rows: select or activate them in the table. The table
        // only calls back into its own data, never into our state.
        {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if let (Some(row), Some(Widget::List(list))) =
                (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
            {
                match action {
                    A11yAction::Select if list.mode() != SelectionMode::None => {
                        list.set_selected(&[row]);
                        list.report_selection();
                    }
                    A11yAction::Activate => list.activate(row),
                    A11yAction::ScrollIntoView => {}
                    _ => return Err(ActionError::Unsupported),
                }
                return Ok(());
            }
        }
        let (widget_view, control_enabled, kind, events, custom) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let custom = match &node.widget {
                Widget::Custom { render, props, .. } => Some((render.clone(), props.props().clone())),
                _ => None,
            };
            let enabled = node.widget.control().map(|c| c.isEnabled());
            (node.widget.view().retain(), enabled, node.kind, state.events.clone(), custom)
        };
        if control_enabled == Some(false) {
            return Err(ActionError::Disabled);
        }
        if let Some((render, props)) = custom {
            return render.perform(&widget_view, &props, action, &Emitter::new(events, id));
        }
        // No state borrow below: AppKit calls back into our targets.
        match (action, kind) {
            (A11yAction::Activate, WidgetKind::Button | WidgetKind::Checkbox | WidgetKind::Switch) => {
                // The press action assistive technology uses. Its return
                // value is unreliable for windows that aren't on screen (it
                // reports NO after pressing), so it is ignored.
                let _: bool = unsafe { msg_send![&*widget_view, accessibilityPerformPress] };
            }
            // Native views: whatever their accessibility element does.
            // Return values are ignored for the same reason as above.
            (A11yAction::Activate, WidgetKind::Native) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformPress] };
            }
            (A11yAction::Increment, WidgetKind::Native) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformIncrement] };
            }
            (A11yAction::Decrement, WidgetKind::Native) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformDecrement] };
            }
            // What VoiceOver's increment and decrement do.
            (A11yAction::Increment, WidgetKind::Slider) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformIncrement] };
            }
            (A11yAction::Decrement, WidgetKind::Slider) => {
                let _: bool = unsafe { msg_send![&*a11y_element(&widget_view), accessibilityPerformDecrement] };
            }
            (A11yAction::SetValue(text), WidgetKind::Slider) => {
                let slider: &NSSlider = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                // As if dragged there: the slider sends its action.
                slider.setDoubleValue(value);
                unsafe { slider.sendAction_to(slider.action(), slider.target().as_deref()) };
            }
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                // As if the item were picked from the open menu: the pop-up
                // button chooses it and sends its action.
                let popup: &NSPopUpButton = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                let index = popup.itemTitles().iter().position(|t| t.to_string() == *text);
                let (Some(index), Some(menu)) = (index, popup.menu()) else { return Err(ActionError::Unsupported) };
                menu.performActionForItemAtIndex(index as isize);
            }
            (A11yAction::SetValue(text), WidgetKind::TextInput | WidgetKind::PasswordInput) => {
                let field: &NSTextField = widget_view.downcast_ref().ok_or(ActionError::Unsupported)?;
                if !field.isEditable() {
                    return Err(ActionError::ReadOnly);
                }
                field.setStringValue(&ns(text));
                // Programmatic edits don't notify the delegate; assistive
                // technology edits are user edits, so report one.
                events.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
            }
            (A11yAction::Focus, _) => {
                let view = self.state.borrow().nodes.get(&id).map(|n| n.widget.key_view()).unwrap_or(widget_view);
                let window = view.window().ok_or(ActionError::Unsupported)?;
                if !window.makeFirstResponder(Some(&view)) {
                    return Err(ActionError::Unsupported);
                }
                // The window delegate reports the focus change.
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }

    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        if let SyntheticInput::Click(point) = input {
            // Drawn widgets only: native controls track the mouse in a
            // loop of their own, waiting for real events.
            let view = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
                Some(Widget::Drawn { view, .. }) => view.clone(),
                Some(_) => return Err(ActionError::Unsupported),
                None => return Err(ActionError::UnknownNode),
            };
            let window = view.window().ok_or(ActionError::Unsupported)?;
            let location = view.convertPoint_toView(NSPoint::new(point.x as f64, point.y as f64), None);
            for (kind, up) in [(NSEventType::LeftMouseDown, false), (NSEventType::LeftMouseUp, true)] {
                let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                    kind,
                    location,
                    NSEventModifierFlags::empty(),
                    0.0,
                    window.windowNumber(),
                    None,
                    0,
                    1,
                    if up { 0.0 } else { 1.0 },
                )
                .ok_or(ActionError::Unsupported)?;
                if up { view.mouseUp(&event) } else { view.mouseDown(&event) }
            }
            return Ok(());
        }
        if let SyntheticInput::Scroll { dx, dy } = input {
            let (scroll, axes) = match self.state.borrow().nodes.get(&id).map(|n| (&n.widget, n.scroll_axes)) {
                Some((Widget::Scroll(scroll), axes)) => (scroll.clone(), axes),
                Some((Widget::List(list), _)) => (list.scroll.clone(), ScrollAxes::Vertical),
                Some(_) => return Err(ActionError::Unsupported),
                None => return Err(ActionError::UnknownNode),
            };
            let clip = scroll.contentView();
            let visible = clip.bounds();
            let content = scroll.documentView().map_or(visible.size, |d| d.frame().size);
            let clamp = |v: f64, content: f64, visible: f64, on: bool| {
                if on { v.clamp(0.0, (content - visible).max(0.0)) } else { 0.0 }
            };
            let origin = NSPoint::new(
                clamp(visible.origin.x + *dx as f64, content.width, visible.size.width, axes.horizontal()),
                clamp(visible.origin.y + *dy as f64, content.height, visible.size.height, axes.vertical()),
            );
            scroll_to(&scroll, origin);
            return Ok(());
        }
        let SyntheticInput::Key(key) = input else { unreachable!() };
        let table = match self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::List(list)) => Some(list.table.clone()),
            _ => None,
        };
        if let Some(table) = table {
            // Real key events, through the table's own key handling: arrows
            // move the selection and scroll to it, Home and End scroll, and
            // Return activates (ours).
            let (code, character) = match key {
                Key::Up => (126, '\u{f700}'),
                Key::Down => (125, '\u{f701}'),
                Key::Home => (115, '\u{f729}'),
                Key::End => (119, '\u{f72b}'),
                Key::Enter => (36, '\r'),
                _ => return Err(ActionError::Unsupported),
            };
            let window = table.window().ok_or(ActionError::Unsupported)?;
            window.makeFirstResponder(Some(&table));
            let characters = ns(&character.to_string());
            let flags = if *key == Key::Enter {
                NSEventModifierFlags::empty()
            } else {
                NSEventModifierFlags::Function | NSEventModifierFlags::NumericPad
            };
            let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                NSEventType::KeyDown,
                NSPoint::new(0.0, 0.0),
                flags,
                0.0,
                window.windowNumber(),
                None,
                &characters,
                &characters,
                false,
                code,
            )
            .ok_or(ActionError::Unsupported)?;
            table.keyDown(&event);
            return Ok(());
        }
        let (view, kind) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.widget.control().is_some_and(|c| !c.isEnabled()) {
                return Err(ActionError::Disabled);
            }
            (node.widget.view().retain(), node.kind)
        };
        match (kind, key) {
            (
                WidgetKind::TextInput | WidgetKind::PasswordInput,
                Key::Char(_) | Key::Backspace | Key::Enter | Key::Tab,
            ) => {
                // Drive the field editor, the object that receives real
                // keystrokes: text goes through `insertText:`, and editing
                // keys through `doCommandBySelector:`, which is what
                // `interpretKeyEvents:` does, delegate hooks included.
                let window = view.window().ok_or(ActionError::Unsupported)?;
                let field: &NSTextField = view.downcast_ref().ok_or(ActionError::Unsupported)?;
                if !field.isEditable() {
                    return Err(ActionError::ReadOnly);
                }
                let just_focused = field.currentEditor().is_none();
                if just_focused {
                    window.makeFirstResponder(Some(&view));
                }
                let editor = field.currentEditor().ok_or(ActionError::Unsupported)?;
                if just_focused {
                    // Focusing selects everything; typing should append, as
                    // after clicking past the end of the text.
                    let end = editor.string().length();
                    editor.setSelectedRange(NSRange::new(end, 0));
                }
                let command = match key {
                    Key::Char(c) => {
                        let text = ns(&c.to_string());
                        let _: () = unsafe { msg_send![&*editor, insertText: &*text] };
                        return Ok(());
                    }
                    Key::Backspace => sel!(deleteBackward:),
                    Key::Enter => sel!(insertNewline:),
                    _ => sel!(insertTab:),
                };
                unsafe { editor.doCommandBySelector(command) };
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
        let checked = |s: isize| Prop::Checked(s == NSControlStateValueOn);
        match &node.widget {
            Widget::Window { window, .. } => props.push(Prop::Title(window.title().to_string())),
            Widget::Label(l) => {
                props.push(Prop::Text(l.stringValue().to_string()));
                let lines = l.maximumNumberOfLines();
                props.push(Prop::MaxLines((lines > 0).then_some(lines as u32)));
            }
            Widget::Field(f) => {
                props.push(Prop::Value(f.stringValue().to_string()));
                if let Some(p) = f.placeholderString() {
                    props.push(Prop::Placeholder(p.to_string()));
                }
                props.push(Prop::ReadOnly(!f.isEditable()));
            }
            Widget::Button(b) => props.push(Prop::Label(b.title().to_string())),
            Widget::Checkbox(b) => {
                props.push(Prop::Label(b.title().to_string()));
                let mixed = b.state() == NSControlStateValueMixed;
                props.push(if mixed { Prop::Checked(node.checked) } else { checked(b.state()) });
                if node.mixed.is_some() {
                    props.push(Prop::Mixed(mixed));
                }
            }
            Widget::Switch(s) => {
                if let Some(label) = s.accessibilityLabel() {
                    props.push(Prop::Label(label.to_string()));
                }
                props.push(checked(s.state()));
            }
            Widget::Select(p) => {
                if let Some(label) = p.accessibilityLabel() {
                    props.push(Prop::Label(label.to_string()));
                }
                props.push(Prop::Options(p.itemTitles().iter().map(|t| t.to_string()).collect()));
                let index = p.indexOfSelectedItem();
                props.push(Prop::SelectedIndex((index >= 0).then_some(index as usize)));
            }
            Widget::Slider { slider, step } => {
                if let Some(label) = slider.accessibilityLabel() {
                    props.push(Prop::Label(label.to_string()));
                }
                props.push(Prop::Range { min: slider.minValue(), max: slider.maxValue() });
                props.push(Prop::Step(*step));
                props.push(Prop::Number(slider.doubleValue()));
                if node.orientation.is_some() {
                    props.push(Prop::Orientation(if slider.isVertical() {
                        Orientation::Vertical
                    } else {
                        Orientation::Horizontal
                    }));
                }
            }
            Widget::Progress(p) => {
                if let Some(label) = p.accessibilityLabel() {
                    props.push(Prop::Label(label.to_string()));
                }
                props.push(Prop::Progress((!p.isIndeterminate()).then(|| p.doubleValue())));
            }
            Widget::Spinner { indicator, running } => {
                if let Some(label) = indicator.accessibilityLabel() {
                    props.push(Prop::Label(label.to_string()));
                }
                props.push(Prop::Running(*running));
            }
            Widget::Scroll(scroll) => {
                let shown = (scroll.hasHorizontalScroller(), scroll.hasVerticalScroller());
                // Hidden scrollers leave only the node to say which axes scroll.
                props.push(Prop::ScrollAxes(match shown {
                    (true, true) => ScrollAxes::Both,
                    (true, false) => ScrollAxes::Horizontal,
                    (false, true) => ScrollAxes::Vertical,
                    (false, false) => node.scroll_axes,
                }));
                props.push(Prop::ScrollBars(shown != (false, false)));
            }
            Widget::Custom { view, render, props: last } => {
                props.push(Prop::Custom(last.with_props(render.read(view, last.props()))))
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
        if let Some(control) = node.widget.control() {
            props.push(Prop::Enabled(control.isEnabled()));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        let view = node.widget.view();
        let f = view.alignmentRectForFrame(view.frame());
        let mut frame = Rect::new(f.origin.x as f32, f.origin.y as f32, f.size.width as f32, f.size.height as f32);
        // A row is where the table put it.
        if let (Some(row), Some(Widget::List(list))) =
            (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
        {
            frame = list.row_rect(row).unwrap_or(frame);
        }
        let by_view = state.by_view.borrow();
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => {
                let origin = list.scroll.contentView().bounds().origin;
                (list.children(), Some(Point::new(origin.x as f32, origin.y as f32)))
            }
            Widget::Scroll(scroll) => {
                let origin = scroll.contentView().bounds().origin;
                (
                    scroll.documentView().and_then(|d| by_view.get(&key(&d)).copied()).into_iter().collect(),
                    Some(Point::new(origin.x as f32, origin.y as f32)),
                )
            }
            _ => (view.subviews().iter().filter_map(|v| by_view.get(&key(&v)).copied()).collect(), None),
        };
        Some(NativeState {
            kind: node.kind,
            props,
            frame,
            parent: node.parent,
            children,
            focused: focused(&node.widget),
            scroll_offset,
        })
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

impl AppKitBackend {
    fn capture_now(&self, id: NodeId) -> Result<Image, CaptureError> {
        let view = {
            let state = self.state.borrow();
            state.nodes.get(&id).ok_or(CaptureError::UnknownNode)?.widget.view().retain()
        };
        // Views that lay out their own subviews (a table's rows) do it in
        // a layout pass, which offscreen windows only get when asked.
        view.layoutSubtreeIfNeeded();
        let bounds = view.bounds();
        let rep = view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .ok_or_else(|| CaptureError::Failed("no bitmap for view".into()))?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        let (width, height) = (rep.pixelsWide() as usize, rep.pixelsHigh() as usize);
        let (samples, bits, row) = (rep.samplesPerPixel() as usize, rep.bitsPerSample(), rep.bytesPerRow() as usize);
        if bits != 8 || !(samples == 3 || samples == 4) {
            return Err(CaptureError::Failed(format!("unsupported bitmap: {samples} samples × {bits} bits")));
        }
        let alpha_first = rep.bitmapFormat().contains(NSBitmapFormat::AlphaFirst);
        let data = rep.bitmapData();
        if data.is_null() {
            return Err(CaptureError::Failed("bitmap has no data".into()));
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, row * height) };
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let p = &bytes[y * row + x * samples..][..samples];
                let [r, g, b, a] = match (samples, alpha_first) {
                    (4, true) => [p[1], p[2], p[3], p[0]],
                    (4, false) => [p[0], p[1], p[2], p[3]],
                    _ => [p[0], p[1], p[2], 255],
                };
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
        let scale = if bounds.size.width > 0.0 { width as f32 / bounds.size.width as f32 } else { 1.0 };
        Ok(Image { width: width as u32, height: height as u32, scale_factor: scale, rgba })
    }
}
