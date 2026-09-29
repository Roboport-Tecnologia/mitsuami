//! The mirror's basics: nodes, events, props, focus and protocol violations.

use mitsuami_core::{Command, NodeId, Prop, UiEvent, WidgetKind, find_prop};

use super::{HeadlessNode, State};

impl State {
    pub(super) fn node(&mut self, id: NodeId, command: &Command) -> &mut HeadlessNode {
        match self.nodes.get_mut(&id) {
            Some(node) => node,
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    pub(super) fn emit(&self, id: NodeId, event: UiEvent) {
        if let Some(events) = &self.events {
            events.emit(id, event);
        }
    }

    pub(super) fn set_prop(&mut self, id: NodeId, prop: Prop) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop);
        }
    }

    /// The next enabled control after `id` in its window's focus order (as
    /// sent by the core), wrapping around. Every control takes focus, as with
    /// full keyboard access.
    pub(super) fn next_focusable(&self, id: NodeId) -> Option<NodeId> {
        let order = self.focus_orders.values().find(|order| order.contains(&id))?;
        let start = order.iter().position(|n| *n == id)?;
        (1..order.len())
            .map(|step| order[(start + step) % order.len()])
            .find(|candidate| self.nodes.get(candidate).is_some_and(|n| find_prop!(n.props, Enabled) != Some(false)))
    }

    pub(super) fn focus(&mut self, id: NodeId) {
        if self.focused == Some(id) {
            return;
        }
        if let Some(previous) = self.focused.replace(id) {
            self.emit(previous, UiEvent::FocusOut);
            // A keyboard grab lasts while its surface has focus.
            self.end_lock(previous, Prop::KeyboardGrab(false));
        }
        self.emit(id, UiEvent::FocusIn);
    }

    /// Ends a GPU surface's pointer lock or keyboard grab (`off` says
    /// which), if it has it, as platforms end them.
    pub(super) fn end_lock(&mut self, id: NodeId, off: Prop) {
        let Some(node) = self.nodes.get(&id) else { return };
        if !node.props.iter().any(|p| p.key() == off.key() && *p != off) {
            return;
        }
        let event =
            if matches!(off, Prop::PointerLock(_)) { UiEvent::PointerLockEnded } else { UiEvent::KeyboardGrabEnded };
        self.set_prop(id, off);
        self.emit(id, event);
    }

    /// The window a node is in (or is).
    pub(super) fn window_of(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            let node = self.nodes.get(&id)?;
            if node.kind == WidgetKind::Window {
                return Some(id);
            }
            id = node.parent?;
        }
    }
}

pub(super) fn violation(command: &Command, problem: &str) -> ! {
    panic!("headless backend: protocol violation in {command:?}: {problem}")
}
