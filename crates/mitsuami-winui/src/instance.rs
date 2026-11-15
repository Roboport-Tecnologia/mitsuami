//! Single instance, as the Windows App SDK has it: `AppInstance` keyed by
//! the app's id. A later launch redirects its activation to the instance
//! that registered the key, and ends; the running one gets `Activated`.
//! The SDK asks for this before XAML starts, so it's the first thing
//! `run` does once the runtime is there.

use std::cell::RefCell;
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::services::Reopen;
use windows_core::{EventRevoker, HSTRING, Interface, PCWSTR};

use crate::backend::WinUiHandle;
use crate::bindings as w;
use crate::later;
use launch::ILaunchActivatedEventArgs;

type Reopened = (WinUiHandle, Rc<dyn Fn(Reopen)>);

thread_local! {
    /// The running instance's `Activated` handler, while it runs.
    static ACTIVATED: RefCell<Option<EventRevoker>> = const { RefCell::new(None) };
    /// Where later launches go: the backend, to bring a window forward, and
    /// the `Ui`.
    static REOPENED: RefCell<Option<Reopened>> = const { RefCell::new(None) };
}

/// Registers this process for the app's id, or, in a later launch, hands
/// the launch to the process that has it. Returns whether this process
/// should go on and run the app. Where the SDK can't tell, the app runs
/// as its own instance.
pub(crate) fn claim(id: &str) -> bool {
    match redirect(id) {
        Ok(redirected) => !redirected,
        Err(error) => {
            eprintln!("mitsuami: {id} runs as its own instance, since it can't look for others: {error}");
            true
        }
    }
}

/// Whether the launch went to another instance.
fn redirect(id: &str) -> windows_core::Result<bool> {
    let found = w::AppInstance::FindOrRegisterForKey(id)?.cast::<w::IAppInstance>()?;
    if found.IsCurrent()? {
        return Ok(false);
    }
    let args = w::AppInstance::GetCurrent()?.cast::<w::IAppInstance>()?.GetActivatedEventArgs()?;
    // Windows lets a process take the foreground only when the one in front
    // allows it: this one is, as the user just launched it.
    unsafe { _ = w::AllowSetForegroundWindow(found.ProcessId()?) };
    wait(found.RedirectActivationToAsync(&args)?)?;
    Ok(true)
}

/// Waits for `action` without blocking this thread's single-threaded
/// apartment, which the SDK calls back into to finish the redirect: a
/// plain wait here never returned, and the launch never ended. Another
/// thread waits, in the multithreaded apartment, while this one keeps
/// dispatching messages (how COM reaches it), as the SDK's samples do.
fn wait(action: windows_future::IAsyncAction) -> windows_core::Result<()> {
    let waiter = std::thread::spawn(move || {
        let com = unsafe { w::CoInitializeEx(std::ptr::null(), w::COINIT_MULTITHREADED as u32) }.is_ok();
        let done = action.join();
        if com {
            unsafe { w::CoUninitialize() };
        }
        done
    });
    let thread = waiter.as_raw_handle() as w::HANDLE;
    // WAIT_OBJECT_0: the thread has ended; anything else, a message came.
    while unsafe {
        w::MsgWaitForMultipleObjectsEx(1, &thread, w::INFINITE, w::QS_ALLINPUT as u32, w::MWMO_INPUTAVAILABLE as u32)
    } != 0
    {
        crate::runtime::pump();
    }
    waiter.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// Connects the running instance to the app: later launches bring its
/// window forward and reach `reopened`.
pub(crate) fn connect(queue: &w::DispatcherQueue, backend: &WinUiHandle, reopened: Rc<dyn Fn(Reopen)>) {
    REOPENED.with(|r| *r.borrow_mut() = Some((backend.clone(), reopened)));
    let queue = queue.clone();
    let current = w::AppInstance::GetCurrent().and_then(|current| current.cast::<w::IAppInstance>());
    // Activations arrive on a worker thread: what they carry is read there,
    // and goes to the UI thread as plain values.
    let revoker = current.and_then(|current| {
        current.Activated(move |_, args| {
            let Some(args) = args.as_ref() else { return };
            let reopen = read(args);
            later::on_ui(&queue, move || reopened_on_ui(reopen));
        })
    });
    match revoker {
        Ok(revoker) => ACTIVATED.with(|a| *a.borrow_mut() = Some(revoker)),
        Err(error) => eprintln!("mitsuami: later launches can't reach this app: {error}"),
    }
}

/// Stops listening, as `run` ends.
pub(crate) fn disconnect() {
    ACTIVATED.with(|a| a.borrow_mut().take());
    REOPENED.with(|r| r.borrow_mut().take());
}

/// A launch's arguments (its command line, without the program), or a
/// file activation's files. The SDK passes no working folder.
fn read(args: &w::AppActivationArguments) -> (Vec<String>, Vec<PathBuf>) {
    let Ok(args) = args.cast::<w::IAppActivationArguments>() else { return Default::default() };
    let (Ok(kind), Ok(data)) = (args.Kind(), args.Data()) else { return Default::default() };
    match kind {
        w::ExtendedActivationKind::Launch => {
            let line = data.cast::<ILaunchActivatedEventArgs>().and_then(|launch| launch.Arguments());
            // An unpackaged app's launch carries its whole command line, a
            // packaged one's only the arguments (both ends are the same app,
            // so this process can tell for the other).
            let program = !crate::backend::new_window::packaged();
            (line.map(|line| split(&line, program)).unwrap_or_default(), Vec::new())
        }
        w::ExtendedActivationKind::File => {
            let files = data.cast::<w::IFileActivatedEventArgs>().and_then(|file| file.Files());
            let paths = files
                .map(|files| files.into_iter().filter_map(|item| item.Path().ok()).map(PathBuf::from).collect())
                .unwrap_or_default();
            (Vec::new(), paths)
        }
        _ => Default::default(),
    }
}

/// Splits a command line as the C runtime does (`CommandLineToArgvW`),
/// without the program. A line of arguments alone gets a stand-in program
/// first: the first token is parsed by the program's rules (no escapes,
/// ended by the first space outside quotes), and an empty line gives the
/// executable's path.
fn split(line: &str, program: bool) -> Vec<String> {
    let line = if program { line.to_owned() } else { format!("x {line}") };
    let wide = HSTRING::from(line);
    let mut count = 0;
    let argv = unsafe { w::CommandLineToArgvW(PCWSTR(wide.as_ptr()), &mut count) };
    if argv.is_null() {
        return Vec::new();
    }
    let args = (1..count.max(0) as usize).map(|i| unsafe { (*argv.add(i)).to_string() }.unwrap_or_default()).collect();
    unsafe { _ = w::LocalFree(argv as w::HLOCAL) };
    args
}

fn reopened_on_ui((args, files): (Vec<String>, Vec<PathBuf>)) {
    let Some((backend, reopened)) = REOPENED.with(|r| r.borrow().clone()) else { return };
    bring_forward(&backend);
    reopened(Reopen::new(args, None, files));
}

/// Brings the app's window forward, restoring it if it's minimized, as
/// Windows does for its own apps; with no window, the one the app opens
/// takes the foreground this process was allowed.
pub(crate) fn bring_forward(backend: &WinUiHandle) {
    let Some(hwnd) = backend.window_parts(None, |p| p.hwnd as isize) else { return };
    raise(hwnd as w::HWND);
}

/// Restores a window if it's minimized and makes it the foreground one,
/// which Windows allows a process that has the foreground or was given
/// it: the tray's menu takes it (`SetForegroundWindow` before
/// `TrackPopupMenuEx`), and a later launch hands it on.
pub(crate) fn raise(hwnd: w::HWND) {
    unsafe {
        if w::IsIconic(hwnd).as_bool() {
            _ = w::ShowWindow(hwnd, w::SW_RESTORE);
        }
        _ = w::SetForegroundWindow(hwnd);
    }
}

/// Windows.ApplicationModel.Activation.ILaunchActivatedEventArgs, what a
/// launch's activation carries, as windows-bindgen writes it. Not in the
/// generated bindings: they're flat, and XAML's interface of the same name
/// is there already.
#[allow(non_snake_case)]
mod launch {
    use windows_core::{HSTRING, Interface};

    windows_core::imp::define_interface!(
        ILaunchActivatedEventArgs,
        ILaunchActivatedEventArgs_Vtbl,
        0xfbc93e26_a14a_4b4f_82b0_33bed920af52
    );
    windows_core::imp::interface_hierarchy!(
        ILaunchActivatedEventArgs,
        windows_core::IUnknown,
        windows_core::IInspectable
    );

    impl ILaunchActivatedEventArgs {
        pub(super) fn Arguments(&self) -> windows_core::Result<String> {
            unsafe {
                let mut result__ = core::mem::zeroed();
                (Interface::vtable(self).Arguments)(Interface::as_raw(self), &mut result__).map(|| {
                    let hstring: HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                })
            }
        }
    }

    #[repr(C)]
    pub struct ILaunchActivatedEventArgs_Vtbl {
        pub base__: windows_core::IInspectable_Vtbl,
        pub Arguments:
            unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    }
}
