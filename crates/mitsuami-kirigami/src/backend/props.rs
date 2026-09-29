//! Setting a prop on a node's widget.

use mitsuami_core::{
    ButtonRole, ButtonStyle, Color, Command, HorizontalAlign, ImageFit, ImageSource, Modality, NodeId, Prop,
    ScrollAxes, TabsStyle, UiEvent, WidgetKind,
};

use crate::custom::{NativePayload, flatten};
use crate::ffi;
use crate::qml;
use crate::services::ContextMenu;

use super::window::sections_json;
use super::{
    ALIGN_H_CENTER, ALIGN_LEFT, ALIGN_RIGHT, CHECKED, PARTIALLY_CHECKED, PURPOSE_HINTS, QT_HORIZONTAL, QT_VERTICAL,
    State, TEXT_EDIT_NO_WRAP, TEXT_EDIT_WRAP, UNCHECKED, Widget, purpose_hint, set_area_text, violation,
};

// `Text.elide` values (`Qt::TextElideMode`).
const ELIDE_RIGHT: i32 = 1;
const ELIDE_NONE: i32 = 3;

/// `Image.FillMode`.
const FILL_STRETCH: i32 = 0;
const FILL_PRESERVE_ASPECT_FIT: i32 = 1;

impl State {
    pub(super) fn set_prop(&mut self, id: NodeId, prop: &Prop, command: &Command) {
        // A dialog over the window it belongs to, as 2ksbox's are: set
        // before the window is first shown, which is when Qt applies them.
        if let Prop::Modal { owner, modality } = prop {
            let parent = owner.and_then(|o| match self.nodes.get(&o).map(|n| &n.widget) {
                Some(Widget::Window { root }) => Some(root.window),
                _ => None,
            });
            let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
            let Widget::Window { root } = &node.widget else { violation(command, "only windows are modal") };
            root.window.set_object("transientParent", parent);
            // Qt::Dialog (which includes Qt::Window).
            root.window.set_int("flags", root.window.int("flags") | 0x3);
            // Escape asks it to close.
            root.window.set_bool("mitsuamiModal", true);
            root.window.set_int(
                "modality",
                match modality {
                    Modality::Window => 1,
                    Modality::Application => 2,
                },
            );
            node.modal = Some((*owner, *modality));
            return;
        }
        let events = self.events.clone();
        let Some(node) = self.nodes.get_mut(&id) else { violation(command, "node does not exist") };
        match (prop, &mut node.widget) {
            (Prop::Title(t), Widget::Window { root }) => {
                root.window.set_str("title", t);
                // The page's title is what Kirigami shows in its toolbar.
                // Beside a sidebar, the sidebar's page has it, and the
                // content's is the item chosen's.
                match root.sidebar.get() {
                    Some((_, sidebar)) => sidebar.set_str("title", t),
                    None => {
                        if let Some(page) = root.window.child("mitsuamiPage") {
                            page.set_str("title", t);
                        }
                    }
                }
            }
            (Prop::Sections(new), Widget::Sidebar { page, sections }) => {
                page.set_str("mitsuamiSections", &sections_json(new));
                *sections = new.clone();
            }
            (Prop::SelectedIndex(index), Widget::Sidebar { page, .. }) => {
                page.set_int("mitsuamiSelected", index.map_or(-1, |i| i as i32));
            }
            // Its window takes the page out of its row, or puts it back.
            (Prop::SidebarShown(shown), Widget::Sidebar { page, .. }) => page.set_bool("mitsuamiShown", *shown),
            // Titles and pages come in either order: each shows again.
            (Prop::Title(title), Widget::Group { root, .. }) => root.set_str("mitsuamiTitle", title),
            (Prop::TabTitles(titles), Widget::Tabs { root, .. }) => {
                root.set_str_list("mitsuamiTitles", titles);
                root.invoke("mitsuamiShow");
            }
            (Prop::TabIcons(icons), Widget::Tabs { root, .. }) => root.set_str_list("mitsuamiIcons", icons),
            // Kirigami's navigation bar unless the app asks for Qt's tab bar.
            (Prop::TabsStyle(style), Widget::Tabs { root, .. }) => {
                root.set_bool("mitsuamiNavigation", *style != TabsStyle::TabBar);
                root.invoke("mitsuamiShow");
                node.tabs_style = Some(*style);
            }
            (Prop::SelectedIndex(index), Widget::Tabs { root, .. }) => {
                root.set_int("mitsuamiSelected", index.map_or(-1, |i| i as i32));
                root.invoke("mitsuamiShow");
            }
            (Prop::FullScreen(on), Widget::Window { root }) => root.set_full_screen(*on),
            (Prop::HeightFollowsContent(on), Widget::Window { root }) => root.set_height_locked(*on),
            (Prop::Maximized(on), Widget::Window { root }) => root.set_maximized(*on),
            (Prop::Resizable(on), Widget::Window { root }) => root.set_resizable(*on),
            // Qt keeps the user from making it smaller; a window smaller
            // already grows, as on the other platforms.
            (Prop::MinSize(min), Widget::Window { root }) => {
                root.min.set(Some(*min));
                root.apply_min();
                let size = root.requested.get().unwrap_or(root.size.get());
                if root.at_least_min(size) != size && !root.in_full_screen() {
                    root.resize_to(size);
                }
            }
            (Prop::Text(t), Widget::Label(l)) => l.set_str("text", t),
            // Qt elides the last line it shows.
            (Prop::MaxLines(lines), Widget::Label(l)) => {
                l.set_int("maximumLineCount", lines.map_or(i32::MAX, |n| n as i32));
                l.set_int("elide", if lines.is_some() { ELIDE_RIGHT } else { ELIDE_NONE });
            }
            (Prop::TextColor(color), Widget::Label(l) | Widget::Icon(l)) => {
                if let Color::Rgba(r, g, b, a) = *color {
                    l.set_int("mitsuamiRgba", u32::from_be_bytes([r, g, b, a]) as i32);
                }
                l.set_int("mitsuamiColor", qml::color(*color));
            }
            (Prop::FontWeight(weight), Widget::Label(l)) => l.set_int("mitsuamiWeight", qml::font_weight(*weight)),
            (Prop::Italic(italic), Widget::Label(l)) => l.set_bool("mitsuamiItalic", *italic),
            // Set, it's what shows: Qt mirrors it only under
            // `LayoutMirroring`, which nothing enables, and aligns by the
            // text's own direction only while it's unset.
            (Prop::TextAlign(align), Widget::Label(l)) => l.set_int(
                "horizontalAlignment",
                match align {
                    HorizontalAlign::Left => ALIGN_LEFT,
                    HorizontalAlign::Center => ALIGN_H_CENTER,
                    HorizontalAlign::Right => ALIGN_RIGHT,
                },
            ),
            (Prop::Label(t), Widget::Button(b) | Widget::MenuButton(b) | Widget::Checkbox(b)) => b.set_str("text", t),
            (
                Prop::Label(t),
                Widget::Switch(s)
                | Widget::Select(s)
                | Widget::RadioGroup(s)
                | Widget::Slider(s)
                | Widget::NumberInput(s)
                | Widget::Progress(s)
                | Widget::Spinner(s)
                | Widget::Icon(s)
                | Widget::Image { item: s, .. },
            ) => {
                s.set_str("mitsuamiA11yName", t);
                node.a11y_label = Some(t.clone());
            }
            (Prop::Label(t), Widget::GpuSurface(surface)) => {
                surface.item.set_str("mitsuamiA11yName", t);
                node.a11y_label = Some(t.clone());
            }
            (Prop::TakesInput(takes), Widget::GpuSurface(surface)) => surface.set_takes_input(*takes),
            (Prop::PointerLock(on), Widget::GpuSurface(surface)) => surface.set_pointer_lock(*on),
            (Prop::KeyboardGrab(on), Widget::GpuSurface(surface)) => surface.set_keyboard_grab(*on),
            (Prop::Cursor(cursor), Widget::GpuSurface(surface)) => surface.set_cursor(cursor),
            (Prop::Options(options), Widget::Select(s)) => {
                // A new model resets the chosen index; it stays if it can,
                // else the first option is chosen, as the core does. It
                // sends the index when that changes it.
                let chosen = s.int("currentIndex");
                s.set_str_list("mitsuamiOptions", options);
                let count = options.len() as i32;
                s.set_int(
                    "currentIndex",
                    if (0..count).contains(&chosen) {
                        chosen
                    } else if count > 0 {
                        0
                    } else {
                        -1
                    },
                );
            }
            (Prop::Range { min, max }, Widget::Slider(s)) => {
                s.set_real("from", *min);
                s.set_real("to", *max);
            }
            (Prop::Step(step), Widget::Slider(s)) => s.set_real("stepSize", step.unwrap_or(0.0)),
            (Prop::Number(n), Widget::Slider(s)) => s.set_real("value", *n),
            // Whole numbers: the core only sends those.
            (Prop::Range { min, max }, Widget::NumberInput(s)) => {
                s.set_int("from", *min as i32);
                s.set_int("to", *max as i32);
            }
            (Prop::Step(step), Widget::NumberInput(s)) => s.set_int("stepSize", step.map_or(1, |s| s as i32)),
            (Prop::Number(n), Widget::NumberInput(s)) => s.set_int("value", *n as i32),
            (Prop::WrapAround(on), Widget::NumberInput(s)) => s.set_bool("wrap", *on),
            (Prop::Orientation(o), Widget::Slider(s)) => {
                s.set_int("orientation", if o.vertical() { QT_VERTICAL } else { QT_HORIZONTAL });
                node.orientation = Some(*o);
            }
            // Kirigami's has no orientation: its frame says which way it
            // runs.
            (Prop::Orientation(o), Widget::Separator(_)) => node.orientation = Some(*o),
            // Stopped, a busy indicator fades out.
            (Prop::Running(r), Widget::Spinner(s)) => s.set_bool("running", *r),
            (Prop::Image(new), Widget::Image { item, source, pixels, .. }) => {
                match new {
                    ImageSource::File(path) => {
                        item.set_url("source", path);
                        *pixels = None;
                    }
                    ImageSource::Pixels(p) => {
                        // The new pixels first, then the url that shows them;
                        // the old ones go once nothing shows them.
                        let provided = ffi::ProvidedPixels::new(p);
                        item.set_url_str("source", &provided.url());
                        *pixels = Some(provided);
                    }
                }
                *source = Some(new.clone());
            }
            (Prop::ImageFit(new), Widget::Image { item, fit, .. }) => {
                item.set_int(
                    "fillMode",
                    match new {
                        ImageFit::Contain => FILL_PRESERVE_ASPECT_FIT,
                        ImageFit::Stretch => FILL_STRETCH,
                    },
                );
                *fit = Some(*new);
            }
            (Prop::Progress(progress), Widget::Progress(p)) => {
                p.set_bool("indeterminate", progress.is_none());
                if let Some(fraction) = progress {
                    p.set_real("value", *fraction);
                }
            }
            // New buttons, chosen as before if they can be, as the core
            // does: it only sends the index when that changes it.
            (Prop::Options(options), Widget::RadioGroup(g)) => {
                g.invoke("mitsuamiReadShown");
                let shown = g.int("mitsuamiShown");
                g.set_int("mitsuamiSelected", if shown < options.len() as i32 { shown } else { -1 });
                g.set_str_list("mitsuamiOptions", options);
            }
            (Prop::SelectedIndex(index), Widget::RadioGroup(g)) => {
                g.set_int("mitsuamiSelected", index.map_or(-1, |i| i as i32));
                // Set to what it was, it doesn't show it again.
                g.invoke("mitsuamiShow");
            }
            (Prop::SelectedIndex(index), Widget::Select(s)) => {
                s.set_int("currentIndex", index.map_or(-1, |i| i as i32))
            }
            (Prop::Value(t), Widget::Field(f)) => {
                // Don't disturb the caret when the field already shows it.
                if f.str("text") != *t {
                    // Kirigami's search for it isn't reported.
                    if node.kind == WidgetKind::SearchInput {
                        f.set_str("mitsuamiShown", t);
                    }
                    f.set_str("text", t);
                }
            }
            (Prop::Placeholder(t), Widget::Field(f)) => f.set_str("placeholderText", t),
            // Still focusable and selectable, so its text can be copied.
            (Prop::ReadOnly(r), Widget::Field(f)) => f.set_bool("readOnly", *r),
            // For the on-screen keyboard and input methods.
            (Prop::InputPurpose(purpose), Widget::Field(f)) => {
                let hints = f.int("inputMethodHints") & !PURPOSE_HINTS;
                f.set_int("inputMethodHints", hints | purpose_hint(*purpose));
            }
            (Prop::Value(t), Widget::TextArea { root, .. }) => {
                if root.str("text") != *t {
                    set_area_text(*root, t);
                }
            }
            (Prop::Placeholder(t), Widget::TextArea { root, .. }) => root.set_str("placeholderText", t),
            (Prop::ReadOnly(r), Widget::TextArea { root, .. }) => root.set_bool("readOnly", *r),
            (Prop::Lines(n), Widget::TextArea { root, .. }) => root.set_int("mitsuamiLines", *n as i32),
            // Unwrapped, long lines scroll sideways in the scroll view.
            (Prop::LineWrap(on), Widget::TextArea { area, .. }) => {
                area.set_int("wrapMode", if *on { TEXT_EDIT_WRAP } else { TEXT_EDIT_NO_WRAP })
            }
            (Prop::Checked(c), Widget::Checkbox(b)) => {
                node.checked = *c;
                // The mixed state shows over it.
                if b.int("checkState") != PARTIALLY_CHECKED {
                    b.set_bool("checked", *c);
                }
            }
            (Prop::Checked(c), Widget::Switch(b)) => b.set_bool("checked", *c),
            (Prop::Mixed(m), Widget::Checkbox(b)) => {
                node.mixed = Some(*m);
                if *m {
                    b.set_int("checkState", PARTIALLY_CHECKED);
                } else {
                    b.set_bool("tristate", false);
                    b.set_int("checkState", if node.checked { CHECKED } else { UNCHECKED });
                }
            }
            // Disabled, a text area shows no selection.
            (Prop::Enabled(e), Widget::TextArea { root, area }) => {
                root.set_bool("enabled", *e);
                if !e {
                    area.invoke("deselect");
                }
            }
            (Prop::Enabled(e), w) if w.is_control() => w.item().set_bool("enabled", *e),
            (Prop::TextStyle(style), w) if w.is_control() => {
                w.item().set_int("mitsuamiTextStyle", qml::text_style(*style));
                node.text_style = Some(*style);
            }
            (Prop::ButtonRole(role), Widget::Button(b)) => {
                // Breeze tints the default button. There is no cancel or
                // destructive style.
                b.set_bool("mitsuamiDefault", *role == ButtonRole::Default);
                node.role = Some(*role);
            }
            (Prop::Icon(name), Widget::Button(b) | Widget::MenuButton(b)) => b.set_str("mitsuamiIcon", name),
            (Prop::IconOnly(only), Widget::Button(b) | Widget::MenuButton(b)) => {
                b.set_bool("mitsuamiIconOnly", *only);
                node.icon_only = true;
            }
            (Prop::Icon(name), Widget::Icon(i)) => i.set_str("mitsuamiName", name),
            (Prop::IconSize(points), Widget::Icon(i)) => {
                i.set_real("mitsuamiSize", *points as f64);
                node.icon_size = true;
            }
            (Prop::FileDrop(drop), Widget::Host(_) | Widget::Group { .. }) => {
                node.file_drop_given = true;
                match (drop, &node.file_drop) {
                    (Some(drop), Some(area)) => area.set(drop.clone()),
                    (Some(drop), None) => {
                        let host = node.widget.item();
                        node.file_drop = Some(crate::file_drop::FileDropArea::new(host, id, events, drop.clone()));
                    }
                    (None, _) => {
                        if let Some(area) = node.file_drop.take() {
                            area.remove();
                        }
                    }
                }
            }
            (Prop::Menu(entries), Widget::MenuButton(b)) => {
                let b = *b;
                node.button_menu
                    .get_or_insert_with(|| {
                        ContextMenu::for_button(move |chosen| events.emit(id, UiEvent::MenuItem(chosen)))
                    })
                    .set(Some(b), entries);
            }
            (Prop::ButtonStyle(style), Widget::Button(b) | Widget::MenuButton(b)) => {
                // Flat buttons have no frame until hovered.
                b.set_bool("flat", *style == ButtonStyle::Borderless);
                node.button_style = Some(*style);
            }
            (Prop::Checked(c), Widget::Button(b)) => b.set_bool("checked", *c),
            (Prop::Tweak(tweak), _) => node.tweak = Some(tweak.clone()),
            (Prop::Tooltip(text), widget) => {
                if widget.has_tooltip() {
                    widget.item().set_str("mitsuamiTooltip", text);
                }
                node.tooltip = text.clone();
            }
            // Custom renders, drawn and native items and text fields keep
            // it on the node without showing it.
            (Prop::ContextMenu(entries), widget) => {
                let item = widget.shows_context_menu().then(|| widget.item());
                node.context_menu
                    .get_or_insert_with(|| {
                        ContextMenu::new(move |chosen| events.emit(id, UiEvent::ContextMenuItem(chosen)))
                    })
                    .set(item, entries);
            }
            (Prop::Rows(rows), Widget::List(list)) => list.set_rows(rows.clone()),
            (Prop::SelectionMode(mode), Widget::List(list)) => list.set_mode(*mode),
            (Prop::ListStyle(style), Widget::List(list)) => list.set_style(*style),
            (Prop::Selected(rows), Widget::List(list)) => list.set_selected(rows),
            (Prop::EstimatedRowHeight(height), Widget::List(list)) => list.set_estimate(*height),
            (Prop::Row(row), Widget::Host(_)) => node.row = Some(*row),
            (Prop::ScrollAxes(axes), Widget::Scroll { view, .. }) => {
                let bits = match axes {
                    ScrollAxes::Horizontal => 1,
                    ScrollAxes::Vertical => 2,
                    ScrollAxes::Both => 3,
                };
                view.set_int("mitsuamiAxes", bits);
                node.scroll_axes = Some(*axes);
            }
            (Prop::ScrollBars(show), Widget::Scroll { view, .. }) => view.set_bool("mitsuamiBars", *show),
            (Prop::Custom(new), Widget::Custom { item, render, props }) => {
                if props != new {
                    render.update(*item, props.props(), new.props());
                    *props = new.clone();
                }
            }
            (Prop::Custom(new), Widget::Drawn { props, .. }) => *props = new.clone(),
            (Prop::Drawing(new), Widget::Drawn { item, drawing, .. }) => {
                item.set_drawn_ops(&flatten(new));
                *drawing = new.clone();
            }
            (Prop::Native(opaque), Widget::Native { item, last, .. }) => {
                // The creating payload was applied on creation.
                if opaque != last
                    && let Some(payload) = opaque.downcast_ref::<NativePayload>()
                {
                    payload.apply(*item);
                }
                *last = opaque.clone();
            }
            _ => {}
        }
        // A window's sidebar leaves its row or comes back, and the content
        // keeps its size: the window shrinks or grows by the column.
        if let Prop::SidebarShown(_) = prop
            && let Some(Widget::Window { root }) = node.parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget)
        {
            root.window.invoke("mitsuamiApplySidebarShown");
            let size = root.size.get();
            if !size.is_empty() {
                root.resize_to(size);
            }
        }
    }
}
