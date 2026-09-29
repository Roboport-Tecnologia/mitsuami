//! Progress bars and spinners.

use std::rc::Rc;

use mitsuami_core::{Element, ElementBuilder, NodeId, Prop, Tweak, Ui, View, WidgetKind};
use mitsuami_reactive::{IntoValue, Value};

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

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Progress label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Progress {
        self.element.prop(label.into_value(), Prop::Label);
        self
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

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Spinner label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Spinner {
        self.element.prop(label.into_value(), Prop::Label);
        self
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
