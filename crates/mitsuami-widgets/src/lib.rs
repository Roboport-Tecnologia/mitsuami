//! Built-in widgets. Platform-free: each is an [`Element`] with typed props,
//! events and accessibility defaults. Backends decide how they look.

use std::rc::Rc;

use mitsuami_core::{
    Align, ButtonVariant, Children, Display, Element, ElementBuilder, EventValue, FlexDirection, Justify, Length,
    NodeId, Point, Prop, ScrollAxes, TextStyle, Track, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value};

macro_rules! widget {
    ($t:ty) => {
        impl ElementBuilder for $t {
            fn element(&mut self) -> &mut Element {
                &mut self.0
            }
        }

        impl View for $t {
            fn build(self, ui: &Ui) -> NodeId {
                self.0.build(ui)
            }
        }
    };
}

// ------------------------------------------------------------- containers

/// A layout host: flexbox (column by default) or grid.
pub struct Container(Element);
widget!(Container);

impl Default for Container {
    fn default() -> Container {
        Container::new()
    }
}

impl Container {
    pub fn new() -> Container {
        Container(Element::new(WidgetKind::Container))
    }

    pub fn children(mut self, children: impl Children) -> Container {
        self.0.add_children(children);
        self
    }

    pub fn child(self, child: impl View) -> Container {
        self.children(child)
    }

    pub fn flex_direction(mut self, direction: impl IntoValue<FlexDirection>) -> Container {
        self.0.style_prop(direction.into_value(), |s, v| s.flex_direction = v);
        self
    }

    /// Gap between rows and columns.
    pub fn gap(mut self, gap: impl IntoValue<Length>) -> Container {
        self.0.style_prop(gap.into_value(), |s, v| {
            s.row_gap = v;
            s.column_gap = v;
        });
        self
    }

    pub fn row_gap(mut self, gap: impl IntoValue<Length>) -> Container {
        self.0.style_prop(gap.into_value(), |s, v| s.row_gap = v);
        self
    }

    pub fn column_gap(mut self, gap: impl IntoValue<Length>) -> Container {
        self.0.style_prop(gap.into_value(), |s, v| s.column_gap = v);
        self
    }

    /// Cross-axis alignment of children (`align-items`).
    pub fn align(mut self, align: impl IntoValue<Align>) -> Container {
        self.0.style_prop(align.into_value(), |s, v| s.align_items = Some(v));
        self
    }

    /// Main-axis distribution of children (`justify-content`).
    pub fn justify(mut self, justify: impl IntoValue<Justify>) -> Container {
        self.0.style_prop(justify.into_value(), |s, v| s.justify_content = Some(v));
        self
    }

    pub fn wrap(self) -> Container {
        self.style(|s| s.flex_wrap = true)
    }

    /// Switches to grid layout with these column tracks.
    pub fn columns<T: Into<Track>>(self, tracks: impl IntoIterator<Item = T>) -> Container {
        let tracks: Vec<Track> = tracks.into_iter().map(Into::into).collect();
        self.style(|s| {
            s.display = Display::Grid;
            s.grid_template_columns = tracks;
        })
    }

    /// Switches to grid layout with these row tracks.
    pub fn rows<T: Into<Track>>(self, tracks: impl IntoIterator<Item = T>) -> Container {
        let tracks: Vec<Track> = tracks.into_iter().map(Into::into).collect();
        self.style(|s| {
            s.display = Display::Grid;
            s.grid_template_rows = tracks;
        })
    }
}

/// Vertical flex container.
pub struct Column;

impl Column {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new().flex_direction(FlexDirection::Column)
    }
}

/// Horizontal flex container.
pub struct Row;

impl Row {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new().flex_direction(FlexDirection::Row)
    }
}

/// Grid container.
pub struct Grid;

impl Grid {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new().style(|s| s.display = Display::Grid)
    }
}

/// A native scroll container. Its children go into a content box that
/// keeps its natural size, so it can be larger than the scroll view.
///
/// Give the scroll view a bounded size (a fixed height, or `grow` inside a
/// sized parent); otherwise it grows with its content and never scrolls.
/// As in CSS, its natural size is its content's, so in a flex container its
/// siblings shrink along with it unless they have `.shrink(0.0)`.
pub struct ScrollView {
    outer: Element,
    content: Container,
}

impl ElementBuilder for ScrollView {
    fn element(&mut self) -> &mut Element {
        &mut self.outer
    }
}

impl View for ScrollView {
    fn build(mut self, ui: &Ui) -> NodeId {
        self.outer.add_children(self.content);
        self.outer.build(ui)
    }
}

impl Default for ScrollView {
    fn default() -> ScrollView {
        ScrollView::new()
    }
}

impl ScrollView {
    /// Scrolls vertically.
    pub fn new() -> ScrollView {
        ScrollView::with_axes(ScrollAxes::Vertical)
    }

    pub fn horizontal() -> ScrollView {
        ScrollView::with_axes(ScrollAxes::Horizontal)
    }

    pub fn both() -> ScrollView {
        ScrollView::with_axes(ScrollAxes::Both)
    }

    fn with_axes(axes: ScrollAxes) -> ScrollView {
        let mut outer = Element::new(WidgetKind::ScrollView);
        outer.prop(axes.into_value(), Prop::ScrollAxes);
        outer.style.scroll_x = axes.horizontal();
        outer.style.scroll_y = axes.vertical();
        // The content stretches across the non-scrolling axis and keeps its
        // natural size along the scrolling ones.
        outer.style.flex_direction =
            if axes == ScrollAxes::Horizontal { FlexDirection::Row } else { FlexDirection::Column };
        // Horizontal content flows in a row; the other kinds in a column.
        let mut content = Container::new().shrink(0.0);
        if axes == ScrollAxes::Horizontal {
            content = content.flex_direction(FlexDirection::Row);
        }
        if axes == ScrollAxes::Both {
            content = content.align_self(Align::Start);
        }
        ScrollView { outer, content }
    }

    pub fn children(mut self, children: impl Children) -> ScrollView {
        self.content = self.content.children(children);
        self
    }

    pub fn child(self, child: impl View) -> ScrollView {
        self.children(child)
    }

    /// Called with the new offset whenever the content scrolls.
    pub fn on_scroll(mut self, handler: impl Fn(Point) + 'static) -> ScrollView {
        self.outer.on(move |event| {
            if let UiEvent::Scrolled(offset) = event {
                handler(*offset);
            }
        });
        self
    }
}

// ----------------------------------------------------------------- leaves

/// Static or reactive text.
pub struct Text(Element);
widget!(Text);

impl Text {
    pub fn new(text: impl IntoValue<String>) -> Text {
        let mut element = Element::new(WidgetKind::Text);
        element.prop(text.into_value(), Prop::Text);
        Text(element)
    }

    pub fn text_style(mut self, style: impl IntoValue<TextStyle>) -> Text {
        self.0.prop(style.into_value(), Prop::TextStyle);
        self
    }
}

pub struct Button(Element);
widget!(Button);

impl Button {
    pub fn new(label: impl IntoValue<String>) -> Button {
        let mut element = Element::new(WidgetKind::Button);
        element.prop(label.into_value(), Prop::Label);
        Button(element)
    }

    pub fn variant(mut self, variant: impl IntoValue<ButtonVariant>) -> Button {
        self.0.prop(variant.into_value(), Prop::Variant);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> Button {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Button {
        self.0.on(move |event| {
            if *event == UiEvent::Click {
                handler();
            }
        });
        self
    }
}

/// Single-line text entry.
pub struct TextInput(Element);
widget!(TextInput);

impl Default for TextInput {
    fn default() -> TextInput {
        TextInput::new()
    }
}

impl TextInput {
    pub fn new() -> TextInput {
        TextInput(Element::new(WidgetKind::TextInput))
    }

    pub fn value(mut self, value: impl IntoValue<String>) -> TextInput {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> TextInput {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> TextInput {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> TextInput {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Called on every edit with the new text.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> TextInput {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Called when the user confirms (Return / Enter).
    pub fn on_submit(mut self, handler: impl Fn() + 'static) -> TextInput {
        self.0.on(move |event| {
            if *event == UiEvent::Submit {
                handler();
            }
        });
        self
    }
}

macro_rules! toggle {
    ($t:ident) => {
        impl $t {
            pub fn checked(mut self, checked: impl IntoValue<bool>) -> $t {
                self.0.prop(checked.into_value(), Prop::Checked);
                self
            }

            /// Two-way binding, Vue's `v-model`.
            pub fn bind(self, signal: Signal<bool>) -> $t {
                self.checked(signal).on_change(move |checked| signal.set(checked))
            }

            pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> $t {
                self.0.prop(enabled.into_value(), Prop::Enabled);
                self
            }

            pub fn on_change(mut self, handler: impl Fn(bool) + 'static) -> $t {
                self.0.on(move |event| {
                    if let UiEvent::Changed(EventValue::Bool(checked)) = event {
                        handler(*checked);
                    }
                });
                self
            }
        }
    };
}

pub struct Checkbox(Element);
widget!(Checkbox);
toggle!(Checkbox);

impl Checkbox {
    pub fn new(label: impl IntoValue<String>) -> Checkbox {
        let mut element = Element::new(WidgetKind::Checkbox);
        element.prop(label.into_value(), Prop::Label);
        Checkbox(element)
    }
}

/// On/off switch. Most platforms draw no caption; the label is its
/// accessible name.
pub struct Switch(Element);
widget!(Switch);
toggle!(Switch);

impl Switch {
    pub fn new(label: impl IntoValue<String>) -> Switch {
        let mut element = Element::new(WidgetKind::Switch);
        element.prop(label.into_value(), Prop::Label);
        Switch(element)
    }
}

/// A pop-up menu of text options to choose one from: `NSPopUpButton`,
/// `ComboBox`, `gtk::DropDown`, `QQC2.ComboBox`. Its label is its
/// accessible name; like a text field's, it isn't drawn, so put a `Text`
/// next to it.
///
/// As with HTML's `<select>`, one option is always chosen (GTK can't show
/// none): the first, unless `selected` says otherwise. An index past the
/// options chooses the first too. Only a `Select` without options has
/// nothing chosen.
///
/// ```ignore
/// let color = signal(0);
/// Select::new("Color").options(["Red", "Green", "Blue"]).bind(color)
/// ```
pub struct Select {
    element: Element,
    options: Value<Vec<String>>,
    selected: Value<usize>,
}

impl ElementBuilder for Select {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Select {
    fn build(mut self, ui: &Ui) -> NodeId {
        let chosen = |index: usize, count: usize| (count > 0).then_some(if index < count { index } else { 0 });
        let selected = match (self.selected, self.options.clone()) {
            (Value::Static(index), Value::Static(options)) => Value::Static(chosen(index, options.len())),
            (index, options) => Value::Dynamic(Rc::new(move || chosen(index.get(), options.get().len()))),
        };
        self.element.prop(self.options, Prop::Options);
        self.element.prop(selected, Prop::SelectedIndex);
        self.element.build(ui)
    }
}

impl Select {
    pub fn new(label: impl IntoValue<String>) -> Select {
        let mut element = Element::new(WidgetKind::Select);
        element.prop(label.into_value(), Prop::Label);
        Select { element, options: Value::Static(Vec::new()), selected: Value::Static(0) }
    }

    /// The options, in order: a list of strings, or a signal or closure
    /// giving one.
    pub fn options(mut self, options: impl IntoValue<Vec<String>>) -> Select {
        self.options = options.into_value();
        self
    }

    /// The index of the chosen option.
    pub fn selected(mut self, index: impl IntoValue<usize>) -> Select {
        self.selected = index.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<usize>) -> Select {
        self.selected(signal).on_change(move |index| signal.set(index))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> Select {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Called with the option's index when the user chooses one.
    pub fn on_change(mut self, handler: impl Fn(usize) + 'static) -> Select {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Index(index)) = event {
                handler(*index);
            }
        });
        self
    }
}

// ----------------------------------------------------------- view! tags
//
// `view!` builds `<Tag …>children</Tag>` as
// `Tag::__tag().….__children(move || children)`: containers take their
// children, text and buttons their text.

impl Container {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Container::new()
    }

    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> Container {
        self.children(children())
    }
}

impl Column {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Column::new()
    }
}

impl Row {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Row::new()
    }
}

impl Grid {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Grid::new()
    }
}

impl ScrollView {
    #[doc(hidden)]
    pub fn __tag() -> ScrollView {
        ScrollView::new()
    }

    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> ScrollView {
        self.children(children())
    }
}

/// Widgets whose one child is their text: `<Text>"Hello"</Text>`.
macro_rules! text_tag {
    ($t:ident, $prop:ident) => {
        impl $t {
            #[doc(hidden)]
            pub fn __tag() -> $t {
                $t(Element::new(WidgetKind::$t))
            }

            #[doc(hidden)]
            pub fn __children<S: IntoValue<String>>(mut self, text: impl FnOnce() -> S) -> $t {
                self.0.prop(text().into_value(), Prop::$prop);
                self
            }
        }
    };
}

text_tag!(Text, Text);
text_tag!(Button, Label);
text_tag!(Checkbox, Label);
text_tag!(Switch, Label);

impl TextInput {
    #[doc(hidden)]
    pub fn __tag() -> TextInput {
        TextInput::new()
    }
}

impl Select {
    /// `<Select a11y_label="Color" options=["Red", "Green"] bind=color/>`
    #[doc(hidden)]
    pub fn __tag() -> Select {
        Select {
            element: Element::new(WidgetKind::Select),
            options: Value::Static(Vec::new()),
            selected: Value::Static(0),
        }
    }
}
