//! Checkboxes and switches.

use mitsuami_core::{Element, ElementBuilder, EventValue, NodeId, Prop, Tweak, Ui, UiEvent, View, WidgetKind};
use mitsuami_reactive::{IntoValue, Signal};

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

text_tag!(Checkbox, Label);
text_tag!(Switch, Label);
