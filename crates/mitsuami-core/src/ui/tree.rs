//! The tree's structure, and keeping the backend's native children and
//! props in step with it.

use crate::command::Command;
use crate::widget::{NodeId, Prop, WidgetKind};

use super::{Inner, Ui, in_chrome};

impl Ui {
    /// Inserts `child` under `parent` at `index` (or at the end), detaching it
    /// from its previous parent first.
    pub fn insert_child(&self, parent: NodeId, child: NodeId, index: Option<usize>) {
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.nodes.contains_key(&parent) || !inner.nodes.contains_key(&child) {
                return;
            }
            inner.detach(child);
            let siblings = &mut inner.nodes.get_mut(&parent).unwrap().children;
            let index = index.unwrap_or(siblings.len()).min(siblings.len());
            siblings.insert(index, child);
            inner.nodes.get_mut(&child).unwrap().parent = Some(parent);
            inner.mark_resync(parent);
            inner.styles_dirty = true;
        }
        self.changed();
    }

    pub fn append_child(&self, parent: NodeId, child: NodeId) {
        self.insert_child(parent, child, None);
    }

    /// Replaces the children of `parent`, keeping nodes that stay.
    pub fn set_children(&self, parent: NodeId, children: Vec<NodeId>) {
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.nodes.contains_key(&parent) {
                return;
            }
            let old = inner.nodes[&parent].children.clone();
            for child in old.iter().filter(|c| !children.contains(c)) {
                inner.detach(*child);
            }
            for child in &children {
                if inner.nodes.get(child).and_then(|n| n.parent) != Some(parent) {
                    inner.detach(*child);
                }
                if let Some(node) = inner.nodes.get_mut(child) {
                    node.parent = Some(parent);
                }
            }
            inner.nodes.get_mut(&parent).unwrap().children = children;
            inner.mark_resync(parent);
            inner.styles_dirty = true;
        }
        self.changed();
    }

    /// Removes a node and its whole subtree, natively too.
    pub fn destroy(&self, id: NodeId) {
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.nodes.contains_key(&id) {
                return;
            }
            inner.detach(id);
            // Remove the subtree's native roots from their native parent now,
            // so the backend sees Remove before Destroy.
            let roots = inner.native_roots(id);
            for root in &roots {
                inner.detach_native(*root);
            }
            let mut subtree = Vec::new();
            inner.collect_subtree(id, &mut subtree);
            for node_id in subtree {
                let node = inner.nodes.remove(&node_id).expect("in subtree");
                if let Some(t) = node.taffy {
                    let _ = inner.taffy.remove(t);
                }
                inner.resync.remove(&node_id);
                inner.windows.retain(|w| *w != node_id);
                inner.focus_orders.remove(&node_id);
                inner.focused.retain(|window, focused| *window != node_id && *focused != node_id);
                if node.kind.is_native() {
                    inner.pending.push(Command::Destroy { id: node_id });
                }
            }
        }
        self.changed();
    }
}

impl Inner {
    /// Sends a prop. Until the node's `Create` has gone out, the prop joins
    /// it, so backends create every widget with its initial props (reactive
    /// ones included): custom widgets and native views can't be created
    /// without theirs.
    pub(super) fn queue_prop(&mut self, id: NodeId, prop: Prop) {
        // A new range may clamp a slider's value: the value follows it.
        let then = match prop {
            Prop::Range { .. } => {
                self.nodes.get(&id).and_then(|n| n.props.iter().find(|p| matches!(p, Prop::Number(_)))).cloned()
            }
            _ => None,
        };
        self.queue_prop_now(id, prop);
        if let Some(number) = then {
            self.queue_prop_now(id, number);
        }
    }

    fn queue_prop_now(&mut self, id: NodeId, prop: Prop) {
        let create = self.pending.iter_mut().rev().find_map(|command| match command {
            Command::Create { id: created, props, .. } if *created == id => Some(props),
            _ => None,
        });
        match create {
            Some(props) => {
                props.retain(|p| p.key() != prop.key());
                props.push(prop);
            }
            None => self.pending.push(Command::SetProp { id, prop }),
        }
    }

    fn mark_resync(&mut self, id: NodeId) {
        if let Some(native) = self.native_ancestor_or_self(id) {
            self.resync.insert(native);
        }
    }

    fn native_ancestor_or_self(&self, id: NodeId) -> Option<NodeId> {
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.nodes.get(&node_id)?;
            if node.kind.is_native() {
                return Some(node_id);
            }
            current = node.parent;
        }
        None
    }

    fn detach(&mut self, child: NodeId) {
        let Some(parent) = self.nodes.get(&child).and_then(|n| n.parent) else { return };
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.children.retain(|c| *c != child);
        }
        self.nodes.get_mut(&child).unwrap().parent = None;
        self.mark_resync(parent);
    }

    /// Removes a native node from its native parent, immediately.
    fn detach_native(&mut self, child: NodeId) {
        let Some(parent) = self.nodes.get(&child).and_then(|n| n.native_parent) else { return };
        self.pending.push(Command::Remove { parent, child });
        // A list's rows and a window's toolbar items and sidebar aren't in
        // its layout box (`layout_list`, `layout_toolbar`).
        let child_taffy = self.nodes[&child].taffy.filter(|_| !in_chrome(self.nodes[&child].kind));
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.native_children.retain(|c| *c != child);
            if let (Some(p), Some(c), false) = (node.taffy, child_taffy, node.kind.has_rows()) {
                let _ = self.taffy.remove_child(p, c);
            }
        }
        self.nodes.get_mut(&child).unwrap().native_parent = None;
    }

    /// The native nodes a subtree contributes to its native parent.
    fn native_roots(&self, id: NodeId) -> Vec<NodeId> {
        let node = &self.nodes[&id];
        if node.kind.is_native() {
            return vec![id];
        }
        node.children.iter().flat_map(|c| self.native_roots(*c)).collect()
    }

    fn flattened_children(&self, id: NodeId) -> Vec<NodeId> {
        let mut children: Vec<NodeId> = self.nodes[&id].children.iter().flat_map(|c| self.native_roots(*c)).collect();
        // A window's toolbar items come after its content, and its sidebar
        // after them.
        children.sort_by_key(|c| match self.nodes[c].kind {
            WidgetKind::ToolbarItem => 1,
            WidgetKind::Sidebar => 2,
            _ => 0,
        });
        children
    }

    fn collect_subtree(&self, id: NodeId, out: &mut Vec<NodeId>) {
        for child in &self.nodes[&id].children {
            self.collect_subtree(*child, out);
        }
        out.push(id);
    }

    pub(super) fn resync_all(&mut self) {
        let dirty = std::mem::take(&mut self.resync);
        for id in dirty {
            if self.nodes.contains_key(&id) {
                self.resync_node(id);
            }
        }
    }

    /// Makes the backend's children of `parent` match the core tree.
    fn resync_node(&mut self, parent: NodeId) {
        let desired = self.flattened_children(parent);
        let current = self.nodes[&parent].native_children.clone();
        for child in current.iter().filter(|c| !desired.contains(c)) {
            self.detach_native(*child);
        }
        let mut working: Vec<NodeId> = self.nodes[&parent].native_children.clone();
        for (index, child) in desired.iter().enumerate() {
            if working.get(index) == Some(child) {
                continue;
            }
            if let Some(pos) = working.iter().position(|c| c == child) {
                working.remove(pos);
                self.pending.push(Command::Remove { parent, child: *child });
            } else if let Some(other) = self.nodes[child].native_parent
                && other != parent
            {
                self.detach_native(*child);
            }
            working.insert(index, *child);
            self.pending.push(Command::Insert { parent, child: *child, index });
        }
        for child in &desired {
            self.nodes.get_mut(child).unwrap().native_parent = Some(parent);
        }
        // Toolbar items are laid out on their own (`layout_toolbar`); the
        // sidebar is the platform's.
        let taffy_children: Vec<_> =
            desired.iter().filter(|c| !in_chrome(self.nodes[*c].kind)).filter_map(|c| self.nodes[c].taffy).collect();
        // A list's rows are laid out on their own (`layout_list`).
        if let Some(t) = self.nodes[&parent].taffy.filter(|_| !self.nodes[&parent].kind.has_rows()) {
            let _ = self.taffy.set_children(t, &taffy_children);
        }
        self.nodes.get_mut(&parent).unwrap().native_children = desired;
    }

    /// Whether a node is a page of a `Tabs`.
    pub(super) fn is_page(&self, id: NodeId) -> bool {
        self.nodes[&id].parent.is_some_and(|p| self.nodes[&p].kind == WidgetKind::Tabs)
    }

    /// Whether a node is a page of a `Tabs` that isn't shown: every other
    /// page than the chosen one.
    pub(super) fn is_hidden_page(&self, id: NodeId) -> bool {
        let Some(tabs) = self.nodes[&id].parent.filter(|p| self.nodes[p].kind == WidgetKind::Tabs) else {
            return false;
        };
        let tabs = &self.nodes[&tabs];
        let shown = crate::find_prop!(tabs.props, SelectedIndex).flatten();
        shown.and_then(|i| tabs.children.get(i)) != Some(&id)
    }
}
