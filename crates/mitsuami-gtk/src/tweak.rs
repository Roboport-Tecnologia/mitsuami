//! Raw GTK settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use gtk::prelude::*;
use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{Opaque, Tweak};
use mitsuami_widgets::{Button, Checkbox, Progress, Select, Slider, Spinner, Switch};

/// A built-in widget, and the GTK widget that shows it.
pub trait Tweakable {
    type Native: IsA<gtk::Widget>;
}

impl Tweakable for Button {
    type Native = gtk::Button;
}

impl Tweakable for Checkbox {
    type Native = gtk::CheckButton;
}

impl Tweakable for Switch {
    type Native = gtk::Switch;
}

impl Tweakable for Select {
    type Native = gtk::DropDown;
}

impl Tweakable for Slider {
    type Native = gtk::Scale;
}

impl Tweakable for Progress {
    type Native = gtk::ProgressBar;
}

impl Tweakable for Spinner {
    type Native = gtk::Spinner;
}

/// Settings as the backend runs them, on the node's widget.
pub(crate) type TweakFn = Rc<dyn Fn(&gtk::Widget)>;

/// Raw settings for a built-in widget's GTK widget:
/// `gtk::tweak(|b: &gtk::Button| b.add_css_class("circular"))`. See
/// [`Tweak`].
pub fn tweak<W: Tweakable>(apply: impl Fn(&W::Native) + 'static) -> Tweak<W> {
    tweak_with(Value::Static(()), move |native, ()| apply(native))
}

/// Raw settings made from a value, applied again whenever it changes:
/// `gtk::tweak_with(pill, |b: &gtk::Button, pill| ..)`.
pub fn tweak_with<W: Tweakable, T: Clone + 'static>(
    value: impl IntoValue<T>,
    apply: impl Fn(&W::Native, &T) + 'static,
) -> Tweak<W> {
    let apply = Rc::new(apply);
    Tweak::new(value.into_value(), move |value| {
        let apply = apply.clone();
        let run: TweakFn = Rc::new(move |widget: &gtk::Widget| {
            if let Some(native) = widget.downcast_ref::<W::Native>() {
                apply(native, &value);
            }
        });
        Opaque::new("gtk tweak", run)
    })
}
