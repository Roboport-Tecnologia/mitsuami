//! Raw AppKit settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{List, Opaque, Table, Tweak};
use mitsuami_widgets::Sidebar;
use mitsuami_widgets::{
    Button, Checkbox, FileIcon, Group, Icon, Image, MenuButton, NumberInput, PasswordInput, Progress, RadioGroup,
    ScrollView, SearchInput, Select, Separator, Slider, Spinner, Switch, Text, TextArea, TextInput, ToggleButton,
};
use objc2::DowncastTarget;
use objc2_app_kit::{
    NSBox, NSButton, NSImageView, NSPopUpButton, NSProgressIndicator, NSScrollView, NSSearchField, NSSecureTextField,
    NSSlider, NSStackView, NSSwitch, NSTableView, NSTextField, NSTextView, NSView,
};

/// A built-in widget, and the AppKit control that shows it.
pub trait Tweakable {
    type Native: DowncastTarget;
}

impl Tweakable for Button {
    type Native = NSButton;
}

impl Tweakable for ToggleButton {
    type Native = NSButton;
}

impl Tweakable for Checkbox {
    type Native = NSButton;
}

impl Tweakable for Switch {
    type Native = NSSwitch;
}

impl Tweakable for Select {
    type Native = NSPopUpButton;
}

/// The stack view its radio buttons are in.
impl Tweakable for RadioGroup {
    type Native = NSStackView;
}

impl Tweakable for Slider {
    type Native = NSSlider;
}

impl Tweakable for NumberInput {
    type Native = crate::NumberField;
}

impl Tweakable for Image {
    type Native = NSImageView;
}

/// The box behind its children (the node's own view is a layout host).
impl Tweakable for Group {
    type Native = NSBox;
}

/// The pull-down pop-up button.
impl Tweakable for MenuButton {
    type Native = NSPopUpButton;
}

impl Tweakable for Icon {
    type Native = NSImageView;
}

impl Tweakable for FileIcon {
    type Native = NSImageView;
}

impl Tweakable for Progress {
    type Native = NSProgressIndicator;
}

impl Tweakable for Spinner {
    type Native = NSProgressIndicator;
}

/// A separator `NSBox`.
impl Tweakable for Separator {
    type Native = NSBox;
}

impl Tweakable for TextInput {
    type Native = NSTextField;
}

impl Tweakable for PasswordInput {
    type Native = NSSecureTextField;
}

impl Tweakable for SearchInput {
    type Native = NSSearchField;
}

/// The text view, not the scroll view around it: reach that with
/// `enclosingScrollView`.
impl Tweakable for TextArea {
    type Native = NSTextView;
}

impl Tweakable for ScrollView {
    type Native = NSScrollView;
}

impl Tweakable for List {
    type Native = NSTableView;
}

/// The table view, not the scroll view around it.
impl Tweakable for Table {
    type Native = NSTableView;
}

/// The source list's table view, not the scroll view around it: the split
/// view that sizes it is the window's.
impl<T> Tweakable for Sidebar<T> {
    type Native = NSTableView;
}

impl Tweakable for Text {
    type Native = NSTextField;
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
