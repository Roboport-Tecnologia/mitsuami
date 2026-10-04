//! Applying commands, checking each against the protocol.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use mitsuami_core::a11y::A11yProps;
use mitsuami_core::{
    Command, Insets, NodeId, Point, Prop, Rect, RowKey, SelectionMode, SurfaceHandle, UiEvent, WidgetKind, find_prop,
};

use super::state::violation;
use super::{HeadlessBackend, HeadlessNode, HeadlessSurface, State};

impl HeadlessBackend {
    pub(super) fn apply_batch(&mut self, batch: &[Command]) {
        let mut state = self.state.borrow_mut();
        // The nodes the batch named, and their parents: the lists to place
        // again and the nodes to check after it are among them.
        let mut touched = BTreeSet::new();
        // Children removed or destroyed, taken out of their parents'
        // lists together once a run of them ends: one at a time scanned
        // the siblings for each (`unlink`).
        let mut unlinked: BTreeMap<NodeId, HashSet<NodeId>> = BTreeMap::new();
        for command in batch {
            if state.recording {
                state.log.push(command.clone());
            }
            if !matches!(command, Command::Remove { .. } | Command::Destroy { .. }) {
                state.unlink(&mut unlinked);
            }
            state.touch(command, &mut touched);
            match command {
                Command::Create { id, kind, props } => {
                    if state.nodes.contains_key(id) {
                        violation(command, "node already exists");
                    }
                    if *kind == WidgetKind::Fragment {
                        violation(command, "fragments are core-only");
                    }
                    if matches!(kind, WidgetKind::Custom(_)) && find_prop!(props, Custom).is_none() {
                        violation(command, "custom widgets are created with their Prop::Custom");
                    }
                    if *kind == WidgetKind::Native && find_prop!(props, Native).is_none() {
                        violation(command, "native views are created with their Prop::Native");
                    }
                    state.nodes.insert(
                        *id,
                        HeadlessNode {
                            kind: *kind,
                            props: props.clone(),
                            a11y: A11yProps::default(),
                            frame: Rect::ZERO,
                            parent: None,
                            children: Vec::new(),
                            scroll_offset: Point::ZERO,
                            viewport_insets: Insets::ZERO,
                            shown: BTreeSet::new(),
                            placed: Vec::new(),
                            placed_index: Default::default(),
                            heights: BTreeMap::new(),
                            column_widths: Vec::new(),
                            surface: None,
                            windowed: None,
                            restored: None,
                        },
                    );
                    if find_prop!(props, Maximized) == Some(true) {
                        state.maximize(*id, true);
                    }
                    if *kind == WidgetKind::GpuSurface {
                        let surface = SurfaceHandle::new(HeadlessSurface);
                        state.nodes.get_mut(id).unwrap().surface = Some(surface.clone());
                        state.emit(*id, UiEvent::SurfaceReady(surface));
                        if find_prop!(props, KeyboardGrab) == Some(true) {
                            state.focus(*id);
                        }
                    }
                }
                Command::SetProp { id, prop } => {
                    state.node(*id, command);
                    // New text puts the caret at its end.
                    if let Prop::Value(text) = prop
                        && state.focused == Some(*id)
                        && find_prop!(state.nodes[id].props, Value).as_ref() != Some(text)
                    {
                        state.selection = None;
                    }
                    state.set_prop(*id, prop.clone());
                    // A grab focuses its surface.
                    if *prop == Prop::KeyboardGrab(true) {
                        state.focus(*id);
                    }
                    match prop {
                        Prop::FullScreen(on) => state.fill_screen(*id, *on),
                        Prop::Maximized(on) => state.maximize(*id, *on),
                        Prop::MinSize(_) => {
                            let size = state.nodes[id].frame.size;
                            state.resize(*id, size);
                        }
                        _ => {}
                    }
                    // Like native lists, removing rows deselects them.
                    if let Prop::Rows(rows) = prop {
                        let selected = find_prop!(state.nodes[id].props, Selected).unwrap_or_default();
                        let rows: BTreeSet<RowKey> =
                            if selected.is_empty() { BTreeSet::new() } else { rows.iter().copied().collect() };
                        let kept: Vec<RowKey> = selected.iter().copied().filter(|k| rows.contains(k)).collect();
                        if kept != selected {
                            state.select(*id, kept);
                        }
                    }
                    // A mode that holds fewer rows lets go of the others:
                    // none, or all but the first.
                    if let Prop::SelectionMode(mode) = prop {
                        let selected = find_prop!(state.nodes[id].props, Selected).unwrap_or_default();
                        let kept = match mode {
                            SelectionMode::None => Vec::new(),
                            SelectionMode::Single => selected.iter().take(1).copied().collect(),
                            SelectionMode::Multiple => selected.clone(),
                        };
                        if kept != selected {
                            state.select(*id, kept);
                        }
                    }
                }
                Command::Insert { parent, child, index } => {
                    if let Some(p) = state.node(*child, command).parent {
                        violation(command, &format!("child is still attached to {p}"));
                    }
                    let parent_node = state.node(*parent, command);
                    if parent_node.kind == WidgetKind::ScrollView && !parent_node.children.is_empty() {
                        violation(command, "a ScrollView has a single native child (its content)");
                    }
                    let siblings = &mut state.node(*parent, command).children;
                    if *index > siblings.len() {
                        violation(command, &format!("index out of bounds (len {})", siblings.len()));
                    }
                    siblings.insert(*index, *child);
                    state.node(*child, command).parent = Some(*parent);
                }
                Command::Remove { parent, child } => {
                    state.node(*parent, command);
                    // Both sides of a link are kept together, so the child
                    // says whether it's in the parent's list.
                    if state.node(*child, command).parent != Some(*parent) {
                        violation(command, "not a child of this parent");
                    }
                    state.node(*child, command).parent = None;
                    unlinked.entry(*parent).or_default().insert(*child);
                }
                Command::Destroy { id } => {
                    let node = state.nodes.remove(id).unwrap_or_else(|| violation(command, "node does not exist"));
                    if let Some(parent) = node.parent.filter(|p| state.nodes.contains_key(p)) {
                        unlinked.entry(parent).or_default().insert(*id);
                    }
                    for child in node.children {
                        if let Some(child) = state.nodes.get_mut(&child) {
                            child.parent = None;
                        }
                    }
                    if state.focused == Some(*id) {
                        state.focused = None;
                    }
                    state.focus_orders.remove(id);
                }
                Command::SetFrame { id, frame } => {
                    let node = state.node(*id, command);
                    if node.kind == WidgetKind::Window {
                        violation(command, "window frames belong to the platform");
                    }
                    if node.kind == WidgetKind::Sidebar {
                        violation(command, "a sidebar's frame belongs to the platform");
                    }
                    node.frame = *frame;
                    state.size_surface(*id);
                }
                Command::SetA11y { id, a11y } => state.node(*id, command).a11y = a11y.clone(),
                Command::SetWindowSize { id, size } => {
                    state.node(*id, command);
                    state.resize(*id, *size);
                }
                Command::SetFocusOrder { window, order } => {
                    if state.node(*window, command).kind != WidgetKind::Window {
                        violation(command, "not a window");
                    }
                    for id in order {
                        state.node(*id, command);
                    }
                    state.focus_orders.insert(*window, order.clone());
                }
                Command::ScrollTo { id, offset } => {
                    if !state.node(*id, command).kind.scrolls() {
                        violation(command, "not a ScrollView, List or Table");
                    }
                    state.scroll(*id, *offset);
                }
                Command::Focus { id } => {
                    state.node(*id, command);
                    state.focus(*id);
                }
                Command::SelectText { id, range } => {
                    let node = state.node(*id, command);
                    if !matches!(
                        node.kind,
                        WidgetKind::TextInput
                            | WidgetKind::PasswordInput
                            | WidgetKind::SearchInput
                            | WidgetKind::TextArea
                    ) {
                        violation(command, "not a text field or text area");
                    }
                    let len = find_prop!(node.props, Value).map_or(0, |v| v.chars().count());
                    if range.start > range.end || range.end > len {
                        violation(command, "a selection past the text");
                    }
                    if state.focused != Some(*id) {
                        violation(command, "a selection of a field without focus");
                    }
                    state.selection = Some(range.clone());
                }
                Command::ScrollToRow { id, row } => {
                    if !state.node(*id, command).kind.has_rows() {
                        violation(command, "not a List or Table");
                    }
                    state.place_rows(*id);
                    state.reveal(*id, *row);
                }
            }
        }
        state.unlink(&mut unlinked);
        touched.retain(|id| state.nodes.contains_key(id));
        if let Some(last) = batch.last() {
            // New data, sizes or rows: what's in view may have changed. Only
            // in the lists the batch touched: the others are as they were.
            let lists: Vec<NodeId> = touched.iter().copied().filter(|id| state.nodes[id].kind.has_rows()).collect();
            for list in &lists {
                if state.nodes[list].kind == WidgetKind::Table {
                    state.size_columns(*list);
                }
                state.place_rows(*list);
            }
            state.check_lists(last, &touched);
            state.check_toolbars(last, &touched);
            state.check_tabs(last, &touched);
            for list in lists {
                let offset = state.nodes[&list].scroll_offset;
                let y = state.clamp_list(list, offset.y);
                if y != offset.y {
                    state.scroll(list, Point::new(offset.x, y));
                }
                state.show_rows(list);
            }
        }
    }
}

impl State {
    /// Adds the nodes a command names, and their parents (a list, for its
    /// hosts' frames and props), to `touched`.
    fn touch(&self, command: &Command, touched: &mut BTreeSet<NodeId>) {
        let ids = match command {
            Command::Create { id, .. }
            | Command::SetProp { id, .. }
            | Command::Destroy { id }
            | Command::SetFrame { id, .. }
            | Command::ScrollToRow { id, .. } => [Some(*id), None],
            Command::Insert { parent, child, .. } | Command::Remove { parent, child } => [Some(*parent), Some(*child)],
            _ => [None, None],
        };
        for id in ids.into_iter().flatten() {
            touched.insert(id);
            touched.extend(self.nodes.get(&id).and_then(|n| n.parent));
        }
    }

    /// Takes children removed or destroyed out of their parents' lists,
    /// each parent's once.
    fn unlink(&mut self, unlinked: &mut BTreeMap<NodeId, HashSet<NodeId>>) {
        for (parent, gone) in std::mem::take(unlinked) {
            if let Some(node) = self.nodes.get_mut(&parent) {
                node.children.retain(|c| !gone.contains(c));
            }
        }
    }

    /// Checks that toolbar items are in windows, after their content, and
    /// that a window has at most one sidebar, after them, whose selection
    /// is one of its items. Only `touched` nodes can have changed.
    pub(super) fn check_toolbars(&self, command: &Command, touched: &BTreeSet<NodeId>) {
        for (id, node) in touched.iter().map(|id| (id, &self.nodes[id])) {
            let kind = |c: &NodeId| self.nodes[c].kind;
            let sidebars = node.children.iter().filter(|c| kind(c) == WidgetKind::Sidebar).count();
            if sidebars > 0 && node.kind != WidgetKind::Window {
                violation(command, &format!("{id} has a sidebar but isn't a window"));
            }
            if sidebars > 1 {
                violation(command, &format!("window {id} has {sidebars} sidebars"));
            }
            if sidebars == 1 && node.children.last().map(kind) != Some(WidgetKind::Sidebar) {
                violation(command, &format!("window {id}'s sidebar isn't after its content and toolbar items"));
            }
            let children = &node.children[..node.children.len() - sidebars];
            let items = children.iter().filter(|c| kind(c) == WidgetKind::ToolbarItem).count();
            if items > 0 && node.kind != WidgetKind::Window {
                violation(command, &format!("{id} has toolbar items but isn't a window"));
            }
            let content = children.len() - items;
            if children[content..].iter().any(|c| kind(c) != WidgetKind::ToolbarItem) {
                violation(command, &format!("window {id}'s toolbar items aren't after its content"));
            }
            if node.kind == WidgetKind::Sidebar {
                let count: usize =
                    find_prop!(node.props, Sections).unwrap_or_default().iter().map(|s| s.items.len()).sum();
                if find_prop!(node.props, SelectedIndex).flatten().is_some_and(|i| i >= count) {
                    violation(command, &format!("sidebar {id}'s selection isn't one of its {count} items"));
                }
            }
        }
    }

    /// Checks that a tab view's children are page hosts, one per title,
    /// and that it shows one of them (none only without pages).
    pub(super) fn check_tabs(&self, command: &Command, touched: &BTreeSet<NodeId>) {
        for (id, node) in touched.iter().map(|id| (id, &self.nodes[id])) {
            if node.kind != WidgetKind::Tabs {
                continue;
            }
            if node.children.iter().any(|c| self.nodes[c].kind != WidgetKind::Container) {
                violation(command, &format!("tabs {id} has a child that isn't a page host (a Container)"));
            }
            let pages = node.children.len();
            let titles = find_prop!(node.props, TabTitles).unwrap_or_default().len();
            if titles != pages {
                violation(command, &format!("tabs {id} has {pages} pages but {titles} titles"));
            }
            let shown = find_prop!(node.props, SelectedIndex).flatten();
            if shown.is_none_or(|i| i >= pages) != (pages == 0) {
                violation(command, &format!("tabs {id} shows {shown:?} of its {pages} pages"));
            }
        }
    }
}
