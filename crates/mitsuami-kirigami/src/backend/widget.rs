//! What a node's widget is made of: its items, and what kind of control it is.

use crate::ffi::QmlObject;

use super::{Widget, strip};

impl Widget {
    /// The item that stands for the node: a window's content host.
    pub(super) fn item(&self) -> QmlObject {
        match self {
            Widget::Window { root } => root.host,
            Widget::ToolbarItem { host: i, .. }
            | Widget::Host(i)
            | Widget::Label(i)
            | Widget::Button(i)
            | Widget::MenuButton(i)
            | Widget::Field(i)
            | Widget::Checkbox(i)
            | Widget::Switch(i)
            | Widget::Select(i)
            | Widget::RadioGroup(i)
            | Widget::Slider(i)
            | Widget::NumberInput(i)
            | Widget::Progress(i)
            | Widget::Spinner(i)
            | Widget::Separator(i)
            | Widget::Icon(i)
            | Widget::FileIcon(i)
            | Widget::Image { item: i, .. }
            | Widget::Scroll { view: i, .. }
            | Widget::Custom { item: i, .. }
            | Widget::Drawn { item: i, .. }
            | Widget::Native { item: i, .. }
            | Widget::Tabs { root: i, .. }
            | Widget::Group { root: i, .. }
            | Widget::TextArea { root: i, .. }
            | Widget::Sidebar { page: i, .. } => *i,
            Widget::GpuSurface(surface) => surface.item,
            Widget::List(list) => list.root,
        }
    }

    /// Made from our templates, which show a tooltip (`qml::a11y`). A
    /// window's host isn't: a window takes no tooltip.
    pub(super) fn has_tooltip(&self) -> bool {
        !matches!(
            self,
            Widget::Window { .. }
                | Widget::Custom { .. }
                | Widget::Drawn { .. }
                | Widget::Native { .. }
                | Widget::Sidebar { .. }
        )
    }

    /// Made from our templates, which show a context menu
    /// (`qml::CONTEXT_MENU`), except text fields and areas: they keep KDE's
    /// own, with Cut, Copy and Paste, as a field's own menu wins on every
    /// platform.
    pub(super) fn shows_context_menu(&self) -> bool {
        self.has_tooltip() && !matches!(self, Widget::Field(_) | Widget::TextArea { .. })
    }

    /// The item that takes keyboard focus and input: a list's list view.
    pub(super) fn input_item(&self) -> QmlObject {
        match self {
            Widget::Scroll { flickable, .. } => *flickable,
            Widget::List(list) => list.view,
            Widget::GpuSurface(surface) => surface.input,
            Widget::Sidebar { page, .. } => page.child("mitsuamiSidebarList").unwrap_or(*page),
            Widget::TextArea { area, .. } => *area,
            // Its bar, which hands focus to its selected tab.
            Widget::Tabs { root, .. } => strip(*root),
            widget => widget.item(),
        }
    }

    /// Where children go: the host, the scrolled content, or a tab view's
    /// page area.
    pub(super) fn content(&self) -> QmlObject {
        match self {
            Widget::Scroll { flickable, .. } => flickable.object("contentItem").expect("flickables have content"),
            Widget::Tabs { pages, .. } => *pages,
            Widget::Group { content, .. } => *content,
            widget => widget.item(),
        }
    }

    /// Built-in controls: they take `Enabled`.
    pub(super) fn is_control(&self) -> bool {
        matches!(
            self,
            Widget::Label(_)
                | Widget::Button(_)
                | Widget::MenuButton(_)
                | Widget::Field(_)
                | Widget::TextArea { .. }
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select(_)
                | Widget::RadioGroup(_)
                | Widget::Slider(_)
                | Widget::NumberInput(_)
        )
    }

    /// Controls that can take keyboard focus.
    pub(super) fn is_focusable(&self) -> bool {
        matches!(
            self,
            Widget::Button(_)
                | Widget::MenuButton(_)
                | Widget::Field(_)
                | Widget::TextArea { .. }
                | Widget::Checkbox(_)
                | Widget::Switch(_)
                | Widget::Select(_)
                | Widget::RadioGroup(_)
                | Widget::Slider(_)
                | Widget::NumberInput(_)
                | Widget::Custom { .. }
                | Widget::Native { .. }
                | Widget::List(_)
                | Widget::Sidebar { .. }
                | Widget::Tabs { .. }
        ) || matches!(self, Widget::GpuSurface(surface) if surface.takes_input())
    }

    /// Measured, never laid out inside: controls and escape hatches.
    pub(super) fn is_leaf(&self) -> bool {
        !matches!(
            self,
            Widget::Window { .. }
                | Widget::Host(_)
                | Widget::ToolbarItem { .. }
                | Widget::Scroll { .. }
                | Widget::List(_)
                | Widget::Sidebar { .. }
                | Widget::Tabs { .. }
                | Widget::Group { .. }
        )
    }
}
