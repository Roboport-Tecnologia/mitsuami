//! Running an app: GTK initialization and the main loop hook.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use mitsuami_core::services::MenuBar;
use mitsuami_core::{AppInfo, Ui};

use crate::backend::{BackendOptions, GtkBackend};

thread_local! {
    /// Schedules a tick. Background threads reach it through
    /// `MainContext::invoke`, which runs closures on the main thread.
    static SCHEDULE_TICK: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

fn init() {
    gtk::init().expect("mitsuami: cannot initialize GTK (is there a display?)");
    // libadwaita takes the dark preference from the system's color scheme
    // and warns when GTK's older setting is on, as a settings.ini for GTK 3
    // apps often leaves it. Its apps ignore the setting, so clear it first.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(false);
    }
    // libadwaita's widgets (the sidebar's split view), and its style.
    adw::init().expect("mitsuami: cannot initialize libadwaita");
}

/// Starts the app: `setup` creates the windows, then GLib's main loop takes
/// over. Returns when the last window closes.
pub fn run(info: AppInfo, setup: impl FnOnce(&Ui)) {
    // GDK's X11 backend takes the windows' class from it as it starts.
    if let Some(id) = &info.id {
        glib::set_prgname(Some(id.as_str()));
    }
    init();
    let backend = GtkBackend::new(BackendOptions::default());
    let handle = backend.handle();
    let ui = Ui::new(backend);
    ui.set_app_info(info);
    // The standard menu (Quit), until the app installs its own.
    ui.set_menu(MenuBar::new());
    let main_loop = glib::MainLoop::new(None, false);

    // One pending tick at a time, as an idle source that runs ahead of GTK's
    // own layout and drawing (those have lower priorities). After each tick,
    // one timer is re-armed for the next `sleep` deadline.
    let scheduled = Rc::new(Cell::new(false));
    let timer: Rc<Cell<Option<glib::SourceId>>> = Rc::default();
    let schedule: Rc<dyn Fn()> = {
        let (weak, handle, main_loop) = (ui.downgrade(), handle.clone(), main_loop.clone());
        Rc::new(move || {
            if scheduled.replace(true) {
                return;
            }
            let (weak, handle, main_loop, scheduled, timer) =
                (weak.clone(), handle.clone(), main_loop.clone(), scheduled.clone(), timer.clone());
            glib::idle_add_local_full(glib::Priority::HIGH_IDLE, move || {
                scheduled.set(false);
                let Some(ui) = weak.upgrade() else { return glib::ControlFlow::Break };
                ui.tick();
                handle.show_pending_windows();
                if let Some(old) = timer.take() {
                    old.remove();
                }
                if let Some(delay) = ui.time_to_next_timer() {
                    let timer_slot = timer.clone();
                    timer.set(Some(glib::timeout_add_local_once(delay, move || {
                        timer_slot.set(None);
                        schedule_tick();
                    })));
                }
                if ui.windows().is_empty() {
                    main_loop.quit();
                }
                glib::ControlFlow::Break
            });
        })
    };
    SCHEDULE_TICK.with(|s| *s.borrow_mut() = Some(schedule.clone()));
    let on_commit = schedule.clone();
    ui.set_commit_scheduler(move || on_commit());
    handle.set_wake(move || schedule());
    // Tasks woken from other threads (background work finishing).
    ui.set_waker(Arc::new(|| glib::MainContext::default().invoke(schedule_tick)));

    setup(&ui);
    ui.tick();
    handle.show_pending_windows();
    // The session ending asks the app as its Quit does; the answer is due
    // now, so its handlers run in this turn.
    let _session = {
        let (ui, handle, main_loop) = (ui.clone(), handle.clone(), main_loop.clone());
        crate::session::watch(
            Rc::new(move || {
                ui.request_quit();
                ui.tick();
                handle.show_pending_windows();
                ui.windows().is_empty()
            }),
            Rc::new(move || main_loop.quit()),
        )
    };
    schedule_tick();
    if !ui.windows().is_empty() {
        main_loop.run();
    }
    SCHEDULE_TICK.with(|s| s.borrow_mut().take());
}

fn schedule_tick() {
    let schedule = SCHEDULE_TICK.with(|s| s.borrow().clone());
    if let Some(schedule) = schedule {
        schedule();
    }
}

/// Initializes GTK for tests, once per process. Unless
/// `MITSUAMI_SHOW_WINDOWS=1`, windows go to a private Broadway display (see
/// the crate's `display` module).
pub fn init_for_tests() {
    if gtk::is_initialized_main_thread() {
        return;
    }
    if !std::env::var("MITSUAMI_SHOW_WINDOWS").is_ok_and(|v| v == "1") {
        crate::display::start_private_display();
    }
    init();
}
