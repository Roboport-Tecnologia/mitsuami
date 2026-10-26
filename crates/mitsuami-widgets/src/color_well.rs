//! Colour wells.

use mitsuami_core::{Color, Element, ElementBuilder, EventValue, NodeId, Prop, Tweak, Ui, UiEvent, View, WidgetKind};
use mitsuami_reactive::{IntoValue, Signal};

/// A colour well: a button showing a colour, which opens the platform's
/// own way to pick another (AppKit's colour panel, GTK's colour dialog,
/// WinUI's colour picker, Qt's colour dialog). Its label is its accessible
/// name. Opaque colours only: a semantic colour or alpha isn't a well's.
///
/// ```ignore
/// let color = signal(Color::rgb(0, 120, 215));
/// ColorWell::new("Colour").bind(color)
/// ```
pub struct ColorWell(Element);
widget!(ColorWell);

impl ColorWell {
    /// The platform's own colour until it's given one.
    pub fn new(label: impl IntoValue<String>) -> ColorWell {
        let mut element = Element::new(WidgetKind::ColorWell);
        element.prop(label.into_value(), Prop::Label);
        ColorWell(element)
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<ColorWell label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> ColorWell {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// The colour it shows, made opaque.
    pub fn color(mut self, color: impl IntoValue<Color>) -> ColorWell {
        let opaque = |color: Color| match color {
            Color::Rgba(r, g, b, _) => Color::rgb(r, g, b),
            other => other,
        };
        self.0.prop(color.into_value(), move |c| Prop::ColorValue(opaque(c)));
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<Color>) -> ColorWell {
        self.color(signal).on_change(move |color| signal.set(color))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> ColorWell {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Called with the colour the user picked, as the platform reports
    /// picks: AppKit's panel and WinUI's picker as it's dragged, GTK's and
    /// Qt's dialogs once they're accepted.
    pub fn on_change(mut self, handler: impl Fn(Color) + 'static) -> ColorWell {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Color(color)) = event {
                handler(*color);
            }
        });
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<ColorWell>) -> ColorWell {
        tweak.apply(&mut self.0);
        self
    }

    /// `<ColorWell label="Colour" bind=color/>`
    #[doc(hidden)]
    pub fn __tag() -> ColorWell {
        ColorWell(Element::new(WidgetKind::ColorWell))
    }
}
