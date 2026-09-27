//! Raw XAML settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{Opaque, Tweak};
use mitsuami_widgets::{Button, Checkbox, PasswordInput, Progress, Select, Slider, Spinner, Switch, TextInput};
use windows_core::Interface;

use crate::bindings as w;

/// A built-in widget, and the XAML control that shows it.
pub trait Tweakable {
    type Native: Interface;
}

impl Tweakable for Button {
    type Native = w::Button;
}

impl Tweakable for Checkbox {
    type Native = w::CheckBox;
}

impl Tweakable for Switch {
    type Native = w::ToggleSwitch;
}

impl Tweakable for Select {
    type Native = w::ComboBox;
}

impl Tweakable for Slider {
    type Native = w::Slider;
}

impl Tweakable for Progress {
    type Native = w::ProgressBar;
}

impl Tweakable for Spinner {
    type Native = w::ProgressRing;
}

impl Tweakable for TextInput {
    type Native = w::TextBox;
}

impl Tweakable for PasswordInput {
    type Native = w::PasswordBox;
}

/// Settings as the backend runs them, on the node's element.
pub(crate) type TweakFn = Rc<dyn Fn(&w::UIElement) -> windows_core::Result<()>>;

/// Raw settings for a built-in widget's XAML control:
/// `winui::tweak(|b: &Button| b.cast::<IControl>()?.SetCornerRadius(..))`.
/// See [`Tweak`].
pub fn tweak<W: Tweakable>(apply: impl Fn(&W::Native) -> windows_core::Result<()> + 'static) -> Tweak<W> {
    tweak_with(Value::Static(()), move |native, ()| apply(native))
}

/// Raw settings made from a value, applied again whenever it changes:
/// `winui::tweak_with(radius, |b: &Button, radius| ..)`.
pub fn tweak_with<W: Tweakable, T: Clone + 'static>(
    value: impl IntoValue<T>,
    apply: impl Fn(&W::Native, &T) -> windows_core::Result<()> + 'static,
) -> Tweak<W> {
    let apply = Rc::new(apply);
    Tweak::new(value.into_value(), move |value| {
        let apply = apply.clone();
        let run: TweakFn = Rc::new(move |element: &w::UIElement| apply(&element.cast::<W::Native>()?, &value));
        Opaque::new("xaml tweak", run)
    })
}
