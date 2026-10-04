//! Setting props on native widgets.

use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib, pango};
use mitsuami_core::{
    ButtonStyle, Color, Command, HorizontalAlign, ImageFit, ImageSource, InputPurpose, LayoutDirection, NodeId, Prop,
    Truncation, UiEvent,
};

use crate::custom::NativePayload;
use crate::file_drop::FileDropTarget;
use crate::services::ContextMenu;

use super::button::{ROLE_CLASSES, role_class};
use super::scroll::{scroll_axes, scroll_bars, set_scroll_policy};
use super::slider::{set_increments, update_marks};
use super::text::{
    COLOR_CLASSES, TEXT_STYLE_CLASSES, buffer_text, color_class, pango_weight, replace_attrs, set_icon_color,
    text_style_class,
};
use super::window::{escape_closes, prop_owner, resize};
use super::{State, Widget, violation};

impl State {
    pub(super) fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) {
        // Transient for the window it belongs to, and modal. GTK's modal
        // windows block the whole app, so both modalities are the same
        // here; GNOME attaches modal dialogs to their parent itself. Not
        // `destroy_with_parent`: the core destroys it.
        if let Prop::Modal { owner, modality } = prop {
            let owner = owner.and_then(|o| match self.nodes.get(&o).map(|n| &n.widget) {
                Some(Widget::Window(parts)) => Some(parts.window.clone()),
                _ => None,
            });
            let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
            if let Widget::Window(parts) = &node.widget {
                parts.window.set_transient_for(owner.as_ref());
                parts.window.set_modal(true);
                if node.modal.is_none() {
                    parts.window.add_controller(escape_closes());
                }
                node.modal = Some((prop_owner(prop), *modality));
                if self.menus.set_modal(id) {
                    self.menus.show_in(id, parts);
                }
            }
            return;
        }
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window(parts)) => {
                parts.window.set_title(Some(t));
                // Beside a sidebar, its page is titled after the window.
                if let Some(split) = &parts.split {
                    split.set_title(t);
                }
            }
            (Prop::Sections(sections), Widget::Sidebar(sidebar)) => sidebar.set_sections(sections.clone()),
            (Prop::SelectedIndex(index), Widget::Sidebar(sidebar)) => sidebar.set_selected(*index),
            (Prop::SidebarShown(shown), Widget::Sidebar(sidebar)) => sidebar.set_shown(*shown),
            (Prop::TabTitles(titles), Widget::Tabs(tabs)) => tabs.set_titles(titles.clone()),
            (Prop::TabIcons(icons), Widget::Tabs(tabs)) => tabs.set_icons(icons.clone()),
            (Prop::TabsStyle(style), Widget::Tabs(tabs)) => tabs.set_style(*style),
            (Prop::Title(title), Widget::Group(group)) => group.set_title(title),
            (Prop::SelectedIndex(index), Widget::Tabs(tabs)) => tabs.set_selected(*index),
            (Prop::FullScreen(on), Widget::Window(parts)) => parts.full_screen.set(&parts.window, *on),
            (Prop::MinSize(min), Widget::Window(parts)) => {
                parts.min_size.app.set(Some(*min));
                parts.min_size.apply(&parts.window, &parts.host);
                // A shown window grows to its new minimum when GTK next
                // sizes it, which Broadway (before GTK 4.16) does only when
                // it's presented: it's resized to it, as the app would.
                let grown = parts.host.window_root().and_then(|r| r.resizing.get());
                if let Some(size) = grown
                    && parts.window.is_mapped()
                    && !parts.window.is_maximized()
                    && !parts.full_screen.in_effect(&parts.window)
                {
                    let (width, height) = parts.extra(size);
                    resize(&parts.window, size.width as i32 + width, size.height as i32 + height);
                }
            }
            // GTK 4 can't hold one side of a window: the content sets its
            // size, which the user can't change, as GNOME's dialogs that
            // fit their content.
            (Prop::HeightFollowsContent(on), Widget::Window(parts)) => {
                parts.height_locked = *on;
                parts.window.set_resizable(parts.resizable && !parts.height_locked);
            }
            (Prop::Resizable(on), Widget::Window(parts)) => {
                parts.resizable = *on;
                parts.window.set_resizable(parts.resizable && !parts.height_locked);
            }
            (Prop::Maximized(on), Widget::Window(parts)) => parts.maximized.set(&parts.window, &parts.host, *on),
            (Prop::Text(t), Widget::Label(l)) => l.set_text(t),
            (Prop::Selectable(on), Widget::Label(l)) => l.set_selectable(*on),
            // GTK limits the lines of wrapping labels that ellipsize.
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                l.set_lines(lines.map_or(-1, |n| n as i32));
                l.set_ellipsize(ellipsize_mode(*lines, node.truncation.unwrap_or_default()));
            }
            (Prop::Truncation(truncation), Widget::Label(l)) => {
                node.truncation = Some(*truncation);
                let lines = (l.ellipsize() != pango::EllipsizeMode::None).then_some(l.lines() as u32);
                l.set_ellipsize(ellipsize_mode(lines, *truncation));
            }
            (Prop::TextColor(color), Widget::Label(l)) => {
                for (class, _) in COLOR_CLASSES {
                    l.remove_css_class(class);
                }
                let mut attrs = Vec::new();
                match color_class(*color) {
                    Some(class) => l.add_css_class(class),
                    // The theme's foreground, as the label has without.
                    None if *color == Color::Label => {}
                    None => {
                        let rgba = crate::custom::rgba(l.upcast_ref(), *color);
                        let channel = |v: f32| (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
                        attrs.push(
                            pango::AttrColor::new_foreground(
                                channel(rgba.red()),
                                channel(rgba.green()),
                                channel(rgba.blue()),
                            )
                            .into(),
                        );
                        attrs.push(pango::AttrInt::new_foreground_alpha(channel(rgba.alpha())).into());
                    }
                }
                replace_attrs(l, &[pango::AttrType::Foreground, pango::AttrType::ForegroundAlpha], attrs);
                node.text_color = Some(*color);
            }
            // Over the text style's class, whose weight it replaces.
            (Prop::FontWeight(weight), Widget::Label(l)) => {
                replace_attrs(
                    l,
                    &[pango::AttrType::Weight],
                    vec![pango::AttrInt::new_weight(pango_weight(*weight)).into()],
                );
            }
            (Prop::Italic(italic), Widget::Label(l)) => {
                let style = if *italic { pango::Style::Italic } else { pango::Style::Normal };
                replace_attrs(l, &[pango::AttrType::Style], vec![pango::AttrInt::new_style(style).into()]);
            }
            (Prop::TextAlign(align), Widget::Label(l)) => {
                node.align = Some(*align);
                align_label(l, *align);
            }
            // A widget GTK hasn't been told takes GTK's default, not its
            // parent's: every node is told (see `locale::set_app_locale`).
            (Prop::LayoutDirection(direction), widget) => {
                let direction = match direction {
                    LayoutDirection::LeftToRight => gtk::TextDirection::Ltr,
                    LayoutDirection::RightToLeft => gtk::TextDirection::Rtl,
                };
                widget.widget().set_direction(direction);
                widget.focus_widget().set_direction(direction);
                if let (Widget::Label(l), Some(align)) = (widget, node.align) {
                    align_label(l, align);
                }
            }
            (Prop::Label(t), Widget::Button(b)) => {
                node.button.label = t.clone();
                node.button.show(b);
            }
            (Prop::Icon(name), Widget::Button(b)) => {
                node.button.icon = Some(name.clone());
                node.button.show(b);
            }
            (Prop::IconOnly(only), Widget::Button(b)) => {
                node.button.icon_only = Some(*only);
                node.button.show(b);
            }
            (Prop::Label(t), Widget::MenuButton { button, .. }) => {
                node.button.label = t.clone();
                node.button.show(button);
            }
            (Prop::Icon(name), Widget::MenuButton { button, .. }) => {
                node.button.icon = Some(name.clone());
                node.button.show(button);
            }
            (Prop::IconOnly(only), Widget::MenuButton { button, .. }) => {
                node.button.icon_only = Some(*only);
                node.button.show(button);
            }
            (Prop::Menu(entries), Widget::MenuButton { button, menu }) => menu.set(button.upcast_ref(), entries),
            (Prop::ButtonStyle(style), Widget::MenuButton { button, .. }) => {
                button.set_has_frame(*style != ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Label(t), Widget::Checkbox(c)) => c.set_label(Some(t)),
            (Prop::Label(t), Widget::Switch(s)) => {
                s.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::Select { dropdown, .. }) => {
                dropdown.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Options(new), Widget::Select { dropdown, options }) => {
                // Replacing the items moves the selection; the chosen index
                // stays if it can, else the first option is chosen, as the
                // core does. It sends the index when that changes it.
                let chosen = dropdown.selected();
                let new: Vec<&str> = new.iter().map(String::as_str).collect();
                options.splice(0, options.n_items(), &new);
                let count = options.n_items();
                if count > 0 {
                    dropdown.set_selected(if chosen < count { chosen } else { 0 });
                }
            }
            (Prop::Label(t), Widget::RadioGroup(group)) => {
                group.column.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Options(options), Widget::RadioGroup(group)) => group.set_options(options),
            (Prop::SelectedIndex(index), Widget::RadioGroup(group)) => group.set_selected(*index),
            (Prop::Label(t), Widget::Slider { scale, .. }) => {
                scale.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Range { min, max }, Widget::Slider { scale, steps }) => {
                scale.set_range(*min, *max);
                set_increments(scale, steps.step.get());
                update_marks(scale, steps);
            }
            (Prop::Step(new), Widget::Slider { scale, steps }) => {
                steps.step.set(*new);
                set_increments(scale, *new);
                update_marks(scale, steps);
            }
            (Prop::Number(n), Widget::Slider { scale, .. }) => scale.set_value(*n),
            (Prop::Orientation(o), Widget::Slider { scale, .. }) => {
                scale.set_orientation(if o.vertical() {
                    gtk::Orientation::Vertical
                } else {
                    gtk::Orientation::Horizontal
                });
                // GTK runs vertical scales top down; apps invert them so
                // that up is more, as on the other platforms.
                scale.set_inverted(o.vertical());
                node.orientation = Some(*o);
            }
            (Prop::Orientation(o), Widget::Separator(line)) => line.set_orientation(if o.vertical() {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            }),
            (Prop::Label(t), Widget::SpinButton(spin)) => {
                spin.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            // May clamp the value; the core sends the value after the range.
            (Prop::Range { min, max }, Widget::SpinButton(spin)) => spin.set_range(*min, *max),
            (Prop::Step(step), Widget::SpinButton(spin)) => {
                let step = step.unwrap_or(1.0);
                spin.set_increments(step, step * 10.0);
            }
            (Prop::Number(n), Widget::SpinButton(spin)) => spin.set_value(*n),
            (Prop::WrapAround(on), Widget::SpinButton(spin)) => spin.set_wrap(*on),
            (Prop::Label(t), Widget::Progress { bar, .. }) => {
                bar.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::Spinner(s)) => {
                s.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::GpuSurface(surface)) => {
                surface.area.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (
                Prop::TakesInput(_) | Prop::PointerLock(_) | Prop::KeyboardGrab(_) | Prop::Cursor(_),
                Widget::GpuSurface(surface),
            ) => surface.set_prop(prop),
            (Prop::Icon(name), Widget::Icon { image, .. }) => {
                image.set_icon_name(Some(name.as_str()).filter(|n| !n.is_empty()))
            }
            // Unsized, GTK's normal icon size (16 px in Adwaita and Breeze).
            (Prop::TextColor(color), Widget::Icon { image, .. }) => {
                set_icon_color(image, *color);
                node.text_color = Some(*color);
            }
            (Prop::IconSize(points), Widget::Icon { image, size }) => {
                image.set_pixel_size(points.round() as i32);
                *size = Some(*points);
            }
            (Prop::File(_) | Prop::Thumbnail(_), Widget::FileIcon { image, file, thumbnail, loading, .. }) => {
                match prop {
                    Prop::File(path) => *file = Some(path.clone()),
                    Prop::Thumbnail(on) => *thumbnail = Some(*on),
                    _ => {}
                }
                if let Some(old) = loading.take() {
                    old.cancel();
                }
                // A new image may have another natural size (a thumbnail
                // isn't square): the core measures it again.
                let events = self.events.clone();
                let shown = move || events.emit(id, UiEvent::Remeasure);
                let thumbnail = *thumbnail == Some(true);
                *loading = super::file_icon::show(image, file.as_deref(), thumbnail, &self.loads, shown);
            }
            (Prop::IconSize(points), Widget::FileIcon { image, size, .. }) => {
                image.set_pixel_size(points.round() as i32);
                *size = Some(*points);
            }
            (Prop::Label(t), Widget::Icon { image, .. } | Widget::FileIcon { image, .. }) => {
                image.update_property(&[gtk::accessible::Property::Label(t)]);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::Picture { picture, .. }) => {
                picture.set_alternative_text(Some(t));
                node.a11y_label = Some(t.clone());
            }
            // GTK decodes files as it's given them; one it can't read
            // shows nothing, and measures nothing.
            (Prop::Image(new), Widget::Picture { picture, source, .. }) => {
                match new {
                    ImageSource::File(path) => picture.set_filename(Some(path)),
                    // GDK makes no texture without pixels: it shows nothing.
                    ImageSource::Pixels(pixels) if pixels.width() == 0 || pixels.height() == 0 => {
                        picture.set_paintable(None::<&gdk::Paintable>)
                    }
                    ImageSource::Pixels(pixels) => {
                        let texture = gdk::MemoryTexture::new(
                            pixels.width() as i32,
                            pixels.height() as i32,
                            gdk::MemoryFormat::R8g8b8a8,
                            &glib::Bytes::from(pixels.rgba()),
                            pixels.width() as usize * 4,
                        );
                        picture.set_paintable(Some(&texture));
                    }
                }
                *source = Some(new.clone());
            }
            (Prop::ImageFit(new), Widget::Picture { picture, fit, .. }) => {
                picture.set_content_fit(match new {
                    ImageFit::Contain => gtk::ContentFit::Contain,
                    ImageFit::Stretch => gtk::ContentFit::Fill,
                });
                *fit = Some(*new);
            }
            // Stopped, a GTK spinner draws nothing.
            (Prop::Running(r), Widget::Spinner(s)) => s.set_spinning(*r),
            (Prop::Progress(progress), Widget::Progress { bar, pulsing }) => match progress {
                // Its timer stops at its next tick. A new flag for the next
                // timer: set again before then, the old one would go on too.
                Some(fraction) => {
                    pulsing.set(false);
                    *pulsing = Rc::default();
                    bar.set_fraction(*fraction);
                }
                None if !pulsing.replace(true) => {
                    let (bar, pulsing) = (bar.downgrade(), pulsing.clone());
                    glib::timeout_add_local(Duration::from_millis(100), move || match bar.upgrade() {
                        Some(bar) if pulsing.get() => {
                            bar.pulse();
                            glib::ControlFlow::Continue
                        }
                        _ => glib::ControlFlow::Break,
                    });
                }
                None => {}
            },
            // With options, GTK always has one chosen (its selection
            // autoselects), and so does the core.
            (Prop::SelectedIndex(Some(index)), Widget::Select { dropdown, .. }) => dropdown.set_selected(*index as u32),
            (Prop::Value(t), Widget::Entry(e)) => {
                // Don't disturb the caret when the field already shows it.
                if e.text() != t.as_str() {
                    e.set_text(t);
                }
            }
            (Prop::Placeholder(t), Widget::Entry(e)) => e.set_placeholder_text(Some(t)),
            // Still focusable and selectable, so its text can be copied.
            (Prop::ReadOnly(r), Widget::Entry(e)) => e.set_editable(!r),
            // For the on-screen keyboard and input methods.
            (Prop::InputPurpose(purpose), Widget::Entry(e)) => e.set_input_purpose(match purpose {
                InputPurpose::Text => gtk::InputPurpose::FreeForm,
                InputPurpose::Email => gtk::InputPurpose::Email,
                InputPurpose::Url => gtk::InputPurpose::Url,
                InputPurpose::Phone => gtk::InputPurpose::Phone,
            }),
            (Prop::Value(t), Widget::Password(e)) => {
                if e.text() != t.as_str() {
                    e.set_text(t);
                }
            }
            (Prop::Placeholder(t), Widget::Password(e)) => e.set_placeholder_text(Some(t)),
            (Prop::Value(t), Widget::Search(e)) => {
                if e.text() != t.as_str() {
                    e.set_text(t);
                }
            }
            (Prop::Placeholder(t), Widget::Search(e)) => e.set_placeholder_text(Some(t)),
            (Prop::Value(t), Widget::TextArea { view, .. }) => {
                let buffer = view.buffer();
                if buffer_text(&buffer) != *t {
                    buffer.set_text(t);
                }
            }
            (Prop::Placeholder(t), Widget::TextArea { placeholder, .. }) => *placeholder = Some(t.clone()),
            // Still focusable and selectable, so its text can be copied.
            (Prop::ReadOnly(r), Widget::TextArea { view, .. }) => view.set_editable(!r),
            (Prop::Lines(n), Widget::TextArea { lines, .. }) => *lines = *n,
            // Unwrapped, long lines scroll sideways.
            (Prop::LineWrap(on), Widget::TextArea { scrolled, view, .. }) => {
                view.set_wrap_mode(if *on { gtk::WrapMode::WordChar } else { gtk::WrapMode::None });
                scrolled.set_policy(
                    if *on { gtk::PolicyType::Never } else { gtk::PolicyType::Automatic },
                    gtk::PolicyType::Automatic,
                );
            }
            (Prop::Checked(c), Widget::Checkbox(b)) => b.set_active(*c),
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                b.set_inconsistent(*m);
                node.mixed = Some(*m);
            }
            (Prop::Checked(c), Widget::Switch(s)) => s.set_active(*c),
            // Disabled, a text area shows no selection: it keeps the caret.
            (Prop::Enabled(e), Widget::TextArea { scrolled, view, .. }) => {
                scrolled.set_sensitive(*e);
                if !e {
                    let buffer = view.buffer();
                    buffer.place_cursor(&buffer.iter_at_mark(&buffer.get_insert()));
                }
            }
            (Prop::Enabled(e), w) if w.is_control() => w.widget().set_sensitive(*e),
            (Prop::TextStyle(style), w) if w.is_control() => {
                let widget = w.widget();
                for class in TEXT_STYLE_CLASSES {
                    widget.remove_css_class(class);
                }
                if let Some(class) = text_style_class(*style) {
                    widget.add_css_class(class);
                }
                node.text_style = Some(*style);
            }
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                for class in ROLE_CLASSES {
                    b.remove_css_class(class);
                }
                if let Some(class) = role_class(*role) {
                    b.add_css_class(class);
                }
                node.role = Some(*role);
            }
            (Prop::ButtonStyle(style), Widget::Button(b)) => {
                b.set_has_frame(*style != ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Checked(c), Widget::Button(b)) => {
                if let Some(toggle) = b.downcast_ref::<gtk::ToggleButton>() {
                    toggle.set_active(*c);
                }
            }
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            // On the widget the pointer rests on: a list's view, not the
            // scrolled window around it. GTK reads it to assistive
            // technology as the description.
            (Prop::Tooltip(t), widget) => {
                widget.focus_widget().set_tooltip_text(Some(t.as_str()).filter(|t| !t.is_empty()))
            }
            // A motion controller reports the pointer entering the host
            // or any of its children, and leaving them all, not moving
            // between them.
            (Prop::Hover(on), widget @ (Widget::Host(_) | Widget::Group(_))) => {
                if let Some(motion) = node.hover.take() {
                    widget.widget().remove_controller(&motion);
                }
                if *on {
                    let motion = gtk::EventControllerMotion::new();
                    let events = self.events.clone();
                    motion.connect_enter(move |_, _, _| events.emit(id, UiEvent::Hover(true)));
                    let events = self.events.clone();
                    motion.connect_leave(move |_| events.emit(id, UiEvent::Hover(false)));
                    widget.widget().add_controller(motion.clone());
                    node.hover = Some(motion);
                }
            }
            // A click gesture counts the presses; a control under the
            // pointer claims its own, and a label's aren't ours to take.
            (Prop::DoubleClick(on), widget @ (Widget::Host(_) | Widget::Group(_))) => {
                if let Some((click, _)) = node.double_click.take() {
                    widget.widget().remove_controller(&click);
                }
                if *on {
                    let click = gtk::GestureClick::new();
                    let events = self.events.clone();
                    let report: Rc<dyn Fn()> = Rc::new(move || events.emit(id, UiEvent::DoubleClick));
                    let fire = report.clone();
                    click.connect_pressed(move |click, presses, x, y| {
                        let Some(host) = click.widget() else { return };
                        if presses != 2 || on_control(&host, x, y) {
                            return;
                        }
                        fire();
                    });
                    widget.widget().add_controller(click.clone());
                    node.double_click = Some((click, report));
                }
            }
            // On the widget the pointer rests on, as the tooltip is, and
            // for its children without one of their own.
            (Prop::ContextMenu(entries), widget) => {
                let events = self.events.clone();
                let focus = widget.focus_widget();
                node.context_menu
                    .get_or_insert_with(|| {
                        let activate = move |item| events.emit(id, UiEvent::ContextMenuItem(item));
                        ContextMenu::new(&focus, Rc::new(activate))
                    })
                    .set(&focus, entries);
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone()),
            (Prop::RowFiles(files), Widget::List(list)) => list.set_row_files(files.clone()),
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode),
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style),
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows),
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::Cell(cell), Widget::Host(_)) => {
                node.row = Some(cell.row);
                node.column = Some(cell.column);
            }
            (Prop::Columns(columns), Widget::List(list)) => list.set_columns(columns),
            (Prop::Sort(sort), Widget::List(list)) => list.set_sort(*sort),
            (Prop::FileDrop(drop), widget @ (Widget::Host(_) | Widget::Group(_))) => {
                let widget = widget.widget().clone();
                let target = node.file_drop.take().flatten();
                node.file_drop = Some(match (target, drop) {
                    (Some(target), Some(drop)) => {
                        target.set(drop.clone());
                        Some(target)
                    }
                    (None, Some(drop)) => Some(FileDropTarget::new(id, self.events.clone(), &widget, drop.clone())),
                    (Some(target), None) => {
                        target.remove();
                        None
                    }
                    (None, None) => None,
                });
            }
            // On the list's scrolled window: after the view, which uses
            // its own keys first.
            (Prop::Keys(keys), widget @ (Widget::Host(_) | Widget::Group(_) | Widget::List(_))) => {
                crate::keys::set_keys(&mut node.keys, widget.widget(), id, &self.events, keys)
            }
            (Prop::ScrollAxes(axes), Widget::Scroll { scrolled, .. }) => {
                set_scroll_policy(scrolled, *axes, scroll_bars(scrolled))
            }
            (Prop::ScrollBars(show), Widget::Scroll { scrolled, .. }) => {
                set_scroll_policy(scrolled, scroll_axes(scrolled), *show)
            }
            (Prop::Custom(new), Widget::Custom { widget, render, props }) => {
                if props != new {
                    render.update(widget, props.props(), new.props());
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(drawing), Widget::Drawn { drawn, .. }) => drawn.set_drawing(drawing.clone()),
            (Prop::Native(opaque), Widget::Native { widget, last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    payload.apply(widget);
                }
                *last = opaque.clone();
            }
            _ => {}
        }
    }
}

/// The core resolved the direction, so left is left: GTK mirrors `xalign`
/// and `justify` in a right-to-left label, so they're mirrored back.
fn align_label(l: &gtk::Label, align: HorizontalAlign) {
    let rtl = l.direction() == gtk::TextDirection::Rtl;
    let (xalign, justify) = match (align, rtl) {
        (HorizontalAlign::Center, _) => (0.5, gtk::Justification::Center),
        (HorizontalAlign::Left, false) | (HorizontalAlign::Right, true) => (0.0, gtk::Justification::Left),
        (HorizontalAlign::Right, false) | (HorizontalAlign::Left, true) => (1.0, gtk::Justification::Right),
    };
    l.set_xalign(xalign);
    l.set_justify(justify);
}

/// Pango cuts off the last line it shows where the app asked, whatever the
/// limit: in the middle, it keeps the start of that line and the end of
/// the text.
fn ellipsize_mode(lines: Option<u32>, truncation: Truncation) -> pango::EllipsizeMode {
    match (lines, truncation) {
        (None, _) => pango::EllipsizeMode::None,
        (Some(_), Truncation::Start) => pango::EllipsizeMode::Start,
        (Some(_), Truncation::Middle) => pango::EllipsizeMode::Middle,
        (Some(_), Truncation::End) => pango::EllipsizeMode::End,
    }
}

/// Whether the point, in `host`'s coordinates, is on a control inside it:
/// one that takes focus (a button, a field), unlike a label.
fn on_control(host: &gtk::Widget, x: f64, y: f64) -> bool {
    let mut at = host.pick(x, y, gtk::PickFlags::DEFAULT);
    while let Some(widget) = at.filter(|w| w != host) {
        if widget.is_focusable() {
            return true;
        }
        at = widget.parent();
    }
    false
}
