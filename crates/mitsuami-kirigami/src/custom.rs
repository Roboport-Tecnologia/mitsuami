//! Escape hatches on KDE: native renders for custom widgets, raw QML items
//! in the shared tree, and the painter for drawn widgets.

use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::draw::{DrawOp, PathElement};
use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{
    AnyValue, Color, CustomWidget, DisplayList, Element, ElementBuilder, MeasureRequest, NodeId, Opaque, Prop,
    Renderer, Shape, Size, Ui, UiEvent, View, WidgetKind,
};

use crate::events::Events;
use crate::ffi::QmlObject;
use crate::theme::{self, Rgba};

/// Sends events from signal handlers to the widget's handlers. Cheap to
/// clone; keep one in closures that outlive creation.
#[derive(Clone)]
pub struct Emitter {
    events: Events,
    id: NodeId,
}

impl Emitter {
    pub(crate) fn new(events: Events, id: NodeId) -> Emitter {
        Emitter { events, id }
    }

    /// Queues an event. Handlers run on the next turn of the event loop,
    /// never inside the signal handler. Events reported while the backend
    /// applies props (its own updates) are dropped.
    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.events.emit(self.id, UiEvent::Custom(AnyValue::new(event)));
    }
}

/// What a native render or native view gets while its item is created.
pub struct KirigamiCx {
    emitter: Emitter,
}

impl KirigamiCx {
    pub(crate) fn new(events: Events, id: NodeId) -> KirigamiCx {
        KirigamiCx { emitter: Emitter::new(events, id) }
    }

    /// Creates an item from QML, with Qt Quick, its controls (`QQC2`), its
    /// layouts and Kirigami (`Kirigami`) imported.
    pub fn load(&self, qml: &str) -> QmlObject {
        QmlObject::load(qml)
    }

    pub fn emitter(&self) -> Emitter {
        self.emitter.clone()
    }

    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.emitter.emit(event);
    }
}

/// A custom widget's KDE render: one per widget, next to its shared
/// definition (e.g. `rating/kde.rs`). Hand it to the widget's `Render` impl
/// with [`native`] or [`ad_hoc`].
///
/// Custom widgets are controlled: when the user changes the item, emit an
/// event; the app answers with new props, and `update` shows them.
pub trait NativeRender: CustomWidget {
    /// Creates the item, usually from QML ([`KirigamiCx::load`]).
    fn create(props: &Self::Props, cx: &mut KirigamiCx) -> QmlObject;

    fn update(item: QmlObject, old: &Self::Props, new: &Self::Props);

    /// Intrinsic size. `None` uses the item's implicit size.
    fn measure(item: QmlObject, props: &Self::Props, request: &MeasureRequest) -> Option<Size> {
        let _ = (item, props, request);
        None
    }

    /// Props as the item shows them, so tests can catch an item that
    /// doesn't show what it was given. The default trusts the last props.
    fn read(item: QmlObject, props: &Self::Props) -> Self::Props {
        let _ = item;
        props.clone()
    }

    /// Carries out an accessibility action on the item. The default,
    /// `Unsupported`, lets the shared `CustomWidget::action` decide.
    fn perform(
        item: QmlObject,
        props: &Self::Props,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        let _ = (item, props, action, emitter);
        Err(ActionError::Unsupported)
    }
}

/// The renderer for a widget whose KDE render is a real Qt Quick or
/// Kirigami control.
pub fn native<W: NativeRender>() -> Renderer<W> {
    Renderer::native(erased::<W>())
}

/// The renderer for a widget Qt and Kirigami have no control for, built ad
/// hoc from their controls the way KDE apps build it. Not labelled native.
pub fn ad_hoc<W: NativeRender>() -> Renderer<W> {
    Renderer::ad_hoc(erased::<W>())
}

fn erased<W: NativeRender>() -> Opaque {
    let render: Rc<dyn ErasedRender> = Rc::new(RenderImpl::<W>(PhantomData));
    Opaque::new("kirigami render", render)
}

/// [`NativeRender`] with the types erased, as the backend holds it.
pub(crate) trait ErasedRender {
    fn create(&self, props: &AnyValue, cx: &mut KirigamiCx) -> QmlObject;
    fn update(&self, item: QmlObject, old: &AnyValue, new: &AnyValue);
    fn measure(&self, item: QmlObject, props: &AnyValue, request: &MeasureRequest) -> Option<Size>;
    fn read(&self, item: QmlObject, props: &AnyValue) -> AnyValue;
    fn perform(
        &self,
        item: QmlObject,
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
}

impl<W: NativeRender> ErasedRender for RenderImpl<W> {
    fn create(&self, props: &AnyValue, cx: &mut KirigamiCx) -> QmlObject {
        W::create(Self::props(props), cx)
    }

    fn update(&self, item: QmlObject, old: &AnyValue, new: &AnyValue) {
        W::update(item, Self::props(old), Self::props(new));
    }

    fn measure(&self, item: QmlObject, props: &AnyValue, request: &MeasureRequest) -> Option<Size> {
        W::measure(item, Self::props(props), request)
    }

    fn read(&self, item: QmlObject, props: &AnyValue) -> AnyValue {
        AnyValue::new(W::read(item, Self::props(props)))
    }

    fn perform(
        &self,
        item: QmlObject,
        props: &AnyValue,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        W::perform(item, Self::props(props), action, emitter)
    }
}

type Factory = Box<dyn FnOnce(&mut KirigamiCx) -> QmlObject>;
type Measure = Rc<dyn Fn(QmlObject, &MeasureRequest) -> Size>;
type Apply = Rc<dyn Fn(QmlObject)>;
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
    pub(crate) fn apply(&self, item: QmlObject) {
        for update in &self.updates {
            update(item);
        }
    }
}

/// Any QML item in the shared tree. It takes part in layout like other
/// leaves; the core sees `WidgetKind::Native`.
///
/// ```ignore
/// NativeView::qml(|cx| {
///     let spin = cx.load("QQC2.SpinBox { from: 0; to: 5 }");
///     let emitter = cx.emitter();
///     // Every change, a screen reader's too; the backend's own are muted.
///     spin.connect("valueChanged()", move || emitter.emit(spin.int("value")));
///     spin
/// })
/// .update(value, |spin, value| spin.set_int("value", *value))
/// .on_event(move |value: &i32| value_signal.set(*value))
/// ```
///
/// Headless tests show it as an empty box: give it a size, or test it natively.
pub struct NativeView {
    element: Element,
    create: Factory,
    measure: Option<Measure>,
    updates: Vec<Snapshot>,
}

impl NativeView {
    /// `create` runs once, when the backend creates the node.
    pub fn qml(create: impl FnOnce(&mut KirigamiCx) -> QmlObject + 'static) -> NativeView {
        NativeView {
            element: Element::new(WidgetKind::Native),
            create: Box::new(create),
            measure: None,
            updates: Vec::new(),
        }
    }

    /// Intrinsic size. Without it, the item's implicit size.
    pub fn measure(mut self, measure: impl Fn(QmlObject, &MeasureRequest) -> Size + 'static) -> NativeView {
        self.measure = Some(Rc::new(measure));
        self
    }

    /// Calls `apply` with the item and the value, now and whenever the
    /// value changes. Values are usually signals or closures.
    pub fn update<T: Clone + 'static>(
        mut self,
        value: impl IntoValue<T>,
        apply: impl Fn(QmlObject, &T) + 'static,
    ) -> NativeView {
        let value = value.into_value();
        let apply = Rc::new(apply);
        self.updates.push(Box::new(move || {
            let value = value.get();
            let apply = apply.clone();
            Rc::new(move |item: QmlObject| apply(item, &value))
        }));
        self
    }

    /// Called with each event of type `E` the item emits (see [`Emitter`]).
    pub fn on_event<E: 'static>(mut self, handler: impl Fn(&E) + 'static) -> NativeView {
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

impl ElementBuilder for NativeView {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for NativeView {
    fn build(self, ui: &Ui) -> NodeId {
        let NativeView { mut element, create, measure, updates } = self;
        let spec = Rc::new(NativeViewSpec { create: RefCell::new(Some(create)), measure });
        let payload = move || NativePayload { spec: spec.clone(), updates: updates.iter().map(|u| u()).collect() };
        element.prop(Value::Dynamic(Rc::new(payload)), |payload| Prop::Native(Opaque::new("kirigami view", payload)));
        element.build(ui)
    }
}

// ------------------------------------------------------------- drawn tier

/// Resolved when the display list arrives. The core draws again when the
/// metrics change (a theme switch among them), so drawn widgets follow the
/// color scheme.
fn resolve(color: Color, colors: &theme::Colors) -> Rgba {
    match color {
        Color::Label => colors.text,
        // What Kirigami uses for secondary text.
        Color::SecondaryLabel => colors.disabled_text,
        Color::Accent => colors.highlight,
        Color::Separator => colors.separator,
        Color::ControlBackground => colors.view_background,
        Color::WindowBackground => colors.background,
        // KDE's status colours (Breeze's red, orange and green).
        Color::Error => colors.negative_text,
        Color::Warning => colors.neutral_text,
        Color::Success => colors.positive_text,
        Color::Rgba(r, g, b, a) => [r, g, b, a].map(|c| c as f32 / 255.0),
    }
}

/// The display list as the C++ painter reads it (see `DrawnItem::paint`).
pub(crate) fn flatten(drawing: &DisplayList) -> Vec<f32> {
    let colors = theme::colors();
    let mut out = Vec::new();
    for op in drawing.ops() {
        let (kind, shape, color, width) = match op {
            DrawOp::Fill { shape, color } => (0.0, shape, color, 0.0),
            DrawOp::Stroke { shape, color, width } => (1.0, shape, color, *width),
        };
        out.push(kind);
        out.push(width);
        out.extend(resolve(*color, &colors));
        match shape {
            Shape::Rect(r) => out.extend([0.0, r.x(), r.y(), r.width(), r.height()]),
            Shape::RoundedRect(r, radius) => out.extend([1.0, r.x(), r.y(), r.width(), r.height(), *radius]),
            Shape::Ellipse(r) => out.extend([2.0, r.x(), r.y(), r.width(), r.height()]),
            Shape::Path(path) => {
                out.push(3.0);
                out.push(path.elements().len() as f32);
                for element in path.elements() {
                    match *element {
                        PathElement::MoveTo(p) => out.extend([0.0, p.x, p.y]),
                        PathElement::LineTo(p) => out.extend([1.0, p.x, p.y]),
                        PathElement::CurveTo { c1, c2, to } => out.extend([2.0, c1.x, c1.y, c2.x, c2.y, to.x, to.y]),
                        PathElement::Close => out.push(3.0),
                    }
                }
            }
        }
    }
    out
}
