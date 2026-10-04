//! Keyboard focus: the Tab order and focus requests.

use crate::command::Command;
use crate::widget::{NodeId, WidgetKind};

use super::{Inner, Ui};

impl Ui {
    /// Moves a control ahead in the Tab order: controls with a tab index
    /// come first, lowest index first, then everything else in tree order.
    pub fn set_tab_index(&self, id: NodeId, index: Option<u32>) {
        let mut inner = self.inner.borrow_mut();
        if let Some(node) = inner.nodes.get_mut(&id) {
            node.tab_index = index;
            inner.focus_dirty = true;
        }
        drop(inner);
        self.changed();
    }

    /// The control that has keyboard focus in `window`, if any.
    pub fn focused(&self, window: NodeId) -> Option<NodeId> {
        self.inner.borrow().focused.get(&window).copied()
    }

    /// The keyboard order of a window's focusable controls, as sent to the
    /// backend.
    pub fn focus_order(&self, window: NodeId) -> Vec<NodeId> {
        self.inner.borrow().focus_order(window)
    }

    /// Gives the control keyboard focus, at the next commit.
    pub fn focus(&self, id: NodeId) {
        self.inner.borrow_mut().pending_focus.push(id);
        self.changed();
    }

    /// Focuses a text field or text area and selects these grapheme
    /// clusters of its text, at the next commit; a range past its end is
    /// cut to it. Other controls are only focused.
    pub fn select_text(&self, id: NodeId, range: std::ops::Range<usize>) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_focus.push(id);
        let text = inner.nodes.get(&id).is_some_and(|n| {
            matches!(
                n.kind,
                WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea
            )
        });
        if text {
            inner.pending_selections.push((id, range));
        }
        drop(inner);
        self.changed();
    }
}

impl Inner {
    /// Focusable controls in reading (tree) order, explicit tab indices first.
    fn focus_order(&self, window: NodeId) -> Vec<NodeId> {
        fn walk(inner: &Inner, id: NodeId, out: &mut Vec<(Option<u32>, NodeId)>) {
            let node = &inner.nodes[&id];
            // Whether Tab reaches the toolbar is the platform's call.
            if node.style.is_hidden() || node.kind == WidgetKind::ToolbarItem || inner.is_hidden_page(id) {
                return;
            }
            if matches!(
                node.kind,
                WidgetKind::Button
                    | WidgetKind::ToggleButton
                    | WidgetKind::MenuButton
                    | WidgetKind::TextInput
                    | WidgetKind::PasswordInput
                    | WidgetKind::SearchInput
                    | WidgetKind::TextArea
                    | WidgetKind::Checkbox
                    | WidgetKind::Switch
                    | WidgetKind::Select
                    | WidgetKind::RadioGroup
                    | WidgetKind::Slider
                    | WidgetKind::NumberInput
                    | WidgetKind::List
                    | WidgetKind::Table
                    | WidgetKind::Sidebar
                    | WidgetKind::Tabs
            ) || (node.kind == WidgetKind::GpuSurface && crate::find_prop!(node.props, TakesInput) == Some(true))
            {
                out.push((node.tab_index, id));
            }
            // A window's sidebar comes first: it's on the leading side, and
            // picks what the content shows.
            let mut children = node.native_children.clone();
            children.sort_by_key(|c| inner.nodes[c].kind != WidgetKind::Sidebar);
            for child in children {
                walk(inner, child, out);
            }
        }
        let mut entries = Vec::new();
        if self.nodes.contains_key(&window) {
            walk(self, window, &mut entries);
        }
        // Stable sort: explicit indices first (ascending), tree order otherwise.
        entries.sort_by_key(|(index, _)| index.unwrap_or(u32::MAX));
        entries.into_iter().map(|(_, id)| id).collect()
    }

    pub(super) fn sync_focus_orders(&mut self) {
        for window in self.windows.clone() {
            let order = self.focus_order(window);
            if self.focus_orders.get(&window) != Some(&order) {
                self.focus_orders.insert(window, order.clone());
                self.pending.push(Command::SetFocusOrder { window, order });
            }
        }
    }
}
