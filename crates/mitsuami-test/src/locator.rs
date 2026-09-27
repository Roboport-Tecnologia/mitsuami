use mitsuami_core::{A11yAction, A11yNode, Key, NativeState, NodeId, Point, Rect, Role, SyntheticInput};

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

    async fn input(&self, key: Key) {
        let node = self.node();
        if let Err(e) = self.app.ui().synthesize(node.id, &SyntheticInput::Key(key)) {
            self.fail(&format!("cannot press {key:?} on {}: {e}", self.query));
        }
        self.app.settle().await;
    }

    /// Activates the control: click a button, toggle a checkbox.
    pub async fn click(&self) {
        self.act(A11yAction::Activate).await;
    }

    /// Selects a list row, as assistive technology would: in place of the
    /// selected row, or of every selected row in a multiple-selection list.
    /// Clicking a row ([`click`](Self::click)) activates it instead.
    pub async fn select(&self) {
        self.act(A11yAction::Select).await;
    }

    /// Replaces a text field's content in one step.
    pub async fn fill(&self, text: &str) {
        self.act(A11yAction::SetValue(text.to_owned())).await;
    }

    /// Chooses the option with this text in a `Select`, as assistive
    /// technology would.
    pub async fn select_option(&self, option: &str) {
        self.act(A11yAction::SetValue(option.to_owned())).await;
    }

    /// Moves a `Slider` to this number, as assistive technology would.
    pub async fn set_number(&self, number: f64) {
        self.act(A11yAction::SetValue(number.to_string())).await;
    }

    /// Types text one key at a time, like a user would.
    pub async fn type_text(&self, text: &str) {
        self.app.settle().await;
        for c in text.chars() {
            self.input(Key::Char(c)).await;
        }
    }

    pub async fn press(&self, key: Key) {
        self.app.settle().await;
        self.input(key).await;
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
