use std::fmt;

use mitsuami_core::{A11yNode, Role};

/// How to find a node: by what users and assistive technology perceive.
#[derive(Clone, Debug, PartialEq)]
pub enum Query {
    /// A role, optionally with an exact accessible name.
    Role(Role, Option<String>),
    /// Visible text: a text node, or a control's caption.
    Text(String),
    /// Accessible name, any role. Use it for form fields.
    Label(String),
    /// Last resort: a `test_id` set in the view.
    TestId(String),
}

pub fn by_role(role: Role, name: impl Into<String>) -> Query {
    Query::Role(role, Some(name.into()))
}

pub fn by_text(text: impl Into<String>) -> Query {
    Query::Text(text.into())
}

pub fn by_label(label: impl Into<String>) -> Query {
    Query::Label(label.into())
}

pub fn by_test_id(id: impl Into<String>) -> Query {
    Query::TestId(id.into())
}

/// Text as people see it: without the invisible marks Fluent isolates
/// the values in a message with (U+2066 to U+2069), which keep a
/// right-to-left name in a left-to-right sentence from reordering it.
pub fn visible_text(text: &str) -> String {
    text.chars().filter(|c| !('\u{2066}'..='\u{2069}').contains(c)).collect()
}

fn same_text(name: Option<&String>, text: &str) -> bool {
    name.is_some_and(|name| name == text || visible_text(name) == visible_text(text))
}

impl Query {
    pub fn matches(&self, node: &A11yNode) -> bool {
        match self {
            Query::Role(role, name) => {
                node.role == *role && name.as_ref().is_none_or(|n| same_text(node.name.as_ref(), n))
            }
            Query::Text(text) => {
                matches!(node.role, Role::StaticText | Role::Heading | Role::Button | Role::Checkbox | Role::Switch)
                    && same_text(node.name.as_ref(), text)
            }
            Query::Label(label) => node.role != Role::StaticText && same_text(node.name.as_ref(), label),
            Query::TestId(id) => node.test_id.as_ref() == Some(id),
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Query::Role(role, Some(name)) => write!(f, "role {role:?} named {name:?}"),
            Query::Role(role, None) => write!(f, "role {role:?}"),
            Query::Text(text) => write!(f, "text {text:?}"),
            Query::Label(label) => write!(f, "label {label:?}"),
            Query::TestId(id) => write!(f, "test id {id:?}"),
        }
    }
}
