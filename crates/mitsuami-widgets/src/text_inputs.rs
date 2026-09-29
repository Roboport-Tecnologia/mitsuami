//! Text fields: single-line, password, search and multi-line.

use mitsuami_core::{
    Element, ElementBuilder, EventValue, InputPurpose, NodeId, Prop, Tweak, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value};

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

    /// What it's for (an email address, a URL, a phone number), which the
    /// platform uses as it uses one: to pick an on-screen keyboard, to
    /// offer autofill. It doesn't check what's typed.
    pub fn input_purpose(mut self, purpose: impl IntoValue<InputPurpose>) -> TextInput {
        self.0.prop(purpose.into_value(), Prop::InputPurpose);
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

/// Single-line entry of search text: the platform's search field, with its
/// search icon and clear button. [`on_search`](Self::on_search) is called
/// when the platform asks for a search, as its own apps search.
pub struct SearchInput(Element);
widget!(SearchInput);

impl Default for SearchInput {
    fn default() -> SearchInput {
        SearchInput::new()
    }
}

impl SearchInput {
    pub fn new() -> SearchInput {
        SearchInput(Element::new(WidgetKind::SearchInput))
    }

    pub fn value(mut self, value: impl IntoValue<String>) -> SearchInput {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> SearchInput {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    /// Shown while it's empty, in place of the platform's own where it has
    /// one (AppKit, Qt). It names the field to assistive technology
    /// without a label.
    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> SearchInput {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> SearchInput {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Called on every edit with the new text, the clear button's too.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> SearchInput {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Called with the text when the platform asks for a search: as the
    /// user types, after a pause where the platform waits for one (AppKit,
    /// GTK, Qt) and at once where it doesn't (WinUI); on Return; and when
    /// the field is cleared. Not for text the app set.
    pub fn on_search(mut self, handler: impl Fn(String) + 'static) -> SearchInput {
        self.0.on(move |event| {
            if let UiEvent::Search(text) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (recent searches on AppKit, GTK's search delay,
    /// Kirigami's longer one, a header on WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<SearchInput>) -> SearchInput {
        tweak.apply(&mut self.0);
        self
    }
}

/// Text over many lines: the platform's text area, whose lines wrap to its
/// width and where Return starts a new line. It's as tall as
/// [`lines`](Self::lines) of the platform's text, and scrolls past them.
pub struct TextArea(Element);
widget!(TextArea);

impl Default for TextArea {
    fn default() -> TextArea {
        TextArea::new()
    }
}

impl TextArea {
    /// Three lines tall.
    pub fn new() -> TextArea {
        let mut element = Element::new(WidgetKind::TextArea);
        element.prop(Value::Static(3), Prop::Lines);
        TextArea(element)
    }

    /// Its lines end in `\n`.
    pub fn value(mut self, value: impl IntoValue<String>) -> TextArea {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> TextArea {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    /// Shown while it's empty, where the platform's text areas show one
    /// (Qt, WinUI); AppKit's and GTK's have none. It still names the area
    /// to assistive technology without a label.
    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> TextArea {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> TextArea {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Shows the text, which can still be selected, copied and focused, but
    /// not edited. Unlike a disabled area, it looks and reads as usual.
    pub fn read_only(mut self, read_only: impl IntoValue<bool>) -> TextArea {
        self.0.prop(read_only.into_value(), Prop::ReadOnly);
        self
    }

    /// How many lines of text tall it is at its natural size: 3 unless
    /// set, and at least 1. The layout can still stretch or shrink it.
    pub fn lines(mut self, lines: impl IntoValue<u32>) -> TextArea {
        self.0.prop(lines.into_value(), |n| Prop::Lines(n.max(1)));
        self
    }

    /// Whether its lines wrap to its width, as they do unless it's set
    /// false. Without, each line is as long as its text, and the area
    /// scrolls sideways: for code, or logs.
    pub fn line_wrap(mut self, wrap: impl IntoValue<bool>) -> TextArea {
        self.0.prop(wrap.into_value(), Prop::LineWrap);
        self
    }

    /// Called on every edit with the new text.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> TextArea {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (spelling and substitutions on AppKit, GTK's
    /// wrap modes and monospace, Qt's text format, WinUI's spell check and
    /// header) is each one's own.
    pub fn native(mut self, tweak: Tweak<TextArea>) -> TextArea {
        tweak.apply(&mut self.0);
        self
    }
}

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

impl SearchInput {
    #[doc(hidden)]
    pub fn __tag() -> SearchInput {
        SearchInput::new()
    }
}

impl TextArea {
    #[doc(hidden)]
    pub fn __tag() -> TextArea {
        TextArea::new()
    }
}
