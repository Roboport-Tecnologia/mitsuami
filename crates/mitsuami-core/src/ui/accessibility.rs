//! The computed accessibility tree.

use crate::a11y::{A11yNode, Role};
use crate::geometry::{Point, Rect};
use crate::widget::{NodeId, RowKey, WidgetKind};

use super::{Inner, Ui};

impl Ui {
    /// The computed accessibility tree of a window.
    pub fn a11y_tree(&self, window: NodeId) -> Option<A11yNode> {
        let inner = self.inner.borrow();
        let mut nodes = inner.a11y(window, Point::ZERO);
        (nodes.len() == 1).then(|| nodes.remove(0))
    }
}

impl Inner {
    fn text_of(&self, id: NodeId) -> Option<String> {
        let node = self.nodes.get(&id)?;
        crate::find_prop!(node.props, Text).or_else(|| crate::find_prop!(node.props, Label))
    }

    fn a11y(&self, id: NodeId, parent_origin: Point) -> Vec<A11yNode> {
        let node = &self.nodes[&id];
        // Custom widgets bring their own semantics; the app's overrides win.
        let a11y = match node.custom() {
            Some(custom) => custom.a11y().overridden_by(&node.a11y),
            None => node.a11y.clone(),
        };
        if a11y.hidden || node.style.is_hidden() || self.is_hidden_page(id) {
            return Vec::new();
        }
        let frame =
            if node.kind == WidgetKind::Window { node.frame } else { self.placed_frame(id).offset(parent_origin) };
        let origin = Inner::child_origin(node, frame);
        // A window's sidebar reads first, as it's on the leading side.
        let mut native_children = node.native_children.clone();
        native_children.sort_by_key(|c| self.nodes[c].kind != WidgetKind::Sidebar);
        let mut children: Vec<A11yNode> = native_children.iter().flat_map(|c| self.a11y(*c, origin)).collect();
        if node.kind == WidgetKind::Sidebar {
            children = self.sidebar_items(id, frame);
        }
        // A tab view's tabs come before the page shown.
        if node.kind == WidgetKind::Tabs {
            children.splice(0..0, self.tabs(id, frame));
        }
        if node.kind == WidgetKind::RadioGroup {
            children = self.radio_buttons(id, frame);
        }
        // A table's headers, then its cells in rows.
        if node.kind == WidgetKind::Table {
            children = self.table_children(id, frame, children);
        }

        let labelled = a11y.label.is_some() || a11y.labelled_by.is_some();
        let row = crate::find_prop!(node.props, Row);
        let cell = crate::find_prop!(node.props, Cell);
        let role = a11y.role.unwrap_or(match node.kind {
            WidgetKind::Window => Role::Window,
            WidgetKind::Container if row.is_some() => Role::ListItem,
            WidgetKind::Container if cell.is_some() => Role::Cell,
            WidgetKind::Container if labelled => Role::Group,
            WidgetKind::Container | WidgetKind::ToolbarItem | WidgetKind::Fragment => Role::None,
            WidgetKind::ScrollView => Role::ScrollArea,
            WidgetKind::List | WidgetKind::Sidebar => Role::List,
            WidgetKind::Table => Role::Table,
            WidgetKind::Tabs => Role::TabGroup,
            // Named by its heading, as a fieldset by its legend.
            WidgetKind::Group => Role::Group,
            WidgetKind::Text => Role::StaticText,
            WidgetKind::Button => Role::Button,
            WidgetKind::ToggleButton => Role::ToggleButton,
            WidgetKind::MenuButton => Role::MenuButton,
            // A text field that hides its text, as every platform exposes
            // one (AppKit's secure subrole, Qt's and UIA's password flag).
            WidgetKind::TextInput | WidgetKind::PasswordInput => Role::TextField,
            WidgetKind::SearchInput => Role::SearchField,
            WidgetKind::TextArea => Role::TextArea,
            WidgetKind::Checkbox => Role::Checkbox,
            WidgetKind::Switch => Role::Switch,
            WidgetKind::Select => Role::ComboBox,
            WidgetKind::RadioGroup => Role::RadioGroup,
            WidgetKind::Slider => Role::Slider,
            WidgetKind::NumberInput => Role::SpinButton,
            // A spinner reads as a progress bar without a value, as in ARIA.
            WidgetKind::Progress | WidgetKind::Spinner => Role::ProgressBar,
            WidgetKind::Separator => Role::Separator,
            // What the app draws there is a picture to assistive technology.
            WidgetKind::Image | WidgetKind::Icon | WidgetKind::FileIcon | WidgetKind::GpuSurface => Role::Image,
            WidgetKind::Custom(_) | WidgetKind::Native => Role::Group,
        });
        if role == Role::None {
            return children;
        }
        let props = &node.props;
        let name =
            a11y.label.clone().or_else(|| a11y.labelled_by.and_then(|l| self.text_of(l))).or_else(|| match node.kind {
                WidgetKind::Window => crate::find_prop!(props, Title),
                WidgetKind::Group => crate::find_prop!(props, Title).filter(|t| !t.is_empty()),
                WidgetKind::Text => crate::find_prop!(props, Text),
                WidgetKind::Button
                | WidgetKind::ToggleButton
                | WidgetKind::MenuButton
                | WidgetKind::Checkbox
                | WidgetKind::Switch
                | WidgetKind::Select
                | WidgetKind::RadioGroup
                | WidgetKind::Tabs
                | WidgetKind::Slider
                | WidgetKind::NumberInput
                | WidgetKind::Progress
                | WidgetKind::Spinner
                | WidgetKind::Image
                | WidgetKind::Icon
                | WidgetKind::FileIcon
                | WidgetKind::GpuSurface => crate::find_prop!(props, Label),
                WidgetKind::TextInput | WidgetKind::PasswordInput | WidgetKind::SearchInput | WidgetKind::TextArea => {
                    crate::find_prop!(props, Placeholder)
                }
                // Rows and cells read as their text, as screen readers read
                // native rows and cells.
                WidgetKind::Container if row.is_some() || cell.is_some() => text_in(&children),
                _ => None,
            });
        let selected = row.map(|key| {
            let list = node.native_parent.and_then(|p| self.nodes.get(&p));
            list.and_then(|l| crate::find_prop!(l.props, Selected)).is_some_and(|s| s.contains(&key))
        });
        vec![A11yNode {
            id,
            role,
            name,
            // A tooltip is read as the description, as every platform
            // reads one, unless the app gave its own.
            description: a11y.description.or_else(|| crate::find_prop!(props, Tooltip).filter(|t| !t.is_empty())),
            value: match node.kind {
                WidgetKind::TextInput | WidgetKind::SearchInput | WidgetKind::TextArea => {
                    Some(crate::find_prop!(props, Value).unwrap_or_default())
                }
                // Never read out.
                WidgetKind::PasswordInput => None,
                // The chosen option; empty without options.
                WidgetKind::Select => {
                    let options = crate::find_prop!(props, Options).unwrap_or_default();
                    let chosen = crate::find_prop!(props, SelectedIndex).flatten();
                    Some(chosen.and_then(|i| options.get(i).cloned()).unwrap_or_default())
                }
                WidgetKind::Slider | WidgetKind::NumberInput => crate::find_prop!(props, Number).map(|n| n.to_string()),
                // As screen readers read progress bars.
                WidgetKind::Progress => {
                    crate::find_prop!(props, Progress).flatten().map(|f| format!("{}%", (f * 100.0).round()))
                }
                _ => a11y.value,
            },
            checked: match node.kind {
                WidgetKind::Checkbox | WidgetKind::Switch | WidgetKind::ToggleButton => {
                    Some(crate::find_prop!(props, Checked).unwrap_or(false))
                }
                _ => None,
            },
            mixed: node.kind == WidgetKind::Checkbox && crate::find_prop!(props, Mixed) == Some(true),
            read_only: crate::find_prop!(props, ReadOnly) == Some(true),
            password: node.kind == WidgetKind::PasswordInput,
            selected,
            enabled: crate::find_prop!(props, Enabled).unwrap_or(true),
            test_id: node.test_id.clone(),
            frame,
            children,
        }]
    }

    /// A tab view's tabs, which are its data, not nodes: named by their
    /// titles, the shown page's selected. They stand for the tab view,
    /// where assistive technology acts on them.
    fn tabs(&self, id: NodeId, frame: Rect) -> Vec<A11yNode> {
        let props = &self.nodes[&id].props;
        let shown = crate::find_prop!(props, SelectedIndex).flatten();
        let titles = crate::find_prop!(props, TabTitles).unwrap_or_default();
        titles
            .into_iter()
            .enumerate()
            .map(|(index, title)| A11yNode {
                id,
                role: Role::Tab,
                name: Some(title),
                description: None,
                value: None,
                checked: None,
                mixed: false,
                read_only: false,
                password: false,
                selected: Some(shown == Some(index)),
                enabled: true,
                test_id: None,
                frame,
                children: Vec::new(),
            })
            .collect()
    }

    /// A radio group's buttons, which are its data, not nodes: named by
    /// their options, the chosen one's checked. They stand for the group,
    /// where assistive technology acts on them.
    fn radio_buttons(&self, id: NodeId, frame: Rect) -> Vec<A11yNode> {
        let props = &self.nodes[&id].props;
        let chosen = crate::find_prop!(props, SelectedIndex).flatten();
        let enabled = crate::find_prop!(props, Enabled).unwrap_or(true);
        let options = crate::find_prop!(props, Options).unwrap_or_default();
        options
            .into_iter()
            .enumerate()
            .map(|(index, option)| A11yNode {
                id,
                role: Role::RadioButton,
                name: Some(option),
                description: None,
                value: None,
                checked: Some(chosen == Some(index)),
                mixed: false,
                read_only: false,
                password: false,
                selected: None,
                enabled,
                test_id: None,
                frame,
                children: Vec::new(),
            })
            .collect()
    }

    /// A table's column headers, which are its data, not nodes (named by
    /// their titles, the sorted one's value its order), then its rows:
    /// its cells, grouped by row. A row stands for its first cell, which
    /// assistive technology selects the row through.
    fn table_children(&self, id: NodeId, frame: Rect, cells: Vec<A11yNode>) -> Vec<A11yNode> {
        let props = &self.nodes[&id].props;
        let sort = crate::find_prop!(props, Sort).flatten();
        let selected = crate::find_prop!(props, Selected).unwrap_or_default();
        let node = |id, role, name: Option<String>, value, selected, frame, children| A11yNode {
            id,
            role,
            name,
            description: None,
            value,
            checked: None,
            mixed: false,
            read_only: false,
            password: false,
            selected,
            enabled: true,
            test_id: None,
            frame,
            children,
        };
        let columns = crate::find_prop!(props, Columns).unwrap_or_default();
        let mut out: Vec<A11yNode> = columns
            .into_iter()
            .enumerate()
            .map(|(index, column)| {
                let order = sort.filter(|s| s.column == index).map(|s| format!("{:?}", s.order));
                node(id, Role::ColumnHeader, Some(column.title), order, None, frame, Vec::new())
            })
            .collect();
        let mut rows: Vec<(RowKey, Vec<A11yNode>)> = Vec::new();
        for cell in cells {
            let Some(key) = crate::find_prop!(self.nodes[&cell.id].props, Cell).map(|c| c.row) else { continue };
            match rows.last_mut() {
                Some((last, cells)) if *last == key => cells.push(cell),
                _ => rows.push((key, vec![cell])),
            }
        }
        out.extend(rows.into_iter().map(|(key, cells)| {
            let frame = cells.iter().map(|c| c.frame).reduce(|a, b| a.union(&b)).unwrap_or_default();
            let name = cells.iter().filter_map(|c| c.name.as_deref()).collect::<Vec<_>>().join(" ");
            let name = (!name.is_empty()).then_some(name);
            node(cells[0].id, Role::Row, name, None, Some(selected.contains(&key)), frame, cells)
        }));
        out
    }

    /// A sidebar's items, which are its data, not nodes: list items named
    /// by their titles, under a heading for each titled section. They
    /// stand for the sidebar, where assistive technology acts on them.
    fn sidebar_items(&self, id: NodeId, frame: Rect) -> Vec<A11yNode> {
        let props = &self.nodes[&id].props;
        let selected = crate::find_prop!(props, SelectedIndex).flatten();
        let node = |role, name: &str, selected| A11yNode {
            id,
            role,
            name: Some(name.to_owned()),
            description: None,
            value: None,
            checked: None,
            mixed: false,
            read_only: false,
            password: false,
            selected,
            enabled: true,
            test_id: None,
            frame,
            children: Vec::new(),
        };
        let mut out = Vec::new();
        let mut index = 0;
        for section in crate::find_prop!(props, Sections).unwrap_or_default() {
            out.extend(section.title.as_deref().map(|title| node(Role::Heading, title, None)));
            for item in &section.items {
                out.push(node(Role::ListItem, &item.title, Some(selected == Some(index))));
                index += 1;
            }
        }
        out
    }
}

/// The text in these nodes, joined, as screen readers read a row or cell.
fn text_in(children: &[A11yNode]) -> Option<String> {
    let texts: Vec<&str> = children
        .iter()
        .flat_map(|c| c.walk())
        .filter(|n| n.role == Role::StaticText)
        .filter_map(|n| n.name.as_deref())
        .collect();
    (!texts.is_empty()).then(|| texts.join(" "))
}
