//! Escape hatches on GTK: native renders for custom widgets, raw GTK
//! widgets in the shared tree, and the rasterizer for drawn widgets.

use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk};
use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::draw::{DrawOp, PathElement};
use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{
    AnyValue, Color, CustomWidget, DisplayList, Element, ElementBuilder, MeasureRequest, NodeId, Opaque, Point,
    PointerEvent, PointerKind, Prop, Renderer, Shape, Size, Ui, UiEvent, View, WidgetKind,
};

use crate::host::Events;

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

    /// Queues an event. Handlers run on the next turn of the main loop,
    /// never inside the signal handler. Events GTK reports while the
    /// backend applies props (programmatic changes) are dropped.
    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.events.emit(self.id, UiEvent::Custom(AnyValue::new(event)));
    }
}

/// What a native render or native view gets while its widget is created.
pub struct GtkCx {
    emitter: Emitter,
}

impl GtkCx {
    pub(crate) fn new(events: Events, id: NodeId) -> GtkCx {
        GtkCx { emitter: Emitter::new(events, id) }
    }

    pub fn emitter(&self) -> Emitter {
        self.emitter.clone()
    }

    pub fn emit<E: Any + PartialEq + fmt::Debug>(&self, event: E) {
        self.emitter.emit(event);
    }
}

/// A custom widget's GTK render: one per widget, next to its shared
/// definition (e.g. `rating/linux.rs`). Hand it to the widget's `Render`
/// impl with [`native`].
///
/// Custom widgets are controlled: when the user changes the widget, emit an
/// event; the app answers with new props, and `update` shows them.
pub trait NativeRender: CustomWidget {
    type Widget: IsA<gtk::Widget>;

    fn create(props: &Self::Props, cx: &mut GtkCx) -> Self::Widget;

    fn update(widget: &Self::Widget, old: &Self::Props, new: &Self::Props);

    /// Intrinsic size. `None` uses GTK's natural size.
    fn measure(widget: &Self::Widget, props: &Self::Props, request: &MeasureRequest) -> Option<Size> {
        let _ = (widget, props, request);
        None
    }

    /// Props as the widget shows them, so tests can catch a widget that
    /// doesn't show what it was given. The default trusts the last props.
    fn read(widget: &Self::Widget, props: &Self::Props) -> Self::Props {
        let _ = widget;
        props.clone()
    }

    /// Carries out an accessibility action on the widget. The default,
    /// `Unsupported`, lets the shared `CustomWidget::action` decide.
    fn perform(
        widget: &Self::Widget,
        props: &Self::Props,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        let _ = (widget, props, action, emitter);
        Err(ActionError::Unsupported)
    }
}

/// The renderer for a widget whose GTK render is GTK's own control.
pub fn native<W: NativeRender>() -> Renderer<W> {
    Renderer::native(erased::<W>())
}

/// The renderer for a widget GTK has no control for, built ad hoc from GTK
/// widgets the way GNOME apps build it. Not labelled native.
pub fn ad_hoc<W: NativeRender>() -> Renderer<W> {
    Renderer::ad_hoc(erased::<W>())
}

fn erased<W: NativeRender>() -> Opaque {
    let render: Rc<dyn ErasedRender> = Rc::new(RenderImpl::<W>(PhantomData));
    Opaque::new("gtk render", render)
}

/// [`NativeRender`] with the types erased, as the backend holds it.
pub(crate) trait ErasedRender {
    fn create(&self, props: &AnyValue, cx: &mut GtkCx) -> gtk::Widget;
    fn update(&self, widget: &gtk::Widget, old: &AnyValue, new: &AnyValue);
    fn measure(&self, widget: &gtk::Widget, props: &AnyValue, request: &MeasureRequest) -> Option<Size>;
    fn read(&self, widget: &gtk::Widget, props: &AnyValue) -> AnyValue;
    fn perform(
        &self,
        widget: &gtk::Widget,
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

    fn widget(widget: &gtk::Widget) -> &W::Widget {
        widget.downcast_ref().unwrap_or_else(|| panic!("{}: the node's widget is not the render's", W::NAME))
    }
}

impl<W: NativeRender> ErasedRender for RenderImpl<W> {
    fn create(&self, props: &AnyValue, cx: &mut GtkCx) -> gtk::Widget {
        W::create(Self::props(props), cx).upcast()
    }

    fn update(&self, widget: &gtk::Widget, old: &AnyValue, new: &AnyValue) {
        W::update(Self::widget(widget), Self::props(old), Self::props(new));
    }

    fn measure(&self, widget: &gtk::Widget, props: &AnyValue, request: &MeasureRequest) -> Option<Size> {
        W::measure(Self::widget(widget), Self::props(props), request)
    }

    fn read(&self, widget: &gtk::Widget, props: &AnyValue) -> AnyValue {
        AnyValue::new(W::read(Self::widget(widget), Self::props(props)))
    }

    fn perform(
        &self,
        widget: &gtk::Widget,
        props: &AnyValue,
        action: &A11yAction,
        emitter: &Emitter,
    ) -> Result<(), ActionError> {
        W::perform(Self::widget(widget), Self::props(props), action, emitter)
    }
}

type Factory = Box<dyn FnOnce(&mut GtkCx) -> gtk::Widget>;
type Measure = Rc<dyn Fn(&gtk::Widget, &MeasureRequest) -> Size>;
type Apply = Rc<dyn Fn(&gtk::Widget)>;
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
    pub(crate) fn apply(&self, widget: &gtk::Widget) {
        for update in &self.updates {
            update(widget);
        }
    }
}

/// Any GTK widget in the shared tree. It takes part in layout like other
/// leaves; the core sees `WidgetKind::Native`.
///
/// ```ignore
/// NativeView::gtk(|cx| {
///     let spin = gtk::SpinButton::with_range(0.0, 5.0, 1.0);
///     let emitter = cx.emitter();
///     spin.connect_value_changed(move |s| emitter.emit(s.value()));
///     spin
/// })
/// .update(value, |spin, value| spin.set_value(*value))
/// .on_event(move |value: &f64| value_signal.set(*value))
/// ```
///
/// Headless tests show it as an empty box: give it a size, or test it natively.
pub struct NativeView<W> {
    element: Element,
    create: Box<dyn FnOnce(&mut GtkCx) -> W>,
    measure: Option<Measure>,
    updates: Vec<Snapshot>,
}

impl<W: IsA<gtk::Widget>> NativeView<W> {
    /// `create` runs once, when the backend creates the node.
    pub fn gtk(create: impl FnOnce(&mut GtkCx) -> W + 'static) -> NativeView<W> {
        NativeView {
            element: Element::new(WidgetKind::Native),
            create: Box::new(create),
            measure: None,
            updates: Vec::new(),
        }
    }

    /// Intrinsic size. Without it, GTK's natural size.
    pub fn measure(mut self, measure: impl Fn(&W, &MeasureRequest) -> Size + 'static) -> NativeView<W> {
        self.measure =
            Some(Rc::new(move |widget: &gtk::Widget, request: &MeasureRequest| match widget.downcast_ref::<W>() {
                Some(widget) => measure(widget, request),
                None => Size::ZERO,
            }));
        self
    }

    /// Calls `apply` with the widget and the value, now and whenever the
    /// value changes. Values are usually signals or closures.
    pub fn update<T: Clone + 'static>(
        mut self,
        value: impl IntoValue<T>,
        apply: impl Fn(&W, &T) + 'static,
    ) -> NativeView<W> {
        let value = value.into_value();
        let apply = Rc::new(apply);
        self.updates.push(Box::new(move || {
            let value = value.get();
            let apply = apply.clone();
            Rc::new(move |widget: &gtk::Widget| {
                if let Some(widget) = widget.downcast_ref::<W>() {
                    apply(widget, &value);
                }
            })
        }));
        self
    }

    /// Called with each event of type `E` the widget emits (see [`Emitter`]).
    pub fn on_event<E: 'static>(mut self, handler: impl Fn(&E) + 'static) -> NativeView<W> {
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

impl<W> ElementBuilder for NativeView<W> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<W: IsA<gtk::Widget>> View for NativeView<W> {
    fn build(self, ui: &Ui) -> NodeId {
        let NativeView { mut element, create, measure, updates } = self;
        let create: Factory = Box::new(move |cx| create(cx).upcast());
        let spec = Rc::new(NativeViewSpec { create: RefCell::new(Some(create)), measure });
        let payload = move || NativePayload { spec: spec.clone(), updates: updates.iter().map(|u| u()).collect() };
        element.prop(Value::Dynamic(Rc::new(payload)), |payload| Prop::Native(Opaque::new("gtk view", payload)));
        element.build(ui)
    }
}

// ------------------------------------------------------------- drawn tier

/// A drawn custom widget: a drawing area that rasterizes the display list
/// the core sends, and reports primary-button presses for the core to
/// interpret.
pub(crate) struct DrawnArea {
    pub(crate) area: gtk::DrawingArea,
    pub(crate) click: gtk::GestureClick,
    drawing: Rc<RefCell<DisplayList>>,
}

impl DrawnArea {
    pub(crate) fn new(events: Events, id: NodeId) -> DrawnArea {
        let area = gtk::DrawingArea::new();
        let drawing: Rc<RefCell<DisplayList>> = Rc::default();
        let shown = drawing.clone();
        area.set_draw_func(move |area, cr, _, _| rasterize(area.upcast_ref(), cr, &shown.borrow()));
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let pointer = move |kind| {
            let events = events.clone();
            move |_: &gtk::GestureClick, _: i32, x: f64, y: f64| {
                let position = Point::new(x as f32, y as f32);
                events.emit(id, UiEvent::Pointer(PointerEvent { kind, position }));
            }
        };
        click.connect_pressed(pointer(PointerKind::Down));
        click.connect_released(pointer(PointerKind::Up));
        area.add_controller(click.clone());
        DrawnArea { area, click, drawing }
    }

    pub(crate) fn drawing(&self) -> DisplayList {
        self.drawing.borrow().clone()
    }

    pub(crate) fn set_drawing(&self, drawing: DisplayList) {
        *self.drawing.borrow_mut() = drawing;
        self.area.queue_draw();
    }

    /// A click at `point`: the gesture's own press and release signals,
    /// since GTK 4 can't inject pointer events.
    pub(crate) fn click(&self, point: Point) {
        let (x, y) = (point.x as f64, point.y as f64);
        self.click.emit_by_name::<()>("pressed", &[&1i32, &x, &y]);
        self.click.emit_by_name::<()>("released", &[&1i32, &x, &y]);
    }
}

/// A named theme color. GTK's theme exports the classic names; the
/// libadwaita ones are tried too, then Adwaita's light values.
#[allow(deprecated)] // `lookup_color`: GTK has no replacement for theme colors yet.
fn theme_color(widget: &gtk::Widget, names: &[&str], fallback: (f32, f32, f32)) -> gdk::RGBA {
    let context = widget.style_context();
    names
        .iter()
        .find_map(|name| context.lookup_color(name))
        .unwrap_or_else(|| gdk::RGBA::new(fallback.0, fallback.1, fallback.2, 1.0))
}

/// Resolved at draw time against the widget's style, so drawn widgets
/// follow the theme and dark mode.
pub(crate) fn rgba(widget: &gtk::Widget, color: Color) -> gdk::RGBA {
    let fg = widget.color();
    let faded = |alpha: f32| gdk::RGBA::new(fg.red(), fg.green(), fg.blue(), fg.alpha() * alpha);
    match color {
        Color::Label => fg,
        Color::SecondaryLabel => faded(0.55),
        Color::Accent => {
            theme_color(widget, &["accent_color", "accent_bg_color", "theme_selected_bg_color"], (0.21, 0.52, 0.89))
        }
        Color::Separator => theme_color(widget, &["borders"], (0.80, 0.78, 0.76)),
        Color::ControlBackground => theme_color(widget, &["view_bg_color", "theme_base_color"], (1.0, 1.0, 1.0)),
        Color::WindowBackground => theme_color(widget, &["window_bg_color", "theme_bg_color"], (0.98, 0.98, 0.98)),
        // libadwaita's names, which GTK's own theme shares.
        Color::Error => theme_color(widget, &["error_color"], (0.75, 0.11, 0.16)),
        Color::Warning => theme_color(widget, &["warning_color"], (0.61, 0.43, 0.01)),
        Color::Success => theme_color(widget, &["success_color"], (0.11, 0.52, 0.33)),
        Color::Rgba(r, g, b, a) => {
            let c = |v: u8| v as f32 / 255.0;
            gdk::RGBA::new(c(r), c(g), c(b), c(a))
        }
    }
}

fn path(cr: &cairo::Context, shape: &Shape) {
    cr.new_path();
    match shape {
        Shape::Rect(r) => cr.rectangle(r.x() as f64, r.y() as f64, r.width() as f64, r.height() as f64),
        Shape::RoundedRect(r, radius) => {
            let (x, y, w, h) = (r.x() as f64, r.y() as f64, r.width() as f64, r.height() as f64);
            let radius = (*radius as f64).min(w / 2.0).min(h / 2.0);
            let quarter = std::f64::consts::FRAC_PI_2;
            cr.new_sub_path();
            cr.arc(x + w - radius, y + radius, radius, -quarter, 0.0);
            cr.arc(x + w - radius, y + h - radius, radius, 0.0, quarter);
            cr.arc(x + radius, y + h - radius, radius, quarter, 2.0 * quarter);
            cr.arc(x + radius, y + radius, radius, 2.0 * quarter, 3.0 * quarter);
            cr.close_path();
        }
        Shape::Ellipse(r) => {
            let (w, h) = (r.width() as f64, r.height() as f64);
            if w > 0.0 && h > 0.0 {
                cr.save().ok();
                cr.translate(r.x() as f64 + w / 2.0, r.y() as f64 + h / 2.0);
                cr.scale(w / 2.0, h / 2.0);
                cr.arc(0.0, 0.0, 1.0, 0.0, 2.0 * std::f64::consts::PI);
                cr.restore().ok();
            }
        }
        Shape::Path(p) => {
            for element in p.elements() {
                match *element {
                    PathElement::MoveTo(p) => cr.move_to(p.x as f64, p.y as f64),
                    PathElement::LineTo(p) => cr.line_to(p.x as f64, p.y as f64),
                    PathElement::CurveTo { c1, c2, to } => {
                        cr.curve_to(c1.x as f64, c1.y as f64, c2.x as f64, c2.y as f64, to.x as f64, to.y as f64)
                    }
                    PathElement::Close => cr.close_path(),
                }
            }
        }
    }
}

/// Draws a display list with Cairo, in the widget's coordinates.
fn rasterize(widget: &gtk::Widget, cr: &cairo::Context, drawing: &DisplayList) {
    for op in drawing.ops() {
        let (shape, color) = match op {
            DrawOp::Fill { shape, color } | DrawOp::Stroke { shape, color, .. } => (shape, color),
        };
        let c = rgba(widget, *color);
        cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
        path(cr, shape);
        let _ = match op {
            DrawOp::Fill { .. } => cr.fill(),
            DrawOp::Stroke { width, .. } => {
                cr.set_line_width(*width as f64);
                cr.stroke()
            }
        };
    }
}
