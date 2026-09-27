//! Raw Qt Quick settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{List, Opaque, Tweak};
use mitsuami_widgets::{
    Button, Checkbox, NumberInput, PasswordInput, Progress, ScrollView, Select, Slider, Spinner, Switch, Text,
    TextInput,
};

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

/// A `QQC2.ComboBox`.
impl Tweakable for Select {}

/// A `QQC2.Slider`.
impl Tweakable for Slider {}

/// A `QQC2.SpinBox`.
impl Tweakable for NumberInput {}

/// A `QQC2.ProgressBar`.
impl Tweakable for Progress {}

/// A `QQC2.BusyIndicator`.
impl Tweakable for Spinner {}

/// A `QQC2.TextField`.
impl Tweakable for TextInput {}

/// A `Kirigami.PasswordField`, a `QQC2.TextField`.
impl Tweakable for PasswordInput {}

/// A `QQC2.ScrollView`, whose `contentItem` is the `Flickable` that
/// scrolls.
impl Tweakable for ScrollView {}

/// A QML `ListView`, in a `QQC2.ScrollView` (its parent's parent).
impl Tweakable for List {}

/// A `QQC2.Label`.
impl Tweakable for Text {}

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
