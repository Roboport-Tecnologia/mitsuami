//! Creating nodes.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{
    Command, EventValue, NodeId, Orientation, Point, Prop, ScrollAxes, UiEvent, WidgetKind, find_prop,
};
use windows_core::{IInspectable, Interface};

use super::fields::{report_text_changes, set_later, submit_on_enter};
use super::scroll::{report_offset, set_scrolling};
use super::styles::separator_style;
use super::{Node, R, State, Widget, key, violation};
use crate::bindings as w;
use crate::custom::{DrawnView, ErasedRender, NativePayload, WinUiCx};
use crate::surface::SurfaceHost;

impl State {
    pub(super) fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) -> R<()> {
        let emitter = self.emitter();
        let shown_text = Rc::new(RefCell::new(String::new()));
        let shown_checked = Rc::new(Cell::new(false));
        let shown_mixed = Rc::new(Cell::new(false));
        let shown_index = Rc::new(Cell::new(-1));
        let shown_number = Rc::new(Cell::new(0.0));
        let offset = Rc::new(Cell::new(Point::ZERO));
        let mut revokers = Vec::new();
        let mut inner = None;
        let (widget, element) = match kind {
            WidgetKind::Window => {
                let modal = match command {
                    Command::Create { props, .. } => props.iter().find_map(|p| match p {
                        Prop::Modal { owner, modality } => Some((*owner, *modality)),
                        _ => None,
                    }),
                    _ => None,
                };
                let (widget, element, window_revokers) = self.create_window(id, modal)?;
                revokers = window_revokers;
                (widget, element)
            }
            WidgetKind::Container | WidgetKind::ToolbarItem => {
                let canvas = w::Canvas::new()?;
                let element = canvas.cast()?;
                (Widget::Host(canvas), element)
            }
            WidgetKind::GpuSurface => {
                let surface = SurfaceHost::new(id, emitter.clone())?;
                let element = surface.canvas.cast()?;
                (Widget::GpuSurface(surface), element)
            }
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not a WinUI one")
                        };
                        let mut cx = WinUiCx::new(emitter.clone(), id);
                        let control = emitter.muted(|| render.create(custom.props(), &mut cx))?;
                        revokers.extend(cx.into_revokers());
                        let element = wrap(&control)?;
                        inner = Some(control);
                        (Widget::Custom { render, props: custom }, element)
                    }
                    None => {
                        let view = DrawnView::new(emitter.clone(), id, &mut revokers)?;
                        let element = view.canvas.cast()?;
                        (Widget::Drawn { view, props: custom }, element)
                    }
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not a WinUI one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let mut cx = WinUiCx::new(emitter.clone(), id);
                let control = emitter.muted(|| -> R<w::UIElement> {
                    let control = create(&mut cx)?;
                    payload.apply(&control)?;
                    Ok(control)
                })?;
                revokers.extend(cx.into_revokers());
                let element = wrap(&control)?;
                inner = Some(control);
                (Widget::Native { measure: payload.spec.measure.clone(), last: opaque }, element)
            }
            WidgetKind::Text => {
                let label = w::TextBlock::new()?;
                label.cast::<w::ITextBlock>()?.SetTextWrapping(w::TextWrapping::Wrap)?;
                let element = label.cast()?;
                (Widget::Label(label), element)
            }
            WidgetKind::Button => {
                let button = w::Button::new()?;
                let emitter = emitter.clone();
                revokers.push(button.cast::<w::IButtonBase>()?.Click(move |_, _| emitter.emit(id, UiEvent::Click))?);
                let element = button.cast()?;
                (Widget::Button(button), element)
            }
            // Reported as a checkbox is: the value it shows, once.
            WidgetKind::ToggleButton => {
                let button = w::ToggleButton::new()?;
                let toggle: w::IToggleButton = button.cast()?;
                for checked in [true, false] {
                    let (emitter, shown) = (emitter.clone(), shown_checked.clone());
                    let handler = move |sender: windows_core::Ref<IInspectable>,
                                        _: windows_core::Ref<w::RoutedEventArgs>| {
                        let value = sender.as_ref().and_then(|s| s.cast::<w::IToggleButton>().ok()?.IsChecked().ok());
                        if let Some(value) = value
                            && shown.replace(value) != value
                        {
                            emitter.emit(id, UiEvent::Changed(EventValue::Bool(value)));
                        }
                    };
                    revokers.push(if checked { toggle.Checked(handler)? } else { toggle.Unchecked(handler)? });
                }
                let element = button.cast()?;
                (Widget::Toggle(button), element)
            }
            // No click of its own: a click opens its flyout.
            WidgetKind::MenuButton => {
                let button = w::DropDownButton::new()?;
                let element = button.cast()?;
                (Widget::MenuButton(button), element)
            }
            WidgetKind::FileIcon => {
                let image = w::Image::new()?;
                image.SetStretch(w::Stretch::Uniform)?;
                let element = image.cast()?;
                let (asked, shown) = (Rc::default(), Rc::default());
                (Widget::FileIcon { image, file: None, thumbnail: None, size: None, asked, shown }, element)
            }
            WidgetKind::Icon => {
                let icon = w::FontIcon::new()?;
                let element = icon.cast()?;
                (Widget::Icon(icon), element)
            }
            WidgetKind::Checkbox => {
                let checkbox = w::CheckBox::new()?;
                let toggle: w::IToggleButton = checkbox.cast()?;
                for checked in [true, false] {
                    let (emitter, shown, mixed) = (emitter.clone(), shown_checked.clone(), shown_mixed.clone());
                    let handler = move |sender: windows_core::Ref<IInspectable>,
                                        _: windows_core::Ref<w::RoutedEventArgs>| {
                        let Some(value) =
                            sender.as_ref().and_then(|s| s.cast::<w::IToggleButton>().ok()?.IsChecked().ok())
                        else {
                            return;
                        };
                        let was_mixed = mixed.replace(false);
                        if shown.replace(value) != value || was_mixed {
                            emitter.emit(id, UiEvent::Changed(EventValue::Bool(value)));
                        }
                    };
                    revokers.push(if checked { toggle.Checked(handler)? } else { toggle.Unchecked(handler)? });
                }
                let element = checkbox.cast()?;
                (Widget::Checkbox(checkbox), element)
            }
            WidgetKind::Switch => {
                let switch = w::ToggleSwitch::new()?;
                // Just the track: its label is its own node. By default it
                // shows "On"/"Off" beside the track and is at least 154 wide.
                let iface: w::IToggleSwitch = switch.cast()?;
                iface.SetOnContent(None::<&IInspectable>)?;
                iface.SetOffContent(None::<&IInspectable>)?;
                switch.cast::<w::IFrameworkElement>()?.SetMinWidth(0.0)?;
                let (emitter, shown) = (emitter.clone(), shown_checked.clone());
                revokers.push(switch.cast::<w::IToggleSwitch>()?.Toggled(move |sender, _| {
                    let Some(value) = sender.as_ref().and_then(|s| s.cast::<w::IToggleSwitch>().ok()?.IsOn().ok())
                    else {
                        return;
                    };
                    if shown.replace(value) != value {
                        emitter.emit(id, UiEvent::Changed(EventValue::Bool(value)));
                    }
                })?);
                let element = switch.cast()?;
                (Widget::Switch(switch), element)
            }
            WidgetKind::Select => {
                let combo = w::ComboBox::new()?;
                // Setting the index or the items reports it too: only an
                // index the core doesn't know about is the user's choice.
                let (emitter, shown) = (emitter.clone(), shown_index.clone());
                revokers.push(combo.cast::<w::ISelector>()?.SelectionChanged(move |sender, _| {
                    let Some(index) = sender.as_ref().and_then(|s| s.cast::<w::ISelector>().ok()?.SelectedIndex().ok())
                    else {
                        return;
                    };
                    if index >= 0 && shown.replace(index) != index {
                        emitter.emit(id, UiEvent::Changed(EventValue::Index(index as usize)));
                    }
                })?);
                let element = combo.cast()?;
                (Widget::Select(combo), element)
            }
            WidgetKind::RadioGroup => {
                let group = w::RadioButtons::new()?;
                // Setting the index or the items reports it too: only an
                // index the core doesn't know about is the user's choice.
                let (emitter, shown) = (emitter.clone(), shown_index.clone());
                revokers.push(group.SelectionChanged(move |sender, _| {
                    let Some(index) =
                        sender.as_ref().and_then(|s| s.cast::<w::IRadioButtons>().ok()?.SelectedIndex().ok())
                    else {
                        return;
                    };
                    if index >= 0 && shown.replace(index) != index {
                        emitter.emit(id, UiEvent::Changed(EventValue::Index(index as usize)));
                    }
                })?);
                let element = group.cast()?;
                (Widget::RadioGroup(group), element)
            }
            WidgetKind::Slider => {
                let slider = w::Slider::new()?;
                // Setting the value or the range reports it too: only a
                // value the core doesn't know about is the user's.
                let (emitter, shown) = (emitter.clone(), shown_number.clone());
                revokers.push(slider.cast::<w::IRangeBase>()?.ValueChanged(move |sender, _| {
                    let Some(value) = sender.as_ref().and_then(|s| s.cast::<w::IRangeBase>().ok()?.Value().ok()) else {
                        return;
                    };
                    if shown.replace(value) != value {
                        emitter.emit(id, UiEvent::Changed(EventValue::Number(value)));
                    }
                })?);
                let element = slider.cast()?;
                (Widget::Slider { slider, step: None }, element)
            }
            WidgetKind::NumberInput => {
                let number = w::NumberBox::new()?;
                let iface: w::INumberBox = number.cast()?;
                // XAML's default hides the spin buttons; `Inline` is its
                // spin box. A tweak can pick `Compact` or `Hidden`.
                iface.SetSpinButtonPlacementMode(w::NumberBoxSpinButtonPlacementMode::Inline)?;
                // Typing commits on Return or leaving the field, the buttons
                // and arrow keys at once; setting the value or the range
                // reports it too, so only a value the core doesn't know
                // about is the user's. NumberBox takes decimals and shows an
                // emptied field as NaN: keep whole numbers, and put back the
                // last one for NaN. It ignores sets from inside its own
                // `ValueChanged`, so those wait until the handler returns;
                // `shown` already holds what they set, so they report
                // nothing.
                let (emitter, shown) = (emitter.clone(), shown_number.clone());
                revokers.push(iface.ValueChanged(move |sender, _| {
                    let Some(number) = sender.as_ref().and_then(|s| s.cast::<w::INumberBox>().ok()) else { return };
                    let Ok(value) = number.Value() else { return };
                    let whole = if value.is_nan() { shown.get() } else { value.round() };
                    if whole.to_bits() != value.to_bits() {
                        set_later(&number, value, whole);
                    }
                    if !value.is_nan() && shown.replace(whole) != whole {
                        emitter.emit(id, UiEvent::Changed(EventValue::Number(whole)));
                    }
                })?);
                let element = number.cast()?;
                (Widget::Number { number, step: None }, element)
            }
            WidgetKind::Progress => {
                let progress = w::ProgressBar::new()?;
                let range: w::IRangeBase = progress.cast()?;
                range.SetMinimum(0.0)?;
                range.SetMaximum(1.0)?;
                let element = progress.cast()?;
                (Widget::Progress(progress), element)
            }
            // Indeterminate by default; inactive, it shows nothing.
            WidgetKind::Spinner => {
                let ring = w::ProgressRing::new()?;
                ring.cast::<w::IProgressRing>()?.SetIsActive(false)?;
                // It has no size until XAML loads it and applies its
                // template: measure it again then.
                revokers.push(ring.cast::<w::IFrameworkElement>()?.Loaded({
                    let emitter = emitter.clone();
                    move |_, _| emitter.emit(id, UiEvent::Remeasure)
                })?);
                let element = ring.cast()?;
                (Widget::Spinner(ring), element)
            }
            WidgetKind::Separator => {
                let line = w::Border::new()?;
                line.cast::<w::IFrameworkElement>()?.SetStyle(&separator_style(Orientation::Horizontal)?)?;
                let element = line.cast()?;
                (Widget::Separator(line), element)
            }
            // XAML decodes files in the background: once it has, the image
            // has a size, and the core measures it again. A file it can't
            // read shows nothing.
            WidgetKind::Image => {
                let image = w::Image::new()?;
                let (failed, opened) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
                revokers.push(image.ImageOpened({
                    let (emitter, opened) = (emitter.clone(), opened.clone());
                    move |_, _| {
                        opened.set(true);
                        emitter.emit(id, UiEvent::Remeasure);
                    }
                })?);
                revokers.push(image.ImageFailed({
                    let (emitter, failed) = (emitter.clone(), failed.clone());
                    move |_, _| {
                        failed.set(true);
                        emitter.emit(id, UiEvent::Remeasure);
                    }
                })?);
                let element = image.cast()?;
                (Widget::Image { image, source: None, fit: None, bitmap: None, failed, opened }, element)
            }
            WidgetKind::TextInput => {
                let field = w::TextBox::new()?;
                revokers.push(report_text_changes(&field, &emitter, &shown_text, id)?);
                revokers.push(submit_on_enter(&field.cast()?, &emitter, id)?);
                let element = field.cast()?;
                (Widget::Field(field), element)
            }
            // A text box that takes Return, wraps, and shows its vertical
            // scroll bar as needed, as Fluent's multi-line text boxes are
            // made: XAML's default style hides it.
            WidgetKind::TextArea => {
                let field = w::TextBox::new()?;
                let iface: w::ITextBox = field.cast()?;
                iface.SetAcceptsReturn(true)?;
                iface.SetTextWrapping(w::TextWrapping::Wrap)?;
                w::ScrollViewer::SetVerticalScrollBarVisibility(
                    &field.cast::<w::DependencyObject>()?,
                    w::ScrollBarVisibility::Auto,
                )?;
                revokers.push(report_text_changes(&field, &emitter, &shown_text, id)?);
                let element = field.cast()?;
                (Widget::TextArea { field, lines: 1 }, element)
            }
            // With XAML's default reveal button, shown while there's text.
            WidgetKind::PasswordInput => {
                let field = w::PasswordBox::new()?;
                // PasswordChanged also fires for programmatic sets: only
                // text the core doesn't know about is a user edit.
                revokers.push(field.cast::<w::IPasswordBox>()?.PasswordChanged({
                    let (emitter, shown) = (emitter.clone(), shown_text.clone());
                    move |sender, _| {
                        let Some(text) =
                            sender.as_ref().and_then(|s| s.cast::<w::IPasswordBox>().ok()?.Password().ok())
                        else {
                            return;
                        };
                        if *shown.borrow() != text {
                            *shown.borrow_mut() = text.clone();
                            emitter.emit(id, UiEvent::Changed(EventValue::Text(text)));
                        }
                    }
                })?);
                revokers.push(submit_on_enter(&field.cast()?, &emitter, id)?);
                let element = field.cast()?;
                (Widget::Password(field), element)
            }
            // An auto-suggest box with the find icon, as Windows' search
            // boxes are made, and no suggestions: a list would pop up.
            WidgetKind::SearchInput => {
                let search = w::AutoSuggestBox::new()?;
                let iface: w::IAutoSuggestBox = search.cast()?;
                iface.SetQueryIcon(
                    &w::SymbolIcon::CreateInstanceWithSymbol(w::Symbol::Find)?.cast::<w::IconElement>()?,
                )?;
                // TextChanged also fires (later) for programmatic sets: only
                // text the core doesn't know about is a user edit. It's a
                // search too, at once: XAML doesn't wait for a pause.
                revokers.push(iface.TextChanged({
                    let (emitter, shown) = (emitter.clone(), shown_text.clone());
                    move |sender, _| {
                        let Some(text) = sender.as_ref().and_then(|s| s.Text().ok()) else { return };
                        if *shown.borrow() != text {
                            *shown.borrow_mut() = text.clone();
                            emitter.emit(id, UiEvent::Changed(EventValue::Text(text.clone())));
                            emitter.emit(id, UiEvent::Search(text));
                        }
                    }
                })?);
                // Return, or a click on the find icon.
                revokers.push(iface.QuerySubmitted({
                    let emitter = emitter.clone();
                    move |_, args| {
                        if let Some(text) = args.as_ref().and_then(|a| a.QueryText().ok()) {
                            emitter.emit(id, UiEvent::Search(text));
                        }
                    }
                })?);
                let element = search.cast()?;
                (Widget::Search(search), element)
            }
            WidgetKind::ScrollView => {
                let scroll = w::ScrollViewer::new()?;
                let iface: w::IScrollViewer = scroll.cast()?;
                set_scrolling(&iface, ScrollAxes::default(), true)?;
                revokers.push(iface.ViewChanged({
                    let (emitter, last) = (emitter.clone(), offset.clone());
                    move |sender, _| {
                        if let Some(scroll) = sender.as_ref().and_then(|s| s.cast::<w::IScrollViewer>().ok()) {
                            report_offset(&emitter, id, &last, &scroll);
                        }
                    }
                })?);
                let element = scroll.cast()?;
                (Widget::Scroll(scroll), element)
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
            WidgetKind::Sidebar => {
                let sidebar = crate::sidebar::Sidebar::new(id, emitter.clone())?;
                let element = sidebar.view.cast()?;
                (Widget::Sidebar(sidebar), element)
            }
            WidgetKind::Group => {
                let group = crate::group::Group::new(id, emitter.clone(), self.group_heading.clone())?;
                let element = group.canvas.cast()?;
                (Widget::Group(group), element)
            }
            WidgetKind::Tabs => {
                let tabs = crate::tabs::Tabs::new(id, emitter.clone(), self.tab_bar.clone())?;
                let element = tabs.canvas.cast()?;
                (Widget::Tabs(tabs), element)
            }
            WidgetKind::List => {
                let list = crate::list::List::new(id, emitter.clone(), false)?;
                let element = list.view.cast()?;
                (Widget::List(list), element)
            }
            // The header and the list view in a grid: the view takes focus
            // and is what assistive technology and tweaks reach.
            WidgetKind::Table => {
                let list = crate::list::List::new(id, emitter.clone(), true)?;
                let element = list.root.as_ref().expect("a table has a root").cast()?;
                inner = Some(list.view.cast()?);
                (Widget::List(list), element)
            }
        };
        // The core assumes new nodes start with a zero frame and only sends
        // frames that differ.
        if !matches!(widget, Widget::Window(_)) {
            let fe: w::IFrameworkElement = element.cast()?;
            fe.SetWidth(0.0)?;
            fe.SetHeight(0.0)?;
        }
        self.by_element.borrow_mut().insert(key(&element), id);
        if let Widget::Window(parts) = &widget {
            // Focus on the window's own parts resolves to no node.
            self.by_element.borrow_mut().remove(&key(&parts.host));
        }
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                element,
                inner,
                parent: None,
                row: None,
                column: None,
                revokers,
                shown_text,
                shown_checked,
                shown_mixed,
                shown_index,
                shown_number,
                offset,
                shift_wheel: None,
                scroll_content: false,
                text_style: None,
                text_color: None,
                role: None,
                button_style: None,
                tabs_style: None,
                orientation: None,
                mixed: None,
                tweak: None,
                a11y_label: None,
                description: None,
                tooltip: String::new(),
                context_menu: None,
                file_drop: None,
                file_drop_sent: false,
                keys: None,
                direction: None,
                align: None,
                password_all: Cell::new(false),
                button_menu: None,
                caption: String::new(),
                icon: String::new(),
                icon_only: None,
                icon_size: false,
            },
        );
        Ok(())
    }
}

/// The `Border` a native render or view sits in: it carries our frame, and
/// the control inside sizes itself.
fn wrap(control: &w::UIElement) -> R<w::UIElement> {
    let border = w::Border::new()?;
    border.cast::<w::IBorder>()?.SetChild(control)?;
    border.cast()
}
