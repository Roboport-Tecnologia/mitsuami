//! Lists: placing, showing, selecting and scrolling rows, and checking row hosts.

use std::collections::BTreeSet;

use mitsuami_core::{Command, EventValue, NodeId, Point, Prop, RowKey, UiEvent, WidgetKind, find_prop};

use super::state::violation;
use super::{Placed, State};

impl State {
    /// Moves a scroll view, reporting it like a platform would. Lists then
    /// show the rows that came into view.
    pub(super) fn scroll(&mut self, id: NodeId, offset: Point) {
        let node = self.nodes.get_mut(&id).unwrap();
        if node.scroll_offset != offset {
            node.scroll_offset = offset;
            let kind = node.kind;
            self.emit(id, UiEvent::Scrolled(offset));
            if kind == WidgetKind::List {
                self.show_rows(id);
            }
        }
    }

    /// A list's rows and where it placed them.
    pub(super) fn rows(&self, list: NodeId) -> &[Placed] {
        &self.nodes[&list].placed
    }

    pub(super) fn row(&self, list: NodeId, key: RowKey) -> Option<Placed> {
        let node = &self.nodes[&list];
        node.placed_index.get(&key).map(|i| node.placed[*i])
    }

    /// Places a list's rows one below the other, as native lists do: rows
    /// measured so far as high as their hosts were, the others as high as
    /// the estimate (the app's, or the mean of the rows measured so far, or
    /// two lines of text).
    pub(super) fn place_rows(&mut self, list: NodeId) {
        let node = &self.nodes[&list];
        let rows: BTreeSet<RowKey> = find_prop!(node.props, Rows).unwrap_or_default().into_iter().collect();
        let measured: Vec<(RowKey, f32)> = node
            .children
            .iter()
            .filter_map(|host| {
                let host = &self.nodes[host];
                Some((find_prop!(host.props, Row)?, host.frame.height()))
            })
            .filter(|(_, height)| *height > 0.0)
            .collect();
        let heights = &mut self.nodes.get_mut(&list).unwrap().heights;
        heights.retain(|key, _| rows.contains(key));
        heights.extend(measured);
        let placed = self.placement(list);
        let node = self.nodes.get_mut(&list).unwrap();
        node.placed_index = placed.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
        node.placed = placed;
    }

    pub(super) fn placement(&self, list: NodeId) -> Vec<Placed> {
        let node = &self.nodes[&list];
        let heights = &node.heights;
        let estimate = find_prop!(node.props, EstimatedRowHeight).unwrap_or_else(|| match heights.len() {
            0 => (self.metrics.font_sizes.body * 2.0).round(),
            n => (heights.values().sum::<f32>() / n as f32).round(),
        });
        let mut top = 0.0;
        find_prop!(node.props, Rows)
            .unwrap_or_default()
            .into_iter()
            .map(|key| {
                let height = heights.get(&key).copied().unwrap_or(estimate);
                top += height;
                Placed { key, top: top - height, height }
            })
            .collect()
    }

    /// Realises the rows in a list's view and lets go of the others,
    /// reporting both, as native lists do (without prefetching any).
    pub(super) fn show_rows(&mut self, list: NodeId) {
        let node = &self.nodes[&list];
        let (start, end) = (node.scroll_offset.y, node.scroll_offset.y + node.frame.height());
        let shown: BTreeSet<RowKey> = if end > start {
            self.rows(list).iter().filter(|r| r.top < end && r.top + r.height > start).map(|r| r.key).collect()
        } else {
            BTreeSet::new()
        };
        let before = std::mem::replace(&mut self.nodes.get_mut(&list).unwrap().shown, shown.clone());
        for row in before.difference(&shown) {
            self.emit(list, UiEvent::RowHidden(*row));
        }
        for row in shown.difference(&before) {
            self.emit(list, UiEvent::RowShown(*row));
        }
    }

    /// The largest offset of a list: its rows' height less its own.
    pub(super) fn clamp_list(&self, list: NodeId, y: f32) -> f32 {
        let total: f32 = self.rows(list).iter().map(|r| r.height).sum();
        y.clamp(0.0, (total - self.nodes[&list].frame.height()).max(0.0))
    }

    /// Selects rows as the user would, reporting it.
    pub(super) fn select(&mut self, list: NodeId, selection: Vec<RowKey>) {
        let current = find_prop!(self.nodes[&list].props, Selected).unwrap_or_default();
        if current != selection {
            self.set_prop(list, Prop::Selected(selection.clone()));
            self.emit(list, UiEvent::Changed(EventValue::Rows(selection)));
        }
    }

    /// Scrolls a list just enough to show a row, as native lists do when
    /// the keyboard moves the selection.
    pub(super) fn reveal(&mut self, list: NodeId, key: RowKey) {
        let Some(row) = self.row(list, key) else { return };
        let node = &self.nodes[&list];
        let (offset, visible) = (node.scroll_offset, node.frame.height());
        let y = if row.top < offset.y {
            row.top
        } else if row.top + row.height > offset.y + visible {
            row.top + row.height - visible
        } else {
            offset.y
        };
        let y = self.clamp_list(list, y);
        self.scroll(list, Point::new(offset.x, y));
    }

    /// Checks what a native list needs of its row hosts, once a batch is in.
    pub(super) fn check_lists(&self, command: &Command) {
        for (id, node) in &self.nodes {
            if node.kind != WidgetKind::List {
                continue;
            }
            let rows = self.rows(*id);
            let mut last = None;
            for child in &node.children {
                let child = &self.nodes[child];
                let Some(key) = find_prop!(child.props, Row).filter(|_| child.kind == WidgetKind::Container) else {
                    violation(
                        command,
                        &format!("list {id} has a child that isn't a row host (a Container with a Prop::Row)"),
                    );
                };
                let Some(index) = rows.iter().position(|r| r.key == key) else {
                    violation(command, &format!("list {id} hosts row {key:?}, which isn't in its Prop::Rows"));
                };
                if last.is_some_and(|last| last >= index) {
                    violation(command, &format!("list {id}'s row hosts aren't in row order"));
                }
                last = Some(index);
            }
        }
    }
}
