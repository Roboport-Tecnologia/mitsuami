use mitsuami_core::services::{MenuEntry, Shortcut, find_menu_item};
use mitsuami_core::{
    A11yAction, A11yNode, Key, NativeState, NodeId, Point, Rect, Role, SyntheticInput, WidgetKind, find_prop,
};

use crate::app::TestApp;
use crate::format;
use crate::query::Query;

/// A lazy reference to a node: the query runs again on every use, so a
/// locator stays valid across re-renders.
pub struct Locator<'a> {
    app: &'a TestApp,
    query: Query,
}

impl<'a> Locator<'a> {
    pub(crate) fn new(app: &'a TestApp, query: Query) -> Locator<'a> {
        Locator { app, query }
    }

    pub fn query(&self) -> &Query {
        &self.query
    }

    fn all(&self) -> Vec<A11yNode> {
        let mut out = Vec::new();
        for window in self.app.ui().windows() {
            let Some(tree) = self.app.ui().a11y_tree(window) else { continue };
            let semantic = tree.walk();
            if let Query::TestId(test_id) = &self.query {
                // Test ids also reach nodes the a11y tree leaves out, such as
                // plain layout containers.
                let Some(native) = self.app.ui().inspect(window) else { continue };
                let mut stack = vec![native];
                while let Some(node) = stack.pop() {
                    if node.test_id.as_ref() == Some(test_id) {
                        out.push(semantic.iter().find(|n| n.id == node.id).map(|n| (*n).clone()).unwrap_or(A11yNode {
                            id: node.id,
                            role: Role::None,
                            name: None,
                            description: None,
                            value: None,
                            checked: None,
                            mixed: false,
                            read_only: false,
                            password: false,
                            selected: None,
                            enabled: true,
                            test_id: node.test_id.clone(),
                            frame: node.frame,
                            children: Vec::new(),
                        }));
                    }
                    stack.extend(node.children);
                }
            } else {
                out.extend(semantic.into_iter().filter(|n| self.query.matches(n)).cloned());
            }
        }
        out
    }

    fn try_node(&self) -> Result<A11yNode, String> {
        let mut found = self.all();
        match found.len() {
            1 => Ok(found.remove(0)),
            0 => Err(format!("no node matches {}", self.query)),
            n => Err(format!("{n} nodes match {}; make the query more specific", self.query)),
        }
    }

    /// The single matching node. Panics, showing the accessibility tree, if
    /// there are none or several.
    pub fn node(&self) -> A11yNode {
        self.try_node().unwrap_or_else(|e| self.fail(&e))
    }

    fn fail(&self, message: &str) -> ! {
        panic!("{message}\n\naccessibility tree:\n{}", format::a11y(&self.app.a11y_tree()))
    }

    pub fn id(&self) -> NodeId {
        self.node().id
    }

    pub fn count(&self) -> usize {
        self.all().len()
    }

    pub fn exists(&self) -> bool {
        self.count() > 0
    }

    /// In window coordinates.
    pub fn frame(&self) -> Rect {
        self.node().frame
    }

    /// The frames of every node the query finds, in window coordinates.
    pub fn frames(&self) -> Vec<Rect> {
        self.all().into_iter().map(|n| n.frame).collect()
    }

    /// Accessible name (for text: its content).
    pub fn text(&self) -> Option<String> {
        self.node().name
    }

    pub fn value(&self) -> Option<String> {
        self.node().value
    }

    pub fn is_checked(&self) -> bool {
        self.node().checked == Some(true)
    }

    pub fn is_enabled(&self) -> bool {
        self.node().enabled
    }

    pub fn is_read_only(&self) -> bool {
        self.node().read_only
    }

    /// Some part of it can be seen: not hidden, not zero-sized, and not
    /// clipped away by the window or an enclosing scroll view.
    pub fn is_visible(&self) -> bool {
        let Ok(node) = self.try_node() else { return false };
        self.app.ui().visible_rect(node.id).is_some_and(|r| !r.size.is_empty())
    }

    /// Has keyboard focus, according to the native widget.
    pub fn is_focused(&self) -> bool {
        self.native_state().focused
    }

    /// A focused text field's or text area's selection, in characters,
    /// as the native widget shows it (an empty one is the caret).
    pub fn text_selection(&self) -> Option<std::ops::Range<usize>> {
        self.native_state().selection
    }

    /// What the native widget actually shows.
    pub fn native_state(&self) -> NativeState {
        let id = self.id();
        self.app.ui().native_state(id).unwrap_or_else(|| self.fail(&format!("{id} has no native widget")))
    }

    async fn act(&self, action: A11yAction) {
        self.app.settle().await;
        let node = self.node();
        if let Err(e) = self.app.ui().perform(node.id, &action) {
            self.fail(&format!("cannot {action:?} {}: {e}", self.query));
        }
        self.app.settle().await;
    }

    async fn input(&self, input: SyntheticInput) {
        let node = self.node();
        if let Err(e) = self.app.ui().synthesize(node.id, &input) {
            let pressed = match &input {
                SyntheticInput::Key(key) => format!("{key:?}"),
                SyntheticInput::Shortcut(shortcut) => format!("{shortcut:?}"),
                other => format!("{other:?}"),
            };
            self.fail(&format!("cannot press {pressed} on {}: {e}", self.query));
        }
        self.app.settle().await;
    }

    /// Activates the control: click a button, toggle a checkbox. A radio
    /// button is chosen by its option (the first with it), as a radio
    /// group's buttons are its data, not nodes of their own; so is a
    /// table's column header pressed, by its title, which sorts the table.
    pub async fn click(&self) {
        self.app.settle().await;
        let node = self.node();
        match (self.app.ui().kind(node.id), node.role, node.name) {
            (Some(WidgetKind::RadioGroup), Role::RadioButton, Some(option)) => {
                self.act(A11yAction::SetValue(option)).await
            }
            (Some(WidgetKind::Table), Role::ColumnHeader, Some(title)) => {
                let columns = find_prop!(self.app.ui().props(node.id), Columns).unwrap_or_default();
                let Some(column) = columns.iter().position(|c| c.title == title) else {
                    self.fail(&format!("the table has no column {title:?}"))
                };
                self.act(A11yAction::PressHeader(column)).await
            }
            _ => self.act(A11yAction::Activate).await,
        }
    }

    /// Selects a list row, as assistive technology would: in place of the
    /// selected row, or of every selected row in a multiple-selection list.
    /// A table's row is selected through it or any of its cells. Clicking
    /// a row ([`click`](Self::click)) activates it instead.
    ///
    /// A sidebar's item is chosen by its title (the first with it), as its
    /// items are the sidebar's data, not nodes of their own; so is a tab
    /// view's tab, which shows its page.
    pub async fn select(&self) {
        let node = self.node();
        match (self.app.ui().kind(node.id), node.role, node.name) {
            (Some(WidgetKind::Sidebar), Role::ListItem, Some(title)) => self.act(A11yAction::SetValue(title)).await,
            (Some(WidgetKind::Tabs), Role::Tab, Some(title)) => self.act(A11yAction::SetValue(title)).await,
            _ => self.act(A11yAction::Select).await,
        }
    }

    /// Replaces a text field's content in one step.
    pub async fn fill(&self, text: &str) {
        self.act(A11yAction::SetValue(text.to_owned())).await;
    }

    /// Chooses the option with this text in a `Select` or `RadioGroup`, as
    /// assistive technology would.
    pub async fn select_option(&self, option: &str) {
        self.act(A11yAction::SetValue(option.to_owned())).await;
    }

    /// Moves a `Slider` or `NumberInput` to this number, as assistive
    /// technology would.
    pub async fn set_number(&self, number: f64) {
        self.act(A11yAction::SetValue(number.to_string())).await;
    }

    /// Types text one key at a time, like a user would.
    pub async fn type_text(&self, text: &str) {
        self.app.settle().await;
        for c in text.chars() {
            self.input(SyntheticInput::Key(Key::Char(c))).await;
        }
    }

    /// Presses a key on the control, with it focused, as the keyboard
    /// does: `press(Key::Enter)`, `press(' ')`, or with modifiers,
    /// `press(Shortcut::primary(Key::Backspace))`. A key the control
    /// doesn't use goes up to the nearest node that takes it (`on_key`).
    pub async fn press(&self, key: impl Into<Shortcut>) {
        self.app.settle().await;
        let input = match key.into() {
            Shortcut { key, primary: false, shift: false, alt: false } => SyntheticInput::Key(key),
            shortcut => SyntheticInput::Shortcut(shortcut),
        };
        self.input(input).await;
    }

    pub async fn focus(&self) {
        self.act(A11yAction::Focus).await;
    }

    /// Scrolls enclosing scroll views until the node is in view.
    pub async fn scroll_into_view(&self) {
        self.app.settle().await;
        self.app.ui().scroll_into_view(self.node().id);
        self.app.settle().await;
    }

    /// Scrolls a scroll view like a scroll wheel or trackpad would.
    pub async fn scroll_by(&self, dx: f32, dy: f32) {
        self.app.settle().await;
        let node = self.node();
        if let Err(e) = self.app.ui().synthesize(node.id, &SyntheticInput::Scroll { dx, dy }) {
            self.fail(&format!("cannot scroll {}: {e}", self.query));
        }
        self.app.settle().await;
    }

    /// Drags these files and folders from the file manager over the node,
    /// through the platform's own drag handling, without dropping them.
    pub async fn drag_files<P: AsRef<std::path::Path>>(&self, paths: &[P]) {
        self.drag(
            SyntheticInput::DragFiles(paths.iter().map(|p| p.as_ref().to_path_buf()).collect()),
            "drag files over",
        )
        .await;
    }

    /// Drags the files being dragged over the node away from it.
    pub async fn drag_leave(&self) {
        self.drag(SyntheticInput::DragLeave, "drag files away from").await;
    }

    /// Drags these files and folders over the node and drops them there.
    pub async fn drop_files<P: AsRef<std::path::Path>>(&self, paths: &[P]) {
        self.drag(SyntheticInput::DropFiles(paths.iter().map(|p| p.as_ref().to_path_buf()).collect()), "drop files on")
            .await;
    }

    async fn drag(&self, input: SyntheticInput, what: &str) {
        self.app.settle().await;
        let node = self.node();
        if let Err(e) = self.app.ui().synthesize(node.id, &input) {
            self.fail(&format!("cannot {what} {}: {e}", self.query));
        }
        self.app.settle().await;
    }

    pub async fn check(&self) {
        if !self.is_checked() {
            self.click().await;
        }
    }

    pub async fn uncheck(&self) {
        if self.is_checked() {
            self.click().await;
        }
    }

    /// Steps an adjustable control (a slider, a rating) up, like assistive
    /// technology does.
    pub async fn increment(&self) {
        self.act(A11yAction::Increment).await;
    }

    pub async fn decrement(&self) {
        self.act(A11yAction::Decrement).await;
    }

    /// The context menu a right-click here shows: the node's, or that of
    /// the nearest container around it with one. `None`: no menu.
    pub fn context_menu(&self) -> Option<(NodeId, Vec<MenuEntry>)> {
        let ui = self.app.ui();
        let mut id = Some(self.id());
        // A list row's host (a table cell's) holds the row's view, which is
        // what a right-click on the row hits.
        if let Some(host) = id
            && (find_prop!(ui.props(host), Row).is_some() || find_prop!(ui.props(host), Cell).is_some())
            && let [view] = ui.children(host)[..]
        {
            id = Some(view);
        }
        while let Some(node) = id {
            // Separators alone show nothing.
            let shown = |menu: &Vec<MenuEntry>| menu.iter().any(|e| !matches!(e, MenuEntry::Separator));
            if let Some(menu) = find_prop!(ui.props(node), ContextMenu).filter(shown) {
                return Some((node, menu));
            }
            id = ui.parent(node);
        }
        None
    }

    /// Chooses an item of the menu a `MenuButton` opens, or else of the
    /// context menu a right-click here shows (see
    /// [`context_menu`](Self::context_menu)), by the titles of its
    /// submenus and its own, as assistive technology would once it has
    /// shown the menu: `choose_menu_item(&["Sort By", "Name"])`.
    pub async fn choose_menu_item(&self, path: &[&str]) {
        self.app.settle().await;
        let id = self.id();
        if self.app.ui().kind(id) == Some(WidgetKind::MenuButton) {
            let menu = find_prop!(self.app.ui().props(id), Menu).unwrap_or_default();
            let Some(item) = find_menu_item(&menu, path) else {
                self.fail(&format!("the menu of {} has no item {path:?}", self.query))
            };
            if let Err(e) = self.app.ui().perform(id, &A11yAction::MenuItem(item.id)) {
                self.fail(&format!("cannot choose {path:?} in the menu of {}: {e}", self.query));
            }
            return self.app.settle().await;
        }
        let Some((node, menu)) = self.context_menu() else { self.fail(&format!("{} has no context menu", self.query)) };
        let Some(item) = find_menu_item(&menu, path) else {
            self.fail(&format!("the context menu of {} has no item {path:?}", self.query))
        };
        if let Err(e) = self.app.ui().perform(node, &A11yAction::ContextMenuItem(item.id)) {
            self.fail(&format!("cannot choose {path:?} in the context menu of {}: {e}", self.query));
        }
        self.app.settle().await;
    }

    /// Clicks at a point in the node's own coordinates. Drawn custom
    /// widgets support it; native controls are driven by `click`.
    pub async fn click_at(&self, x: f32, y: f32) {
        self.app.settle().await;
        let node = self.node();
        if let Err(e) = self.app.ui().synthesize(node.id, &SyntheticInput::Click(Point::new(x, y))) {
            self.fail(&format!("cannot click {} at {x},{y}: {e}", self.query));
        }
        self.app.settle().await;
    }
}

/// An assertion about a [`Locator`]. Every assertion first lets the UI
/// settle, so no test ever needs to sleep.
pub struct Expectation<'a> {
    locator: Locator<'a>,
}

impl<'a> Expectation<'a> {
    pub(crate) fn new(locator: Locator<'a>) -> Expectation<'a> {
        Expectation { locator }
    }

    /// Settles and checks; while tasks are still running (e.g. background
    /// work), retries until it passes or the wait timeout expires.
    async fn check(&self, ok: impl Fn(&Locator<'a>) -> Result<(), String>) {
        let app = self.locator.app;
        let deadline = std::time::Instant::now() + crate::app::wait_timeout();
        loop {
            app.settle().await;
            let Err(message) = ok(&self.locator) else { return };
            if app.ui().pending_tasks() == 0 || std::time::Instant::now() > deadline {
                self.locator.fail(&format!("expected {}: {message}", self.locator.query));
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    pub async fn to_exist(&self) {
        self.check(|l| l.try_node().map(drop)).await;
    }

    pub async fn not_to_exist(&self) {
        self.check(|l| match l.count() {
            0 => Ok(()),
            n => Err(format!("not to exist, found {n}")),
        })
        .await;
    }

    pub async fn to_be_visible(&self) {
        self.check(|l| {
            l.try_node()?;
            if l.is_visible() { Ok(()) } else { Err(format!("to be visible, frame is {}", l.frame())) }
        })
        .await;
    }

    /// Passes if the node is missing or not visible.
    pub async fn to_be_hidden(&self) {
        self.check(|l| if l.is_visible() { Err(format!("to be hidden, frame is {}", l.frame())) } else { Ok(()) })
            .await;
    }

    pub async fn to_have_text(&self, text: &str) {
        self.check(|l| {
            let actual = l.try_node()?.name;
            if actual.as_deref() == Some(text) { Ok(()) } else { Err(format!("to have text {text:?}, got {actual:?}")) }
        })
        .await;
    }

    pub async fn to_have_value(&self, value: &str) {
        self.check(|l| {
            let actual = l.try_node()?.value;
            if actual.as_deref() == Some(value) {
                Ok(())
            } else {
                Err(format!("to have value {value:?}, got {actual:?}"))
            }
        })
        .await;
    }

    pub async fn to_be_checked(&self) {
        self.check(|l| if l.try_node()?.checked == Some(true) { Ok(()) } else { Err("to be checked".into()) }).await;
    }

    pub async fn not_to_be_checked(&self) {
        self.check(|l| if l.try_node()?.checked == Some(false) { Ok(()) } else { Err("not to be checked".into()) })
            .await;
    }

    pub async fn to_be_focused(&self) {
        self.check(|l| {
            l.try_node()?;
            if l.is_focused() { Ok(()) } else { Err("to be focused".into()) }
        })
        .await;
    }

    pub async fn not_to_be_focused(&self) {
        self.check(|l| {
            l.try_node()?;
            if l.is_focused() { Err("not to be focused".into()) } else { Ok(()) }
        })
        .await;
    }

    pub async fn to_be_enabled(&self) {
        self.check(|l| if l.try_node()?.enabled { Ok(()) } else { Err("to be enabled".into()) }).await;
    }

    pub async fn to_be_disabled(&self) {
        self.check(|l| if l.try_node()?.enabled { Err("to be disabled".into()) } else { Ok(()) }).await;
    }

    pub async fn to_be_read_only(&self) {
        self.check(|l| if l.try_node()?.read_only { Ok(()) } else { Err("to be read-only".into()) }).await;
    }

    pub async fn to_be_editable(&self) {
        self.check(|l| if l.try_node()?.read_only { Err("to be editable".into()) } else { Ok(()) }).await;
    }

    /// Frame in window coordinates.
    pub async fn to_have_frame(&self, frame: Rect) {
        self.check(|l| {
            let actual = l.try_node()?.frame;
            if actual == frame { Ok(()) } else { Err(format!("to have frame {frame}, got {actual}")) }
        })
        .await;
    }
}
