//! Choosing one of a list of options: pop-up menus and radio groups.

use std::rc::Rc;

use mitsuami_core::{Element, ElementBuilder, EventValue, NodeId, Prop, Tweak, Ui, UiEvent, View, WidgetKind};
use mitsuami_reactive::{IntoValue, Signal, Value};

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

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Select label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Select {
        self.element.prop(label.into_value(), Prop::Label);
        self
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

/// Radio buttons, one for each of its options, to choose one from:
/// `NSButton`s of the radio type, `RadioButtons`, `gtk::CheckButton`s in a
/// group, `QQC2.RadioButton`s. Down a column, as each platform stacks
/// them. Its label is its accessible name; it isn't drawn, so put a `Text`
/// above it.
///
/// Unlike a `Select`, a radio group can have none chosen, as on every
/// platform: until the user picks one, if `selected` says `None` or an
/// index past the options. The user can't go back to none.
///
/// ```ignore
/// let size = signal(Some(1));
/// RadioGroup::new("Size").options(["Small", "Medium", "Large"]).bind(size)
/// ```
pub struct RadioGroup {
    element: Element,
    options: Value<Vec<String>>,
    selected: Value<Option<usize>>,
}

impl ElementBuilder for RadioGroup {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for RadioGroup {
    fn build(mut self, ui: &Ui) -> NodeId {
        let chosen = |index: Option<usize>, count: usize| index.filter(|i| *i < count);
        let selected = match (self.selected, self.options.clone()) {
            (Value::Static(index), Value::Static(options)) => Value::Static(chosen(index, options.len())),
            (index, options) => Value::Dynamic(Rc::new(move || chosen(index.get(), options.get().len()))),
        };
        self.element.prop(self.options, Prop::Options);
        self.element.prop(selected, Prop::SelectedIndex);
        self.element.build(ui)
    }
}

impl RadioGroup {
    pub fn new(label: impl IntoValue<String>) -> RadioGroup {
        let mut element = Element::new(WidgetKind::RadioGroup);
        element.prop(label.into_value(), Prop::Label);
        RadioGroup { element, options: Value::Static(Vec::new()), selected: Value::Static(None) }
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<RadioGroup label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> RadioGroup {
        self.element.prop(label.into_value(), Prop::Label);
        self
    }

    /// The options, in order: a list of strings, or a signal or closure
    /// giving one.
    pub fn options(mut self, options: impl IntoValue<Vec<String>>) -> RadioGroup {
        self.options = options.into_value();
        self
    }

    /// The index of the chosen option, `None` for none.
    pub fn selected(mut self, index: impl IntoValue<Option<usize>>) -> RadioGroup {
        self.selected = index.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<Option<usize>>) -> RadioGroup {
        self.selected(signal).on_change(move |index| signal.set(Some(index)))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> RadioGroup {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings: see [`Tweak`]. The tweak gets the group's
    /// container (an `NSStackView`, the `RadioButtons`, a `gtk::Box`, a
    /// QML `ColumnLayout`), which holds the buttons.
    pub fn native(mut self, tweak: Tweak<RadioGroup>) -> RadioGroup {
        tweak.apply(&mut self.element);
        self
    }

    /// Called with the option's index when the user chooses one.
    pub fn on_change(mut self, handler: impl Fn(usize) + 'static) -> RadioGroup {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Index(index)) = event {
                handler(*index);
            }
        });
        self
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

impl RadioGroup {
    /// `<RadioGroup a11y_label="Size" options=["Small", "Large"] bind=size/>`
    #[doc(hidden)]
    pub fn __tag() -> RadioGroup {
        RadioGroup {
            element: Element::new(WidgetKind::RadioGroup),
            options: Value::Static(Vec::new()),
            selected: Value::Static(None),
        }
    }
}
