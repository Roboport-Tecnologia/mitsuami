//! Raw Qt Quick settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{Opaque, Tweak};
use mitsuami_widgets::{Button, Checkbox, Switch};

use crate::ffi::QmlObject;

/// A built-in widget, and what shows it: every widget is a QML item,
/// reached through a [`QmlObject`].
pub trait Tweakable {}

/// A `QQC2.Button`.
impl Tweakable for Button {}

/// A `QQC2.CheckBox`.
impl Tweakable for Checkbox {}

/// A `QQC2.Switch`.
impl Tweakable for Switch {}

/// Settings as the backend runs them, on the node's item.
pub(crate) type TweakFn = Rc<dyn Fn(QmlObject)>;

/// Raw settings for a built-in widget's QML item, by property name:
/// `kirigami::tweak(|b: &QmlObject| b.set_bool("checkable", true))`. See
/// [`Tweak`].
pub fn tweak<W: Tweakable>(apply: impl Fn(&QmlObject) + 'static) -> Tweak<W> {
    tweak_with(Value::Static(()), move |item, ()| apply(item))
}

/// Raw settings made from a value, applied again whenever it changes:
/// `kirigami::tweak_with(on, |b: &QmlObject, on| b.set_bool("checked", *on))`.
pub fn tweak_with<W: Tweakable, T: Clone + 'static>(
    value: impl IntoValue<T>,
    apply: impl Fn(&QmlObject, &T) + 'static,
) -> Tweak<W> {
    let apply = Rc::new(apply);
    Tweak::new(value.into_value(), move |value| {
        let apply = apply.clone();
        let run: TweakFn = Rc::new(move |item: QmlObject| apply(&item, &value));
        Opaque::new("qml tweak", run)
    })
}
