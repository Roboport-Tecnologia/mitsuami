//! Numbers in a range: sliders and number inputs.

use std::rc::Rc;

use mitsuami_core::{
    Element, ElementBuilder, EventValue, NodeId, Orientation, Prop, Tweak, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value};

/// A slider: a number in a range, as the platform's slider shows it,
/// horizontal or vertical. Its label is its accessible name. What its step does is the platform's:
/// AppKit shows it as tick marks the knob stops at, WinUI and GTK snap the
/// user's moves to it, and Qt moves by it from the keyboard.
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

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Slider label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Slider {
        self.element.prop(label.into_value(), Prop::Label);
        self
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

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<NumberInput label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> NumberInput {
        self.element.prop(label.into_value(), Prop::Label);
        self
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

    /// Stepped past one end of its range, it goes on from the other.
    /// Without, it's the platform's: AppKit's stepper wraps round, the
    /// other platforms' spin boxes stop.
    pub fn wrap_around(mut self, wrap: impl IntoValue<bool>) -> NumberInput {
        self.element.prop(wrap.into_value(), Prop::WrapAround);
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
