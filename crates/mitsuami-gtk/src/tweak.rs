//! Raw GTK settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use gtk::prelude::*;
use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{List, Opaque, Table, Tweak};
use mitsuami_widgets::{
    Button, Checkbox, Group, Icon, Image, MenuButton, NumberInput, PasswordInput, Progress, RadioGroup, ScrollView,
    SearchInput, Select, Separator, Slider, Spinner, Switch, Text, TextArea, TextInput, ToggleButton,
};

use crate::backend::{STEPS, Steps, update_marks};

/// A built-in widget, and the GTK widget that shows it.
pub trait Tweakable {
    type Native: IsA<gtk::Widget>;
}

impl Tweakable for Button {
    type Native = gtk::Button;
}

impl Tweakable for ToggleButton {
    type Native = gtk::ToggleButton;
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

/// The box the check buttons are in.
impl Tweakable for RadioGroup {
    type Native = gtk::Box;
}

impl Tweakable for Slider {
    type Native = gtk::Scale;
}

impl Tweakable for NumberInput {
    type Native = gtk::SpinButton;
}

impl Tweakable for Image {
    type Native = gtk::Picture;
}

/// The card: the `gtk::Box` with libadwaita's `card` class.
impl Tweakable for Group {
    type Native = gtk::Box;
}

impl Tweakable for Icon {
    type Native = gtk::Image;
}

impl Tweakable for MenuButton {
    type Native = gtk::MenuButton;
}

impl Tweakable for Progress {
    type Native = gtk::ProgressBar;
}

impl Tweakable for Spinner {
    type Native = gtk::Spinner;
}

impl Tweakable for Separator {
    type Native = gtk::Separator;
}

impl Tweakable for TextInput {
    type Native = gtk::Entry;
}

impl Tweakable for PasswordInput {
    type Native = gtk::PasswordEntry;
}

impl Tweakable for SearchInput {
    type Native = gtk::SearchEntry;
}

/// The text view, not the scrolled window around it: reach that with
/// `parent()`.
impl Tweakable for TextArea {
    type Native = gtk::TextView;
}

impl Tweakable for ScrollView {
    type Native = gtk::ScrolledWindow;
}

impl Tweakable for List {
    type Native = gtk::ListView;
}

/// The column view, not the scrolled window around it.
impl Tweakable for Table {
    type Native = gtk::ColumnView;
}

impl Tweakable for Text {
    type Native = gtk::Label;
}

/// Whether a slider's scale shows a mark at each step; it does by default,
/// when the steps are far enough apart to drag between. For a tweak:
/// `gtk::tweak(|s: &gtk::Scale| gtk::show_step_marks(s, false))`.
pub fn show_step_marks(scale: &gtk::Scale, show: bool) {
    // SAFETY: the backend only ever keeps an `Rc<Steps>` under this key.
    let Some(steps) = (unsafe { scale.data::<Rc<Steps>>(STEPS) }) else { return };
    // SAFETY: the scale owns it, and it's only read here.
    let steps = unsafe { steps.as_ref() }.clone();
    // Tweaks run after every prop: only redraw when it changes.
    if steps.hidden.replace(!show) == show {
        update_marks(scale, &steps);
    }
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
