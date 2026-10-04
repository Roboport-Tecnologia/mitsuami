//! The tree's structure, and keeping the backend's native children and
//! props in step with it.

use std::collections::{BTreeMap, HashMap, HashSet};

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
            // The parent's children are replaced below, so the ones that go
            // only lose their parent: detaching each would scan them all.
            let staying: HashSet<NodeId> = children.iter().copied().collect();
            let old = std::mem::take(&mut inner.nodes.get_mut(&parent).unwrap().children);
            let mut roots = Vec::new();
            for child in old.iter().filter(|c| !staying.contains(c)) {
                if let Some(node) = inner.nodes.get_mut(child) {
                    node.parent = None;
                    roots.extend(inner.native_roots(*child));
                }
            }
            // And leave their native parent together, so destroying them
            // next (as `For` does) doesn't look for each among the others.
            inner.detach_native_all(&roots);
            // Children coming from other parents leave each together: one
            // at a time scanned its siblings for each.
            let mut moving: BTreeMap<NodeId, HashSet<NodeId>> = BTreeMap::new();
            for child in &children {
                let Some(node) = inner.nodes.get_mut(child) else { continue };
                if let Some(other) = node.parent.filter(|p| *p != parent) {
                    moving.entry(other).or_default().insert(*child);
                }
                node.parent = Some(parent);
            }
            for (other, gone) in moving {
                if let Some(node) = inner.nodes.get_mut(&other) {
                    node.children.retain(|c| !gone.contains(c));
                }
                inner.mark_resync(other);
            }
            inner.nodes.get_mut(&parent).unwrap().children = children;
            inner.mark_resync(parent);
            inner.styles_dirty = true;
        }
        self.changed();
    }

    /// Removes a node and its whole subtree, natively too.
    pub fn destroy(&self, id: NodeId) {
        // Dropped after the borrow: a handler's drop may dispose a scope
        // (`owner_or_node_scope`) whose cleanups use the `Ui`.
        let mut removed = Vec::new();
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.nodes.contains_key(&id) {
                return;
            }
            inner.detach(id);
            inner.focus_dirty = true;
            // Remove the subtree's native roots from their native parent now,
            // so the backend sees Remove before Destroy.
            let roots = inner.native_roots(id);
            inner.detach_native_all(&roots);
            let mut subtree = Vec::new();
            inner.collect_subtree(id, &mut subtree);
            // Layout boxes go parents first: Taffy takes a box out of its
            // parent's children by scanning them, unless the parent is gone.
            for node_id in subtree.iter().rev() {
                if let Some(t) = inner.nodes[node_id].taffy {
                    let _ = inner.taffy.remove(t);
                }
            }
            for node_id in subtree {
                let node = inner.nodes.remove(&node_id).expect("in subtree");
                inner.resync.remove(&node_id);
                if node.kind == WidgetKind::Window {
                    inner.windows.retain(|w| *w != node_id);
                    inner.destroyed_windows.push(node_id);
                }
                inner.focus_orders.remove(&node_id);
                inner.focused.retain(|window, focused| *window != node_id && *focused != node_id);
                if node.kind.is_native() {
                    inner.pending.push(Command::Destroy { id: node_id });
                }
                removed.push(node);
            }
        }
        drop(removed);
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
        if !self.created.contains(&id) {
            self.pending.push(Command::SetProp { id, prop });
            return;
        }
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

    /// Removes native nodes from their native parents, immediately. Each
    /// parent's children are scanned once, however many of them go.
    fn detach_native_all(&mut self, children: &[NodeId]) {
        let mut gone: BTreeMap<NodeId, HashSet<NodeId>> = BTreeMap::new();
        for &child in children {
            let Some(parent) = self.nodes.get(&child).and_then(|n| n.native_parent) else { continue };
            self.pending.push(Command::Remove { parent, child });
            self.nodes.get_mut(&child).unwrap().native_parent = None;
            gone.entry(parent).or_default().insert(child);
        }
        for (parent, children) in gone {
            let Some(node) = self.nodes.get_mut(&parent) else { continue };
            node.native_children.retain(|c| !children.contains(c));
            // A list's rows and a window's toolbar items and sidebar aren't
            // in its layout box (`layout_list`, `layout_toolbar`).
            let Some(p) = node.taffy.filter(|_| !node.kind.has_rows()) else { continue };
            let boxes: HashSet<taffy::NodeId> = children.iter().filter_map(|c| self.nodes[c].taffy).collect();
            let Ok(mut kept) = self.taffy.children(p) else { continue };
            let before = kept.len();
            kept.retain(|t| !boxes.contains(t));
            if kept.len() != before {
                let _ = self.taffy.set_children(p, &kept);
            }
        }
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
    ///
    /// The children that stay in the same order (the longest run of them)
    /// stay put; the others that stay are removed, and then every child
    /// not in place is inserted, in order, each at its final index. A
    /// reorder costs one pass, and the fewest moves, where moving each
    /// child into place in turn was quadratic for a list reversed.
    fn resync_node(&mut self, parent: NodeId) {
        let desired = self.flattened_children(parent);
        let current = self.nodes[&parent].native_children.clone();
        let staying: HashSet<NodeId> = desired.iter().copied().collect();
        let leaving: Vec<NodeId> = current.iter().copied().filter(|c| !staying.contains(c)).collect();
        self.detach_native_all(&leaving);
        let at: HashMap<NodeId, usize> =
            current.iter().filter(|c| staying.contains(c)).enumerate().map(|(i, c)| (*c, i)).collect();
        let kept = in_order(&desired, &at);
        for child in current.iter().filter(|c| at.contains_key(c) && !kept.contains(c)) {
            self.pending.push(Command::Remove { parent, child: *child });
        }
        // Children coming from other parents leave them together, so each
        // parent's children are scanned once.
        let adopted: Vec<NodeId> = desired
            .iter()
            .copied()
            .filter(|c| !at.contains_key(c) && self.nodes[c].native_parent.is_some_and(|other| other != parent))
            .collect();
        self.detach_native_all(&adopted);
        // Everything before `index` is in place by now: the kept children
        // are in their order, and the others went in in theirs.
        for (index, child) in desired.iter().enumerate() {
            if !kept.contains(child) {
                self.pending.push(Command::Insert { parent, child: *child, index });
            }
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

/// The longest run of `children` already in order: those whose positions
/// in `at` (where they are now) increase, found by patience sorting.
fn in_order(children: &[NodeId], at: &HashMap<NodeId, usize>) -> HashSet<NodeId> {
    let present: Vec<(NodeId, usize)> = children.iter().filter_map(|c| Some((*c, *at.get(c)?))).collect();
    // `tails[n]`: the child ending the best run of length n + 1 so far.
    let mut tails: Vec<usize> = Vec::new();
    let mut previous: Vec<Option<usize>> = Vec::with_capacity(present.len());
    for (i, (_, position)) in present.iter().enumerate() {
        let n = tails.partition_point(|&t| present[t].1 < *position);
        previous.push(n.checked_sub(1).map(|n| tails[n]));
        match tails.get_mut(n) {
            Some(tail) => *tail = i,
            None => tails.push(i),
        }
    }
    let mut kept = HashSet::with_capacity(tails.len());
    let mut next = tails.last().copied();
    while let Some(i) = next {
        kept.insert(present[i].0);
        next = previous[i];
    }
    kept
}
