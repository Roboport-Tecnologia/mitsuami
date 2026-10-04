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
    waiting: VecDeque<QueuedAlert>,
}

pub struct WinUiServices {
    backend: WinUiHandle,
    /// The UI thread's queue: pickers and the clipboard complete elsewhere.
    queue: w::DispatcherQueue,
    /// Tests keep clipboard text here instead of the system clipboard.
    private_clipboard: Option<String>,
    alerts: Rc<RefCell<AlertQueue>>,
}

impl WinUiServices {
    pub(crate) fn new(backend: WinUiHandle, private_clipboard: bool) -> WinUiServices {
        WinUiServices {
            backend,
            queue: w::DispatcherQueue::GetForCurrentThread().expect("a dispatcher queue on the UI thread"),
            private_clipboard: private_clipboard.then(String::new),
            alerts: Rc::default(),
        }
    }
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
                let ticket = later::park(reply);
                let queue = self.queue.clone();
                let watched = operation.when(move |text| {
                    let text = text.ok().map(|t| t.to_string_lossy());
                    later::on_ui(&queue, move || {
                        if let Some(reply) = later::take::<Reply<Option<String>>>(ticket) {
                            reply(text);
                        }
                    });
                });
                if watched.is_err()
                    && let Some(reply) = later::take::<Reply<Option<String>>>(ticket)
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
        let Some(window) = self.backend.window_parts(parent, |p| p.id) else { return reply(None) };
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
        let started: R<()> = (|| {
            if request.directories {
                let picker = w::FolderPicker::CreateInstance(window)?;
                if let Some(folder) = folder_path(&request.start_folder) {
                    picker.cast::<w::IFolderPicker2>()?.SetSuggestedFolder(&folder)?;
                }
                picker.PickSingleFolderAsync()?.when(move |folder| {
                    finish(folder.ok().and_then(|f| f.Path().ok()).map(|p| vec![PathBuf::from(p)]));
                })
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
                    picker.PickMultipleFilesAsync()?.when(move |files| {
                        let paths: Option<Vec<PathBuf>> =
                            files.ok().map(|files| (&files).into_iter().filter_map(|f| path(&f)).collect());
                        finish(paths.filter(|p| !p.is_empty()));
                    })
                } else {
                    picker
                        .PickSingleFileAsync()?
                        .when(move |file| finish(file.ok().and_then(|f| path(&f)).map(|p| vec![p])))
                }
            }
        })();
        if started.is_err()
            && let Some(reply) = later::take::<Reply<Option<Vec<PathBuf>>>>(ticket)
        {
            reply(None);
        }
    }

    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>) {
        let Some(window) = self.backend.window_parts(parent, |p| p.id) else { return reply(None) };
        let ticket = later::park(reply);
        let started: R<()> = (|| {
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
            picker.PickSaveFileAsync()?.when(move |file| {
                let path = file.ok().and_then(|f| f.Path().ok()).map(PathBuf::from);
                later::on_ui(&queue, move || {
                    if let Some(reply) = later::take::<Reply<Option<PathBuf>>>(ticket) {
                        reply(path);
                    }
                });
            })
        })();
        if started.is_err()
            && let Some(reply) = later::take::<Reply<Option<PathBuf>>>(ticket)
        {
            reply(None);
        }
    }

    /// The shell's own file operation, as Explorer's Delete: it asks first
    /// when the Recycle Bin's settings say so, offers to delete what's too
    /// big to recycle, and shows its progress and errors on the window. It
    /// runs its own message loop, so it starts from the dispatcher queue,
    /// outside the `Ui`'s call.
    fn trash(&mut self, parent: Option<NodeId>, paths: &[PathBuf], reply: Reply<Result<(), ServiceError>>) {
        let owner = self.backend.window_parts(parent, |p| p.hwnd as isize);
        let ticket = later::park((paths.to_vec(), reply));
        later::on_ui(&self.queue, move || {
            if let Some((paths, reply)) = later::take::<(Vec<PathBuf>, Reply<Result<(), ServiceError>>)>(ticket) {
                reply(recycle(owner.map(|hwnd| hwnd as w::HWND), &paths));
            }
        });
    }

    /// The shell's default verb, as a double-click in Explorer: it asks
    /// which app ("How do you want to open this?") when none is set. It
    /// may wait on the app and show UI, so it too starts from the
    /// dispatcher queue.
    fn launch(&mut self, parent: Option<NodeId>, target: &Launch, reply: Reply<Result<(), ServiceError>>) {
        let owner = self.backend.window_parts(parent, |p| p.hwnd as isize);
        let target: HSTRING = match target {
            Launch::Path(path) => path.as_os_str().into(),
            Launch::Url(url) if is_url(url) => url.into(),
            Launch::Url(url) => return reply(Err(ServiceError::Failed(format!("\u{201C}{url}\u{201D} isn't a URL")))),
        };
        let ticket = later::park(reply);
        later::on_ui(&self.queue, move || {
            if let Some(reply) = later::take::<Reply<Result<(), ServiceError>>>(ticket) {
                reply(shell_open(owner.map(|hwnd| hwnd as w::HWND), &target));
            }
        });
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
            let ticket = later::park((reply, backend, queue));
            let watched = operation.when(move |result| {
                type Parked = (Reply<usize>, WinUiHandle, Rc<RefCell<AlertQueue>>);
                if let Some((reply, backend, queue)) = later::take::<Parked>(ticket) {
                    reply(answer(result.ok()));
                    queue.borrow_mut().showing = false;
                    show_next_alert(backend, queue);
                }
            });
            if watched.is_err() {
                type Parked = (Reply<usize>, WinUiHandle, Rc<RefCell<AlertQueue>>);
                if let Some((reply, _, queue)) = later::take::<Parked>(ticket) {
                    queue.borrow_mut().showing = false;
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
