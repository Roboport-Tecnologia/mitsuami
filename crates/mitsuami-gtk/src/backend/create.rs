//! Creating a node's native widget.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, pango};
use mitsuami_core::{Command, EventValue, NodeId, Point, Prop, UiEvent, WidgetKind, find_prop};

use crate::custom::{DrawnArea, ErasedRender, GtkCx, NativePayload};
use crate::host::Host;
use crate::radio::RadioGroup;
use crate::services::ContextMenu;
use crate::sidebar::Sidebar;
use crate::surface::SurfaceArea;
use crate::tabs::Tabs;

use super::button::ButtonFace;
use super::slider::{STEPS, Steps, snap};
use super::text::{TEXT_AREA_MARGIN, buffer_text};
use super::{Node, State, Widget, violation};

impl State {
    pub(super) fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) {
        let events = self.events.clone();
        let mut settings_handlers = Vec::new();
        let widget = match kind {
            WidgetKind::Window => {
                // A dialog's menu holds only its own menus.
                if let Command::Create { props, .. } = command
                    && props.iter().any(|p| matches!(p, Prop::Modal { .. }))
                {
                    self.menus.set_modal(id);
                }
                Widget::Window(self.create_window(id, &mut settings_handlers))
            }
            WidgetKind::Container | WidgetKind::ToolbarItem => Widget::Host(Host::new(self.frames.clone(), None)),
            WidgetKind::Group => Widget::Group(crate::group::Group::new(self.frames.clone())),
            WidgetKind::Sidebar => Widget::Sidebar(Sidebar::new(id, events.clone())),
            WidgetKind::Tabs => Widget::Tabs(Tabs::new(id, events.clone(), self.frames.clone())),
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not a GTK one")
                        };
                        let widget = render.create(custom.props(), &mut GtkCx::new(events.clone(), id));
                        Widget::Custom { widget, render, props: custom }
                    }
                    None => Widget::Drawn { drawn: DrawnArea::new(events.clone(), id), props: custom },
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not a GTK one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let measure = payload.spec.measure.clone();
                let widget = create(&mut GtkCx::new(events.clone(), id));
                payload.apply(&widget);
                Widget::Native { widget, measure, last: opaque }
            }
            WidgetKind::Text => {
                let label = gtk::Label::new(None);
                label.set_wrap(true);
                label.set_wrap_mode(pango::WrapMode::Word);
                // Start-aligned and top-aligned in frames larger than the
                // text (GTK mirrors xalign for right-to-left text).
                label.set_xalign(0.0);
                label.set_yalign(0.0);
                Widget::Label(label)
            }
            WidgetKind::Button => {
                let button = gtk::Button::new();
                button.connect_clicked(move |_| events.emit(id, UiEvent::Click));
                Widget::Button(button)
            }
            // A button still, whose caption, icon and style are a button's.
            WidgetKind::ToggleButton => {
                let toggle = gtk::ToggleButton::new();
                toggle.connect_toggled(move |t| events.emit(id, UiEvent::Changed(EventValue::Bool(t.is_active()))));
                Widget::Button(toggle.upcast())
            }
            WidgetKind::MenuButton => {
                let button = gtk::MenuButton::new();
                let menu = ContextMenu::for_menu_button(
                    &button,
                    Rc::new(move |item| events.emit(id, UiEvent::MenuItem(item))),
                );
                Widget::MenuButton { button, menu }
            }
            WidgetKind::Checkbox => {
                let check = gtk::CheckButton::new();
                check.connect_toggled(move |c| {
                    // GTK leaves `inconsistent` to the app, and apps clear it
                    // when the user toggles the box.
                    if !events.is_muted() {
                        c.set_inconsistent(false);
                    }
                    events.emit(id, UiEvent::Changed(EventValue::Bool(c.is_active())))
                });
                Widget::Checkbox(check)
            }
            WidgetKind::Switch => {
                let switch = gtk::Switch::new();
                switch
                    .connect_active_notify(move |s| events.emit(id, UiEvent::Changed(EventValue::Bool(s.is_active()))));
                Widget::Switch(switch)
            }
            WidgetKind::Select => {
                let options = gtk::StringList::new(&[]);
                let dropdown = gtk::DropDown::new(Some(options.clone()), None::<gtk::Expression>);
                dropdown.connect_selected_notify(move |d| {
                    if d.selected() != gtk::INVALID_LIST_POSITION {
                        events.emit(id, UiEvent::Changed(EventValue::Index(d.selected() as usize)))
                    }
                });
                Widget::Select { dropdown, options }
            }
            WidgetKind::RadioGroup => Widget::RadioGroup(RadioGroup::new(events.clone(), id)),
            WidgetKind::Slider => {
                let scale = gtk::Scale::new(gtk::Orientation::Horizontal, None::<&gtk::Adjustment>);
                scale.connect_value_changed(move |s| events.emit(id, UiEvent::Changed(EventValue::Number(s.value()))));
                let steps = Rc::new(Steps::default());
                // SAFETY: the key only ever holds an `Rc<Steps>`.
                unsafe { scale.set_data(STEPS, steps.clone()) };
                // GTK's scales have no stepped mode: with a step, the user's
                // moves stop only on steps, as on AppKit and WinUI.
                let s = steps.clone();
                scale.connect_change_value(move |scale, _, value| match s.step.get() {
                    Some(step) if step > 0.0 => {
                        scale.set_value(snap(&scale.adjustment(), value, step));
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                });
                Widget::Slider { scale, steps }
            }
            WidgetKind::NumberInput => {
                // Steps of 1 and pages of 10, as `gtk_spin_button_new_with_range`
                // makes them, until the app gives a step.
                let spin = gtk::SpinButton::with_range(0.0, 100.0, 1.0);
                spin.set_digits(0);
                spin.set_numeric(true);
                // Typing is reported when GTK commits it: on Return or
                // when the field loses focus.
                spin.connect_value_changed(move |s| events.emit(id, UiEvent::Changed(EventValue::Number(s.value()))));
                Widget::SpinButton(spin)
            }
            WidgetKind::Progress => Widget::Progress { bar: gtk::ProgressBar::new(), pulsing: Rc::default() },
            WidgetKind::Spinner => Widget::Spinner(gtk::Spinner::new()),
            WidgetKind::Separator => Widget::Separator(gtk::Separator::new(gtk::Orientation::Horizontal)),
            WidgetKind::Icon => Widget::Icon { image: gtk::Image::new(), size: None },
            WidgetKind::FileIcon => {
                Widget::FileIcon { image: gtk::Image::new(), file: None, thumbnail: None, size: None }
            }
            WidgetKind::Image => Widget::Picture { picture: gtk::Picture::new(), source: None, fit: None },
            WidgetKind::GpuSurface => Widget::GpuSurface(SurfaceArea::new(id, events.clone())),
            WidgetKind::TextInput => {
                let entry = gtk::Entry::new();
                let e = events.clone();
                entry.connect_changed(move |entry| {
                    e.emit(id, UiEvent::Changed(EventValue::Text(entry.text().to_string())))
                });
                // Only Return activates; leaving the field doesn't submit.
                entry.connect_activate(move |_| events.emit(id, UiEvent::Submit));
                Widget::Entry(entry)
            }
            // Without the peek icon, GTK's default: GNOME apps add it where
            // they want it.
            WidgetKind::PasswordInput => {
                let entry = gtk::PasswordEntry::new();
                let e = events.clone();
                entry.connect_changed(move |entry| {
                    e.emit(id, UiEvent::Changed(EventValue::Text(entry.text().to_string())))
                });
                // Only Return activates; leaving the field doesn't submit.
                entry.connect_activate(move |_| events.emit(id, UiEvent::Submit));
                Widget::Password(entry)
            }
            // GTK's `search-changed` comes 150 ms after typing pauses, and at
            // once when the field is emptied, but for any change, the app's
            // too: `set_text` starts the same timer, which fires after the
            // commands are applied. So only a search that follows a user
            // edit is reported. Return searches at once.
            WidgetKind::SearchInput => {
                let entry = gtk::SearchEntry::new();
                let edited = Rc::new(Cell::new(false));
                let (e, ed) = (events.clone(), edited.clone());
                entry.connect_changed(move |entry| {
                    ed.set(!e.is_muted());
                    e.emit(id, UiEvent::Changed(EventValue::Text(entry.text().to_string())))
                });
                let (e, ed) = (events.clone(), edited.clone());
                entry.connect_search_changed(move |entry| {
                    if ed.replace(false) {
                        e.emit(id, UiEvent::Search(entry.text().to_string()));
                    }
                });
                // The search the timer would make is this one.
                entry.connect_activate(move |entry| {
                    edited.set(false);
                    events.emit(id, UiEvent::Search(entry.text().to_string()));
                });
                Widget::Search(entry)
            }
            WidgetKind::TextArea => {
                let view = gtk::TextView::new();
                // Wrapped at words, or within a word too long for a line, and
                // clear of the frame, as GNOME apps set up their text views:
                // GTK's own don't wrap, and have no margins.
                view.set_wrap_mode(gtk::WrapMode::WordChar);
                view.set_top_margin(TEXT_AREA_MARGIN);
                view.set_bottom_margin(TEXT_AREA_MARGIN);
                view.set_left_margin(TEXT_AREA_MARGIN);
                view.set_right_margin(TEXT_AREA_MARGIN);
                view.buffer().connect_changed(move |buffer| {
                    events.emit(id, UiEvent::Changed(EventValue::Text(buffer_text(buffer))))
                });
                let scrolled = gtk::ScrolledWindow::new();
                scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                scrolled.set_has_frame(true);
                scrolled.set_child(Some(&view));
                Widget::TextArea { scrolled, view, placeholder: None, lines: 1 }
            }
            WidgetKind::ScrollView => {
                let scrolled = gtk::ScrolledWindow::new();
                scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                let viewport = gtk::Viewport::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
                scrolled.set_child(Some(&viewport));
                let (h, v) = (scrolled.hadjustment(), scrolled.vadjustment());
                for adjustment in [&h, &v] {
                    let (events, h, v) = (events.clone(), h.clone(), v.clone());
                    adjustment.connect_value_changed(move |_| {
                        events.emit(id, UiEvent::Scrolled(Point::new(h.value() as f32, v.value() as f32)))
                    });
                }
                Widget::Scroll { scrolled, viewport }
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
            WidgetKind::List => Widget::List(crate::list::List::new(id, events.clone())),
            WidgetKind::Table => Widget::List(crate::list::List::new_table(id, events.clone())),
        };
        self.by_widget.borrow_mut().insert(widget.widget().clone(), id);
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                parent: None,
                row: None,
                column: None,
                text_style: None,
                text_color: None,
                role: None,
                button_style: None,
                orientation: None,
                mixed: None,
                button: ButtonFace::default(),
                tweak: None,
                a11y_label: None,
                modal: None,
                settings_handlers,
                context_menu: None,
                file_drop: None,
                keys: None,
                align: None,
                truncation: None,
            },
        );
    }
}
