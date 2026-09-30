//! Reading back what a node's widget shows.

use mitsuami_core::backend::NativeState;
use mitsuami_core::{
    HorizontalAlign, InputPurpose, Modality, NodeId, Orientation, Point, Prop, Rect, TabsStyle, WidgetKind,
};

use crate::events::{node_from_key, node_key};
use crate::qml;

use super::{
    ALIGN_H_CENTER, ALIGN_RIGHT, KirigamiBackend, PARTIALLY_CHECKED, QT_VERTICAL, TEXT_EDIT_NO_WRAP, Widget, frame_of,
    option_texts, purpose_hint, radio_options, scroll_offset, strip, tab_titles,
};

impl KirigamiBackend {
    pub(super) fn read_native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        let mut props = Vec::new();
        let item = node.widget.item();
        props.push(Prop::LayoutDirection(match node.direction {
            Some(kept) => kept,
            None if item.mirrored() => mitsuami_core::LayoutDirection::RightToLeft,
            None => mitsuami_core::LayoutDirection::LeftToRight,
        }));
        match &node.widget {
            Widget::Window { root } => {
                props.push(Prop::Title(root.window.str("title")));
                props.push(Prop::FullScreen(root.in_full_screen()));
                props.push(Prop::MinSize(root.min_size()));
                props.push(Prop::HeightFollowsContent(root.height_locked()));
                props.push(Prop::Maximized(root.is_maximized()));
                props.push(Prop::Resizable(!root.width_fixed()));
                // The modality as Qt has it; the owner as the node has it.
                if let Some((owner, modality)) = node.modal {
                    let modality = match root.window.int("modality") {
                        1 => Modality::Window,
                        2 => Modality::Application,
                        _ => modality,
                    };
                    props.push(Prop::Modal { owner, modality });
                }
            }
            Widget::Label(l) => {
                props.push(Prop::Text(l.str("text")));
                props.push(Prop::Selectable(l.bool("mitsuamiSelectable")));
                let lines = l.int("maximumLineCount");
                props.push(Prop::MaxLines((lines != i32::MAX).then_some(lines as u32)));
                props
                    .extend(qml::color_from(l.int("mitsuamiColor"), l.int("mitsuamiRgba") as u32).map(Prop::TextColor));
                props.push(Prop::FontWeight(qml::font_weight_from(l.int("mitsuamiShownWeight"))));
                props.push(Prop::Italic(l.bool("mitsuamiShownItalic")));
                let align = l.int("effectiveHorizontalAlignment");
                props.push(Prop::TextAlign(match align {
                    ALIGN_RIGHT => HorizontalAlign::Right,
                    ALIGN_H_CENTER => HorizontalAlign::Center,
                    _ => HorizontalAlign::Left,
                }));
            }
            Widget::Field(f) | Widget::TextArea { root: f, .. } => {
                props.push(Prop::Value(f.str("text")));
                props.push(Prop::Placeholder(f.str("placeholderText")));
                props.push(Prop::ReadOnly(f.bool("readOnly")));
                if let Widget::TextArea { root, area } = &node.widget {
                    props.push(Prop::Lines(root.int("mitsuamiLines") as u32));
                    props.push(Prop::LineWrap(area.int("wrapMode") != TEXT_EDIT_NO_WRAP));
                } else if node.kind == WidgetKind::TextInput {
                    let hints = f.int("inputMethodHints");
                    let shown = [InputPurpose::Phone, InputPurpose::Email, InputPurpose::Url]
                        .into_iter()
                        .find(|p| hints & purpose_hint(*p) != 0);
                    props.push(Prop::InputPurpose(shown.unwrap_or_default()));
                }
            }
            Widget::Button(b) | Widget::MenuButton(b) => {
                props.push(Prop::Label(b.str("text")));
                if node.kind == WidgetKind::ToggleButton {
                    props.push(Prop::Checked(b.bool("checked")));
                }
                props.extend(node.button_menu.as_ref().map(|menu| Prop::Menu(menu.shown())));
                props.push(Prop::Icon(b.str("mitsuamiShownIcon")));
                if node.icon_only {
                    // Only with an icon: without, it shows its text.
                    let icon = !b.str("mitsuamiShownIcon").is_empty();
                    props
                        .push(Prop::IconOnly(b.bool("mitsuamiShownIconOnly") || (!icon && b.bool("mitsuamiIconOnly"))));
                }
            }
            Widget::Checkbox(c) => {
                props.push(Prop::Label(c.str("text")));
                let mixed = c.int("checkState") == PARTIALLY_CHECKED;
                props.push(Prop::Checked(if mixed { node.checked } else { c.bool("checked") }));
                if node.mixed.is_some() {
                    props.push(Prop::Mixed(mixed));
                }
            }
            Widget::Switch(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Checked(s.bool("checked")));
            }
            Widget::Slider(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Range { min: s.real("from"), max: s.real("to") });
                let step = s.real("stepSize");
                props.push(Prop::Step((step > 0.0).then_some(step)));
                props.push(Prop::Number(s.real("value")));
                if node.orientation.is_some() {
                    props.push(Prop::Orientation(if s.int("orientation") == QT_VERTICAL {
                        Orientation::Vertical
                    } else {
                        Orientation::Horizontal
                    }));
                }
            }
            Widget::NumberInput(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Range { min: s.int("from").into(), max: s.int("to").into() });
                props.push(Prop::Step(Some(s.int("stepSize").into())));
                props.push(Prop::WrapAround(s.bool("wrap")));
                props.push(Prop::Number(s.int("value").into()));
            }
            Widget::Progress(p) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Progress((!p.bool("indeterminate")).then(|| p.real("value"))));
            }
            Widget::Spinner(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Running(s.bool("running")));
            }
            Widget::Separator(_) => props.extend(node.orientation.map(Prop::Orientation)),
            Widget::FileIcon(i) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let file = i.str("mitsuamiFile");
                if !file.is_empty() {
                    props.push(Prop::File(file.into()));
                }
                props.push(Prop::Thumbnail(i.bool("mitsuamiThumbnail")));
                if node.icon_size {
                    props.push(Prop::IconSize(i.real("implicitWidth") as f32));
                }
            }
            Widget::Icon(i) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Icon(i.str("mitsuamiShownName")));
                props
                    .extend(qml::color_from(i.int("mitsuamiColor"), i.int("mitsuamiRgba") as u32).map(Prop::TextColor));
                // Its size, which it takes no room at without a name.
                if node.icon_size {
                    let shown = if i.str("mitsuamiName").is_empty() { "mitsuamiSize" } else { "implicitWidth" };
                    props.push(Prop::IconSize(i.real(shown) as f32));
                }
            }
            Widget::Image { source, fit, .. } => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.extend(source.clone().map(Prop::Image));
                props.extend(fit.map(Prop::ImageFit));
            }
            Widget::GpuSurface(surface) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                let (takes, locked, grabbed, cursor) = surface.props();
                props.extend(takes.map(Prop::TakesInput));
                props.extend(locked.map(Prop::PointerLock));
                props.extend(grabbed.map(Prop::KeyboardGrab));
                props.extend(cursor.map(Prop::Cursor));
            }
            Widget::RadioGroup(g) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Options(radio_options(*g)));
                g.invoke("mitsuamiReadShown");
                props.push(Prop::SelectedIndex(usize::try_from(g.int("mitsuamiShown")).ok()));
            }
            Widget::Select(s) => {
                props.extend(node.a11y_label.clone().map(Prop::Label));
                props.push(Prop::Options(option_texts(*s)));
                props.push(Prop::SelectedIndex(usize::try_from(s.int("currentIndex")).ok()));
            }
            Widget::Scroll { view, .. } => {
                props.extend(node.scroll_axes.map(Prop::ScrollAxes));
                props.push(Prop::ScrollBars(view.bool("mitsuamiBars")));
            }
            Widget::Custom { item, render, props: last } => {
                props.push(Prop::Custom(last.with_props(render.read(*item, last.props()))))
            }
            Widget::Drawn { props: last, drawing, .. } => {
                props.push(Prop::Custom(last.clone()));
                props.push(Prop::Drawing(drawing.clone()));
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
            Widget::ToolbarItem { .. } => {}
            Widget::Sidebar { page, sections } => {
                props.push(Prop::Sections(sections.clone()));
                props.push(Prop::SelectedIndex(usize::try_from(page.int("mitsuamiSelected")).ok()));
                // As its window's row has it, or the app wants it before.
                let root = node.parent.and_then(|p| state.nodes.get(&p)).and_then(|p| match &p.widget {
                    Widget::Window { root } => Some(root.window),
                    _ => None,
                });
                props.push(Prop::SidebarShown(match root {
                    Some(window) => window.bool("mitsuamiSidebarIn"),
                    None => page.bool("mitsuamiShown"),
                }));
            }
            Widget::Group { group, .. } => props.push(Prop::Title(group.str("title"))),
            Widget::Tabs { root, .. } => {
                props.push(Prop::TabTitles(tab_titles(*root)));
                props.push(Prop::TabIcons(root.str("mitsuamiShownIcons").split('\u{1f}').map(str::to_owned).collect()));
                props.push(Prop::SelectedIndex(usize::try_from(strip(*root).int("currentIndex")).ok()));
                // Which strip it shows; `Automatic` is the navigation bar.
                let navigation = root.bool("mitsuamiNavigation");
                props.extend(
                    node.tabs_style
                        .map(|chosen| match (chosen, navigation) {
                            (TabsStyle::TabBar, false) | (TabsStyle::Automatic | TabsStyle::Navigation, true) => chosen,
                            (_, true) => TabsStyle::Navigation,
                            (_, false) => TabsStyle::TabBar,
                        })
                        .map(Prop::TabsStyle),
                );
            }
        }
        if node.widget.is_control() {
            props.push(Prop::Enabled(item.bool("enabled")));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        if node.file_drop_given {
            props.push(Prop::FileDrop(node.file_drop.as_ref().map(|area| area.drop_value())));
        }
        props.extend(node.keys.as_ref().map(|keys| Prop::Keys(keys.keys())));
        props.push(Prop::Tooltip(if node.widget.has_tooltip() {
            node.widget.item().str("mitsuamiTooltip")
        } else {
            node.tooltip.clone()
        }));
        props.extend(node.context_menu.as_ref().map(|menu| Prop::ContextMenu(menu.shown())));
        let frame = match &node.widget {
            Widget::Window { root } => {
                let size = root.size.get();
                Rect::new(0.0, 0.0, size.width, size.height)
            }
            _ => frame_of(item),
        };
        // A row is where the list view put it (a cell where the table view
        // did), a toolbar item where the toolbar did: above the content,
        // in its coordinates.
        let frame = match (node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget), &node.widget) {
            (Some(Widget::List(list)), _) => list.row_rect(item, frame),
            (Some(Widget::Window { root }), Widget::ToolbarItem { host, action }) => {
                if !action.bool("visible") || host.object("parent").is_none() {
                    Rect::ZERO
                } else {
                    let (at, origin) = (host.map_to_scene(Point::ZERO), root.host.map_to_scene(Point::ZERO));
                    Rect::new(at.x - origin.x, at.y - origin.y, frame.width(), frame.height())
                }
            }
            // A sidebar's page is beside the content (at negative x), where
            // the page row shows it.
            (Some(Widget::Window { root }), Widget::Sidebar { page, .. }) => {
                if !root.window.bool("mitsuamiSidebarIn") || !page.bool("visible") || page.real("width") <= 0.0 {
                    Rect::ZERO
                } else {
                    let (at, origin) = (page.map_to_scene(Point::ZERO), root.host.map_to_scene(Point::ZERO));
                    Rect::new(at.x - origin.x, at.y - origin.y, frame.width(), frame.height())
                }
            }
            // A tab view's page is in its page area, below the bar, while
            // it's the one shown.
            (Some(Widget::Tabs { root, .. }), _) => {
                if !item.bool("visible") {
                    Rect::ZERO
                } else {
                    let (at, origin) = (item.map_to_scene(Point::ZERO), root.map_to_scene(Point::ZERO));
                    Rect::new(at.x - origin.x, at.y - origin.y, frame.width(), frame.height())
                }
            }
            _ => frame,
        };
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => (Vec::new(), Some(list.scroll_offset())),
            Widget::Scroll { flickable, .. } => (node.widget.content().child_items(), Some(scroll_offset(*flickable))),
            Widget::Window { .. } | Widget::Host(_) | Widget::ToolbarItem { .. } => (item.child_items(), None),
            Widget::Tabs { pages, .. } => (pages.child_items(), None),
            Widget::Group { content, .. } => (content.child_items(), None),
            _ => (Vec::new(), None),
        };
        // Items that stand for nodes themselves: `node()` walks up the tree.
        let own = node_key(id);
        let mut children: Vec<NodeId> = match &node.widget {
            Widget::List(list) => list.children(),
            _ => children.iter().filter_map(|c| c.node()).filter(|key| *key != own).map(node_from_key).collect(),
        };
        // A window's toolbar items come after its content, and its sidebar
        // after them.
        if let Widget::Window { root } = &node.widget {
            children.extend(root.toolbar.borrow().iter().copied());
            children.extend(root.sidebar.get().map(|(id, _)| id));
        }
        let window = {
            let mut top = id;
            while let Some(parent) = state.nodes.get(&top).and_then(|n| n.parent) {
                top = parent;
            }
            match state.nodes.get(&top).map(|n| &n.widget) {
                Some(Widget::Window { root }) => Some(root.window),
                _ => None,
            }
        };
        let focused = !matches!(node.widget, Widget::Window { .. })
            && window.and_then(|w| w.focus_item()).and_then(|f| f.node()) == Some(node_key(id));
        let selection = super::selection::selection(&node.widget).filter(|_| focused);
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
