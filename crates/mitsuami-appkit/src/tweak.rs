//! Raw AppKit settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{Opaque, Tweak};
use mitsuami_widgets::{Button, Checkbox, Switch};
use objc2::DowncastTarget;
use objc2_app_kit::{NSButton, NSSwitch, NSView};

/// A built-in widget, and the AppKit control that shows it.
pub trait Tweakable {
    type Native: DowncastTarget;
}

impl Tweakable for Button {
    type Native = NSButton;
}

impl Tweakable for Checkbox {
    type Native = NSButton;
}

impl Tweakable for Switch {
    type Native = NSSwitch;
}

/// Settings as the backend runs them, on the node's view.
pub(crate) type TweakFn = Rc<dyn Fn(&NSView)>;

/// Raw settings for a built-in widget's AppKit control:
/// `appkit::tweak(|b: &NSButton| b.setControlSize(NSControlSize::Large))`.
/// See [`Tweak`].
pub fn tweak<W: Tweakable>(apply: impl Fn(&W::Native) + 'static) -> Tweak<W> {
    tweak_with(Value::Static(()), move |native, ()| apply(native))
}

/// Raw settings made from a value, applied again whenever it changes:
/// `appkit::tweak_with(tint, |b: &NSButton, tint| ..)`.
pub fn tweak_with<W: Tweakable, T: Clone + 'static>(
    value: impl IntoValue<T>,
    apply: impl Fn(&W::Native, &T) + 'static,
) -> Tweak<W> {
    let apply = Rc::new(apply);
    Tweak::new(value.into_value(), move |value| {
        let apply = apply.clone();
        let run: TweakFn = Rc::new(move |view: &NSView| {
            if let Some(native) = view.downcast_ref::<W::Native>() {
                apply(native, &value);
            }
        });
        Opaque::new("appkit tweak", run)
    })
}
