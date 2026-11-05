//! Notifications: the Windows App SDK's app notifications (toasts), shown
//! by the shell and kept in the Notification Center.
//!
//! The app registers with Windows on its first notification, not at start:
//! an unpackaged app's registration writes its id, name and icon to the
//! registry, which an app that never notifies has no need of.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::services::{Notification, ServiceError};
use windows_core::{EventRevoker, Interface};

use crate::backend::WinUiHandle;
use crate::bindings as w;
use crate::later;

/// The argument a notification carries its id in, which a click hands back.
const ID_ARGUMENT: &str = "mitsuami-id";

/// Windows refuses longer tags.
const MAX_TAG: usize = 64;

/// What a click reaches: the backend, to bring a window forward, and the
/// `Ui`'s handler.
#[derive(Clone)]
struct Clicked {
    backend: WinUiHandle,
    activated: Rc<dyn Fn(String)>,
}

struct Registered {
    manager: w::IAppNotificationManager,
    /// Clicks come here while it lives.
    _invoked: EventRevoker,
}

thread_local! {
    /// Once per process, on the UI thread.
    static REGISTERED: RefCell<Option<Registered>> = const { RefCell::new(None) };
    static CLICKED: RefCell<Option<Clicked>> = const { RefCell::new(None) };
}

/// Shows `notification`, replacing the one with its id. `Cancelled` when
/// the user turned the app's notifications (or all of them) off,
/// `Unavailable` when Windows can't show them: policy, the package's
/// manifest, or no notification platform at all.
pub(crate) fn show(
    queue: &w::DispatcherQueue,
    backend: &WinUiHandle,
    notification: &Notification,
    activated: Rc<dyn Fn(String)>,
) -> Result<(), ServiceError> {
    let id = notification.id.clone().unwrap_or_default();
    let manager = registered(queue)?;
    match manager.Setting().map_err(failed)? {
        w::AppNotificationSetting::Enabled => {}
        w::AppNotificationSetting::DisabledForApplication | w::AppNotificationSetting::DisabledForUser => {
            return Err(ServiceError::Cancelled);
        }
        // Group policy, the manifest, or `Unsupported`.
        _ => return Err(ServiceError::Unavailable),
    }
    CLICKED.with(|c| *c.borrow_mut() = Some(Clicked { backend: backend.clone(), activated }));
    let built = (|| {
        let builder = w::AppNotificationBuilder::new()?.cast::<w::IAppNotificationBuilder>()?;
        // The first text is the title, in bold.
        builder.AddText(&notification.title)?;
        if let Some(body) = &notification.body {
            builder.AddText(body)?;
        }
        builder.AddArgument(ID_ARGUMENT, &id)?;
        // The tag alone names it: Windows replaces the one with the same
        // tag (and group, which we leave empty).
        builder.SetTag(&tag(&id))?;
        builder.BuildNotification()
    })();
    let notification = built.map_err(failed)?;
    manager.Show(&notification).map_err(failed)
}

/// Takes the notification with this id away, from the screen and the
/// Notification Center. Registers first, so one an earlier run showed
/// goes too.
pub(crate) fn withdraw(queue: &w::DispatcherQueue, id: &str) {
    if let Ok(manager) = registered(queue) {
        // Done when it's done: there's no answer to wait for.
        _ = manager.RemoveByTagAsync(&tag(id));
    }
}

/// Lets Windows launch the app again for a click once it has quit, as the
/// Windows App SDK asks of apps before they exit.
pub(crate) fn unregister() {
    if let Some(registered) = REGISTERED.with(|r| r.borrow_mut().take()) {
        _ = registered.manager.Unregister();
    }
}

/// The manager, registered for this process. The click handler goes in
/// first, as the SDK requires, so clicks come to this process instead of
/// launching another; a failed registration drops it, to try again.
fn registered(queue: &w::DispatcherQueue) -> Result<w::IAppNotificationManager, ServiceError> {
    if let Some(manager) = REGISTERED.with(|r| r.borrow().as_ref().map(|r| r.manager.clone())) {
        return Ok(manager);
    }
    // False where the platform has none, as for an elevated unpackaged app.
    if !w::AppNotificationManager::IsSupported().unwrap_or(false) {
        return Err(ServiceError::Unavailable);
    }
    let manager = w::AppNotificationManager::Default()
        .and_then(|m| m.cast::<w::IAppNotificationManager>())
        .map_err(|_| ServiceError::Unavailable)?;
    let invoked = manager.NotificationInvoked(on_invoked(queue.clone())).map_err(failed)?;
    // Unpackaged, Windows makes the app an identity from its
    // AppUserModelID (set from `AppInfo::id` before any window) or its
    // executable, and takes its name and icon from the shell.
    manager.Register().map_err(failed)?;
    REGISTERED.with(|r| *r.borrow_mut() = Some(Registered { manager: manager.clone(), _invoked: invoked }));
    Ok(manager)
}

/// Clicks arrive on a worker thread: the handler holds only the UI
/// thread's queue, and hands it the id.
fn on_invoked(
    queue: w::DispatcherQueue,
) -> impl Fn(windows_core::Ref<w::AppNotificationManager>, windows_core::Ref<w::AppNotificationActivatedEventArgs>)
+ Send
+ Sync
+ 'static {
    move |_, args| {
        let id = args
            .as_ref()
            .and_then(|args| args.cast::<w::IAppNotificationActivatedEventArgs2>().ok())
            .and_then(|args| args.Arguments().ok())
            .and_then(|arguments| arguments.Lookup(&windows_core::HSTRING::from(ID_ARGUMENT)).ok());
        if let Some(id) = id {
            let id = id.to_string_lossy();
            later::on_ui(&queue, move || clicked(id));
        }
    }
}

/// Brings the app's window forward, then tells the app. Windows lets the
/// process it activates for a click take the foreground, as it does for
/// its own apps' notifications; a minimized window is restored first.
fn clicked(id: String) {
    let Some(Clicked { backend, activated }) = CLICKED.with(|c| c.borrow().clone()) else { return };
    if let Some(hwnd) = backend.window_parts(None, |p| p.hwnd as isize) {
        let hwnd = hwnd as w::HWND;
        unsafe {
            if w::IsIconic(hwnd).as_bool() {
                _ = w::ShowWindow(hwnd, w::SW_RESTORE);
            }
            _ = w::SetForegroundWindow(hwnd);
        }
    }
    activated(id);
}

/// The id as a tag: as it is when Windows takes it, else a hash of it.
fn tag(id: &str) -> String {
    if id.encode_utf16().count() <= MAX_TAG {
        return id.to_owned();
    }
    // FNV-1a, the same in every build and Rust release: a later version of
    // the app may withdraw what an earlier one showed.
    let hash = id
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3));
    format!("mitsuami-{hash:016x}")
}

fn failed(error: windows_core::Error) -> ServiceError {
    ServiceError::Failed(error.message())
}
