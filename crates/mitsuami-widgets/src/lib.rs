//! Built-in widgets. Platform-free: each is an [`Element`] with typed props,
//! events and accessibility defaults. Backends decide how they look.

use std::rc::Rc;

use mitsuami_core::{
    Align, AnyView, ButtonRole, ButtonStyle, Children, CurrentWindow, Display, Element, ElementBuilder, EventValue,
    FlexDirection, ImageFit, ImageSource, Justify, Length, Modality, NodeId, Orientation, Pixels, Point, Prop,
    ScrollAxes, TextStyle, Track, Tweak, Ui, UiEvent, View, WidgetKind, WindowSize,
};
use mitsuami_reactive::{IntoValue, Signal, Value, computed, effect, inject, on_cleanup, provide, untrack};

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

// ---------------------------------------------------------------- windows

/// A window the app opens while it runs: shown while `open` is true, with
/// its content built when it opens and disposed when it closes. Declare it
/// anywhere in the tree, next to the state that opens it; it closes with
/// the scope that declared it, and belongs to the window it's declared in.
///
/// The close button asks: [`bind`](Self::bind) closes it by setting the
/// flag, or [`on_close_request`](Self::on_close_request) lets the app
/// decide (to ask about unsaved changes, say). Without either, the close
/// button does nothing.
///
/// ```ignore
/// let editing = signal(false);
/// Column::new().children((
///     Button::new("Edit…").on_click(move || editing.set(true)),
///     Window::new("Machine").bind(editing).content(|| machine_form()),
/// ))
/// ```
pub struct Window {
    title: Value<String>,
    size: WindowSize,
    modality: Value<Option<Modality>>,
    open: Value<bool>,
    on_close_request: Option<Rc<dyn Fn()>>,
    content: Option<Rc<dyn Fn() -> AnyView>>,
}

impl Window {
    /// Open, 480 wide, as tall as its content.
    pub fn new(title: impl IntoValue<String>) -> Window {
        Window {
            title: title.into_value(),
            size: WindowSize::FitHeight(480.0),
            modality: Value::Static(None),
            open: Value::Static(true),
            on_close_request: None,
            content: None,
        }
    }

    /// Its content size when it opens: a [`Size`](mitsuami_core::Size),
    /// or [`WindowSize::FitHeight`] to fit the height to the content.
    pub fn size(mut self, size: impl Into<WindowSize>) -> Window {
        self.size = size.into();
        self
    }

    /// Modal: while it's open, it blocks the window it's declared in
    /// ([`Modality::Window`], a sheet on macOS) or the whole app
    /// ([`Modality::Application`]), and stays above that window. Set
    /// before it opens. A sheet has no close button: give its content
    /// one, e.g. a `ButtonRole::Cancel` button, which Escape presses.
    pub fn modal(mut self, modality: Modality) -> Window {
        self.modality = Value::Static(Some(modality));
        self
    }

    /// Modal or not, as a value that can change: read each time it opens,
    /// so a change applies at the next opening.
    pub fn modality(mut self, modality: impl IntoValue<Option<Modality>>) -> Window {
        self.modality = modality.into_value();
        self
    }

    /// Shown while true.
    pub fn open(mut self, open: impl IntoValue<bool>) -> Window {
        self.open = open.into_value();
        self
    }

    /// Shown while the signal is true; the close button sets it false.
    pub fn bind(self, open: Signal<bool>) -> Window {
        self.open(open).on_close_request(move || open.set(false))
    }

    /// Called when the user asks to close it (the close button, ⌘W, Alt+F4).
    /// It stays open unless the app closes it.
    pub fn on_close_request(mut self, handler: impl Fn() + 'static) -> Window {
        self.on_close_request = Some(Rc::new(handler));
        self
    }

    /// What it shows, built each time it opens.
    pub fn content<V: View>(mut self, content: impl Fn() -> V + 'static) -> Window {
        self.content = Some(Rc::new(move || AnyView::new(content())));
        self
    }
}

impl View for Window {
    /// A placeholder in the tree, which takes no room; the window is
    /// top-level.
    fn build(self, ui: &Ui) -> NodeId {
        let placeholder = ui.create(WidgetKind::Fragment, Vec::new());
        let Window { title, size, modality, open, on_close_request, content } = self;
        // The window it's declared in, which a modal window belongs to.
        let owner = inject::<CurrentWindow>().map(|w| w.0);
        let ui = ui.clone();
        let show = move || {
            let window = ui.create_window(String::new(), size);
            if let Some(modality) = modality.get() {
                // With no window to block, it blocks the app.
                let modality = if owner.is_some() { modality } else { Modality::Application };
                ui.set_prop(window, Prop::Modal { owner, modality });
            }
            let title = title.clone();
            let titled = ui.clone();
            effect(move || titled.set_prop(window, Prop::Title(title.get())));
            provide(CurrentWindow(window));
            if let Some(content) = &content {
                let root = content().build(&ui);
                ui.append_child(window, root);
            }
            if let Some(handler) = on_close_request.clone() {
                ui.on_event(window, move |event| {
                    if *event == UiEvent::WindowCloseRequested {
                        handler();
                    }
                });
            }
            let ui = ui.clone();
            on_cleanup(move || ui.destroy(window));
        };
        // In a scope of its own even when it's always open, so what it
        // provides (its `CurrentWindow`) stays inside it.
        let open = computed(move || open.get());
        effect(move || {
            if open.get() {
                untrack(&show);
            }
        });
        placeholder
    }
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
/// Also as in CSS, a flex item doesn't shrink below its content's width:
/// a horizontal scroll view in a column that grows in a row needs
/// `.min_width(0)` on that column, or both are as wide as the content, and
/// there's nothing to scroll.
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

    /// Whether it shows scroll bars, on by default. They're the platform's,
    /// shown as it shows them (overlay bars, say, as the user set on
    /// macOS). Without, it still scrolls by wheel, trackpad and touch, as a
    /// strip of photos or a carousel does.
    pub fn scroll_bars(mut self, show: impl IntoValue<bool>) -> ScrollView {
        self.outer.prop(show.into_value(), Prop::ScrollBars);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (elasticity and a border on AppKit, classic
    /// scroll bars on GTK, overshoot on Qt, always-expanded scroll bars and
    /// zoom on WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<ScrollView>) -> ScrollView {
        tweak.apply(&mut self.outer);
        self
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

    /// Shows at most this many lines, the last one cut off with an
    /// ellipsis where the text goes on, as the platform draws one; 0 shows
    /// them all. It's still read out in full.
    pub fn max_lines(mut self, lines: impl IntoValue<u32>) -> Text {
        self.0.prop(lines.into_value(), |n| Prop::MaxLines((n > 0).then_some(n)));
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (colours on AppKit and GTK, Markdown on Qt,
    /// character spacing on WinUI, selection on all but Qt's labels) is
    /// each one's own.
    pub fn native(mut self, tweak: Tweak<Text>) -> Text {
        tweak.apply(&mut self.0);
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

    /// What the button does in its window: see [`ButtonRole`].
    pub fn role(mut self, role: impl IntoValue<ButtonRole>) -> Button {
        self.0.prop(role.into_value(), Prop::ButtonRole);
        self
    }

    /// How the button is drawn: see [`ButtonStyle`].
    pub fn button_style(mut self, style: impl IntoValue<ButtonStyle>) -> Button {
        self.0.prop(style.into_value(), Prop::ButtonStyle);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Button>) -> Button {
        tweak.apply(&mut self.0);
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

    /// Shows the text, which can still be selected, copied and focused, but
    /// not edited. Unlike a disabled field, it looks and reads as usual.
    pub fn read_only(mut self, read_only: impl IntoValue<bool>) -> TextInput {
        self.0.prop(read_only.into_value(), Prop::ReadOnly);
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

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (a borderless field on AppKit, icons on GTK, a
    /// length limit on GTK, Qt and WinUI, a header on WinUI) is each one's
    /// own.
    pub fn native(mut self, tweak: Tweak<TextInput>) -> TextInput {
        tweak.apply(&mut self.0);
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

/// Single-line entry of a password: the platform's password field, which
/// hides the text as the platform does (bullets, and a button to show it
/// where the platform has one) and never reads it out.
pub struct PasswordInput(Element);
widget!(PasswordInput);

impl Default for PasswordInput {
    fn default() -> PasswordInput {
        PasswordInput::new()
    }
}

impl PasswordInput {
    pub fn new() -> PasswordInput {
        PasswordInput(Element::new(WidgetKind::PasswordInput))
    }

    pub fn value(mut self, value: impl IntoValue<String>) -> PasswordInput {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> PasswordInput {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> PasswordInput {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> PasswordInput {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings: see [`Tweak`]. Password fields have no
    /// semantic options past a text field's: what the platforms offer
    /// (GTK's peek icon, WinUI's reveal mode, the bullet on Qt and WinUI)
    /// is each one's own.
    pub fn native(mut self, tweak: Tweak<PasswordInput>) -> PasswordInput {
        tweak.apply(&mut self.0);
        self
    }

    /// Called on every edit with the new text.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> PasswordInput {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Called when the user confirms (Return / Enter).
    pub fn on_submit(mut self, handler: impl Fn() + 'static) -> PasswordInput {
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

    /// Shows the mixed state, whatever `checked` says: some of what the box
    /// stands for is checked, as in a "Select all" box. A click leaves it,
    /// checked or not as the platform decides (AppKit checks it, GTK flips
    /// `checked`), and reports that with `on_change`: work out `mixed`
    /// again from there.
    pub fn mixed(mut self, mixed: impl IntoValue<bool>) -> Checkbox {
        self.0.prop(mixed.into_value(), Prop::Mixed);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Checkbox>) -> Checkbox {
        tweak.apply(&mut self.0);
        self
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

    /// Raw platform settings: see [`Tweak`]. Switches have no semantic
    /// options past `checked`: what the platforms offer (sizes, GTK's
    /// delayed state, WinUI's on and off text) is each one's own.
    pub fn native(mut self, tweak: Tweak<Switch>) -> Switch {
        tweak.apply(&mut self.0);
        self
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

    /// Raw platform settings: see [`Tweak`]. Selects have no semantic
    /// options past their options and choice: what the platforms offer (a
    /// borderless pop-up on AppKit and Qt, search on GTK, a header on
    /// WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<Select>) -> Select {
        tweak.apply(&mut self.element);
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

/// A slider: a number in a range, as the platform's slider shows it,
/// horizontal or vertical. Its label is its accessible name. What its step does is the platform's:
/// AppKit shows it as tick marks the knob stops at, WinUI snaps to it, GTK
/// and Qt move by it from the keyboard.
///
/// ```ignore
/// let volume = signal(50.0);
/// Slider::new("Volume").range(0.0, 100.0).step(10.0).bind(volume)
/// ```
pub struct Slider {
    element: Element,
    range: Value<(f64, f64)>,
    value: Value<f64>,
}

impl ElementBuilder for Slider {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Slider {
    fn build(mut self, ui: &Ui) -> NodeId {
        // Clamped as the native slider clamps it.
        let clamp = |v: f64, (min, max): (f64, f64)| v.max(min).min(max);
        let value = match (self.value, self.range.clone()) {
            (Value::Static(v), Value::Static(range)) => Value::Static(clamp(v, range)),
            (value, range) => Value::Dynamic(Rc::new(move || clamp(value.get(), range.get()))),
        };
        self.element.prop(self.range, |(min, max)| Prop::Range { min, max });
        self.element.prop(value, Prop::Number);
        self.element.build(ui)
    }
}

impl Slider {
    /// From 0 to 100, at 0.
    pub fn new(label: impl IntoValue<String>) -> Slider {
        let mut element = Element::new(WidgetKind::Slider);
        element.prop(label.into_value(), Prop::Label);
        Slider { element, range: Value::Static((0.0, 100.0)), value: Value::Static(0.0) }
    }

    pub fn range(mut self, min: f64, max: f64) -> Slider {
        self.range = Value::Static((min, max));
        self
    }

    /// A range that changes: `(min, max)`.
    pub fn range_with(mut self, range: impl IntoValue<(f64, f64)>) -> Slider {
        self.range = range.into_value();
        self
    }

    /// The step the platform snaps to (where its sliders snap) and moves by
    /// from the keyboard. Without one, the platform's default.
    pub fn step(mut self, step: impl IntoValue<f64>) -> Slider {
        let step = step.into_value();
        let step = match step {
            Value::Static(s) => Value::Static(Some(s)),
            dynamic => Value::Dynamic(Rc::new(move || Some(dynamic.get()))),
        };
        self.element.prop(step, Prop::Step);
        self
    }

    pub fn value(mut self, value: impl IntoValue<f64>) -> Slider {
        self.value = value.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<f64>) -> Slider {
        self.value(signal).on_change(move |value| signal.set(value))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> Slider {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Which way it runs; larger values are up when vertical.
    pub fn orientation(mut self, orientation: impl IntoValue<Orientation>) -> Slider {
        self.element.prop(orientation.into_value(), Prop::Orientation);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Slider>) -> Slider {
        tweak.apply(&mut self.element);
        self
    }

    /// Called with the new value as the user moves the slider.
    pub fn on_change(mut self, handler: impl Fn(f64) + 'static) -> Slider {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Number(value)) = event {
                handler(*value);
            }
        });
        self
    }
}

/// A field for a whole number, with buttons that step it up and down, as
/// the platform's spin box shows it. Its label is its accessible name; show
/// one beside it with a `Text`. Typing reports the number when the edit is
/// done (Return, or leaving the field), as each platform commits one.
/// Whole numbers in an `i32`, because Qt's spin box holds an `int`.
///
/// ```ignore
/// let memory = signal(64);
/// NumberInput::new("Memory (MB)").range(16, 512).step(16).bind(memory)
/// ```
pub struct NumberInput {
    element: Element,
    range: Value<(i32, i32)>,
    value: Value<i32>,
}

impl ElementBuilder for NumberInput {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for NumberInput {
    fn build(mut self, ui: &Ui) -> NodeId {
        // Clamped as the native spin box clamps it.
        let clamp = |v: i32, (min, max): (i32, i32)| v.max(min).min(max);
        let value = match (self.value, self.range.clone()) {
            (Value::Static(v), Value::Static(range)) => Value::Static(clamp(v, range)),
            (value, range) => Value::Dynamic(Rc::new(move || clamp(value.get(), range.get()))),
        };
        self.element.prop(self.range, |(min, max)| Prop::Range { min: min.into(), max: max.into() });
        self.element.prop(value, |v| Prop::Number(v.into()));
        self.element.build(ui)
    }
}

impl NumberInput {
    /// From 0 to 100, at 0.
    pub fn new(label: impl IntoValue<String>) -> NumberInput {
        let mut element = Element::new(WidgetKind::NumberInput);
        element.prop(label.into_value(), Prop::Label);
        NumberInput { element, range: Value::Static((0, 100)), value: Value::Static(0) }
    }

    pub fn range(mut self, min: i32, max: i32) -> NumberInput {
        self.range = Value::Static((min, max));
        self
    }

    /// A range that changes: `(min, max)`.
    pub fn range_with(mut self, range: impl IntoValue<(i32, i32)>) -> NumberInput {
        self.range = range.into_value();
        self
    }

    /// What the buttons and arrow keys add or take away. Without one, the
    /// platform's default (1 on every platform).
    pub fn step(mut self, step: impl IntoValue<i32>) -> NumberInput {
        let step = step.into_value();
        let step = match step {
            Value::Static(s) => Value::Static(Some(s.into())),
            dynamic => Value::Dynamic(Rc::new(move || Some(dynamic.get().into()))),
        };
        self.element.prop(step, Prop::Step);
        self
    }

    pub fn value(mut self, value: impl IntoValue<i32>) -> NumberInput {
        self.value = value.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<i32>) -> NumberInput {
        self.value(signal).on_change(move |value| signal.set(value))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> NumberInput {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<NumberInput>) -> NumberInput {
        tweak.apply(&mut self.element);
        self
    }

    /// Called with the new number when the user steps it or finishes
    /// typing one.
    pub fn on_change(mut self, handler: impl Fn(i32) + 'static) -> NumberInput {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Number(value)) = event {
                // Backends report whole numbers in range; `as` saturates.
                handler(value.round() as i32);
            }
        });
        self
    }
}

/// A progress bar, as the platform draws one: how far along a task is, from
/// 0 to 1, or, until a value is given (or while `indeterminate`), an
/// animated bar for work of unknown length. Its label is its accessible
/// name.
///
/// ```ignore
/// Progress::new("Upload").value(move || sent.get() / total)
/// ```
pub struct Progress {
    element: Element,
    value: Option<Value<f64>>,
    indeterminate: Value<bool>,
}

impl ElementBuilder for Progress {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Progress {
    fn build(mut self, ui: &Ui) -> NodeId {
        let progress = match (self.value, self.indeterminate) {
            (None, _) => Value::Static(None),
            (Some(Value::Static(v)), Value::Static(unknown)) => Value::Static((!unknown).then_some(v.clamp(0.0, 1.0))),
            (Some(value), unknown) => {
                Value::Dynamic(Rc::new(move || (!unknown.get()).then(|| value.get().clamp(0.0, 1.0))))
            }
        };
        self.element.prop(progress, Prop::Progress);
        self.element.build(ui)
    }
}

impl Progress {
    pub fn new(label: impl IntoValue<String>) -> Progress {
        let mut element = Element::new(WidgetKind::Progress);
        element.prop(label.into_value(), Prop::Label);
        Progress { element, value: None, indeterminate: Value::Static(false) }
    }

    /// How far along, from 0 to 1.
    pub fn value(mut self, value: impl IntoValue<f64>) -> Progress {
        self.value = Some(value.into_value());
        self
    }

    /// Shows work of unknown length instead of the value while true.
    pub fn indeterminate(mut self, indeterminate: impl IntoValue<bool>) -> Progress {
        self.indeterminate = indeterminate.into_value();
        self
    }

    /// Raw platform settings: see [`Tweak`]. Progress bars have no semantic
    /// options past the value: what the platforms offer (sizes on AppKit,
    /// text on GTK, paused and error states on WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<Progress>) -> Progress {
        tweak.apply(&mut self.element);
        self
    }
}

/// A picture, in the platform's image view: from a file, or from pixels
/// in memory. It's as large as the image (in points: pixels over their
/// scale) unless the layout sizes it; then `fit` says how it fills the
/// frame, or the platform does as it does by default. Its label is its
/// accessible name; without one it's decorative.
///
/// ```ignore
/// Image::file("logo.png").label("2ksbox")
/// Image::new(move || ImageSource::Pixels(frame.get())).label("Preview")
/// ```
pub struct Image(Element);

widget!(Image);

impl Image {
    pub fn new(source: impl IntoValue<ImageSource>) -> Image {
        let mut element = Element::new(WidgetKind::Image);
        element.prop(source.into_value(), Prop::Image);
        Image(element)
    }

    /// An image file, read by the platform.
    pub fn file(path: impl Into<std::path::PathBuf>) -> Image {
        Image::new(ImageSource::File(path.into()))
    }

    /// Pixels in memory.
    pub fn pixels(pixels: impl IntoValue<Pixels>) -> Image {
        let pixels = pixels.into_value();
        Image::new(match pixels {
            Value::Static(p) => Value::Static(ImageSource::Pixels(p)),
            dynamic => Value::Dynamic(Rc::new(move || ImageSource::Pixels(dynamic.get()))),
        })
    }

    /// Its accessible name: what the picture shows.
    pub fn label(mut self, label: impl IntoValue<String>) -> Image {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// How it fills a frame of another size than its own.
    pub fn fit(mut self, fit: impl IntoValue<ImageFit>) -> Image {
        self.0.prop(fit.into_value(), Prop::ImageFit);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Image>) -> Image {
        tweak.apply(&mut self.0);
        self
    }
}

/// A spinner, as the platform draws one, for work of unknown length: a
/// spinning `NSProgressIndicator`, `gtk::Spinner`, `ProgressRing`,
/// `QQC2.BusyIndicator`. It spins while running (from the start, unless
/// told otherwise), and shows nothing while stopped, keeping its place. Its
/// label is its accessible name.
///
/// ```ignore
/// Spinner::new("Loading").running(move || loading.get())
/// ```
pub struct Spinner {
    element: Element,
    running: Value<bool>,
}

impl ElementBuilder for Spinner {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Spinner {
    fn build(mut self, ui: &Ui) -> NodeId {
        self.element.prop(self.running, Prop::Running);
        self.element.build(ui)
    }
}

impl Spinner {
    pub fn new(label: impl IntoValue<String>) -> Spinner {
        let mut element = Element::new(WidgetKind::Spinner);
        element.prop(label.into_value(), Prop::Label);
        Spinner { element, running: Value::Static(true) }
    }

    /// Spins while true; shows nothing while false.
    pub fn running(mut self, running: impl IntoValue<bool>) -> Spinner {
        self.running = running.into_value();
        self
    }

    /// Raw platform settings: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Spinner>) -> Spinner {
        tweak.apply(&mut self.element);
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

impl PasswordInput {
    #[doc(hidden)]
    pub fn __tag() -> PasswordInput {
        PasswordInput::new()
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

impl Slider {
    /// `<Slider a11y_label="Volume" bind=volume/>`
    #[doc(hidden)]
    pub fn __tag() -> Slider {
        Slider {
            element: Element::new(WidgetKind::Slider),
            range: Value::Static((0.0, 100.0)),
            value: Value::Static(0.0),
        }
    }
}

impl Image {
    /// `<Image source=ImageSource::File("logo.png".into()) label="Logo"/>`
    #[doc(hidden)]
    pub fn __tag() -> Image {
        Image(Element::new(WidgetKind::Image))
    }

    #[doc(hidden)]
    pub fn source(mut self, source: impl IntoValue<ImageSource>) -> Image {
        self.0.prop(source.into_value(), Prop::Image);
        self
    }
}

impl NumberInput {
    /// `<NumberInput a11y_label="Copies" bind=copies/>`
    #[doc(hidden)]
    pub fn __tag() -> NumberInput {
        NumberInput {
            element: Element::new(WidgetKind::NumberInput),
            range: Value::Static((0, 100)),
            value: Value::Static(0),
        }
    }
}

impl Spinner {
    /// `<Spinner a11y_label="Loading" running=loading/>`
    #[doc(hidden)]
    pub fn __tag() -> Spinner {
        Spinner { element: Element::new(WidgetKind::Spinner), running: Value::Static(true) }
    }
}

impl Progress {
    /// `<Progress a11y_label="Upload" value=done/>`
    #[doc(hidden)]
    pub fn __tag() -> Progress {
        Progress { element: Element::new(WidgetKind::Progress), value: None, indeterminate: Value::Static(false) }
    }
}
