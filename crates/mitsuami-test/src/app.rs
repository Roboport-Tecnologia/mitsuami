use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mitsuami_core::task::ManualClock;

use mitsuami_core::{
    A11yNode, Appearance, Command, CurrentWindow, NativeAppInfo, NodeId, NodeInfo, Role, Size, Ui, View, WindowSize,
};
use mitsuami_headless::{FakeServices, FakeServicesHandle, HeadlessHandle};
use mitsuami_reactive::{Owner, provide};

use crate::driver::{Driver, Mode};
use crate::locator::{Expectation, Locator};
use crate::query::{Query, by_label, by_role, by_test_id, by_text};
use crate::visual::VisualOptions;
use crate::{format, snapshot, visual};

pub(crate) struct TestContext {
    /// `module::path::test_name`, without the crate.
    pub name: String,
    pub file_prefix: String,
    pub manifest_dir: &'static str,
    /// Snapshots that are missing or don't match. They fail the test when
    /// it ends rather than stopping it, so one run writes every snapshot
    /// pending review.
    pub snapshot_failures: Rc<RefCell<Vec<String>>>,
}

impl TestContext {
    #[track_caller]
    pub(crate) fn fail_snapshot(&self, message: String) {
        let location = std::panic::Location::caller();
        self.snapshot_failures.borrow_mut().push(format!(
            "{message}
  at {}:{}",
            location.file(),
            location.line()
        ));
    }
}

/// The app under test: a [`Ui`] with one window, driven by the test.
pub struct TestApp {
    ui: Ui,
    clock: Rc<ManualClock>,
    services: FakeServicesHandle,
    driver: Driver,
    /// Holds what the app provides (the `Ui`, stores, test fakes) and
    /// outlives unmounting.
    owner: Owner,
    /// What's mounted, a child of `owner`: `unmount` disposes it, and the
    /// next mount starts a new one.
    mounted: Cell<Option<Owner>>,
    window: Cell<Option<NodeId>>,
    window_size: WindowSize,
    pub(crate) context: TestContext,
}

/// How long assertions and `wait_for_tasks` wait for background work.
/// `MITSUAMI_WAIT_MS` overrides the default of 2 seconds.
pub(crate) fn wait_timeout() -> Duration {
    let ms = std::env::var("MITSUAMI_WAIT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(2000);
    Duration::from_millis(ms)
}

/// Default window content size for tests.
pub const DEFAULT_WINDOW: Size = Size::new(800.0, 600.0);

impl TestApp {
    pub(crate) fn new(context: TestContext, mode: Mode, appearance: Appearance, window_size: WindowSize) -> TestApp {
        let (ui, driver) = Driver::create(mode, appearance);
        // Tests own time: timers only fire when the test advances the clock.
        let clock = Rc::new(ManualClock::default());
        ui.set_clock(clock.clone());
        // Never real dialogs or the real clipboard, even on native backends.
        let (fake, services) = FakeServices::new();
        ui.set_services(Box::new(fake));
        let owner = Owner::new_root();
        owner.with(|| {
            mitsuami_reactive::provide(ui.clone());
            mitsuami_core::provide_stores();
        });
        TestApp {
            ui,
            clock,
            services,
            driver,
            owner,
            mounted: Cell::new(None),
            window: Cell::new(None),
            window_size,
            context,
        }
    }

    /// The scripted platform services: answer dialogs, inspect the
    /// clipboard and the menus, choose menu items.
    pub fn services(&self) -> &FakeServicesHandle {
        &self.services
    }

    /// Moves the test clock forward, firing due timers, then settles.
    pub async fn advance(&self, by: Duration) {
        self.clock.advance(by);
        self.settle().await;
    }

    /// Waits for a future the platform completes later (a capture, a
    /// service reply), keeping the UI turning meanwhile.
    pub(crate) fn drive<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let deadline = Instant::now() + wait_timeout();
        loop {
            if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut cx) {
                return value;
            }
            if Instant::now() > deadline {
                panic!("the platform did not answer within {:?}", wait_timeout());
            }
            self.settle_now();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Settles repeatedly until no tasks are left (background work included),
    /// or fails after the wait timeout.
    pub async fn wait_for_tasks(&self) {
        let deadline = Instant::now() + wait_timeout();
        loop {
            self.settle_now();
            if self.ui.pending_tasks() == 0 {
                return;
            }
            if Instant::now() > deadline {
                panic!("{} task(s) still running after {:?}", self.ui.pending_tasks(), wait_timeout());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    /// The backend this test runs on: `"headless"`, `"appkit"`, …
    pub fn backend_name(&self) -> &'static str {
        self.driver.name()
    }

    pub fn is_headless(&self) -> bool {
        self.driver.headless().is_some()
    }

    /// Whether a `GpuSurface`'s handle has window and display handles:
    /// not headless, nor on the private displays Linux tests run on
    /// (Broadway, Qt's offscreen platform), which have no surfaces to give.
    pub fn has_surface_handles(&self) -> bool {
        !self.is_headless() && (!cfg!(target_os = "linux") || crate::driver::show_windows())
    }

    /// The headless backend, for simulating system changes. Tests using it
    /// must be marked `#[mitsuami_test::test(headless)]`.
    pub fn headless(&self) -> &HeadlessHandle {
        self.driver.headless().expect("this test uses headless-only APIs; mark it #[mitsuami_test::test(headless)]")
    }

    /// Commands the backend applied since the last call.
    pub fn take_command_log(&self) -> Vec<Command> {
        self.driver.take_command_log()
    }

    /// Number of live native widgets: a leak detector.
    pub fn native_node_count(&self) -> usize {
        self.driver.node_count()
    }

    /// What a window shows of the app's id, name and icon, read from the
    /// platform: what it took of [`Ui::set_app_info`]'s.
    /// The files dragging a list's or table's row out would carry, through
    /// the backend's own drag source; see [`Locator::dragged_files`].
    pub(crate) fn dragged_files(&self, node: NodeId) -> Option<Vec<PathBuf>> {
        self.driver.dragged_files(node)
    }

    pub fn native_app_info(&self, window: NodeId) -> NativeAppInfo {
        self.driver.app_info(window)
    }

    /// Name of the running test.
    pub fn test_name(&self) -> &str {
        &self.context.name
    }

    /// Makes `value` available to [`inject`](mitsuami_reactive::inject) in
    /// everything mounted afterwards. Use it to swap in fake stores.
    pub fn provide<T: 'static>(&self, value: T) {
        self.owner.with(|| mitsuami_reactive::provide(value));
    }

    /// Mounts a view in the test window (800×600 unless resized, or the
    /// story's size) and settles.
    pub fn mount<V: View>(&self, view: impl FnOnce() -> V) -> NodeId {
        let window = self.window();
        let mounted = self.mounted.get().unwrap_or_else(|| self.owner.child());
        self.mounted.set(Some(mounted));
        let root = mounted.with(|| {
            provide(CurrentWindow(window));
            view().build(&self.ui)
        });
        self.ui.append_child(window, root);
        self.settle_now();
        root
    }

    /// The test window, created on first use.
    pub fn window(&self) -> NodeId {
        if let Some(window) = self.window.get() {
            return window;
        }
        let window = self.ui.create_window("mitsuami test", self.window_size);
        self.window.set(Some(window));
        window
    }

    /// Runs until the UI is idle: events dispatched, effects flushed, native
    /// tree committed and laid out.
    pub async fn settle(&self) {
        self.settle_now();
    }

    pub(crate) fn settle_now(&self) {
        // The same loop the run loop uses: events a commit produces (a
        // resize, a scroll, focus moving) are handled before we look.
        // Platforms that deliver some of them asynchronously get to catch
        // up, until nothing new arrives.
        for _ in 0..16 {
            self.ui.tick();
            self.driver.settle();
            if self.ui.events().is_empty() {
                break;
            }
        }
        self.check_mirror();
    }

    /// Simulates the user resizing the window.
    pub async fn resize(&self, width: f32, height: f32) {
        self.driver.resize_window(self.window(), Size::new(width, height));
        self.settle().await;
    }

    /// Simulates the user resizing one of the app's windows.
    pub async fn resize_window(&self, window: NodeId, size: Size) {
        self.settle().await;
        self.driver.resize_window(window, size);
        self.settle().await;
    }

    /// Clicks a window's close button, as the user would. Whether it
    /// closes is the app's call.
    pub async fn close_window(&self, window: NodeId) {
        self.settle().await;
        self.driver.close_window(window);
        self.settle().await;
    }

    /// The open window with this title, other than the test window.
    pub fn window_titled(&self, title: &str) -> Option<NodeId> {
        self.ui.windows().into_iter().find(|w| self.ui.a11y_tree(*w).and_then(|n| n.name).as_deref() == Some(title))
    }

    pub fn get(&self, query: Query) -> Locator<'_> {
        Locator::new(self, query)
    }

    pub fn get_by_role(&self, role: Role, name: impl Into<String>) -> Locator<'_> {
        self.get(by_role(role, name))
    }

    pub fn get_by_text(&self, text: impl Into<String>) -> Locator<'_> {
        self.get(by_text(text))
    }

    pub fn get_by_label(&self, label: impl Into<String>) -> Locator<'_> {
        self.get(by_label(label))
    }

    pub fn get_by_test_id(&self, id: impl Into<String>) -> Locator<'_> {
        self.get(by_test_id(id))
    }

    /// Starts an assertion about the node `query` finds.
    pub fn expect(&self, query: Query) -> Expectation<'_> {
        Expectation::new(self.get(query))
    }

    pub fn a11y_tree(&self) -> A11yNode {
        self.ui.a11y_tree(self.window()).expect("the test window exists")
    }

    pub fn inspect(&self) -> NodeInfo {
        self.ui.inspect(self.window()).expect("the test window exists")
    }

    /// Where the native backends' snapshots and baselines go, under
    /// `tests/snapshots/` and `tests/visual/`: `<backend>/<image>`. `None`
    /// headless, whose snapshots don't depend on the machine.
    fn machine_dir(&self) -> Option<PathBuf> {
        (!self.is_headless())
            .then(|| Path::new(self.backend_name()).join(snapshot::image(self.ui.metrics().scale_factor)))
    }

    /// Compares the native tree (kinds, text, frames in window
    /// coordinates) with `tests/snapshots/[<backend>/<image>/]<test>@<name>.tree.txt`.
    #[track_caller]
    pub fn assert_tree_snapshot(&self, name: &str) {
        let machine = self.machine_dir();
        snapshot::assert(&self.context, machine.as_deref(), name, "tree.txt", &format::tree(&self.inspect()));
    }

    /// Compares the accessibility tree with a snapshot.
    #[track_caller]
    pub fn assert_a11y_snapshot(&self, name: &str) {
        // The accessibility tree is computed by the core: one snapshot for all backends.
        snapshot::assert(&self.context, None, name, "a11y.txt", &format::a11y(&self.a11y_tree()));
    }

    /// Compares an SVG wireframe of the layout with a snapshot.
    #[track_caller]
    pub fn assert_wireframe_snapshot(&self, name: &str) {
        let machine = self.machine_dir();
        snapshot::assert(&self.context, machine.as_deref(), name, "wireframe.svg", &format::wireframe(&self.inspect()));
    }

    /// Compares the commands sent to the backend since the last call.
    #[track_caller]
    pub fn assert_commands_snapshot(&self, name: &str) {
        let log = self.driver.take_command_log();
        let machine = self.machine_dir();
        snapshot::assert(&self.context, machine.as_deref(), name, "commands.txt", &format::commands(&log));
    }

    /// Captures the window and compares it with the PNG baseline in
    /// `tests/visual/<backend>/<image>/`. Headless has no pixels, so it skips.
    pub async fn assert_visual_snapshot(&self, name: &str) {
        self.assert_visual_snapshot_with(name, &VisualOptions::default()).await;
    }

    /// [`assert_visual_snapshot`](Self::assert_visual_snapshot) with a
    /// threshold, an allowance for changed pixels, or regions to leave out.
    pub async fn assert_visual_snapshot_with(&self, name: &str, options: &VisualOptions) {
        // Where the ignored nodes are now. Resolved headless too, so a query
        // that finds nothing fails everywhere.
        let mut ignored = options.ignore_rects.clone();
        for query in &options.ignore {
            let frames = self.get(query.clone()).frames();
            assert!(!frames.is_empty(), "a visual snapshot ignores {query}, which finds nothing");
            ignored.extend(frames);
        }
        let capture = self.ui.capture(self.window());
        match self.drive(capture) {
            Ok(image) => {
                let machine = self.machine_dir().expect("only native backends capture");
                let layout = format::tree(&self.inspect());
                visual::assert(&self.context, &machine, name, &image, &layout, options, &ignored)
            }
            Err(mitsuami_core::backend::CaptureError::Unsupported) => {}
            Err(e) => panic!("cannot capture the window: {e:?}"),
        }
    }

    /// Disposes everything mounted and destroys the window.
    pub fn unmount(&self) {
        if let Some(mounted) = self.mounted.take() {
            mounted.dispose();
        }
        if let Some(window) = self.window.take() {
            self.ui.destroy(window);
        }
        self.settle_now();
    }

    /// The native mirror must match the core tree after every settle.
    fn check_mirror(&self) {
        let Some(window) = self.window.get() else { return };
        let Some(root) = self.ui.inspect(window) else { return };
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            let native = self
                .ui
                .native_state(node.id)
                .unwrap_or_else(|| panic!("backend desync: {} {} has no native widget", node.kind.name(), node.id));
            let children: Vec<NodeId> = node.children.iter().map(|c| c.id).collect();
            assert_eq!(native.children, children, "backend desync: children of {} {}", node.kind.name(), node.id);
            if node.kind != mitsuami_core::WidgetKind::Window {
                let frame = self.ui.frame(node.id).unwrap_or_default();
                assert_eq!(native.frame, frame, "backend desync: frame of {} {}", node.kind.name(), node.id);
            }
            assert_eq!(
                native.scroll_offset,
                self.ui.scroll_offset(node.id),
                "backend desync: scroll offset of {} {}",
                node.kind.name(),
                node.id
            );
            let core_focused = self.ui.focused(window) == Some(node.id);
            assert_eq!(
                native.focused,
                core_focused,
                "backend desync: {} {} is {}focused natively but {}focused in the core (missing focus event?)",
                node.kind.name(),
                node.id,
                if native.focused { "" } else { "not " },
                if core_focused { "" } else { "not " },
            );
            for prop in &node.props {
                assert!(
                    native.props.contains(prop),
                    "backend desync: {} {} shows {:?}, core has {prop:?}",
                    node.kind.name(),
                    node.id,
                    native.props.iter().find(|p| p.key() == prop.key())
                );
            }
            stack.extend(node.children);
        }
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if self.owner.is_alive() {
            self.owner.dispose();
        }
    }
}
