//! `Tweak`: raw platform settings for a built-in widget.

use std::marker::PhantomData;

use mitsuami_reactive::Value;

use crate::any_value::Opaque;
use crate::element::Element;
use crate::widget::Prop;

/// Sends the settings to an element, as a prop.
type SendFn = Box<dyn FnOnce(&mut Element)>;

/// Raw platform settings for a built-in widget `W`: the escape hatch past
/// its semantic props, for what only one platform has. Each backend makes
/// them with its `tweak` (or `tweak_with`), which gets the native control
/// itself; pick one per platform with `platform!`:
///
/// ```ignore
/// use mitsuami::{appkit, gtk};
///
/// Button::new("Share").native(platform! {
///     macos => appkit::tweak(|b: &appkit::objc2_app_kit::NSButton| {
///         b.setControlSize(appkit::objc2_app_kit::NSControlSize::Large)
///     }),
///     gtk => gtk::tweak(|b: &gtk::gtk::Button| b.add_css_class("circular")),
///     _ => Tweak::none(),
/// })
/// ```
///
/// A tweak runs after the widget's other props, and again whenever they
/// change, so what it sets wins; keep it idempotent. Headless tests don't
/// run it.
pub struct Tweak<W> {
    send: Option<SendFn>,
    widget: PhantomData<fn() -> W>,
}

impl<W> Tweak<W> {
    /// No settings: for platforms the app doesn't tweak.
    pub fn none() -> Tweak<W> {
        Tweak { send: None, widget: PhantomData }
    }

    /// For backends: settings made from a value, in the backend's own form,
    /// and made again whenever a dynamic value changes.
    pub fn new<T: Clone + 'static>(value: Value<T>, settings: impl Fn(T) -> Opaque + 'static) -> Tweak<W> {
        let send = move |element: &mut Element| element.prop(value, move |value| Prop::Tweak(settings(value)));
        Tweak { send: Some(Box::new(send)), widget: PhantomData }
    }

    /// For widgets: sends the settings as `Prop::Tweak`.
    pub fn apply(self, element: &mut Element) {
        if let Some(send) = self.send {
            send(element);
        }
    }
}
