//! Running an app: NSApplication, the menu bar and the run-loop hook.

use block2::RcBlock;
use mitsuami_core::Ui;
use mitsuami_core::services::MenuBar;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSApplicationTerminateReply, NSEvent,
    NSEventModifierFlags, NSEventType,
};
use std::sync::Arc;

use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFRunLoop, CFRunLoopActivity, CFRunLoopObserver, CFRunLoopTimer, kCFRunLoopCommonModes,
};
use objc2_foundation::NSPoint;

use crate::backend::{AppKitBackend, BackendOptions};

/// Ends `NSApplication::run`. `stop:` only takes effect once an event has
/// been processed, and we're in a run-loop observer rather than an event
/// handler, so post an empty one.
fn stop(app: &NSApplication) {
    app.stop(None);
    let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
        NSEventType::ApplicationDefined,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags::empty(),
        0.0,
        0,
        None,
        0,
        0,
        0,
    );
    if let Some(event) = event {
        app.postEvent_atStart(&event, true);
    }
}

pub(crate) struct AppIvars {
    /// Asks the app to quit, as its Quit item or its windows' close
    /// buttons would; returns whether it did (no window is left).
    quit: Box<dyn Fn() -> bool>,
}

define_class!(
    /// The app's delegate, for the system's Quit.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        /// `terminate:` from anywhere: the app menu's own Quit, the Dock's,
        /// logging out or restarting. The app decides, as for its own
        /// Quit, and a quit it doesn't go through with is cancelled (macOS
        /// says the app cancelled the log out, as for TextEdit with unsaved
        /// documents). If it closed every window, the process ends here.
        #[unsafe(method(applicationShouldTerminate:))]
        fn application_should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            if (self.ivars().quit)() {
                NSApplicationTerminateReply::TerminateNow
            } else {
                NSApplicationTerminateReply::TerminateCancel
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker, quit: impl Fn() -> bool + 'static) -> Retained<AppDelegate> {
        let this = AppDelegate::alloc(mtm).set_ivars(AppIvars { quit: Box::new(quit) });
        unsafe { msg_send![super(this), init] }
    }
}

/// A fire date that never comes (about 30 years out).
const NEVER: f64 = 1.0e9;

/// Safe from any thread.
fn wake_main_run_loop() {
    if let Some(run_loop) = CFRunLoop::main() {
        run_loop.wake_up();
    }
}

fn main_thread() -> MainThreadMarker {
    MainThreadMarker::new().expect("mitsuami: AppKit must be driven from the main thread")
}

/// Starts the app: `setup` creates the windows, then AppKit's run loop takes
/// over. Returns when the last window closes.
pub fn run(setup: impl FnOnce(&Ui)) {
    let mtm = main_thread();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let backend = AppKitBackend::new(mtm, BackendOptions::default());
    let handle = backend.handle();
    let ui = Ui::new(backend);
    // The standard app and Edit menus, until the app installs its own.
    ui.set_menu(MenuBar::new());
    ui.set_commit_scheduler(wake_main_run_loop);
    // Tasks woken from other threads (background work finishing) make the
    // main run loop turn, which ticks the UI.
    ui.set_waker(Arc::new(wake_main_run_loop));

    setup(&ui);
    ui.tick();
    handle.show_pending_windows();

    // The app's own handlers run now, in this turn: the answer is due
    // before `applicationShouldTerminate:` returns.
    let delegate = {
        let (ui, handle) = (ui.clone(), handle.clone());
        AppDelegate::new(mtm, move || {
            ui.request_quit();
            ui.tick();
            handle.show_pending_windows();
            ui.windows().is_empty()
        })
    };
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    let run_loop = CFRunLoop::main().expect("main run loop");
    // Common modes include event tracking, so live resizing relayouts too.
    let common = unsafe { kCFRunLoopCommonModes };

    // One timer, re-armed after every tick for the next `sleep` deadline.
    // Firing just makes the loop turn; the observer below does the work.
    let fire = RcBlock::new(|_: *mut CFRunLoopTimer| {});
    let timer =
        unsafe { CFRunLoopTimer::with_handler(None, NEVER, 1.0e9, 0, 0, Some(&fire)) }.expect("create run loop timer");
    run_loop.add_timer(Some(&timer), common);

    let tick = RcBlock::new(move |_: *mut CFRunLoopObserver, _: CFRunLoopActivity| {
        ui.tick();
        handle.show_pending_windows();
        let next = ui.time_to_next_timer().map_or(NEVER, |d| CFAbsoluteTimeGetCurrent() + d.as_secs_f64());
        timer.set_next_fire_date(next);
        if ui.windows().is_empty() {
            stop(&NSApplication::sharedApplication(main_thread()));
        }
    });
    let observer =
        unsafe { CFRunLoopObserver::with_handler(None, CFRunLoopActivity::BeforeWaiting.0, true, 0, Some(&tick)) };
    run_loop.add_observer(observer.as_deref(), common);

    app.activate();
    app.run();
}

/// Prepares AppKit for tests: no Dock icon, no menu bar takeover.
pub fn init_for_tests() -> MainThreadMarker {
    let mtm = main_thread();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    mtm
}
