//! Running an app: Qt initialization and the event loop hook.

use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use mitsuami_core::services::MenuBar;
use mitsuami_core::{AppInfo, RunOptions, Ui};

use crate::backend::{BackendOptions, KirigamiBackend};
use crate::ffi;

fn init() {
    if !ffi::is_initialized() {
        ffi::init();
    }
}

/// Tells the user the app can't run, in a `QMessageBox::critical` (KDE's
/// apps show theirs with `KMessageBox::error`, the same widget dialog),
/// the title its main text and the message its informative text, and
/// returns once they dismiss it. Qt starts for it if it hasn't.
pub fn fatal_error(title: &str, message: &str) {
    init();
    ffi::fatal_error(title, message);
}

/// Tests: accept the next [`fatal_error`] once it's on screen, as its OK
/// button does. Call before it, which blocks.
#[doc(hidden)]
pub fn test_accept_next_fatal_error() {
    init();
    ffi::test_accept_next_fatal_error();
}

/// Tests: the title and message the fatal error
/// [`test_accept_next_fatal_error`] accepted showed in its content (not its
/// title bar, which Plasma may not show), or `None` if none was.
#[doc(hidden)]
pub fn test_accepted_fatal_error() -> Option<(String, String)> {
    ffi::test_accepted_fatal_error()
}

/// Starts the app: `setup` creates the windows, then Qt's event loop takes
/// over. Returns when the last window closes.
pub fn run(info: AppInfo, setup: impl FnOnce(&Ui)) {
    run_with(info, RunOptions::default(), setup);
}

/// [`run`], with options. A single-instance app owns its id on the
/// session bus once Qt has started, exporting `org.freedesktop.Application`
/// as KDE apps' `KDBusService(Unique)` does; a later launch calls the
/// running one's `Activate` with its arguments, and returns.
pub fn run_with(info: AppInfo, options: RunOptions, setup: impl FnOnce(&Ui)) {
    // Before Qt starts: its Wayland plugin takes the token from there.
    let token = std::env::var("XDG_ACTIVATION_TOKEN").ok().filter(|token| !token.is_empty());
    // X11's, which Qt's xcb plugin takes from the environment as it starts.
    let startup_id = std::env::var("DESKTOP_STARTUP_ID").ok().filter(|id| !id.is_empty());
    init();
    if options.single_instance
        && let Some(id) = &info.id
    {
        // Lossy, as D-Bus strings are UTF-8: `args()` would panic on a
        // name in another encoding.
        let args: Vec<String> = std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
        let cwd = std::env::current_dir().ok().map(|cwd| cwd.to_string_lossy().into_owned());
        match ffi::claim_instance(id, token.as_deref(), startup_id.as_deref(), &args, cwd.as_deref()) {
            ffi::Instance::Primary => {}
            ffi::Instance::HandedOver => return,
            ffi::Instance::Unclaimed(why) => {
                eprintln!("mitsuami: {id} runs as its own instance, since it can't look for others: {why}");
            }
            // The running one is there but didn't take it: running a
            // second instance would do what single instance prevents.
            ffi::Instance::Failed(why) => {
                eprintln!("mitsuami: couldn't hand this launch to the running {id}: {why}");
                return;
            }
        }
    }
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
                if ui.should_end() {
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

    {
        let weak = ui.downgrade();
        ffi::on_reopen(move |reopen| {
            if let Some(ui) = weak.upgrade() {
                ui.reopen(reopen);
            }
        });
    }
    setup(&ui);
    ui.tick();
    handle.show_pending_windows();
    if !ui.should_end() {
        ffi::exec();
    }
    *running.borrow_mut() = false;
}

/// A new directory of this process's own, only it can read, for files Qt
/// reads: in the user's runtime directory if there is one. Never one that
/// was there already: another user could have made it, with links in it
/// that we'd write through, or settings of theirs for us to read.
pub(crate) fn private_dir(name: &str) -> io::Result<PathBuf> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(std::env::temp_dir);
    let pid = std::process::id();
    let mut taken = None;
    for attempt in 0..100 {
        let dir = base.join(format!("{name}-{pid}-{attempt}"));
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => taken = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(taken.expect("tried at least once"))
}

/// Writes a file that mustn't be there yet, in a [`private_dir`].
pub(crate) fn write_new(path: &Path, contents: &str) -> io::Result<()> {
    fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?.write_all(contents.as_bytes())
}

/// Initializes Qt for tests, once per process. Unless
/// `MITSUAMI_SHOW_WINDOWS=1`, windows go to Qt's offscreen platform: they
/// get exactly the size they ask for, on a 1920×1080 screen, and nothing
/// else competes for focus.
/// The desktop's settings don't leak into tests: its platform theme is off,
/// and KDE's settings (`kdeglobals`) are a private file with animations off,
/// so captures never catch a control mid-animation. Plasma's default font
/// is used.
pub fn init_for_tests() {
    if ffi::is_initialized() {
        return;
    }
    let config = private_dir("mitsuami-kirigami-config").expect("mitsuami: a directory for Qt's test settings");
    write_new(&config.join("kdeglobals"), "[KDE]\nAnimationDurationFactor=0\n").expect("mitsuami: kdeglobals");
    // The offscreen screen is 800×600 unless told otherwise, too small for
    // a sidebar beside a content's minimum: a common desktop's instead.
    let screens = config.join("offscreen.json");
    write_new(
        &screens,
        r#"{"screens": [{"name": "Offscreen", "x": 0, "y": 0, "width": 1920, "height": 1080, "logicalDpi": 96}]}"#,
    )
    .expect("mitsuami: the offscreen screen's settings");
    // SAFETY: before Qt starts, while the test runner is single-threaded.
    unsafe {
        if !std::env::var("MITSUAMI_SHOW_WINDOWS").is_ok_and(|v| v == "1") {
            std::env::set_var("QT_QPA_PLATFORM", format!("offscreen:configfile={}", screens.display()));
        }
        std::env::remove_var("QT_QPA_PLATFORMTHEME");
        std::env::remove_var("QT_QUICK_CONTROLS_STYLE");
        std::env::set_var("XDG_CONFIG_HOME", &config);
        // Every desktop session names its desktop, and KDE's colour schemes
        // colour icons only in one: a container without a session (CI's)
        // would draw them all in the icon's own colours. Empty is no
        // desktop in particular, as an unset one is to Qt.
        if std::env::var_os("XDG_CURRENT_DESKTOP").is_none() {
            std::env::set_var("XDG_CURRENT_DESKTOP", "");
        }
    }
    init();
    crate::theme::use_default_font();
}
