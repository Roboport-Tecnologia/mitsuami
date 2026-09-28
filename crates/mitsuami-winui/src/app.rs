//! Running an app: our own message loop around XAML.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use mitsuami_core::Ui;
use mitsuami_core::services::MenuBar;
use windows_core::Interface;

use crate::backend::{BackendOptions, WinUiBackend, WinUiHandle};
use crate::bindings as w;
use crate::runtime;

/// Starts the app: `setup` creates the windows, then the message loop takes
/// over. Returns when the last window closes.
///
/// The loop ticks the UI whenever it is about to sleep, like AppKit's
/// run-loop observer, and sleeps until a message arrives or the next timer is
/// due. Ticks are also scheduled on the dispatcher queue, which keeps working
/// inside modal loops (window moves and live resizing) that bypass ours.
pub fn run(setup: impl FnOnce(&Ui)) {
    let backend = WinUiBackend::new(BackendOptions::default());
    let handle = backend.handle();
    let ui = Ui::new(backend);
    ui.set_menu(MenuBar::new());

    let schedule = scheduler(&ui, &handle);
    ui.set_commit_scheduler({
        let schedule = schedule.clone();
        move || schedule()
    });
    handle.set_wake(move || schedule());
    // Background work finishing on another thread: wake our loop.
    let thread = unsafe { w::GetCurrentThreadId() };
    ui.set_waker(Arc::new(move || unsafe {
        _ = w::PostThreadMessageW(thread, w::WM_NULL as u32, 0, 0);
    }));

    setup(&ui);
    ui.tick();
    handle.show_pending_windows();
    // The session ending asks the app as its own Quit does; its handlers
    // run now, as the answer is due before the query returns.
    crate::session::set_quit({
        let (weak, handle) = (ui.downgrade(), handle.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return true };
            ui.request_quit();
            ui.tick();
            handle.show_pending_windows();
            ui.windows().is_empty()
        }
    });
    loop {
        runtime::pump();
        ui.tick();
        handle.show_pending_windows();
        if ui.windows().is_empty() {
            break;
        }
        runtime::wait(ui.time_to_next_timer());
    }
    // Close everything while XAML is still running.
    drop(ui);
    runtime::pump();
}

/// A tick on the dispatcher queue, at most one pending at a time.
fn scheduler(ui: &Ui, handle: &WinUiHandle) -> Rc<dyn Fn()> {
    let queue = w::DispatcherQueue::GetForCurrentThread().expect("a dispatcher queue on the UI thread");
    let scheduled = Rc::new(Cell::new(false));
    let (weak, handle) = (ui.downgrade(), handle.clone());
    let tick: Rc<dyn Fn()> = Rc::new({
        let scheduled = scheduled.clone();
        move || {
            // Cleared first, whatever happens: a flag left set would stop
            // all later scheduling (live resizing relies on it).
            scheduled.set(false);
            // Pumping from inside a Ui call (a window coming alive): our own
            // loop ticks right after anyway.
            if runtime::nested() {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                ui.tick();
                handle.show_pending_windows();
            }
        }
    });
    Rc::new(move || {
        if scheduled.replace(true) {
            return;
        }
        let tick = tick.clone();
        let queued = queue
            .cast::<w::IDispatcherQueue>()
            .and_then(|q| q.TryEnqueue(&w::DispatcherQueueHandler::new(move || tick())));
        if !queued.unwrap_or(false) {
            scheduled.set(false);
        }
    })
}

/// Prepares WinUI for tests. XAML starts on the calling thread, which must
/// be the thread the tests run on.
pub fn init_for_tests() {
    runtime::init();
}

/// Dispatches pending platform messages. Tests call this while settling:
/// XAML reports some changes (text edits, scrolling, focus) asynchronously.
pub fn pump() {
    runtime::pump();
}
