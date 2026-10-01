//! Applying the core's commands.

use std::rc::Rc;

use mitsuami_core::a11y::A11yProps;
use mitsuami_core::{Command, NodeId, Point, WidgetKind};

use crate::ffi::QmlObject;

use super::window::hide_sidebar;
use super::{State, Widget, WindowRoot, sync_scroll, violation};

impl State {
    fn widget(&self, id: NodeId, command: &Command) -> &Widget {
        match self.nodes.get(&id) {
            Some(node) => &node.widget,
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    fn window_root(&self, id: NodeId, command: &Command) -> Rc<WindowRoot> {
        match self.nodes.get(&id).map(|n| &n.widget) {
            Some(Widget::Window { root }) => root.clone(),
            _ => violation(command, "not a window"),
        }
    }

    /// The flickable of the scroll view a node is the content of.
    fn scroll_parent(&self, id: NodeId) -> Option<QmlObject> {
        match &self.nodes.get(&self.nodes.get(&id)?.parent?)?.widget {
            Widget::Scroll { flickable, .. } => Some(*flickable),
            _ => None,
        }
    }

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) {
        let node = &self.nodes[&id];
        if let Some(run) = node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            match &node.widget {
                // The list or table view or text area, not the scroll view
                // around it.
                Widget::List(list) => run(list.view),
                Widget::TextArea { area, .. } => run(*area),
                Widget::Group { group, .. } => run(*group),
                widget => run(widget.item()),
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
                let item = self.widget(*child, command).item();
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if let Widget::Sidebar { page, .. } = self.nodes[child].widget {
                    let Widget::Window { root } = &self.nodes[parent].widget else {
                        violation(command, "a sidebar goes in a window")
                    };
                    if root.sidebar.get().is_some() {
                        violation(command, "a window has one sidebar");
                    }
                    let content = root.window.child("mitsuamiPage").expect("windows have a page");
                    page.set_str("title", &root.window.str("title"));
                    page.set_object("mitsuamiContent", Some(content));
                    root.window.set_object("mitsuamiSidebar", Some(page));
                    root.window.invoke("mitsuamiShowSidebar");
                    root.sidebar.set(Some((*child, page)));
                    // Again now the page has its row (`mitsuamiStack`), which
                    // sizes its column, and before the window grows by it.
                    self.run_tweak(*child);
                    // The content keeps its size: the window grows by the
                    // sidebar's column.
                    let size = root.size.get();
                    if !size.is_empty() {
                        root.resize_to(size);
                    }
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if let Widget::ToolbarItem { action, .. } = self.nodes[child].widget {
                    // Items come after the window's content, and before its sidebar.
                    let content = self
                        .nodes
                        .values()
                        .filter(|n| {
                            n.parent == Some(*parent)
                                && !matches!(n.kind, WidgetKind::ToolbarItem | WidgetKind::Sidebar)
                        })
                        .count();
                    let Some(index) = index.checked_sub(content) else {
                        violation(command, "toolbar items go after the window's content")
                    };
                    let Widget::Window { root } = &self.nodes[parent].widget else {
                        violation(command, "toolbar items go in windows")
                    };
                    let page = root.window.child("mitsuamiPage").expect("windows have a page");
                    page.set_object("mitsuamiAction", Some(action));
                    page.set_int("mitsuamiIndex", index as i32);
                    page.invoke("mitsuamiInsert");
                    root.toolbar.borrow_mut().insert(index, *child);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if let Widget::List(list) = &self.nodes[parent].widget {
                    let Some(row) = self.nodes[child].row else {
                        violation(command, "a List's children are row hosts, a Table's cell hosts")
                    };
                    list.insert(row, self.nodes[child].column.unwrap_or(0), *child, item);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                let parent_widget = self.widget(*parent, command);
                let content = parent_widget.content();
                if matches!(parent_widget, Widget::Scroll { .. }) && !content.child_items().is_empty() {
                    violation(command, "a ScrollView has a single native child (its content)");
                }
                if matches!(parent_widget, Widget::Tabs { .. }) && !matches!(self.nodes[child].widget, Widget::Host(_))
                {
                    violation(command, "a Tabs' children are page hosts (Containers)");
                }
                item.set_parent_item(Some(content), *index);
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                match &self.nodes[parent].widget {
                    Widget::Scroll { flickable, .. } => sync_scroll(*flickable),
                    // Shown if it's the page chosen, hidden if not.
                    Widget::Tabs { root, .. } => {
                        root.invoke("mitsuamiShow");
                    }
                    _ => {}
                }
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                match (&self.nodes[parent].widget, &self.nodes[child].widget) {
                    (Widget::List(list), _) => list.remove(
                        self.nodes[child].row.expect("inserted with a row"),
                        self.nodes[child].column.unwrap_or(0),
                    ),
                    (Widget::Window { root }, Widget::Sidebar { .. }) => hide_sidebar(root),
                    // Out of the page area, shown again wherever it goes.
                    (Widget::Tabs { root, .. }, child) => {
                        let item = child.item();
                        item.set_parent_item(None, 0);
                        item.set_bool("visible", true);
                        root.invoke("mitsuamiShow");
                    }
                    (Widget::Window { root }, Widget::ToolbarItem { host, action }) => {
                        // Out of the toolbar's item first, which goes with
                        // the action.
                        host.set_parent_item(None, 0);
                        let page = root.window.child("mitsuamiPage").expect("windows have a page");
                        page.set_object("mitsuamiAction", Some(*action));
                        page.invoke("mitsuamiRemove");
                        page.set_object("mitsuamiAction", None);
                        root.toolbar.borrow_mut().retain(|i| i != child);
                    }
                    _ => self.widget(*child, command).item().set_parent_item(None, 0),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                let Some(mut node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                self.pending_show.retain(|w| w != id);
                self.menus.forget(*id);
                // Before its item, which it's registered on.
                if let Some(hover) = node.hover.take() {
                    hover.remove();
                }
                if let Some(double_click) = node.double_click.take() {
                    double_click.remove();
                }
                // Not the item's child: a popup only has it as its parent.
                if let Some(menu) = &node.context_menu {
                    menu.delete_later();
                }
                if let Some(menu) = &node.button_menu {
                    menu.delete_later();
                }
                match &node.widget {
                    Widget::Window { root } => {
                        // The menu drawer isn't the window's child: it
                        // would outlive it, bound to a parent that's gone.
                        if let Some(drawer) = root.drawer.take() {
                            drawer.destroy();
                        }
                        root.window.destroy()
                    }
                    Widget::ToolbarItem { host, action } => {
                        host.destroy();
                        action.destroy();
                    }
                    // Out of its window's page row first, if it's still in
                    // it (the window goes too).
                    Widget::Sidebar { page, .. } => {
                        if let Some(Widget::Window { root }) =
                            node.parent.and_then(|p| self.nodes.get(&p)).map(|n| &n.widget)
                        {
                            hide_sidebar(root);
                        }
                        page.destroy();
                    }
                    // The app's handle may keep its surface: it just stops
                    // showing.
                    Widget::GpuSurface(surface) => {
                        surface.detach();
                        surface.item.destroy();
                    }
                    widget => widget.item().destroy(),
                }
            }
            Command::SetFrame { id, frame } => {
                // A toolbar item: its size; the toolbar places it, and
                // doesn't show it while it's empty.
                if let Widget::ToolbarItem { host, action } = self.widget(*id, command) {
                    host.set_geometry(0.0, 0.0, frame.width() as f64, frame.height() as f64);
                    action.set_bool("visible", !frame.size.is_empty());
                    return;
                }
                let widget = self.widget(*id, command);
                let item = widget.item();
                let parent = self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget);
                // A tab view's page: its size; it's at the top-left of the
                // page area.
                let at = match parent {
                    Some(Widget::Tabs { .. }) => Point::ZERO,
                    _ => frame.origin,
                };
                item.set_geometry(at.x as f64, at.y as f64, frame.width() as f64, frame.height() as f64);
                // Leaves with an empty frame (hidden, or not laid out yet)
                // aren't shown: controls draw their frames regardless of size.
                if widget.is_leaf() {
                    item.set_bool("visible", !frame.size.is_empty());
                }
                if let Some(Widget::List(list)) = parent {
                    list.row_measured(frame.height());
                }
                let flickable = match widget {
                    Widget::Scroll { flickable, .. } => Some(*flickable),
                    _ => self.scroll_parent(*id),
                };
                if let Some(flickable) = flickable {
                    sync_scroll(flickable);
                }
            }
            Command::SetA11y { id, a11y } => {
                let item = self.widget(*id, command).item();
                let A11yProps { label, description, hidden, .. } = a11y;
                let node = &self.nodes[id];
                // A switch's or select's accessible name is its label prop.
                let named_by_label = matches!(
                    node.widget,
                    Widget::Switch(_)
                        | Widget::Select(_)
                        | Widget::RadioGroup(_)
                        | Widget::Slider(_)
                        | Widget::NumberInput(_)
                        | Widget::Progress(_)
                        | Widget::Spinner(_)
                        | Widget::Icon(_)
                        | Widget::FileIcon(_)
                        | Widget::Image { .. }
                        | Widget::GpuSurface(_)
                );
                if !named_by_label || label.is_some() {
                    item.set_str("mitsuamiA11yName", label.as_deref().unwrap_or_default());
                }
                item.set_str("mitsuamiA11yDescription", description.as_deref().unwrap_or_default());
                item.set_bool("mitsuamiA11yHidden", *hidden);
            }
            // A window in full screen keeps the screen's size.
            Command::SetWindowSize { id, size } => {
                let root = self.window_root(*id, command);
                if !root.in_full_screen() {
                    root.resize_to(*size);
                }
            }
            Command::SetFocusOrder { window, order } => {
                // A tab view's bar, not its pages, whose controls follow it.
                let items: Vec<QmlObject> = order
                    .iter()
                    .map(|id| match self.widget(*id, command) {
                        widget @ Widget::Tabs { .. } => widget.input_item(),
                        widget => widget.item(),
                    })
                    .collect();
                let root = self.window_root(*window, command);
                root.window.set_tab_order(&items);
                // Qt Quick focuses nothing in a new window, so keys went
                // nowhere until a click or Tab. AppKit focuses its initial
                // first responder and GTK its first control; so does this.
                if !root.focused_first.get() {
                    let first = order
                        .iter()
                        .map(|id| self.widget(*id, command))
                        .find(|w| w.is_focusable() && (!w.is_control() || w.item().bool("enabled")));
                    if root.window.focus_item().and_then(|f| f.node()).is_some() {
                        root.focused_first.set(true);
                    } else if let Some(widget) = first {
                        widget.input_item().force_focus();
                        root.focused_first.set(true);
                    }
                }
            }
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll { flickable, .. }) => {
                    // Qt reports both through `contentX/YChanged`.
                    flickable.set_real("contentX", offset.x as f64);
                    flickable.set_real("contentY", offset.y as f64);
                }
                Some(Widget::List(list)) => list.scroll_to(*offset),
                _ => violation(command, "not a ScrollView or List"),
            },
            Command::Focus { id } => {
                let widget = self.widget(*id, command);
                if widget.is_focusable() {
                    widget.input_item().force_focus();
                }
            }
            Command::SelectText { id, range } => super::selection::select(self.widget(*id, command), range.clone()),
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row),
                _ => violation(command, "not a List"),
            },
        }
    }
}
