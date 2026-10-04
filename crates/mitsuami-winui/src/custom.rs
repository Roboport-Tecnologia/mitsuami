//! Escape hatches on WinUI: native renders for custom widgets, raw XAML
//! elements in the shared tree, and the rasterizer for drawn widgets.

use std::any::Any;
use std::cell::RefCell;
use std::fmt::{self, Write as _};
use std::marker::PhantomData;
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::draw::{DrawOp, PathElement};
use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{
    AnyValue, Color, CustomWidget, DisplayList, Element, ElementBuilder, MeasureRequest, NodeId, Opaque, Point,
    PointerEvent, PointerKind, Prop, Rect, Renderer, Shape, Size, Ui, UiEvent, View, WidgetKind,
};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// Sends a widget's events from XAML event handlers to its handlers. Cheap
/// to clone; keep one in closures that outlive creation.
#[derive(Clone)]
pub struct Emitter {
    events: Events,
    id: NodeId,
}

impl Emitter {
    pub(crate) fn new(events: Events, id: NodeId) -> Emitter {
        Emitter { events, id }
    }

    /// Queues an event. Handlers run on the next turn of the run loop,
    /// never inside the XAML handler. Events XAML raises while the backend
    /// applies props (programmatic changes) are dropped.
    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.events.emit_custom(self.id, AnyValue::new(event));
    }
}

/// What a native render or native view gets while its widget is created.
pub struct WinUiCx {
    emitter: Emitter,
    revokers: Vec<EventRevoker>,
}

impl WinUiCx {
    pub(crate) fn new(events: Events, id: NodeId) -> WinUiCx {
        WinUiCx { emitter: Emitter::new(events, id), revokers: Vec::new() }
    }

    pub fn emitter(&self) -> Emitter {
        self.emitter.clone()
    }

    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.emitter.emit(event);
    }

    /// Keeps an event subscription for as long as the node lives:
    /// windows-rs unsubscribes when an `EventRevoker` is dropped.
    ///
    /// ```ignore
    /// cx.keep(rating.ValueChanged(move |sender, _| { /* … */ })?);
    /// ```
    pub fn keep(&mut self, revoker: EventRevoker) {
        self.revokers.push(revoker);
    }

    /// Calls `handler` whenever `property` of `element` changes, whatever
    /// changed it: the user, the keyboard, or UI Automation. Prefer it to a
    /// control's change event, which may not cover them all
    /// (`RatingControl.ValueChanged` ignores values set by screen readers).
    /// It runs synchronously, so the backend's own prop updates are muted.
    ///
    /// ```ignore
    /// cx.observe(&rating, &RatingControl::ValueProperty()?, move |rating| { /* … */ })?;
    /// ```
    pub fn observe<E: Interface>(
        &mut self,
        element: &E,
        property: &w::DependencyProperty,
        handler: impl Fn(&E) + 'static,
    ) -> R<()> {
        let callback = w::DependencyPropertyChangedCallback::new(move |sender, _| {
            if let Some(element) = sender.as_ref().and_then(|s| s.cast::<E>().ok()) {
                handler(&element);
            }
        });
        // The callback goes away with the element; it holds only the handler.
        element.cast::<w::IDependencyObject>()?.RegisterPropertyChangedCallback(property, &callback)?;
        Ok(())
    }

    pub(crate) fn into_revokers(self) -> Vec<EventRevoker> {
        self.revokers
    }
}

/// A custom widget's WinUI render: one per widget, next to its shared
/// definition (e.g. `pips_pager/windows.rs`). Hand it to the widget's
/// `Render` impl with [`native`].
///
/// `Element` is any XAML element type: from [`bindings`](crate::bindings),
/// or from bindings of your own for controls those don't cover (casts
/// between them work, they're the same COM objects).
///
/// Custom widgets are controlled: when the user changes the widget, emit an
/// event; the app answers with new props, and `update` shows them.
pub trait NativeRender: CustomWidget {
    type Element: Interface;

    fn create(props: &Self::Props, cx: &mut WinUiCx) -> R<Self::Element>;

    fn update(element: &Self::Element, old: &Self::Props, new: &Self::Props) -> R<()>;

    /// Intrinsic size. `None` uses XAML's `Measure`.
    fn measure(element: &Self::Element, props: &Self::Props, request: &MeasureRequest) -> Option<Size> {
        let _ = (element, props, request);
        None
    }

    /// Props as the widget shows them, so tests can catch a widget that
    /// doesn't show what it was given. The default trusts the last props.
    fn read(element: &Self::Element, props: &Self::Props) -> Self::Props {
        let _ = element;
        props.clone()
    }

    /// Carries out an accessibility action on the widget. The default,
    /// `Unsupported`, lets the shared `CustomWidget::action` decide.
    fn perform(
        element: &Self::Element,
        props: &Self::Props,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        let _ = (element, props, action, emitter);
        Err(ActionError::Unsupported)
    }
}

/// The renderer for a widget whose WinUI render is WinUI's own control.
pub fn native<W: NativeRender>() -> Renderer<W> {
    Renderer::native(erased::<W>())
}

/// The renderer for a widget WinUI has no control for, built ad hoc from
/// XAML elements the way Windows apps build it. Not labelled native.
pub fn ad_hoc<W: NativeRender>() -> Renderer<W> {
    Renderer::ad_hoc(erased::<W>())
}

fn erased<W: NativeRender>() -> Opaque {
    let render: Rc<dyn ErasedRender> = Rc::new(RenderImpl::<W>(PhantomData));
    Opaque::new("winui render", render)
}

/// [`NativeRender`] with the types erased, as the backend holds it.
pub(crate) trait ErasedRender {
    fn create(&self, props: &AnyValue, cx: &mut WinUiCx) -> R<w::UIElement>;
    fn update(&self, element: &w::UIElement, old: &AnyValue, new: &AnyValue) -> R<()>;
    fn measure(&self, element: &w::UIElement, props: &AnyValue, request: &MeasureRequest) -> Option<Size>;
    fn read(&self, element: &w::UIElement, props: &AnyValue) -> AnyValue;
    fn perform(
        &self,
        element: &w::UIElement,
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

    fn element(element: &w::UIElement) -> W::Element {
        element.cast().unwrap_or_else(|_| panic!("{}: the node's element is not the render's", W::NAME))
    }
}

impl<W: NativeRender> ErasedRender for RenderImpl<W> {
    fn create(&self, props: &AnyValue, cx: &mut WinUiCx) -> R<w::UIElement> {
        W::create(Self::props(props), cx)?.cast()
    }

    fn update(&self, element: &w::UIElement, old: &AnyValue, new: &AnyValue) -> R<()> {
        W::update(&Self::element(element), Self::props(old), Self::props(new))
    }

    fn measure(&self, element: &w::UIElement, props: &AnyValue, request: &MeasureRequest) -> Option<Size> {
        W::measure(&Self::element(element), Self::props(props), request)
    }

    fn read(&self, element: &w::UIElement, props: &AnyValue) -> AnyValue {
        AnyValue::new(W::read(&Self::element(element), Self::props(props)))
    }

    fn perform(
        &self,
        element: &w::UIElement,
        props: &AnyValue,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        W::perform(&Self::element(element), Self::props(props), action, emitter)
    }
}

// ------------------------------------------------------------ native views

type Factory = Box<dyn FnOnce(&mut WinUiCx) -> R<w::UIElement>>;
pub(crate) type Measure = Rc<dyn Fn(&w::UIElement, &MeasureRequest) -> Size>;
type Apply = Rc<dyn Fn(&w::UIElement) -> R<()>>;
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
    updates: Vec<Apply>,
}

impl NativePayload {
    pub(crate) fn apply(&self, element: &w::UIElement) -> R<()> {
        self.updates.iter().try_for_each(|update| update(element))
    }
}

type Create<E> = Box<dyn FnOnce(&mut WinUiCx) -> R<E>>;

/// Any XAML element in the shared tree. It takes part in layout like other
/// leaves; the core sees `WidgetKind::Native`.
///
/// ```ignore
/// NativeView::xaml(|cx| {
///     let slider = bindings::Slider::new()?;
///     let emitter = cx.emitter();
///     cx.keep(slider.cast::<bindings::IRangeBase>()?.ValueChanged(move |sender, _| { /* … */ })?);
///     Ok(slider)
/// })
/// .update(value, |slider, value| slider.cast::<bindings::IRangeBase>()?.SetValue(*value))
/// .on_event(move |value: &f64| value_signal.set(*value))
/// ```
///
/// Headless tests show it as an empty box: give it a size, or test it natively.
pub struct NativeView<E> {
    element: Element,
    create: Create<E>,
    measure: Option<Measure>,
    updates: Vec<Snapshot>,
}

impl<E: Interface + 'static> NativeView<E> {
    /// `create` runs once, when the backend creates the node.
    pub fn xaml(create: impl FnOnce(&mut WinUiCx) -> R<E> + 'static) -> NativeView<E> {
        NativeView {
            element: Element::new(WidgetKind::Native),
            create: Box::new(create),
            measure: None,
            updates: Vec::new(),
        }
    }

    /// Intrinsic size. Without it, XAML's `Measure`.
    pub fn measure(mut self, measure: impl Fn(&E, &MeasureRequest) -> Size + 'static) -> NativeView<E> {
        self.measure =
            Some(Rc::new(move |element: &w::UIElement, request: &MeasureRequest| match element.cast::<E>() {
                Ok(element) => measure(&element, request),
                Err(_) => Size::ZERO,
            }));
        self
    }

    /// Calls `apply` with the element and the value, now and whenever the
    /// value changes. Values are usually signals or closures.
    pub fn update<T: Clone + 'static>(
        mut self,
        value: impl IntoValue<T>,
        apply: impl Fn(&E, &T) -> R<()> + 'static,
    ) -> NativeView<E> {
        let value = value.into_value();
        let apply = Rc::new(apply);
        self.updates.push(Box::new(move || {
            let value = value.get();
            let apply = apply.clone();
            Rc::new(move |element: &w::UIElement| apply(&element.cast::<E>()?, &value))
        }));
        self
    }

    /// Called with each event of type `Ev` the element emits (see [`Emitter`]).
    pub fn on_event<Ev: 'static>(mut self, handler: impl Fn(&Ev) + 'static) -> NativeView<E> {
        self.element.on(move |event| {
            if let UiEvent::Custom(value) = event
                && let Some(event) = value.downcast_ref::<Ev>()
            {
                handler(event);
            }
        });
        self
    }
}

impl<E> ElementBuilder for NativeView<E> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<E: Interface + 'static> View for NativeView<E> {
    fn build(self, ui: &Ui) -> NodeId {
        let NativeView { mut element, create, measure, updates } = self;
        let create: Factory = Box::new(move |cx| create(cx)?.cast());
        let spec = Rc::new(NativeViewSpec { create: RefCell::new(Some(create)), measure });
        let payload = move || NativePayload { spec: spec.clone(), updates: updates.iter().map(|u| u()).collect() };
        element.prop(Value::Dynamic(Rc::new(payload)), |payload| Prop::Native(Opaque::new("winui view", payload)));
        element.build(ui)
    }
}

// ------------------------------------------------------------- drawn tier

/// A drawn custom widget: a canvas holding XAML shapes built from the
/// display list the core sends, and reporting primary-button presses for
/// the core to interpret.
pub(crate) struct DrawnView {
    pub(crate) canvas: w::Canvas,
    drawing: RefCell<DisplayList>,
    emitter: Events,
    id: NodeId,
}

impl DrawnView {
    pub(crate) fn new(events: Events, id: NodeId, revokers: &mut Vec<EventRevoker>) -> R<DrawnView> {
        let canvas = w::Canvas::new()?;
        // A background, even a clear one, makes the whole area hit-testable.
        let clear = w::SolidColorBrush::CreateInstanceWithColor(w::Color { a: 0, r: 0, g: 0, b: 0 })?;
        canvas.cast::<w::IPanel>()?.SetBackground(&clear)?;
        let element: w::IUIElement = canvas.cast()?;
        for kind in [PointerKind::Down, PointerKind::Up] {
            let events = events.clone();
            let handler = move |sender: windows_core::Ref<windows_core::IInspectable>,
                                args: windows_core::Ref<w::PointerRoutedEventArgs>| {
                let position = (|| -> R<w::Point> {
                    let relative: w::UIElement = sender.as_ref().ok_or_else(windows_core::Error::empty)?.cast()?;
                    let args: w::IPointerRoutedEventArgs =
                        args.as_ref().ok_or_else(windows_core::Error::empty)?.cast()?;
                    args.GetCurrentPoint(&relative)?.cast::<w::IPointerPoint>()?.Position()
                })();
                if let Ok(p) = position {
                    let position = Point::new(p.x, p.y);
                    events.emit(id, UiEvent::Pointer(PointerEvent { kind, position }));
                }
            };
            revokers.push(match kind {
                PointerKind::Down => element.PointerPressed(handler)?,
                PointerKind::Up => element.PointerReleased(handler)?,
            });
        }
        Ok(DrawnView { canvas, drawing: RefCell::default(), emitter: events, id })
    }

    pub(crate) fn drawing(&self) -> DisplayList {
        self.drawing.borrow().clone()
    }

    pub(crate) fn set_drawing(&self, drawing: &DisplayList) -> R<()> {
        let shapes: w::UIElement = w::XamlReader::Load(&markup(drawing))?.cast()?;
        let children = self.canvas.cast::<w::IPanel>()?.Children()?;
        children.Clear()?;
        children.Append(&shapes)?;
        *self.drawing.borrow_mut() = drawing.clone();
        Ok(())
    }

    /// A click at `point`, reported like the pointer handlers do: XAML
    /// offers no way to inject pointer input.
    pub(crate) fn click(&self, point: Point) {
        for kind in [PointerKind::Down, PointerKind::Up] {
            self.emitter.emit(self.id, UiEvent::Pointer(PointerEvent { kind, position: point }));
        }
    }
}

/// A theme brush for a semantic color, as markup: `{ThemeResource}` is
/// resolved against the element's theme, and follows it live.
fn brush(color: Color) -> String {
    let resource = match color {
        Color::Label => "TextFillColorPrimaryBrush",
        Color::SecondaryLabel => "TextFillColorSecondaryBrush",
        Color::Accent => "AccentFillColorDefaultBrush",
        Color::Separator => "DividerStrokeColorDefaultBrush",
        Color::ControlBackground => "ControlFillColorDefaultBrush",
        Color::WindowBackground => "SolidBackgroundFillColorBaseBrush",
        // Fluent's status colours, as InfoBar and validation use them.
        Color::Error => "SystemFillColorCriticalBrush",
        Color::Warning => "SystemFillColorCautionBrush",
        Color::Success => "SystemFillColorSuccessBrush",
        Color::Rgba(r, g, b, a) => return format!("#{a:02X}{r:02X}{g:02X}{b:02X}"),
    };
    format!("{{ThemeResource {resource}}}")
}

/// A brush for text in a semantic color, as markup: the accent has a
/// brush of its own for text, which keeps its contrast.
pub(crate) fn text_brush(color: Color) -> String {
    match color {
        Color::Accent => "{ThemeResource AccentTextFillColorPrimaryBrush}".to_owned(),
        _ => brush(color),
    }
}

/// Path markup for a shape. Everything is a path, so strokes are centred on
/// the outline as on the other platforms (XAML's `Rectangle` strokes inside).
fn geometry(shape: &Shape) -> String {
    let mut d = String::new();
    let rect = |d: &mut String, r: &Rect| {
        let _ = write!(d, "M {},{} H {} V {} H {} Z", r.x(), r.y(), r.max_x(), r.max_y(), r.x());
    };
    match shape {
        Shape::Rect(r) => rect(&mut d, r),
        Shape::RoundedRect(r, radius) => {
            let k = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
            if k == 0.0 {
                rect(&mut d, r);
            } else {
                let (x, y, x2, y2) = (r.x(), r.y(), r.max_x(), r.max_y());
                let _ = write!(
                    d,
                    "M {},{y} H {} A {k},{k} 0 0 1 {x2},{} V {} A {k},{k} 0 0 1 {},{y2} H {} \
                     A {k},{k} 0 0 1 {x},{} V {} A {k},{k} 0 0 1 {},{y} Z",
                    x + k,
                    x2 - k,
                    y + k,
                    y2 - k,
                    x2 - k,
                    x + k,
                    y2 - k,
                    y + k,
                    x + k,
                );
            }
        }
        Shape::Ellipse(r) => {
            // Two half arcs around the bounding box.
            let (rx, ry, cy) = (r.width() / 2.0, r.height() / 2.0, r.y() + r.height() / 2.0);
            let _ =
                write!(d, "M {},{cy} A {rx},{ry} 0 1 1 {},{cy} A {rx},{ry} 0 1 1 {},{cy} Z", r.x(), r.max_x(), r.x());
        }
        Shape::Path(path) => {
            for element in path.elements() {
                let _ = match *element {
                    PathElement::MoveTo(p) => write!(d, "M {},{} ", p.x, p.y),
                    PathElement::LineTo(p) => write!(d, "L {},{} ", p.x, p.y),
                    PathElement::CurveTo { c1, c2, to } => {
                        write!(d, "C {},{} {},{} {},{} ", c1.x, c1.y, c2.x, c2.y, to.x, to.y)
                    }
                    PathElement::Close => write!(d, "Z "),
                };
            }
        }
    }
    d
}

/// Whether every number in a shape is finite: markup with `NaN` or `inf`
/// fails to parse, and the drawing with it.
fn finite(shape: &Shape) -> bool {
    let rect = |r: &Rect| [r.x(), r.y(), r.width(), r.height()].iter().all(|v| v.is_finite());
    let point = |p: &Point| p.x.is_finite() && p.y.is_finite();
    match shape {
        Shape::Rect(r) | Shape::Ellipse(r) => rect(r),
        Shape::RoundedRect(r, radius) => rect(r) && radius.is_finite(),
        Shape::Path(path) => path.elements().iter().all(|element| match element {
            PathElement::MoveTo(p) | PathElement::LineTo(p) => point(p),
            PathElement::CurveTo { c1, c2, to } => point(c1) && point(c2) && point(to),
            PathElement::Close => true,
        }),
    }
}

/// The display list as XAML markup: a canvas of paths, in paint order.
/// Shapes with numbers that aren't finite (a chart's 0/0) are left out.
fn markup(drawing: &DisplayList) -> String {
    let mut xaml = String::from(
        r#"<Canvas xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" IsHitTestVisible="False">"#,
    );
    for op in drawing.ops() {
        let _ = match op {
            DrawOp::Fill { shape, .. } | DrawOp::Stroke { shape, .. } if !finite(shape) => Ok(()),
            DrawOp::Stroke { width, .. } if !width.is_finite() => Ok(()),
            DrawOp::Fill { shape, color } => {
                write!(xaml, r#"<Path Data="{}" Fill="{}"/>"#, geometry(shape), brush(*color))
            }
            DrawOp::Stroke { shape, color, width } => write!(
                xaml,
                r#"<Path Data="{}" Stroke="{}" StrokeThickness="{width}"/>"#,
                geometry(shape),
                brush(*color)
            ),
        };
    }
    xaml.push_str("</Canvas>");
    xaml
}
