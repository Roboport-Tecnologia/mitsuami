//! Clipboard, dialogs, the Recycle Bin, launching and the menu bar on
//! Windows.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::NodeId;
use mitsuami_core::services::{
    Alert, FileFilter, Launch, MenuBarData, OpenFile, Reply, SaveFile, ServiceError, Services, existing_folder,
};
use windows_core::{HSTRING, Interface};

use crate::backend::{WinUiHandle, boxed};
use crate::bindings as w;
use crate::later;

type R<T> = windows_core::Result<T>;

struct QueuedAlert {
    parent: Option<NodeId>,
    alert: Alert,
    reply: Reply<usize>,
}

/// Alerts wait their turn: XAML shows one `ContentDialog` at a time.
#[derive(Default)]
struct AlertQueue {
    showing: bool,
    /// The one showing: the window it's on, and whether that window went,
    /// which makes its answer the cancel one.
    shown: Option<ShownAlert>,
    waiting: VecDeque<QueuedAlert>,
}

struct ShownAlert {
    window: NodeId,
    operation: windows_future::IAsyncOperation<w::ContentDialogResult>,
    cancelled: bool,
}

/// A picker waiting for its answer: the window it was made for, its reply's
/// ticket, and what cancels it.
struct OpenPicker {
    window: NodeId,
    ticket: u64,
    cancel: Box<dyn Fn()>,
}

pub struct WinUiServices {
    backend: WinUiHandle,
    /// The UI thread's queue: pickers and the clipboard complete elsewhere.
    queue: w::DispatcherQueue,
    /// Tests keep clipboard text here instead of the system clipboard.
    private_clipboard: Option<String>,
    alerts: Rc<RefCell<AlertQueue>>,
    pickers: Vec<OpenPicker>,
}

impl WinUiServices {
    pub(crate) fn new(backend: WinUiHandle, private_clipboard: bool) -> WinUiServices {
        WinUiServices {
            backend,
            queue: w::DispatcherQueue::GetForCurrentThread().expect("a dispatcher queue on the UI thread"),
            private_clipboard: private_clipboard.then(String::new),
            alerts: Rc::default(),
            pickers: Vec::new(),
        }
    }

    /// Keeps a picker until its window goes, forgetting those that answered.
    fn track_picker(&mut self, window: NodeId, ticket: u64, cancel: Box<dyn Fn()>) {
        self.pickers.retain(|p| later::is_parked(p.ticket));
        self.pickers.push(OpenPicker { window, ticket, cancel });
    }
}

/// Answers a parked reply with `None`, if it's still waiting, and cancels
/// the picker's operation.
fn cancel_picker<T, A>(operation: &windows_future::IAsyncOperation<T>, ticket: u64) -> Box<dyn Fn()>
where
    T: windows_core::RuntimeType + 'static,
    A: 'static,
{
    let operation = operation.clone();
    Box::new(move || {
        if let Some(reply) = later::take::<Reply<Option<A>>>(ticket) {
            reply(None);
        }
        _ = operation.Cancel();
    })
}

fn failed(error: windows_core::Error) -> ServiceError {
    ServiceError::Failed(error.message())
}

/// The pickers take their start folder as a path string.
fn folder_path(folder: &Option<PathBuf>) -> Option<String> {
    existing_folder(folder).map(|f| f.to_string_lossy().into_owned())
}

/// `.png`-style patterns for the pickers; `*` when nothing is filtered, or
/// for a filter that lets every file through.
fn patterns(filters: &[FileFilter]) -> Vec<HSTRING> {
    let patterns: Vec<HSTRING> =
        filters.iter().flat_map(|f| &f.extensions).map(|e| format!(".{}", e.trim_start_matches('.')).into()).collect();
    if patterns.is_empty() { vec!["*".into()] } else { patterns }
}

impl Services for WinUiServices {
    fn clipboard_text(&mut self, reply: Reply<Option<String>>) {
        if let Some(text) = &self.private_clipboard {
            return reply((!text.is_empty()).then(|| text.clone()));
        }
        let read = (|| -> R<_> {
            let content = w::Clipboard::GetContent()?;
            if !content.Contains(&w::StandardDataFormats::Text()?)? {
                return Ok(None);
            }
            Ok(Some(content.GetTextAsync()?))
        })();
        match read {
            Ok(Some(operation)) => {
                let queue = self.queue.clone();
                let ticket = later::park_until(&queue, reply);
                let id = ticket.id();
                let watched = operation.when(move |text| {
                    let text = text.ok().map(|t| t.to_string_lossy());
                    later::on_ui_take(&queue, ticket, move |reply: Reply<Option<String>>| reply(text));
                });
                if watched.is_err()
                    && let Some(reply) = later::take::<Reply<Option<String>>>(id)
                {
                    reply(None);
                }
            }
            _ => reply(None),
        }
    }

    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>) {
        if let Some(stored) = &mut self.private_clipboard {
            *stored = text.to_string();
            return reply(Ok(()));
        }
        // Fails while another process holds the clipboard open.
        let written = (|| {
            let package = w::DataPackage::new()?;
            package.SetText(text)?;
            w::Clipboard::SetContent(&package)?;
            // Keep the text available after we exit.
            w::Clipboard::Flush()
        })();
        reply(written.map_err(failed));
    }

    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>) {
        self.alerts.borrow_mut().waiting.push_back(QueuedAlert { parent, alert: alert.clone(), reply });
        show_next_alert(self.backend.clone(), self.alerts.clone());
    }

    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>) {
        let Some((window, node)) = self.backend.window_parts(parent, |p| (p.id, p.node)) else { return reply(None) };
        let ticket = later::park(reply);
        let queue = self.queue.clone();
        let finish = move |paths: Option<Vec<PathBuf>>| {
            later::on_ui(&queue, move || {
                if let Some(reply) = later::take::<Reply<Option<Vec<PathBuf>>>>(ticket) {
                    reply(paths);
                }
            });
        };
        let path = |result: &w::PickFileResult| result.Path().ok().map(PathBuf::from);
        type Paths = Vec<PathBuf>;
        let started: R<Box<dyn Fn()>> = (|| {
            if request.directories {
                let picker = w::FolderPicker::CreateInstance(window)?;
                if let Some(folder) = folder_path(&request.start_folder) {
                    picker.cast::<w::IFolderPicker2>()?.SetSuggestedFolder(&folder)?;
                }
                let operation = picker.PickSingleFolderAsync()?;
                operation.when(move |folder| {
                    finish(folder.ok().and_then(|f| f.Path().ok()).map(|p| vec![PathBuf::from(p)]));
                })?;
                Ok(cancel_picker::<_, Paths>(&operation, ticket))
            } else {
                let picker = w::FileOpenPicker::CreateInstance(window)?;
                let picker2 = picker.cast::<w::IFileOpenPicker2>()?;
                if let Some(folder) = folder_path(&request.start_folder) {
                    picker2.SetSuggestedFolder(&folder)?;
                }
                if request.filters.is_empty() {
                    picker.FileTypeFilter()?.Append(&HSTRING::from("*"))?;
                } else {
                    // Named choices, as the save picker has, so an "every
                    // file" filter is one the user can pick; the flat
                    // FileTypeFilter would merge them into one list.
                    let choices = picker2.FileTypeChoices()?;
                    for filter in &request.filters {
                        let extensions: Vec<HSTRING> = patterns(std::slice::from_ref(filter));
                        choices
                            .Insert(&HSTRING::from(&filter.name), &windows_collections::IVector::from(extensions))?;
                    }
                }
                if request.multiple {
                    let operation = picker.PickMultipleFilesAsync()?;
                    operation.when(move |files| {
                        let paths: Option<Vec<PathBuf>> =
                            files.ok().map(|files| (&files).into_iter().filter_map(|f| path(&f)).collect());
                        finish(paths.filter(|p| !p.is_empty()));
                    })?;
                    Ok(cancel_picker::<_, Paths>(&operation, ticket))
                } else {
                    let operation = picker.PickSingleFileAsync()?;
                    operation.when(move |file| finish(file.ok().and_then(|f| path(&f)).map(|p| vec![p])))?;
                    Ok(cancel_picker::<_, Paths>(&operation, ticket))
                }
            }
        })();
        match started {
            Ok(cancel) => self.track_picker(node, ticket, cancel),
            Err(_) => {
                if let Some(reply) = later::take::<Reply<Option<Vec<PathBuf>>>>(ticket) {
                    reply(None);
                }
            }
        }
    }

    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>) {
        let Some((window, node)) = self.backend.window_parts(parent, |p| (p.id, p.node)) else { return reply(None) };
        let ticket = later::park(reply);
        let started: R<Box<dyn Fn()>> = (|| {
            let picker = w::FileSavePicker::CreateInstance(window)?;
            if let Some(name) = &request.default_name {
                picker.SetSuggestedFileName(name)?;
            }
            if let Some(folder) = folder_path(&request.start_folder) {
                picker.SetSuggestedFolder(&folder)?;
            }
            let choices = picker.FileTypeChoices()?;
            // A save choice is the extension the name is given, which
            // "every file" isn't.
            for filter in request.filters.iter().filter(|f| !f.is_all()) {
                let extensions: Vec<HSTRING> = patterns(std::slice::from_ref(filter));
                choices.Insert(&HSTRING::from(&filter.name), &windows_collections::IVector::from(extensions))?;
            }
            let queue = self.queue.clone();
            let operation = picker.PickSaveFileAsync()?;
            operation.when(move |file| {
                let path = file.ok().and_then(|f| f.Path().ok()).map(PathBuf::from);
                later::on_ui(&queue, move || {
                    if let Some(reply) = later::take::<Reply<Option<PathBuf>>>(ticket) {
                        reply(path);
                    }
                });
            })?;
            Ok(cancel_picker::<_, PathBuf>(&operation, ticket))
        })();
        match started {
            Ok(cancel) => self.track_picker(node, ticket, cancel),
            Err(_) => {
                if let Some(reply) = later::take::<Reply<Option<PathBuf>>>(ticket) {
                    reply(None);
                }
            }
        }
    }

    /// The shell's own file operation, as Explorer's Delete: it asks first
    /// when the Recycle Bin's settings say so, offers to delete what's too
    /// big to recycle, and shows its progress and errors on the window. It
    /// runs its own message loop, so it starts from the dispatcher queue,
    /// outside the `Ui`'s call.
    fn trash(&mut self, parent: Option<NodeId>, paths: &[PathBuf], reply: Reply<Result<(), ServiceError>>) {
        let owner = self.backend.window_parts(parent, |p| p.hwnd as isize);
        let ticket = later::park_until(&self.queue, (paths.to_vec(), reply));
        later::on_ui_take(
            &self.queue,
            ticket,
            move |(paths, reply): (Vec<PathBuf>, Reply<Result<(), ServiceError>>)| {
                reply(recycle(owner.map(|hwnd| hwnd as w::HWND), &paths));
            },
        );
    }

    /// The shell's default verb, as a double-click in Explorer: it asks
    /// which app ("How do you want to open this?") when none is set. It
    /// may wait on the app and show UI, so it too starts from the
    /// dispatcher queue.
    fn launch(&mut self, parent: Option<NodeId>, target: &Launch, reply: Reply<Result<(), ServiceError>>) {
        let owner = self.backend.window_parts(parent, |p| p.hwnd as isize);
        let target: HSTRING = match target {
            // Made absolute: `ShellExecuteExW` searches the path for a
            // relative name (`notepad` ran Notepad), and runs one with a
            // scheme (`ms-settings:x`, `search-ms:…`) as a URL, where AppKit
            // and GTK open the file under the working directory.
            Launch::Path(path) => match std::path::absolute(path) {
                Ok(path) => path.as_os_str().into(),
                Err(error) => return reply(Err(ServiceError::Failed(error.to_string()))),
            },
            Launch::Url(url) if refused(url) => return reply(Err(ServiceError::Unavailable)),
            Launch::Url(url) if is_url(url) => url.into(),
            Launch::Url(url) => return reply(Err(ServiceError::Failed(format!("\u{201C}{url}\u{201D} isn't a URL")))),
        };
        let ticket = later::park_until(&self.queue, reply);
        later::on_ui_take(&self.queue, ticket, move |reply: Reply<Result<(), ServiceError>>| {
            reply(shell_open(owner.map(|hwnd| hwnd as w::HWND), &target));
        });
    }

    /// The alert showing on the window is cancelled (`Cancel` on its
    /// `ShowAsync` closes a `ContentDialog`) and answers its close button;
    /// alerts waiting for the window answer that at once. Its pickers
    /// answer no paths right away, and their operations are cancelled.
    fn window_destroyed(&mut self, window: NodeId) {
        let (waiting, shown) = {
            let mut queue = self.alerts.borrow_mut();
            let (waiting, kept): (VecDeque<_>, _) =
                std::mem::take(&mut queue.waiting).into_iter().partition(|a| a.parent == Some(window));
            queue.waiting = kept;
            let shown = queue.shown.as_mut().filter(|s| s.window == window).map(|shown| {
                shown.cancelled = true;
                shown.operation.clone()
            });
            (waiting, shown)
        };
        for QueuedAlert { alert, reply, .. } in waiting {
            reply(alert.effective_buttons().len() - 1);
        }
        if let Some(operation) = shown {
            _ = operation.Cancel();
        }
        let (gone, kept): (Vec<_>, _) = std::mem::take(&mut self.pickers).into_iter().partition(|p| p.window == window);
        self.pickers = kept;
        gone.iter().for_each(|picker| (picker.cancel)());
    }

    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        self.backend.set_menu(window, menu, activate);
    }
}

/// Whether `ShellExecuteExW` may take `url` as a URL: it runs anything else
/// as a path, programs included, where AppKit and GTK only open URLs. A
/// scheme of one letter is a drive (`C:\…`), and `file:` URLs run programs
/// too: files go through `Launch::Path`.
fn is_url(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else { return false };
    let mut chars = scheme.chars();
    let well_formed = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    well_formed && scheme.len() > 1 && !scheme.eq_ignore_ascii_case("file") && w::Uri::CreateUri(url).is_ok()
}

/// Schemes Microsoft disabled or warned against after attacks used them
/// from links and documents: `ms-msdt:` (Follina, CVE-2022-30190),
/// `search-ms:` and `search:` (Explorer searches of remote shares that
/// pass for folders), `ms-officecmd:` (Office's launcher, which ran
/// commands from a link) and `ms-appinstaller:` (installs apps; off by
/// default since December 2023).
/// Refused as no app opening them, as Windows answers once a scheme's
/// handler is turned off.
const REFUSED_SCHEMES: [&str; 5] = ["ms-msdt", "search-ms", "search", "ms-officecmd", "ms-appinstaller"];

fn refused(url: &str) -> bool {
    let scheme = url.split_once(':').map_or("", |(scheme, _)| scheme);
    REFUSED_SCHEMES.iter().any(|refused| scheme.eq_ignore_ascii_case(refused))
}

fn shell_open(owner: Option<w::HWND>, target: &HSTRING) -> Result<(), ServiceError> {
    let mut info = w::SHELLEXECUTEINFOW {
        cbSize: size_of::<w::SHELLEXECUTEINFOW>() as u32,
        hwnd: owner.unwrap_or(std::ptr::null_mut()),
        lpFile: windows_core::PCWSTR(target.as_ptr()),
        nShow: w::SW_SHOWNORMAL,
        ..Default::default()
    };
    if unsafe { w::ShellExecuteExW(&mut info) }.as_bool() {
        return Ok(());
    }
    match unsafe { w::GetLastError() } as i32 {
        w::ERROR_NO_ASSOCIATION => Err(ServiceError::Unavailable),
        w::ERROR_CANCELLED => Err(ServiceError::Cancelled),
        _ => Err(ServiceError::Failed(windows_core::HRESULT::from_thread().message())),
    }
}

fn recycle(owner: Option<w::HWND>, paths: &[PathBuf]) -> Result<(), ServiceError> {
    let cancelled = |hr: windows_core::HRESULT| {
        // HRESULT_FROM_WIN32(ERROR_CANCELLED) too, from the confirmation.
        hr == w::COPYENGINE_E_USER_CANCELLED || hr.0 as u32 == 0x8007_0000 | w::ERROR_CANCELLED as u32
    };
    let failed = |hr: windows_core::HRESULT| {
        if cancelled(hr) { ServiceError::Cancelled } else { ServiceError::Failed(hr.message()) }
    };
    unsafe {
        let mut raw = std::ptr::null_mut();
        let clsctx = w::CLSCTX_ALL as u32;
        w::CoCreateInstance(&w::FileOperation, std::ptr::null_mut(), clsctx, &w::IFileOperation::IID, &mut raw)
            .ok()
            .map_err(|e| failed(e.code()))?;
        let operation = w::IFileOperation::from_raw(raw);
        // Undo is what sends deletes to the Recycle Bin (and before
        // Windows 8, all there was).
        let flags = (w::FOF_ALLOWUNDO | w::FOFX_RECYCLEONDELETE) as u32;
        operation.SetOperationFlags(flags).ok().map_err(|e| failed(e.code()))?;
        if let Some(owner) = owner {
            operation.SetOwnerWindow(owner).ok().map_err(|e| failed(e.code()))?;
        }
        for path in paths {
            let mut raw = std::ptr::null_mut();
            // The shell parses only absolute paths with backslashes.
            let path = std::path::absolute(path).map_err(|e| ServiceError::Failed(e.to_string()))?;
            let name = HSTRING::from(path.as_os_str());
            w::SHCreateItemFromParsingName(
                windows_core::PCWSTR(name.as_ptr()),
                std::ptr::null_mut(),
                &w::IShellItem::IID,
                &mut raw,
            )
            .ok()
            .map_err(|e| failed(e.code()))?;
            let item = w::IShellItem::from_raw(raw);
            operation.DeleteItem(&item, None::<&w::IFileOperationProgressSink>).ok().map_err(|e| failed(e.code()))?;
        }
        operation.PerformOperations().ok().map_err(|e| failed(e.code()))?;
        if operation.GetAnyOperationsAborted().is_ok_and(|aborted| aborted.as_bool()) {
            return Err(ServiceError::Cancelled);
        }
    }
    Ok(())
}

/// Shows the next queued alert unless one is showing.
///
/// Buttons map onto `ContentDialog`'s three: the first is the primary
/// (default) one; with several, the last is the close button, which Escape
/// also chooses; a third goes in the middle.
fn show_next_alert(backend: WinUiHandle, queue: Rc<RefCell<AlertQueue>>) {
    let next = {
        let mut queue = queue.borrow_mut();
        if queue.showing {
            return;
        }
        let Some(next) = queue.waiting.pop_front() else { return };
        queue.showing = true;
        next
    };
    let QueuedAlert { parent, alert, reply } = next;
    let buttons = alert.effective_buttons();
    let window = backend.window_parts(parent, |p| p.node);
    let shown: R<_> = (|| {
        let root = backend.xaml_root(parent).ok_or_else(|| windows_core::Error::from_hresult(w::E_FAIL))?;
        let dialog = w::ContentDialog::new()?;
        let iface: w::IContentDialog = dialog.cast()?;
        iface.SetTitle(&boxed(&alert.title))?;
        if let Some(message) = &alert.message {
            dialog.cast::<w::IContentControl>()?.SetContent(&boxed(message))?;
        }
        iface.SetPrimaryButtonText(&buttons[0])?;
        iface.SetDefaultButton(w::ContentDialogButton::Primary)?;
        if buttons.len() >= 2 {
            iface.SetCloseButtonText(&buttons[buttons.len() - 1])?;
        }
        if buttons.len() >= 3 {
            iface.SetSecondaryButtonText(&buttons[1])?;
        }
        dialog.cast::<w::IUIElement>()?.SetXamlRoot(&root)?;
        iface.ShowAsync()
    })();
    let count = buttons.len();
    let answer = move |result: Option<w::ContentDialogResult>| match result {
        Some(w::ContentDialogResult::Primary) | None => 0,
        Some(w::ContentDialogResult::Secondary) => 1,
        // Close (or Escape): the last button.
        Some(_) => count - 1,
    };
    match shown {
        Ok(operation) => {
            if let Some(window) = window {
                queue.borrow_mut().shown = Some(ShownAlert { window, operation: operation.clone(), cancelled: false });
            }
            let ticket = later::park((reply, backend, queue));
            let watched = operation.when(move |result| {
                type Parked = (Reply<usize>, WinUiHandle, Rc<RefCell<AlertQueue>>);
                if let Some((reply, backend, queue)) = later::take::<Parked>(ticket) {
                    // Its window went: what Escape would have answered.
                    let shown = queue.borrow_mut().shown.take();
                    reply(if shown.is_some_and(|s| s.cancelled) { count - 1 } else { answer(result.ok()) });
                    queue.borrow_mut().showing = false;
                    show_next_alert(backend, queue);
                }
            });
            if watched.is_err() {
                type Parked = (Reply<usize>, WinUiHandle, Rc<RefCell<AlertQueue>>);
                if let Some((reply, _, queue)) = later::take::<Parked>(ticket) {
                    let mut queue = queue.borrow_mut();
                    queue.showing = false;
                    queue.shown = None;
                    drop(queue);
                    reply(answer(None));
                }
            }
        }
        Err(_) => {
            // No window to show it in: the default answer.
            queue.borrow_mut().showing = false;
            reply(0);
        }
    }
}
