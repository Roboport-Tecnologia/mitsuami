//! Applying commands: create, insert, remove, destroy, frames, focus.

use gtk::prelude::*;
use mitsuami_core::a11y::A11yProps;
use mitsuami_core::{Command, NodeId, Size, WidgetKind};

use crate::host::WindowRoot;
use crate::sidebar::Split;

use super::scroll::{scroll_to, sync_scroll};
use super::slider::set_travel;
use super::window::{keep_content_size, pack_items, requested, resize};
use super::{State, Widget, WindowParts, violation};

impl State {
    /// Leaves with an empty frame (hidden ones, or not laid out yet) are
    /// kept out of GTK's allocation: controls can't be allocated smaller
    /// than their padding. Containers stay, since content may overflow them,
    /// except tab views, which can't be smaller than their tabs either, and
    /// lists and tables, which scroll their rows and can't be smaller than
    /// their scroll bars and header.
    fn update_child_visible(&self, id: NodeId) {
        let Some(node) = self.nodes.get(&id) else { return };
        if node.widget.is_leaf() || matches!(node.widget, Widget::Tabs(_) | Widget::List(_)) {
            let widget = node.widget.widget();
            let empty = self.frames.borrow().get(widget).is_none_or(|f| f.size.is_empty());
            widget.set_child_visible(!empty);
        }
    }

    fn widget(&self, id: NodeId, command: &Command) -> gtk::Widget {
        match self.nodes.get(&id) {
            Some(node) => node.widget.widget().clone(),
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    fn window_root(&self, id: NodeId, command: &Command) -> (&WindowParts, &WindowRoot) {
        match self.nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => (parts, parts.host.window_root().expect("window hosts have a root")),
            _ => violation(command, "not a window"),
        }
    }

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) {
        let node = &self.nodes[&id];
        if let Some(run) = node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            match &node.widget {
                // The list or text view, not the scrolled window around it.
                Widget::List(list) => run(&list.view),
                Widget::TextArea { view, .. } => run(view.upcast_ref()),
                // The card, not the host the children are in.
                Widget::Group(group) => run(group.card.upcast_ref()),
                widget => run(widget.widget()),
            }
        }
    }

    pub(super) fn apply(&mut self, command: &Command) {
        match command {
            Command::Create { id, kind, props } => {
                if self.nodes.contains_key(id) {
                    violation(command, "node already exists");
                }
                self.create(*id, *kind, command);
                for prop in props {
                    self.set_prop(*id, prop, command);
                }
                self.run_tweak(*id);
            }
            Command::SetProp { id, prop } => {
                self.set_prop(*id, prop, command);
                self.run_tweak(*id);
            }
            Command::Insert { parent, child, index } => {
                let child_widget = self.widget(*child, command);
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if self.nodes[child].kind == WidgetKind::Sidebar {
                    let sidebar = match self.nodes.remove(child) {
                        Some(node) => node,
                        None => violation(command, "node does not exist"),
                    };
                    let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget) else {
                        violation(command, "a sidebar goes in a window")
                    };
                    if parts.split.is_some() {
                        violation(command, "a window has one sidebar");
                    }
                    let Widget::Sidebar(list) = &sidebar.widget else { unreachable!() };
                    parts.split = Some(Split::new(&parts.window, &parts.header, &parts.host, *child, list));
                    // The content keeps its size: the window grows by the
                    // sidebar.
                    let size = parts.host.window_root().expect("window hosts have a root").size.get();
                    let (width, height) = parts.extra(size);
                    resize(&parts.window, size.width as i32 + width, size.height as i32 + height);
                    self.nodes.insert(*child, sidebar);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if self.nodes[child].kind == WidgetKind::ToolbarItem {
                    // Items come after the window's content, and before its sidebar.
                    let content = self
                        .nodes
                        .values()
                        .filter(|n| {
                            n.parent == Some(*parent)
                                && !matches!(n.kind, WidgetKind::ToolbarItem | WidgetKind::Sidebar)
                        })
                        .count();
                    let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget) else {
                        violation(command, "toolbar items go in windows")
                    };
                    let Some(index) = index.checked_sub(content) else {
                        violation(command, "toolbar items go after the window's content")
                    };
                    // Hidden until it has a size. The header bar stretches
                    // its children to its height; centred, the host keeps
                    // its own, as a label's text is centred in the bar.
                    child_widget.set_visible(false);
                    child_widget.set_valign(gtk::Align::Center);
                    parts.items.insert(index.min(parts.items.len()), (*child, child_widget));
                    pack_items(parts);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                match &self.nodes.get(parent).map(|n| &n.widget) {
                    Some(Widget::Scroll { viewport, .. }) => {
                        if viewport.child().is_some() {
                            violation(command, "a ScrollView has a single native child (its content)");
                        }
                        viewport.set_child(Some(&child_widget));
                    }
                    Some(Widget::List(list)) => {
                        let Some(row) = self.nodes[child].row else {
                            violation(command, "a List's children are row hosts, a Table's cell hosts")
                        };
                        list.insert(row, self.nodes[child].column.unwrap_or(0), *child, child_widget);
                    }
                    Some(Widget::Tabs(tabs)) => {
                        if self.nodes[child].kind != WidgetKind::Container {
                            violation(command, "a Tabs' children are page hosts (Containers)");
                        }
                        tabs.insert(*index, &child_widget);
                    }
                    parent_kind => {
                        // After a group's heading and card.
                        let own = if matches!(parent_kind, Some(Widget::Group(_))) {
                            crate::group::Group::OWN_CHILDREN
                        } else {
                            0
                        };
                        let parent_widget = self.widget(*parent, command);
                        let mut before = parent_widget.first_child();
                        for _ in 0..*index + own {
                            before = before.and_then(|w| w.next_sibling());
                        }
                        child_widget.insert_before(&parent_widget, before.as_ref());
                    }
                }
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                self.update_child_visible(*child);
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                if let Widget::Window(parts) = &mut self.nodes.get_mut(parent).unwrap().widget
                    && parts.split.as_ref().is_some_and(|s| s.sidebar == *child)
                {
                    // The content keeps its size: the window loses the
                    // sidebar's.
                    let size = parts.host.window_root().expect("window hosts have a root").size.get();
                    parts.split.take().unwrap().remove(&parts.window, &parts.header, &parts.host);
                    resize(&parts.window, size.width as i32, size.height as i32 + parts.header_height);
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return;
                }
                if let Widget::Window(parts) = &mut self.nodes.get_mut(parent).unwrap().widget
                    && let Some(at) = parts.items.iter().position(|(id, _)| id == child)
                {
                    let (_, widget) = parts.items.remove(at);
                    parts.header.remove(&widget);
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return;
                }
                match &self.nodes[parent].widget {
                    Widget::Scroll { viewport, .. } => viewport.set_child(None::<&gtk::Widget>),
                    Widget::List(list) => list.remove(
                        self.nodes[child].row.expect("inserted with a row"),
                        self.nodes[child].column.unwrap_or(0),
                    ),
                    Widget::Tabs(tabs) => tabs.remove(&self.widget(*child, command)),
                    _ => self.widget(*child, command).unparent(),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                if let Some(menu) = &node.context_menu {
                    menu.close();
                }
                let widget = node.widget.widget().clone();
                self.by_widget.borrow_mut().remove(&widget);
                self.frames.borrow_mut().remove(&widget);
                self.pending_show.retain(|w| w != id);
                for (object, handler) in node.settings_handlers {
                    object.disconnect(handler);
                }
                if let Some(Widget::Window(parts)) =
                    node.parent.and_then(|p| self.nodes.get_mut(&p)).map(|n| &mut n.widget)
                    && let Some(at) = parts.items.iter().position(|(item, _)| item == id)
                {
                    parts.items.remove(at);
                    parts.header.remove(&widget);
                }
                // A sidebar destroyed with its window: out of the split
                // view, which owns its list.
                if let Some(Widget::Window(parts)) =
                    node.parent.and_then(|p| self.nodes.get_mut(&p)).map(|n| &mut n.widget)
                    && parts.split.as_ref().is_some_and(|s| s.sidebar == *id)
                {
                    parts.split.take().unwrap().remove(&parts.window, &parts.header, &parts.host);
                }
                // A page destroyed without being removed: out of its tab view.
                if let Some(Widget::Tabs(tabs)) = node.parent.and_then(|p| self.nodes.get(&p)).map(|n| &n.widget) {
                    tabs.remove(&widget);
                }
                // The app's handle may keep its surface: it just stops showing.
                if let Widget::GpuSurface(surface) = &node.widget {
                    surface.detach();
                }
                match &node.widget {
                    Widget::Window(parts) => {
                        self.menus.forget(*id);
                        parts.window.destroy();
                    }
                    _ => {
                        if let Some(viewport) = widget.parent().and_then(|p| p.downcast::<gtk::Viewport>().ok()) {
                            viewport.set_child(None::<&gtk::Widget>);
                        } else if widget.parent().is_some() {
                            widget.unparent();
                        }
                    }
                }
            }
            Command::SetFrame { id, frame } => {
                let widget = self.widget(*id, command);
                self.frames.borrow_mut().insert(widget.clone(), *frame);
                // A toolbar item: the header bar places it, at this size.
                if self.nodes[id].kind == WidgetKind::ToolbarItem {
                    widget.set_visible(!frame.size.is_empty());
                    widget.queue_resize();
                    if let Some(window) = self.nodes[id].parent
                        && let Some(Widget::Window(parts)) = self.nodes.get_mut(&window).map(|n| &mut n.widget)
                    {
                        keep_content_size(parts);
                    }
                    return;
                }
                // A page: the tab view places it.
                if let Some(Widget::Tabs(tabs)) =
                    self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget)
                {
                    tabs.place(&widget);
                }
                self.update_child_visible(*id);
                if let Widget::Group(group) = &self.nodes[id].widget {
                    group.place();
                }
                if let Widget::Slider { scale, steps } = &self.nodes[id].widget {
                    let vertical = scale.orientation() == gtk::Orientation::Vertical;
                    set_travel(scale, steps, if vertical { frame.height() } else { frame.width() });
                }
                // Hosts ask for their frame size, so their parents must
                // measure again, not just reallocate.
                widget.queue_resize();
                if let Some(Widget::List(list)) =
                    self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget)
                    && let Some(row) = self.nodes[id].row
                {
                    list.row_measured(row, frame.height());
                }
                if let Some((scrolled, viewport)) = self.scroll_of(&widget) {
                    sync_scroll(&self.frames, &scrolled, &viewport);
                }
            }
            Command::SetA11y { id, a11y } => {
                let widget = self.widget(*id, command);
                let A11yProps { label, description, hidden, .. } = a11y;
                use gtk::accessible::{Property, State as A11yState};
                match label {
                    Some(label) => widget.update_property(&[Property::Label(label)]),
                    None => widget.reset_property(gtk::AccessibleProperty::Label),
                }
                match description {
                    Some(description) => widget.update_property(&[Property::Description(description)]),
                    None => widget.reset_property(gtk::AccessibleProperty::Description),
                }
                widget.update_state(&[A11yState::Hidden(*hidden)]);
            }
            // A window in full screen keeps the screen's size (its default
            // size would apply when it leaves); none goes below its minimum.
            Command::SetWindowSize { id, size } => {
                let (parts, root) = self.window_root(*id, command);
                if parts.full_screen.in_effect(&parts.window) {
                    return;
                }
                let (min_width, min_height) = parts.host.size_request();
                let asked = *size;
                let size = Size::new(size.width.max(requested(min_width)), size.height.max(requested(min_height)));
                // Before it's first allocated, it's the size the content
                // has, as the core asked; after, the allocation reports it
                // (the app resizing it, `Ui::set_window_size`, hears only
                // from that), and so does one the minimum grows.
                if !parts.window.is_mapped() {
                    root.size.set(asked);
                }
                root.resizing.set((root.size.get() != size).then_some(size));
                let (width, height) = parts.extra(size);
                resize(&parts.window, size.width as i32 + width, size.height as i32 + height);
            }
            Command::SetFocusOrder { window, order } => {
                let widgets: Vec<gtk::Widget> = order
                    .iter()
                    .map(|id| match self.nodes.get(id) {
                        Some(node) => node.widget.focus_widget(),
                        None => violation(command, &format!("node {id} does not exist")),
                    })
                    .collect();
                let (_, root) = self.window_root(*window, command);
                *root.focus_order.borrow_mut() = widgets;
            }
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll { scrolled, .. }) => scroll_to(scrolled, *offset),
                Some(Widget::List(list)) => scroll_to(&list.scrolled, *offset),
                _ => violation(command, "not a ScrollView or List"),
            },
            Command::Focus { id } => match self.nodes.get(id) {
                Some(node) => {
                    node.widget.focus_widget().grab_focus();
                }
                None => violation(command, "node does not exist"),
            },
            // After the field's `Focus`, so this selection replaces the
            // one focusing makes (`gtk-entry-select-on-focus`).
            Command::SelectText { id, range } => match self.nodes.get(id) {
                Some(node) => {
                    if !super::selection::select(&node.widget, range.clone()) {
                        violation(command, "not a text field or text area");
                    }
                }
                None => violation(command, "node does not exist"),
            },
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row),
                _ => violation(command, "not a List"),
            },
        }
    }
}
