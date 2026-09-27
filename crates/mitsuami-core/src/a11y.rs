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
    TextField,
    Checkbox,
    Switch,
    /// A button that pops up a list of options to choose one from (a
    /// `Select`). Its value is the chosen option.
    ComboBox,
    /// How far along a task is. Its value is the percentage, unless it
    /// isn't known.
    ProgressBar,
    Image,
    Slider,
    /// A field for a number with buttons that step it (a `NumberInput`).
    /// Its value is the number.
    SpinButton,
    List,
    ListItem,
    ScrollArea,
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
    /// Replace a text field's text, choose the `Select` option with this
    /// text, or move a `Slider` or `NumberInput` to this number.
    SetValue(String),
    /// Step an adjustable control (a slider, a spin button) up or down.
    Increment,
    Decrement,
    ScrollIntoView,
    /// Select a `List` row (in a single-selection list, instead of the
    /// selected one; in a multiple-selection list, as the only one).
    Select,
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
    /// Text fields: shown, but not editable.
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
        let mut out = vec![self];
        for child in &self.children {
            out.extend(child.walk());
        }
        out
    }
}
