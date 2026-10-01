//! Setting props on the native widgets.

use mitsuami_core::{
    ButtonRole, ButtonStyle, Command, HorizontalAlign, ImageFit, InputPurpose, LayoutDirection, NodeId, Prop,
    Truncation, UiEvent,
};
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSControlStateValueMixed, NSControlStateValueOff, NSControlStateValueOn, NSImageScaling,
    NSLineBreakMode, NSMenuItem, NSProgressIndicatorStyle, NSSlider, NSTextAlignment, NSTextContent, NSTextContentType,
    NSTextContentTypeEmailAddress, NSTextContentTypeTelephoneNumber, NSTextContentTypeURL, NSTextField,
    NSUserInterfaceLayoutDirection, NSView, NSWindowStyleMask,
};

use crate::classes::ClosureTarget;
use crate::custom::NativePayload;
use crate::services::ItemTarget;

use super::fonts::{font, label_font};
use super::group::set_group_title;
use super::images::{FILE_ICON_SIZE, image_position, ns_image, show_file_icon, symbol};
use super::menus::{pull_down_title, show_pull_down};
use super::scrolling::set_scrollers;
use super::{State, Widget, ns, violation};

impl State {
    pub(super) fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) {
        let mtm = self.mtm;
        let events = self.events.clone();
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window { window, .. }) => window.setTitle(&ns(t)),
            (Prop::Title(t), Widget::Group { frame, .. }) => set_group_title(frame, t),
            (Prop::FileDrop(drop), Widget::Host(host) | Widget::Group { host, .. }) => {
                host.set_file_drop(drop.clone().map(|files| crate::classes::Drop {
                    id,
                    files,
                    events: events.clone(),
                    hover: Default::default(),
                }));
                node.file_drop = true;
            }
            (Prop::Keys(keys), Widget::Host(host) | Widget::Group { host, .. }) => {
                host.set_keys(Some(crate::keys::Keys { id, keys: keys.clone(), events: events.clone() }));
            }
            (Prop::Keys(keys), Widget::List(list)) => list.set_keys(keys.clone()),
            (Prop::FullScreen(on), Widget::Window { window, _delegate, .. }) => _delegate.set_full_screen(window, *on),
            // AppKit's maximize is zoom: the title bar's green button with
            // Option, or a double-click on the title bar.
            (Prop::Maximized(on), Widget::Window { window, _delegate, .. }) => _delegate.set_zoomed(window, *on),
            (Prop::Resizable(on), Widget::Window { window, .. }) => {
                let mask = window.styleMask();
                window.setStyleMask(if *on {
                    mask | NSWindowStyleMask::Resizable
                } else {
                    mask & !NSWindowStyleMask::Resizable
                });
            }
            (Prop::MinSize(min), Widget::Window { window, _delegate, .. }) => _delegate.set_min_size(window, *min),
            (Prop::HeightFollowsContent(on), Widget::Window { window, _delegate, .. }) => {
                _delegate.set_height_locked(window, *on)
            }
            // Acted on when the window is shown.
            (Prop::Modal { owner, modality }, Widget::Window { _delegate, .. }) => {
                _delegate.set_modal(true);
                node.modal = Some((*owner, *modality));
            }
            (Prop::Text(t), Widget::Label(l)) => l.setStringValue(&ns(t)),
            (Prop::Selectable(on), Widget::Label(l)) => l.setSelectable(*on),
            (Prop::MaxLines(lines), Widget::Label(l)) => set_line_limit(l, *lines, node.truncation.unwrap_or_default()),
            (Prop::Truncation(truncation), Widget::Label(l)) => {
                node.truncation = Some(*truncation);
                let lines = l.maximumNumberOfLines();
                set_line_limit(l, (lines > 0).then_some(lines as u32), *truncation);
            }
            (Prop::Label(t), Widget::Checkbox(b)) => b.setTitle(&ns(t)),
            // A new title puts the image back beside it: an icon shown
            // alone stays so.
            (Prop::Label(t), Widget::Button(b)) => {
                b.setTitle(&ns(t));
                if node.icon.is_some() {
                    b.setImagePosition(image_position(node.icon_only));
                }
            }
            (Prop::Label(t), Widget::Switch(s)) => s.setAccessibilityLabel(Some(&ns(t))),
            (Prop::Label(t), Widget::Select(p)) => p.setAccessibilityLabel(Some(&ns(t))),
            (Prop::Label(t), Widget::RadioGroup(group)) => group.stack.setAccessibilityLabel(Some(&ns(t))),
            (Prop::Options(options), Widget::RadioGroup(group)) => group.set_options(options),
            (Prop::SelectedIndex(index), Widget::RadioGroup(group)) => group.set_selected(*index),
            (Prop::Enabled(e), Widget::RadioGroup(group)) => group.set_enabled(*e),
            (Prop::Label(_) | Prop::Icon(_) | Prop::IconOnly(_) | Prop::Menu(_), Widget::MenuButton { .. }) => {
                match prop {
                    Prop::Icon(name) => node.icon = Some(name.clone()),
                    Prop::IconOnly(only) => node.icon_only = Some(*only),
                    Prop::Menu(entries) => {
                        if let Widget::MenuButton { sent, .. } = &mut node.widget {
                            *sent = entries.clone();
                        }
                    }
                    _ => {}
                }
                let label = match prop {
                    Prop::Label(t) => t.clone(),
                    _ => pull_down_title(&node.widget),
                };
                show_pull_down(&node.widget, &label, node.icon.as_deref(), node.icon_only);
            }
            (Prop::ButtonStyle(style), Widget::MenuButton { popup, .. }) => {
                popup.setBordered(*style != ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Options(options), Widget::Select(p)) => {
                // Items go straight into the menu: `addItemWithTitle:`
                // drops earlier items with the same title. The chosen index
                // stays if it can, else the first option is chosen, as the
                // core does; it sends the index when that changes it.
                let chosen = p.indexOfSelectedItem();
                p.removeAllItems();
                if let Some(menu) = p.menu() {
                    for option in options {
                        let item = unsafe {
                            NSMenuItem::initWithTitle_action_keyEquivalent(
                                NSMenuItem::alloc(mtm),
                                &ns(option),
                                None,
                                &ns(""),
                            )
                        };
                        menu.addItem(&item);
                    }
                }
                let count = p.numberOfItems();
                p.selectItemAtIndex(if (0..count).contains(&chosen) {
                    chosen
                } else if count > 0 {
                    0
                } else {
                    -1
                });
            }
            (Prop::SelectedIndex(index), Widget::Select(p)) => {
                p.selectItemAtIndex(index.map_or(-1, |i| i as isize));
            }
            (Prop::Sections(sections), Widget::Sidebar(sidebar)) => sidebar.set_sections(sections.clone()),
            (Prop::SelectedIndex(index), Widget::Sidebar(sidebar)) => sidebar.set_selected(*index),
            // Applied to the window's split, once it has one.
            (Prop::SidebarShown(on), Widget::Sidebar(sidebar)) => sidebar.shown.set(Some(*on)),
            (Prop::TabTitles(titles), Widget::Tabs(tabs)) => tabs.set_titles(titles.clone()),
            // An `NSTabView` draws only titles: its items' images are for
            // a tab view controller's toolbar style. Kept for the mirror.
            (Prop::TabIcons(icons), Widget::Tabs(_)) => node.tab_icons = Some(icons.clone()),
            // One way to show tabs: the app's choice is kept, not shown.
            (Prop::TabsStyle(style), Widget::Tabs(_)) => node.tabs_style = Some(*style),
            (Prop::SelectedIndex(index), Widget::Tabs(tabs)) => tabs.set_shown(*index),
            (Prop::Label(t), Widget::Slider { slider, .. }) => slider.setAccessibilityLabel(Some(&ns(t))),
            (Prop::Range { min, max }, Widget::Slider { slider, step }) => {
                slider.setMinValue(*min);
                slider.setMaxValue(*max);
                set_ticks(slider, *step);
            }
            (Prop::Step(new), Widget::Slider { slider, step }) => {
                *step = *new;
                set_ticks(slider, *new);
            }
            (Prop::Number(n), Widget::Slider { slider, .. }) => slider.setDoubleValue(*n),
            (Prop::Orientation(o), Widget::Separator(_)) => node.orientation = Some(*o),
            (Prop::Orientation(o), Widget::Slider { slider, .. }) => {
                slider.setVertical(o.vertical());
                node.orientation = Some(*o);
            }
            (Prop::Label(t), Widget::NumberInput(n)) => {
                n.field().setAccessibilityLabel(Some(&ns(t)));
                n.stepper().setAccessibilityLabel(Some(&ns(t)));
            }
            // The stepper holds the number; the field shows it. A new range
            // may clamp the number, which the core sends again after it.
            (Prop::Range { min, max }, Widget::NumberInput(n)) => {
                n.stepper().setMinValue(*min);
                n.stepper().setMaxValue(*max);
                n.show_number();
            }
            (Prop::Step(step), Widget::NumberInput(n)) => n.stepper().setIncrement(step.unwrap_or(1.0)),
            // On by default: AppKit's stepper wraps unless it's turned off.
            (Prop::WrapAround(on), Widget::NumberInput(n)) => n.stepper().setValueWraps(*on),
            (Prop::Number(v), Widget::NumberInput(n)) => {
                n.stepper().setDoubleValue(*v);
                n.show_number();
            }
            (Prop::Enabled(e), Widget::NumberInput(n)) => n.controls().iter().for_each(|c| c.setEnabled(*e)),
            (Prop::Image(source), Widget::Image(view)) => {
                view.setImage(ns_image(source).as_deref());
                node.image = Some(source.clone());
            }
            (Prop::ImageFit(fit), Widget::Image(view)) => {
                view.setImageScaling(match fit {
                    ImageFit::Contain => NSImageScaling::ScaleProportionallyUpOrDown,
                    ImageFit::Stretch => NSImageScaling::ScaleAxesIndependently,
                });
                node.fit = Some(*fit);
            }
            (Prop::Label(t), Widget::Image(view) | Widget::Icon(view) | Widget::FileIcon(view)) => {
                view.setAccessibilityLabel(Some(&ns(t)))
            }
            (Prop::File(_) | Prop::IconSize(_) | Prop::Thumbnail(_), Widget::FileIcon(view)) => {
                match prop {
                    Prop::File(path) => node.file = Some(path.clone()),
                    Prop::IconSize(points) => node.icon_size = Some(*points),
                    Prop::Thumbnail(on) => node.thumbnail = Some(*on),
                    _ => {}
                }
                let side = node.icon_size.unwrap_or(FILE_ICON_SIZE);
                show_file_icon(mtm, view, node.file.as_deref(), side, node.thumbnail == Some(true));
            }
            (Prop::Icon(name), Widget::Icon(view)) => {
                node.icon = Some(name.clone());
                view.setImage(symbol(name, node.icon_size).as_deref());
            }
            // Tints the symbol, as a template image; images in colour keep
            // theirs.
            (Prop::TextColor(color), Widget::Icon(view)) => {
                node.text_color = Some(*color);
                view.setContentTintColor(Some(&crate::custom::ns_color(*color)));
            }
            (Prop::IconSize(points), Widget::Icon(view)) => {
                node.icon_size = Some(*points);
                view.setImage(node.icon.as_deref().and_then(|name| symbol(name, node.icon_size)).as_deref());
            }
            // The button sizes the symbol for its bezel and font, before
            // its title unless it shows only the image.
            (Prop::Icon(name), Widget::Button(b)) => {
                node.icon = Some(name.clone());
                b.setImage(symbol(name, None).as_deref());
                b.setImagePosition(image_position(node.icon_only));
            }
            (Prop::IconOnly(only), Widget::Button(b)) => {
                node.icon_only = Some(*only);
                b.setImagePosition(image_position(node.icon_only));
            }
            (Prop::Label(t), Widget::GpuSurface(view)) => view.setAccessibilityLabel(Some(&ns(t))),
            (Prop::TakesInput(takes), Widget::GpuSurface(view)) => view.set_takes_input(*takes),
            (Prop::PointerLock(on), Widget::GpuSurface(view)) => view.set_pointer_lock(*on),
            (Prop::KeyboardGrab(on), Widget::GpuSurface(view)) => view.set_keyboard_grab(*on),
            (Prop::Cursor(cursor), Widget::GpuSurface(view)) => view.set_cursor(cursor),
            (Prop::Label(t), Widget::Progress(p) | Widget::Spinner { indicator: p, .. }) => {
                p.setAccessibilityLabel(Some(&ns(t)))
            }
            (Prop::Running(r), Widget::Spinner { indicator, running }) => {
                if *r {
                    unsafe { indicator.startAnimation(None) };
                } else {
                    unsafe { indicator.stopAnimation(None) };
                }
                *running = *r;
            }
            (Prop::Progress(progress), Widget::Progress(p)) => match progress {
                Some(fraction) => {
                    if p.isIndeterminate() {
                        unsafe { p.stopAnimation(None) };
                        p.setIndeterminate(false);
                        // Once animated, macOS 26's bar layer keeps drawing
                        // the indeterminate animation after it stops, whatever
                        // the value; setting the style again rebuilds it.
                        p.setStyle(NSProgressIndicatorStyle::Spinning);
                        p.setStyle(NSProgressIndicatorStyle::Bar);
                    }
                    p.setDoubleValue(*fraction);
                }
                None => {
                    p.setIndeterminate(true);
                    unsafe { p.startAnimation(None) };
                }
            },
            (Prop::Value(t), Widget::Field(f)) => {
                // Don't disturb the caret when the field already shows it.
                if f.stringValue().to_string() != *t {
                    f.setStringValue(&ns(t));
                }
            }
            (Prop::Placeholder(t), Widget::Field(f)) => f.setPlaceholderString(Some(&ns(t))),
            // For autofill; AppKit has no on-screen keyboard to pick.
            (Prop::InputPurpose(purpose), Widget::Field(f)) => f.setContentType(content_type(*purpose)),
            (Prop::Value(t), Widget::TextArea(area)) => area.set_string(t),
            // A text view has no placeholder: kept for the mirror.
            (Prop::Placeholder(t), Widget::TextArea(area)) => area.placeholder = Some(t.clone()),
            (Prop::ReadOnly(r), Widget::TextArea(area)) => area.set_read_only(*r),
            (Prop::Enabled(e), Widget::TextArea(area)) => area.set_enabled(*e),
            (Prop::Lines(lines), Widget::TextArea(area)) => area.lines = *lines,
            (Prop::LineWrap(on), Widget::TextArea(area)) => area.set_line_wrap(*on),
            // Still selectable, so its text can be copied; it takes focus
            // from a click, and from the keyboard only with Full Keyboard
            // Access.
            (Prop::ReadOnly(r), Widget::Field(f)) => f.setEditable(!r),
            (Prop::Checked(c), Widget::Checkbox(b)) => {
                node.checked = *c;
                // The mixed state shows over it.
                if b.state() != NSControlStateValueMixed {
                    b.setState(if *c { NSControlStateValueOn } else { NSControlStateValueOff })
                }
            }
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                node.mixed = Some(*m);
                // Allowed only while shown: clicks would cycle through it.
                b.setAllowsMixedState(*m);
                b.setState(match (*m, node.checked) {
                    (true, _) => NSControlStateValueMixed,
                    (false, true) => NSControlStateValueOn,
                    (false, false) => NSControlStateValueOff,
                });
            }
            (Prop::Checked(c), Widget::Button(b)) => {
                b.setState(if *c { NSControlStateValueOn } else { NSControlStateValueOff })
            }
            (Prop::Checked(c), Widget::Switch(s)) => {
                s.setState(if *c { NSControlStateValueOn } else { NSControlStateValueOff })
            }
            (Prop::Enabled(e), w) if w.control().is_some() => w.control().unwrap().setEnabled(*e),
            (Prop::TextStyle(style), Widget::Label(l)) => {
                node.text_style = Some(*style);
                l.setFont(Some(&label_font(node.text_style, node.weight, node.italic)));
            }
            (Prop::TextStyle(style), w) if w.control().is_some() => {
                w.control().unwrap().setFont(Some(&font(*style)));
                node.text_style = Some(*style);
            }
            // Weight and italics go on the text style's font.
            (Prop::FontWeight(weight), Widget::Label(l)) => {
                node.weight = Some(*weight);
                l.setFont(Some(&label_font(node.text_style, node.weight, node.italic)));
            }
            (Prop::Italic(italic), Widget::Label(l)) => {
                node.italic = Some(*italic);
                l.setFont(Some(&label_font(node.text_style, node.weight, node.italic)));
            }
            // Semantic colours are dynamic: they follow the appearance.
            (Prop::TextColor(color), Widget::Label(l)) => {
                node.text_color = Some(*color);
                l.setTextColor(Some(&crate::custom::ns_color(*color)));
            }
            (Prop::TextAlign(align), Widget::Label(l)) => {
                node.align = true;
                l.setAlignment(match align {
                    HorizontalAlign::Left => NSTextAlignment::Left,
                    HorizontalAlign::Center => NSTextAlignment::Center,
                    HorizontalAlign::Right => NSTextAlignment::Right,
                });
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone()),
            (Prop::RowFiles(files), Widget::List(list)) => list.set_row_files(files.clone()),
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode),
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style),
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::Cell(cell), Widget::Host(_)) => {
                node.row = Some(cell.row);
                node.column = Some(cell.column);
            }
            (Prop::Columns(columns), Widget::List(list)) => list.set_columns(mtm, columns),
            (Prop::Sort(sort), Widget::List(list)) => list.set_sort(*sort),
            (Prop::ScrollAxes(axes), Widget::Scroll(scroll)) => {
                node.scroll_axes = *axes;
                set_scrollers(scroll, node.scroll_axes, node.scroll_bars);
            }
            // A scroll view without scrollers still scrolls by wheel and
            // trackpad.
            (Prop::ScrollBars(show), Widget::Scroll(scroll)) => {
                node.scroll_bars = *show;
                set_scrollers(scroll, node.scroll_axes, node.scroll_bars);
            }
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                // Return clicks the default button, Escape the cancel button.
                b.setKeyEquivalent(&ns(match role {
                    ButtonRole::Default => "\r",
                    ButtonRole::Cancel => "\u{1b}",
                    ButtonRole::Normal | ButtonRole::Destructive => "",
                }));
                b.setHasDestructiveAction(*role == ButtonRole::Destructive);
                node.role = Some(*role);
            }
            (Prop::ButtonStyle(style), Widget::Button(b)) => {
                b.setBordered(*style != ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::LayoutDirection(direction), widget) => set_direction(widget, *direction),
            (Prop::Tooltip(t), widget) => {
                let text = (!t.is_empty()).then(|| ns(t));
                // On the views the pointer rests on, which cover the node's.
                match widget {
                    Widget::NumberInput(n) => {
                        n.field().setToolTip(text.as_deref());
                        n.stepper().setToolTip(text.as_deref());
                    }
                    Widget::List(list) => list.table.setToolTip(text.as_deref()),
                    Widget::TextArea(area) => area.text.setToolTip(text.as_deref()),
                    _ => {}
                }
                widget.view().setToolTip(text.as_deref());
            }
            // On the host: tracking areas are geometric, so its subviews
            // count too.
            (Prop::Hover(on), widget @ (Widget::Host(_) | Widget::Group { .. })) => {
                let view = widget.view();
                if let Some((_, area)) = node.hover.take() {
                    view.removeTrackingArea(&area);
                }
                if *on {
                    node.hover = Some(crate::classes::HoverTracker::track(mtm, view, id, events));
                }
            }
            (Prop::ContextMenu(entries), widget) => {
                let target = match node.context_menu.take() {
                    Some((_, target)) => target,
                    None => ClosureTarget::new(mtm, move |sender| {
                        if let Some(item) = sender.downcast_ref::<NSMenuItem>() {
                            events.emit(id, UiEvent::ContextMenuItem(item.tag() as u32));
                        }
                    }),
                };
                // AppKit shows a view's menu on a right-click or a Control-
                // click, and a view without one passes the click up to its
                // superview: children show their container's.
                let menu = (!entries.is_empty()).then(|| {
                    let target = ItemTarget { object: &target, action: sel!(fire:) };
                    crate::services::context_menu(mtm, entries, target)
                });
                // On the views the pointer rests on, as tooltips are. A
                // pop-up button's menu is its options, which a right-click
                // shows too: it keeps the menu on the node.
                let set = |view: &NSView| unsafe { view.setMenu(menu.as_deref()) };
                match widget {
                    Widget::Select(_) | Widget::MenuButton { .. } => {}
                    Widget::NumberInput(n) => {
                        set(n.field());
                        set(n.stepper());
                    }
                    Widget::List(list) => set(&list.table),
                    _ => {}
                }
                if !matches!(widget, Widget::Select(_) | Widget::MenuButton { .. }) {
                    set(widget.view());
                }
                node.context_menu = Some((entries.clone(), target));
            }
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            (Prop::Custom(new), Widget::Custom { view, render, props }) => {
                if props != new {
                    render.update(view, props.props(), new.props());
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(drawing), Widget::Drawn { view, .. }) => view.set_drawing(drawing.clone()),
            (Prop::Native(opaque), Widget::Native { view, last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    payload.apply(view);
                }
                *last = opaque.clone();
            }
            _ => {}
        }
        if let Prop::SidebarShown(on) = prop
            && let Some(Widget::Window { split: Some(split), .. }) =
                node.parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget)
        {
            split.set_shown(*on);
        }
    }
}

/// AppKit's steps are tick marks, which the knob then only stops at.
fn set_ticks(slider: &NSSlider, step: Option<f64>) {
    let range = slider.maxValue() - slider.minValue();
    match step.filter(|s| *s > 0.0 && range > 0.0) {
        Some(step) => {
            slider.setNumberOfTickMarks((range / step).round() as isize + 1);
            slider.setAllowsTickMarkValuesOnly(true);
        }
        None => {
            slider.setNumberOfTickMarks(0);
            slider.setAllowsTickMarkValuesOnly(false);
        }
    }
}

/// The content type AppKit's autofill takes for a purpose.
fn content_type(purpose: InputPurpose) -> Option<&'static NSTextContentType> {
    Some(unsafe {
        match purpose {
            InputPurpose::Text => return None,
            InputPurpose::Email => NSTextContentTypeEmailAddress,
            InputPurpose::Url => NSTextContentTypeURL,
            InputPurpose::Phone => NSTextContentTypeTelephoneNumber,
        }
    })
}

/// The purpose a field's content type stands for; any other, or none, is
/// text.
pub(super) fn input_purpose(shown: Option<&NSTextContentType>) -> InputPurpose {
    [InputPurpose::Email, InputPurpose::Url, InputPurpose::Phone]
        .into_iter()
        .find(|p| shown.zip(content_type(*p)).is_some_and(|(shown, t)| shown.isEqualToString(t)))
        .unwrap_or_default()
}

/// Mirrors a widget's own drawing, and the parts it places itself. A view's
/// direction mirrors what reads it (sliders, stack views, tables, scroll
/// views' scrollers); a checkbox's box and a field's text are placed by
/// the app's direction, so they get theirs here too (see
/// `locale::set_app_locale`).
fn set_direction(widget: &Widget, direction: LayoutDirection) {
    let direction = match direction {
        LayoutDirection::LeftToRight => NSUserInterfaceLayoutDirection::LeftToRight,
        LayoutDirection::RightToLeft => NSUserInterfaceLayoutDirection::RightToLeft,
    };
    widget.view().setUserInterfaceLayoutDirection(direction);
    match widget {
        Widget::Checkbox(b) => b.setImagePosition(super::toggle_image_position(direction)),
        Widget::RadioGroup(group) => group.set_direction(direction),
        Widget::Field(f) => f.setAlignment(super::field_alignment(direction)),
        Widget::NumberInput(n) => n.set_direction(direction),
        Widget::TextArea(area) => {
            area.text.setUserInterfaceLayoutDirection(direction);
            area.text.setAlignment(super::field_alignment(direction));
        }
        Widget::List(list) => list.table.setUserInterfaceLayoutDirection(direction),
        Widget::Sidebar(sidebar) => sidebar.table.setUserInterfaceLayoutDirection(direction),
        // Its title is laid out again only when the box is: a title set
        // just before kept the old side, cut at the old width.
        Widget::Group { frame, .. } => {
            frame.setUserInterfaceLayoutDirection(direction);
            frame.setNeedsLayout(true);
        }
        _ => {}
    }
}

/// 0 is AppKit's "no limit". A truncating line-break mode makes a label
/// one line, so only a single line has its start or middle cut off; more
/// lines wrap at words, and the cell cuts off the end of the last one it
/// shows, the only truncation AppKit wraps with.
fn set_line_limit(label: &NSTextField, lines: Option<u32>, truncation: Truncation) {
    label.setMaximumNumberOfLines(lines.map_or(0, |n| n as isize));
    label.setLineBreakMode(match (lines, truncation) {
        (Some(1), Truncation::Start) => NSLineBreakMode::ByTruncatingHead,
        (Some(1), Truncation::Middle) => NSLineBreakMode::ByTruncatingMiddle,
        _ => NSLineBreakMode::ByWordWrapping,
    });
    if let Some(cell) = label.cell() {
        cell.setTruncatesLastVisibleLine(lines.is_some());
    }
}
