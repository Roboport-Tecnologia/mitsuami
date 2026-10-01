//! Raw XAML settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{List, Opaque, Table, Tweak};
use mitsuami_widgets::Sidebar;
use mitsuami_widgets::{
    Button, Checkbox, FileIcon, Group, Icon, Image, MenuButton, NumberInput, PasswordInput, Progress, RadioGroup,
    ScrollView, SearchInput, Select, Separator, Slider, Spinner, Switch, Text, TextArea, TextInput, ToggleButton,
};
use windows_core::Interface;

use crate::bindings as w;

/// A built-in widget, and the XAML control that shows it.
pub trait Tweakable {
    type Native: Interface;
}

impl Tweakable for Button {
    type Native = w::Button;
}

impl Tweakable for ToggleButton {
    type Native = w::ToggleButton;
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

impl Tweakable for RadioGroup {
    type Native = w::RadioButtons;
}

impl Tweakable for Slider {
    type Native = w::Slider;
}

impl Tweakable for NumberInput {
    type Native = w::NumberBox;
}

impl Tweakable for Image {
    type Native = w::Image;
}

impl Tweakable for Icon {
    type Native = w::FontIcon;
}

impl Tweakable for FileIcon {
    type Native = w::Image;
}

impl Tweakable for MenuButton {
    type Native = w::DropDownButton;
}

/// The card; its heading is a `TextBlock` over it.
impl Tweakable for Group {
    type Native = w::Border;
}

impl Tweakable for Progress {
    type Native = w::ProgressBar;
}

impl Tweakable for Spinner {
    type Native = w::ProgressRing;
}

/// The line: XAML has no separator control.
impl Tweakable for Separator {
    type Native = w::Border;
}

impl Tweakable for TextInput {
    type Native = w::TextBox;
}

/// A text box that takes Return.
impl Tweakable for TextArea {
    type Native = w::TextBox;
}

impl Tweakable for PasswordInput {
    type Native = w::PasswordBox;
}

/// An auto-suggest box with the find icon.
impl Tweakable for SearchInput {
    type Native = w::AutoSuggestBox;
}

impl Tweakable for ScrollView {
    type Native = w::ScrollViewer;
}

impl Tweakable for List {
    type Native = w::ListView;
}

/// The list view under the header.
impl Tweakable for Table {
    type Native = w::ListView;
}

/// The `NavigationView` whose pane it is.
impl<T> Tweakable for Sidebar<T> {
    type Native = w::NavigationView;
}

impl Tweakable for Text {
    type Native = w::TextBlock;
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

/// A list's or table's rows without XAML's item animations: `ListView`
/// slides and fades rows in as they're added, and again when they're all
/// replaced, as a file manager's are on every folder it opens. An app whose
/// rows change wholesale may prefer them to just show, as File Explorer's
/// do: `.native(winui::without_item_animations())`.
pub fn without_item_animations<W: Tweakable<Native = w::ListView>>() -> Tweak<W> {
    tweak(remove_item_animations)
}

/// What [`without_item_animations`] does, for a tweak of the app's own
/// that does more.
pub fn remove_item_animations(list: &w::ListView) -> windows_core::Result<()> {
    // An empty one of the list's own: the default style's collection is
    // shared by every list view. Set even when the list has none yet: the
    // first time, before the list is in the window, its style hasn't
    // given it its transitions, and a local value wins over the style's.
    let none: w::TransitionCollection =
        windows_core::factory::<w::TransitionCollection, windows_core::imp::IGenericFactory>()?.ActivateInstance()?;
    list.cast::<w::IItemsControl>()?.SetItemContainerTransitions(&none)
}
