//! Applying the core's commands: the tree, frames, focus and scrolling.

use mitsuami_core::a11y::A11yProps;
use mitsuami_core::{ButtonStyle, Command, NodeId, Size, WidgetKind};
use objc2::Message;
use objc2::rc::Retained;
use objc2_app_kit::{NSAccessibility, NSApplication, NSView, NSWindowOrderingMode};
use objc2_foundation::{NSNotificationCenter, NSPoint, NSRect, NSSize};

use crate::sidebar::Split;
use crate::toolbar::Toolbar;

use super::scrolling::scroll_to;
use super::windows::{in_full_screen, post_empty_event};
use super::{State, Widget, key, ns, violation};

impl State {
    fn view(&self, id: NodeId, command: &Command) -> Retained<NSView> {
        match self.nodes.get(&id) {
            Some(node) => node.widget.view().retain(),
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) {
        let node = &self.nodes[&id];
        if let Some(run) = node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            match &node.widget {
                // The table or text view, not the scroll view around it.
                Widget::List(list) => run(&list.table),
                Widget::Sidebar(sidebar) => run(&sidebar.table),
                Widget::TextArea(area) => run(&area.text),
                // The box, not the layout host its children are in.
                Widget::Group { frame, .. } => run(frame),
                widget => run(widget.view()),
            }
        }
    }

    /// The toolbar item a node is a child of.
    pub(super) fn toolbar_item_of(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(&id)?.parent.filter(|p| self.nodes[p].kind == WidgetKind::ToolbarItem)
    }

    /// The item shows its button as its view (see `toolbar`).
    pub(super) fn toolbar_adopted(&self, item: NodeId) -> bool {
        let window = self.nodes[&item].parent.and_then(|w| self.nodes.get(&w));
        matches!(window.map(|w| &w.widget), Some(Widget::Window { toolbar: Some(toolbar), .. }) if toolbar.adopted(item))
    }

    fn release_toolbar_item(&mut self, item: NodeId) {
        if let Some(window) = self.nodes[&item].parent
            && let Some(Widget::Window { toolbar: Some(toolbar), .. }) =
                self.nodes.get_mut(&window).map(|n| &mut n.widget)
        {
            toolbar.release(item);
        }
    }

    /// The toolbar item a node is in, as its child or its grandchild (a
    /// button in a row of them).
    fn toolbar_item_above(&self, id: NodeId) -> Option<NodeId> {
        self.toolbar_item_of(id).or_else(|| self.toolbar_item_of(self.nodes.get(&id)?.parent?))
    }

    /// A node's children, in order.
    fn ordered_children(&self, id: NodeId) -> Vec<NodeId> {
        let by_view = self.by_view.borrow();
        self.nodes[&id].widget.view().subviews().iter().filter_map(|v| by_view.get(&key(&v)).copied()).collect()
    }

    /// A node or any node in it takes input: a toolbar item with none (a
    /// label, a progress bar) has no capsule.
    fn has_control(&self, id: NodeId) -> bool {
        // Indexed by parent once: scanning every node for each container on
        // the way down cost a pass per level.
        let mut children: std::collections::HashMap<NodeId, Vec<NodeId>> = std::collections::HashMap::new();
        for (child, node) in &self.nodes {
            if let Some(parent) = node.parent {
                children.entry(parent).or_default().push(*child);
            }
        }
        self.has_control_in(id, &children)
    }

    fn has_control_in(&self, id: NodeId, children: &std::collections::HashMap<NodeId, Vec<NodeId>>) -> bool {
        let Some(node) = self.nodes.get(&id) else { return false };
        match node.kind {
            WidgetKind::Container | WidgetKind::ToolbarItem => {
                children.get(&id).is_some_and(|c| c.iter().any(|child| self.has_control_in(*child, children)))
            }
            WidgetKind::Text
            | WidgetKind::Progress
            | WidgetKind::Spinner
            | WidgetKind::Separator
            | WidgetKind::Image
            | WidgetKind::Icon
            | WidgetKind::FileIcon => false,
            _ => true,
        }
    }

    /// An item whose only child is a button or a search field shows the
    /// control itself, as AppKit's toolbars have them; one whose only
    /// child is a row of two or more buttons shows them as one segmented
    /// control; any other shows its host, in a capsule only if there's a
    /// control in it.
    fn sync_toolbar_item(&mut self, item: NodeId) {
        enum Shown {
            Button(Retained<objc2_app_kit::NSButton>, bool),
            Field(Retained<objc2_app_kit::NSSearchField>),
            Group(Vec<Retained<objc2_app_kit::NSButton>>),
            Host(bool),
        }
        let Some(window) = self.nodes[&item].parent else { return };
        let mut children = self.nodes.iter().filter(|(_, n)| n.parent == Some(item));
        let shown = match (children.next(), children.next()) {
            (Some((&only_id, only)), None) => {
                let bordered = only.button_style != Some(ButtonStyle::Borderless);
                match &only.widget {
                    Widget::Button(button) => Shown::Button(button.clone(), bordered),
                    Widget::MenuButton { popup, .. } => Shown::Button(popup.clone().into_super(), bordered),
                    Widget::Host(_) if only.kind == WidgetKind::Container => {
                        let row = self.ordered_children(only_id);
                        let buttons: Vec<_> = row
                            .iter()
                            .filter_map(|c| match (&self.nodes[c].kind, &self.nodes[c].widget) {
                                (WidgetKind::Button, Widget::Button(button)) => Some(button.clone()),
                                _ => None,
                            })
                            .collect();
                        if buttons.len() >= 2 && buttons.len() == row.len() {
                            Shown::Group(buttons)
                        } else {
                            Shown::Host(self.has_control(item))
                        }
                    }
                    widget => match widget.view().downcast_ref::<objc2_app_kit::NSSearchField>() {
                        Some(field) => Shown::Field(field.retain()),
                        None => Shown::Host(self.has_control(item)),
                    },
                }
            }
            _ => Shown::Host(self.has_control(item)),
        };
        let mtm = self.mtm;
        let Some(Widget::Window { toolbar: Some(toolbar), .. }) = self.nodes.get_mut(&window).map(|n| &mut n.widget)
        else {
            return;
        };
        match shown {
            Shown::Button(button, bordered) => toolbar.adopt_button(item, &button, bordered),
            Shown::Field(field) => toolbar.adopt_field(item, &field),
            Shown::Group(buttons) => toolbar.adopt_group(mtm, item, buttons),
            Shown::Host(glass) => toolbar.show_host(item, glass),
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
                self.load_file_icon(*id);
                self.run_tweak(*id);
            }
            Command::SetProp { id, prop } => {
                self.set_prop(*id, prop, command);
                // A toolbar's button keeps the toolbar's bezel, and a
                // group's segment shows its button.
                if let Some(item) = self.toolbar_item_above(*id) {
                    self.sync_toolbar_item(item);
                }
                self.run_tweak(*id);
            }
            Command::Insert { parent, child, index } => {
                let parent_view = self.view(*parent, command);
                let child_view = self.view(*child, command);
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if let Widget::Scroll(scroll) = &self.nodes[parent].widget {
                    if scroll.documentView().is_some() {
                        violation(command, "a ScrollView has a single native child (its content)");
                    }
                    scroll.setDocumentView(Some(&child_view));
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if self.nodes[child].kind == WidgetKind::Sidebar {
                    let (mtm, animate) = (self.mtm, self.options.show_windows);
                    let Widget::Sidebar(sidebar) = &self.nodes[child].widget else { unreachable!() };
                    let scroll = sidebar.scroll.clone();
                    let shown = sidebar.shown.get();
                    let Widget::Window { window, host, _delegate, toolbar, split } =
                        &mut self.nodes.get_mut(parent).unwrap().widget
                    else {
                        violation(command, "a sidebar goes in a window")
                    };
                    if split.is_some() {
                        violation(command, "a window has one sidebar");
                    }
                    // The content keeps its size: the window grows by the
                    // sidebar. The title and toolbar items go over the
                    // content, as a unified toolbar puts them.
                    let size = host.frame().size;
                    toolbar.get_or_insert_with(|| Toolbar::new(mtm, window, *parent, animate)).set_sidebar(true);
                    let made = Split::new(mtm, window, host, *child, &scroll, self.events.clone());
                    if let Some(shown) = shown {
                        made.set_shown(shown);
                    }
                    *split = Some(made);
                    _delegate.set_detail(Some(host));
                    let (window, delegate) = (window.clone(), _delegate.clone());
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    // Again, now that it's in the window's split view,
                    // which its props came before: a width it sets is in
                    // the window's.
                    self.run_tweak(*child);
                    window.setContentSize(
                        delegate.at_least_min(&window, Size::new(size.width as f32, size.height as f32)),
                    );
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
                    let (mtm, animate) = (self.mtm, self.options.show_windows);
                    let Widget::Window { window, toolbar, .. } = &mut self.nodes.get_mut(parent).unwrap().widget else {
                        violation(command, "toolbar items go in windows")
                    };
                    let Some(index) = index.checked_sub(content) else {
                        violation(command, "toolbar items go after the window's content")
                    };
                    toolbar.get_or_insert_with(|| Toolbar::new(mtm, window, *parent, animate)).insert(
                        mtm,
                        *child,
                        &child_view,
                        index,
                    );
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    self.sync_toolbar_item(*child);
                    return;
                }
                if let Widget::Tabs(tabs) = &self.nodes[parent].widget {
                    tabs.insert(self.mtm, *child, child_view, *index);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                if let Widget::List(list) = &self.nodes[parent].widget {
                    let Some(row) = self.nodes[child].row else {
                        violation(command, "a List's children are row hosts, a Table's cell hosts")
                    };
                    list.insert(row, self.nodes[child].column.unwrap_or(0), *child, child_view);
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return;
                }
                // Back in its host first, among the children it's placed.
                let item = (self.nodes[parent].kind == WidgetKind::ToolbarItem).then_some(*parent);
                if let Some(item) = item {
                    self.release_toolbar_item(item);
                }
                let siblings = parent_view.subviews();
                // A group's box is behind its children.
                let index = &(index + usize::from(matches!(self.nodes[parent].widget, Widget::Group { .. })));
                if *index >= siblings.len() {
                    parent_view.addSubview(&child_view);
                } else {
                    let before = siblings.objectAtIndex(*index);
                    parent_view.addSubview_positioned_relativeTo(
                        &child_view,
                        NSWindowOrderingMode::Below,
                        Some(&before),
                    );
                }
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                // A button in a row of them changes the group.
                if let Some(item) = item.or_else(|| self.toolbar_item_of(*parent)) {
                    self.sync_toolbar_item(item);
                }
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                let (row, column) = (self.nodes[child].row, self.nodes[child].column.unwrap_or(0));
                if let Widget::Window { window, host, _delegate, toolbar, split } =
                    &mut self.nodes.get_mut(parent).unwrap().widget
                    && split.as_ref().is_some_and(|s| s.sidebar == *child)
                {
                    // The window loses the sidebar, and the content keeps
                    // its size.
                    let size = host.frame().size;
                    split.take().unwrap().remove(window, host);
                    _delegate.set_detail(None);
                    if let Some(bar) = toolbar {
                        bar.set_sidebar(false);
                        if bar.is_empty() {
                            window.setToolbar(None);
                            *toolbar = None;
                        }
                    }
                    window.setContentSize(
                        _delegate.at_least_min(window, Size::new(size.width as f32, size.height as f32)),
                    );
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return;
                }
                let item = (self.nodes[parent].kind == WidgetKind::ToolbarItem).then_some(*parent);
                if let Some(item) = item {
                    self.release_toolbar_item(item);
                }
                match &mut self.nodes.get_mut(parent).unwrap().widget {
                    Widget::Window { toolbar: Some(toolbar), .. } if toolbar.contains(*child) => toolbar.remove(*child),
                    Widget::Scroll(scroll) => scroll.setDocumentView(None),
                    Widget::List(list) => list.remove(row.expect("inserted with a row"), column),
                    Widget::Tabs(tabs) => tabs.remove(*child),
                    _ => self.view(*child, command).removeFromSuperview(),
                }
                self.nodes.get_mut(child).unwrap().parent = None;
                if let Some(item) = item.or_else(|| self.toolbar_item_of(*parent)) {
                    self.sync_toolbar_item(item);
                }
            }
            Command::Destroy { id } => {
                let Some(mut node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                self.by_view.borrow_mut().remove(&key(node.widget.view()));
                self.pending_show.retain(|w| w != id);
                self.focus_orders.remove(id);
                if let Some(target) = &node._target {
                    unsafe { NSNotificationCenter::defaultCenter().removeObserver(target) };
                }
                // A tracking area doesn't retain its owner, and the view may
                // outlive the node (the app's native view, a surface).
                if let Some((_, area)) = &node.hover {
                    node.widget.view().removeTrackingArea(area);
                }
                if let Some((_, recognizer)) = &node.double_click {
                    node.widget.view().removeGestureRecognizer(recognizer);
                }
                match &mut node.widget {
                    Widget::Window { window, _delegate, split, .. } => {
                        // Out of the sheet, or of the modal loop, first.
                        if let Some(parent) = window.sheetParent() {
                            parent.endSheet(window);
                        }
                        let app = NSApplication::sharedApplication(self.mtm);
                        if app.modalWindow().is_some_and(|m| std::ptr::eq(&*m, &**window)) {
                            // `stopModal` only works from an event handler;
                            // this runs in a tick. The empty event makes the
                            // loop notice.
                            app.abortModal();
                            post_empty_event(&app);
                        }
                        _delegate.stop_observing_focus(window);
                        // Its sidebar was destroyed without a Remove: the
                        // split's item outlives the node, with the window.
                        if let Some(split) = split.take() {
                            split.detach();
                        }
                        window.setDelegate(None);
                        window.close();
                    }
                    Widget::Sidebar(sidebar) => {
                        sidebar.detach();
                        sidebar.scroll.removeFromSuperview();
                    }
                    Widget::List(list) => {
                        list.detach();
                        list.scroll.removeFromSuperview();
                    }
                    // The app's handle may keep it: it just stops showing.
                    Widget::GpuSurface(view) => {
                        view.detach();
                        view.removeFromSuperview();
                    }
                    widget => widget.view().removeFromSuperview(),
                }
            }
            Command::SetFrame { id, frame } => {
                let rect = NSRect::new(
                    NSPoint::new(frame.x() as f64, frame.y() as f64),
                    NSSize::new(frame.width() as f64, frame.height() as f64),
                );
                if let Some(Widget::List(list)) = self.nodes.get(id).map(|n| &n.widget) {
                    list.set_frame(rect);
                    return;
                }
                // A toolbar item is where the toolbar puts it, at this size.
                if let Some(window) =
                    self.nodes.get(id).filter(|n| n.kind == WidgetKind::ToolbarItem).and_then(|n| n.parent)
                    && let Some(Widget::Window { toolbar: Some(toolbar), .. }) =
                        self.nodes.get_mut(&window).map(|n| &mut n.widget)
                {
                    toolbar.set_size(*id, rect.size.width, rect.size.height);
                    return;
                }
                // A row fills its cell, and the table makes the row as high;
                // a table's cell sits centred in its cell, and the table
                // makes the row as high as its highest.
                // A toolbar's control is the toolbar's to size and place.
                if self.toolbar_item_of(*id).is_some_and(|item| self.toolbar_adopted(item)) {
                    return;
                }
                let parent = self.nodes.get(id).and_then(|n| n.parent).map(|p| &self.nodes[&p].widget);
                // A page is where its tab view puts it, at this size.
                if let Some(Widget::Tabs(tabs)) = parent {
                    tabs.set_page_size(*id, rect.size);
                    return;
                }
                if let Some(Widget::List(list)) = parent {
                    self.view(*id, command).setFrame(rect);
                    if let Some(row) = self.nodes[id].row {
                        list.set_host_height(row, self.nodes[id].column.unwrap_or(0), frame.height());
                    }
                    return;
                }
                let view = self.view(*id, command);
                let rect = NSRect::new(
                    NSPoint::new(frame.x() as f64, frame.y() as f64),
                    NSSize::new(frame.width() as f64, frame.height() as f64),
                );
                // A separator is its line: its alignment insets follow its
                // frame's shape, so a vertical one set through them while
                // it's still horizontal would land 2pt off each end.
                if matches!(self.nodes[id].widget, Widget::Separator(_)) {
                    view.setFrame(rect);
                    return;
                }
                // Layout places what the user sees, the alignment rect, as
                // Auto Layout does; controls draw their bezels inset from
                // their frames (a push button by 7pt a side before macOS 26).
                view.setFrame(view.frameForAlignmentRect(rect));
            }
            Command::SetA11y { id, a11y } => {
                let view = self.view(*id, command);
                let A11yProps { label, description, hidden, .. } = a11y;
                view.setAccessibilityLabel(label.as_deref().map(ns).as_deref());
                view.setAccessibilityHelp(description.as_deref().map(ns).as_deref());
                if *hidden {
                    view.setAccessibilityElement(false);
                }
            }
            // AppKit would resize a window in full screen, and below its
            // minimum: it keeps the screen's size, and its minimum. A
            // locked height is the user's limit, not the app's.
            Command::SetWindowSize { id, size } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Window { window, _delegate, .. }) => {
                    if !in_full_screen(window) {
                        window.setContentSize(_delegate.at_least_min(window, *size));
                    }
                }
                _ => violation(command, "not a window"),
            },
            Command::SetFocusOrder { window, order } => {
                let ns_window = match self.nodes.get(window).map(|n| &n.widget) {
                    Some(Widget::Window { window, .. }) => window.clone(),
                    _ => violation(command, "not a window"),
                };
                // Unlink the previous chain, then link the new one as a loop.
                // AppKit still skips views that can't take focus right now
                // (disabled, or not reachable under the user's keyboard
                // navigation setting).
                for old in self.focus_orders.remove(window).unwrap_or_default() {
                    if let Some(node) = self.nodes.get(&old) {
                        unsafe { node.widget.key_view().setNextKeyView(None) };
                    }
                }
                let views: Vec<Retained<NSView>> = order
                    .iter()
                    .map(|id| match self.nodes.get(id) {
                        Some(node) => node.widget.key_view(),
                        None => violation(command, &format!("node {id} does not exist")),
                    })
                    .collect();
                for (i, view) in views.iter().enumerate() {
                    let next = &views[(i + 1) % views.len()];
                    unsafe { view.setNextKeyView(Some(next)) };
                }
                ns_window.setInitialFirstResponder(views.first().map(|v| &**v));
                self.focus_orders.insert(*window, order.clone());
            }
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll(scroll)) => scroll_to(scroll, NSPoint::new(offset.x as f64, offset.y as f64)),
                Some(Widget::List(list)) => scroll_to(&list.scroll, NSPoint::new(offset.x as f64, offset.y as f64)),
                _ => violation(command, "not a ScrollView, List or Table"),
            },
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row),
                _ => violation(command, "not a List"),
            },
            Command::Focus { id } => {
                let Some(node) = self.nodes.get(id) else { violation(command, "node does not exist") };
                let view = node.widget.key_view();
                if let Some(window) = view.window() {
                    window.makeFirstResponder(Some(&view));
                }
            }
            Command::SelectText { id, range } => {
                let Some(node) = self.nodes.get(id) else { violation(command, "node does not exist") };
                super::selection::select(&node.widget, range.clone());
            }
        }
    }
}
