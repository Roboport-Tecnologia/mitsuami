//! Making a node's widget.

use std::rc::Rc;

use mitsuami_core::{Command, DisplayList, EventValue, NodeId, PointerEvent, UiEvent, WidgetKind, find_prop};

use crate::custom::{ErasedRender, KirigamiCx, NativePayload};
use crate::events::node_key;
use crate::ffi::{self, Callback, QmlObject};
use crate::qml;
use crate::surface::SurfaceItem;

use super::{Node, State, Widget, scroll_offset, violation};

/// `Image.Status`: loaded, or failed.
const IMAGE_READY: i32 = 1;
const IMAGE_ERROR: i32 = 3;

impl State {
    pub(super) fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) {
        let events = self.events.clone();
        let widget = match kind {
            WidgetKind::Window => self.create_window(id, command),
            WidgetKind::Container => Widget::Host(QmlObject::load(&qml::container())),
            WidgetKind::ToolbarItem => {
                let host = QmlObject::load(&qml::container());
                let action = QmlObject::load(&qml::toolbar_action());
                action.set_object("mitsuamiItem", Some(host));
                Widget::ToolbarItem { host, action }
            }
            WidgetKind::Sidebar => {
                let page = QmlObject::load(&qml::sidebar());
                // The user's choice only: the app's doesn't emit it.
                page.connect("mitsuamiChosen()", move || {
                    if let Ok(index) = usize::try_from(page.int("mitsuamiSelected")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)));
                    }
                });
                Widget::Sidebar { page, sections: Vec::new() }
            }
            WidgetKind::Tabs => {
                let root = QmlObject::load(&qml::tabs());
                let pages = root.child("mitsuamiPages").expect("tab views have a page area");
                // The user's choice only: the app's doesn't emit it.
                root.connect("mitsuamiChosen()", move || {
                    if let Ok(index) = usize::try_from(root.int("mitsuamiSelected")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)));
                    }
                });
                Widget::Tabs { root, pages }
            }
            WidgetKind::Group => {
                let root = QmlObject::load(&qml::group());
                let group = root.child("mitsuamiGroupBox").expect("groups have a group box");
                let content = root.child("mitsuamiGroupContent").expect("groups have a content item");
                Widget::Group { root, group, content }
            }
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not a Kirigami one")
                        };
                        let item = render.create(custom.props(), &mut KirigamiCx::new(events.clone(), id));
                        Widget::Custom { item, render, props: custom }
                    }
                    None => {
                        let key = ffi::register(move |callback| {
                            if let Callback::Pointer(kind, position) = callback {
                                events.emit(id, UiEvent::Pointer(PointerEvent { kind, position }));
                            }
                        });
                        Widget::Drawn { item: QmlObject::drawn(key), props: custom, drawing: DisplayList::default() }
                    }
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not a Kirigami one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let measure = payload.spec.measure.clone();
                let item = create(&mut KirigamiCx::new(events.clone(), id));
                payload.apply(item);
                Widget::Native { item, measure, last: opaque }
            }
            // Selectable text is another item: chosen once, when it's made.
            WidgetKind::Text => {
                let selectable = matches!(command, Command::Create { props, .. }
                    if find_prop!(props, Selectable) == Some(true));
                Widget::Label(QmlObject::load(&if selectable { qml::selectable_label() } else { qml::label() }))
            }
            // `toggled` is the user's; `checkedChanged` fires for ours too.
            WidgetKind::ToggleButton => {
                let button = QmlObject::load(&qml::toggle_button());
                button.connect("toggled()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Bool(button.bool("checked"))))
                });
                Widget::Button(button)
            }
            WidgetKind::Button => {
                let button = QmlObject::load(&qml::button());
                button.connect("clicked()", move || events.emit(id, UiEvent::Click));
                Widget::Button(button)
            }
            // A click opens its menu: it reports only the item chosen.
            WidgetKind::MenuButton => Widget::MenuButton(QmlObject::load(&qml::menu_button())),
            WidgetKind::Checkbox | WidgetKind::Switch => {
                let switch = kind == WidgetKind::Switch;
                let toggle = QmlObject::load(&if switch { qml::switch() } else { qml::checkbox() });
                // `toggled` is the user's; `checkedChanged` fires for ours too.
                toggle.connect("toggled()", move || {
                    // Setting `checkState` to partly checked makes a box
                    // tristate; after a click, clicks mustn't cycle back.
                    if !switch {
                        toggle.set_bool("tristate", false);
                    }
                    events.emit(id, UiEvent::Changed(EventValue::Bool(toggle.bool("checked"))))
                });
                if switch { Widget::Switch(toggle) } else { Widget::Checkbox(toggle) }
            }
            WidgetKind::Select => {
                let select = QmlObject::load(&qml::select());
                // `activated` is the user's; `currentIndexChanged` fires for
                // ours too.
                select.connect("activated(int)", move || {
                    if let Ok(index) = usize::try_from(select.int("currentIndex")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)))
                    }
                });
                Widget::Select(select)
            }
            WidgetKind::RadioGroup => {
                let group = QmlObject::load(&qml::radio_group());
                group.connect("mitsuamiChosen()", move || {
                    if let Ok(index) = usize::try_from(group.int("mitsuamiSelected")) {
                        events.emit(id, UiEvent::Changed(EventValue::Index(index)))
                    }
                });
                Widget::RadioGroup(group)
            }
            WidgetKind::Slider => {
                let slider = QmlObject::load(&qml::slider());
                // `moved` is the user's; `valueChanged` fires for ours too.
                slider.connect("moved()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Number(slider.real("value"))))
                });
                Widget::Slider(slider)
            }
            WidgetKind::NumberInput => {
                let spin = QmlObject::load(&qml::number_input());
                // `valueModified` is the user's (buttons, arrow keys, a typed
                // number once it's committed); `valueChanged` fires for ours
                // too.
                spin.connect("valueModified()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Number(spin.int("value").into())))
                });
                Widget::NumberInput(spin)
            }
            WidgetKind::Progress => Widget::Progress(QmlObject::load(&qml::progress())),
            WidgetKind::Spinner => Widget::Spinner(QmlObject::load(&qml::spinner())),
            WidgetKind::Separator => Widget::Separator(QmlObject::load(&qml::separator())),
            WidgetKind::Icon => Widget::Icon(QmlObject::load(&qml::icon())),
            WidgetKind::GpuSurface => Widget::GpuSurface(SurfaceItem::new(id, events.clone())),
            WidgetKind::Image => {
                let item = QmlObject::load(&qml::image());
                // Loading is synchronous, but should an image finish (or
                // fail) later, its size changed.
                item.connect("statusChanged()", move || {
                    if matches!(item.int("status"), IMAGE_READY | IMAGE_ERROR) {
                        events.emit(id, UiEvent::Remeasure);
                    }
                });
                Widget::Image { item, source: None, fit: None, pixels: None }
            }
            WidgetKind::TextInput | WidgetKind::PasswordInput => {
                let qml = if kind == WidgetKind::TextInput { qml::text_field() } else { qml::password_field() };
                let field = QmlObject::load(&qml);
                let e = events.clone();
                // `textEdited` is the user's; `textChanged` fires for ours too.
                field
                    .connect("textEdited()", move || e.emit(id, UiEvent::Changed(EventValue::Text(field.str("text")))));
                // Return and Enter only; leaving the field doesn't submit.
                field.connect("accepted()", move || events.emit(id, UiEvent::Submit));
                Widget::Field(field)
            }
            WidgetKind::SearchInput => {
                let field = QmlObject::load(&qml::search_field());
                let e = events.clone();
                // The user's edits, the clear button's too; not ours.
                field.connect("mitsuamiEdited()", move || {
                    e.emit(id, UiEvent::Changed(EventValue::Text(field.str("text"))))
                });
                field.connect("mitsuamiSearched()", move || events.emit(id, UiEvent::Search(field.str("text"))));
                Widget::Field(field)
            }
            WidgetKind::TextArea => {
                let root = QmlObject::load(&qml::text_area());
                let area = root.child("mitsuamiTextArea").expect("text areas have their area");
                // `textChanged` fires for ours too; our sets are marked.
                root.connect("mitsuamiEdited()", move || {
                    events.emit(id, UiEvent::Changed(EventValue::Text(root.str("text"))))
                });
                Widget::TextArea { root, area }
            }
            WidgetKind::ScrollView => {
                let view = QmlObject::load(&qml::scroll_view());
                let flickable = view.child("mitsuamiFlickable").expect("scroll views have a flickable");
                for signal in ["contentXChanged()", "contentYChanged()"] {
                    let events = events.clone();
                    flickable.connect(signal, move || events.emit(id, UiEvent::Scrolled(scroll_offset(flickable))));
                }
                Widget::Scroll { view, flickable }
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
            WidgetKind::List => Widget::List(crate::list::List::new(id, events.clone())),
        };
        let item = widget.item();
        item.set_node(node_key(id));
        // A zero frame: the core only sends frames that differ from the
        // last one, and an item's size follows its implicit size until set.
        if kind != WidgetKind::Window {
            item.set_geometry(0.0, 0.0, 0.0, 0.0);
        }
        if widget.is_leaf() {
            // Not shown until it has a frame.
            item.set_bool("visible", false);
        }
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                parent: None,
                row: None,
                text_style: None,
                role: None,
                button_style: None,
                tabs_style: None,
                orientation: None,
                mixed: None,
                checked: false,
                icon_size: false,
                icon_only: false,
                tweak: None,
                tooltip: String::new(),
                modal: None,
                scroll_axes: None,
                a11y_label: None,
                context_menu: None,
                button_menu: None,
                file_drop: None,
                file_drop_given: false,
            },
        );
    }
}
