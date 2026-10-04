//! Custom widgets: one platform-free definition, rendered natively on each
//! platform or drawn with the [`Canvas`] API.
//!
//! - [`CustomWidget`]: the shared part. Props, events, semantics, and what
//!   accessibility actions mean.
//! - [`Drawn`]: the drawn tier. Measure, draw and hit-test in shared code;
//!   runs everywhere, and stands in for the widget in headless tests.
//! - The composed tier ([`Renderer::composed`]): built from the built-in
//!   widgets, for widgets the [`Canvas`] can't draw (text, editing).
//! - A native render per platform, e.g. `mitsuami_appkit::NativeRender`.
//! - [`Render`]: says which render each platform uses. Using a widget
//!   requires it, so a platform with no render doesn't compile.
//!
//! A widget unique to one platform is native there. Elsewhere it's built
//! ad hoc from the platform's widgets, the way that platform's apps build
//! it ([`Renderer::ad_hoc`]), or drawn, or composed; [`Renderer::is_native`]
//! says which.

use std::fmt;
use std::rc::Rc;

use mitsuami_reactive::{IntoValue, Value};

use crate::a11y::{A11yAction, A11yProps};
use crate::any_value::{AnyValue, Opaque};
use crate::backend::{MeasureRequest, PlatformMetrics};
use crate::command::{PointerEvent, UiEvent};
use crate::draw::{Canvas, DisplayList};
use crate::element::{Element, ElementBuilder};
use crate::geometry::Size;
use crate::ui::Ui;
use crate::view::{AnyView, View};
use crate::widget::{NodeId, Prop, WidgetKind};

/// The shared, platform-free definition of a custom widget.
///
/// Its functions answer from their arguments alone. [`a11y`](Self::a11y),
/// [`Drawn::measure`] and [`Drawn::draw`] run while the `Ui` holds its
/// tree (building the accessibility tree, laying out, drawing), so one
/// that reaches a `Ui` (builds nodes, sets props, commits, reads a query)
/// panics with a `RefCell` borrow error. [`action`](Self::action) and
/// [`Drawn::pointer`] run with the tree free, but their job is to return
/// an event, which the app handles in [`Custom::on_event`].
pub trait CustomWidget: Sized + 'static {
    /// Shown in trees, logs and snapshots.
    const NAME: &'static str;
    type Props: Clone + PartialEq + fmt::Debug + 'static;
    type Event: Clone + PartialEq + fmt::Debug + 'static;

    /// Semantics, shared by every render. Apps can still override the
    /// label and description where they use the widget.
    fn a11y(props: &Self::Props) -> A11yProps;

    /// What an accessibility action (increment, set value, …) means for
    /// this widget. Used unless the native render performs it itself.
    fn action(props: &Self::Props, action: &A11yAction) -> Option<Self::Event> {
        let _ = (props, action);
        None
    }
}

/// The drawn tier: measured, drawn and hit-tested in shared code.
pub trait Drawn: CustomWidget {
    /// Intrinsic size, like a backend measures a native control.
    fn measure(props: &Self::Props, request: &MeasureRequest, metrics: &PlatformMetrics) -> Size;

    /// Draws into `canvas`, sized to the widget's frame. Called again
    /// whenever the props, the size or the platform metrics change.
    fn draw(props: &Self::Props, canvas: &mut Canvas);

    /// What a pointer event at `event.position` (widget coordinates) means.
    fn pointer(props: &Self::Props, size: Size, event: &PointerEvent) -> Option<Self::Event> {
        let _ = (props, size, event);
        None
    }
}

/// Which render this platform uses. Implement it once, choosing per
/// platform with `platform!`:
///
/// ```ignore
/// impl Render for Rating {
///     fn renderer() -> Renderer<Self> {
///         platform! {
///             macos => mitsuami::appkit::native::<Self>().with_drawn(),
///             _ => Renderer::drawn(),
///         }
///     }
/// }
/// ```
pub trait Render: CustomWidget {
    fn renderer() -> Renderer<Self>;
}

/// A widget's renders on the current platform: native, drawn or composed.
/// The first one present is used: native, then drawn, then composed. The
/// others stay available (for headless tests, `.drawn()`, `.composed()`).
pub struct Renderer<W: CustomWidget> {
    native: Option<Opaque>,
    /// The native render is the platform's own control, not built ad hoc.
    platform_control: bool,
    drawn: Option<DrawnFns>,
    composed: Option<ComposeFn<W>>,
}

/// Builds the composed render from the widget's props and a way to emit
/// its events.
type ComposeFn<W> = Rc<dyn Fn(Composed<W>) -> AnyView>;

/// What a composed render gets: the props (reactive), a way to emit the
/// widget's events, and the accessible label the app gave the widget.
pub struct Composed<W: CustomWidget> {
    props: Rc<dyn Fn() -> W::Props>,
    emit: Rc<dyn Fn(W::Event)>,
    label: Option<String>,
}

impl<W: CustomWidget> Clone for Composed<W> {
    fn clone(&self) -> Self {
        Composed { props: self.props.clone(), emit: self.emit.clone(), label: self.label.clone() }
    }
}

impl<W: CustomWidget> Composed<W> {
    /// The current props. Reading them in a reactive closure tracks them.
    pub fn props(&self) -> W::Props {
        (self.props)()
    }

    /// Sends an event to the widget's `on_event` handlers. Call it from the
    /// built-in widgets' handlers (a click, a submit).
    pub fn emit(&self, event: W::Event) {
        (self.emit)(event)
    }

    /// The accessible label the app gave the widget: put it on the control
    /// that stands for the widget, so it's found the same way.
    pub fn label(&self) -> Option<String> {
        self.label.clone()
    }
}

impl<W: CustomWidget> Renderer<W> {
    /// Draws the widget with its [`Drawn`] implementation.
    pub fn drawn() -> Renderer<W>
    where
        W: Drawn,
    {
        Renderer { native: None, platform_control: false, drawn: Some(DrawnFns::of::<W>()), composed: None }
    }

    /// For backend crates: a native render that is the platform's own
    /// control, in whatever form the backend understands. Apps use the
    /// backend's constructor instead.
    pub fn native(render: Opaque) -> Renderer<W> {
        Renderer { native: Some(render), platform_control: true, drawn: None, composed: None }
    }

    /// For backend crates: a render built ad hoc from the platform's widgets,
    /// where the platform has no such control. It's rendered like a native
    /// one, but [`is_native`](Renderer::is_native) says no.
    pub fn ad_hoc(render: Opaque) -> Renderer<W> {
        Renderer { native: Some(render), platform_control: false, drawn: None, composed: None }
    }

    /// Keeps the drawn render too: headless tests lay the widget out with
    /// it, and `Custom::drawn` can pick it.
    pub fn with_drawn(mut self) -> Renderer<W>
    where
        W: Drawn,
    {
        self.drawn = Some(DrawnFns::of::<W>());
        self
    }

    /// Composes the widget from built-in widgets: the tier for widgets the
    /// canvas can't draw.
    pub fn composed<V: View>(compose: impl Fn(Composed<W>) -> V + 'static) -> Renderer<W> {
        Renderer {
            native: None,
            platform_control: false,
            drawn: None,
            composed: Some(Rc::new(move |c| AnyView::new(compose(c)))),
        }
    }

    /// Keeps a composed render too, for `Custom::composed`.
    pub fn with_composed<V: View>(mut self, compose: impl Fn(Composed<W>) -> V + 'static) -> Renderer<W> {
        self.composed = Some(Rc::new(move |c| AnyView::new(compose(c))));
        self
    }

    /// Whether the widget is the platform's own control here (rather than
    /// built ad hoc, drawn or composed as a stand-in).
    pub fn is_native(&self) -> bool {
        self.native.is_some() && self.platform_control
    }
}

/// The drawn tier, type-erased.
#[derive(Clone, Copy)]
struct DrawnFns {
    measure: fn(&AnyValue, &MeasureRequest, &PlatformMetrics) -> Size,
    draw: fn(&AnyValue, &mut Canvas),
    pointer: fn(&AnyValue, Size, &PointerEvent) -> Option<AnyValue>,
}

impl DrawnFns {
    fn of<W: Drawn>() -> DrawnFns {
        DrawnFns {
            measure: |props, request, metrics| W::measure(props_of::<W>(props), request, metrics),
            draw: |props, canvas| W::draw(props_of::<W>(props), canvas),
            pointer: |props, size, event| W::pointer(props_of::<W>(props), size, event).map(AnyValue::new),
        }
    }
}

fn props_of<W: CustomWidget>(props: &AnyValue) -> &W::Props {
    props.downcast_ref().unwrap_or_else(|| panic!("{}: props of another type: {props:?}", W::NAME))
}

/// Everything about one custom widget instance that is fixed at build time.
struct Definition {
    name: &'static str,
    a11y: fn(&AnyValue) -> A11yProps,
    action: fn(&AnyValue, &A11yAction) -> Option<AnyValue>,
    native: Option<Opaque>,
    drawn: Option<DrawnFns>,
    /// Draw it even where a native render exists.
    force_drawn: bool,
}

/// The props of a custom widget, as carried by [`Prop::Custom`]: the value
/// plus the widget's type-erased definition (semantics and renders).
#[derive(Clone)]
pub struct CustomProps {
    props: AnyValue,
    definition: Rc<Definition>,
}

impl CustomProps {
    pub fn name(&self) -> &'static str {
        self.definition.name
    }

    pub fn props(&self) -> &AnyValue {
        &self.props
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.props.downcast_ref()
    }

    /// The same widget with other props, e.g. read back from a native view.
    pub fn with_props(&self, props: AnyValue) -> CustomProps {
        CustomProps { props, definition: self.definition.clone() }
    }

    /// Whether the widget is drawn (by the core's display list) rather than
    /// rendered natively.
    pub fn is_drawn(&self) -> bool {
        self.definition.drawn.is_some() && (self.definition.force_drawn || self.definition.native.is_none())
    }

    /// The native render, unless the widget is drawn.
    pub fn native(&self) -> Option<&Opaque> {
        if self.is_drawn() { None } else { self.definition.native.as_ref() }
    }

    pub fn a11y(&self) -> A11yProps {
        (self.definition.a11y)(&self.props)
    }

    /// The event an accessibility action stands for, if any.
    pub fn action(&self, action: &A11yAction) -> Option<AnyValue> {
        (self.definition.action)(&self.props, action)
    }

    /// Size from the drawn render, if the widget has one.
    pub fn measure_drawn(&self, request: &MeasureRequest, metrics: &PlatformMetrics) -> Option<Size> {
        self.definition.drawn.map(|d| (d.measure)(&self.props, request, metrics))
    }

    pub fn draw(&self, size: Size, metrics: &PlatformMetrics) -> Option<DisplayList> {
        let drawn = self.definition.drawn?;
        let mut canvas = Canvas::new(size, metrics);
        (drawn.draw)(&self.props, &mut canvas);
        Some(canvas.finish())
    }

    pub fn pointer(&self, size: Size, event: &PointerEvent) -> Option<AnyValue> {
        self.definition.drawn.and_then(|d| (d.pointer)(&self.props, size, event))
    }
}

impl PartialEq for CustomProps {
    fn eq(&self, other: &CustomProps) -> bool {
        self.props == other.props && self.name() == other.name() && self.is_drawn() == other.is_drawn()
    }
}

impl fmt::Debug for CustomProps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.props)?;
        if self.is_drawn() {
            f.write_str(" (drawn)")?;
        }
        Ok(())
    }
}

/// A custom widget in the view tree: `Custom::<Rating>::new(props)`, or
/// `Rating::view(props)` through [`CustomView`].
///
/// `P` is the props: `Value<W::Props>`, or `()` for a `<Rating …>` tag in
/// `view!` whose `props=…` isn't set yet. Only a widget with props is a
/// [`View`].
pub struct Custom<W: CustomWidget, P = Value<<W as CustomWidget>::Props>> {
    element: Element,
    props: P,
    renderer: Renderer<W>,
    force_drawn: bool,
    force_composed: bool,
}

impl<W: Render> Custom<W> {
    /// Props are usually reactive: `move || RatingProps { value: rating.get(), .. }`.
    pub fn new(props: impl IntoValue<W::Props>) -> Custom<W> {
        Custom {
            element: Element::new(WidgetKind::Custom(W::NAME)),
            props: props.into_value(),
            renderer: W::renderer(),
            force_drawn: false,
            force_composed: false,
        }
    }
}

impl<W: CustomWidget, P> Custom<W, P> {
    /// Sets the props; usually reactive: `move || RatingProps { … }`.
    pub fn props(self, props: impl IntoValue<W::Props>) -> Custom<W> {
        let Custom { element, props: _, renderer, force_drawn, force_composed } = self;
        Custom { element, props: props.into_value(), renderer, force_drawn, force_composed }
    }

    /// Uses the drawn render even where a native one exists.
    pub fn drawn(mut self) -> Custom<W, P>
    where
        W: Drawn,
    {
        self.renderer.drawn = Some(DrawnFns::of::<W>());
        self.force_drawn = true;
        self
    }

    /// Uses the composed render even where another one exists. Panics at
    /// build time if the widget has none.
    pub fn composed(mut self) -> Custom<W, P> {
        self.force_composed = true;
        self
    }

    /// Called with each event the widget emits.
    pub fn on_event(mut self, handler: impl Fn(&W::Event) + 'static) -> Custom<W, P> {
        self.element.on(move |event| {
            if let UiEvent::Custom(value) = event
                && let Some(event) = value.downcast_ref::<W::Event>()
            {
                handler(event);
            }
        });
        self
    }
}

impl<W: CustomWidget, P> ElementBuilder for Custom<W, P> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<W: CustomWidget> View for Custom<W> {
    fn build(self, ui: &Ui) -> NodeId {
        let Custom { mut element, props, renderer, force_drawn, force_composed } = self;
        let composed = force_composed || (!force_drawn && renderer.native.is_none() && renderer.drawn.is_none());
        if composed {
            let compose = renderer.composed.unwrap_or_else(|| panic!("{}: no composed render", W::NAME));
            return build_composed(ui, element, props, compose);
        }
        let definition = Rc::new(Definition {
            name: W::NAME,
            a11y: |props| W::a11y(props_of::<W>(props)),
            action: |props, action| W::action(props_of::<W>(props), action).map(AnyValue::new),
            native: renderer.native,
            drawn: renderer.drawn,
            force_drawn,
        });
        element.prop(props, move |props| {
            Prop::Custom(CustomProps { props: AnyValue::new(props), definition: definition.clone() })
        });
        element.build(ui)
    }
}

/// The composed tier: a plain container around what the render builds. The
/// app's accessible label goes to the render (onto the control that stands
/// for the widget); styles, test id and handlers stay on the container.
fn build_composed<W: CustomWidget>(
    ui: &Ui,
    mut element: Element,
    props: Value<W::Props>,
    compose: ComposeFn<W>,
) -> NodeId {
    element.kind = WidgetKind::Container;
    let label = element.a11y.label.take();
    let props: Rc<dyn Fn() -> W::Props> = match props {
        Value::Static(props) => Rc::new(move || props.clone()),
        Value::Dynamic(props) => props,
    };
    // Events go to the container's handlers, the widget's `on_event` ones.
    let node: Rc<std::cell::Cell<Option<NodeId>>> = Rc::default();
    let (target, weak) = (node.clone(), ui.downgrade());
    let emit = Rc::new(move |event: W::Event| {
        if let (Some(id), Some(ui)) = (target.get(), weak.upgrade()) {
            ui.events().emit(id, UiEvent::Custom(AnyValue::new(event)));
        }
    });
    element.add_children(compose(Composed { props, emit, label }));
    let id = element.build(ui);
    node.set(Some(id));
    id
}

/// `Rating::view(props)`: shorthand for `Custom::<Rating>::new(props)`.
pub trait CustomView: Render {
    fn view(props: impl IntoValue<Self::Props>) -> Custom<Self> {
        Custom::new(props)
    }

    /// `<Rating props=… @event=…/>` in `view!`: the widget before its props.
    #[doc(hidden)]
    fn __tag() -> Custom<Self, ()> {
        Custom {
            element: Element::new(WidgetKind::Custom(Self::NAME)),
            props: (),
            renderer: Self::renderer(),
            force_drawn: false,
            force_composed: false,
        }
    }
}

impl<W: Render> CustomView for W {}
