//! Accessibility model. Every node carries semantics from day one. The test
//! driver uses the same roles, names and actions as assistive technology.

use crate::NodeId;
use crate::geometry::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Not exposed; children are exposed in its place.
    None,
    Window,
    Group,
    StaticText,
    Heading,
    Button,
    /// A button that stays pressed until it's pressed again (a
    /// `ToggleButton`): AppKit's toggle, GTK's toggle button, ARIA's
    /// pressed button, Qt's and UIA's checkable button. `checked` while
    /// it's pressed.
    ToggleButton,
    /// A button that opens a menu of actions (a `MenuButton`): AppKit's
    /// menu button, Qt's `ButtonMenu`, a button with a menu pop-up on GTK
    /// and in ARIA, UIA's expand-collapse button.
    MenuButton,
    TextField,
    /// A field for search text (a `SearchInput`): AppKit's search field,
    /// GTK's search box, ARIA's searchbox, a text field on Qt and UIA (the
    /// edit in an auto-suggest box). Its value is the text.
    SearchField,
    /// A field for text over many lines (a `TextArea`): AppKit's text
    /// area, a multi-line text field elsewhere (GTK's and Qt's multi-line
    /// state, UIA's multi-line edit). Its value is the text.
    TextArea,
    Checkbox,
    Switch,
    /// Radio buttons, one of which at most is checked (a `RadioGroup`):
    /// its radio buttons.
    RadioGroup,
    /// An option of a `RadioGroup`, named by its text, checked while it's
    /// the one chosen.
    RadioButton,
    /// A button that pops up a list of options to choose one from (a
    /// `Select`). Its value is the chosen option.
    ComboBox,
    /// How far along a task is. Its value is the percentage, unless it
    /// isn't known.
    ProgressBar,
    Image,
    /// A line between groups of content (a `Separator`), which assistive
    /// technology announces or skips as the platform does.
    Separator,
    Slider,
    /// A field for a number with buttons that step it (a `NumberInput`).
    /// Its value is the number.
    SpinButton,
    List,
    ListItem,
    /// Rows of cells under column headers (a `Table`): its headers, then
    /// its rows.
    Table,
    /// A column's header in a `Table`, named by its title. Its value is
    /// the table's sort order while the table is sorted by it.
    ColumnHeader,
    /// A row of a `Table`, named by its cells' text, selected while it's
    /// selected: its cells.
    Row,
    /// A cell of a `Table`'s row, named by its text.
    Cell,
    ScrollArea,
    /// Pages with a tab each, one shown (a `Tabs`): its tabs, then the
    /// shown page's content.
    TabGroup,
    /// A tab of a `TabGroup`, named by its title, selected while its page
    /// shows.
    Tab,
}

/// Overrides and additions to the semantics a widget derives on its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct A11yProps {
    pub role: Option<Role>,
    pub label: Option<String>,
    pub description: Option<String>,
    /// Current value, as read out: "3 of 5", "50%".
    pub value: Option<String>,
    pub labelled_by: Option<NodeId>,
    /// Remove this node and its subtree from the accessibility tree.
    pub hidden: bool,
}

impl A11yProps {
    pub fn new(role: Role) -> A11yProps {
        A11yProps { role: Some(role), ..A11yProps::default() }
    }

    pub fn label(mut self, label: impl Into<String>) -> A11yProps {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> A11yProps {
        self.description = Some(description.into());
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> A11yProps {
        self.value = Some(value.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        *self == A11yProps::default()
    }

    /// These semantics, with every field `overrides` sets taking its place.
    pub fn overridden_by(&self, overrides: &A11yProps) -> A11yProps {
        A11yProps {
            role: overrides.role.or(self.role),
            label: overrides.label.clone().or_else(|| self.label.clone()),
            description: overrides.description.clone().or_else(|| self.description.clone()),
            value: overrides.value.clone().or_else(|| self.value.clone()),
            labelled_by: overrides.labelled_by.or(self.labelled_by),
            hidden: overrides.hidden || self.hidden,
        }
    }
}

/// Something assistive technology (or a test) can ask a control to do.
#[derive(Clone, Debug, PartialEq)]
pub enum A11yAction {
    /// Press / click / toggle, whatever the control's primary action is.
    Activate,
    Focus,
    /// Replace a text field's text, choose the `Select` or `RadioGroup`
    /// option with this text, or move a `Slider` or `NumberInput` to this number.
    SetValue(String),
    /// Step an adjustable control (a slider, a spin button) up or down.
    Increment,
    Decrement,
    ScrollIntoView,
    /// Select a `List` row or a `Table` row (on a row or one of its cells;
    /// in a single-selection list, instead of the selected one; in a
    /// multiple-selection list, as the only one).
    Select,
    /// Press the header of a `Table`'s column, by its index, as assistive
    /// technology presses one: the table sorts by that column, or the
    /// other way round, as the platform does. Only sortable columns.
    PressHeader(usize),
    /// Choose the item of the node's context menu with this id, as
    /// assistive technology does once it has shown the menu. The menu
    /// itself never opens.
    ContextMenuItem(u32),
    /// Choose the item of a `MenuButton`'s menu with this id, as
    /// assistive technology does once the button has shown it. The menu
    /// itself never opens. (`Activate` would open it, so a menu button
    /// doesn't take it.)
    MenuItem(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionError {
    UnknownNode,
    Disabled,
    /// A read-only text field can't be edited.
    ReadOnly,
    Unsupported,
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ActionError::UnknownNode => "the node does not exist",
            ActionError::Disabled => "the control is disabled",
            ActionError::ReadOnly => "the control is read-only",
            ActionError::Unsupported => "the control does not support this action",
        })
    }
}

/// A node of the computed accessibility tree.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yNode {
    pub id: NodeId,
    pub role: Role,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub checked: Option<bool>,
    /// Checkboxes: in the mixed state, whatever `checked` says.
    pub mixed: bool,
    /// Text fields and areas: shown, but not editable.
    pub read_only: bool,
    /// Text fields: the text is hidden, and not in `value`.
    pub password: bool,
    /// List rows: whether the row is selected.
    pub selected: Option<bool>,
    pub enabled: bool,
    pub test_id: Option<String>,
    /// In window coordinates.
    pub frame: Rect,
    pub children: Vec<A11yNode>,
}

impl A11yNode {
    /// Depth-first iteration over this node and all descendants.
    pub fn walk(&self) -> Vec<&A11yNode> {
        // One list, filled from a stack: a list per subtree, appended to
        // its parent's, copied each node once per level above it.
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(node) = stack.pop() {
            out.push(node);
            stack.extend(node.children.iter().rev());
        }
        out
    }
}
