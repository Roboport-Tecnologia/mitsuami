//! Static or reactive text.

use mitsuami_core::{
    Color, Element, ElementBuilder, FontWeight, NodeId, Prop, TextAlign, TextStyle, Truncation, Tweak, Ui, View,
    WidgetKind,
};
use mitsuami_reactive::{IntoValue, Value};

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

    /// Where the ellipsis goes when it's cut off at `max_lines`: in place
    /// of its start, its middle (to keep a path's file name, say) or its
    /// end, the default. AppKit, Qt and WinUI cut off a single line's
    /// start or middle only, and the end of more lines, as their own
    /// labels do. A selectable label on WinUI or KDE cuts off its end.
    pub fn truncation(mut self, truncation: impl IntoValue<Truncation>) -> Text {
        self.0.prop(truncation.into_value(), Prop::Truncation);
        self
    }

    /// Its colour. A semantic one (`Color::SecondaryLabel`, `Color::Error`,
    /// …) is the platform's own, and follows dark mode and high contrast;
    /// `Color::Rgba` is fixed, whatever the appearance.
    pub fn color(mut self, color: impl IntoValue<Color>) -> Text {
        self.0.prop(color.into_value(), Prop::TextColor);
        self
    }

    /// Its weight, in place of its text style's. A platform whose font
    /// lacks one uses the nearest it has.
    pub fn weight(mut self, weight: impl IntoValue<FontWeight>) -> Text {
        self.0.prop(weight.into_value(), Prop::FontWeight);
        self
    }

    pub fn italic(mut self, italic: impl IntoValue<bool>) -> Text {
        self.0.prop(italic.into_value(), Prop::Italic);
        self
    }

    /// Where its lines go across its frame: `Start` and `End` follow its
    /// direction (`TextDirection`). Like CSS's `text-align`, it shows only
    /// where the frame is wider than the text, e.g. stretched across a
    /// column, or wrapping.
    pub fn text_align(mut self, align: impl IntoValue<TextAlign>) -> Text {
        self.0.style_prop(align.into_value(), |s, align| s.text_align = Some(align));
        self
    }

    /// Its text can be selected and copied, as the platform's selectable
    /// labels are (Kirigami's `SelectableLabel` on KDE). Fixed when it's
    /// built: on KDE a selectable label is another item.
    pub fn selectable(mut self, selectable: bool) -> Text {
        self.0.prop(Value::Static(selectable), Prop::Selectable);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (style classes on GTK, Markdown on Qt, character
    /// spacing on WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<Text>) -> Text {
        tweak.apply(&mut self.0);
        self
    }
}

text_tag!(Text, Text);
