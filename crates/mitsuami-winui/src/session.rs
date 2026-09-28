//! The session ending (signing out, shutting down): Windows asks each
//! top-level window with `WM_QUERYENDSESSION`. The app decides, as for its
//! own Quit (`Ui::request_quit`), once per session end. If it keeps a
//! window, the answer is no, with a reason Windows shows on its "these
//! apps are preventing shutdown" screen, as Notepad does with unsaved
//! changes; the user can still end it anyway.

use std::cell::{Cell, RefCell};

use crate::bindings as w;

thread_local! {
    /// Asks the app to quit, ticks, and says whether it did (no window
    /// is left). Set by `run`; tests have none, and answer yes.
    static QUIT: RefCell<Option<Box<dyn Fn() -> bool>>> = const { RefCell::new(None) };
    /// The answer for the session end under way, given to every window.
    static ANSWER: Cell<Option<bool>> = const { Cell::new(None) };
    /// The app is being asked: a window queried meanwhile waits for no one.
    static ASKING: Cell<bool> = const { Cell::new(false) };
    /// The window holding the reason shown while the app blocks.
    static BLOCKING: Cell<Option<isize>> = const { Cell::new(None) };
}

/// What the app answers with, in `run`.
pub(crate) fn set_quit(quit: impl Fn() -> bool + 'static) {
    QUIT.with(|q| *q.borrow_mut() = Some(Box::new(quit)));
}

/// Answers the session's queries for a top-level window.
pub(crate) fn watch(hwnd: w::HWND) {
    unsafe { _ = w::SetWindowSubclass(hwnd, Some(session_proc), 0, 0) };
}

unsafe extern "system" fn session_proc(
    hwnd: w::HWND,
    message: u32,
    wparam: w::WPARAM,
    lparam: w::LPARAM,
    _: usize,
    _: usize,
) -> w::LRESULT {
    if message == w::WM_NCDESTROY as u32 {
        unsafe { _ = w::RemoveWindowSubclass(hwnd, Some(session_proc), 0) };
        if BLOCKING.get() == Some(hwnd as isize) {
            BLOCKING.set(None);
        }
    } else if message == w::WM_QUERYENDSESSION as u32 {
        return if query(hwnd) { 1 } else { 0 };
    } else if message == w::WM_ENDSESSION as u32 {
        // The session end is over: it goes on (the process is ended soon
        // after), or was cancelled, and the app no longer holds it up.
        ANSWER.set(None);
        if wparam == 0 {
            unblock();
        }
    }
    unsafe { w::DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Whether the session may end: the app is asked once, and every window
/// gets its answer.
fn query(hwnd: w::HWND) -> bool {
    if let Some(answer) = ANSWER.get() {
        return answer;
    }
    let has_hook = QUIT.with(|q| q.borrow().is_some());
    if !has_hook {
        return true;
    }
    // Asked while the app is being asked, or from inside a Ui call (a pump
    // nested in one), where it can't be: hold the session up, and say why.
    let quit = if ASKING.get() || crate::runtime::nested() {
        false
    } else {
        ASKING.set(true);
        let quit = QUIT.with(|q| q.borrow().as_ref().is_some_and(|quit| quit()));
        ASKING.set(false);
        ANSWER.set(Some(quit));
        quit
    };
    if quit {
        unblock();
    } else {
        block(hwnd);
    }
    quit
}

fn block(hwnd: w::HWND) {
    if BLOCKING.get().is_some() {
        return;
    }
    let reason = windows_core::w!("Asking before it quits");
    if unsafe { w::ShutdownBlockReasonCreate(hwnd, reason) }.as_bool() {
        BLOCKING.set(Some(hwnd as isize));
    }
}

fn unblock() {
    if let Some(hwnd) = BLOCKING.take() {
        unsafe { _ = w::ShutdownBlockReasonDestroy(hwnd as w::HWND) };
    }
}
