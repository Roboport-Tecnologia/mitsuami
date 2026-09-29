//! Built-in widgets. Platform-free: each is an
//! [`Element`](mitsuami_core::Element) with typed props, events and
//! accessibility defaults. Backends decide how they look.

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

// ----------------------------------------------------------- view! tags
//
// `view!` builds `<Tag …>children</Tag>` as
// `Tag::__tag().….__children(move || children)`: containers take their
// children, text and buttons their text.

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

mod buttons;
mod choices;
mod containers;
mod media;
mod progress;
mod ranges;
mod scroll;
mod separator;
mod sidebar;
mod tabs;
mod text;
mod text_inputs;
mod toggles;
mod toolbar;
mod window;

pub use buttons::*;
pub use choices::*;
pub use containers::*;
pub use media::*;
pub use progress::*;
pub use ranges::*;
pub use scroll::*;
pub use separator::*;
pub use sidebar::*;
pub use tabs::*;
pub use text::*;
pub use text_inputs::*;
pub use toggles::*;
pub use toolbar::*;
pub use window::*;
