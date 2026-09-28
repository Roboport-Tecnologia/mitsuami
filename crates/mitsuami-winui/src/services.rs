//! Clipboard, dialogs and the menu bar on Windows.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::NodeId;
use mitsuami_core::services::{
    Alert, FileFilter, MenuBarData, OpenFile, Reply, SaveFile, ServiceError, Services, existing_folder,
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

    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        self.backend.set_menu(window, menu, activate);
    }
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
