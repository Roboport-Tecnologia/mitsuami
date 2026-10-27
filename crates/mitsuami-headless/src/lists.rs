//! Lists, tables and trees: placing, showing, selecting and scrolling rows,
//! sizing tables' columns, opening and closing trees' rows, and checking
//! row and cell hosts.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use mitsuami_core::{
    CellKey, Command, EventValue, NodeId, Point, Prop, Rect, RowKey, Size, TreeRow, UiEvent, WidgetKind, find_prop,
};

use super::metrics::{COLUMN_SPACING, COLUMN_WIDTH, TABLE_HEADER_HEIGHT, TABLE_ROW_HEIGHT, TREE_INDENT};
use super::state::violation;
use super::{HeadlessNode, Placed, State};

/// A list's rows, or a tree's that show, looked at in place: a copy of ten
/// thousand keys per batch adds up.
fn row_keys(node: &HeadlessNode) -> Box<dyn Iterator<Item = RowKey> + '_> {
    match node.props.iter().find(|p| matches!(p, Prop::Rows(_) | Prop::TreeRows(_))) {
        Some(Prop::Rows(rows)) => Box::new(rows.iter().copied()),
        Some(Prop::TreeRows(rows)) => Box::new(rows.iter().map(|r| r.row)),
        _ => Box::new(std::iter::empty()),
    }
}

/// A tree's rows that show, looked at in place.
pub(super) fn tree_rows(node: &HeadlessNode) -> &[TreeRow] {
    node.props
        .iter()
        .find_map(|p| match p {
            Prop::TreeRows(rows) => Some(rows.as_slice()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Checks a tree's rows: each after its parent, which opens and is open,
/// one level below it, and right after its parent's other rows, as a
/// native tree shows them.
pub(super) fn check_tree_rows(command: &Command, rows: &[TreeRow]) {
    let mut seen = HashSet::new();
    // The open rows down to the last row, and their depths.
    let mut path: Vec<&TreeRow> = Vec::new();
    for row in rows {
        if !seen.insert(row.row) {
            violation(command, &format!("tree row {:?} shows twice", row.row));
        }
        if row.expanded && !row.expandable {
            violation(command, &format!("tree row {:?} is open but doesn't open", row.row));
        }
        while path.last().is_some_and(|p| Some(p.row) != row.parent) {
            path.pop();
        }
        match (row.parent, path.last()) {
            (None, _) if row.depth != 0 => violation(command, &format!("top tree row {:?} isn't at depth 0", row.row)),
            (None, _) => {}
            (Some(parent), None) => {
                violation(command, &format!("tree row {:?} isn't right after its parent {parent:?}'s rows", row.row))
            }
            (Some(_), Some(parent)) => {
                if !parent.expanded {
                    violation(command, &format!("tree row {:?} shows in a closed row", row.row));
                }
                if row.depth != parent.depth + 1 {
                    violation(command, &format!("tree row {:?} isn't one level below its parent", row.row));
                }
            }
        }
        path.push(row);
    }
}

impl State {
    /// Moves a scroll view, reporting it like a platform would. Lists then
    /// show the rows that came into view.
    pub(super) fn scroll(&mut self, id: NodeId, offset: Point) {
        let node = self.nodes.get_mut(&id).unwrap();
        if node.scroll_offset != offset {
            node.scroll_offset = offset;
            let kind = node.kind;
            self.emit(id, UiEvent::Scrolled(offset));
            if kind.has_rows() {
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

    /// Where a list shows its rows: all of it, or a table's below its
    /// header.
    fn viewport_height(&self, list: NodeId) -> f32 {
        let node = &self.nodes[&list];
        match node.kind {
            WidgetKind::Table => (node.frame.height() - TABLE_HEADER_HEIGHT).max(0.0),
            _ => node.frame.height(),
        }
    }

    /// Sizes a table's columns, reporting the widths they give their cells
    /// when they change: each as wide as the app says, or the default, the
    /// room left shared by the ones that expand, less the room between
    /// cells, unless its cells fill their rows.
    pub(super) fn size_columns(&mut self, table: NodeId) {
        let node = &self.nodes[&table];
        let columns = find_prop!(node.props, Columns).unwrap_or_default();
        let mut widths: Vec<f32> = columns.iter().map(|c| c.width.unwrap_or(COLUMN_WIDTH) + COLUMN_SPACING).collect();
        let used: f32 = widths.iter().sum();
        let expanding = columns.iter().filter(|c| c.expand).count();
        let left = node.frame.width() - used;
        if expanding > 0 && left > 0.0 {
            let share = (left / expanding as f32).floor();
            for (width, column) in widths.iter_mut().zip(&columns) {
                if column.expand {
                    *width += share;
                }
            }
        }
        if widths != node.column_widths {
            let cells = widths.iter().zip(&columns).map(|(w, c)| if c.fills_row { *w } else { w - COLUMN_SPACING });
            let cells = cells.collect();
            self.nodes.get_mut(&table).unwrap().column_widths = widths;
            self.emit(table, UiEvent::ColumnWidths(cells));
        }
    }

    /// Where a table put a cell of this size, in its content: across from
    /// the columns before it (in from its column's edge, unless it fills
    /// its row), below its header and the rows above, centred in its row's
    /// height.
    pub(super) fn cell_frame(&self, table: NodeId, cell: CellKey, size: Size) -> Option<Rect> {
        let row = self.row(table, cell.row)?;
        let node = &self.nodes[&table];
        let fills = find_prop!(node.props, Columns).and_then(|c| c.get(cell.column).map(|c| c.fills_row));
        let inset = if fills == Some(true) { 0.0 } else { COLUMN_SPACING / 2.0 };
        let x: f32 = node.column_widths.iter().take(cell.column).sum::<f32>() + inset;
        let y = TABLE_HEADER_HEIGHT + row.top + ((row.height - size.height) / 2.0).round();
        Some(Rect::new(x, y, size.width, size.height))
    }

    /// Places a list's rows one below the other, as native lists do: rows
    /// measured so far as high as their hosts were (a table's as its
    /// highest cell, and at least its rows' height), the others as high as
    /// the estimate (the app's, or the mean of the rows measured so far, or
    /// two lines of text).
    pub(super) fn place_rows(&mut self, list: NodeId) {
        let node = &self.nodes[&list];
        let rows: HashSet<RowKey> = row_keys(node).collect();
        let fill: Vec<bool> = find_prop!(node.props, Columns).unwrap_or_default().iter().map(|c| c.fills_row).collect();
        let mut measured: BTreeMap<RowKey, f32> = BTreeMap::new();
        // Tables with cells that fill their rows: each row's height without them.
        let mut bare: BTreeMap<RowKey, f32> = BTreeMap::new();
        for host in &node.children {
            let host = &self.nodes[host];
            let (key, height) = match find_prop!(host.props, Cell) {
                Some(cell) => {
                    let row = bare.entry(cell.row).or_insert(TABLE_ROW_HEIGHT);
                    if !fill.get(cell.column).copied().unwrap_or(false) {
                        *row = row.max(host.frame.height());
                    }
                    (cell.row, host.frame.height().max(TABLE_ROW_HEIGHT))
                }
                None => match find_prop!(host.props, Row) {
                    Some(key) => (key, host.frame.height()),
                    None => continue,
                },
            };
            if host.frame.height() > 0.0 {
                let row = measured.entry(key).or_default();
                *row = row.max(height);
            }
        }
        if fill.contains(&true) {
            let node = self.nodes.get_mut(&list).unwrap();
            node.bare_heights.retain(|key, _| bare.contains_key(key));
            let changed: Vec<(RowKey, f32)> =
                bare.into_iter().filter(|(key, h)| node.bare_heights.insert(*key, *h) != Some(*h)).collect();
            for (key, height) in changed {
                self.emit(list, UiEvent::RowHeight(key, height));
            }
        }
        let heights = &mut self.nodes.get_mut(&list).unwrap().heights;
        heights.retain(|key, _| rows.contains(key));
        heights.extend(measured);
        let placed = self.placement(list);
        let node = self.nodes.get_mut(&list).unwrap();
        node.placed_index = placed.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
        node.placed = placed;
        // The list is as high as its rows, as native lists measure: when
        // that changes, it's measured again.
        let header = if node.kind == WidgetKind::Table { TABLE_HEADER_HEIGHT } else { 0.0 };
        let natural = header + node.placed.iter().map(|r| r.height).sum::<f32>();
        if node.natural_height != natural {
            node.natural_height = natural;
            self.emit(list, UiEvent::Remeasure);
        }
    }

    pub(super) fn placement(&self, list: NodeId) -> Vec<Placed> {
        let node = &self.nodes[&list];
        let heights = &node.heights;
        let estimate = find_prop!(node.props, EstimatedRowHeight).unwrap_or_else(|| match heights.len() {
            0 => (self.metrics.font_sizes.body * 2.0).round(),
            n => (heights.values().sum::<f32>() / n as f32).round(),
        });
        let mut top = 0.0;
        row_keys(node)
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
        let (start, end) = (node.scroll_offset.y, node.scroll_offset.y + self.viewport_height(list));
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
        y.clamp(0.0, (total - self.viewport_height(list)).max(0.0))
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
        let (offset, visible) = (node.scroll_offset, self.viewport_height(list));
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

    /// Opens or closes a tree's row as the user does, reporting it. Closing
    /// it drops the rows below it at once, as platforms do; opening it shows
    /// it open, and its children's rows come from the core.
    pub(super) fn expand(&mut self, tree: NodeId, row: RowKey, open: bool) -> Result<(), ()> {
        let rows = tree_rows(&self.nodes[&tree]);
        let Some(at) = rows.iter().position(|r| r.row == row).filter(|i| rows[*i].expandable) else { return Err(()) };
        if rows[at].expanded == open {
            return Ok(());
        }
        let mut rows = rows.to_vec();
        rows[at].expanded = open;
        if !open {
            let depth = rows[at].depth;
            let end = rows[at + 1..].iter().position(|r| r.depth <= depth).map_or(rows.len(), |i| at + 1 + i);
            rows.drain(at + 1..end);
        }
        let kept: HashSet<RowKey> = rows.iter().map(|r| r.row).collect();
        self.set_prop(tree, Prop::TreeRows(rows));
        // Like a native tree, closing a row deselects the rows it hid.
        let selected = find_prop!(self.nodes[&tree].props, Selected).unwrap_or_default();
        let still: Vec<RowKey> = selected.iter().copied().filter(|k| kept.contains(k)).collect();
        if still != selected {
            self.select(tree, still);
        }
        self.place_rows(tree);
        self.show_rows(tree);
        self.emit(tree, UiEvent::RowExpanded(row, open));
        Ok(())
    }

    /// Where a tree's row's content starts.
    pub(super) fn tree_inset(&self, tree: NodeId, row: RowKey) -> f32 {
        tree_rows(&self.nodes[&tree]).iter().find(|r| r.row == row).map_or(0.0, |r| TREE_INDENT.at(r.depth))
    }

    /// Checks what a native list needs of its row hosts, once a batch is
    /// in. Only `touched` lists can have changed.
    pub(super) fn check_lists(&self, command: &Command, touched: &BTreeSet<NodeId>) {
        for (id, node) in touched.iter().map(|id| (id, &self.nodes[id])) {
            if node.kind == WidgetKind::Table {
                self.check_table(command, *id);
            }
            if !matches!(node.kind, WidgetKind::List | WidgetKind::Tree) {
                continue;
            }
            let mut last = None;
            for child in &node.children {
                let child = &self.nodes[child];
                let Some(key) = find_prop!(child.props, Row).filter(|_| child.kind == WidgetKind::Container) else {
                    violation(
                        command,
                        &format!("list {id} has a child that isn't a row host (a Container with a Prop::Row)"),
                    );
                };
                let Some(index) = node.placed_index.get(&key).copied() else {
                    violation(command, &format!("list {id} hosts row {key:?}, which isn't in its Prop::Rows"));
                };
                if last.is_some_and(|last| last >= index) {
                    violation(command, &format!("list {id}'s row hosts aren't in row order"));
                }
                last = Some(index);
            }
        }
    }

    /// Checks a table's cell hosts: each a cell of a row in its data and
    /// one of its columns, in row order, then column order.
    fn check_table(&self, command: &Command, id: NodeId) {
        let node = &self.nodes[&id];
        let columns = find_prop!(node.props, Columns).unwrap_or_default().len();
        let mut last = None;
        for child in &node.children {
            let child = &self.nodes[child];
            let Some(cell) = find_prop!(child.props, Cell).filter(|_| child.kind == WidgetKind::Container) else {
                violation(
                    command,
                    &format!("table {id} has a child that isn't a cell host (a Container with a Prop::Cell)"),
                );
            };
            let Some(index) = node.placed_index.get(&cell.row).copied() else {
                violation(
                    command,
                    &format!("table {id} hosts a cell of row {:?}, which isn't in its Prop::Rows", cell.row),
                );
            };
            if cell.column >= columns {
                violation(command, &format!("table {id} hosts a cell of column {}, of its {columns}", cell.column));
            }
            if last.is_some_and(|last| last >= (index, cell.column)) {
                violation(command, &format!("table {id}'s cell hosts aren't in row, then column order"));
            }
            last = Some((index, cell.column));
        }
    }
}
