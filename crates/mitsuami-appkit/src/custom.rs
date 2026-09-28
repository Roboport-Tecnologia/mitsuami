//! Escape hatches on AppKit: native renders for custom widgets, raw native
//! views in the shared tree, and the rasterizer for drawn widgets.

use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::draw::{DrawOp, PathElement};
use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{
    AnyValue, Color, CustomWidget, DisplayList, Element, ElementBuilder, EventSink, MeasureRequest, NodeId, Opaque,
    Prop, Renderer, Shape, Size, Ui, UiEvent, View, WidgetKind,
};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DowncastTarget, MainThreadMarker, Message, sel};
use objc2_app_kit::{NSBezierPath, NSColor, NSControl, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::classes::ClosureTarget;

/// Sends events from native callbacks to the widget's handlers. Cheap to
/// clone; keep one in closures that outlive creation.
#[derive(Clone)]
pub struct Emitter {
    events: EventSink,
    id: NodeId,
}

impl Emitter {
    pub(crate) fn new(events: EventSink, id: NodeId) -> Emitter {
        Emitter { events, id }
    }

    /// Queues an event. Handlers run on the next turn of the run loop, never
    /// inside the native callback.
    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.events.emit(self.id, UiEvent::Custom(AnyValue::new(event)));
    }
}

/// What a native render or native view gets while its view is created.
pub struct AppKitCx<'a> {
    mtm: MainThreadMarker,
    emitter: Emitter,
    targets: &'a mut Vec<Retained<ClosureTarget>>,
}

impl<'a> AppKitCx<'a> {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        id: NodeId,
        events: EventSink,
        targets: &'a mut Vec<Retained<ClosureTarget>>,
    ) -> AppKitCx<'a> {
        AppKitCx { mtm, emitter: Emitter::new(events, id), targets }
    }

    pub fn mtm(&self) -> MainThreadMarker {
        self.mtm
    }

    pub fn emitter(&self) -> Emitter {
        self.emitter.clone()
    }

    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.emitter.emit(event);
    }

    /// Runs `handler` when the control sends its action (a click, a new
    /// value). The target lives as long as the node.
    pub fn on_action<C>(&mut self, control: &C, handler: impl Fn(&C) + 'static)
    where
        C: Message + DowncastTarget + AsRef<NSControl>,
    {
        let target = ClosureTarget::new(self.mtm, move |sender| {
            if let Some(control) = sender.downcast_ref::<C>() {
                handler(control);
            }
        });
        let control: &NSControl = control.as_ref();
        let target_obj: &AnyObject = target.as_ref();
        // SAFETY: the node keeps the target alive as long as the control.
        unsafe {
            control.setTarget(Some(target_obj));
            control.setAction(Some(sel!(fire:)));
        }
        self.targets.push(target);
    }
}

/// A custom widget's AppKit render: one per widget, next to its shared
/// definition (e.g. `rating/macos.rs`). Hand it to the widget's `Render`
/// impl with [`native`].
///
/// Custom widgets are controlled: when the user changes the view, emit an
/// event; the app answers with new props, and `update` shows them.
pub trait NativeRender: CustomWidget {
    type View: Message + DowncastTarget + AsRef<NSView>;

    fn create(props: &Self::Props, cx: &mut AppKitCx) -> Retained<Self::View>;

    fn update(view: &Self::View, old: &Self::Props, new: &Self::Props);

    /// Intrinsic size. `None` uses the view's `intrinsicContentSize`.
    fn measure(view: &Self::View, props: &Self::Props, request: &MeasureRequest) -> Option<Size> {
        let _ = (view, props, request);
        None
    }

    /// Props as the view shows them, so tests can catch a view that
    /// doesn't show what it was given. The default trusts the last props.
    fn read(view: &Self::View, props: &Self::Props) -> Self::Props {
        let _ = view;
        props.clone()
    }

    /// Carries out an accessibility action on the view. The default,
    /// `Unsupported`, lets the shared `CustomWidget::action` decide.
    fn perform(
        view: &Self::View,
        props: &Self::Props,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        let _ = (view, props, action, emitter);
        Err(ActionError::Unsupported)
    }
}

/// The renderer for a widget whose AppKit render is AppKit's own control.
pub fn native<W: NativeRender>() -> Renderer<W> {
    Renderer::native(erased::<W>())
}

/// The renderer for a widget AppKit has no control for, built ad hoc from
/// AppKit views the way Mac apps build it. Not labelled native.
pub fn ad_hoc<W: NativeRender>() -> Renderer<W> {
    Renderer::ad_hoc(erased::<W>())
}

fn erased<W: NativeRender>() -> Opaque {
    let render: Rc<dyn ErasedRender> = Rc::new(RenderImpl::<W>(PhantomData));
    Opaque::new("appkit render", render)
}

/// [`NativeRender`] with the types erased, as the backend holds it.
pub(crate) trait ErasedRender {
    fn create(&self, props: &AnyValue, cx: &mut AppKitCx) -> Retained<NSView>;
    fn update(&self, view: &NSView, old: &AnyValue, new: &AnyValue);
    fn measure(&self, view: &NSView, props: &AnyValue, request: &MeasureRequest) -> Option<Size>;
    fn read(&self, view: &NSView, props: &AnyValue) -> AnyValue;
    fn perform(
        &self,
        view: &NSView,
        props: &AnyValue,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError>;
}

struct RenderImpl<W>(PhantomData<fn() -> W>);

impl<W: NativeRender> RenderImpl<W> {
    fn props(props: &AnyValue) -> &W::Props {
        props.downcast_ref().unwrap_or_else(|| panic!("{}: props of another type: {props:?}", W::NAME))
    }

    fn view(view: &NSView) -> &W::View {
        view.downcast_ref().unwrap_or_else(|| panic!("{}: the node's view is not the render's", W::NAME))
    }
}

impl<W: NativeRender> ErasedRender for RenderImpl<W> {
    fn create(&self, props: &AnyValue, cx: &mut AppKitCx) -> Retained<NSView> {
        let view = W::create(Self::props(props), cx);
        let view: &NSView = (*view).as_ref();
        view.retain()
    }

    fn update(&self, view: &NSView, old: &AnyValue, new: &AnyValue) {
        W::update(Self::view(view), Self::props(old), Self::props(new));
    }

    fn measure(&self, view: &NSView, props: &AnyValue, request: &MeasureRequest) -> Option<Size> {
        W::measure(Self::view(view), Self::props(props), request)
    }

    fn read(&self, view: &NSView, props: &AnyValue) -> AnyValue {
        AnyValue::new(W::read(Self::view(view), Self::props(props)))
    }

    fn perform(
        &self,
        view: &NSView,
        props: &AnyValue,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        W::perform(Self::view(view), Self::props(props), action, emitter)
    }
}

type Factory = Box<dyn FnOnce(&mut AppKitCx) -> Retained<NSView>>;
type TypedFactory<V> = Box<dyn FnOnce(&mut AppKitCx) -> Retained<V>>;
type Measure = Rc<dyn Fn(&NSView, &MeasureRequest) -> Size>;
type Apply = Rc<dyn Fn(&NSView)>;
/// Reads one `update` source (tracked) and returns how to apply its value.
type Snapshot = Box<dyn Fn() -> Apply>;

/// How to create a native view.
pub(crate) struct NativeViewSpec {
    pub(crate) create: RefCell<Option<Factory>>,
    pub(crate) measure: Option<Measure>,
}

/// A native view's `Prop::Native`: how to create it, and every `update`
/// with its current value. When any value changes, the whole payload is
/// sent again and every update re-applied.
#[derive(Clone)]
pub(crate) struct NativePayload {
    pub(crate) spec: Rc<NativeViewSpec>,
    pub(crate) updates: Vec<Apply>,
}

impl NativePayload {
    pub(crate) fn apply(&self, view: &NSView) {
        for update in &self.updates {
            update(view);
        }
    }
}

/// Any `NSView` in the shared tree. It takes part in layout like other
/// leaves; the core sees `WidgetKind::Native`.
///
/// ```ignore
/// NativeView::appkit(|cx| {
///     let stepper = NSStepper::new(cx.mtm());
///     let emitter = cx.emitter();
///     cx.on_action(&*stepper, move |s| emitter.emit(s.doubleValue()));
///     stepper
/// })
/// .update(value, |stepper, value| stepper.setDoubleValue(*value))
/// .on_event(move |value: &f64| value_signal.set(*value))
/// ```
///
/// Headless tests show it as an empty box: give it a size, or test it natively.
pub struct NativeView<V> {
    element: Element,
    create: TypedFactory<V>,
    measure: Option<Measure>,
    updates: Vec<Snapshot>,
}

impl<V: Message + DowncastTarget + AsRef<NSView>> NativeView<V> {
    /// `create` runs once, when the backend creates the node.
    pub fn appkit(create: impl FnOnce(&mut AppKitCx) -> Retained<V> + 'static) -> NativeView<V> {
        NativeView {
            element: Element::new(WidgetKind::Native),
            create: Box::new(create),
            measure: None,
            updates: Vec::new(),
        }
    }

    /// Intrinsic size. Without it, the view's `intrinsicContentSize` (or
    /// zero where it has none: size it with styles).
    pub fn measure(mut self, measure: impl Fn(&V, &MeasureRequest) -> Size + 'static) -> NativeView<V> {
        self.measure = Some(Rc::new(move |view: &NSView, request: &MeasureRequest| match view.downcast_ref::<V>() {
            Some(view) => measure(view, request),
            None => Size::ZERO,
        }));
        self
    }

    /// Calls `apply` with the view and the value, now and whenever the
    /// value changes. Values are usually signals or closures.
    pub fn update<T: Clone + 'static>(
        mut self,
        value: impl IntoValue<T>,
        apply: impl Fn(&V, &T) + 'static,
    ) -> NativeView<V> {
        let value = value.into_value();
        let apply = Rc::new(apply);
        self.updates.push(Box::new(move || {
            let value = value.get();
            let apply = apply.clone();
            Rc::new(move |view: &NSView| {
                if let Some(view) = view.downcast_ref::<V>() {
                    apply(view, &value);
                }
            })
        }));
        self
    }

    /// Called with each event of type `E` the view emits (see [`Emitter`]).
    pub fn on_event<E: 'static>(mut self, handler: impl Fn(&E) + 'static) -> NativeView<V> {
        self.element.on(move |event| {
            if let UiEvent::Custom(value) = event
                && let Some(event) = value.downcast_ref::<E>()
            {
                handler(event);
            }
        });
        self
    }
}

impl<V> ElementBuilder for NativeView<V> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<V: Message + DowncastTarget + AsRef<NSView>> View for NativeView<V> {
    fn build(self, ui: &Ui) -> NodeId {
        let NativeView { mut element, create, measure, updates } = self;
        let create: Factory = Box::new(move |cx| {
            let view = create(cx);
            let view: &NSView = (*view).as_ref();
            view.retain()
        });
        let spec = Rc::new(NativeViewSpec { create: RefCell::new(Some(create)), measure });
        let payload = move || NativePayload { spec: spec.clone(), updates: updates.iter().map(|u| u()).collect() };
        element.prop(Value::Dynamic(Rc::new(payload)), |payload| Prop::Native(Opaque::new("appkit view", payload)));
        element.build(ui)
    }
}

// ------------------------------------------------------------- drawn tier

fn ns_rect(r: &mitsuami_core::Rect) -> NSRect {
    NSRect::new(NSPoint::new(r.x() as f64, r.y() as f64), NSSize::new(r.width() as f64, r.height() as f64))
}

fn ns_point(p: mitsuami_core::Point) -> NSPoint {
    NSPoint::new(p.x as f64, p.y as f64)
}

pub(crate) fn ns_color(color: Color) -> Retained<NSColor> {
    match color {
        Color::Label => NSColor::labelColor(),
        Color::SecondaryLabel => NSColor::secondaryLabelColor(),
        Color::Accent => NSColor::controlAccentColor(),
        Color::Separator => NSColor::separatorColor(),
        Color::ControlBackground => NSColor::controlBackgroundColor(),
        Color::WindowBackground => NSColor::windowBackgroundColor(),
        Color::Error => NSColor::systemRedColor(),
        Color::Warning => NSColor::systemOrangeColor(),
        Color::Success => NSColor::systemGreenColor(),
        Color::Rgba(r, g, b, a) => {
            let c = |v: u8| v as f64 / 255.0;
            NSColor::colorWithSRGBRed_green_blue_alpha(c(r), c(g), c(b), c(a))
        }
    }
}

fn bezier(shape: &Shape) -> Retained<NSBezierPath> {
    match shape {
        Shape::Rect(r) => NSBezierPath::bezierPathWithRect(ns_rect(r)),
        Shape::RoundedRect(r, radius) => {
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(ns_rect(r), *radius as f64, *radius as f64)
        }
        Shape::Ellipse(r) => NSBezierPath::bezierPathWithOvalInRect(ns_rect(r)),
        Shape::Path(path) => {
            let bezier = NSBezierPath::new();
            for element in path.elements() {
                match *element {
                    PathElement::MoveTo(p) => bezier.moveToPoint(ns_point(p)),
                    PathElement::LineTo(p) => bezier.lineToPoint(ns_point(p)),
                    PathElement::CurveTo { c1, c2, to } => {
                        bezier.curveToPoint_controlPoint1_controlPoint2(ns_point(to), ns_point(c1), ns_point(c2))
                    }
                    PathElement::Close => bezier.closePath(),
                }
            }
            bezier
        }
    }
}

/// Draws a display list into the current (flipped) view. Semantic colors
/// resolve against the view's appearance, so dark mode just works.
pub(crate) fn rasterize(drawing: &DisplayList) {
    for op in drawing.ops() {
        match op {
            DrawOp::Fill { shape, color } => {
                ns_color(*color).setFill();
                bezier(shape).fill();
            }
            DrawOp::Stroke { shape, color, width } => {
                ns_color(*color).setStroke();
                let path = bezier(shape);
                path.setLineWidth(*width as f64);
                path.stroke();
            }
        }
    }
}
