//! The clipboard and alerts. File dialogs and menus aren't done yet.

use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::NodeId;
use mitsuami_core::services::{Alert, AlertStyle, MenuBarData, OpenFile, Reply, SaveFile, ServiceError, Services};
use windows_sys::Win32::Foundation::{GlobalFree, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TD_ERROR_ICON, TD_INFORMATION_ICON, TD_WARNING_ICON,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;

use crate::registry::Shared;
use crate::runtime::{comctl, later, wide};

/// The platform's services. With `private_clipboard`, copying and pasting
/// stay in the process (tests).
pub struct Win32Services {
    shared: Rc<Shared>,
    private: Option<String>,
    private_clipboard: bool,
}

impl Win32Services {
    pub(crate) fn new(shared: Rc<Shared>, private_clipboard: bool) -> Win32Services {
        Win32Services { shared, private: None, private_clipboard }
    }
}

/// Custom buttons' ids start here, past the common ones (`IDOK`, …).
const FIRST_BUTTON: i32 = 100;

impl Services for Win32Services {
    fn clipboard_text(&mut self, reply: Reply<Option<String>>) {
        if self.private_clipboard {
            return reply(self.private.clone());
        }
        reply(read_clipboard());
    }

    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>) {
        if self.private_clipboard {
            self.private = Some(text.to_owned());
            return reply(Ok(()));
        }
        reply(write_clipboard(text));
    }

    /// A task dialog, the platform's message box with buttons of its own,
    /// shown from the message loop: it runs a modal loop, which mustn't
    /// run inside a tick.
    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>) {
        let owner = parent.and_then(|p| self.shared.hwnd(p)).map_or(0, |h| h as isize);
        let alert = alert.clone();
        later(move || {
            let owner = if owner == 0 { unsafe { GetActiveWindow() } } else { owner as HWND };
            reply(task_dialog(owner, &alert));
        });
    }

    fn open_file(&mut self, _parent: Option<NodeId>, _request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>) {
        reply(None);
    }

    fn save_file(&mut self, _parent: Option<NodeId>, _request: &SaveFile, reply: Reply<Option<PathBuf>>) {
        reply(None);
    }

    fn set_menu(&mut self, _window: Option<NodeId>, _menu: &MenuBarData, _activate: Rc<dyn Fn(u32)>) {}
}

/// The index of the button chosen; closing the dialog chooses the last.
fn task_dialog(owner: HWND, alert: &Alert) -> usize {
    type TaskDialogIndirect = unsafe extern "system" fn(*const TASKDIALOGCONFIG, *mut i32, *mut i32, *mut i32) -> i32;
    let buttons = alert.effective_buttons();
    let last = buttons.len() - 1;
    let Some(show) = comctl(c"TaskDialogIndirect") else { return last };
    let show: TaskDialogIndirect = unsafe { std::mem::transmute(show) };
    let titles: Vec<Vec<u16>> = buttons.iter().map(|b| wide(b)).collect();
    let native: Vec<TASKDIALOG_BUTTON> = titles
        .iter()
        .enumerate()
        .map(|(i, t)| TASKDIALOG_BUTTON { nButtonID: FIRST_BUTTON + i as i32, pszButtonText: t.as_ptr() })
        .collect();
    let title = wide(&alert.title);
    let message = alert.message.as_deref().map(wide);
    let caption = wide(
        &std::env::current_exe()
            .ok()
            .and_then(|p| Some(p.file_stem()?.to_string_lossy().into_owned()))
            .unwrap_or_default(),
    );
    let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    config.cbSize = size_of::<TASKDIALOGCONFIG>() as u32;
    config.hwndParent = owner;
    config.pszWindowTitle = caption.as_ptr();
    config.pszMainInstruction = title.as_ptr();
    config.pszContent = message.as_ref().map_or(std::ptr::null(), |m| m.as_ptr());
    config.cButtons = native.len() as u32;
    config.pButtons = native.as_ptr();
    config.nDefaultButton = FIRST_BUTTON;
    config.Anonymous1.pszMainIcon = match alert.style {
        AlertStyle::Warning => TD_WARNING_ICON,
        AlertStyle::Critical => TD_ERROR_ICON,
        _ => TD_INFORMATION_ICON,
    };
    let mut chosen = 0;
    let shown = unsafe { show(&config, &mut chosen, std::ptr::null_mut(), std::ptr::null_mut()) };
    if shown < 0 {
        return last;
    }
    usize::try_from(chosen - FIRST_BUTTON).ok().filter(|i| *i <= last).unwrap_or(last)
}

fn read_clipboard() -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let data = GetClipboardData(CF_UNICODETEXT as u32);
        let text = (!data.is_null()).then(|| {
            let ptr = GlobalLock(data) as *const u16;
            let text = (!ptr.is_null()).then(|| {
                let len = (0..).take_while(|i| *ptr.add(*i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
            });
            GlobalUnlock(data);
            text
        });
        CloseClipboard();
        text.flatten()
    }
}

fn write_clipboard(text: &str) -> Result<(), ServiceError> {
    let text = wide(text);
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err(ServiceError::Failed("another app has the clipboard open".into()));
        }
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * 2);
        let ok = !memory.is_null() && {
            let ptr = GlobalLock(memory) as *mut u16;
            std::ptr::copy_nonoverlapping(text.as_ptr(), ptr, text.len());
            GlobalUnlock(memory);
            !SetClipboardData(CF_UNICODETEXT as u32, memory).is_null()
        };
        if !ok && !memory.is_null() {
            GlobalFree(memory);
        }
        CloseClipboard();
        if ok { Ok(()) } else { Err(ServiceError::Failed("the clipboard refused the text".into())) }
    }
}
