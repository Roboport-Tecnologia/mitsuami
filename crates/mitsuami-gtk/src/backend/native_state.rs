//! Reading back what native widgets show.

use gtk::prelude::*;
use gtk::{glib, pango};
use mitsuami_core::backend::NativeState;
use mitsuami_core::{HorizontalAlign, ImageFit, InputPurpose, NodeId, Orientation, Prop, Rect, WidgetKind};

use crate::file_drop::FileDropTarget;

use super::scroll::{scroll_axes, scroll_bars, scroll_offset};
use super::text::{buffer_text, find_attr, font_weight, icon_color, label_color};
use super::{GtkBackend, Widget, WindowParts, owning_node};

/// A select's options, as it shows them.
pub(super) fn option_texts(options: &gtk::StringList) -> Vec<String> {
    (0..options.n_items()).filter_map(|i| options.string(i)).map(|s| s.to_string()).collect()
}

impl GtkBackend {
    pub(super) fn read_native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        let mut props = Vec::new();
        let text = |s: Option<glib::GString>| s.map(|s| s.to_string()).unwrap_or_default();
        match &node.widget {
            Widget::Window(parts) => {
                props.push(Prop::Title(text(parts.window.title())));
                props.push(Prop::FullScreen(parts.full_screen.shown(&parts.window)));
                props.push(Prop::MinSize(parts.min_size.shown(&parts.window, &parts.host)));
                // Either keeps the window from resizing: each as the app
                // gave it while the other does.
                let resizable = parts.window.is_resizable();
                props.push(Prop::HeightFollowsContent(if parts.resizable { !resizable } else { parts.height_locked }));
                props.push(Prop::Resizable(if parts.height_locked { parts.resizable } else { resizable }));
                props.push(Prop::Maximized(parts.maximized.shown(&parts.window)));
                props.extend(node.modal.map(|(owner, modality)| Prop::Modal { owner, modality }));
            }
            Widget::Label(l) => {
                props.push(Prop::Text(l.text().to_string()));
                props.push(Prop::Selectable(l.is_selectable()));
                let limited = l.ellipsize() != pango::EllipsizeMode::None && l.lines() > 0;
                props.push(Prop::MaxLines(limited.then(|| l.lines() as u32)));
                props.extend(label_color(l, node.text_color).map(Prop::TextColor));
                let int =
                    |type_| find_attr(l, type_).and_then(|a| a.downcast_ref::<pango::AttrInt>().map(|a| a.value()));
                props.extend(int(pango::AttrType::Weight).map(|w| Prop::FontWeight(font_weight(w))));
                // `PANGO_STYLE_NORMAL` is 0; oblique shows as italics too.
                props.extend(int(pango::AttrType::Style).map(|s| Prop::Italic(s != 0)));
                // Where the text shows: GTK mirrors `xalign` in right-to-left
                // widgets (labels the app hasn't aligned, in such a locale).
                let x = if l.direction() == gtk::TextDirection::Rtl { 1.0 - l.xalign() } else { l.xalign() };
                props.push(Prop::TextAlign(match x {
                    x if x < 0.25 => HorizontalAlign::Left,
                    x if x > 0.75 => HorizontalAlign::Right,
                    _ => HorizontalAlign::Center,
                }));
            }
            Widget::Entry(e) => {
                props.push(Prop::Value(e.text().to_string()));
                if let Some(p) = e.placeholder_text() {
                    props.push(Prop::Placeholder(p.to_string()));
                }
                props.push(Prop::ReadOnly(!e.is_editable()));
                props.push(Prop::InputPurpose(match e.input_purpose() {
                    gtk::InputPurpose::Email => InputPurpose::Email,
                    gtk::InputPurpose::Url => InputPurpose::Url,
                    gtk::InputPurpose::Phone => InputPurpose::Phone,
                    _ => InputPurpose::Text,
                }));
            }
            Widget::Password(e) => {
                props.push(Prop::Value(e.text().to_string()));
                if let Some(p) = e.placeholder_text() {
                    props.push(Prop::Placeholder(p.to_string()));
                }
            }
            Widget::Search(e) => {
                props.push(Prop::Value(e.text().to_string()));
                if let Some(p) = e.placeholder_text() {
                    props.push(Prop::Placeholder(p.to_string()));
                }
            }
            Widget::TextArea { view, placeholder, lines, .. } => {
                props.push(Prop::Value(buffer_text(&view.buffer())));
                props.extend(placeholder.clone().map(Prop::Placeholder));
                props.push(Prop::ReadOnly(!view.is_editable()));
                props.push(Prop::Lines(*lines));
                props.push(Prop::LineWrap(view.wrap_mode() != gtk::WrapMode::None));
            }
            Widget::Button(b) => {
                props.extend(node.button.read(b));
                props.extend(b.downcast_ref::<gtk::ToggleButton>().map(|t| Prop::Checked(t.is_active())));
            }
            Widget::MenuButton { button, menu } => {
                props.extend(node.button.read(button));
                props.push(Prop::Menu(menu.entries(button.upcast_ref())));
            }
            Widget::Checkbox(c) => {
                props.push(Prop::Label(text(c.label())));
                props.push(Prop::Checked(c.is_active()));
                if node.mixed.is_some() {
                    props.push(Prop::Mixed(c.is_inconsistent()));
                }
            }
            Widget::Switch(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Checked(s.is_active()));
            }
            Widget::Select { dropdown, options } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Options(option_texts(options)));
                let index = dropdown.selected();
                props.push(Prop::SelectedIndex((index != gtk::INVALID_LIST_POSITION).then_some(index as usize)));
            }
            Widget::RadioGroup(group) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Options(group.options()));
                props.push(Prop::SelectedIndex(group.selected()));
            }
            Widget::Slider { scale, steps } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let adjustment = scale.adjustment();
                props.push(Prop::Range { min: adjustment.lower(), max: adjustment.upper() });
                props.push(Prop::Step(steps.step.get()));
                props.push(Prop::Number(adjustment.value()));
                if node.orientation.is_some() {
                    props.push(Prop::Orientation(match scale.orientation() {
                        gtk::Orientation::Vertical => Orientation::Vertical,
                        _ => Orientation::Horizontal,
                    }));
                }
            }
            Widget::SpinButton(spin) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let adjustment = spin.adjustment();
                props.push(Prop::Range { min: adjustment.lower(), max: adjustment.upper() });
                props.push(Prop::Step(Some(adjustment.step_increment())));
                props.push(Prop::Number(spin.value()));
                props.push(Prop::WrapAround(spin.wraps()));
            }
            Widget::Progress { bar, pulsing } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Progress((!pulsing.get()).then(|| bar.fraction())));
            }
            Widget::Spinner(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Running(s.is_spinning()));
            }
            Widget::Separator(line) => props.push(Prop::Orientation(match line.orientation() {
                gtk::Orientation::Vertical => Orientation::Vertical,
                _ => Orientation::Horizontal,
            })),
            Widget::Picture { picture, source, fit } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(source.clone().map(Prop::Image));
                // Read back, but only if the app chose one.
                if fit.is_some() {
                    props.push(Prop::ImageFit(match picture.content_fit() {
                        gtk::ContentFit::Fill => ImageFit::Stretch,
                        _ => ImageFit::Contain,
                    }));
                }
            }
            Widget::Icon { image, size } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(icon_color(image, node.text_color).map(Prop::TextColor));
                props.push(Prop::Icon(image.icon_name().map(|n| n.to_string()).unwrap_or_default()));
                // What GTK shows, as the size given when it rounds to it.
                if let Some(size) = size {
                    let shown = image.pixel_size() as f32;
                    props.push(Prop::IconSize(if shown == size.round() { *size } else { shown }));
                }
            }
            Widget::FileIcon { image, file, thumbnail, size } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(file.clone().map(Prop::File));
                props.extend(thumbnail.map(Prop::Thumbnail));
                if let Some(size) = size {
                    let shown = image.pixel_size() as f32;
                    props.push(Prop::IconSize(if shown == size.round() { *size } else { shown }));
                }
            }
            Widget::GpuSurface(surface) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(surface.props());
            }
            Widget::Scroll { scrolled, .. } => {
                props.push(Prop::ScrollAxes(scroll_axes(scrolled)));
                props.push(Prop::ScrollBars(scroll_bars(scrolled)));
            }
            Widget::Custom { widget, render, props: last } => {
                props.push(Prop::Custom(last.with_props(render.read(widget, last.props()))))
            }
            Widget::Drawn { drawn, props: last } => {
                props.push(Prop::Custom(last.clone()));
                props.push(Prop::Drawing(drawn.drawing()));
            }
            Widget::Native { last, .. } => props.push(Prop::Native(last.clone())),
            Widget::List(list) => {
                if node.kind == WidgetKind::Table {
                    props.push(Prop::Columns(list.columns()));
                    props.push(Prop::Sort(list.sort()));
                }
                props.push(Prop::Rows(list.rows()));
                props.extend(list.estimate().map(Prop::EstimatedRowHeight));
                props.push(Prop::SelectionMode(list.mode()));
                props.extend(list.style().map(Prop::ListStyle));
                props.push(Prop::Selected(list.selected()));
                props.extend(list.row_files().map(Prop::RowFiles));
            }
            Widget::Host(_) => props.extend(node.row.map(|row| match node.column {
                Some(column) => Prop::Cell(mitsuami_core::CellKey { row, column }),
                None => Prop::Row(row),
            })),
            Widget::Sidebar(sidebar) => {
                props.push(Prop::Sections(sidebar.sections()));
                props.push(Prop::SelectedIndex(sidebar.selected()));
                props.extend(sidebar.shown().map(Prop::SidebarShown));
            }
            Widget::Group(group) => props.push(Prop::Title(group.title())),
            Widget::Tabs(tabs) => {
                props.push(Prop::TabTitles(tabs.titles()));
                props.push(Prop::TabIcons(tabs.icons()));
                props.push(Prop::SelectedIndex(tabs.selected()));
                props.extend(tabs.style().map(Prop::TabsStyle));
            }
        }
        if let Some(target) = &node.file_drop {
            props.push(Prop::FileDrop(target.as_ref().map(FileDropTarget::file_drop)));
        }
        props.extend(node.keys.as_ref().map(|c| Prop::Keys(crate::keys::keys(c))));
        let widget = node.widget.widget();
        if node.widget.is_control() {
            props.push(Prop::Enabled(widget.is_sensitive()));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        props.push(Prop::Tooltip(text(node.widget.focus_widget().tooltip_text())));
        props.extend(node.context_menu.as_ref().map(|m| Prop::ContextMenu(m.entries(&node.widget.focus_widget()))));
        let frame = match &node.widget {
            Widget::Window(parts) => {
                let size = parts.host.window_root().expect("window hosts have a root").size.get();
                Rect::new(0.0, 0.0, size.width, size.height)
            }
            _ => state.frames.borrow().get(widget).copied().unwrap_or_default(),
        };
        // A row is where the list view put it, and a cell where the column
        // view did.
        let frame = match (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget)) {
            (Some(row), Some(Widget::List(list))) => match node.column {
                Some(column) => list.cell_rect(row, column, &state.frames),
                None => list.row_rect(row, &state.frames),
            }
            .unwrap_or(frame),
            // A toolbar item is where the header bar put it, in the content
            // host's coordinates (above it); a hidden one isn't shown.
            (_, Some(Widget::Window(parts))) if node.kind == WidgetKind::ToolbarItem => {
                match widget.compute_point(&parts.host, &gtk::graphene::Point::new(0.0, 0.0)) {
                    Some(origin) if widget.is_visible() => {
                        Rect::new(origin.x(), origin.y(), frame.width(), frame.height())
                    }
                    _ => Rect::ZERO,
                }
            }
            // A sidebar is its page, beside the content (at negative x); in
            // a collapsed split view, one of the two isn't shown.
            (_, Some(Widget::Window(WindowParts { host, split: Some(split), .. })))
                if node.kind == WidgetKind::Sidebar =>
            {
                let page = split.sidebar_page();
                match page.compute_bounds(host) {
                    Some(b) if page.is_mapped() && host.is_mapped() => Rect::new(b.x(), b.y(), b.width(), b.height()),
                    _ => Rect::ZERO,
                }
            }
            // A page is where the tab view put it; the others aren't shown.
            (_, Some(Widget::Tabs(tabs))) => tabs.page_frame(widget, frame.size),
            _ => frame,
        };
        let by_widget = state.by_widget.borrow();
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => (list.children(), Some(scroll_offset(&list.scrolled))),
            Widget::Scroll { scrolled, viewport } => (
                viewport.child().and_then(|c| by_widget.get(&c).copied()).into_iter().collect(),
                Some(scroll_offset(scrolled)),
            ),
            Widget::Tabs(tabs) => (tabs.pages().iter().filter_map(|p| by_widget.get(p).copied()).collect(), None),
            _ => {
                let mut children = Vec::new();
                let mut next = widget.first_child();
                while let Some(child) = next {
                    children.extend(by_widget.get(&child).copied());
                    next = child.next_sibling();
                }
                // Then its toolbar items, in the header bar, and its sidebar.
                if let Widget::Window(parts) = &node.widget {
                    children.extend(parts.items.iter().map(|(id, _)| *id));
                    children.extend(parts.split.as_ref().map(|s| s.sidebar));
                }
                (children, None)
            }
        };
        drop(by_widget);
        let focus = widget.root().and_then(|r| r.focus());
        let focused = !matches!(node.widget, Widget::Window(_)) && owning_node(&state.by_widget, focus) == Some(id);
        let selection = super::selection::selection(&node.widget, focused);
        // A list's focus is on its view, or on one of its rows' item
        // widgets; a control in a row owns its own.
        Some(NativeState {
            kind: node.kind,
            props,
            frame,
            parent: node.parent,
            children,
            focused,
            scroll_offset,
            selection,
        })
    }
}
