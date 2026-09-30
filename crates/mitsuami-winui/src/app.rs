//! Running an app: XAML's own event loop, ticking the UI from its
//! dispatcher queue.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use mitsuami_core::services::MenuBar;
use mitsuami_core::{AppInfo, Ui};
use windows_core::Interface;

use crate::backend::{BackendOptions, WinUiBackend, WinUiHandle};
use crate::bindings as w;
use crate::runtime;

thread_local! {
    /// Schedules a tick; set while an app runs.
    static SCHEDULE: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Starts the app: `setup` creates the windows, then XAML's event loop
/// takes over. Returns when the last window closes.
///
/// The loop is the Windows App SDK's (`DispatcherQueue::RunEventLoop`), as
/// in WinUI's own apps: XAML runs its deferred work (async events, its
/// ticks) from dispatcher queue timers, which a `PeekMessage` loop of our
/// own served too slowly, so under steady input (a list's arrow keys) that
/// work piled up and every input event took longer. The UI ticks from the
/// queue, as GTK's backend ticks from an idle source: one tick pending at a
/// time, scheduled by events, commits and background work, and a timer for
/// the next `sleep` deadline. Ticks keep working inside modal loops (window
/// moves and live resizing), which run the queue too.
pub fn run(info: AppInfo, setup: impl FnOnce(&Ui)) {
    let backend = WinUiBackend::new(BackendOptions::default());
    let handle = backend.handle();
    let ui = Ui::new(backend);
    // Before any window: the AppUserModelID must be set before the app
    // shows anything.
    ui.set_app_info(info);
    ui.set_menu(MenuBar::new());

    let queue = w::DispatcherQueue::GetForCurrentThread().expect("a dispatcher queue on the UI thread");
    let turn = turn(&ui, &handle, &queue);
    let schedule = scheduler(&queue, turn.clone());
    SCHEDULE.with(|s| *s.borrow_mut() = Some(schedule.clone()));
    ui.set_commit_scheduler({
        let schedule = schedule.clone();
        move || schedule()
    });
    handle.set_wake(move || schedule());
    // Background work finishing on another thread: a tick on this one.
    ui.set_waker(Arc::new({
        let queue = queue.clone();
        move || {
            let handler = w::DispatcherQueueHandler::new(schedule_tick);
            _ = queue.cast::<w::IDispatcherQueue>().and_then(|q| q.TryEnqueue(&handler));
        }
    }));

    setup(&ui);
    turn();
    // The session ending asks the app as its own Quit does; its handlers
    // run now, as the answer is due before the query returns.
    crate::session::set_quit({
        let (weak, turn) = (ui.downgrade(), turn.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return true };
            ui.request_quit();
            turn();
            ui.windows().is_empty()
        }
    });
    let run = queue.cast::<w::IDispatcherQueue3>().and_then(|q| q.RunEventLoop());
    if let Err(error) = run {
        panic!("winui: XAML's event loop failed: {error}");
    }
    SCHEDULE.with(|s| s.borrow_mut().take());
    // Close everything while XAML is still running.
    drop(ui);
    runtime::pump();
}

/// Schedules a tick of the running app, if there is one.
pub(crate) fn schedule_tick() {
    let schedule = SCHEDULE.with(|s| s.borrow().clone());
    if let Some(schedule) = schedule {
        schedule();
    }
}

/// One tick: the UI's, the windows it made shown, the timer armed for the
/// next deadline, and the loop left once the last window has closed.
fn turn(ui: &Ui, handle: &WinUiHandle, queue: &w::DispatcherQueue) -> Rc<dyn Fn()> {
    let timer = queue.cast::<w::IDispatcherQueue>().and_then(|q| q.CreateTimer()).expect("a dispatcher queue timer");
    let timer = timer.cast::<w::IDispatcherQueueTimer>().expect("a dispatcher queue timer");
    _ = timer.SetIsRepeating(false);
    let ticks = timer.Tick(|_, _| schedule_tick()).expect("a dispatcher queue timer's Tick");
    let (weak, handle, queue) = (ui.downgrade(), handle.clone(), queue.clone());
    Rc::new(move || {
        let _ = &ticks;
        let Some(ui) = weak.upgrade() else { return };
        // Pumping from inside a Ui call (a window coming alive): the tick
        // comes once that pump is done.
        if runtime::nested() {
            runtime::tick_after_nested();
            return;
        }
        ui.tick();
        handle.show_pending_windows();
        _ = timer.Stop();
        if let Some(delay) = ui.time_to_next_timer() {
            let micros = i64::try_from(delay.as_micros()).unwrap_or(i64::MAX);
            _ = timer.SetInterval(windows_time::TimeSpan::from_micros(micros));
            _ = timer.Start();
        }
        if ui.windows().is_empty() {
            _ = queue.cast::<w::IDispatcherQueue3>().and_then(|q| q.EnqueueEventLoopExit());
        }
    })
}

/// A turn on the dispatcher queue, at most one pending at a time.
fn scheduler(queue: &w::DispatcherQueue, turn: Rc<dyn Fn()>) -> Rc<dyn Fn()> {
    let queue = queue.clone();
    let scheduled = Rc::new(Cell::new(false));
    let tick: Rc<dyn Fn()> = Rc::new({
        let scheduled = scheduled.clone();
        move || {
            // Cleared first, whatever happens: a flag left set would stop
            // all later scheduling (live resizing relies on it).
            scheduled.set(false);
            turn();
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
