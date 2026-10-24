//! The backend's handle: its command log and test hooks.

use mitsuami_core::{Command, NativeAppInfo, NativeIcon, NodeId, Size, WidgetKind};
use objc2::Message;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSView};
use objc2_foundation::{NSBundle, NSDate, NSDefaultRunLoopMode, NSProcessInfo, NSRunLoop};

use super::{AppKitHandle, State, Widget};

impl AppKitHandle {
    /// The app's name as its menu shows it: the app's, or else the
    /// process's (a bundled app's executable).
    pub(crate) fn app_name(&self) -> String {
        let name = self.state.borrow().app_name.clone();
        name.unwrap_or_else(|| NSProcessInfo::processInfo().processName().to_string())
    }

    pub fn command_log(&self) -> Vec<Command> {
        self.state.borrow().log.clone()
    }

    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    /// Number of live native nodes.
    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }

    /// Escape hatch: the native view of any node (a window's content view).
    pub fn ns_view(&self, id: NodeId) -> Option<Retained<NSView>> {
        let state = self.state.borrow();
        let view = state.nodes.get(&id)?.widget.view();
        Some(view.retain())
    }
}

impl mitsuami_core::TestHooks for AppKitHandle {
    fn name(&self) -> &'static str {
        "appkit"
    }

    fn dragged_files(&self, node: NodeId) -> Option<Vec<std::path::PathBuf>> {
        let state = self.state.borrow();
        let host = state.nodes.get(&node)?;
        match &state.nodes.get(&host.parent?)?.widget {
            Widget::List(list) => list.dragged_files(host.row?),
            _ => None,
        }
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        AppKitHandle::resize_window(self, window, size);
    }

    /// As the close button does: the window asks its delegate.
    fn close_window(&self, window: NodeId) {
        if let Some(window) = self.ns_window(window) {
            window.performClose(None);
        }
    }

    fn take_command_log(&self) -> Vec<Command> {
        AppKitHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        AppKitHandle::node_count(self)
    }

    /// The id is the bundle's; the name is kept, as only the menus show
    /// it; the icon is the Dock's, in points.
    fn app_info(&self, _window: NodeId) -> NativeAppInfo {
        let mtm = self.state.borrow().mtm;
        let id = NSBundle::mainBundle().bundleIdentifier().map(|id| id.to_string());
        // In points: AppKit keeps a snapshot at the screen's scale.
        let icon = NSApplication::sharedApplication(mtm).applicationIconImage().map(|image| {
            let size = image.size();
            NativeIcon::Image { width: size.width.round() as u32, height: size.height.round() as u32 }
        });
        NativeAppInfo { id, name: self.state.borrow().app_name.clone(), icon }
    }

    /// Offscreen windows get no display cycle, where tables add the rows
    /// scrolling brought into view and toolbars place their items. While a
    /// search field is shown, timers that are due fire, as in the app's run
    /// loop: it searches once typing pauses. Not always yet: tables and tab
    /// views run timers too, which change what their tests capture.
    fn settle(&self) {
        if self.state.borrow().nodes.values().any(|n| n.kind == WidgetKind::SearchInput) {
            NSRunLoop::currentRunLoop().limitDateForMode(unsafe { NSDefaultRunLoopMode });
        }
        // Thumbnails arrive through the main queue: run the run loop until
        // they're shown, as the app's would, so captures have them.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while super::images::thumbnails_pending() && std::time::Instant::now() < deadline {
            let until = NSDate::dateWithTimeIntervalSinceNow(0.01);
            NSRunLoop::currentRunLoop().runMode_beforeDate(unsafe { NSDefaultRunLoopMode }, &until);
        }
        let state = self.state.borrow();
        state.layout_lists();
        state.layout_toolbars();
        state.extend_under_bars();
    }
}

impl State {
    /// Lets toolbars place their items now, and split windows their
    /// content: a window that was never shown (as in tests) doesn't even
    /// make its toolbar's views before.
    pub(super) fn layout_toolbars(&self) {
        for node in self.nodes.values() {
            layout_toolbar(&node.widget);
        }
    }

    /// Lets tables lay out their rows now rather than at the next display:
    /// they report the rows they show, and the core builds those in the
    /// same run-loop turn, before anything is drawn. Their callbacks only
    /// emit.
    pub(super) fn layout_lists(&self) {
        for node in self.nodes.values() {
            layout_list(&node.widget);
        }
    }

    /// Notes the lists and windows a command touched: a list made, given
    /// props, sized, given or losing rows, or scrolled; a window made,
    /// given props or sized, or its toolbar items or sidebar changed.
    /// Anything that can bring a list into view without touching it (a tab
    /// view's page, a scroll view's offset) touches every list.
    pub(super) fn note_touched(&mut self, command: &Command) {
        let ids = match command {
            Command::Create { id, .. }
            | Command::SetProp { id, .. }
            | Command::SetFrame { id, .. }
            | Command::SetWindowSize { id, .. }
            | Command::ScrollTo { id, .. }
            | Command::ScrollToRow { id, .. } => [Some(*id), None],
            Command::Insert { parent, child, .. } | Command::Remove { parent, child } => [Some(*parent), Some(*child)],
            _ => return,
        };
        for id in ids.into_iter().flatten() {
            let Some(node) = self.nodes.get(&id) else { continue };
            match &node.widget {
                Widget::List(_) => {
                    self.touched_lists.insert(id);
                }
                Widget::Window { .. } => {
                    self.touched_windows.insert(id);
                }
                Widget::Tabs(_) | Widget::Scroll(_) => self.touched_all_lists = true,
                _ => {}
            }
            let parent = node.parent.and_then(|p| Some((p, &self.nodes.get(&p)?.widget)));
            match parent {
                // A row or cell host: its height is its row's.
                Some((list, Widget::List(_))) => {
                    self.touched_lists.insert(list);
                }
                // A sidebar or toolbar item.
                Some((window, Widget::Window { .. })) => {
                    self.touched_windows.insert(window);
                }
                _ => {}
            }
            // In a toolbar item, as its child or grandchild.
            if let Some(window) = self.toolbar_item_above(id).and_then(|item| self.nodes[&item].parent) {
                self.touched_windows.insert(window);
            }
        }
    }

    /// Lays out the lists and toolbars the batch touched (see
    /// `layout_lists` and `layout_toolbars`): every one, after every
    /// batch, forced each table to lay out again.
    pub(super) fn layout_touched(&mut self) {
        if std::mem::take(&mut self.touched_all_lists) {
            self.touched_lists.clear();
            self.layout_lists();
        }
        for id in std::mem::take(&mut self.touched_lists) {
            if let Some(node) = self.nodes.get(&id) {
                layout_list(&node.widget);
            }
        }
        for id in std::mem::take(&mut self.touched_windows) {
            if let Some(node) = self.nodes.get(&id) {
                layout_toolbar(&node.widget);
            }
        }
        self.place_headers();
        self.extend_under_bars();
    }

    /// Puts each header at the top of a window's content in that window's
    /// title bar, and takes out those that left it (see `header`). Beside a
    /// sidebar a header stays where it is: the title bar is over the
    /// sidebar too.
    fn place_headers(&mut self) {
        let headers: Vec<NodeId> = self.nodes.iter().filter(|(_, n)| n.header.is_some()).map(|(id, _)| *id).collect();
        for id in headers {
            let want = self.header_window(id);
            let node = &self.nodes[&id];
            let (header, placeholder) = (node.header.as_ref().unwrap(), node.widget.view());
            let height = placeholder.frame().size.height;
            let had = header.window.get();
            if had != want {
                let host_of = |w: Option<NodeId>| match w.and_then(|w| self.nodes.get(&w)).map(|n| &n.widget) {
                    Some(Widget::Window { host, .. }) => Some(host.clone()),
                    _ => None,
                };
                header.detach(placeholder, host_of(had).as_deref().map(|h| &**h));
                if let Some(w) = want
                    && let Some(Widget::Window { window, host, _delegate, .. }) = self.nodes.get(&w).map(|n| &n.widget)
                {
                    // As the first toolbar item does: the content keeps its
                    // size, and the window grows by the bar.
                    let size = host.frame().size;
                    crate::sidebar::content_under_bar(self.mtm, window, host);
                    _delegate.set_detail(Some(host));
                    header.attach(w, window, host, height);
                    window.setContentSize(
                        _delegate.at_least_min(window, Size::new(size.width as f32, size.height as f32)),
                    );
                }
            } else if let Some(w) = want
                && let Some(Widget::Window { host, .. }) = self.nodes.get(&w).map(|n| &n.widget)
            {
                header.set_height(host, height);
            }
        }
    }

    /// The window whose title bar a header goes in: the one it's at the
    /// top of the content of, if that window has no sidebar.
    fn header_window(&self, id: NodeId) -> Option<NodeId> {
        let node = self.nodes.get(&id)?;
        if !node.header.as_ref()?.on.get() {
            return None;
        }
        let mut parent = node.parent;
        while let Some(p) = parent {
            let above = self.nodes.get(&p)?;
            match &above.widget {
                Widget::Window { host, split: None, .. } => return at_top(node.widget.view(), host, 0.0).then_some(p),
                Widget::Window { .. } | Widget::Scroll(_) | Widget::List(_) | Widget::Tabs(_) | Widget::Sidebar(_) => {
                    return None;
                }
                _ if above.kind == WidgetKind::ToolbarItem => return None,
                _ => parent = above.parent,
            }
        }
        None
    }

    /// Runs the scroll views, lists and tables at the top of a window with
    /// a sidebar or a toolbar up under its title bar and toolbar, as
    /// Finder's content does: the content view is full size there, and the
    /// host below the bar. One inside a scroll view or a list moves as that scrolls, so
    /// it stays put. A list that moved lays out the rows now in view. In a
    /// window with a header in its title bar, the one right under the
    /// header runs up under it too, and so do the splits it's in, with
    /// their panes: macOS shows nothing scrolling under its bars from a
    /// split's pane that doesn't reach them.
    pub(super) fn extend_under_bars(&self) {
        // The windows' headers' heights.
        let headers: std::collections::HashMap<NodeId, f64> = self
            .nodes
            .values()
            .filter_map(|n| Some((n.header.as_ref()?.window.get()?, n.widget.view().frame().size.height)))
            .collect();
        // How far up each split runs, for the scroll views at their tops.
        let mut splits: std::collections::HashMap<NodeId, f64> = std::collections::HashMap::new();
        for node in self.nodes.values() {
            let scroll = match &node.widget {
                Widget::Scroll(scroll) => scroll,
                Widget::List(list) => &list.scroll,
                _ => continue,
            };
            let mut host = None;
            let mut around = Vec::new();
            let mut parent = node.parent;
            while let Some((id, node)) = parent.and_then(|id| Some((id, self.nodes.get(&id)?))) {
                match &node.widget {
                    Widget::SplitView(_) => {
                        around.push(id);
                        parent = node.parent;
                        continue;
                    }
                    Widget::Window { host: window, split: Some(_), .. } => host = Some((window, 0.0)),
                    Widget::Window { host: window, toolbar: Some(_), .. } => {
                        host = Some((window, headers.get(&id).copied().unwrap_or(0.0)))
                    }
                    Widget::Window { host: window, .. } => host = headers.get(&id).map(|h| (window, *h)),
                    Widget::Scroll(_) | Widget::List(_) => {}
                    _ => {
                        parent = node.parent;
                        continue;
                    }
                }
                break;
            }
            // From where the core put it to the window's top: past the
            // header, if it's right under one, and the bar.
            let height = match host {
                Some((host, header)) if at_top(scroll, host, header) => {
                    unsafe { host.superview() }.map_or(0.0, |v| v.safeAreaInsets().top)
                }
                _ => 0.0,
            };
            if height > 0.0 {
                for split in around {
                    splits.insert(split, height);
                }
            }
            if scroll.set_under_bar(height) {
                layout_list(&node.widget);
            }
        }
        for (id, node) in &self.nodes {
            if let Widget::SplitView(split) = &node.widget {
                split.set_under_bar(splits.get(id).copied().unwrap_or(0.0));
            }
        }
    }
}

/// Whether the core put the view at `top` in its window's host: its top
/// (0), or right under a header that high.
fn at_top(view: &NSView, host: &NSView, top: f64) -> bool {
    let Some(superview) = unsafe { view.superview() }.filter(|_| view.isDescendantOf(host)) else { return false };
    let placed = superview.convertRect_toView(view.alignmentRectForFrame(view.frame()), Some(host));
    (placed.origin.y - top).abs() < 0.5 && placed.size.height > 0.0
}

fn layout_toolbar(widget: &Widget) {
    if let Widget::Window { window, toolbar, split, .. } = widget
        && (toolbar.is_some() || split.is_some())
    {
        window.layoutIfNeeded();
    }
}

fn layout_list(widget: &Widget) {
    if let Widget::List(list) = widget {
        // Scrolling doesn't mark the table as needing layout; its rows
        // follow at the next display.
        list.table.setNeedsLayout(true);
        list.scroll.layoutSubtreeIfNeeded();
    }
}
