//! Separator lines.

use mitsuami_core::{Element, ElementBuilder, NodeId, Orientation, Prop, Tweak, Ui, View, WidgetKind};
use mitsuami_reactive::{IntoValue, Value};

/// A line between groups of content, as the platform draws one: a
/// separator `NSBox`, `gtk::Separator`, `Kirigami.Separator`, and on
/// WinUI, which has no separator control, a `Border` in the divider brush,
/// as Fluent apps draw one. Horizontal unless told otherwise, as thick as
/// the platform makes it; its length is the layout's, so it spans a
/// column (or a row, vertical) that stretches its children, as they do
/// by default.
///
/// ```ignore
/// Column::new().children((general(), Separator::new(), advanced()))
/// Row::new().children((back(), Separator::vertical(), forward()))
/// ```
pub struct Separator(Element);

widget!(Separator);

impl Separator {
    pub fn new() -> Separator {
        let mut element = Element::new(WidgetKind::Separator);
        element.prop(Value::Static(Orientation::Horizontal), Prop::Orientation);
        Separator(element)
    }

    /// A vertical one, between things side by side.
    pub fn vertical() -> Separator {
        Separator::new().orientation(Orientation::Vertical)
    }

    /// Which way it runs.
    pub fn orientation(mut self, orientation: impl IntoValue<Orientation>) -> Separator {
        self.0.prop(orientation.into_value(), Prop::Orientation);
        self
    }

    /// Raw platform settings: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Separator>) -> Separator {
        tweak.apply(&mut self.0);
        self
    }
}

impl Default for Separator {
    fn default() -> Separator {
        Separator::new()
    }
}

impl Separator {
    /// `<Separator/>`, `<Separator orientation=Orientation::Vertical/>`
    #[doc(hidden)]
    pub fn __tag() -> Separator {
        Separator::new()
    }
}
