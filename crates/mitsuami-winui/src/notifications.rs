//! Notifications: the Windows App SDK's app notifications (toasts), shown
//! by the shell and kept in the Notification Center.
//!
//! The app registers with Windows on its first notification, not at start:
//! an unpackaged app's registration writes its id, name and icon to the
//! registry, which an app that never notifies has no need of.
//!
//! An unpackaged app's registration needs the runtime's Singleton package
//! (its long-running process is a COM server), which the full installer
//! adds but the framework package, all a Store app brings, doesn't: there
//! `Register()` answers "class not registered", and the app shows Windows'
//! own toasts (`ToastNotificationManager`), as unpackaged Win32 apps do.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::services::{Notification, ServiceError};
use windows_core::{EventRevoker, HSTRING, Interface, PCWSTR};

use crate::backend::WinUiHandle;
use crate::bindings as w;
use crate::later;

/// The argument a notification carries its id in, which a click hands back.
const ID_ARGUMENT: &str = "mitsuami-id";

/// Windows refuses longer tags.
const MAX_TAG: usize = 64;

/// Every toast's group, for withdrawing by tag.
const GROUP: &str = "mitsuami";

/// What a click reaches: the backend, to bring a window forward, and the
/// `Ui`'s handler.
#[derive(Clone)]
struct Clicked {
    backend: WinUiHandle,
    activated: Rc<dyn Fn(String)>,
}

enum Registered {
    AppSdk {
        manager: w::IAppNotificationManager,
        /// Clicks come here while it lives.
        _invoked: EventRevoker,
    },
    Toasts(Toasts),
}

/// Windows' own toasts, under the app's AppUserModelID.
struct Toasts {
    app_id: String,
    notifier: w::IToastNotifier,
    /// A toast's clicks come to its own object, kept while it may show.
    shown: HashMap<String, (w::ToastNotification, EventRevoker)>,
}

thread_local! {
    /// Once per process, on the UI thread.
    static REGISTERED: RefCell<Option<Registered>> = const { RefCell::new(None) };
    static CLICKED: RefCell<Option<Clicked>> = const { RefCell::new(None) };
    /// `AppInfo::name`, the toasts' sender.
    static APP_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The app's name, for registering its toasts (`set_app_info`).
pub(crate) fn set_app_name(name: Option<&str>) {
    APP_NAME.with(|n| *n.borrow_mut() = name.map(str::to_owned));
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
    register(queue)?;
    CLICKED.with(|c| *c.borrow_mut() = Some(Clicked { backend: backend.clone(), activated }));
    REGISTERED.with(|r| match r.borrow_mut().as_mut() {
        Some(Registered::AppSdk { manager, .. }) => show_app_sdk(manager, notification, &id),
        Some(Registered::Toasts(toasts)) => show_toast(queue, toasts, notification, &id),
        None => Err(ServiceError::Unavailable),
    })
}

fn show_app_sdk(
    manager: &w::IAppNotificationManager,
    notification: &Notification,
    id: &str,
) -> Result<(), ServiceError> {
    match manager.Setting().map_err(failed)? {
        w::AppNotificationSetting::Enabled => {}
        w::AppNotificationSetting::DisabledForApplication | w::AppNotificationSetting::DisabledForUser => {
            return Err(ServiceError::Cancelled);
        }
        // Group policy, the manifest, or `Unsupported`.
        _ => return Err(ServiceError::Unavailable),
    }
    let built = (|| {
        let builder = w::AppNotificationBuilder::new()?.cast::<w::IAppNotificationBuilder>()?;
        // The first text is the title, in bold.
        builder.AddText(&notification.title)?;
        if let Some(body) = &notification.body {
            builder.AddText(body)?;
        }
        builder.AddArgument(ID_ARGUMENT, id)?;
        // The tag alone names it: Windows replaces the one with the same
        // tag (and group, which we leave empty).
        builder.SetTag(&tag(id))?;
        builder.BuildNotification()
    })();
    let notification = built.map_err(failed)?;
    manager.Show(&notification).map_err(failed)
}

/// A toast of the title and the body, as the App SDK's builder writes
/// one, with the id as its launch argument, which a click hands back.
fn show_toast(
    queue: &w::DispatcherQueue,
    toasts: &mut Toasts,
    notification: &Notification,
    id: &str,
) -> Result<(), ServiceError> {
    // Windows reads no setting for an id it knows only from the registry
    // ("element not found"): `Show` answers then.
    match toasts.notifier.Setting().unwrap_or(w::NotificationSetting::Enabled) {
        w::NotificationSetting::Enabled => {}
        w::NotificationSetting::DisabledForApplication | w::NotificationSetting::DisabledForUser => {
            return Err(ServiceError::Cancelled);
        }
        // Group policy or the manifest.
        _ => return Err(ServiceError::Unavailable),
    }
    let mut xml = format!(
        r#"<toast launch="{}"><visual><binding template="ToastGeneric"><text>{}</text>"#,
        escape(id),
        escape(&notification.title)
    );
    if let Some(body) = &notification.body {
        xml += &format!("<text>{}</text>", escape(body));
    }
    xml += "</binding></visual></toast>";
    let shown = (|| {
        let document = w::XmlDocument::new()?;
        document.cast::<w::IXmlDocumentIO>()?.LoadXml(&xml)?;
        let toast = w::ToastNotification::CreateToastNotification(&document)?;
        let second = toast.cast::<w::IToastNotification2>()?;
        second.SetTag(&tag(id))?;
        second.SetGroup(GROUP)?;
        // Clicks come on a worker thread, as the App SDK's do.
        let queue = queue.clone();
        let activated = toast.cast::<w::IToastNotification>()?.Activated(move |_, args| {
            let id = args
                .as_ref()
                .and_then(|args| args.cast::<w::IToastActivatedEventArgs>().ok())
                .and_then(|args| args.Arguments().ok());
            if let Some(id) = id {
                later::on_ui(&queue, move || clicked(id));
            }
        })?;
        toasts.notifier.Show(&toast)?;
        Ok((toast, activated))
    })()
    .map_err(failed)?;
    toasts.shown.insert(id.to_owned(), shown);
    Ok(())
}

/// Takes the notification with this id away, from the screen and the
/// Notification Center. Registers first, so one an earlier run showed
/// goes too.
pub(crate) fn withdraw(queue: &w::DispatcherQueue, id: &str) {
    if register(queue).is_err() {
        return;
    }
    REGISTERED.with(|r| match r.borrow_mut().as_mut() {
        Some(Registered::AppSdk { manager, .. }) => {
            // Done when it's done: there's no answer to wait for.
            _ = manager.RemoveByTagAsync(&tag(id));
        }
        Some(Registered::Toasts(toasts)) => {
            toasts.shown.remove(id);
            if let Ok(history) =
                w::ToastNotificationManager::History().and_then(|h| h.cast::<w::IToastNotificationHistory>())
            {
                _ = history.RemoveGroupedTagWithId(&tag(id), GROUP, &toasts.app_id);
            }
        }
        None => {}
    });
}

/// Lets Windows launch the app again for a click once it has quit, as the
/// Windows App SDK asks of apps before they exit.
pub(crate) fn unregister() {
    if let Some(Registered::AppSdk { manager, .. }) = REGISTERED.with(|r| r.borrow_mut().take()) {
        _ = manager.Unregister();
    }
}

/// Registers this process, once. The click handler goes in first, as the
/// SDK requires, so clicks come to this process instead of launching
/// another; a failed registration drops it, to try again.
fn register(queue: &w::DispatcherQueue) -> Result<(), ServiceError> {
    if REGISTERED.with(|r| r.borrow().is_some()) {
        return Ok(());
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
    let registered = match manager.Register() {
        Ok(()) => Registered::AppSdk { manager, _invoked: invoked },
        // No Singleton package (the module's comment). A package always
        // registers: Windows knows its identity.
        Err(error) if error.code() == w::REGDB_E_CLASSNOTREG && !crate::backend::packaged() => {
            drop(invoked);
            Registered::Toasts(toasts()?)
        }
        Err(error) => return Err(failed(error)),
    };
    REGISTERED.with(|r| *r.borrow_mut() = Some(registered));
    Ok(())
}

/// Windows shows an unpackaged app's toasts for an AppUserModelID it finds
/// in the registry, with the name there, as the App SDK registers one: the
/// process's (`AppInfo::id`), else one made from the executable's path,
/// as the App SDK makes its own.
fn toasts() -> Result<Toasts, ServiceError> {
    let exe = std::env::current_exe().ok();
    let app_id = process_app_id()
        .or_else(|| exe.as_ref().map(|exe| format!("mitsuami-{:016x}", fnv(exe.to_string_lossy().as_bytes()))))
        .ok_or(ServiceError::Unavailable)?;
    let name = APP_NAME
        .with(|n| n.borrow().clone())
        .or_else(|| exe.as_ref().and_then(|exe| exe.file_stem()).map(|stem| stem.to_string_lossy().into_owned()))
        .unwrap_or_else(|| app_id.clone());
    let key = format!(r"Software\Classes\AppUserModelId\{app_id}");
    set_value(&key, "DisplayName", &name)?;
    // Clicks while the app runs come to the toast itself. The App SDK's
    // failed registration left its activator, which no one serves now: a
    // click would launch the executable again instead.
    let (key, activator) = (HSTRING::from(key), HSTRING::from("CustomActivator"));
    unsafe { w::RegDeleteKeyValueW(w::HKEY_CURRENT_USER, PCWSTR(key.as_ptr()), PCWSTR(activator.as_ptr())) };
    let notifier = w::ToastNotificationManager::CreateToastNotifierWithId(&app_id)
        .and_then(|n| n.cast::<w::IToastNotifier>())
        .map_err(failed)?;
    Ok(Toasts { app_id, notifier, shown: HashMap::new() })
}

/// A string under `HKEY_CURRENT_USER`, making the key if it's missing.
fn set_value(key: &str, name: &str, value: &str) -> Result<(), ServiceError> {
    let (key, name, value) = (HSTRING::from(key), HSTRING::from(name), HSTRING::from(value));
    let status = unsafe {
        w::RegSetKeyValueW(
            w::HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            w::REG_SZ,
            value.as_ptr().cast(),
            ((value.len() + 1) * 2) as u32,
        )
    };
    match status {
        0 => Ok(()),
        error => Err(ServiceError::Failed(std::io::Error::from_raw_os_error(error).to_string())),
    }
}

fn process_app_id() -> Option<String> {
    let mut id = windows_core::PWSTR::null();
    unsafe {
        w::GetCurrentProcessExplicitAppUserModelID(&mut id).ok().ok()?;
        let text = id.to_string().ok();
        w::CoTaskMemFree(id.0.cast());
        text
    }
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
    format!("mitsuami-{:016x}", fnv(id.as_bytes()))
}

/// FNV-1a, the same in every build and Rust release: a later version of
/// the app may withdraw what an earlier one showed.
fn fnv(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// Text as toast XML takes it.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn failed(error: windows_core::Error) -> ServiceError {
    ServiceError::Failed(error.message())
}
