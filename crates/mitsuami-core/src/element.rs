//! The generic builder every widget is made of.

use std::rc::Rc;

use mitsuami_reactive::{IntoValue, Value, effect};

use crate::a11y::{A11yProps, Role};
use crate::command::UiEvent;
use crate::measure::NodeRef;
use crate::services::{Menu, MenuEntries, install_context_menu};
use crate::style::{Align, Edges, GridPlacement, Position, Style, TextDirection};
use crate::ui::{Handler, Ui};
use crate::units::Length;
use crate::view::{AnyView, Children, View};
use crate::widget::{NodeId, Prop, WidgetKind};

type Binder = Box<dyn FnOnce(&Ui, NodeId)>;

/// A node description: kind, style, props (static or reactive), handlers,
/// semantics and children. Widgets wrap an `Element` and add typed methods.
pub struct Element {
    pub kind: WidgetKind,
    pub style: Style,
    pub a11y: A11yProps,
    pub test_id: Option<String>,
    pub tab_index: Option<u32>,
    static_props: Vec<Prop>,
    binders: Vec<Binder>,
    handlers: Vec<Handler>,
    children: Vec<AnyView>,
}

impl Element {
    pub fn new(kind: WidgetKind) -> Element {
        Element {
            kind,
            style: Style::default(),
            a11y: A11yProps::default(),
            test_id: None,
            tab_index: None,
            static_props: Vec::new(),
            binders: Vec::new(),
            handlers: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Sets a prop from a static or reactive value. Reactive values get an
    /// effect that updates the native widget whenever they change.
    pub fn prop<T: Clone + 'static>(&mut self, value: Value<T>, to_prop: impl Fn(T) -> Prop + 'static) {
        match value {
            Value::Static(v) => {
                let prop = to_prop(v);
                self.static_props.retain(|p| p.key() != prop.key());
                self.static_props.push(prop);
            }
            Value::Dynamic(f) => self.binders.push(Box::new(move |ui, id| {
                let ui = ui.clone();
                effect(move || ui.set_prop(id, to_prop(f())));
            })),
        }
    }

    /// Sets part of the style from a static or reactive value.
    pub fn style_prop<T: Clone + 'static>(&mut self, value: Value<T>, apply: impl Fn(&mut Style, T) + 'static) {
        match value {
            Value::Static(v) => apply(&mut self.style, v),
            Value::Dynamic(f) => self.binders.push(Box::new(move |ui, id| {
                let ui = ui.clone();
                effect(move || {
                    let v = f();
                    ui.update_style(id, |s| apply(s, v));
                });
            })),
        }
    }

    pub fn on(&mut self, handler: impl Fn(&UiEvent) + 'static) {
        self.handlers.push(Rc::new(handler));
    }

    /// Runs `f` right after the node is created, in the building scope.
    pub fn after_build(&mut self, f: impl FnOnce(&Ui, NodeId) + 'static) {
        self.binders.push(Box::new(f));
    }

    pub fn add_children(&mut self, children: impl Children) {
        children.into_views(&mut self.children);
    }
}

impl View for Element {
    fn build(self, ui: &Ui) -> NodeId {
        let id = ui.create(self.kind, self.static_props);
        ui.set_style(id, self.style);
        if !self.a11y.is_empty() {
            ui.set_a11y(id, self.a11y);
        }
        if let Some(test_id) = self.test_id {
            ui.set_test_id(id, test_id);
        }
        if self.tab_index.is_some() {
            ui.set_tab_index(id, self.tab_index);
        }
        for handler in self.handlers {
            ui.on_event(id, move |e| handler(e));
        }
        for binder in self.binders {
            binder(ui, id);
        }
        for child in self.children {
            let child = child.build(ui);
            ui.append_child(id, child);
        }
        id
    }
}

/// Defines style setters that take static or reactive values:
/// `.width(200)`, `.width(width_signal)`, `.width(move || …)`.
macro_rules! style_setters {
    ($( $(#[$doc:meta])* fn $name:ident($ty:ty) => |$s:ident, $v:ident| $body:expr; )*) => {$(
        $(#[$doc])*
        fn $name(mut self, value: impl IntoValue<$ty>) -> Self {
            self.element().style_prop(value.into_value(), |$s: &mut Style, $v: $ty| $body);
            self
        }
    )*};
}

/// Style, semantics and test hooks shared by every widget builder.
///
/// Every style setter accepts a literal, a signal or a closure; reactive
/// values update the layout whenever they change.
pub trait ElementBuilder: Sized {
    fn element(&mut self) -> &mut Element;

    /// Escape hatch: edit the style directly (static).
    fn style(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(&mut self.element().style);
        self
    }

    /// Escape hatch: edit the style reactively. `f` re-runs whenever the
    /// signals it reads change, and must set every field it cares about.
    fn style_with(mut self, f: impl Fn(&mut Style) + 'static) -> Self {
        self.element().style_prop(Value::Dynamic(Rc::new(|| ())), move |s, ()| f(s));
        self
    }

    style_setters! {
        fn width(Length) => |s, v| s.width = v;
        fn height(Length) => |s, v| s.height = v;
        fn min_width(Length) => |s, v| s.min_width = v;
        fn min_height(Length) => |s, v| s.min_height = v;
        fn max_width(Length) => |s, v| s.max_width = v;
        fn max_height(Length) => |s, v| s.max_height = v;
        fn aspect_ratio(f32) => |s, v| s.aspect_ratio = Some(v);

        fn padding(Length) => |s, v| s.padding = Edges::all(v);
        fn padding_x(Length) => |s, v| { s.padding.start = v; s.padding.end = v; };
        fn padding_y(Length) => |s, v| { s.padding.top = v; s.padding.bottom = v; };
        fn padding_start(Length) => |s, v| s.padding.start = v;
        fn padding_end(Length) => |s, v| s.padding.end = v;
        fn padding_top(Length) => |s, v| s.padding.top = v;
        fn padding_bottom(Length) => |s, v| s.padding.bottom = v;

        fn margin(Length) => |s, v| s.margin = Edges::all(v);
        fn margin_x(Length) => |s, v| { s.margin.start = v; s.margin.end = v; };
        fn margin_y(Length) => |s, v| { s.margin.top = v; s.margin.bottom = v; };
        fn margin_start(Length) => |s, v| s.margin.start = v;
        fn margin_end(Length) => |s, v| s.margin.end = v;
        fn margin_top(Length) => |s, v| s.margin.top = v;
        fn margin_bottom(Length) => |s, v| s.margin.bottom = v;

        fn grow(f32) => |s, v| s.flex_grow = v;
        fn shrink(f32) => |s, v| s.flex_shrink = v;
        fn basis(Length) => |s, v| s.flex_basis = v;
        fn align_self(Align) => |s, v| s.align_self = Some(v);
        /// Alignment inside a grid cell along the inline axis.
        fn justify_self(Align) => |s, v| s.justify_self = Some(v);

        /// Inset from the top of the containing block (with `absolute()`).
        fn top(Length) => |s, v| s.inset.top = v;
        fn bottom(Length) => |s, v| s.inset.bottom = v;
        /// Inset from the inline start (left in LTR, right in RTL).
        fn start(Length) => |s, v| s.inset.start = v;
        fn end(Length) => |s, v| s.inset.end = v;

        fn grid_column(GridPlacement) => |s, v| s.grid_column = v;
        fn grid_row(GridPlacement) => |s, v| s.grid_row = v;

        fn direction(TextDirection) => |s, v| s.direction = v;
        /// Not laid out, not visible, not focusable, not in the a11y tree.
        /// Un-hiding restores whatever `display` the node had.
        fn hidden(bool) => |s, v| s.hidden = v;
    }

    fn size(self, width: impl IntoValue<Length>, height: impl IntoValue<Length>) -> Self {
        self.width(width).height(height)
    }

    /// Takes the node out of flow; position it with `top`/`start`/….
    fn absolute(self) -> Self {
        self.style(|s| s.position = Position::Absolute)
    }

    /// A tooltip: text the platform shows, its own way, when the pointer
    /// rests on the widget, and assistive technology reads as its
    /// description. Empty: none.
    fn tooltip(mut self, text: impl IntoValue<String>) -> Self {
        self.element().prop(text.into_value(), Prop::Tooltip);
        self
    }

    /// A context menu: the platform shows it its own way, on a right-click,
    /// a long press or the menu key, and over children without one of
    /// their own. Takes items, separators and submenus, as a [`Menu`]
    /// does, with the same reactive titles, states and check marks:
    ///
    /// ```ignore
    /// Text::new(name).context_menu((
    ///     MenuItem::new("Rename").on_select(rename),
    ///     MenuSeparator,
    ///     MenuItem::new("Delete").on_select(delete),
    /// ))
    /// ```
    fn context_menu(mut self, entries: impl MenuEntries) -> Self {
        let menu = Menu::new(String::new()).children(entries);
        self.element().after_build(move |ui, id| install_context_menu(ui, id, menu));
        self
    }

    fn a11y_label(mut self, label: impl Into<String>) -> Self {
        self.element().a11y.label = Some(label.into());
        self
    }
    fn a11y_description(mut self, description: impl Into<String>) -> Self {
        self.element().a11y.description = Some(description.into());
        self
    }
    fn a11y_role(mut self, role: Role) -> Self {
        self.element().a11y.role = Some(role);
        self
    }
    fn a11y_hidden(mut self) -> Self {
        self.element().a11y.hidden = true;
        self
    }
    /// Called when this control gains keyboard focus.
    fn on_focus(mut self, handler: impl Fn() + 'static) -> Self {
        self.element().on(move |event| {
            if *event == UiEvent::FocusIn {
                handler();
            }
        });
        self
    }

    /// Called when this control loses keyboard focus.
    fn on_blur(mut self, handler: impl Fn() + 'static) -> Self {
        self.element().on(move |event| {
            if *event == UiEvent::FocusOut {
                handler();
            }
        });
        self
    }

    /// Moves this control ahead in the Tab order. Controls with a tab index
    /// come first (lowest first); the rest follow in reading order. Prefer
    /// arranging the tree in the order users should visit it.
    fn tab_index(mut self, index: u32) -> Self {
        self.element().tab_index = Some(index);
        self
    }

    /// Points `node` at this widget's node, for [`use_size`](crate::use_size).
    fn node_ref(mut self, node: NodeRef) -> Self {
        self.element().after_build(move |_, id| node.attach(id));
        self
    }

    /// Last-resort handle for tests; prefer finding nodes by role and name.
    fn test_id(mut self, id: impl Into<String>) -> Self {
        self.element().test_id = Some(id.into());
        self
    }
}

impl ElementBuilder for Element {
    fn element(&mut self) -> &mut Element {
        self
    }
}
