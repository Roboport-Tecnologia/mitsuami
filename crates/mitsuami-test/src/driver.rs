//! What a test needs from a backend beyond the `Backend` contract.

use std::rc::Rc;

use mitsuami_core::{Appearance, Command, NativeAppInfo, NodeId, Size, TestHooks, Ui};
use mitsuami_headless::{HeadlessBackend, HeadlessHandle};

/// Which backend runs the test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Headless,
    Native,
}

pub(crate) struct Driver {
    hooks: Rc<dyn TestHooks>,
    headless: Option<HeadlessHandle>,
}

impl Driver {
    pub(crate) fn create(mode: Mode, appearance: Appearance) -> (Ui, Driver) {
        match mode {
            Mode::Headless => {
                let backend = HeadlessBackend::new();
                let handle = backend.handle();
                handle.set_metrics(mitsuami_core::PlatformMetrics {
                    dark_mode: appearance == Appearance::Dark,
                    ..mitsuami_headless::metrics()
                });
                let driver = Driver { hooks: Rc::new(handle.clone()), headless: Some(handle) };
                (Ui::new(backend), driver)
            }
            Mode::Native => {
                let (ui, hooks) = native(appearance);
                let hooks: Rc<dyn TestHooks> = Rc::from(hooks);
                let idle = hooks.clone();
                crate::exec::set_idle(Some(Rc::new(move || idle.settle())));
                (ui, Driver { hooks, headless: None })
            }
        }
    }

    pub(crate) fn headless(&self) -> Option<&HeadlessHandle> {
        self.headless.as_ref()
    }

    /// Short name used in snapshot and baseline paths.
    pub(crate) fn name(&self) -> &'static str {
        self.hooks.name()
    }

    pub(crate) fn resize_window(&self, window: NodeId, size: Size) {
        self.hooks.resize_window(window, size);
    }

    pub(crate) fn close_window(&self, window: NodeId) {
        self.hooks.close_window(window);
    }

    pub(crate) fn take_command_log(&self) -> Vec<Command> {
        self.hooks.take_command_log()
    }

    pub(crate) fn node_count(&self) -> usize {
        self.hooks.node_count()
    }

    pub(crate) fn app_info(&self, window: NodeId) -> NativeAppInfo {
        self.hooks.app_info(window)
    }

    pub(crate) fn settle(&self) {
        self.hooks.settle();
    }

    pub(crate) fn dragged_files(&self, node: NodeId) -> Option<Vec<std::path::PathBuf>> {
        self.hooks.dragged_files(node)
    }
}

impl Drop for Driver {
    /// The idle hook holds the backend, whose windows live as long as it.
    fn drop(&mut self) {
        crate::exec::set_idle(None);
    }
}

pub(crate) fn show_windows() -> bool {
    std::env::var("MITSUAMI_SHOW_WINDOWS").is_ok_and(|v| v == "1")
}

// One `native()` per platform: create the backend configured for tests
// (offscreen unless MITSUAMI_SHOW_WINDOWS=1, recording commands, the
// test's appearance whatever the system's, a private clipboard) and return its test hooks.

#[cfg(target_os = "macos")]
fn native(appearance: Appearance) -> (Ui, Box<dyn TestHooks>) {
    use mitsuami_appkit::{AppKitBackend, BackendOptions};
    let mtm = mitsuami_appkit::init_for_tests();
    let backend = AppKitBackend::new(
        mtm,
        BackendOptions {
            show_windows: show_windows(),
            record_commands: true,
            appearance: Some(appearance),
            private_clipboard: true,
        },
    );
    let hooks = backend.handle();
    (Ui::new(backend), Box::new(hooks))
}

/// Tests run on a private display unless MITSUAMI_SHOW_WINDOWS=1 (see
/// `mitsuami_gtk::init_for_tests`). GTK has no private clipboard, but
/// the private display's clipboard is its own.
#[cfg(all(target_os = "linux", not(feature = "kde")))]
fn native(appearance: Appearance) -> (Ui, Box<dyn TestHooks>) {
    use mitsuami_gtk::{BackendOptions, GtkBackend};
    let _ = show_windows;
    mitsuami_gtk::init_for_tests();
    let backend = GtkBackend::new(BackendOptions { record_commands: true, appearance: Some(appearance) });
    let hooks = backend.handle();
    (Ui::new(backend), Box::new(hooks))
}

/// Tests run on Qt's offscreen platform unless MITSUAMI_SHOW_WINDOWS=1 (see
/// `mitsuami_kirigami::init_for_tests`), whose clipboard is the process's own.
#[cfg(all(target_os = "linux", feature = "kde"))]
fn native(appearance: Appearance) -> (Ui, Box<dyn TestHooks>) {
    use mitsuami_kirigami::{BackendOptions, KirigamiBackend};
    let _ = show_windows;
    mitsuami_kirigami::init_for_tests();
    let backend = KirigamiBackend::new(BackendOptions { record_commands: true, appearance: Some(appearance) });
    let hooks = backend.handle();
    (Ui::new(backend), Box::new(hooks))
}

/// Test windows are alive but invisible (and click-through) unless
/// MITSUAMI_SHOW_WINDOWS=1: XAML only lays out and renders live windows.
#[cfg(windows)]
fn native(appearance: Appearance) -> (Ui, Box<dyn TestHooks>) {
    use mitsuami_winui::{BackendOptions, WinUiBackend};
    mitsuami_winui::init_for_tests();
    let backend = WinUiBackend::new(BackendOptions {
        show_windows: show_windows(),
        record_commands: true,
        appearance: Some(appearance),
        private_clipboard: true,
    });
    let hooks = backend.handle();
    (Ui::new(backend), Box::new(hooks))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn native(_: Appearance) -> (Ui, Box<dyn TestHooks>) {
    let _ = show_windows;
    panic!("mitsuami-test: no native backend for this platform")
}

/// Whether this platform has a native backend to run `--native` tests on.
pub(crate) fn native_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "linux", windows))
}

/// Runs `f` inside an autorelease pool where the platform needs one, so
/// native objects created by a test are released when it ends.
pub(crate) fn with_pool<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(target_os = "macos")]
    {
        objc2::rc::autoreleasepool(|_| f())
    }
    #[cfg(not(target_os = "macos"))]
    {
        f()
    }
}
