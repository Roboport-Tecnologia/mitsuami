//! Running an app: our own message loop.

use std::rc::Rc;
use std::sync::Arc;

use mitsuami_core::services::MenuBar;
use mitsuami_core::{AppInfo, Ui};

use crate::backend::{BackendOptions, Win32Backend};
use crate::runtime;

/// Starts the app: `setup` creates the windows, then the message loop takes
/// over. Returns when the last window closes.
///
/// The loop ticks the UI whenever it is about to sleep, and sleeps until a
/// message arrives or the next timer is due. Ticks are also posted to a
/// message window, which modal loops (a window being moved or resized, a
/// dialog) dispatch too.
pub fn run(info: AppInfo, setup: impl FnOnce(&Ui)) {
    let backend = Win32Backend::new(BackendOptions::default());
    let handle = backend.handle();
    let ui = Ui::new(backend);
    ui.set_app_info(info);
    ui.set_menu(MenuBar::new());

    let tick: Rc<dyn Fn()> = Rc::new({
        let (weak, handle) = (ui.downgrade(), handle.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.tick();
                handle.show_pending_windows();
            }
        }
    });
    runtime::set_tick(Some(tick));
    ui.set_commit_scheduler(runtime::schedule_tick);
    // Background work finishing on another thread: wake our loop.
    let messages = runtime::messages();
    ui.set_waker(Arc::new(move || runtime::wake(messages)));

    setup(&ui);
    ui.tick();
    handle.show_pending_windows();
    loop {
        runtime::pump();
        ui.tick();
        handle.show_pending_windows();
        if ui.windows().is_empty() {
            break;
        }
        runtime::wait(ui.time_to_next_timer());
    }
    runtime::set_tick(None);
}

/// Prepares Win32 for tests, on the thread they run on.
pub fn init_for_tests() {
    runtime::init();
}

/// Dispatches pending messages.
pub fn pump() {
    runtime::pump();
}
