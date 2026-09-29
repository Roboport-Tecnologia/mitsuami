//! The thread's Win32 set-up: per-monitor DPI, the themed common controls,
//! our window classes, and the message loop's pieces.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{COLOR_BTNFACE, HBRUSH};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{ACTCTXW, ActivateActCtx, CreateActCtxW};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::UI::Controls::{
    ICC_BAR_CLASSES, ICC_PROGRESS_CLASS, ICC_STANDARD_CLASSES, ICC_WIN95_CLASSES, INITCOMMONCONTROLSEX,
};
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DispatchMessageW, HWND_MESSAGE, IDC_ARROW, LoadCursorW, MSG,
    MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, PostMessageW, QS_ALLINPUT,
    RegisterClassExW, TranslateMessage, WM_APP, WNDCLASSEXW, WS_POPUP,
};

/// Our top-level windows, and the hosts children are placed in.
pub(crate) const WINDOW_CLASS: &str = "MitsuamiWin32Window";
pub(crate) const HOST_CLASS: &str = "MitsuamiWin32Host";
const MESSAGE_CLASS: &str = "MitsuamiWin32Messages";

/// Posted to the message window: tick the UI, or run what's waiting.
const WM_TICK: u32 = WM_APP + 1;
const WM_LATER: u32 = WM_APP + 2;

/// Common controls 6 (themed, with `BCM_GETIDEALSIZE`, marquee progress
/// bars and `TBM_SETPOSNOTIFY`): what an app's manifest asks for.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;

type Later = Box<dyn FnOnce()>;

thread_local! {
    static READY: Cell<bool> = const { Cell::new(false) };
    /// A hidden window that holds controls out of the tree.
    static PARKING: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    /// A message-only window for ticks and deferred work, which modal loops
    /// (a window being resized, a dialog) dispatch too.
    static MESSAGES: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    /// The common controls 6 module, for what only it exports.
    static COMCTL: Cell<HMODULE> = const { Cell::new(std::ptr::null_mut()) };
    static TICK: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static TICK_POSTED: Cell<bool> = const { Cell::new(false) };
    static LATER: RefCell<VecDeque<Later>> = const { RefCell::new(VecDeque::new()) };
}

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// Sets up Win32 on this thread. Idempotent.
pub(crate) fn init() {
    if READY.get() {
        return;
    }
    READY.set(true);
    unsafe {
        // Pixels are ours to scale, per monitor. Fails if the process has
        // set it already (its manifest), which is as good.
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    activate_common_controls();
    register_class(WINDOW_CLASS, crate::registry::window_proc);
    register_class(HOST_CLASS, crate::registry::window_proc);
    register_class(MESSAGE_CLASS, message_proc);
    PARKING.set(create(HOST_CLASS, WS_POPUP, std::ptr::null_mut()));
    MESSAGES.set(create(MESSAGE_CLASS, 0, HWND_MESSAGE));
}

/// Activates common controls 6 for the thread, for good, as a manifest
/// would for the process: the manifest goes in a file, since an
/// activation context is made from one. Then asks for the classes we use.
fn activate_common_controls() {
    let path = std::env::temp_dir().join("mitsuami-win32-comctl6.manifest");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(MANIFEST) {
        _ = std::fs::write(&path, MANIFEST);
    }
    let source = wide(&path.to_string_lossy());
    unsafe {
        let context = ACTCTXW { cbSize: size_of::<ACTCTXW>() as u32, lpSource: source.as_ptr(), ..std::mem::zeroed() };
        let handle = CreateActCtxW(&context);
        if handle as isize != -1 {
            let mut cookie = 0;
            ActivateActCtx(handle, &mut cookie);
        }
        // Loaded through the context: version 6, whose classes the
        // controls we create are then. Linking to it would get the system's
        // version 5 instead.
        let module = LoadLibraryW(wide("comctl32.dll").as_ptr());
        COMCTL.set(module);
        type Init = unsafe extern "system" fn(*const INITCOMMONCONTROLSEX) -> i32;
        if let Some(init) = GetProcAddress(module, c"InitCommonControlsEx".as_ptr().cast()) {
            let init: Init = std::mem::transmute(init);
            let classes = INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES | ICC_BAR_CLASSES | ICC_PROGRESS_CLASS | ICC_WIN95_CLASSES,
            };
            init(&classes);
        }
    }
}

/// A function only common controls 6 exports.
pub(crate) fn comctl(name: &std::ffi::CStr) -> Option<unsafe extern "system" fn() -> isize> {
    unsafe { GetProcAddress(COMCTL.get(), name.as_ptr().cast()) }
}

fn register_class(name: &str, proc_: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT) {
    let name = wide(name);
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(proc_),
            hInstance: GetModuleHandleW(std::ptr::null()),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            // The dialog colour, which the controls draw their own
            // backgrounds in.
            hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
            lpszClassName: name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&class);
    }
}

fn create(class: &str, style: u32, parent: HWND) -> HWND {
    let class = wide(class);
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            style,
            0,
            0,
            0,
            0,
            parent,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

/// Where controls wait while they're in no tree.
pub(crate) fn parking() -> HWND {
    PARKING.get()
}

/// The message window, as a number another thread can hold.
pub(crate) fn messages() -> isize {
    MESSAGES.get() as isize
}

/// What a posted tick runs: the run loop's tick. Tests have none.
pub(crate) fn set_tick(tick: Option<Rc<dyn Fn()>>) {
    TICK.set(tick);
}

/// Ticks soon, at most one pending at a time: from our loop, or from the
/// modal loop of a window being moved or resized.
pub(crate) fn schedule_tick() {
    if !TICK_POSTED.replace(true) {
        unsafe { PostMessageW(MESSAGES.get(), WM_TICK, 0, 0) };
    }
}

/// Wakes the UI thread from any thread, to tick.
pub(crate) fn wake(messages: isize) {
    unsafe { PostMessageW(messages as HWND, WM_TICK, 0, 0) };
}

/// Runs `f` from the message loop, outside whatever is running now: for
/// modal UI (a dialog runs a loop of its own).
pub(crate) fn later(f: impl FnOnce() + 'static) {
    LATER.with_borrow_mut(|later| later.push_back(Box::new(f)));
    unsafe { PostMessageW(MESSAGES.get(), WM_LATER, 0, 0) };
}

unsafe extern "system" fn message_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_TICK => {
            TICK_POSTED.set(false);
            if let Some(tick) = TICK.with_borrow(|tick| tick.clone()) {
                tick();
            }
            0
        }
        WM_LATER => {
            while let Some(job) = LATER.with_borrow_mut(|later| later.pop_front()) {
                job();
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// Dispatches every queued message. Tab, Return and Escape go through
/// our keyboard handling first, as a dialog's go through its manager's.
pub(crate) fn pump() {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            if crate::keys::handle(&msg) {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Sleeps until a message arrives or `timeout` passes.
pub(crate) fn wait(timeout: Option<Duration>) {
    let ms = timeout.map_or(INFINITE, |t| t.as_millis().min(u128::from(INFINITE - 1)) as u32);
    unsafe {
        MsgWaitForMultipleObjectsEx(0, std::ptr::null(), ms, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
}
