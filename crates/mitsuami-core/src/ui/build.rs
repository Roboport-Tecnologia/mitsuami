//! Creating nodes and setting their props, styles and handlers.

use std::rc::Rc;

use crate::a11y::A11yProps;
use crate::command::{Command, UiEvent};
use crate::geometry::{Point, Rect, Size, WindowSize};
use crate::style::Style;
use crate::widget::{NodeId, Prop, WidgetKind};

use super::{Fit, Node, Ui};

impl Ui {
    pub fn create(&self, kind: WidgetKind, props: Vec<Prop>) -> NodeId {
        let id = {
            let mut inner = self.inner.borrow_mut();
            let id = NodeId(inner.next_id);
            inner.next_id += 1;
            let taffy = if kind.is_native() {
                let t = if kind.is_container() {
                    inner.taffy.new_leaf(taffy::Style::default())
                } else {
                    inner.taffy.new_leaf_with_context(taffy::Style::default(), id)
                };
                Some(t.expect("taffy: failed to create node"))
            } else {
                None
            };
            if kind.is_native() {
                inner.pending.push(Command::Create { id, kind, props: props.clone() });
            }
            inner.nodes.insert(
                id,
                Node {
                    kind,
                    parent: None,
                    children: Vec::new(),
                    native_children: Vec::new(),
                    native_parent: None,
                    props,
                    style: Style::default(),
                    a11y: A11yProps::default(),
                    test_id: None,
                    tab_index: None,
                    handlers: Vec::new(),
                    taffy,
                    frame: Rect::ZERO,
                    window_size: Size::ZERO,
                    fit: Fit::None,
                    heights: Vec::new(),
                    scroll_offset: Point::ZERO,
                    row_width: None,
                    strip: Size::ZERO,
                    insets: None,
                },
            );
            inner.styles_dirty = true;
            id
        };
        self.changed();
        id
    }

    /// A window fitting its height is sized at its first layout, so give it
    /// its content before the next commit.
    pub fn create_window(&self, title: impl Into<String>, size: impl Into<WindowSize>) -> NodeId {
        let size = size.into();
        let mut props = vec![Prop::Title(title.into())];
        if let WindowSize::FollowHeight(_) = size {
            props.push(Prop::HeightFollowsContent(true));
        }
        let id = self.create(WidgetKind::Window, props);
        let mut inner = self.inner.borrow_mut();
        let node = inner.nodes.get_mut(&id).expect("just created");
        let (size, fit) = match size {
            WindowSize::Fixed(size) => (size, Fit::None),
            WindowSize::FitHeight(width) => (Size::new(width, 0.0), Fit::Once),
            WindowSize::FollowHeight(width) => (Size::new(width, 0.0), Fit::Follow { until_resized: false }),
            WindowSize::FollowHeightUntilResized(width) => (Size::new(width, 0.0), Fit::Follow { until_resized: true }),
        };
        node.window_size = size;
        node.fit = fit;
        node.frame = Rect { origin: Point::ZERO, size };
        inner.windows.push(id);
        // A fitted size is sent with the first frames.
        if fit == Fit::None {
            inner.pending.push(Command::SetWindowSize { id, size });
        }
        id
    }

    /// Asks for a window's content size, in points, as the app would resize
    /// it. The platform may refuse it (a window in full screen keeps the
    /// screen's), and gives it no smaller than the window's `MinSize`; the
    /// content is laid out at the size it reports (`WindowResized`). A
    /// `WindowSize::FollowHeight` window takes only the width: its content
    /// sets its height. Other windows stop fitting their height.
    pub fn set_window_size(&self, window: NodeId, size: Size) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(node) = inner.nodes.get_mut(&window).filter(|n| n.kind == WidgetKind::Window) else { return };
            let size = match node.fit {
                Fit::Follow { until_resized: false } => {
                    Size::new(size.width, node.heights.last().copied().unwrap_or(node.window_size.height))
                }
                _ => {
                    node.fit = Fit::None;
                    node.heights.clear();
                    size
                }
            };
            inner.pending.push(Command::SetWindowSize { id: window, size });
        }
        self.changed();
    }

    /// Sets a prop, sending it to the backend only if it changed.
    pub fn set_prop(&self, id: NodeId, prop: Prop) {
        {
            let mut inner = self.inner.borrow_mut();
            let inner = &mut *inner;
            let Some(node) = inner.nodes.get_mut(&id) else { return };
            if node.prop(&prop) == Some(&prop) {
                return;
            }
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop.clone());
            if prop.affects_measure(node.kind)
                && let Some(t) = node.taffy
            {
                let _ = inner.taffy.mark_dirty(t);
            }
            if matches!(prop, Prop::TextStyle(_)) {
                inner.styles_dirty = true;
            }
            // New titles, icons or another style may make the tab strip wider
            // (`size_tab_strips`), and a group's heading wider, or give it
            // the titled insets.
            if (matches!(prop, Prop::TabTitles(_) | Prop::TabIcons(_) | Prop::TabsStyle(_))
                || (node.kind == WidgetKind::Group && matches!(prop, Prop::Title(_))))
                && let Some(t) = node.taffy
            {
                let _ = inner.taffy.mark_dirty(t);
                inner.styles_dirty |= node.kind == WidgetKind::Group;
            }
            if node.kind.is_native() {
                inner.queue_prop(id, prop);
            }
        }
        self.changed();
    }

    pub fn set_style(&self, id: NodeId, style: Style) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(node) = inner.nodes.get_mut(&id) else { return };
            if node.style == style {
                return;
            }
            node.style = style;
            inner.styles_dirty = true;
        }
        self.changed();
    }

    /// Edits a node's style in place (used by reactive style bindings).
    pub fn update_style(&self, id: NodeId, f: impl FnOnce(&mut Style)) {
        let style = {
            let inner = self.inner.borrow();
            let Some(node) = inner.nodes.get(&id) else { return };
            let mut style = node.style.clone();
            f(&mut style);
            style
        };
        self.set_style(id, style);
    }

    pub fn set_a11y(&self, id: NodeId, a11y: A11yProps) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(node) = inner.nodes.get_mut(&id) else { return };
            if node.a11y == a11y {
                return;
            }
            node.a11y = a11y.clone();
            if node.kind.is_native() {
                inner.pending.push(Command::SetA11y { id, a11y });
            }
        }
        self.changed();
    }

    pub fn set_test_id(&self, id: NodeId, test_id: impl Into<String>) {
        if let Some(node) = self.inner.borrow_mut().nodes.get_mut(&id) {
            node.test_id = Some(test_id.into());
        }
    }

    /// Adds an event handler. It runs in the reactive scope that is current
    /// now (usually the component being built), so `inject`, `spawn_local`
    /// and friends work inside it.
    pub fn on_event(&self, id: NodeId, handler: impl Fn(&UiEvent) + 'static) {
        let owner = mitsuami_reactive::Owner::current();
        let handler = move |event: &UiEvent| match owner {
            Some(owner) if owner.is_alive() => owner.with(|| handler(event)),
            _ => handler(event),
        };
        if let Some(node) = self.inner.borrow_mut().nodes.get_mut(&id) {
            node.handlers.push(Rc::new(handler));
        }
    }
}
