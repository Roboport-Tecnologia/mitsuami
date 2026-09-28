//! Running an app: Qt initialization and the event loop hook.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use mitsuami_core::services::MenuBar;
use mitsuami_core::{AppInfo, Ui};

use crate::backend::{BackendOptions, KirigamiBackend};
use crate::ffi;

fn init() {
    if !ffi::is_initialized() {
        ffi::init();
    }
}

/// Starts the app: `setup` creates the windows, then Qt's event loop takes
/// over. Returns when the last window closes.
pub fn run(info: AppInfo, setup: impl FnOnce(&Ui)) {
    init();
    let backend = KirigamiBackend::new(BackendOptions::default());
    let handle = backend.handle();
    let ui = Ui::new(backend);
    ui.set_app_info(info);
    // The standard menu (Quit), until the app installs its own.
    ui.set_menu(MenuBar::new());

    // The UI ticks whenever Qt's loop is about to sleep, and anything that
    // needs a tick wakes the loop, so it goes round once more. After each
    // tick, one timer is re-armed for the next `sleep` deadline.
    let timer = Rc::new(ffi::Timer::new(ffi::wake));
    let ticking = Rc::new(Cell::new(false));
    let running = Rc::new(RefCell::new(true));
    {
        let (weak, handle, timer, running, ticking) =
            (ui.downgrade(), handle.clone(), timer.clone(), running.clone(), ticking.clone());
        ffi::watch_loop(move || {
            if ticking.replace(true) || !*running.borrow() {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                ui.tick();
                handle.show_pending_windows();
                timer.stop();
                if let Some(delay) = ui.time_to_next_timer() {
                    timer.start(delay.as_millis().min(i32::MAX as u128) as i32);
                }
                if ui.windows().is_empty() {
                    *running.borrow_mut() = false;
                    ffi::quit();
                }
            }
            ticking.set(false);
        });
    }
    // The session ending (logging out) is the app's Quit, as the drawer's
    // is, and runs now: the answer is due before the signal returns. A
    // window left open keeps the session. Inside a tick (Qt's loop turned
    // from within one) the Ui can't run, and the app keeps the session.
    {
        let (weak, handle, ticking) = (ui.downgrade(), handle.clone(), ticking.clone());
        ffi::watch_session_end(move || {
            let Some(ui) = weak.upgrade() else { return false };
            if ticking.replace(true) {
                return true;
            }
            ui.request_quit();
            ui.tick();
            handle.show_pending_windows();
            ticking.set(false);
            !ui.windows().is_empty()
        });
    }
    ui.set_commit_scheduler(ffi::wake);
    handle.set_wake(ffi::wake);
    // Tasks woken from other threads (background work finishing).
    ui.set_waker(Arc::new(ffi::wake));

    setup(&ui);
    ui.tick();
    handle.show_pending_windows();
    if !ui.windows().is_empty() {
        ffi::exec();
    }
    *running.borrow_mut() = false;
}

/// Initializes Qt for tests, once per process. Unless
/// `MITSUAMI_SHOW_WINDOWS=1`, windows go to Qt's offscreen platform: they
/// get exactly the size they ask for, and nothing else competes for focus.
/// The desktop's settings don't leak into tests: its platform theme is off,
/// and KDE's settings (`kdeglobals`) are a private file with animations off,
/// so captures never catch a control mid-animation. Plasma's default font
/// is used.
pub fn init_for_tests() {
    if ffi::is_initialized() {
        return;
    }
    let config = std::env::temp_dir().join(format!("mitsuami-kirigami-config-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&config);
    let _ = std::fs::write(config.join("kdeglobals"), "[KDE]\nAnimationDurationFactor=0\n");
    // SAFETY: before Qt starts, while the test runner is single-threaded.
    unsafe {
        if !std::env::var("MITSUAMI_SHOW_WINDOWS").is_ok_and(|v| v == "1") {
            std::env::set_var("QT_QPA_PLATFORM", "offscreen");
        }
        std::env::remove_var("QT_QPA_PLATFORMTHEME");
        std::env::remove_var("QT_QUICK_CONTROLS_STYLE");
        std::env::set_var("XDG_CONFIG_HOME", &config);
    }
    init();
    crate::theme::use_default_font();
}
