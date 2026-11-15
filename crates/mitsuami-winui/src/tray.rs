//! The tray icon on Windows: a notification area icon (`Shell_NotifyIconW`,
//! `NOTIFYICON_VERSION_4`), which WinUI doesn't wrap. Its callbacks come to
//! a message-only window of its own. A click, left or right, opens its
//! menu, a Win32 popup menu (`TrackPopupMenuEx` after
//! `SetForegroundWindow`), as Windows 11's own tray menus are: a XAML
//! flyout would need a window to show in. The icon has a GUID made from
//! the app's id, so Windows keeps the user's choice to show it; for an
//! unsigned app Windows ties that GUID to the executable's path, and when
//! it's taken by another path the icon goes without one. Explorer
//! restarting (`TaskbarCreated`) adds it again.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::AppIcon;
use mitsuami_core::services::{MenuCheck, MenuEntry, TrayData};
use windows_core::{GUID, HSTRING, PCWSTR, PWSTR};

use crate::bindings as w;

/// The message the icon's callbacks come as.
const CALLBACK: u32 = w::WM_APP as u32 + 1;
const CLASS: PCWSTR = windows_core::w!("MitsuamiTray");

type Activate = Rc<dyn Fn(u32)>;
type Shown = Rc<dyn Fn(bool)>;

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

struct State {
    window: w::HWND,
    /// `TaskbarCreated`, which Explorer sends when it starts again.
    taskbar_created: u32,
    data: TrayData,
    /// `AppInfo::icon`, for a tray icon that gives none.
    app_icon: Option<AppIcon>,
    guid: GUID,
    /// Whether Windows took the GUID: without, the icon goes by its id.
    with_guid: bool,
    icon: w::HICON,
    added: bool,
    activate: Activate,
    shown: Shown,
}

/// `Services::set_tray`.
pub(crate) fn set(tray: Option<&TrayData>, info: &mitsuami_core::AppInfo, activate: Activate, shown: Shown) {
    let Some(tray) = tray else {
        if let Some(state) = STATE.with(|s| s.borrow_mut().take()) {
            state.remove();
            (state.shown)(false);
        }
        return;
    };
    let exists = STATE.with(|s| s.borrow().is_some());
    if !exists {
        let Some(window) = message_window() else { return shown(false) };
        let taskbar_created = unsafe { w::RegisterWindowMessageW(windows_core::w!("TaskbarCreated")) };
        let exe = || std::env::current_exe().map(|exe| exe.display().to_string()).unwrap_or_default();
        let state = State {
            window,
            taskbar_created,
            data: tray.clone(),
            app_icon: None,
            guid: guid(&info.id.clone().unwrap_or_else(exe)),
            with_guid: true,
            icon: std::ptr::null_mut(),
            added: false,
            activate: activate.clone(),
            shown: shown.clone(),
        };
        STATE.with(|s| *s.borrow_mut() = Some(state));
    }
    let added = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let state = s.as_mut().expect("the tray's state");
        state.data = tray.clone();
        state.app_icon = info.icon.clone();
        state.activate = activate;
        state.shown = shown.clone();
        state.update();
        state.added
    });
    shown(added);
}

impl State {
    /// Adds the icon, or sends what changed.
    fn update(&mut self) {
        let icon = load_icon(self.data.icon.as_ref().or(self.app_icon.as_ref()));
        let old = std::mem::replace(&mut self.icon, icon);
        let mut data = self.notify_data();
        if self.added {
            unsafe { _ = w::Shell_NotifyIconW(w::NIM_MODIFY as u32, &data) };
        } else {
            self.added = unsafe { w::Shell_NotifyIconW(w::NIM_ADD as u32, &data) }.as_bool();
            if !self.added && self.with_guid {
                // The GUID belongs to this app at another path.
                self.with_guid = false;
                data = self.notify_data();
                self.added = unsafe { w::Shell_NotifyIconW(w::NIM_ADD as u32, &data) }.as_bool();
            }
            if self.added {
                data.Anonymous.uVersion = w::NOTIFYICON_VERSION_4 as u32;
                unsafe { _ = w::Shell_NotifyIconW(w::NIM_SETVERSION as u32, &data) };
            }
        }
        if !old.is_null() {
            unsafe { _ = w::DestroyIcon(old) };
        }
    }

    fn notify_data(&self) -> w::NOTIFYICONDATAW {
        let mut data = w::NOTIFYICONDATAW {
            cbSize: size_of::<w::NOTIFYICONDATAW>() as u32,
            hWnd: self.window,
            uID: 1,
            uFlags: (w::NIF_MESSAGE | w::NIF_ICON | w::NIF_TIP | w::NIF_SHOWTIP) as u32,
            uCallbackMessage: CALLBACK,
            hIcon: self.icon,
            ..Default::default()
        };
        if self.with_guid {
            data.uFlags |= w::NIF_GUID as u32;
            data.guidItem = self.guid;
        }
        // The tooltip, else the title: it's what Narrator reads too.
        let tip = if self.data.tooltip.is_empty() { &self.data.title } else { &self.data.tooltip };
        for (slot, unit) in data.szTip.iter_mut().zip(tip.encode_utf16().take(127)) {
            *slot = unit;
        }
        data
    }

    fn remove(&self) {
        let data = self.notify_data();
        unsafe {
            if self.added {
                _ = w::Shell_NotifyIconW(w::NIM_DELETE as u32, &data);
            }
            if !self.icon.is_null() {
                _ = w::DestroyIcon(self.icon);
            }
            _ = w::DestroyWindow(self.window);
        }
    }
}

/// A GUID made from the app's id, the same in every run: FNV-1a twice,
/// with the version and variant bits of a name-based one.
fn guid(id: &str) -> GUID {
    let fnv = |seed: u64| id.bytes().fold(seed, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3));
    let high = fnv(0xcbf2_9ce4_8422_2325);
    let low = fnv(0x8422_2325_cbf2_9ce4);
    let value = (u128::from(high) << 64 | u128::from(low)) & !(0xf000 << 64) | (0x5000 << 64);
    let value = value & !(0xc000 << 48) | (0x8000 << 48);
    GUID::from_u128(value)
}

/// The small icon's size for the system's DPI: an `.ico` file at the size
/// it has, else the image as an icon of its own.
fn load_icon(icon: Option<&AppIcon>) -> w::HICON {
    let dpi = unsafe { w::GetDpiForSystem() };
    let size = unsafe { w::GetSystemMetricsForDpi(w::SM_CXSMICON, dpi) };
    let loaded = match icon {
        Some(AppIcon::File(path)) if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ico")) => {
            let path = HSTRING::from(path.as_os_str());
            unsafe {
                w::LoadImageW(
                    std::ptr::null_mut(),
                    PCWSTR(path.as_ptr()),
                    w::IMAGE_ICON as u32,
                    size,
                    size,
                    w::LR_LOADFROMFILE as u32,
                )
            }
        }
        Some(icon) => match icon.read() {
            Some(bytes) => unsafe {
                w::CreateIconFromResourceEx(
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    true.into(),
                    0x0003_0000,
                    size,
                    size,
                    w::LR_DEFAULTCOLOR as u32,
                )
            },
            None => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    };
    if loaded.is_null() { unsafe { w::LoadIconW(std::ptr::null_mut(), w::IDI_APPLICATION) } } else { loaded }
}

fn message_window() -> Option<w::HWND> {
    static REGISTER: std::sync::Once = std::sync::Once::new();
    REGISTER.call_once(|| unsafe {
        let class = w::WNDCLASSEXW {
            cbSize: size_of::<w::WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: w::GetModuleHandleW(PCWSTR::null()),
            lpszClassName: CLASS,
            ..Default::default()
        };
        w::RegisterClassExW(&class);
    });
    let hwnd = unsafe {
        w::CreateWindowExW(
            0,
            CLASS,
            PCWSTR::null(),
            0,
            0,
            0,
            0,
            0,
            w::HWND_MESSAGE,
            std::ptr::null_mut(),
            w::GetModuleHandleW(PCWSTR::null()),
            std::ptr::null(),
        )
    };
    (!hwnd.is_null()).then_some(hwnd)
}

unsafe extern "system" fn window_proc(hwnd: w::HWND, message: u32, wparam: w::WPARAM, lparam: w::LPARAM) -> w::LRESULT {
    if message == CALLBACK {
        // Version 4: the event in the low word, the anchor in `wparam`.
        let event = (lparam & 0xffff) as u32;
        if event == w::NIN_SELECT as u32 || event == w::NIN_KEYSELECT as u32 || event == w::WM_CONTEXTMENU as u32 {
            let x = (wparam & 0xffff) as u16 as i16 as i32;
            let y = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
            show_menu(hwnd, x, y);
        }
        return 0;
    }
    let restarted = STATE.with(|s| s.borrow().as_ref().is_some_and(|state| message == state.taskbar_created));
    if restarted {
        let (added, shown) = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let state = s.as_mut().expect("the tray's state");
            state.added = false;
            state.update();
            (state.added, state.shown.clone())
        });
        shown(added);
        return 0;
    }
    unsafe { w::DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// Opens the menu at the icon and reports the item chosen. Windows wants
/// the window in front first, or the menu doesn't close on a click
/// elsewhere, and a message after, so it closes at once next time.
fn show_menu(hwnd: w::HWND, x: i32, y: i32) {
    let Some((entries, activate)) =
        STATE.with(|s| s.borrow().as_ref().map(|s| (s.data.menu.clone(), s.activate.clone())))
    else {
        return;
    };
    let mut titles = Vec::new();
    let menu = popup(&entries, &mut titles);
    let chosen = unsafe {
        _ = w::SetForegroundWindow(hwnd);
        let flags = w::TPM_RETURNCMD | w::TPM_RIGHTBUTTON | w::TPM_BOTTOMALIGN | w::TPM_RIGHTALIGN;
        let chosen = w::TrackPopupMenuEx(menu, flags as u32, x, y, hwnd, std::ptr::null());
        _ = w::PostMessageW(hwnd, w::WM_NULL as u32, 0, 0);
        _ = w::DestroyMenu(menu);
        chosen.0
    };
    drop(titles);
    if chosen > 0 {
        activate(chosen as u32);
    }
}

/// A popup menu of the entries. Titles stay in `titles` while it lives.
fn popup(entries: &[MenuEntry], titles: &mut Vec<Vec<u16>>) -> w::HMENU {
    let menu = unsafe { w::CreatePopupMenu() };
    for (position, entry) in entries.iter().enumerate() {
        let mut info = w::MENUITEMINFOW { cbSize: size_of::<w::MENUITEMINFOW>() as u32, ..Default::default() };
        let mut title = |text: &str| {
            // `&` marks an access key: a title's own is doubled.
            let mut wide: Vec<u16> = text.replace('&', "&&").encode_utf16().chain([0]).collect();
            let pointer = PWSTR(wide.as_mut_ptr());
            titles.push(wide);
            pointer
        };
        match entry {
            MenuEntry::Separator => {
                info.fMask = w::MIIM_FTYPE as u32;
                info.fType = w::MFT_SEPARATOR as u32;
            }
            MenuEntry::Item(item) => {
                info.fMask = (w::MIIM_ID | w::MIIM_STRING | w::MIIM_STATE | w::MIIM_FTYPE) as u32;
                info.wID = item.id;
                info.dwTypeData = title(&item.title);
                info.fType = w::MFT_STRING as u32;
                if !item.enabled {
                    info.fState |= w::MFS_DISABLED as u32;
                }
                match item.check {
                    MenuCheck::None => {}
                    MenuCheck::Check(on) => {
                        if on {
                            info.fState |= w::MFS_CHECKED as u32;
                        }
                    }
                    MenuCheck::Radio(on) => {
                        info.fType |= w::MFT_RADIOCHECK as u32;
                        if on {
                            info.fState |= w::MFS_CHECKED as u32;
                        }
                    }
                }
            }
            MenuEntry::Submenu(submenu) => {
                info.fMask = (w::MIIM_STRING | w::MIIM_SUBMENU | w::MIIM_FTYPE) as u32;
                info.fType = w::MFT_STRING as u32;
                info.dwTypeData = title(&submenu.title);
                info.hSubMenu = popup(&submenu.entries, titles);
            }
        }
        unsafe { _ = w::InsertMenuItemW(menu, position as u32, true.into(), &info) };
    }
    menu
}
