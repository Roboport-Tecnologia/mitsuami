//! Setting props on nodes.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use mitsuami_core::{Command, ImageFit, ImageSource, LayoutDirection, NodeId, Pixels, Prop, TextStyle, UiEvent};
use mitsuami_core::{HorizontalAlign, InputPurpose};
use windows_core::{IInspectable, Interface};

use super::controls::{set_button_content, set_button_style, set_menu_button_style, set_toggle_style};
use super::fields::box_text;
use super::focus::is_control;
use super::menus::refresh_menu;
use super::new_window::escape_closes;
use super::scroll::{scroll_axes, scroll_bars, set_scrolling};
use super::styles::{
    MONOSPACE, font_size, font_weight, foreground_style, separator_style, set_label_style, weight_value,
};
use super::truncate;
use super::windows::{apply_full_screen, apply_maximized, apply_min_size, in_full_screen, overlapped};
use super::{ContextMenu, R, State, Widget, boxed, set_help_text, set_hit_testable, violation};
use crate::bindings as w;
use crate::custom::NativePayload;

impl State {
    pub(super) fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) -> R<()> {
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        // Custom widgets and native views keep what they were last given.
        match (prop, &mut node.widget) {
            (Prop::Custom(new), Widget::Custom { render, props }) => {
                if props != new {
                    let element = node.inner.clone().unwrap_or_else(|| node.element.clone());
                    let (render, events) = (render.clone(), self.emitter.clone());
                    events.muted(|| render.update(&element, props.props(), new.props()))?;
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(drawing), Widget::Drawn { view, .. }) => view.set_drawing(drawing)?,
            (Prop::Step(new), Widget::Slider { step, .. } | Widget::Number { step, .. }) => *step = *new,
            (Prop::Lines(n), Widget::TextArea { lines, .. }) => *lines = *n,
            // A dialog doesn't take full screen (its presenter is modal,
            // over its owner), as a sheet can't on macOS.
            (Prop::FullScreen(on), Widget::Window(parts)) => {
                if *on && parts.modal.is_some() {
                    parts.full_screen.set(false);
                    parts.emitter.emit(parts.node, UiEvent::FullScreenChanged(false));
                } else {
                    parts.full_screen.set(*on);
                    apply_full_screen(parts);
                }
            }
            (Prop::MinSize(min), Widget::Window(parts)) => {
                parts.min_size = Some(*min);
                apply_min_size(parts);
            }
            (Prop::HeightFollowsContent(on), Widget::Window(parts)) => {
                parts.height_locked = *on;
                apply_min_size(parts);
            }
            (Prop::Maximized(on), Widget::Window(parts)) => {
                parts.maximized.set(*on);
                apply_maximized(parts);
            }
            // On its own presenter, which comes back after full screen.
            (Prop::Resizable(on), Widget::Window(parts)) => {
                parts.resizable = *on;
                let presenter = match (&parts.overlapped, in_full_screen(&parts.app_window)) {
                    (Some(saved), true) => saved.cast::<w::IOverlappedPresenter>().ok(),
                    _ => overlapped(&parts.app_window),
                };
                if let Some(presenter) = presenter {
                    presenter.SetIsResizable(*on)?;
                }
            }
            // Acted on when it's shown.
            (Prop::Modal { owner, modality }, Widget::Window(parts)) => {
                let was_modal = parts.modal.replace((*owner, *modality)).is_some();
                if parts.escape.is_none() {
                    parts.escape = Some(escape_closes(&parts.root, parts.hwnd)?);
                }
                if !was_modal {
                    refresh_menu(parts, &self.menus);
                }
            }
            (Prop::Image(new), Widget::Image { image, source, bitmap, failed, opened, .. }) => {
                failed.set(false);
                opened.set(false);
                *bitmap = None;
                match new {
                    ImageSource::Pixels(pixels) => {
                        image.SetSource(&writeable_bitmap(pixels)?.cast::<w::ImageSource>()?)?
                    }
                    ImageSource::File(path) => {
                        let decoded = w::BitmapImage::CreateInstanceWithUriSource(&file_uri(path)?)?;
                        image.SetSource(&decoded.cast::<w::ImageSource>()?)?;
                        *bitmap = Some(decoded);
                    }
                }
                *source = Some(new.clone());
            }
            // Loaded once after the batch (`load_file_icons`): a new icon
            // comes with its file, size and thumbnail together.
            (
                Prop::File(_) | Prop::IconSize(_) | Prop::Thumbnail(_),
                Widget::FileIcon { image, file, thumbnail, size, asked, shown, latest },
            ) => {
                match prop {
                    Prop::File(path) => *file = Some(path.clone()),
                    Prop::Thumbnail(on) => *thumbnail = Some(*on),
                    Prop::IconSize(points) => *size = Some(*points),
                    _ => {}
                }
                let side = size.unwrap_or(FILE_ICON_SIZE);
                let element = image.cast::<w::IFrameworkElement>()?;
                element.SetWidth(f64::from(side))?;
                element.SetHeight(f64::from(side))?;
                let ticket = asked.get() + 1;
                asked.set(ticket);
                latest.store(ticket, Ordering::Relaxed);
                if file.is_none() {
                    image.SetSource(None::<&w::ImageSource>)?;
                    shown.set(ticket);
                    return Ok(());
                }
                if !self.pending_icons.contains(&id) {
                    self.pending_icons.push(id);
                }
            }
            (Prop::ImageFit(new), Widget::Image { image, fit, .. }) => {
                image.SetStretch(match new {
                    ImageFit::Contain => w::Stretch::Uniform,
                    ImageFit::Stretch => w::Stretch::Fill,
                })?;
                *fit = Some(*new);
            }
            (Prop::Native(opaque), Widget::Native { last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    let control = node.inner.clone().unwrap_or_else(|| node.element.clone());
                    self.emitter.muted(|| payload.apply(&control))?;
                }
                *last = opaque.clone();
            }
            _ => {}
        }
        match (prop, &node.widget) {
            (Prop::Title(t), Widget::Window(parts)) => {
                // The window's own title still names it in the taskbar and Alt+Tab.
                parts.window.cast::<w::IWindow>()?.SetTitle(t)?;
                parts.title_bar.cast::<w::ITitleBar>()?.SetTitle(t)?;
            }
            (Prop::Text(t), Widget::Label(_)) => node.label.text = t.clone(),
            (Prop::Selectable(on), Widget::Label(l)) => l.cast::<w::ITextBlock>()?.SetIsTextSelectionEnabled(*on)?,
            // 0 is XAML's "no limit"; trimming puts an ellipsis at the end
            // of the last line shown, or `truncate` at a line's start or
            // middle (`show_text`, below).
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                l.cast::<w::ITextBlock>()?.SetMaxLines(lines.map_or(0, |n| n as i32))?
            }
            (Prop::Label(t), Widget::Button(_) | Widget::Toggle(_) | Widget::MenuButton(_)) => {
                node.caption = t.clone();
                set_button_content(node)?;
            }
            (Prop::Icon(name), Widget::Button(_) | Widget::Toggle(_) | Widget::MenuButton(_)) => {
                node.icon = name.clone();
                set_button_content(node)?;
            }
            (Prop::IconOnly(only), Widget::Button(_) | Widget::Toggle(_) | Widget::MenuButton(_)) => {
                node.icon_only = Some(*only);
                set_button_content(node)?;
            }
            (Prop::Label(t), Widget::Checkbox(_)) => {
                node.element.cast::<w::IContentControl>()?.SetContent(&boxed(t))?
            }
            (Prop::Icon(name), Widget::Icon(icon)) => icon.cast::<w::IFontIcon>()?.SetGlyph(name)?,
            // A glyph is text: the brushes text takes, the accent's own
            // for text included.
            (Prop::TextColor(color), Widget::Icon(icon)) => {
                node.text_color = Some(*color);
                icon.cast::<w::IFrameworkElement>()?.SetStyle(&foreground_style("FontIcon", *color)?)?;
            }
            (Prop::IconSize(points), Widget::Icon(icon)) => {
                icon.cast::<w::IFontIcon>()?.SetFontSize(f64::from(*points))?;
                node.icon_size = true;
            }
            (
                Prop::Label(t),
                Widget::Switch(_)
                | Widget::Select(_)
                | Widget::RadioGroup(_)
                | Widget::Slider { .. }
                | Widget::Number { .. }
                | Widget::Progress(_)
                | Widget::Spinner(_)
                | Widget::Image { .. }
                | Widget::Icon(_)
                | Widget::FileIcon { .. }
                | Widget::GpuSurface(_),
            ) => {
                w::AutomationProperties::SetName(&node.element, t)?;
                node.a11y_label = Some(t.clone());
            }
            (Prop::Options(options), Widget::Select(combo)) => {
                // Items are `ComboBoxItem`s, so options with the same text
                // stay apart. Changing them moves the selection; the chosen
                // index stays if it can, else the first option is chosen,
                // as the core does. It sends the index when that changes it.
                // Items are retitled in place, and only the extra ones added
                // or removed: XAML fails fast when the items are replaced
                // while the drop-down closes after the user's pick, as when
                // the pick changes the app's language.
                let selector: w::ISelector = combo.cast()?;
                let chosen = selector.SelectedIndex()?;
                let count = options.len() as i32;
                let index = if (0..count).contains(&chosen) {
                    chosen
                } else if count > 0 {
                    0
                } else {
                    -1
                };
                node.shown_index.set(index);
                let items = combo.cast::<w::IItemsControl>()?.Items()?;
                for (i, option) in options.iter().enumerate() {
                    if (i as u32) < items.Size()? {
                        items.GetAt(i as u32)?.cast::<w::IContentControl>()?.SetContent(&boxed(option))?;
                    } else {
                        let item = w::ComboBoxItem::new()?;
                        item.cast::<w::IContentControl>()?.SetContent(&boxed(option))?;
                        items.Append(&item.cast::<IInspectable>()?)?;
                    }
                }
                // Chosen before the rest go, so the chosen one isn't removed.
                selector.SetSelectedIndex(index)?;
                while items.Size()? > count as u32 {
                    items.RemoveAtEnd()?;
                }
            }
            (Prop::Range { min, max }, Widget::Slider { slider, .. }) => {
                let range: w::IRangeBase = slider.cast()?;
                // A value the new range clamps isn't the user's, and XAML
                // reports it while the range is set: expect it first.
                node.shown_number.set(range.Value()?.max(*min).min(*max));
                // Widen first, so the value is clamped once, to the new range.
                if *min < range.Maximum()? {
                    range.SetMinimum(*min)?;
                    range.SetMaximum(*max)?;
                } else {
                    range.SetMaximum(*max)?;
                    range.SetMinimum(*min)?;
                }
                // A value the new range clamped isn't the user's; the core
                // sends it next.
                node.shown_number.set(range.Value()?);
            }
            (Prop::Step(new), Widget::Slider { slider, .. }) => {
                // XAML snaps to `StepFrequency` and steps by `SmallChange`;
                // both are 1 unless set.
                let value = new.unwrap_or(1.0);
                slider.cast::<w::ISlider>()?.SetStepFrequency(value)?;
                slider.cast::<w::IRangeBase>()?.SetSmallChange(value)?;
            }
            (Prop::Orientation(o), Widget::Separator(line)) => {
                line.cast::<w::IFrameworkElement>()?.SetStyle(&separator_style(*o)?)?;
                node.orientation = Some(*o);
            }
            (Prop::Orientation(o), Widget::Slider { slider, .. }) => {
                let orientation = if o.vertical() { w::Orientation::Vertical } else { w::Orientation::Horizontal };
                slider.cast::<w::ISlider>()?.SetOrientation(orientation)?;
                node.orientation = Some(*o);
            }
            (Prop::Number(n), Widget::Slider { slider, .. }) => {
                node.shown_number.set(*n);
                slider.cast::<w::IRangeBase>()?.SetValue(*n)?;
                // XAML clamps what it's given to its range.
                node.shown_number.set(slider.cast::<w::IRangeBase>()?.Value()?);
            }
            (Prop::Range { min, max }, Widget::Number { number, .. }) => {
                let iface: w::INumberBox = number.cast()?;
                // A value the new range clamps isn't the user's, and XAML
                // reports it while the range is set: expect it first.
                node.shown_number.set(iface.Value()?.max(*min).min(*max));
                // Widen first, so the value is clamped once, to the new range.
                if *min < iface.Maximum()? {
                    iface.SetMinimum(*min)?;
                    iface.SetMaximum(*max)?;
                } else {
                    iface.SetMaximum(*max)?;
                    iface.SetMinimum(*min)?;
                }
                // A value the new range clamped isn't the user's; the core
                // sends it next.
                node.shown_number.set(iface.Value()?);
            }
            // What the spin buttons and arrow keys add; 1 unless set.
            (Prop::WrapAround(on), Widget::Number { number, .. }) => {
                number.cast::<w::INumberBox>()?.SetIsWrapEnabled(*on)?
            }
            (Prop::Step(new), Widget::Number { number, .. }) => {
                number.cast::<w::INumberBox>()?.SetSmallChange(new.unwrap_or(1.0))?
            }
            (Prop::Number(n), Widget::Number { number, .. }) => {
                let iface: w::INumberBox = number.cast()?;
                node.shown_number.set(*n);
                iface.SetValue(*n)?;
                // It clamps what it's given to its range.
                node.shown_number.set(iface.Value()?);
            }
            (Prop::Running(r), Widget::Spinner(ring)) => ring.cast::<w::IProgressRing>()?.SetIsActive(*r)?,
            (Prop::Options(options), Widget::RadioGroup(group)) => {
                // Strings: RadioButtons makes a button for each. Replacing
                // them moves the selection; the chosen index stays if it
                // can, else none is chosen, as the core does. It sends the
                // index when that changes it.
                let chosen = group.SelectedIndex()?;
                let index = if (0..options.len() as i32).contains(&chosen) { chosen } else { -1 };
                node.shown_index.set(index);
                let items = group.Items()?;
                items.Clear()?;
                for option in options {
                    items.Append(&boxed(option))?;
                }
                group.SetSelectedIndex(index)?;
            }
            (Prop::Progress(progress), Widget::Progress(p)) => {
                p.cast::<w::IProgressBar>()?.SetIsIndeterminate(progress.is_none())?;
                if let Some(fraction) = progress {
                    p.cast::<w::IRangeBase>()?.SetValue(*fraction)?;
                }
            }
            (Prop::SelectedIndex(index), Widget::RadioGroup(group)) => {
                let index = index.map_or(-1, |i| i as i32);
                node.shown_index.set(index);
                group.SetSelectedIndex(index)?;
            }
            (Prop::SelectedIndex(index), Widget::Select(combo)) => {
                let index = index.map_or(-1, |i| i as i32);
                node.shown_index.set(index);
                combo.cast::<w::ISelector>()?.SetSelectedIndex(index)?;
            }
            (Prop::Sections(sections), Widget::Sidebar(sidebar)) => sidebar.set_sections(sections.clone())?,
            (Prop::SelectedIndex(index), Widget::Sidebar(sidebar)) => sidebar.set_selected(*index)?,
            (Prop::SidebarShown(shown), Widget::Sidebar(sidebar)) => sidebar.set_shown(*shown)?,
            (Prop::TabTitles(titles), Widget::Tabs(tabs)) => tabs.set_titles(titles)?,
            (Prop::TabIcons(icons), Widget::Tabs(tabs)) => tabs.set_icons(icons)?,
            // One way to show tabs: the app's choice is kept, not shown.
            (Prop::TabsStyle(style), Widget::Tabs(_)) => node.tabs_style = Some(*style),
            (Prop::Title(title), Widget::Group(group)) => group.set_title(title)?,
            (Prop::SelectedIndex(index), Widget::Tabs(tabs)) => tabs.set_selected(*index)?,
            // Its tabs; its pages are the app's.
            (Prop::Enabled(e), Widget::Tabs(tabs)) => tabs.bar.cast::<w::IControl>()?.SetIsEnabled(*e)?,
            (Prop::Value(t), Widget::Field(f) | Widget::TextArea { field: f, .. }) => {
                let field: w::ITextBox = f.cast()?;
                // Don't disturb the caret when the field already shows it.
                if box_text(&field)? != *t {
                    *node.shown_text.borrow_mut() = t.clone();
                    field.SetText(t)?;
                }
            }
            (Prop::Placeholder(t), Widget::Field(f) | Widget::TextArea { field: f, .. }) => {
                f.cast::<w::ITextBox>()?.SetPlaceholderText(t)?
            }
            // Still focusable and selectable, so its text can be copied.
            // The touch keyboard's layout, and the input panel's.
            (Prop::InputPurpose(purpose), Widget::Field(f)) => {
                let name = w::InputScopeName::new()?;
                name.cast::<w::IInputScopeName>()?.SetNameValue(match purpose {
                    InputPurpose::Text => w::InputScopeNameValue::Default,
                    InputPurpose::Email => w::InputScopeNameValue::EmailSmtpAddress,
                    InputPurpose::Url => w::InputScopeNameValue::Url,
                    InputPurpose::Phone => w::InputScopeNameValue::TelephoneNumber,
                })?;
                let scope = w::InputScope::new()?;
                scope.cast::<w::IInputScope>()?.Names()?.Append(&name)?;
                f.cast::<w::ITextBox>()?.SetInputScope(&scope)?;
            }
            // Unwrapped, long lines scroll sideways.
            (Prop::LineWrap(on), Widget::TextArea { field, .. }) => {
                field.cast::<w::ITextBox>()?.SetTextWrapping(if *on {
                    w::TextWrapping::Wrap
                } else {
                    w::TextWrapping::NoWrap
                })?;
                w::ScrollViewer::SetHorizontalScrollBarVisibility(
                    &field.cast::<w::DependencyObject>()?,
                    if *on { w::ScrollBarVisibility::Disabled } else { w::ScrollBarVisibility::Auto },
                )?;
            }
            (Prop::ReadOnly(r), Widget::Field(f) | Widget::TextArea { field: f, .. }) => {
                f.cast::<w::ITextBox>()?.SetIsReadOnly(*r)?
            }
            (Prop::Value(t), Widget::Password(f)) => {
                let field: w::IPasswordBox = f.cast()?;
                if field.Password()? != *t {
                    *node.shown_text.borrow_mut() = t.clone();
                    field.SetPassword(t)?;
                }
            }
            (Prop::Placeholder(t), Widget::Password(f)) => f.cast::<w::IPasswordBox>()?.SetPlaceholderText(t)?,
            (Prop::Value(t), Widget::Search(s)) => {
                let search: w::IAutoSuggestBox = s.cast()?;
                if search.Text()? != *t {
                    *node.shown_text.borrow_mut() = t.clone();
                    search.SetText(t)?;
                }
            }
            (Prop::Placeholder(t), Widget::Search(s)) => s.cast::<w::IAutoSuggestBox>()?.SetPlaceholderText(t)?,
            (Prop::Checked(c), Widget::Checkbox(b)) => {
                node.shown_checked.set(*c);
                // The mixed state shows over it.
                if !node.shown_mixed.get() {
                    b.cast::<w::IToggleButton>()?.SetIsChecked(Some(*c))?;
                }
            }
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                node.mixed = Some(*m);
                node.shown_mixed.set(*m);
                let checked = node.shown_checked.get();
                b.cast::<w::IToggleButton>()?.SetIsChecked(if *m { None } else { Some(checked) })?;
            }
            (Prop::Checked(c), Widget::Toggle(b)) => {
                node.shown_checked.set(*c);
                b.cast::<w::IToggleButton>()?.SetIsChecked(Some(*c))?;
            }
            (Prop::Checked(c), Widget::Switch(s)) => {
                node.shown_checked.set(*c);
                s.cast::<w::IToggleSwitch>()?.SetIsOn(*c)?;
            }
            // Disabled, a text area shows no selection.
            (Prop::Enabled(e), Widget::TextArea { field, .. }) => {
                field.cast::<w::IControl>()?.SetIsEnabled(*e)?;
                if !e {
                    field.cast::<w::ITextBox>()?.SetSelectionLength(0)?;
                }
            }
            (Prop::Enabled(e), _) if is_control(&node.widget) => {
                node.element.cast::<w::IControl>()?.SetIsEnabled(*e)?
            }
            (Prop::TextStyle(text_style), Widget::Label(l)) => {
                node.text_style = Some(*text_style);
                set_label_style(l, node.text_style, node.text_color)?;
                if *text_style == TextStyle::Monospace {
                    l.cast::<w::ITextBlock>()?.SetFontFamily(&w::FontFamily::CreateInstanceWithName(MONOSPACE)?)?;
                }
            }
            (Prop::TextColor(color), Widget::Label(l)) => {
                node.text_color = Some(*color);
                set_label_style(l, node.text_style, node.text_color)?;
            }
            // Local values, so they win over the text style's setters.
            (Prop::FontWeight(weight), Widget::Label(l)) => {
                l.cast::<w::ITextBlock>()?.SetFontWeight(w::FontWeight { weight: weight_value(*weight) })?
            }
            (Prop::Italic(italic), Widget::Label(l)) => l.cast::<w::ITextBlock>()?.SetFontStyle(if *italic {
                w::FontStyle::Italic
            } else {
                w::FontStyle::Normal
            })?,
            (Prop::Truncation(truncation), Widget::Label(_)) => node.truncation = Some(*truncation),
            (Prop::TextAlign(align), Widget::Label(l)) => {
                node.align = Some(*align);
                align_label(l, *align)?;
            }
            (Prop::TextStyle(text_style), _) if is_control(&node.widget) => {
                let control: w::IControl = node.element.cast()?;
                control.SetFontSize(font_size(*text_style))?;
                control.SetFontWeight(w::FontWeight { weight: font_weight(*text_style) })?;
                if *text_style == TextStyle::Monospace {
                    control.SetFontFamily(&w::FontFamily::CreateInstanceWithName(MONOSPACE)?)?;
                }
                node.text_style = Some(*text_style);
            }
            (Prop::ScrollAxes(axes), Widget::Scroll(s)) => {
                let scroll = s.cast()?;
                set_scrolling(&scroll, *axes, scroll_bars(&scroll)?)?
            }
            (Prop::ScrollBars(show), Widget::Scroll(s)) => {
                let scroll = s.cast()?;
                set_scrolling(&scroll, scroll_axes(&scroll)?, *show)?
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone())?,
            (Prop::RowFiles(files), Widget::List(list)) => list.set_row_files(files.clone())?,
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode)?,
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style)?,
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows)?,
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::Cell(cell), Widget::Host(_)) => {
                node.row = Some(cell.row);
                node.column = Some(cell.column);
            }
            (Prop::Columns(columns), Widget::List(list)) => list.set_columns(columns)?,
            (Prop::Sort(sort), Widget::List(list)) => list.set_sort(*sort),
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                node.role = Some(*role);
                set_button_style(b, node.role, node.button_style)?;
            }
            (Prop::ButtonStyle(button_style), Widget::Button(b)) => {
                node.button_style = Some(*button_style);
                set_button_style(b, node.role, node.button_style)?;
            }
            (Prop::ButtonStyle(button_style), Widget::MenuButton(b)) => {
                node.button_style = Some(*button_style);
                set_menu_button_style(b, *button_style)?;
            }
            (Prop::ButtonStyle(button_style), Widget::Toggle(b)) => {
                node.button_style = Some(*button_style);
                set_toggle_style(b, *button_style)?;
            }
            (Prop::TakesInput(on), Widget::GpuSurface(surface)) => surface.set_takes_input(*on)?,
            (Prop::PointerLock(on), Widget::GpuSurface(surface)) => surface.set_pointer_lock(*on),
            (Prop::KeyboardGrab(on), Widget::GpuSurface(surface)) => surface.set_keyboard_grab(*on),
            (Prop::Cursor(cursor), Widget::GpuSurface(surface)) => surface.set_cursor(cursor),
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            (Prop::FileDrop(drop), Widget::Host(_) | Widget::Group(_)) => {
                node.file_drop_sent = true;
                match (drop, &node.file_drop) {
                    (Some(drop), Some(target)) => target.set(drop.clone()),
                    (Some(drop), None) => {
                        node.file_drop =
                            Some(crate::drop::DropTarget::new(id, self.emitter.clone(), drop.clone(), &node.element)?);
                    }
                    (None, _) => {
                        if let Some(target) = node.file_drop.take() {
                            target.leave();
                        }
                        node.element.cast::<w::IUIElement>()?.SetAllowDrop(false)?;
                    }
                }
                set_hit_testable(node)?;
            }
            // The host's `Canvas` gets a clear background meanwhile, to
            // get the pointer over its empty areas.
            (Prop::Hover(on), Widget::Host(_) | Widget::Group(_)) => {
                if let Some(hover) = node.hover.take() {
                    hover.remove();
                }
                if *on {
                    node.hover = Some(crate::hover::HoverTracker::new(id, self.emitter.clone(), &node.element)?);
                }
                set_hit_testable(node)?;
            }
            // Clear too, so a double tap on its empty areas is heard.
            (Prop::DoubleClick(on), Widget::Host(_) | Widget::Group(_)) => {
                node.double_click = None;
                if *on {
                    node.double_click =
                        Some(crate::double_click::DoubleClicker::new(id, self.emitter.clone(), &node.element)?);
                }
                set_hit_testable(node)?;
            }
            (Prop::Keys(keys), Widget::Host(_) | Widget::Group(_) | Widget::List(_)) => match &node.keys {
                Some(taken) => taken.set(keys.clone()),
                None => {
                    node.keys = Some(super::keys::Keys::new(id, self.emitter.clone(), &node.element, keys.clone())?);
                }
            },
            // XAML mirrors a panel's children, which the core placed for
            // their direction already, and a scroll viewer's or list's
            // offsets: they stay left to right, and so what they hold
            // doesn't inherit it. Controls mirror their own drawing.
            (Prop::LayoutDirection(direction), widget) => {
                node.direction = Some(*direction);
                let holds_ours = node.element.cast::<w::Canvas>().is_ok()
                    || matches!(widget, Widget::Scroll(_) | Widget::List(_) | Widget::Tabs(_) | Widget::Group(_));
                let flow = match direction {
                    LayoutDirection::RightToLeft if !holds_ours => w::FlowDirection::RightToLeft,
                    _ => w::FlowDirection::LeftToRight,
                };
                let control = node.inner.as_ref().unwrap_or(&node.element);
                control.cast::<w::IFrameworkElement>()?.SetFlowDirection(flow)?;
                if let Widget::Label(l) = widget
                    && let Some(align) = node.align
                {
                    align_label(l, align)?;
                }
                // A tab view's bar is a control of its own.
                if let Widget::Tabs(tabs) = widget {
                    let flow = if *direction == LayoutDirection::RightToLeft {
                        w::FlowDirection::RightToLeft
                    } else {
                        w::FlowDirection::LeftToRight
                    };
                    tabs.bar.cast::<w::IFrameworkElement>()?.SetFlowDirection(flow)?;
                }
            }
            (Prop::Tooltip(text), _) => {
                // On the control itself, not the Border a native render sits in.
                let control = node.inner.as_ref().unwrap_or(&node.element);
                let value = (!text.is_empty()).then(|| boxed(text));
                w::ToolTipService::SetToolTip(control, value.as_ref())?;
                node.tooltip = text.clone();
                set_hit_testable(node)?;
                set_help_text(node)?;
            }
            (Prop::ContextMenu(entries), _) => {
                // On the control the pointer is on, as the tooltip. XAML
                // shows it on a right-click, a press and hold, Shift+F10
                // and the Menu key, and `ContextRequested` bubbles: a child
                // without one shows its container's.
                let control: w::IUIElement = node.control().cast()?;
                let events = self.emitter.clone();
                let menu = node.context_menu.get_or_insert_with(|| {
                    ContextMenu::new(
                        Rc::new(move |item| events.emit(id, UiEvent::ContextMenuItem(item))),
                        control.ContextFlyout().ok(),
                    )
                });
                if menu.update(entries, format!("node-{id}"))? {
                    match &menu.flyout {
                        Some(flyout) => control.SetContextFlyout(flyout)?,
                        None => control.SetContextFlyout(menu.own.as_ref())?,
                    }
                }
                set_hit_testable(node)?;
            }
            // The button's own flyout, which XAML opens on a click, Enter,
            // Space or UIA's Expand. Radio groups are named apart from its
            // context menu's. Flyouts open above by default; drop-down
            // buttons' open below, as WinUI's own samples set them.
            (Prop::Menu(entries), Widget::MenuButton(b)) => {
                let events = self.emitter.clone();
                let menu = node.button_menu.get_or_insert_with(|| {
                    ContextMenu::new(Rc::new(move |item| events.emit(id, UiEvent::MenuItem(item))), None)
                });
                if menu.update(entries, format!("button-{id}"))? {
                    let button = b.cast::<w::IButton>()?;
                    match &menu.flyout {
                        Some(flyout) => {
                            flyout.cast::<w::IFlyoutBase>()?.SetPlacement(w::FlyoutPlacementMode::Bottom)?;
                            button.SetFlyout(flyout)?
                        }
                        None => button.SetFlyout(None::<&w::FlyoutBase>)?,
                    }
                }
            }
            _ => {}
        }
        // What a label shows depends on its text, font and limit.
        let shows_text = matches!(
            prop,
            Prop::Text(_)
                | Prop::MaxLines(_)
                | Prop::Truncation(_)
                | Prop::Selectable(_)
                | Prop::TextStyle(_)
                | Prop::FontWeight(_)
                | Prop::Italic(_)
        );
        if shows_text && matches!(node.widget, Widget::Label(_)) {
            node.label.min_width.set(None);
            truncate::show(node)?;
        }
        Ok(())
    }
}

/// A file icon's side when the app gives none: Explorer's details view's.
pub(super) const FILE_ICON_SIZE: f32 = 16.0;

impl State {
    /// Starts the loads of the file icons the batch changed, one each, at
    /// the latest ticket. The image comes back on the UI thread, unless a
    /// later load replaced it by then.
    pub(super) fn load_file_icons(&mut self) {
        for id in std::mem::take(&mut self.pending_icons) {
            let Some(Widget::FileIcon { image, file: Some(path), thumbnail, size, asked, shown, latest }) =
                self.nodes.get(&id).map(|n| &n.widget)
            else {
                continue;
            };
            let ticket = asked.get();
            // In the window's pixels, so the shell's image isn't scaled.
            let scale = image
                .cast::<w::IUIElement>()
                .and_then(|e| e.XamlRoot())
                .and_then(|r| r.RasterizationScale())
                .unwrap_or(1.0);
            let pixels = (f64::from(size.unwrap_or(FILE_ICON_SIZE)) * scale).round() as i32;
            let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() else { continue };
            let parked = crate::later::park((image.clone(), asked.clone(), shown.clone()));
            let request = crate::file_icon::Request {
                path: path.clone(),
                pixels,
                thumbnail: *thumbnail == Some(true),
                ticket,
                latest: latest.clone(),
            };
            crate::file_icon::load(request, move |loaded| {
                crate::later::on_ui(&queue, move || {
                    let Some((image, asked, shown)) =
                        crate::later::take::<(w::Image, Rc<Cell<u64>>, Rc<Cell<u64>>)>(parked)
                    else {
                        return;
                    };
                    // A later load replaces this one.
                    if asked.get() != ticket {
                        return;
                    }
                    let source = loaded.and_then(|l| crate::file_icon::bitmap(&l).ok());
                    _ = match source.and_then(|b| b.cast::<w::ImageSource>().ok()) {
                        Some(source) => image.SetSource(&source),
                        None => image.SetSource(None::<&w::ImageSource>),
                    };
                    shown.set(ticket);
                });
            });
        }
    }
}

/// A bitmap of the pixels, as XAML takes them: premultiplied BGRA.
fn writeable_bitmap(pixels: &Pixels) -> R<w::WriteableBitmap> {
    let bitmap = w::WriteableBitmap::CreateInstanceWithDimensions(pixels.width() as i32, pixels.height() as i32)?;
    let buffer = bitmap.PixelBuffer()?;
    let length = buffer.Length()? as usize;
    let access: w::IBufferByteAccess = buffer.cast()?;
    // SAFETY: the buffer holds `length` bytes and outlives the slice.
    let bytes = unsafe { std::slice::from_raw_parts_mut(access.Buffer()?, length) };
    let premultiply = |c: u8, a: u8| ((c as u16 * a as u16 + 127) / 255) as u8;
    for (to, from) in bytes.chunks_exact_mut(4).zip(pixels.rgba().chunks_exact(4)) {
        let [r, g, b, a] = [from[0], from[1], from[2], from[3]];
        to.copy_from_slice(&[premultiply(b, a), premultiply(g, a), premultiply(r, a), a]);
    }
    bitmap.Invalidate()?;
    Ok(bitmap)
}

/// A `file:///` URI for a path, made absolute.
fn file_uri(path: &std::path::Path) -> R<w::Uri> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    w::Uri::CreateUri(&format!("file:///{}", path.display().to_string().replace('\\', "/")))
}

/// XAML's left is the start of the text's flow: the right in a
/// right-to-left label. The core resolved the direction, so left is left.
fn align_label(l: &w::TextBlock, align: HorizontalAlign) -> R<()> {
    let rtl = l.cast::<w::IFrameworkElement>()?.FlowDirection()? == w::FlowDirection::RightToLeft;
    l.cast::<w::ITextBlock>()?.SetTextAlignment(match (align, rtl) {
        (HorizontalAlign::Center, _) => w::TextAlignment::Center,
        (HorizontalAlign::Left, false) | (HorizontalAlign::Right, true) => w::TextAlignment::Left,
        (HorizontalAlign::Right, false) | (HorizontalAlign::Left, true) => w::TextAlignment::Right,
    })?;
    Ok(())
}
