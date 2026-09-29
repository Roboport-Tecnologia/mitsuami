//! What the mirror shows: each node's props and frame, with the frames platforms place themselves.

use mitsuami_core::backend::NativeState;
use mitsuami_core::{NodeId, Rect, WidgetKind, find_prop};

use super::metrics::{SIDEBAR_WIDTH, TAB_INSETS, TOOLBAR_HEIGHT, TOOLBAR_SPACING};
use super::{HeadlessBackend, State};

impl HeadlessBackend {
    pub(super) fn read_native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        // A list places its rows itself: the core only gives their sizes.
        let row = find_prop!(node.props, Row)
            .zip(node.parent.filter(|p| state.nodes[p].kind == WidgetKind::List))
            .and_then(|(key, list)| state.row(list, key));
        let frame = match row {
            Some(row) => Rect::new(0.0, row.top, node.frame.width(), node.frame.height()),
            None if node.kind == WidgetKind::ToolbarItem => state.toolbar_item_frame(id),
            None if node.kind == WidgetKind::Sidebar => state.sidebar_frame(id),
            // A tab view places its pages itself, and shows one.
            None if let Some(tabs) = node.parent.filter(|p| state.nodes[p].kind == WidgetKind::Tabs) => {
                state.page_frame(id, tabs)
            }
            None => node.frame,
        };
        Some(NativeState {
            kind: node.kind,
            props: node.props.clone(),
            frame,
            parent: node.parent,
            children: node.children.clone(),
            focused: state.focused == Some(id),
            scroll_offset: node.kind.scrolls().then_some(node.scroll_offset),
        })
    }
}

impl State {
    /// Where a tab view shows a page: inside its strip and border, if it's
    /// the one shown; nowhere otherwise.
    pub(super) fn page_frame(&self, page: NodeId, tabs: NodeId) -> Rect {
        let tabs = &self.nodes[&tabs];
        let shown = find_prop!(tabs.props, SelectedIndex).flatten().and_then(|i| tabs.children.get(i));
        if shown != Some(&page) {
            return Rect::ZERO;
        }
        let size = self.nodes[&page].frame.size;
        Rect::new(TAB_INSETS.left, TAB_INSETS.top, size.width, size.height)
    }

    /// Where a window shows its sidebar, in the window's content
    /// coordinates: beside the content, on its leading side, as high.
    pub(super) fn sidebar_frame(&self, sidebar: NodeId) -> Rect {
        let node = &self.nodes[&sidebar];
        let Some(window) = node.parent.map(|p| &self.nodes[&p]) else { return Rect::ZERO };
        // A hidden one is empty, as a collapsed one is on every platform.
        if find_prop!(node.props, SidebarShown) == Some(false) {
            return Rect::ZERO;
        }
        Rect::new(-SIDEBAR_WIDTH, 0.0, SIDEBAR_WIDTH, window.frame.height())
    }

    /// Where a window's toolbar shows an item, in the window's content
    /// coordinates: in the bar above the content, centred in its height,
    /// the last item at the trailing edge. Empty items are hidden, and take
    /// no room.
    pub(super) fn toolbar_item_frame(&self, item: NodeId) -> Rect {
        let node = &self.nodes[&item];
        let Some(window) = node.parent.map(|p| &self.nodes[&p]) else { return node.frame };
        let size = node.frame.size;
        if size.is_empty() {
            return Rect::ZERO;
        }
        let after: f32 = window
            .children
            .iter()
            .skip_while(|c| **c != item)
            .skip(1)
            .map(|c| self.nodes[c].frame.size)
            .filter(|s| !s.is_empty())
            .map(|s| s.width + TOOLBAR_SPACING)
            .sum();
        let x = window.frame.width() - TOOLBAR_SPACING - after - size.width;
        let y = -TOOLBAR_HEIGHT + ((TOOLBAR_HEIGHT - size.height) / 2.0).round();
        Rect::new(x, y, size.width, size.height)
    }
}
